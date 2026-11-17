//! `POST /api/v1/voice/asr` 的回归（2026-11-16）。
//!
//! 全部用 `127.0.0.1` 上的假上游，**从不连真的 `api.deepseek.com`**。
//!
//! 守的六件事（对应 §4 V3 的验收清单）：
//! 1. 假上游收到的是 **multipart**，且含 `file` 字段；
//! 2. 响应 `text` **去首尾空白后**回到客户端，`duration_ms` 原样透传；
//! 3. 8 MiB + 1 字节的请求**不出现在假上游的访问日志里**（本机 413）；
//! 4. 钥匙为空时**不访问假上游**（503 `asr_unconfigured`）；
//! 5. 路由处理函数里**搜不到 `say` 调用**（识别成功不自动开一轮对话）；
//! 6. 超过 120 秒的 WAV 在本机 400（不打上游）。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tiny_http::{Method, StatusCode};

use live2d_ai_runtime::AppSettings;

use super::*;

/// 假上游收到的一条请求（只要断言需要的两项）。
#[derive(Debug, Clone)]
struct Seen {
    head: String,
    body: Vec<u8>,
}

impl Seen {
    fn is_multipart(&self) -> bool {
        self.head
            .to_ascii_lowercase()
            .contains("content-type: multipart/form-data; boundary=----live2d-ai-asr-")
    }
}

/// 起一个假上游（标准库线程 + TcpListener，不引第三方 mock）。
///
/// 返回 `(endpoint, seen)`：`endpoint` 直接喂给 `transcribe`，
/// `seen` 是**访问日志**（每收到一条请求记一条）。
fn spawn_fake(response_body: &'static str) -> (String, Arc<Mutex<Vec<Seen>>>) {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::new(Mutex::new(Vec::new()));
    let recorded = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            let mut buf = Vec::new();
            let mut tmp = [0u8; 8192];
            // 读头
            let mut header_end = 0usize;
            loop {
                if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = p + 4;
                    break;
                }
                match sock.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                }
            }
            if header_end == 0 {
                continue;
            }
            let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
            let len = head
                .lines()
                .find_map(|line| {
                    let (k, v) = line.split_once(':')?;
                    k.trim()
                        .eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            let mut body = buf[header_end..].to_vec();
            while body.len() < len {
                match sock.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => body.extend_from_slice(&tmp[..n]),
                }
            }
            recorded.lock().unwrap().push(Seen {
                head,
                body: body.clone(),
            });
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                response_body.len()
            );
            let _ = sock.write_all(head.as_bytes());
            let _ = sock.write_all(response_body.as_bytes());
        }
    });
    // 给监听线程一点起步时间（bind 已完成，这里只是让 accept 就绪）。
    std::thread::sleep(Duration::from_millis(30));
    (format!("http://{addr}/v1/tools/asr"), seen)
}

/// 一份最小的 16 位 PCM WAV（秒数可调）。
fn wav(seconds: u32, rate: u32) -> Vec<u8> {
    let data_len = rate * 2 * seconds;
    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVE");
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    v
}

/// 只读响应体 JSON。
fn body_json(resp: Resp) -> serde_json::Value {
    use std::io::Read as _;
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).expect("读响应体");
    serde_json::from_slice(&buf).expect("响应体必须是 JSON")
}

fn ctx_with_key_env(name: &str) -> ServerContext {
    let mut settings = AppSettings::default();
    settings.llm.base_url = "http://127.0.0.1:9/v1".into();
    settings.asr.api_key_env = Some(name.to_string());
    ServerContext::new(settings, ".scratch".to_string(), None)
}

// ① 假上游收到的是 multipart，且含 file 字段。
#[test]
fn asr_upstream_receives_multipart_with_a_file_field() {
    let (endpoint, seen) = spawn_fake(r#"{"text":"你好。"}"#);
    let upload = deepseek::AsrUpload {
        bytes: &wav(1, 16_000),
        container: AudioContainer::Wav,
    };
    let http = reqwest::Client::new();
    let got = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt")
        .block_on(deepseek::transcribe(&http, "sk-test", &upload, &endpoint))
        .expect("上游应当成功");
    assert_eq!(got.text, "你好。");

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "必须恰好打过一次上游");
    assert!(seen[0].is_multipart(), "必须是 multipart：{}", seen[0].head);
    let body = String::from_utf8_lossy(&seen[0].body);
    assert!(
        body.contains("form-data; name=\"file\""),
        "必须带 file 字段：{body}"
    );
    assert!(body.contains("Content-Type: audio/wav"), "必须声明容器类型");
}

