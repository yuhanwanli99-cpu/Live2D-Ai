//! `POST /api/v1/voice/transcript` **退休**回归（2026-11-16）。
//!
//! 守三件事：
//! 1. 路径命中 → `410` 且 `error.code == "voice_retired"`（**不再是** `mod_disabled`）；
//! 2. 响应正文直接给出新地址 `/api/v1/voice/asr`（旧 sidecar 不会被误导成
//!    「把 Mod 打开就好」）；
//! 3. 路径不命中 → `None`（交还 dispatch）。
//!
//! supervisor / 密钥 / body 一概不读：这条路已经没有实现。

use std::io::Read;

use tiny_http::{Method, StatusCode};

use live2d_ai_runtime::AppSettings;

use super::*;

fn ctx_without_supervisor() -> ServerContext {
    ServerContext::new(AppSettings::default(), ".scratch".to_string(), None)
}

fn body_json(resp: Response<Cursor<Vec<u8>>>) -> serde_json::Value {
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).expect("读响应体");
    serde_json::from_slice(&buf).expect("410 body 必须是 JSON")
}

#[test]
fn voice_transcript_is_410_voice_retired_on_every_method() {
    let ctx = ctx_without_supervisor();
    assert!(
        ctx.try_get_supervisor().is_none(),
        "前提：supervisor 未就绪（退休端点不依赖它）"
    );
    for method in [Method::Post, Method::Get, Method::Put] {
        let resp = handle_voice_transcript(
            &ctx,
            &method,
            VOICE_TRANSCRIPT_PATH,
            r#"{"text":"小可爱 你好"}"#,
            None,
            Some("application/json"),
            None,
        )
        .expect("路径命中必须返回响应");
        assert_eq!(resp.status_code(), StatusCode(410), "{method:?}");
        let value = body_json(resp);
        assert_eq!(value["error"]["code"], "voice_retired", "body = {value}");
        assert_ne!(
            value["error"]["code"], "mod_disabled",
            "退休端点不得再回 mod_disabled（那会把旧 sidecar 引到「开 Mod」上）"
        );
        let message = value["error"]["message"].as_str().unwrap_or_default();
        assert!(
            message.contains("/api/v1/voice/asr"),
            "必须直接给出新端点：{message}"
        );
    }
}

#[test]
fn voice_transcript_other_paths_return_none() {
    let ctx = ctx_without_supervisor();
    for path in [
        "/api/v1/voice/transcript/extra",
        "/api/v1/voice",
        "/api/v1/voice/asr",
        "/api/v1/external/chat",
        "/api/v1/chat",
    ] {
        assert!(
            handle_voice_transcript(&ctx, &Method::Post, path, "", None, None, None).is_none(),
            "{path} 不归 voice_routes 管（必须交还 dispatch）"
        );
    }
}
