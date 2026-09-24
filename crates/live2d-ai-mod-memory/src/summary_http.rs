//! OpenAI 兼容的**摘要真实 HTTP 客户端**（P1-5）。
//!
//! 一句话：把 [crate::summary::Summarizer] 的同步契约落到一次
//! `POST {base_url}/chat/completions`（**非流式**，只要
//! `choices[0].message.content`）。
//!
//! # 口径（与导演二路 / 项目其余 HTTP 客户端一致）
//!
//! - **独立配置**：`summary_base_url` / `summary_model` / `summary_timeout_ms` /
//!   `summary_api_key_env`——**不读** `live2d-ai.toml` 的 `[llm]`（host 注入给
//!   Mod 的设置快照是**脱敏**的，根本没有 base_url / key 变量名，也不该有）；
//! - **TLS**：reqwest + `rustls-tls`（与 runtime / desktop 同款，无 native-tls）；
//! - **密钥**：一律走 [live2d_ai_runtime::secrets::lookup]（`.env` 快照 > 进程环境）；
//!   只进 `Authorization: Bearer` 头，**不进 URL / body / 日志**；
//! - **非流式**：`stream=false` + `temperature=0`——要的是一段摘要，不是逐字流。
//!
//! # 思考（reasoning_content）**不得进请求，也不得进摘要**
//!
//! 请求体只有 system + user 两条消息（与导演二路同一条纪律）；解析只读
//! `content`，`reasoning_content` 忽略。回归直接抓原始请求文本 + 只回思考的响应。
//!
//! # 失败 = None（静默回退）
//!
//! 连接失败 / 超时 / 非 2xx / 响应不是 JSON / `content` 缺失或空白 -> 一律
//! `None`，由调用方按「失败 = 无摘要」处理（旁车一字不改）。**不 panic**。
//!
//! # 阻塞性（诚实标注）
//!
//! `summarize` 是**同步**调用，会占用调用它的线程至多一个 `summary_timeout_ms`。
//! memory Mod **不在事件 worker 上直接调它**：`handle_turn_prompt` 只 `spawn`
//! 一个后台线程，主链本轮不等它（见 `lib.rs` 的 `spawn_summary`）。

use std::time::Duration;

use crate::summary::{self, Summarizer};

/// `/chat/completions` 路径（OpenAI 兼容约定的唯一后缀）。
const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

/// 拼 `base_url + path` 并校验协议（与 `live2d_ai_runtime::join_endpoint` 同语义）。
///
/// 这里保留一份最小副本：Mod 不依赖 runtime 的网络层，摘要客户端的错误口径是
/// `Result<_, String>`（`summary_base_url` 前缀由本层给出）；改 runtime 那份时同步对照。
pub fn join_endpoint(base: &str, path: &str) -> Result<reqwest::Url, String> {
    let base = base.trim();
    if base.is_empty() {
        return Err("summary_base_url 未配置".to_string());
    }
    let trimmed = base.trim_end_matches('/');
    let url = reqwest::Url::parse(&format!("{trimmed}{path}"))
        .map_err(|e| format!("summary_base_url 非法: {e}"))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(format!("summary_base_url 协议不支持: {other}")),
    }
}

/// OpenAI 兼容的同步摘要客户端。
pub struct OpenAiSummarizer {
    http: reqwest::blocking::Client,
    url: reqwest::Url,
    model: String,
    api_key: Option<String>,
}

impl OpenAiSummarizer {
    /// 生产构造：`api_key_env` 经 [live2d_ai_runtime::secrets::lookup] 解析
    /// （空名字 = 不鉴权）。缺 base_url / model -> Err（调用方转 Disabled + degraded）。
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

    /// 显式注入密钥的构造（测试 / 未来 host 替身）；生产走 [Self::new]。
    pub fn with_api_key(
        base_url: &str,
        model: &str,
        api_key: Option<String>,
        timeout_ms: u64,
    ) -> Result<Self, String> {
        let url = join_endpoint(base_url, CHAT_COMPLETIONS_PATH)?;
        let model = model.trim();
        if model.is_empty() {
            return Err("summary_model 未配置".to_string());
        }
        let http = summary::build_http_client(timeout_ms)
            .map_err(|e| format!("构造摘要 HTTP 客户端失败: {e}"))?;
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
}

impl Summarizer for OpenAiSummarizer {
    fn enabled(&self) -> bool {
        true
    }

    fn kind(&self) -> &'static str {
        "openai"
    }

    fn has_api_key(&self) -> bool {
        self.has_api_key()
    }

