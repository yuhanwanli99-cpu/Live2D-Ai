//! persona-polish：`import_card` / `clear_import` 与 `state_json` 的回归。
//!
//! 三条契约（与 `src/tests.rs` / `src/tests_e2e.rs` 不重叠）：
//!
//! 1. **`state_json` 形状稳定、脱敏、零 IO**：没接管 → `ready=false`、
//!    `card_source="none"`；接管后报来源 / 名称 / 字数，且**不含**卡正文；
//! 2. **`import_card` 成功 = 规范化落盘 + 立刻写回主链**；坏 JSON / 坏 PNG /
//!    缺参数一律 `Err`（host 回 409 `command_failed`，带可读原因），
//!    且**不留**导入卡——失败不得静默成功；
//! 3. **导入卡优先于 config**，重启后仍生效；`clear_import` 回到 config 的卡，
//!    config 也没卡则还原基线。
//!
//! 「假主链」：`apply_settings` 真的更新一份 `Arc<Mutex<String>>`，settings
//! 快照读的也是同一份（与 `tests_e2e.rs` 同款）——断言打在**值**上，
//! 不是「被调了几次」。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::*;
use crate::tests::{CARD_V2, MockRegistrar, TempDir, noop_services, png_with_chara, push_chunk};

/// 假主链的当前 `persona.system_prompt`。
type Chain = Arc<Mutex<String>>;

/// 另一张**好卡**（与 `CARD_V2` 的 name 不同，用来断言「谁接管了」）。
const OTHER_CARD: &str =
    r#"{"spec":"chara_card_v2","data":{"name":"OTHER","description":"另一张卡"}}"#;

/// `CARD_V2` 关掉纪律模板时应逐字写入主链的整段提示词。
///
/// 用**字面量**而不是复用 `compose_system_prompt`：断言不能与被断言者共用同一条
/// 公式，否则两边一起错也会全绿（与 `tests_e2e.rs` 同一条纪律）。
const NEKO_WITHOUT_DISCIPLINE: &str = "[角色] NEKO\n\n猫娘";

fn new_chain(initial: &str) -> Chain {
    Arc::new(Mutex::new(initial.to_string()))
}

fn prompt(chain: &Chain) -> String {
    chain.lock().unwrap().clone()
}

/// 假主链 + 可开关的写盘闸（`gate=false` 模拟 `live2d-ai.toml` 不可写）。
fn chain_services(dir: &TempDir, chain: &Chain, gate: Arc<AtomicBool>) -> ModServices {
    let read = Arc::clone(chain);
    let write = Arc::clone(chain);
    noop_services()
        .with_apply_settings(ModSettingsApplier::new(move |patch| {
            if !gate.load(Ordering::SeqCst) {
                return false;
            }
            let Some(next) = patch["persona"]["system_prompt"].as_str() else {
                return false;
            };
            *write.lock().unwrap() = next.to_string();
            true
        }))
        .with_settings_reader(ModSettingsReader::new(
            move || serde_json::json!({"persona": {"system_prompt": read.lock().unwrap().clone()}}),
        ))
        .with_config_path(dir.config_path())
}

fn runtime(
    dir: &TempDir,
    chain: &Chain,
    config: serde_json::Value,
    gate: Arc<AtomicBool>,
) -> PersonaRuntime {
    PersonaRuntime::new(
        chain_services(dir, chain, gate),
        PersonaConfig::from_value(&config),
    )
}

/// 启用成功的 runtime（写盘闸默认打开）。
fn started(dir: &TempDir, chain: &Chain, config: serde_json::Value) -> PersonaRuntime {
    let mut rt = runtime(dir, chain, config, Arc::new(AtomicBool::new(true)));
    rt.start(&mut MockRegistrar::default()).expect("start");
    rt
}

fn state_of(rt: &mut PersonaRuntime) -> serde_json::Value {
    rt.state_json()
        .expect("persona 现在实现了 state_json（本轨有意升级）")
}

fn card_arg(text: &str) -> serde_json::Value {
    serde_json::json!({"card_json": text})
}

