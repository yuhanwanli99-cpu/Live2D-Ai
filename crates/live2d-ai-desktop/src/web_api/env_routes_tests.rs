//! `env_routes` 的单元测试（自 `env_routes.rs` 拆出，2026-10-16 D5 同批：该文件
//! 越 >500 行棘轮）。**行为一行未改**，只换宿主模块——断言、夹具、逐字相同。

#[cfg(test)]
mod tests {
    use tiny_http::Method;

    use live2d_ai_runtime::settings::AppSettings;

    use crate::web_api::dto::ApplyStatus;
    use crate::web_api::env_routes::*;
    use live2d_ai_runtime::settings::{LlmSettings, PersonaSettings, TtsSettings};

    fn settings_with(keys: (Option<&str>, Option<&str>)) -> AppSettings {
        let mut s = AppSettings {
            // 2026-09-16：action 段默认（幅度倍率）。
            action: Default::default(),
            // 2026-09-22：表演层默认关（[performance] 段，客户端在 host 侧构造）。
            performance: Default::default(),
            llm: LlmSettings {
                mode: live2d_ai_runtime::settings::LlmMode::Cloud,
                base_url: "http://127.0.0.1:1/v1".into(),
                model: "m".into(),
                api_key_env: keys.0.map(str::to_string),
                max_tokens: None,
                show_reasoning: None,
                vendor: None,
                vendors: Default::default(),
            },
            tts: TtsSettings {
                mode: live2d_ai_runtime::settings::TtsMode::Cloud,
                base_url: String::new(),
                model: None,
                voice: "v".into(),
                response_format: "pcm".into(),
                api_key_env: keys.1.map(str::to_string),
                sample_rate: 24_000,
                channels: 1,
                vendor: None,
                vendors: Default::default(),
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

    /// 写盘成功的假写入器：单测**绝不**碰仓库根的 `.env`（真 `write_key` 会写它）。
    fn ok_write(_key: &str, _value: &str) -> Result<(), String> {
        Ok(())
    }

    /// 写盘失败的假写入器（走 500 分支）。
    fn fail_write(_key: &str, _value: &str) -> Result<(), String> {
        Err("磁盘只读".to_string())
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
            handle_put(
                &s,
                &Method::Put,
                r#"{"key":"OTHER","value":"x"}"#,
                &ok_write,
                None,
            ),
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
            &fail_write,
            None,
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
            handle_put(
                &s,
                &Method::Put,
                r#"{"key":"OTHER","value":"x"}"#,
                &ok_write,
                None,
            ),
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
        let err = expect_err(
            handle_put(&s, &Method::Put, "not json", &ok_write, None),
            "坏 body",
        );
        assert_eq!(err.status_code().0, 400);
        assert!(body(err).contains("invalid_payload"));

        let err = expect_err(
            handle_put(&s, &Method::Get, "{}", &ok_write, None),
            "GET 不接受",
        );
        assert_eq!(err.status_code().0, 405);
    }

    /// FINDINGS.md S-002（2026-10-16）：**写盘成功 ≠ 引擎装上**。
    ///
    /// 响应必须把热重载状态带出来；只有 `applied` 才配叫「立即生效」。从前响应只有
    /// `{key, set}`，界面只能写死「已写入 KEY，立即生效（不用重启）」——密钥落进
    /// `.env` 了，链路却还拿着旧 client。
    #[test]
    fn put_reports_the_reload_status_not_only_the_write() {
        let s = settings_with((Some("L2D_S002_KEY"), None));
        let body_json = r#"{"key":"L2D_S002_KEY","value":"sk-x"}"#;

        // 1) 写盘成功、引擎没装上：两件事分开说，且热重载那一步只走一次。
        let called = std::cell::Cell::new(0u32);
        let restart = || {
            called.set(called.get() + 1);
            ApplyStatus::RestartRequired
        };
        let resp = match handle_put(&s, &Method::Put, body_json, &ok_write, Some(&restart)) {
            Ok(r) => r,
            Err(_) => panic!("写盘成功必须回 200"),
        };
        assert_eq!(resp.status_code().0, 200);
        let text = body(resp);
        assert!(
            text.contains("\"apply_status\":\"restart_required\""),
            "write ok but engine not reloaded must be said separately: {text}"
        );
        assert!(text.contains("\"key\":\"L2D_S002_KEY\""), "{text}");
        assert_eq!(called.get(), 1, "热重载那一步只能走一次");

        // 2) 真的装上了：只有这一种取值配「立即生效」。
        let applied = || ApplyStatus::Applied;
        let resp = match handle_put(&s, &Method::Put, body_json, &ok_write, Some(&applied)) {
            Ok(r) => r,
            Err(_) => panic!("写盘成功必须回 200"),
        };
        assert!(
            body(resp).contains("\"apply_status\":\"applied\""),
            "applied must be reported as applied"
        );

        // 3) 没有 supervisor（纯控制平面）：**不得**回落成 applied。
        let resp = match handle_put(&s, &Method::Put, body_json, &ok_write, None) {
            Ok(r) => r,
            Err(_) => panic!("写盘成功必须回 200"),
        };
        assert!(
            body(resp).contains("\"apply_status\":\"no_supervisor\""),
            "缺钩子不得谎报已生效"
        );

        // 4) 写盘失败：钩子一次都不许被调（没落盘就谈不上生效）。
        let boom = || panic!("写盘失败时不得触碰热重载");
        let err = expect_err(
            handle_put(&s, &Method::Put, body_json, &fail_write, Some(&boom)),
            "写盘失败必须 500",
        );
        assert_eq!(err.status_code().0, 500);
        assert!(body(err).contains("persist_failed"));
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
                &ok_write,
                None,
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
