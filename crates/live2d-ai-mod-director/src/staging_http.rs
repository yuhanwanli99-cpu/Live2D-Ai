//! OpenAI 兼容的异步第二路 LLM **真实 HTTP 客户端**（P1-4，2026-09-19）。
//!
//! 一句话：把 [`crate::staging::StagingClient`] 的同步契约落到一次
//! `POST {base_url}/chat/completions`（**非流式**，只要 `choices[0].message.content`）。
//!
//! # 口径（与项目其余 HTTP 客户端一致）
//!
//! - **独立配置**：`staging_base_url` / `staging_model` / `staging_timeout_ms` /
//!   `staging_api_key_env`——**不读** `live2d-ai.toml` 的 `[llm]`，二路断了不影响主链；
//! - **TLS**：reqwest + `rustls-tls`（与 runtime / desktop 同款，无 native-tls）；
//! - **密钥**：一律走 [`live2d_ai_runtime::secrets::lookup`]（`.env` 快照 > 进程
//!   环境）；只进 `Authorization: Bearer` 头，**不进 URL / body / 日志**；
//! - **非流式**：`stream=false` + 单次 `temperature=0`——导演要的是一份 JSON，
//!   不是逐字流。
//!
//! # 思考（reasoning_content）**不得进请求**
//!
//! 请求体只有两条消息（system + user），user 由
//! [`crate::staging::build_user_prompt`] 组装（本轮 epoch + 用户正文 +
//! 送 TTS 的助手正文 + 能力集）。**没有任何** `reasoning_content` / `reasoning`
//! 字段，也不把上游思考回灌。回归：`openai_client_posts_and_parses_content`
//! 直接抓原始请求文本断言。
//!
//! # 失败 = `None`（静默回退规则）
//!
//! 连接失败 / 超时 / 非 2xx / 响应不是 JSON / `content` 缺失或空白 →
//! 一律 `None`，由调用方（[crate::DirectorRuntime]）静默回退规则层并把计数写进
//! `state_json.staging.async_failures`。**不 panic**。
//!
//! # 阻塞性（诚实标注）
//!
//! `complete` 是同步调用，会占用 Mod 事件 worker 线程至多一个
//! `staging_timeout_ms`（缺省 1500ms，上限 5000ms）。supervisor 侧投递是
//! `try_send` **非阻塞**，所以主链不等待；但同一 worker 上排队的其它 Mod 事件
//! 会被这条调用推迟。这是 P1-3 同步契约的既有取舍，本波不改成异步。

use std::time::Duration;

use crate::staging::StagingClient;

/// `/chat/completions` 路径（OpenAI 兼容约定的唯一后缀）。
const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";
/// 超时下限（与 `DirectorConfig::from_value` 的钳位同口径）。
pub const MIN_TIMEOUT_MS: u64 = 100;
/// 超时上限。
pub const MAX_TIMEOUT_MS: u64 = 5_000;
/// 二路输出 token 上限（plan 很短；显式给硬上限，不靠提示词求模型守规矩）。
pub const MAX_TOKENS: u32 = 512;

/// 拼 `base_url + path` 并校验协议（与 `live2d_ai_runtime::join_endpoint` 同语义）。
///
/// 这里保留一份最小副本：Mod 不依赖 runtime 的网络层，二路客户端的错误口径是
/// `Result<_, String>`（`staging_base_url` 前缀由本层给出）；改 runtime 那份时同步对照。
pub fn join_endpoint(base: &str, path: &str) -> Result<reqwest::Url, String> {
    let base = base.trim();
    if base.is_empty() {
        return Err("staging_base_url 未配置".to_string());
    }
    let trimmed = base.trim_end_matches('/');
    let url = reqwest::Url::parse(&format!("{trimmed}{path}"))
        .map_err(|e| format!("staging_base_url 非法: {e}"))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(format!("staging_base_url 协议不支持: {other}")),
    }
}

