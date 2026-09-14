//! `live2d-ai-mod-persona` 回归。两组各守一条契约：
//!
//! 1. **启停循环** —— `card_json` / `card_path`(JSON) / `card_path`(PNG) 三条入口
//!    都要能「启用 → 写回主链 system_prompt → 停用 → 还原基线」；
//! 2. **坏输入** —— 坏 JSON / 文件不存在 / 路径是目录 / 超大 / PNG 截断
//!    **一律显式失败**（`start` 返回 `Err`，主链提示词一个字不动），永不 panic。

use super::*;
use std::sync::{Arc, Mutex};

// ------------------------------------------------------------------ 测试脚手架

/// 每个测试一个独立临时目录（并行跑：共用目录名会互相踩，症状是随机红）。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("l2d-persona-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// 与 `live2d-ai.toml` 同目录的路径（基线快照住这儿）。
    fn config_path(&self) -> String {
        self.0.join("live2d-ai.toml").display().to_string()
    }

    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).expect("写测试文件");
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 记录 register_settings 的测试注册器。
#[derive(Default)]
struct MockRegistrar {
    specs: Vec<ModSettingsSpec>,
}

impl ModRegistrar for MockRegistrar {
    fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
        self.specs.push(spec);
        Ok(())
    }
    fn subscribe(&mut self, _topic: ModEventTopic) -> Result<SubscriptionId, ModError> {
        Ok(SubscriptionId(1))
    }
    fn unsubscribe(&mut self, _id: SubscriptionId) -> Result<(), ModError> {
        Ok(())
    }
}

fn noop_services() -> ModServices {
    ModServices::new(
        ModActionSender::new(|_| false),
        SaySender::new(|_| true),
        ModEventSender::new(|_, _| true),
        ModLogger::new(|_, _| {}),
    )
}

fn new_calls() -> Arc<Mutex<Vec<serde_json::Value>>> {
    Arc::new(Mutex::new(Vec::new()))
}

fn accepting_applier(calls: Arc<Mutex<Vec<serde_json::Value>>>) -> ModSettingsApplier {
    ModSettingsApplier::new(move |patch| {
        calls.lock().unwrap().push(patch);
        true
    })
}

/// 「写盘被拒」的 applier：**仍然记录** patch，证明 Mod 确实试过。
fn rejecting_applier(calls: Arc<Mutex<Vec<serde_json::Value>>>) -> ModSettingsApplier {
    ModSettingsApplier::new(move |patch| {
        calls.lock().unwrap().push(patch);
        false
    })
}

/// 脱敏设置快照（`persona.system_prompt` = `base_prompt`）。
fn reader(base_prompt: &str) -> ModSettingsReader {
    let s = base_prompt.to_string();
    ModSettingsReader::new(move || serde_json::json!({"persona": {"system_prompt": s}}))
}

/// 带捕获 apply + 设置快照 + `live2d-ai.toml` 路径的 services。
fn harness_services(
    dir: &TempDir,
    calls: Arc<Mutex<Vec<serde_json::Value>>>,
    base: &str,
    accept: bool,
) -> ModServices {
    let applier = if accept {
        accepting_applier(calls)
    } else {
        rejecting_applier(calls)
    };
    noop_services()
        .with_apply_settings(applier)
        .with_settings_reader(reader(base))
        .with_config_path(dir.config_path())
}

/// 带「说了什么」捕获的 services（`say_first_mes` 的时机由它守）。
fn services_with_say(
    dir: &TempDir,
    calls: Arc<Mutex<Vec<serde_json::Value>>>,
    base: &str,
    accept: bool,
    said: Arc<Mutex<Vec<String>>>,
) -> ModServices {
    ModServices::new(
        ModActionSender::new(|_| false),
        SaySender::new(move |t| {
            said.lock().unwrap().push(t);
            true
        }),
        ModEventSender::new(|_, _| true),
        ModLogger::new(|_, _| {}),
    )
    .with_apply_settings(if accept {
        accepting_applier(calls)
    } else {
        rejecting_applier(calls)
    })
    .with_settings_reader(reader(base))
    .with_config_path(dir.config_path())
}

fn patches(calls: &Arc<Mutex<Vec<serde_json::Value>>>) -> Vec<serde_json::Value> {
    calls.lock().unwrap().clone()
}