fn sidecar(dir: &TempDir) -> PathBuf {
    dir.path().join(IMPORTED_CARD_FILE)
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// ------------------------------------------------------------ state_json 形状

/// 形状是**契约**：字段名集合钉死，加字段必须来这里改一次。
#[test]
fn state_json_is_a_stable_desensitized_shape() {
    let dir = TempDir::new("cmd-state-shape");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({"card_json": CARD_V2}));

    let s = state_of(&mut rt);
    let mut keys: Vec<&str> = s
        .as_object()
        .expect("state 必须是对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    // L1 会话绑定加了四个键；既有九个必须**一个不少**（它们是全局主链的摘要，
    // 会话绑定的回归在 `src/tests_e2e.rs`）。
    assert_eq!(
        keys,
        vec![
            "active_session",
            "applied_chars",
            "card_format",
            "card_name",
            "card_source",
            "config_has_card",
            "has_base_snapshot",
            "include_discipline",
            "ready",
            "say_first_mes",
            "scope",
            "session_bound",
            "sessions",
        ]
    );
    // 单测环境没有会话表 / 没有 config_path：如实报「没有会话能力」，
    // 绝不假装绑好了。
    assert_eq!(s["session_bound"], false);
    assert_eq!(s["sessions"].as_array().map(Vec::len), Some(0));
    assert!(s["active_session"].is_null());
    assert_eq!(s["scope"], "global");
    assert_eq!(s["ready"], true);
    assert_eq!(s["card_source"], "config_json");
    assert_eq!(s["card_name"], "NEKO");
    assert_eq!(s["card_format"], "v2");
    assert_eq!(s["include_discipline"], true);
    assert_eq!(s["say_first_mes"], false);
    assert_eq!(s["has_base_snapshot"], true, "接管后基线已锚定");
    assert_eq!(s["config_has_card"], true);
    assert!(s["applied_chars"].as_u64().unwrap_or(0) > 0);
    // 脱敏：只报名称与长度，**不含**卡正文（描述 / 开场白 / 提示词正文）。
    let text = s.to_string();
    assert!(text.contains("NEKO"), "名称是给用户看的: {text}");
    assert!(!text.contains("猫娘"), "卡正文不得进运行态: {text}");
    assert!(!text.contains("你好"), "开场白不得进运行态: {text}");
}

/// 没配卡 = 合法 no-op：运行态如实说「没有」，不假装 ready。
#[test]
fn state_json_without_any_card_reports_none_not_a_fake_ready() {
    let dir = TempDir::new("cmd-state-none");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));

    let s = state_of(&mut rt);
    assert_eq!(s["ready"], false);
    assert_eq!(s["card_source"], "none");
    assert_eq!(s["card_name"], "");
    assert_eq!(s["card_format"], "");
    assert_eq!(s["applied_chars"], 0);
    assert_eq!(s["has_base_snapshot"], false, "没接管就不该有基线");
    assert_eq!(s["config_has_card"], false);
    assert_eq!(prompt(&chain), "BASE", "no-op 不得改主链");
}

// ------------------------------------------------------------ import_card 成功

/// 先开开关、后导入：导入那一刻就写回主链，不必再启停一次。
#[test]
fn import_card_from_json_applies_immediately_and_persists_a_normalized_v2_card() {
    let dir = TempDir::new("cmd-import-json");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));
    assert_eq!(prompt(&chain), "BASE");

    let result = rt
        .command("import_card", &card_arg(CARD_V2))
        .expect("导入应成功");
    assert_eq!(result["ok"], true);
    assert_eq!(result["card_source"], "imported");
    assert_eq!(result["card_name"], "NEKO");
    assert_eq!(result["card_format"], "v2");
    assert!(result["bytes"].as_u64().unwrap_or(0) > 0);

    let written = prompt(&chain);
    assert!(written.contains("[角色] NEKO"), "prompt = {written}");
    assert!(written.contains("猫娘"));
    assert!(
        written.contains("【对话纪律】"),
        "纪律模板缺省开: {written}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join(BASE_STATE_FILE)).unwrap(),
        "BASE",
        "基线快照 = 导入那一刻的主链值"
    );

    // 落盘的是**规范化 V2 JSON**（可读、可再解析、不是原样文本）。
    let saved = std::fs::read_to_string(sidecar(&dir)).expect("导入卡应落盘");
    let reparsed = PersonaCard::parse_json(&saved).expect("规范化产物必须能再解析");
    assert_eq!(reparsed.name, "NEKO");
    assert_eq!(reparsed.description, "猫娘");
    assert_eq!(reparsed.first, "你好");
    assert_eq!(reparsed.format, "v2");

    // 运行态跟着变。
    let s = state_of(&mut rt);
    assert_eq!(s["ready"], true);
    assert_eq!(s["card_source"], "imported");
    assert_eq!(s["card_name"], "NEKO");
}