/// 从 `chat.completion` JSON 里取 `choices[0].message.content`（trim 后非空才算成功）。
///
/// **只读正文**：`reasoning_content` / `reasoning` 即使存在也被忽略
/// （思考不进句子装配器，更不进 TTS）。
pub fn parse_completion(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let content = value
        .get("choices")?
        .as_array()?
        .first()?
        .get("message")?
        .get("content")?
        .as_str()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// OpenAI 兼容的同步二路客户端。
pub struct OpenAiStagingClient {
    http: reqwest::blocking::Client,
    url: reqwest::Url,
    model: String,
    api_key: Option<String>,
}

impl OpenAiStagingClient {
    /// 生产构造：`api_key_env` 经 [`live2d_ai_runtime::secrets::lookup`] 解析
    /// （空名字 = 不鉴权）。缺 base_url / model → `Err`（调用方转 Disabled + degraded）。
    pub fn new(
        base_url: &str,
        model: &str,
        api_key_env: &str,
        timeout_ms: u64,
    ) -> Result<Self, String> {
        let env_name = api_key_env.trim();
        let api_key = if env_name.is_empty() {
            None
        } else {
            live2d_ai_runtime::secrets::lookup(env_name)
        };
        Self::with_api_key(base_url, model, api_key, timeout_ms)
    }

    /// 显式注入密钥的构造（测试 / 未来 host 替身）；生产走 [`Self::new`]。
    pub fn with_api_key(
        base_url: &str,
        model: &str,
        api_key: Option<String>,
        timeout_ms: u64,
    ) -> Result<Self, String> {
        let url = join_endpoint(base_url, CHAT_COMPLETIONS_PATH)?;
        let model = model.trim();
        if model.is_empty() {
            return Err("staging_model 未配置".to_string());
        }
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(
                timeout_ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS),
            ))
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(|e| format!("构造二路 HTTP 客户端失败: {e}"))?;
        Ok(Self {
            http,
            url,
            model: model.to_string(),
            api_key,
        })
    }

    /// 是否解析出了一把 API key（布尔可以出门，值不可以）。
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// 目标端点（日志 / 排障用；不含密钥）。
    pub fn endpoint(&self) -> &str {
        self.url.as_str()
    }
}

impl StagingClient for OpenAiStagingClient {
    fn enabled(&self) -> bool {
        true
    }

