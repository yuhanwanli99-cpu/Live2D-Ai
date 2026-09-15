//! `voice_routes` 的唤醒闸 / 手动闸 handler 级回归（L1 产品级，2026-09-15）。
//!
//! 从 `voice_routes_tests.rs` 拆出（测试文件 ≤800 行纪律）。挂载见同文件末尾的
//! `#[path]`；`use super::*` 拿到主测试模块的 helper（`call` / `ctx_voice` / `with_wake` …）。

use super::*;

// ---------------------------------------------- 唤醒闸 / 手动闸（L1 handler 级）

/// 总闸关（启用但没配唤醒短语）→ **403 voice_gate_closed**，且 message 给处置。
#[test]
fn gate_closed_403_when_wake_phrase_missing() {
    let ctx = ctx_voice(serde_json::json!({}));
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &serde_json::json!({"text": with_wake("你好")}).to_string(),
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(403));
    let b = body_of(resp);
    assert!(b.contains("voice_gate_closed"), "got: {b}");
    assert!(b.contains("唤醒短语"), "message 必须给可执行处置: {b}");
}

/// 手动闸关 → **403 voice_manual_off**，且优先于其它闸门。
#[test]
fn manual_off_403_takes_priority() {
    let ctx = ctx_voice(serde_json::json!({"wake_phrase": WAKE, "manual_enabled": false}));
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &serde_json::json!({"text": with_wake("你好")}).to_string(),
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(403));
    let b = body_of(resp);
    assert!(b.contains("voice_manual_off"), "got: {b}");
    assert!(!b.contains("voice_gate_closed"), "手动闸优先: {b}");
}

/// 短语不匹配 → **400 wake_phrase_required**（可读：告诉发送方先叫唤醒词）。
#[test]
fn wake_phrase_required_400_when_not_present() {
    let ctx = ctx_voice(wake_only());
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"你好"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
    let b = body_of(resp);
    assert!(b.contains("wake_phrase_required"), "got: {b}");
    assert!(b.contains(WAKE), "错误里带上期待的词（非密钥）: {b}");
}

/// 命中 + 短语在开头紧跟标点：响应 text 已剥离，且标点一起去掉。
#[test]
fn matched_wake_phrase_with_punctuation_is_stripped() {
    let (ctx, handle, tmp) = ctx_with_supervisor("wake-strip", UNREACHABLE_LLM);
    let ctx = with_manifest(ctx, &voice_manifest(wake_only()));
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &serde_json::json!({"text": format!("{WAKE}，把窗户关小一点")}).to_string(),
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let v: serde_json::Value = serde_json::from_str(&body_of(resp)).unwrap();
    assert_eq!(
        v["text"],
        serde_json::json!("把窗户关小一点"),
        "短语与其后的标点都要被剥掉: {v}"
    );
    assert_eq!(v["wake_phrase_matched"], serde_json::json!(true));
    handle.quit();
    let _ = std::fs::remove_file(&tmp);
}

/// 顺序断言：token（401）先于总闸（403）——token 未对齐时不该先被告知「总闸没开」。
#[test]
fn token_check_precedes_gate() {
    if std::env::var(TOKEN_ENV_VAR).is_ok() {
        return;
    }
    let ctx = ctx_voice(serde_json::json!({"token": "cfg-secret"}));
    let resp = call_auth(&ctx, r#"{"text":"你好"}"#, None).unwrap();
    assert_eq!(
        resp.status_code(),
        StatusCode(401),
        "token 已配置且缺失 → 401，先于总闸判定"
    );
}