/// 从 PNG 导入：落盘的仍是 **JSON**（不是把 PNG 原样存一份）。
#[test]
fn import_card_from_png_bytes_stores_json_not_the_png() {
    let dir = TempDir::new("cmd-import-png");
    let chain = new_chain("BASE");
    let mut rt = started(
        &dir,
        &chain,
        serde_json::json!({"include_discipline": false}),
    );

    let png = png_with_chara(&b64(CARD_V2.as_bytes()));
    let result = rt
        .command(
            "import_card",
            &serde_json::json!({"data_base64": b64(&png)}),
        )
        .expect("PNG 导入应成功");
    assert_eq!(result["card_name"], "NEKO");
    assert_eq!(result["card_format"], "v2");
    assert_eq!(prompt(&chain), NEKO_WITHOUT_DISCIPLINE);

    let saved = std::fs::read_to_string(sidecar(&dir)).expect("导入卡应落盘");
    assert!(
        saved.trim_start().starts_with('{'),
        "落盘的必须是 JSON: {saved}"
    );
    assert!(!saved.contains("tEXt"), "不得原样存 PNG 块: {saved}");
}

/// 面板直接传浏览器 `readAsDataURL` 的产物（带 `data:…;base64,` 前缀）也要能用。
#[test]
fn import_card_accepts_a_browser_data_url_prefix() {
    let dir = TempDir::new("cmd-import-data-url");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));

    let png = png_with_chara(&b64(CARD_V2.as_bytes()));
    let data_url = format!("data:image/png;base64,{}", b64(&png));
    let result = rt
        .command("import_card", &serde_json::json!({"data_base64": data_url}))
        .expect("dataURL 导入应成功");
    assert_eq!(result["card_name"], "NEKO");
    assert!(prompt(&chain).contains("[角色] NEKO"));
}

/// 手工覆盖项对导入的卡同样生效（覆盖优先于卡字段——与 config 路径同一口径）。
#[test]
fn import_card_respects_the_manual_overrides() {
    let dir = TempDir::new("cmd-import-override");
    let chain = new_chain("BASE");
    let mut rt = started(
        &dir,
        &chain,
        serde_json::json!({"name": "OVERRIDE", "include_discipline": false}),
    );

    let result = rt
        .command("import_card", &card_arg(CARD_V2))
        .expect("导入应成功");
    assert!(prompt(&chain).contains("OVERRIDE"), "{}", prompt(&chain));
    assert!(!prompt(&chain).contains("NEKO"), "{}", prompt(&chain));
    assert_eq!(
        result["card_name"], "OVERRIDE",
        "运行态报的是**生效**的名称（含覆盖）"
    );
}

// ------------------------------------------------------------ import_card 失败

/// 缺参数 → 明确失败（不是「导入了一张空卡」）。
#[test]
fn import_card_without_arguments_fails_explicitly() {
    let dir = TempDir::new("cmd-import-noargs");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));

    let err = rt
        .command("import_card", &serde_json::json!({}))
        .expect_err("缺参数必须失败");
    match err {
        ModError::Other(message) => {
            assert!(message.contains("import_card"), "err = {message}");
            assert!(message.contains("card_json"), "err = {message}");
        }
        other => panic!("必须是 command_failed（Other），得到 {other:?}"),
    }
    assert_eq!(prompt(&chain), "BASE");
    assert!(!sidecar(&dir).exists(), "失败不得留下导入卡");
}

/// 坏 JSON → 可读原因 + 不落盘 + 主链不动。
#[test]
fn import_card_bad_json_is_rejected_and_leaves_no_card_file() {
    let dir = TempDir::new("cmd-import-bad-json");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));

    let err = rt
        .command("import_card", &card_arg("{ not json"))
        .expect_err("坏 JSON 必须失败");
    let message = err.to_string();
    assert!(message.contains("不是可识别的角色卡"), "err = {message}");
    assert_eq!(prompt(&chain), "BASE");
    assert!(!sidecar(&dir).exists());
}

/// 合法 JSON 但不是角色卡（数组）→ 同样失败。
#[test]
fn import_card_rejects_json_that_is_not_a_card() {
    let dir = TempDir::new("cmd-import-array");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));

    let err = rt
        .command("import_card", &card_arg("[1,2,3]"))
        .expect_err("数组不是角色卡");
    assert!(err.to_string().contains("不是可识别的角色卡"), "{err}");
    assert!(!sidecar(&dir).exists());
}