/// 取 patch 里的 `persona.system_prompt`。
fn prompt_of(patch: &serde_json::Value) -> String {
    patch["persona"]["system_prompt"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

const CARD_V2: &str =
    r#"{"spec":"chara_card_v2","data":{"name":"NEKO","description":"猫娘","first_mes":"你好"}}"#;

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&[0, 0, 0, 0]); // CRC（本 Mod 不校验）
}

fn png_with_chara(payload: &str) -> Vec<u8> {
    let mut out = PNG_SIGNATURE.to_vec();
    push_chunk(&mut out, b"tEXt", format!("chara\0{payload}").as_bytes());
    push_chunk(&mut out, b"IEND", &[]);
    out
}

// ------------------------------------------------------------------ 静态身份

#[test]
fn descriptor_is_static_and_api_compatible() {
    assert_eq!(DESCRIPTOR.id, "persona");
    assert_eq!(DESCRIPTOR.api_version, MOD_API_VERSION);
    assert!(!DESCRIPTOR.version.is_empty(), "Mod 版本号不得为空");
    assert!(
        persona_settings_spec().validate().is_ok(),
        "字段 key 不得重复"
    );
}

/// 启停语义：**唯一真源 = manifest `enabled`**，schema 里不得有第二个开关。
#[test]
fn settings_schema_has_no_enabled_switch() {
    let spec = PersonaFactory.settings_spec().expect("静态 schema");
    let keys: Vec<&str> = spec.fields.iter().map(|f| f.key()).collect();
    assert_eq!(
        keys,
        vec![
            "card_path",
            "card_json",
            "include_discipline",
            "say_first_mes",
            "name",
            "description",
            "personality",
            "scenario",
        ]
    );
    assert!(
        !keys.contains(&"enabled"),
        "启停只由 Mod manifest 的 enabled 表达，schema 不再重复（两处真相 = 必然分叉）"
    );
}

/// `start` 注册的 spec 与 `factory.settings_spec()`（未启用也能拿到的静态那份）
/// **必须完全一致**：否则「先填配置再启用」看到的是另一套字段。
#[test]
fn start_registers_same_spec_as_static() {
    let dir = TempDir::new("spec-parity");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls, "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({"card_json": CARD_V2})),
    );
    let mut mock = MockRegistrar::default();
    rt.start(&mut mock).expect("start");
    assert_eq!(mock.specs.len(), 1);
    assert_eq!(mock.specs[0], persona_settings_spec());
    assert_eq!(
        PersonaFactory.settings_spec().as_ref(),
        Some(&mock.specs[0])
    );
}

// ------------------------------------------------------------------ 卡解析

#[test]
fn parse_v1_flat_json() {
    let card = PersonaCard::parse_json(
        r#"{"name":"NEKO","description":"猫娘","personality":"傲娇","scenario":"咖啡馆","first_mes":"你好"}"#,
    )
    .expect("V1 卡应解析");
    assert_eq!(card.name, "NEKO");
    assert_eq!(card.first, "你好");
    assert_eq!(card.format, "v1");
}

#[test]
fn parse_v2_nested_json() {
    let card = PersonaCard::parse_json(
        r#"{"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"NEKO","description":"猫娘","system_prompt":"用短句。"}}"#,
    )
    .expect("V2 卡应解析");
    assert_eq!(card.name, "NEKO");
    assert_eq!(card.system_prompt, "用短句。");
    assert_eq!(card.format, "v2");
}

