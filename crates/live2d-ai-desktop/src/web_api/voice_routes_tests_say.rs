//! `voice_routes` 的 200 / busy 路径回归（从 `voice_routes_tests.rs` 拆出）。
//!
//! 这里都需要一个真 supervisor（成功路径 / 确定性 busy），故与纯安全门禁用例分开。
//! 挂载见 `voice_routes_tests.rs` 末尾的 `#[path]`；`use super::*` 拿到
//! 主测试模块的 helper（`call` / `ctx_with_supervisor` / `blackhole_endpoint` …）。

use super::*;

// ------------------------------------------------------------ 200 / busy

#[test]
fn success_returns_200_with_cleaned_text() {
    let (ctx, handle, tmp) = ctx_with_supervisor("ok", UNREACHABLE_LLM);
    let ctx = with_manifest(ctx, &voice_manifest(wake_only()));
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"  小爱  你好\u3000世界  "}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let b = body_of(resp);
    let v: serde_json::Value = serde_json::from_str(&b).expect("响应是 JSON");
    assert_eq!(v["ok"], serde_json::json!(true), "got: {b}");
    assert_eq!(
        v["text"],
        serde_json::json!("你好世界"),
        "响应必须回**剥掉唤醒短语 + 清洗 + 缺省 zh-CN 归一化**后的文本: {b}"
    );
    assert_eq!(
        v["wake_phrase_matched"],
        serde_json::json!(true),
        "L1：唤醒短语命中必须可观察: {b}"
    );
    assert_eq!(v["backend"], serde_json::json!("mock"), "缺省 backend: {b}");
    assert_eq!(v["locale"], serde_json::json!("zh-CN"), "缺省 locale: {b}");

    handle.quit();
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn zero_width_chars_are_cleaned_before_say() {
    let (ctx, handle, tmp) = ctx_with_supervisor("zw", UNREACHABLE_LLM);
    let ctx = with_manifest(ctx, &voice_manifest(wake_only()));
    // 零宽 / 控制符被清洗后，**句首**的唤醒短语被剥离（P0-4 句首锚定）。
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"\uFEFF小\u200B爱\u0007你好"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let v: serde_json::Value = serde_json::from_str(&body_of(resp)).unwrap();
    assert_eq!(v["text"], serde_json::json!("你好"));
    handle.quit();
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn busy_returns_200_ok_false() {
    let port = blackhole_endpoint();
    let (ctx, handle, tmp) = ctx_with_supervisor("busy", &format!("http://127.0.0.1:{port}/v1"));
    let ctx = with_manifest(ctx, &voice_manifest(wake_only()));
    // 先用一条占位 say 让 supervisor 进入 in-flight 回合（黑洞端点不回包，
    // 回合悬停；turn 期间 say 缓冲不被消费）。
    assert!(handle.say("占位回合"));
    std::thread::sleep(std::time::Duration::from_millis(150));
    // 缓冲空着 → 本条被接受（填充容量 1 的缓冲）。
    let first = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &serde_json::json!({"text": with_wake("第一条")}).to_string(),
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(first.status_code(), StatusCode(200));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body_of(first)).unwrap()["ok"],
        serde_json::json!(true)
    );
    // 缓冲已满 → 200 + ok:false busy（**不是** 5xx）。
    let second = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &serde_json::json!({"text": with_wake("第二条")}).to_string(),
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(
        second.status_code(),
        StatusCode(200),
        "忙碌刻意用 200（发送方自行退避），不要 5xx"
    );
    let b = body_of(second);
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["ok"], serde_json::json!(false), "got: {b}");
    assert_eq!(v["error"]["code"], serde_json::json!("busy"), "got: {b}");

    handle.quit();
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn empty_transcript_never_reaches_say() {
    let (ctx, handle, tmp) = ctx_with_supervisor("nosay", UNREACHABLE_LLM);
    let ctx = with_manifest(ctx, &voice_manifest(wake_only()));
    // 整句就是唤醒短语 → empty_transcript（Allow 空正文）。
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &serde_json::json!({"text": format!("{WAKE}！")}).to_string(),
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
    assert!(body_of(resp).contains("empty_transcript"));
    // 未消费任何 pending：紧接着一条正常转写仍能被接受（= 前一条没进 say）。
    let next = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &serde_json::json!({"text": with_wake("正常一条")}).to_string(),
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(next.status_code(), StatusCode(200));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body_of(next)).unwrap()["ok"],
        serde_json::json!(true),
        "空转写不得占用 say 缓冲"
    );
    handle.quit();
    let _ = std::fs::remove_file(&tmp);
}