    fn kind(&self) -> &'static str {
        "openai"
    }

    fn has_api_key(&self) -> bool {
        self.has_api_key()
    }

    fn complete(&self, system: &str, user: &str, timeout_ms: u64) -> Option<String> {
        let timeout = Duration::from_millis(timeout_ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS));
        // 请求体**只有** system + user 两条消息：不给它理由带上思考，也没有
        // reasoning_content 字段（见模块头注）。
        //
        // 2026-10-08：**显式关思考**——DeepSeek 官方 Chat Completions 的思考
        // 缺省是开的，不写字段等于让上游继续想（二路要的是一份 JSON）。
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "stream": false,
            "temperature": 0,
            "thinking": {"type": "disabled"},
            "max_tokens": MAX_TOKENS,
        });
        let mut request = self
            .http
            .post(self.url.clone())
            .timeout(timeout)
            .json(&body);
        if let Some(key) = self.api_key.as_deref() {
            request = request.bearer_auth(key);
        }
        let response = request.send().ok()?;
        if !response.status().is_success() {
            return None;
        }
        let text = response.text().ok()?;
        parse_completion(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DirectorConfig;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// 二路 plan 的正文（与 `plan::parse_plan` 契约一致）。
    const PLAN: &str =
        r#"{"epoch":9,"covers_upto_seq":2,"cues":[{"sentence_seq":1,"preset_id":"nod"}]}"#;

    /// 一次性 loopback HTTP 服务：返回 status + body，回传收到的原始请求文本。
    fn one_shot_server(
        status: u16,
        response_body: String,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 1024];
            loop {
                let n = match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                buf.extend_from_slice(&tmp[..n]);
                if let Some(head_end) = find_subslice(&buf, b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                    let cl = content_length(&head);
                    if buf.len() >= head_end + 4 + cl {
                        break;
                    }
                }
            }
            let raw = String::from_utf8_lossy(&buf).to_string();
            let reason = if status == 200 { "OK" } else { "ERR" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
            raw
        });
        (format!("http://{addr}/v1"), handle)
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn content_length(head: &str) -> usize {
        head.lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0)
    }

    #[test]
    fn openai_client_posts_and_parses_content() {
        let response = serde_json::json!({
            "choices": [{
                "message": {"role": "assistant", "content": PLAN},
                "finish_reason": "stop"
            }],
            "reasoning_content": "SECRET_THOUGHT 不该出现在请求里"
        })
        .to_string();
        let (base, handle) = one_shot_server(200, response);

        let client = OpenAiStagingClient::with_api_key(
            &base,
            "director-model",
            Some("sk-test-123".to_string()),
            3_000,
        )
        .expect("构造应成功");
        assert!(client.enabled());
        assert_eq!(client.kind(), "openai");
        assert!(client.has_api_key());
        assert!(client.endpoint().ends_with("/v1/chat/completions"));

        let out = client
            .complete("SYS", "USER", 3_000)
            .expect("非流式响应应解析出正文");
        assert_eq!(out, PLAN);
        assert!(crate::plan::parse_plan(&out, crate::PRESET_IDS).is_ok());

        let raw = handle.join().expect("mock server 线程");
        assert!(
            raw.starts_with("POST /v1/chat/completions"),
            "必须真发到 /chat/completions：{raw}"
        );
        assert!(
            raw.to_ascii_lowercase()
                .contains("authorization: bearer sk-test-123"),
            "密钥必须进 Authorization 头：{raw}"
        );
        assert!(raw.contains("\"stream\":false"), "非流式：{raw}");
        assert!(
            raw.contains("\"thinking\":{\"type\":\"disabled\"}"),
            "二路请求体必须显式关思考（缺省 = 上游继续想）：{raw}"
        );
        assert!(raw.contains("\"model\":\"director-model\""));
        assert!(
            !raw.contains("reasoning") && !raw.contains("SECRET_THOUGHT"),
            "请求体不得携带思考：{raw}"
        );
        assert_eq!(
            raw.matches("\"role\"").count(),
            2,
            "只有 system+user：{raw}"
        );
    }

    #[test]
    fn upstream_error_and_bad_json_fall_back_to_none() {
        let (base, handle) = one_shot_server(500, r#"{"error":"boom"}"#.to_string());
        let client =
            OpenAiStagingClient::with_api_key(&base, "m", None, 2_000).expect("构造应成功");
        assert!(!client.has_api_key());
        assert_eq!(client.complete("s", "u", 2_000), None);
        let raw = handle.join().expect("mock server 线程");
        assert!(!raw.to_ascii_lowercase().contains("authorization"), "{raw}");

        let (base, handle) = one_shot_server(200, "not json at all".to_string());
        let client =
            OpenAiStagingClient::with_api_key(&base, "m", None, 2_000).expect("构造应成功");
        assert_eq!(client.complete("s", "u", 2_000), None);
        let _ = handle.join();

        // 只有思考、正文为空 → 也算失败（不得把内心独白当正文）。
        let (base, handle) = one_shot_server(
            200,
            r#"{"choices":[{"message":{"role":"assistant","content":"","reasoning_content":"想一想"}}]}"#
                .to_string(),
        );
        let client =
            OpenAiStagingClient::with_api_key(&base, "m", None, 2_000).expect("构造应成功");
        assert_eq!(client.complete("s", "u", 2_000), None);
        let _ = handle.join();
    }

    #[test]
    fn join_endpoint_and_missing_config_are_classified() {
        assert!(join_endpoint("", "/x").is_err());
        assert!(join_endpoint("ftp://h/v1", "/x").is_err());
        assert_eq!(
            join_endpoint("http://h/v1/", "/chat/completions")
                .expect("合法")
                .as_str(),
            "http://h/v1/chat/completions"
        );
        assert!(OpenAiStagingClient::new("", "m", "", 1_000).is_err());
        assert!(OpenAiStagingClient::new("http://h/v1", "", "", 1_000).is_err());
        // 超时被钳到上限内（构造成功即证明没炸）。
        assert!(OpenAiStagingClient::new("http://h/v1", "m", "", 10_000_000).is_ok());
    }

    #[test]
    fn assemble_gate_off_is_disabled_and_missing_endpoint_is_degraded() {
        let off = crate::staging::assemble(&DirectorConfig {
            staging_enabled: false,
            staging_base_url: "http://127.0.0.1:11434/v1".to_string(),
            staging_model: "qwen".to_string(),
            ..DirectorConfig::default()
        });
        assert!(!off.client.enabled(), "关闸必须仍是 Disabled");
        assert_eq!(off.client.kind(), "disabled");
        assert!(off.degraded_note.is_none(), "关闸不是 degraded");

        let no_base = crate::staging::assemble(&DirectorConfig {
            staging_enabled: true,
            ..DirectorConfig::default()
        });
        assert!(!no_base.client.enabled());
        assert!(no_base.degraded_note.is_some(), "缺端点要可观察 degraded");

        let no_model = crate::staging::assemble(&DirectorConfig {
            staging_enabled: true,
            staging_base_url: "http://127.0.0.1:11434/v1".to_string(),
            ..DirectorConfig::default()
        });
        assert!(!no_model.client.enabled());
        assert!(no_model.degraded_note.is_some());

        let configured = crate::staging::assemble(&DirectorConfig {
            staging_enabled: true,
            staging_base_url: "http://127.0.0.1:11434/v1".to_string(),
            staging_model: "qwen".to_string(),
            ..DirectorConfig::default()
        });
        assert!(configured.client.enabled(), "配齐端点必须真接上");
        assert_eq!(configured.client.kind(), "openai");
    }
}