/// 坏 PNG（没有 chara 块）与截断 PNG → 可读原因，永不 panic。
#[test]
fn import_card_bad_png_is_rejected_with_a_readable_reason() {
    let dir = TempDir::new("cmd-import-bad-png");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));

    let mut no_chara = PNG_SIGNATURE.to_vec();
    push_chunk(&mut no_chara, b"IEND", &[]);
    let err = rt
        .command(
            "import_card",
            &serde_json::json!({"data_base64": b64(&no_chara)}),
        )
        .expect_err("无 chara 的 PNG 必须失败");
    assert!(err.to_string().contains("没有角色卡数据"), "{err}");

    let mut truncated = PNG_SIGNATURE.to_vec();
    truncated.extend_from_slice(&10_000_u32.to_be_bytes());
    truncated.extend_from_slice(b"tEXt");
    truncated.extend_from_slice(b"chara\0tiny");
    let err = rt
        .command(
            "import_card",
            &serde_json::json!({"data_base64": b64(&truncated)}),
        )
        .expect_err("截断的 PNG 必须失败");
    assert!(err.to_string().contains("PNG 数据不完整"), "{err}");

    assert_eq!(prompt(&chain), "BASE");
    assert!(!sidecar(&dir).exists());
}

/// 不是 base64 → 明确说 base64 坏了（不去猜「可能是 JSON」）。
#[test]
fn import_card_rejects_invalid_base64() {
    let dir = TempDir::new("cmd-import-bad-b64");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));

    let err = rt
        .command(
            "import_card",
            &serde_json::json!({"data_base64": "!%not-base64!%"}),
        )
        .expect_err("坏 base64 必须失败");
    assert!(err.to_string().contains("base64"), "{err}");
    assert!(!sidecar(&dir).exists());
}

/// host 没注入 `config_path` → 没有导入卡的落点，明确失败而不是假装成功。
#[test]
fn import_card_without_config_path_fails_explicitly() {
    let mut rt = PersonaRuntime::new(
        noop_services(),
        PersonaConfig::from_value(&serde_json::json!({})),
    );
    rt.start(&mut MockRegistrar::default())
        .expect("空配置可启动");
    let err = rt
        .command("import_card", &card_arg(CARD_V2))
        .expect_err("没有落点就必须失败");
    assert!(err.to_string().contains("config_path"), "{err}");
}

/// 写主链失败 → 命令失败，并且**回滚**成上一次的导入卡（不留半个新卡）。
#[test]
fn import_card_rolls_back_the_card_file_when_the_chain_write_fails() {
    let dir = TempDir::new("cmd-import-rollback");
    let chain = new_chain("BASE");
    let gate = Arc::new(AtomicBool::new(true));
    let mut rt = runtime(&dir, &chain, serde_json::json!({}), Arc::clone(&gate));
    rt.start(&mut MockRegistrar::default()).unwrap();

    rt.command("import_card", &card_arg(CARD_V2))
        .expect("第一次导入应成功");
    let good = std::fs::read(sidecar(&dir)).expect("旧导入卡");
    let good_prompt = prompt(&chain);

    gate.store(false, Ordering::SeqCst);
    let err = rt
        .command("import_card", &card_arg(OTHER_CARD))
        .expect_err("写主链失败必须冒泡");
    assert!(err.to_string().contains("apply_settings"), "{err}");
    assert_eq!(
        std::fs::read(sidecar(&dir)).unwrap(),
        good,
        "失败的导入必须把导入卡回滚成上一次的"
    );
    assert_eq!(prompt(&chain), good_prompt, "主链不得改变");
    assert_eq!(state_of(&mut rt)["card_name"], "NEKO", "运行态仍报旧卡");
}

/// 从来没有过导入卡时，回滚 = 删除（不得留下「导入成功」的卡）。
#[test]
fn import_card_rolls_back_to_no_card_file_at_all() {
    let dir = TempDir::new("cmd-import-rollback-empty");
    let chain = new_chain("BASE");
    let gate = Arc::new(AtomicBool::new(false));
    let mut rt = runtime(&dir, &chain, serde_json::json!({}), Arc::clone(&gate));
    rt.start(&mut MockRegistrar::default())
        .expect("没有卡 → no-op");

    rt.command("import_card", &card_arg(CARD_V2))
        .expect_err("写主链失败必须失败");
    assert!(!sidecar(&dir).exists(), "回滚要把它删掉");
    assert_eq!(prompt(&chain), "BASE");
}

// ------------------------------------------------------- 优先级 / 清除 / 命令词

