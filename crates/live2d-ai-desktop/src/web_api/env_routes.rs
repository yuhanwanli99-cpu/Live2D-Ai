//! 密钥真源端点（rc.2 2026-09-12）：`GET /api/v1/env` + `PUT /api/v1/env`。
//!
//! # 契约
//!
//! | 端点 | 作用 | 回显 |
//! |---|---|---|
//! | `GET /api/v1/env` | 列出**配置里声明的**键名（`llm` / `tts` / `performance` 的 `api_key_env`）与「是否已设置」 | **只回键名 + 布尔，永不回值** |
//! | `PUT /api/v1/env` | 写 `.env` 的单个键（`{"key":"…","value":"…"}`），写完热重载 | 只回 `{key, set}` |
//!
//! # 为什么单开一个端点而不是塞进 `PATCH /api/v1/settings`
//!
//! settings 是**可导出、可展示、可备份**的配置（`live2d-ai.toml`）；
//! 密钥不是——它住在 `.env`（gitignored、`0600`）。把写密钥混进 settings 的
//! PATCH，就等于让「保存配置」这条日常路径顺带碰密钥文件，语义与风险都不对。
//!
//! # 安全
//!
//! - `PUT` 在 `is_mutating_route` 里 → 由 dispatch 入口做 Origin + Content-Type 校验；
//! - 审计日志只记 `key`，**不记 value**（`dispatch` 本来就不记 body；
//!   这里额外保证错误信息里也不出现 value）；
//! - `GET` 只回是否已设置（[`live2d_ai_runtime::secrets::is_set`]）。
//!
//! # 值从哪里来
//!
//! 键名**不是**前端随便填的：`GET` 返回的就是配置里 `api_key_env` 指向的那个名字。
//! 前端照着填/写，因此不存在「前端写了一个后端不读的变量名」这种静默失效。
//!
//! 键名清单的**唯一真源**是 [`AppSettings::declared_key_envs`]——本模块的
//! `GET` 与 `PUT`（白名单）都从那一份结果推导，**不得**再各自抄一遍段清单。
//! 以前两边各抄 `llm` + `tts`，于是 `[performance].api_key_env` 既列不出、
//! 也写不进（PUT 回 `400 unknown_key`）。

use std::io::Cursor;

use serde::Deserialize;
use tiny_http::{Header, Method, Response, StatusCode};

use live2d_ai_runtime::settings::AppSettings;

use crate::web_api::app_routes::json_response;

/// HTTP 响应类型别名：`tiny_http::Response<Cursor<Vec<u8>>>` 长到 clippy 会抱怨
/// `type_complexity`，而这个文件里每个 handler 都要写它两遍。
type Resp = Response<Cursor<Vec<u8>>>;

/// `GET /api/v1/env` 的单条。
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct EnvKeyEntry {
    /// 归属段（`"llm"` / `"tts"` / `"performance"`）。
    pub section: &'static str,
    /// 环境变量名（来自配置的 `api_key_env`）。
    pub key: String,
    /// 该键**是否有非空值**（`.env` 快照或进程环境）。
    pub set: bool,
}

/// `GET /api/v1/env` 响应。
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct EnvListResponse {
    /// 配置里声明的键（没有 `api_key_env` 的段不出现）。
    pub keys: Vec<EnvKeyEntry>,
    /// `.env` 路径（**不是**密钥，是排障必需的：用户要知道该改哪个文件）。
    pub env_file: String,
}

/// `PUT /api/v1/env` 请求体。
#[derive(Debug, Clone, Deserialize)]
pub struct EnvWriteRequest {
    /// 环境变量名（必须是配置里声明的那个，见 [`handle_get`]）。
    pub key: String,
    /// 新值。**空串 = 删除该键**。
    #[serde(default)]
    pub value: String,
}

/// `PUT /api/v1/env` 响应（**不回显 value**）。
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct EnvWriteResponse {
    pub key: String,
    /// 写完之后该键是否已设置（空值写入 = 删除 = `false`）。
    pub set: bool,
}

