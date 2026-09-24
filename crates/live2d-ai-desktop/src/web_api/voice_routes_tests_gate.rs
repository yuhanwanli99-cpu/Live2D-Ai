//! `voice_routes` 的唤醒闸 / 手动闸 handler 级回归（L1 产品级，2026-09-15）。
//!
//! 从 `voice_routes_tests.rs` 拆出（测试文件 ≤800 行纪律）。挂载见同文件末尾的
//! `#[path]`；`use super::*` 拿到主测试模块的 helper（`call` / `ctx_voice` / `with_wake` …）。

use super::*;

// ---------------------------------------------- 唤醒闸 / 手动闸（L1 handler 级）

/// 总闸**显式**关（键存在且为空）→ **403 voice_gate_closed**，且 message 给处置。
#[test]
fn gate_closed_403_when_wake_phrase_explicitly_empty() {
    let ctx = ctx_voice(serde_json::json!({ "wake_phrase": "" }));
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
    assert!(b.contains("唤醒词"), "message 必须给可执行处置: {b}");
}

/// 键**缺失** ≠ 总闸关：走产品缺省唤醒词（小可爱），于是对「小爱 你好」回
/// 400 wake_phrase_required（不是 403 voice_gate_closed）。
///
/// 这是 2026-09-15 的用户裁决：「产品默认应开着唤醒词」。
#[test]
fn missing_wake_phrase_falls_back_to_product_default() {
    let ctx = ctx_voice(serde_json::json!({}));
    let missing = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &serde_json::json!({"text": with_wake("你好")}).to_string(),
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(
        missing.status_code(),
        StatusCode(400),
        "缺键走缺省词，不是总闸关"
    );
    let b = body_of(missing);
    assert!(b.contains("wake_phrase_required"), "got: {b}");
    assert!(!b.contains("voice_gate_closed"), "got: {b}");
    assert!(b.contains("小可爱"), "错误里应带缺省唤醒词: {b}");

    // 用缺省词本身开头则命中（这里只验闸门；supervisor 未就绪 → 503 也算放行）。
    let matched = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"小可爱 你好"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_ne!(matched.status_code(), StatusCode(403));
    assert_ne!(matched.status_code(), StatusCode(400));
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

/// PTT（`ptt:true`）跳过唤醒匹配：裸正文不再 400，且成功响应回显 ptt。
#[test]
fn ptt_true_bypasses_wake_match_and_echoes_flag() {
    let (ctx, handle, tmp) = ctx_with_supervisor("ptt-bypass", UNREACHABLE_LLM);
    let ctx = with_manifest(ctx, &voice_manifest(wake_only()));
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"把灯打开","ptt":true}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200), "PTT 裸正文应放行");
    let v: serde_json::Value = serde_json::from_str(&body_of(resp)).unwrap();
    assert_eq!(v["text"], serde_json::json!("把灯打开"));
    assert_eq!(v["ptt"], serde_json::json!(true));
    // L1（2026-09-16）：PTT 裸正文**没有**唤醒词，命中标志必须如实为 false
    // （旧实现恒 true，等于谎报「听见了唤醒词」）。
    assert_eq!(
        v["wake_phrase_matched"],
        serde_json::json!(false),
        "PTT 裸正文不得谎报命中唤醒短语: {v}"
    );
    handle.quit();
    let _ = std::fs::remove_file(&tmp);
}

/// 同一个裸正文**不带** ptt → 仍然 400 wake_phrase_required（默认路径不变）。
#[test]
fn without_ptt_bare_body_still_requires_wake_phrase() {
    let ctx = ctx_voice(wake_only());
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"把灯打开"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
    assert!(body_of(resp).contains("wake_phrase_required"));
}

/// `ptt` 类型写错 → 400 invalid_payload（不静默当 false）。
#[test]
fn ptt_wrong_type_is_invalid_payload() {
    let ctx = ctx_voice(wake_only());
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"把灯打开","ptt":"yes"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
    assert!(body_of(resp).contains("invalid_payload"));
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