#[test]
fn reject_non_card_json() {
    assert!(PersonaCard::parse_json(r#"{"foo":1}"#).is_none());
    assert!(PersonaCard::parse_json("not json").is_none());
    assert!(PersonaCard::parse_json("").is_none());
    assert!(PersonaCard::parse_json("[1,2,3]").is_none());
}

#[test]
fn parse_png_with_base64_chara() {
    let card_json = r#"{"spec":"chara_card_v2","data":{"name":"PNG猫","description":"来自 PNG"}}"#;
    let b64 = base64::engine::general_purpose::STANDARD.encode(card_json.as_bytes());
    let png = png_with_chara(&b64);
    assert!(png.starts_with(&PNG_SIGNATURE));
    let card = PersonaCard::parse_bytes(&png).expect("PNG 卡应解析");
    assert_eq!(card.name, "PNG猫");
    assert_eq!(card.description, "来自 PNG");
    assert_eq!(card.format, "v2");
}

#[test]
fn png_without_chara_reports_honestly() {
    let mut out = PNG_SIGNATURE.to_vec();
    push_chunk(&mut out, b"IEND", &[]);
    let err = PersonaCard::parse_bytes(&out).expect_err("无 chara 应报错");
    assert!(err.contains("没有角色卡数据"), "err = {err}");
}

/// 截断的 PNG → 明确报错，**不得 panic**：
/// 块头声称的长度超出文件，以及连块头都读不全这两种残缺都要收。
#[test]
fn truncated_png_errors_without_panic() {
    let mut over_long = PNG_SIGNATURE.to_vec();
    over_long.extend_from_slice(&1_000_u32.to_be_bytes()); // 声称 1000 字节…
    over_long.extend_from_slice(b"tEXt");
    over_long.extend_from_slice(b"chara\0only-a-few"); // …实际只有一小截。
    let err = PersonaCard::parse_bytes(&over_long).expect_err("截断的 PNG 应报错");
    assert!(err.contains("PNG 数据不完整"), "err = {err}");

    let mut half_header = PNG_SIGNATURE.to_vec();
    half_header.extend_from_slice(&[0, 0]);
    let err = PersonaCard::parse_bytes(&half_header).expect_err("半截块头应报错");
    assert!(err.contains("没有角色卡数据"), "err = {err}");
}

/// 压缩 `iTXt` 需要 zlib：明确说「不支持」，不返回空卡。
#[test]
fn compressed_itxt_reports_unsupported() {
    let mut payload = b"chara\0".to_vec();
    payload.extend_from_slice(&[1, 0]); // compression flag=1, method=0
    payload.extend_from_slice(&[0, 0]); // language\0 translated\0
    payload.extend_from_slice(b"whatever");
    let mut out = PNG_SIGNATURE.to_vec();
    push_chunk(&mut out, b"iTXt", &payload);
    push_chunk(&mut out, b"IEND", &[]);
    let err = PersonaCard::parse_bytes(&out).expect_err("压缩 iTXt 应报错");
    assert!(err.contains("压缩的 iTXt"), "err = {err}");
}

/// 非 UTF-8 且非 PNG → 明确报错。
#[test]
fn non_utf8_non_png_reports_honestly() {
    let err = PersonaCard::parse_bytes(&[0xFF, 0xFE, 0x00, 0x01]).expect_err("应报错");
    assert!(err.contains("不是 PNG 也不是 UTF-8"), "err = {err}");
}

#[test]
fn compose_skips_empty_and_includes_discipline() {
    let card = PersonaCard {
        name: "NEKO".into(),
        description: "猫娘".into(),
        ..Default::default()
    };
    let with = compose_system_prompt(&card, true);
    assert!(with.contains("[角色] NEKO"));
    assert!(with.contains("猫娘"));
    assert!(with.contains("【对话纪律】"));
    let without = compose_system_prompt(&card, false);
    assert!(!without.contains("【对话纪律】"));
    assert_eq!(compose_system_prompt(&PersonaCard::default(), false), "");
}

// -------------------------------------------------------- 启停循环（三条入口）

/// 入口 A：`card_json`。启用 → 写回 → 停用 → 还原基线。
#[test]
fn card_json_cycle_applies_then_restores_base() {
    let dir = TempDir::new("cycle-card-json");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "card_json": CARD_V2,
            "include_discipline": true,
        })),
    );
    let mut mock = MockRegistrar::default();
    rt.start(&mut mock).expect("start");
    assert_eq!(mock.specs.len(), 1, "应注册 settings schema");

    {
        let got = patches(&calls);
        assert_eq!(got.len(), 1, "start 应写一次 system_prompt");
        let prompt = prompt_of(&got[0]);
        assert!(prompt.contains("[角色] NEKO"), "prompt = {prompt}");
        assert!(prompt.contains("猫娘"));
        assert!(prompt.contains("【对话纪律】"));
    }
    // 基线快照落盘（与 live2d-ai.toml 同目录）。
    let saved = std::fs::read_to_string(dir.path().join(BASE_STATE_FILE)).expect("基线快照应存在");
    assert_eq!(saved, "BASE");

    rt.shutdown().expect("shutdown");
    let got = patches(&calls);
    assert_eq!(got.len(), 2, "shutdown 应还原一次");
    assert_eq!(prompt_of(&got[1]), "BASE");
}