// ② 响应 text 去首尾空白后回到客户端；duration_ms 原样透传。
#[test]
fn asr_response_text_is_trimmed_and_duration_passes_through() {
    let (endpoint, _seen) = spawn_fake(r#"{"text":"  你好，世界。  ","duration_ms":4321}"#);
    let upload = deepseek::AsrUpload {
        bytes: &wav(1, 16_000),
        container: AudioContainer::Wav,
    };
    let http = reqwest::Client::new();
    let parsed = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt")
        .block_on(deepseek::transcribe(&http, "sk-test", &upload, &endpoint))
        .expect("上游应当成功");
    let body = serde_json::to_value(deepseek::AsrOkBody::from(parsed)).expect("json");
    assert_eq!(body["ok"], true);
    assert_eq!(body["text"], "你好，世界。", "只去首尾空白");
    assert_eq!(body["duration_ms"], 4321);
}

// ③ 8 MiB + 1 字节：本机 413，且**不出现在假上游的访问日志里**。
#[test]
fn asr_oversize_body_is_rejected_locally_and_never_reaches_upstream() {
    let (endpoint, seen) = spawn_fake(r#"{"text":"不该到这里"}"#);
    // 直接调 `transcribe`：它才是「本机判据 → 转发」那段。
    let oversized = vec![0u8; MAX_UPLOAD_BYTES + 1];
    let upload = deepseek::AsrUpload {
        bytes: &oversized,
        container: AudioContainer::Wav,
    };
    let http = reqwest::Client::new();
    let err = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt")
        .block_on(deepseek::transcribe(&http, "sk-test", &upload, &endpoint))
        .expect_err("超限必须在本机就失败");
    assert!(err.to_string().contains("本机就被拒"), "{err}");
    assert!(
        seen.lock().unwrap().is_empty(),
        "超限请求不得出现在上游访问日志里"
    );
}

// ④ 钥匙为空：不访问假上游（503 asr_unconfigured）。
#[test]
fn asr_without_a_key_never_contacts_upstream() {
    let (endpoint, seen) = spawn_fake(r#"{"text":"不该到这里"}"#);
    // [asr] 段根本没声明变量名 → asr_key 为 None。
    let ctx = ServerContext::new(AppSettings::default(), ".scratch".to_string(), None);
    let resp = super::transcribe(&ctx, AudioContainer::Wav, wav(1, 16_000), &endpoint);
    assert_eq!(resp.status_code(), StatusCode(503));
    let body = body_json(resp);
    assert_eq!(body["error"]["code"], "asr_unconfigured");
    assert!(
        seen.lock().unwrap().is_empty(),
        "没配钥匙时一个字节都不该发给上游"
    );
}

// ⑤ 路由处理函数里搜不到 say 调用（识别成功不自动开一轮对话）。
#[test]
fn asr_route_never_calls_say() {
    // 源码扫描：`voice_asr.rs` 的实现（剥掉注释）里不得出现 `supervisor` /
    // `.say(` —— 这条路径的产物是**文本**，不是一轮对话。
    let src = include_str!("voice_asr.rs");
    let code = strip_comments(src);
    assert!(
        !code.contains(".say("),
        "ASR 端点不得调用 say：识别成功不自动开一轮对话"
    );
    assert!(
        !code.contains("supervisor"),
        "ASR 端点不读 supervisor（产物是文本，不是一轮对话）"
    );
}

// ⑥ 超过 120 秒的 WAV：本机 400，不打上游。
#[test]
fn asr_overlong_wav_is_rejected_locally() {
    let (endpoint, seen) = spawn_fake(r#"{"text":"不该到这里"}"#);
    let ctx = ctx_with_key_env("L2D_ASR_TEST_KEY_UNSET");
    // 121 秒 @ 16 kHz。
    let resp = super::transcribe(&ctx, AudioContainer::Wav, wav(121, 16_000), &endpoint);
    assert_eq!(resp.status_code(), StatusCode(400));
    let body = body_json(resp);
    assert_eq!(body["error"]["code"], "asr_too_long");
    assert!(seen.lock().unwrap().is_empty(), "超长音频不得发给上游");
}

// 路径之外的请求交还给 `run_request_loop`。
#[test]
fn asr_handler_ignores_other_paths() {
    let ctx = ServerContext::new(AppSettings::default(), ".scratch".to_string(), None);
    for path in [
        "/api/v1/voice/transcript",
        "/api/v1/voice/asr/extra",
        "/api/v1/chat",
    ] {
        assert!(
            super::handle_voice_asr(
                &ctx,
                &Method::Post,
                path,
                Vec::new(),
                None,
                Some("audio/wav")
            )
            .is_none(),
            "{path} 不归本模块管"
        );
    }
}

// 方法不对 → 405；Content-Type 不对 → 415（都在读 body 之后、判据之前）。
#[test]
fn asr_rejects_wrong_method_and_wrong_content_type() {
    let ctx = ServerContext::new(AppSettings::default(), ".scratch".to_string(), None);
    let resp = super::handle_voice_asr(
        &ctx,
        &Method::Get,
        super::VOICE_ASR_PATH,
        Vec::new(),
        None,
        Some("audio/wav"),
    )
    .expect("路径命中");
    assert_eq!(resp.status_code(), StatusCode(405));

    let resp = super::handle_voice_asr(
        &ctx,
        &Method::Post,
        super::VOICE_ASR_PATH,
        Vec::new(),
        None,
        Some("application/json"),
    )
    .expect("路径命中");
    assert_eq!(resp.status_code(), StatusCode(415));
}

// Origin 不同源 → 403（与 /api/v1/chat 同一套判据）。
#[test]
fn asr_rejects_cross_origin_requests() {
    let ctx = ServerContext::new(AppSettings::default(), ".scratch".to_string(), None);
    let resp = super::handle_voice_asr(
        &ctx,
        &Method::Post,
        super::VOICE_ASR_PATH,
        wav(1, 16_000),
        Some("http://evil.example"),
        Some("audio/wav"),
    )
    .expect("路径命中");
    assert_eq!(resp.status_code(), StatusCode(403));
    let body = body_json(resp);
    assert_eq!(body["error"]["code"], "origin_denied");
}

/// 剥掉 `//` 行注释（源码扫描门禁用；块注释本项目几乎不用，这里一并处理）。
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_block = false;
    for line in src.lines() {
        let trimmed = line.trim_start();
        if in_block {
            if trimmed.contains("*/") {
                in_block = false;
            }
            out.push('\n');
            continue;
        }
        if trimmed.starts_with("/*") {
            if !trimmed.contains("*/") {
                in_block = true;
            }
            out.push('\n');
            continue;
        }
        match line.find("//") {
            Some(pos) => {
                // 只在「// 之前没有引号」时当注释（本文件里 URL 走 include_str，
                // 不用字面量，所以简单判就够）。
                out.push_str(&line[..pos]);
                out.push('\n');
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}