    fn endpoint(&self) -> &str {
        self.url.as_str()
    }

    fn summarize(&self, system: &str, user: &str, timeout_ms: u64) -> Option<String> {
        let timeout = Duration::from_millis(summary::clamp_timeout_ms(timeout_ms));
        // 请求体**只有** system + user 两条消息：不给它理由带上思考。
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "stream": false,
            "temperature": 0,
            "max_tokens": summary::MAX_TOKENS,
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
        summary::parse_completion(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::{SummaryConfig, assemble};
    use std::io::{Read, Write};
    use std::net::TcpListener;

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

    fn completion(content: &str) -> String {
        serde_json::json!({
            "choices": [{
                "message": {"role": "assistant", "content": content},
                "finish_reason": "stop"
            }],
            "reasoning_content": "SECRET_THOUGHT 不该出现在请求里"
        })
        .to_string()
    }

    #[test]
    fn client_posts_only_two_messages_and_parses_the_summary() {
        let (base, handle) = one_shot_server(200, completion("  用户喜欢薄荷。\n  "));
        let client = OpenAiSummarizer::with_api_key(
            &base,
            "summary-model",
            Some("sk-test-123".to_string()),
            3_000,
        )
        .expect("构造应成功");
        assert!(client.enabled());
        assert_eq!(client.kind(), "openai");
        assert!(client.has_api_key());
        assert!(client.endpoint().ends_with("/v1/chat/completions"));

        let out = client
            .summarize("SYS", "USER", 3_000)
            .expect("非流式响应应解析出摘要");
        assert_eq!(out, "用户喜欢薄荷。", "归一化：压平空白 + 去首尾");

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
        assert!(raw.contains("\"model\":\"summary-model\""));
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
    fn upstream_error_bad_json_and_reasoning_only_fall_back_to_none() {
        let (base, handle) = one_shot_server(500, r#"{"error":"boom"}"#.to_string());
        let client = OpenAiSummarizer::with_api_key(&base, "m", None, 2_000).expect("构造应成功");
        assert!(!client.has_api_key());
        assert_eq!(client.summarize("s", "u", 2_000), None);
        let raw = handle.join().expect("mock server 线程");
        assert!(!raw.to_ascii_lowercase().contains("authorization"), "{raw}");

        let (base, handle) = one_shot_server(200, "not json at all".to_string());
        let client = OpenAiSummarizer::with_api_key(&base, "m", None, 2_000).expect("构造应成功");
        assert_eq!(client.summarize("s", "u", 2_000), None);
        let _ = handle.join();

        // 只有思考、正文为空 -> 也算失败（不得把内心独白当摘要）。
        let (base, handle) = one_shot_server(
            200,
            r#"{"choices":[{"message":{"role":"assistant","content":"","reasoning_content":"想一想"}}]}"#
                .to_string(),
        );
        let client = OpenAiSummarizer::with_api_key(&base, "m", None, 2_000).expect("构造应成功");
        assert_eq!(client.summarize("s", "u", 2_000), None);
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
        assert!(OpenAiSummarizer::new("", "m", "", 1_000).is_err());
        assert!(OpenAiSummarizer::new("http://h/v1", "", "", 1_000).is_err());
        // 超时被钳到上限内（构造成功即证明没炸）。
        assert!(OpenAiSummarizer::new("http://h/v1", "m", "", 10_000_000).is_ok());
    }

    #[test]
    fn assemble_gate_off_is_disabled_and_missing_endpoint_is_degraded() {
        let off = assemble(&SummaryConfig {
            enabled: false,
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            model: "qwen".to_string(),
            api_key_env: String::new(),
            timeout_ms: 8_000,
            keep_recent: 4,
        });
        assert!(!off.client.enabled(), "关闸必须仍是 Disabled");
        assert_eq!(off.client.kind(), "disabled");
        assert!(off.degraded_note.is_none(), "关闸不是 degraded");

        let no_base = assemble(&SummaryConfig {
            enabled: true,
            base_url: String::new(),
            model: "m".to_string(),
            api_key_env: String::new(),
            timeout_ms: 8_000,
            keep_recent: 4,
        });
        assert!(!no_base.client.enabled());
        assert!(no_base.degraded_note.is_some(), "缺端点要可观察 degraded");

        let configured = assemble(&SummaryConfig {
            enabled: true,
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            model: "qwen".to_string(),
            api_key_env: String::new(),
            timeout_ms: 8_000,
            keep_recent: 4,
        });
        assert!(configured.client.enabled(), "配齐端点必须真接上");
        assert_eq!(configured.client.kind(), "openai");
    }
}