/// 入口 B：`card_path` 指向 **JSON 文件**（rc.4 只在坏路径上测过，好消息径没有回归）。
#[test]
fn card_path_json_cycle_applies_then_restores_base() {
    let dir = TempDir::new("cycle-card-path-json");
    let card_path = dir.write("neko.json", CARD_V2);
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "card_path": card_path.display().to_string(),
        })),
    );
    let mut mock = MockRegistrar::default();
    rt.start(&mut mock).expect("start（card_path=JSON）");
    assert_eq!(mock.specs.len(), 1);

    let got = patches(&calls);
    assert_eq!(got.len(), 1);
    assert!(prompt_of(&got[0]).contains("NEKO"), "got = {got:?}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join(BASE_STATE_FILE)).unwrap(),
        "BASE"
    );

    rt.shutdown().unwrap();
    let got = patches(&calls);
    assert_eq!(prompt_of(&got[1]), "BASE");
}

/// 入口 C：`card_path` 指向 **内嵌 chara 的 PNG**。
#[test]
fn card_path_png_cycle_applies_then_restores_base() {
    let dir = TempDir::new("cycle-card-path-png");
    let card_json =
        r#"{"spec":"chara_card_v2","data":{"name":"PNG猫","system_prompt":"只说实话。"}}"#;
    let b64 = base64::engine::general_purpose::STANDARD.encode(card_json.as_bytes());
    let png_path = dir.write("neko.png", png_with_chara(&b64));

    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "card_path": png_path.display().to_string(),
            "include_discipline": false,
        })),
    );
    let mut mock = MockRegistrar::default();
    rt.start(&mut mock).expect("start（card_path=PNG）");

    let got = patches(&calls);
    assert_eq!(got.len(), 1);
    let prompt = prompt_of(&got[0]);
    assert!(prompt.contains("PNG猫"), "prompt = {prompt}");
    assert!(prompt.contains("只说实话。"));
    assert!(!prompt.contains("【对话纪律】"), "纪律模板已关: {prompt}");

    rt.shutdown().unwrap();
    assert_eq!(prompt_of(&patches(&calls)[1]), "BASE");
}

/// 二次启停：磁盘上的基线是**锚**——期间用户手改的主链提示词不会被
/// 「启用→停用」连带吃掉（这是快照存在的全部理由）。
#[test]
fn second_cycle_restores_the_saved_baseline_not_the_current_value() {
    let dir = TempDir::new("cycle-twice");
    let calls = new_calls();
    let card_config = serde_json::json!({"card_json": CARD_V2});

    // 第一次：基线 = BASE。
    let mut first = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&card_config),
    );
    first.start(&mut MockRegistrar::default()).unwrap();
    first.shutdown().unwrap();

    // 用户在此期间手改了 live2d-ai.toml 的 system_prompt（快照里读到的仍是 BASE）。
    let mut second = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "用户手改的值", true),
        PersonaConfig::from_value(&card_config),
    );
    second.start(&mut MockRegistrar::default()).unwrap();
    second.shutdown().unwrap();

    let got = patches(&calls);
    assert_eq!(got.len(), 4, "两轮各写一次 + 各还原一次: {got:?}");
    assert_eq!(
        prompt_of(&got[3]),
        "BASE",
        "还原的是首次基线，不是「当前值」"
    );
}

/// 没配卡 = 合法 no-op：`start` 成功（`Running`），主链提示词一字不动，
/// 停用也无需还原。
#[test]
fn no_card_configured_is_a_noop_not_an_error() {
    let dir = TempDir::new("no-card");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({})),
    );
    let mut mock = MockRegistrar::default();
    rt.start(&mut mock)
        .expect("空配置应启动成功（先启用、后填卡是合法流程）");
    assert_eq!(mock.specs.len(), 1, "空配置也要给出 schema");
    rt.shutdown().unwrap();
    assert!(
        patches(&calls).is_empty(),
        "空配置不得写主链: {:?}",
        patches(&calls)
    );
    assert!(
        !dir.path().join(BASE_STATE_FILE).exists(),
        "没接管就不该有基线快照"
    );
}

