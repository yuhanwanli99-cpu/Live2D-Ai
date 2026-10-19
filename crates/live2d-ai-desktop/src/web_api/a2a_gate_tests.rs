//! `a2a_gate` 未挂载存根回归（2026-10-19）。
//!
//! 守三件事：
//! 1. 两条路径（agent card / JSON-RPC）都回 `404` 且 `error.code == "mod_disabled"`；
//! 2. supervisor **未就绪**时同样是 404（证明存根不打上游 LLM、不 `say`）；
//! 3. `is_a2a_path` 只认这两条路径（路径不匹配 = 交还 dispatch）。

use std::io::Read;

use tiny_http::StatusCode;

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
fn is_a2a_path_matches_only_the_two_known_paths() {
    assert!(is_a2a_path(AGENT_CARD_PATH));
    assert!(is_a2a_path(A2A_PATH));
    for other in [
        "/",
        "/api/v1/chat",
        "/api/v1/a2a/extra",
        "/.well-known/agent-card.json/x",
        "/api/v1/a2a?x=1",
        "/.well-known/agent-card.json ",
        "/well-known/agent-card.json",
    ] {
        assert!(!is_a2a_path(other), "{other} 不是 A2A 路径");
    }
}

#[test]
fn both_a2a_paths_are_404_mod_disabled_without_supervisor() {
    let ctx = ctx_without_supervisor();
    assert!(
        ctx.try_get_supervisor().is_none(),
        "前提：supervisor 未就绪（本测试要证明存根不依赖它）"
    );
    for path in [AGENT_CARD_PATH, A2A_PATH] {
        assert!(is_a2a_path(path), "{path} 必须是 A2A 路径");
        let resp = handle_a2a(&ctx);
        assert_eq!(resp.status_code(), StatusCode(404));
        let value = body_json(resp);
        assert_eq!(
            value["error"]["code"], "mod_disabled",
            "path = {path}, body = {value}"
        );
        let message = value["error"]["message"].as_str().unwrap_or_default();
        assert!(
            message.contains("未挂载") && message.contains("移出主仓"),
            "message 必须说清「未挂载 / 实现已移出主仓」：{message}"
        );
    }
}
