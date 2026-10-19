//! `voice_routes` 未挂载存根回归（2026-10-19）。
//!
//! 守三件事：
//! 1. 路径命中 → `404` 且 `error.code == "mod_disabled"`；
//! 2. supervisor **未就绪**时同样是 404（证明存根不打 LLM、不 `say`）；
//! 3. 路径不命中 → `None`（交还 dispatch）。

use std::io::Read;

use tiny_http::{Method, StatusCode};

use live2d_ai_runtime::AppSettings;

use super::*;

fn ctx_without_supervisor() -> ServerContext {
    // 空槽位 = supervisor 未就绪：存根不读它，本测试钉住「不代理主链」。
    ServerContext::new(AppSettings::default(), ".scratch".to_string(), None)
}

fn body_json(resp: Response<Cursor<Vec<u8>>>) -> serde_json::Value {
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).expect("读响应体");
    serde_json::from_slice(&buf).expect("404 body 必须是 JSON")
}

#[test]
fn voice_transcript_path_hit_is_404_mod_disabled_without_supervisor() {
    let ctx = ctx_without_supervisor();
    assert!(
        ctx.try_get_supervisor().is_none(),
        "前提：supervisor 未就绪（本测试要证明存根不依赖它）"
    );
    for method in [Method::Post, Method::Get] {
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
        assert_eq!(resp.status_code(), StatusCode(404));
        let value = body_json(resp);
        assert_eq!(value["error"]["code"], "mod_disabled", "body = {value}");
        let message = value["error"]["message"].as_str().unwrap_or_default();
        assert!(
            message.contains("未挂载") && message.contains("移出主仓"),
            "message 必须说清「未挂载 / 实现已移出主仓」：{message}"
        );
    }
}

#[test]
fn voice_transcript_other_paths_return_none() {
    let ctx = ctx_without_supervisor();
    for path in [
        "/api/v1/voice/transcript/extra",
        "/api/v1/voice",
        "/api/v1/external/chat",
        "/api/v1/chat",
    ] {
        assert!(
            handle_voice_transcript(&ctx, &Method::Post, path, "", None, None, None).is_none(),
            "{path} 不归 voice_routes 管（必须交还 dispatch）"
        );
    }
}