/// 手工覆盖：没有卡时自成一张卡；有卡时优先于卡字段（未覆盖的仍来自卡）。
#[test]
fn overrides_compose_alone_and_win_over_card_fields() {
    let dir = TempDir::new("overrides-only");
    let calls = new_calls();
    let mut alone = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "name": "OVERRIDE",
            "personality": "冷静",
            "include_discipline": false,
        })),
    );
    alone.start(&mut MockRegistrar::default()).unwrap();
    let got = patches(&calls);
    assert_eq!(got.len(), 1);
    let prompt = prompt_of(&got[0]);
    assert!(prompt.contains("OVERRIDE"), "prompt = {prompt}");
    assert!(prompt.contains("性格：冷静"));

    let dir2 = TempDir::new("overrides-win");
    let calls2 = new_calls();
    let mut on_card = PersonaRuntime::new(
        harness_services(&dir2, calls2.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "card_json": r#"{"name":"CARD","description":"卡描述"}"#,
            "name": "OVERRIDE",
        })),
    );
    on_card.start(&mut MockRegistrar::default()).unwrap();
    let prompt = prompt_of(&patches(&calls2)[0]);
    assert!(prompt.contains("OVERRIDE"), "覆盖应生效: {prompt}");
    assert!(!prompt.contains("CARD"), "卡里的 name 应被覆盖: {prompt}");
    assert!(prompt.contains("卡描述"), "未覆盖的字段仍来自卡: {prompt}");
}

/// `card_json` 优先于 `card_path`（两者同时存在时以 JSON 为准）。
#[test]
fn card_json_wins_over_card_path() {
    let dir = TempDir::new("json-wins");
    let path = dir.write("onfile.json", r#"{"name":"FROM_FILE"}"#);
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "card_json": r#"{"name":"FROM_JSON"}"#,
            "card_path": path.display().to_string(),
            "include_discipline": false,
        })),
    );
    rt.start(&mut MockRegistrar::default()).unwrap();
    let prompt = prompt_of(&patches(&calls)[0]);
    assert!(prompt.contains("FROM_JSON"), "prompt = {prompt}");
    assert!(!prompt.contains("FROM_FILE"), "prompt = {prompt}");
}

// ---------------------------------------------------------------- 坏输入

/// 坏 JSON → 显式 `Err`（不再「warn 一行然后报 Running」），主链不动、无副作用。
#[test]
fn bad_card_json_fails_explicitly() {
    let dir = TempDir::new("bad-json");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({"card_json": "{ not json"})),
    );
    let mut mock = MockRegistrar::default();
    let err = rt.start(&mut mock).expect_err("坏 JSON 必须显式失败");
    assert!(err.to_string().contains("card_json"), "err = {err}");
    assert!(patches(&calls).is_empty(), "坏输入不得写主链");
    assert!(mock.specs.is_empty(), "失败路径不得留下半个副作用");
    assert!(
        !dir.path().join(BASE_STATE_FILE).exists(),
        "没接管就不该有基线快照"
    );
}

/// 合法 JSON 但不是角色卡 → 同样显式失败。
#[test]
fn non_card_json_fails_explicitly() {
    let dir = TempDir::new("non-card");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({"card_json": r#"{"foo":1}"#})),
    );
    let err = rt
        .start(&mut MockRegistrar::default())
        .expect_err("非卡 JSON 必须失败");
    assert!(
        err.to_string().contains("不是可识别的角色卡"),
        "err = {err}"
    );
    assert!(patches(&calls).is_empty());
}

/// 文件不存在 → 错误里带路径（用户才知道该改哪个字段），主链不动。
#[test]
fn missing_card_file_fails_with_path_in_message() {
    let dir = TempDir::new("missing-file");
    let missing = dir.path().join("nope.png");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "card_path": missing.display().to_string(),
        })),
    );
    let err = rt
        .start(&mut MockRegistrar::default())
        .expect_err("缺失文件必须失败");
    let msg = err.to_string();
    assert!(msg.contains("nope.png"), "错误应带路径: {msg}");
    assert!(patches(&calls).is_empty(), "读卡失败不得改主链");
}

/// 路径指向目录 → 明确说「不是普通文件」，不去读它。
#[test]
fn card_path_pointing_to_a_directory_is_rejected() {
    let dir = TempDir::new("dir-as-card");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "card_path": dir.path().display().to_string(),
        })),
    );
    let err = rt
        .start(&mut MockRegistrar::default())
        .expect_err("目录必须被拒");
    assert!(err.to_string().contains("不是普通文件"), "err = {err}");
    assert!(patches(&calls).is_empty());
}