/// `GET /api/v1/env`：配置声明的键名 + 是否已设置。
pub fn handle_get(settings: &AppSettings) -> Resp {
    // 唯一真源：`AppSettings::declared_key_envs()`（顺序 llm → tts → performance）。
    // 这里**不**枚举段清单——多抄一遍就多一处「新增段忘了同步」的缺口。
    let keys: Vec<EnvKeyEntry> = settings
        .declared_key_envs()
        .into_iter()
        .map(|d| EnvKeyEntry {
            section: d.section,
            set: live2d_ai_runtime::secrets::is_set(&d.name),
            key: d.name,
        })
        .collect();
    json_response(
        StatusCode(200),
        &EnvListResponse {
            keys,
            env_file: live2d_ai_runtime::secrets::env_file_path()
                .display()
                .to_string(),
        },
    )
}

/// `PUT /api/v1/env`：写 `.env` 的单个键；成功后由 caller 触发 supervisor 热重载。
///
/// 返回 `Ok(response)` = 已落盘；`Err(response)` = 400/500，**错误信息里不含 value**。
pub fn handle_put(settings: &AppSettings, method: &Method, body_str: &str) -> Result<Resp, Resp> {
    if *method != Method::Put {
        return Err(method_not_allowed());
    }
    let req: EnvWriteRequest = match serde_json::from_str(body_str) {
        Ok(r) => r,
        Err(e) => {
            return Err(bad_request(
                "invalid_payload",
                &format!("请求 body JSON 解析失败: {e}"),
            ));
        }
    };
    // 键名白名单 = 配置里声明的那些（**与 `GET` 同一份真源**：
    // `AppSettings::declared_key_envs`）。不校验的话，前端可以往 `.env` 里写任意
    // 变量名（无害但会让「写了却没生效」变成常态，且 .env 会慢慢长满垃圾）。
    let declared = settings.declared_key_envs();
    if !declared.iter().any(|d| d.name == req.key) {
        return Err(bad_request(
            "unknown_key",
            &format!(
                "配置里没有声明这个键名（可选：{}）；键名来自 [llm]/[tts]/[performance] 的 api_key_env",
                if declared.is_empty() {
                    "无".to_string()
                } else {
                    declared
                        .iter()
                        .map(|d| d.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
        ));
    }
    if let Err(e) = live2d_ai_runtime::secrets::write_key(&req.key, &req.value) {
        // `e` 来自写盘/校验路径，**不含 value**（写盘失败信息只有路径与 io 错误）。
        return Err(json_error(StatusCode(500), "persist_failed", &e));
    }
    let set = live2d_ai_runtime::secrets::is_set(&req.key);
    tracing::info!(key = %req.key, set, "env 写入成功（值不记录）");
    Ok(json_response(
        StatusCode(200),
        &EnvWriteResponse { key: req.key, set },
    ))
}

/// 非 GET/PUT 的统一 405（dispatch 直接调用）。
pub(crate) fn method_not_allowed() -> Resp {
    json_error(
        StatusCode(405),
        "method_not_allowed",
        "`/api/v1/env` 只支持 GET（读键名）与 PUT（写入）",
    )
}

fn bad_request(code: &'static str, message: &str) -> Resp {
    json_error(StatusCode(400), code, message)
}

fn json_error(status: StatusCode, code: &'static str, message: &str) -> Resp {
    let body = serde_json::json!({"error": {"code": code, "message": message}}).to_string();
    Response::from_data(body.into_bytes())
        .with_status_code(status)
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use live2d_ai_runtime::settings::{LlmSettings, PersonaSettings, TtsSettings};

    fn settings_with(keys: (Option<&str>, Option<&str>)) -> AppSettings {
        let mut s = AppSettings {
            // 2026-09-16：action 段默认（幅度倍率）。
            action: Default::default(),
            // 2026-09-22：表演层默认关（[performance] 段，客户端在 host 侧构造）。
            performance: Default::default(),
            llm: LlmSettings {
                base_url: "http://127.0.0.1:1/v1".into(),
                model: "m".into(),
                api_key_env: keys.0.map(str::to_string),
                max_tokens: None,
                show_reasoning: None,
            },
            tts: TtsSettings {
                base_url: String::new(),
                model: None,
                voice: "v".into(),
                response_format: "pcm".into(),
                api_key_env: keys.1.map(str::to_string),
                sample_rate: 24_000,
                channels: 1,
            },
            persona: PersonaSettings::default(),
            dev_mode: false,
        };
        s.persona = PersonaSettings::default();
        s
    }

    /// `Response` 没有 `Debug`，所以不能直接用 `expect_err`。
    fn expect_err(res: Result<Resp, Resp>, msg: &str) -> Resp {
        match res {
            Ok(_) => panic!("{msg}"),
            Err(r) => r,
        }
    }

    fn body(resp: Resp) -> String {
        use std::io::Read;
        let mut s = String::new();
        let _ = resp.into_reader().read_to_string(&mut s);
        s
    }

    /// **永不回显**：GET 只给键名与布尔。
    #[test]
    fn get_never_returns_values() {
        let s = settings_with((Some("L2D_TEST_LLM_KEY"), Some("L2D_TEST_TTS_KEY")));
        let resp = handle_get(&s);
        assert_eq!(resp.status_code().0, 200);
        let text = body(resp);
        assert!(text.contains("L2D_TEST_LLM_KEY"), "{text}");
        assert!(text.contains("L2D_TEST_TTS_KEY"), "{text}");
        assert!(text.contains("\"set\":"), "{text}");
        // 夹具里这两个变量不可能有值；关键是没有 "value" 字段。
        assert!(!text.contains("\"value\""), "不得回显值：{text}");
    }

    #[test]
    fn get_omits_sections_without_a_key_name() {
        let s = settings_with((None, None));
        let text = body(handle_get(&s));
        assert!(text.contains("\"keys\":[]"), "{text}");
    }

    #[test]
    fn put_rejects_undeclared_key() {
        let s = settings_with((Some("L2D_DECLARED"), None));
        let err = expect_err(
            handle_put(&s, &Method::Put, r#"{"key":"OTHER","value":"x"}"#),
            "未声明的键必须被拒",
        );
        assert_eq!(err.status_code().0, 400);
        let text = body(err);
        assert!(text.contains("unknown_key"), "{text}");
        // 提示里带上可选键名，用户不用猜。
        assert!(text.contains("L2D_DECLARED"), "{text}");
    }

    /// P1 回归：`[performance].api_key_env` 必须进 GET 列表
    ///（旧实现只枚举 `llm` + `tts`，performance 的键永远列不出来）。
    #[test]
    fn get_lists_performance_section() {
        let mut s = settings_with((None, None));
        s.performance.api_key_env = Some("L2D_TEST_PERF_KEY".into());
        let text = body(handle_get(&s));
        assert!(text.contains("\"section\":\"performance\""), "{text}");
        assert!(text.contains("L2D_TEST_PERF_KEY"), "{text}");
        assert!(!text.contains("\"value\""), "不得回显值：{text}");
    }

    /// P1 回归：`[performance]` 声明的键必须过 PUT 白名单
    ///（旧实现把 `llm`/`tts` 各抄一遍，写 performance 的键一律回
    /// `400 unknown_key`）。
    ///
    /// 用**含换行的值**驱动接受路径：它会过白名单，然后在写盘层因
    ///「值不允许换行」失败（500），**不碰仓库里的 `.env`**（测试进程的 cwd
    /// 就长在源码树里；指向临时 `.env` 要 `set_var`，而本项目 `unsafe_code`
    /// 是 deny 且明令禁止用进程环境做密钥真源）。断言的是「不再被白名单
    /// 拒绝」，即本任务验收点；真正的 200 落盘路径由点火端到端验收覆盖。
    #[test]
    fn put_no_longer_rejects_performance_declared_key() {
        let mut s = settings_with((None, None));
        s.performance.api_key_env = Some("L2D_TEST_PERF_KEY".into());
        let resp = match handle_put(
            &s,
            &Method::Put,
            r#"{"key":"L2D_TEST_PERF_KEY","value":"a\nb"}"#,
        ) {
            Ok(r) => r,
            Err(r) => r,
        };
        assert_ne!(
            resp.status_code().0,
            400,
            "performance 键名不该被白名单拒绝"
        );
        assert!(!body(resp).contains("unknown_key"), "不得回 unknown_key");
    }

    /// 白名单收紧不变：即使 performance 已声明，未声明的键仍是 400
    /// `unknown_key`，且错误文案要列出**全部**候选键名（llm + performance）。
    #[test]
    fn put_still_rejects_undeclared_with_performance_declared() {
        let mut s = settings_with((Some("L2D_LLM_DECLARED"), None));
        s.performance.api_key_env = Some("L2D_TEST_PERF_KEY".into());
        let err = expect_err(
            handle_put(&s, &Method::Put, r#"{"key":"OTHER","value":"x"}"#),
            "未声明的键必须被拒",
        );
        assert_eq!(err.status_code().0, 400);
        let text = body(err);
        assert!(text.contains("unknown_key"), "{text}");
        assert!(text.contains("L2D_LLM_DECLARED"), "{text}");
        assert!(text.contains("L2D_TEST_PERF_KEY"), "{text}");
        assert!(text.contains("performance"), "候选来源要写明: {text}");
    }

    #[test]
    fn put_rejects_bad_json_and_wrong_method() {
        let s = settings_with((Some("L2D_X"), None));
        let err = expect_err(handle_put(&s, &Method::Put, "not json"), "坏 body");
        assert_eq!(err.status_code().0, 400);
        assert!(body(err).contains("invalid_payload"));

        let err = expect_err(handle_put(&s, &Method::Get, "{}"), "GET 不接受");
        assert_eq!(err.status_code().0, 405);
    }

    /// 错误信息里**不得**出现 value（密钥泄漏最常见的位置就是报错文案）。
    #[test]
    fn error_messages_never_contain_the_value() {
        let s = settings_with((Some("L2D_X"), None));
        let secret = "sk-super-secret-value";
        let err = expect_err(
            handle_put(
                &s,
                &Method::Put,
                &format!(r#"{{"key":"NOT_DECLARED","value":"{secret}"}}"#),
            ),
            "未声明键",
        );
        assert!(!body(err).contains(secret), "错误信息泄漏了 value");
    }
}

#[cfg(test)]
mod route_tests {
    use crate::web_api::{RouteId, match_route};
    use tiny_http::Method;

    #[test]
    fn env_route_matches_get_and_put_only_by_path() {
        // 路径匹配不按 method 分叉：非 GET/PUT 也路由到这里，由 handler 返 405。
        // （若落到 NotImplemented，用户看到的是「未实现」而不是「方法不对」。）
        assert_eq!(match_route(&Method::Get, "/api/v1/env"), RouteId::Env);
        assert_eq!(match_route(&Method::Put, "/api/v1/env"), RouteId::Env);
        assert_eq!(match_route(&Method::Post, "/api/v1/env"), RouteId::Env);
        assert_eq!(
            match_route(&Method::Get, "/api/v1/env/extra"),
            RouteId::NotImplemented
        );
    }

    /// 写密钥是 mutating（要过 Origin + Content-Type 门禁）；读不是。
    #[test]
    fn env_put_is_mutating_get_is_not() {
        use crate::web_api::security::is_mutating_route;
        assert!(is_mutating_route(RouteId::Env, &Method::Put));
        assert!(!is_mutating_route(RouteId::Env, &Method::Get));
        assert!(!is_mutating_route(RouteId::Env, &Method::Post));
    }
}