/// 导入卡优先于 config 的卡，且**重启后仍然优先**（config 没动，导入卡还在）。
#[test]
fn imported_card_wins_over_the_config_card_and_survives_a_restart() {
    let dir = TempDir::new("cmd-precedence");
    let chain = new_chain("BASE");
    let gate = Arc::new(AtomicBool::new(true));
    let config = serde_json::json!({"card_json": CARD_V2, "include_discipline": false});

    let mut first = runtime(&dir, &chain, config.clone(), Arc::clone(&gate));
    first.start(&mut MockRegistrar::default()).unwrap();
    assert_eq!(
        prompt(&chain),
        NEKO_WITHOUT_DISCIPLINE,
        "先由 config 的卡接管"
    );

    first
        .command("import_card", &card_arg(OTHER_CARD))
        .expect("导入应成功");
    assert!(prompt(&chain).contains("OTHER"), "{}", prompt(&chain));
    assert!(!prompt(&chain).contains("NEKO"), "{}", prompt(&chain));
    first.shutdown().unwrap();
    assert_eq!(prompt(&chain), "BASE");

    let mut second = runtime(&dir, &chain, config, Arc::clone(&gate));
    second.start(&mut MockRegistrar::default()).unwrap();
    assert!(
        prompt(&chain).contains("OTHER"),
        "重启后导入卡仍应优先: {}",
        prompt(&chain)
    );
    let s = state_of(&mut second);
    assert_eq!(s["card_source"], "imported");
    assert_eq!(s["card_name"], "OTHER");
}

/// `clear_import`：删掉导入卡 → 回到 config 的卡。
#[test]
fn clear_import_falls_back_to_the_config_card() {
    let dir = TempDir::new("cmd-clear-to-config");
    let chain = new_chain("BASE");
    let mut rt = started(
        &dir,
        &chain,
        serde_json::json!({"card_json": CARD_V2, "include_discipline": false}),
    );
    rt.command("import_card", &card_arg(OTHER_CARD))
        .expect("导入应成功");
    assert!(prompt(&chain).contains("OTHER"));

    let result = rt
        .command("clear_import", &serde_json::json!({}))
        .expect("清除应成功");
    assert_eq!(result["cleared"], true);
    assert_eq!(result["card_source"], "config_json");
    assert_eq!(result["card_name"], "NEKO");
    assert_eq!(prompt(&chain), NEKO_WITHOUT_DISCIPLINE);
    assert!(!sidecar(&dir).exists());
    assert_eq!(state_of(&mut rt)["card_source"], "config_json");
}

/// `clear_import`：config 也没卡 → 主链**还原基线**，运行态回到 `none`。
#[test]
fn clear_import_without_any_config_card_restores_the_baseline() {
    let dir = TempDir::new("cmd-clear-to-base");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));
    rt.command("import_card", &card_arg(CARD_V2))
        .expect("导入应成功");
    assert!(prompt(&chain).contains("NEKO"));

    let result = rt
        .command("clear_import", &serde_json::json!({}))
        .expect("清除应成功");
    assert_eq!(result["card_source"], "none");
    assert_eq!(result["card_name"], "");
    assert_eq!(prompt(&chain), "BASE", "清除后主链必须回到基线");
    let s = state_of(&mut rt);
    assert_eq!(s["ready"], false);
    assert_eq!(s["card_source"], "none");
    assert_eq!(s["card_name"], "");
    // 关开关仍然是幂等的「还原基线」，不会把 BASE 换成别的东西。
    rt.shutdown().unwrap();
    assert_eq!(prompt(&chain), "BASE");
}

/// 本来就没人导入过 → 清除是幂等的成功（`cleared=false`），不是错误。
#[test]
fn clear_import_is_idempotent_when_nothing_was_imported() {
    let dir = TempDir::new("cmd-clear-idempotent");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));

    let result = rt
        .command("clear_import", &serde_json::json!({}))
        .expect("应幂等");
    assert_eq!(result["cleared"], false);
    assert_eq!(result["card_source"], "none");
    assert_eq!(prompt(&chain), "BASE", "没有导入卡时不得写主链");
}

/// 不认识的命令 → `UnsupportedCommand`（host 回 409 `unsupported_command`，
/// 与「命令执行失败」是两个不同的码）。
#[test]
fn unknown_command_is_unsupported_not_a_failure() {
    let dir = TempDir::new("cmd-unknown");
    let chain = new_chain("BASE");
    let mut rt = started(&dir, &chain, serde_json::json!({}));

    let err = rt
        .command("wipe_everything", &serde_json::json!({}))
        .expect_err("未知命令必须报不支持");
    assert!(
        matches!(err, ModError::UnsupportedCommand { ref command } if command == "wipe_everything"),
        "err = {err:?}"
    );
}