/// 超大卡文件 → **读盘之前**就拒（`metadata` 先量），错误里给出实际大小与上限。
///
/// 用 `set_len` 造稀疏文件：真去读的话会分配 [`MAX_CARD_FILE_BYTES`] 以上的内存，
/// 那正是这条闸要拦的事。
#[test]
fn oversized_card_file_is_rejected_before_read() {
    let dir = TempDir::new("oversized-file");
    let big = dir.path().join("huge.png");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(MAX_CARD_FILE_BYTES + 1)
        .unwrap();

    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "card_path": big.display().to_string(),
        })),
    );
    let err = rt
        .start(&mut MockRegistrar::default())
        .expect_err("超大文件必须被拒");
    let msg = err.to_string();
    assert!(msg.contains("过大"), "err = {msg}");
    assert!(msg.contains("上限"), "错误要给出上限: {msg}");
    assert!(patches(&calls).is_empty());
}

/// 超大 `card_json` → 解析之前就拒。
#[test]
fn oversized_card_json_is_rejected() {
    let dir = TempDir::new("oversized-json");
    let huge = "x".repeat(MAX_CARD_JSON_BYTES + 1);
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({"card_json": huge})),
    );
    let err = rt
        .start(&mut MockRegistrar::default())
        .expect_err("超大 card_json 必须被拒");
    let msg = err.to_string();
    assert!(msg.contains("card_json 过大"), "err = {msg}");
    assert!(patches(&calls).is_empty());
}

/// 边界：正好等于上限的 `card_json` **不**因大小被拒（闸门是「大于」）。
#[test]
fn card_json_exactly_at_the_limit_is_not_rejected_for_size() {
    let dir = TempDir::new("json-at-limit");
    // 恰好 MAX_CARD_JSON_BYTES 字节的合法 JSON 对象：大小合法（内容成不成卡另说）。
    let pad = " ".repeat(MAX_CARD_JSON_BYTES - r#"{"name":"X"}"#.len());
    let json = format!(r#"{{"name":"X"}}{pad}"#);
    assert_eq!(json.len(), MAX_CARD_JSON_BYTES);
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, new_calls(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({"card_json": json})),
    );
    // 闸门是「大于」：边界值即便被别的理由拒，也不能是「过大」。
    if let Err(e) = rt.start(&mut MockRegistrar::default()) {
        assert!(!e.to_string().contains("过大"), "边界值不得按超大处理: {e}");
    }
}

/// 卡在、但合成结果为空（字段全空 + 关掉纪律模板）→ 显式失败，
/// 绝不把主链提示词写成空串。
#[test]
fn card_composing_to_nothing_is_an_error() {
    let dir = TempDir::new("empty-compose");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "card_json": r#"{"name":"","description":""}"#,
            "include_discipline": false,
        })),
    );
    let err = rt
        .start(&mut MockRegistrar::default())
        .expect_err("空人设必须失败");
    assert!(
        err.to_string().contains("没有产生任何可写入"),
        "err = {err}"
    );
    assert!(
        patches(&calls).is_empty(),
        "空人设不得把 system_prompt 写成空串"
    );
}

/// `apply_settings` 被拒（磁盘不可写）→ 显式失败，且**不在磁盘上留基线快照**：
/// 没真的接管主链，就不该留下「已接管」的痕迹。
#[test]
fn rejected_apply_fails_explicitly_and_leaves_no_baseline() {
    let dir = TempDir::new("apply-rejected");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", false),
        PersonaConfig::from_value(&serde_json::json!({"card_json": CARD_V2})),
    );
    let err = rt
        .start(&mut MockRegistrar::default())
        .expect_err("写盘被拒必须显式失败");
    assert!(err.to_string().contains("apply_settings"), "err = {err}");
    assert_eq!(patches(&calls).len(), 1, "它确实试过一次");
    assert!(
        !dir.path().join(BASE_STATE_FILE).exists(),
        "写盘失败不得落基线快照"
    );
    // 还原路径同理：即便硬走到 shutdown 也只记日志，不 panic、不谎报。
    assert!(rt.shutdown().is_ok());
}

/// 开场白只在**真的接管了主链**之后才念。
#[test]
fn say_first_mes_fires_only_after_a_successful_apply() {
    let config = serde_json::json!({"card_json": CARD_V2, "say_first_mes": true});

    // 成功：说。
    let said: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let dir = TempDir::new("first-mes-ok");
    let mut ok = PersonaRuntime::new(
        services_with_say(&dir, new_calls(), "BASE", true, said.clone()),
        PersonaConfig::from_value(&config),
    );
    ok.start(&mut MockRegistrar::default()).unwrap();
    assert_eq!(said.lock().unwrap().as_slice(), ["你好"]);

    // 写盘被拒：一个字都不该说（没接管主链就别开口）。
    let said2: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let dir2 = TempDir::new("first-mes-rejected");
    let mut rejected = PersonaRuntime::new(
        services_with_say(&dir2, new_calls(), "BASE", false, said2.clone()),
        PersonaConfig::from_value(&config),
    );
    assert!(rejected.start(&mut MockRegistrar::default()).is_err());
    assert!(said2.lock().unwrap().is_empty(), "没接管主链就别开口");
}

/// config 里混进 `enabled` 不改变任何行为（启停只由 manifest 表达）。
#[test]
fn enabled_key_in_config_is_inert() {
    let dir = TempDir::new("enabled-inert");
    let calls_a = new_calls();
    let mut a = PersonaRuntime::new(
        harness_services(&dir, calls_a.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({
            "card_json": CARD_V2,
            "enabled": false,
        })),
    );
    a.start(&mut MockRegistrar::default()).unwrap();

    let dir_b = TempDir::new("enabled-inert-b");
    let calls_b = new_calls();
    let mut b = PersonaRuntime::new(
        harness_services(&dir_b, calls_b.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({"card_json": CARD_V2})),
    );
    b.start(&mut MockRegistrar::default()).unwrap();

    assert_eq!(
        patches(&calls_a),
        patches(&calls_b),
        "config.enabled 必须完全惰性"
    );
}

/// `shutdown` 幂等：第二次调用不再写主链。host 现在只调一次，但
/// `disable()` 与「runtime 被丢弃」两条路径都可能走到这里——写两次会把
/// 用户在此期间的手改覆盖掉。
#[test]
fn shutdown_is_idempotent() {
    let dir = TempDir::new("shutdown-twice");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({"card_json": CARD_V2})),
    );
    rt.start(&mut MockRegistrar::default()).unwrap();
    rt.shutdown().unwrap();
    assert_eq!(patches(&calls).len(), 2, "启用写一次 + 还原写一次");
    rt.shutdown().unwrap();
    assert_eq!(patches(&calls).len(), 2, "第二次 shutdown 不得再写一次");
}

/// 注册 schema 失败（host 侧异常）→ 连**已经写回**的主链提示词也一并回滚。
///
/// 一个 `Failed` 的 Mod 不得在主链上留痕：半接管比不接管更坏——界面显示
/// 「角色卡 Mod 失败」，人设却已经悄悄换了。
#[test]
fn register_failure_rolls_back_the_applied_prompt() {
    struct FailingRegistrar;
    impl ModRegistrar for FailingRegistrar {
        fn register_settings(&mut self, _: ModSettingsSpec) -> Result<(), ModError> {
            Err(ModError::InvalidSettings {
                mod_id: DESCRIPTOR.id.to_string(),
                message: "boom".to_string(),
            })
        }
        fn subscribe(&mut self, _: ModEventTopic) -> Result<SubscriptionId, ModError> {
            Ok(SubscriptionId(1))
        }
        fn unsubscribe(&mut self, _: SubscriptionId) -> Result<(), ModError> {
            Ok(())
        }
    }

    let dir = TempDir::new("register-fail");
    let calls = new_calls();
    let mut rt = PersonaRuntime::new(
        harness_services(&dir, calls.clone(), "BASE", true),
        PersonaConfig::from_value(&serde_json::json!({"card_json": CARD_V2})),
    );
    let err = rt
        .start(&mut FailingRegistrar)
        .expect_err("注册失败必须冒泡，不得谎报 Running");
    assert!(err.to_string().contains("boom"), "err = {err}");

    let got = patches(&calls);
    assert_eq!(got.len(), 2, "写回一次 + 回滚一次: ${got:?}");
    assert!(prompt_of(&got[0]).contains("NEKO"), "got = ${got:?}");
    assert_eq!(prompt_of(&got[1]), "BASE", "已写回的主链提示词必须还原");
}
