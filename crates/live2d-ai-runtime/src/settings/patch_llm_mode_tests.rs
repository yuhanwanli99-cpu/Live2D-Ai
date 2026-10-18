//! `[llm]` 云 / 本地二选一（`[llm].mode`，2026-10-18 0.3.7 T1）的回归。
//!
//! 为什么要单列：`patch_tests.rs` / `patch_new_tests.rs` 已贴近 `crates/*/src > 800`
//! 的测试行数纪律，且本组断言是**新契约**（旧的测试一字未改）：
//!
//! 1. 缺省 = `cloud`，旧 toml（没有 `mode` 键）**逐字 round-trip**；
//! 2. `local` 只是标签：transport / 鉴权 / 厂商快照语义都不变；
//! 3. 未知 mode 由 serde 拒绝（路由层据此回 400），不会静默落一个坏值；
//! 4. 不自动回退：local 也好 cloud 也好，`resolve` 只按当前段原样产出。
//!
//! 自检（`GET {base_url}/models` 的分类）在 desktop 侧
//! （`settings_routes/test_endpoints.rs`），这里只管配置语义。

use crate::settings::patch::{PatchOutcome, SettingsPatch, apply_patch};
use crate::settings::view::{settings_to_view, settings_to_view_with_keys};
use crate::settings::{AppSettings, LlmMode, LlmVendor};

/// 一份**旧 toml（0.3.6 形态：`[llm]` 段没有 `mode` 键）**，带手写注释与行尾注释。
///
/// 覆盖所有会被序列化出的键（否则 merge 会「补」键，就不是逐字 round-trip 了）。
const OLD_TOML: &str = r#"# 旧配置（0.3.6 形态）：[llm] 段没有 mode 键
dev_mode = false

[llm]
# 云端点
base_url = "https://api.deepseek.com/v1"   # 行尾注释
model = "deepseek-flash"

[tts]
mode = "local"
base_url = "http://127.0.0.1:8091/v1"
voice = "ZH"
response_format = "pcm"
sample_rate = 44100
channels = 1

[persona]
system_prompt = ""
max_history_pairs = 0

[action]
head_scale = 0.75
body_scale = 0.8
expression_scale = 1.0

[performance]
enabled = false
base_url = ""
model = ""
timeout_ms = 4000
structured = "auto"
"#;

fn parse(text: &str) -> AppSettings {
    AppSettings::from_toml_str(text).expect("测试配置应可解析")
}

fn no_keys(_: &str) -> Option<String> {
    None
}

/// 一份**云端形态 + 厂商标签/快照**的样本（快照语义回归的起点）。
fn cloud_with_vendor() -> AppSettings {
    let mut s = parse(OLD_TOML);
    s.llm.base_url = "https://api.deepseek.com/v1".into();
    s.llm.model = "deepseek-flash".into();
    s.llm.api_key_env = Some("DEEPSEEK_API_KEY".into());
    s.llm.max_tokens = Some(4096);
    s.llm.show_reasoning = Some(true);
    s.llm.vendor = Some("deepseek".into());
    s.llm.vendors.insert(
        "deepseek".into(),
        LlmVendor {
            base_url: "https://api.deepseek.com/v1".into(),
            model: "deepseek-flash".into(),
            api_key_env: Some("DEEPSEEK_API_KEY".into()),
            max_tokens: Some(4096),
            show_reasoning: Some(true),
        },
    );
    s
}

// ───────────────────────── 缺省 + 旧 toml 逐字 round-trip

#[test]
fn llm_mode_defaults_to_cloud_and_old_toml_roundtrips_verbatim() {
    // ① 字段缺省 = cloud（`#[serde(default)]` + `Default`）。
    assert_eq!(AppSettings::default().llm.mode, LlmMode::Cloud);
    assert_eq!(LlmMode::default().as_str(), "cloud");

    // ② 旧 toml（没有 mode 键）解析后语义 = cloud。
    let old = parse(OLD_TOML);
    assert_eq!(old.llm.mode, LlmMode::Cloud, "旧 toml 无 mode 必须 = cloud");

    // ③ PATCH 真实写回路径（merge_into_toml）：读到什么写回什么，**逐字相同**
    //    ——不得凭空长出 `mode = "cloud"`，也不得丢注释/行尾注释。
    let merged = old.merge_into_toml(OLD_TOML).expect("合并应成功");
    assert_eq!(merged, OLD_TOML, "旧 toml 无 mode 必须逐字 round-trip");

    // ④ 直接序列化同样不写缺省值（skip_serializing_if）。
    let written = old.to_toml_string();
    assert!(
        !written.contains("mode = \"cloud\""),
        "cloud 是缺省值，不得落盘：\n{written}"
    );
    let back = AppSettings::from_toml_str(&written).expect("re-parse");
    assert_eq!(back, old);

    // ⑤ 视图给稳定字符串（前端据此选中二选一；缺省 = "cloud"）。
    assert_eq!(settings_to_view(&old).llm.mode, "cloud");
}

/// 旧手写 toml 切到本地：**注释与行尾注释都留住**，只多出一行 `mode = "local"`。
///
/// 这是 PATCH 真实写回路径（merge_into_toml）的行为：与 rc.5/rc.6 同一条纪律——
/// 改一个值不许抹掉用户手写的说明。
#[test]
fn switching_old_toml_to_local_keeps_comments() {
    let mut s = parse(OLD_TOML);
    s.llm.mode = LlmMode::Local;
    let merged = s.merge_into_toml(OLD_TOML).expect("合并应成功");
    for needle in ["# 旧配置（0.3.6 形态）", "# 云端点", "# 行尾注释", "[tts]"] {
        assert!(
            merged.contains(needle),
            "切本地后丢了注释 {needle}：\n{merged}"
        );
    }
    assert!(
        merged.contains("mode = \"local\""),
        "切换后要写进 [llm].mode：\n{merged}"
    );
    // 除新增的那一行外，旧内容逐字仍在（每行都还在）。
    for line in OLD_TOML.lines() {
        assert!(merged.contains(line), "旧行被改动/丢失：{line:?}\n{merged}");
    }
}

#[test]
fn llm_local_mode_serializes_into_llm_section_and_round_trips() {
    let mut s = parse(OLD_TOML);
    s.llm.mode = LlmMode::Local;
    let written = s.to_toml_string();
    assert!(
        written.contains("mode = \"local\""),
        "local 必须落盘：\n{written}"
    );
    let back = AppSettings::from_toml_str(&written).expect("re-parse");
    assert_eq!(back.llm.mode, LlmMode::Local);
    assert_eq!(back, s, "local 模式 round-trip 必须逐字段相等");
    assert_eq!(settings_to_view(&back).llm.mode, "local");
}

// ───────────────────────── PATCH：未知 mode 拒绝

#[test]
fn unknown_llm_mode_is_rejected_by_serde() {
    // PATCH body 里 mode 是**普通可选值**（不是三态）：未知取值 serde 直接报错，
    // 路由层映射 400 invalid_payload。静默忽略 = 用户以为切了、实际没切。
    for body in [
        r#"{"llm":{"mode":"bogus"}}"#,
        r#"{"llm":{"mode":""}}"#,
        r#"{"llm":{"mode":1}}"#,
    ] {
        assert!(
            serde_json::from_str::<SettingsPatch>(body).is_err(),
            "未知 mode 必须被拒：{body}"
        );
    }
    // 合法值能解析（`null` / 缺键 = 不改，见 next 测试）。
    for (raw, want) in [("cloud", LlmMode::Cloud), ("local", LlmMode::Local)] {
        let body = format!(r#"{{"llm":{{"mode":"{raw}"}}}}"#);
        let patch: SettingsPatch = serde_json::from_str(&body).expect("合法 mode 应可解析");
        let inner = patch.llm.expect("段存在").expect("字段级补丁");
        assert_eq!(inner.mode, Some(want));
    }
}

// ───────────────────────── PATCH：只动 mode，vendor 快照语义不变

#[test]
fn patch_llm_mode_switch_only_touches_mode() {
    let s = cloud_with_vendor();
    let before = s.clone();

    // 切本地：地址 / 模型 / 密钥绑定 / token / 思考 / 标签 / 快照**全都不动**。
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"mode":"local"}}"#).expect("补丁应可解析");
    let (next, outcome) = apply_patch(&s, &patch).expect("切模式应成功");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.llm.mode, LlmMode::Local);
    assert_eq!(next.llm.base_url, before.llm.base_url);
    assert_eq!(next.llm.model, before.llm.model);
    assert_eq!(next.llm.api_key_env, before.llm.api_key_env);
    assert_eq!(next.llm.max_tokens, before.llm.max_tokens);
    assert_eq!(next.llm.show_reasoning, before.llm.show_reasoning);
    assert_eq!(next.llm.vendor, before.llm.vendor, "切模式不动厂商标签");
    assert_eq!(next.llm.vendors, before.llm.vendors, "切模式不动快照表");
    // vendor_modified 语义不变：当前值与标签快照仍一致 → false。
    assert!(
        !settings_to_view_with_keys(&next, &no_keys)
            .llm
            .vendor_modified
    );

    // 重复一次 = NoChange（幂等，不写盘）。
    let (same, outcome) = apply_patch(&next, &patch).expect("apply again");
    assert_eq!(outcome, PatchOutcome::NoChange);
    assert_eq!(same, next);

    // 切回云端同理：原值原样保留（local 失败/切回都不改写配置）。
    let back: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"mode":"cloud"}}"#).expect("补丁应可解析");
    let (back, outcome) = apply_patch(&next, &back).expect("切回云端应成功");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(back.llm.mode, LlmMode::Cloud);
    assert_eq!(back.llm.base_url, before.llm.base_url);
}

/// 界面切模式会**同时**发 `vendor: null`（与 TTS 的 `_switchMode` 逐条对齐）：
/// 服务端只清标签，**不复制、不删除快照**，也不动当前值。
#[test]
fn patch_llm_mode_with_vendor_clear_keeps_snapshot_and_values() {
    let s = cloud_with_vendor();
    let before = s.llm.clone();
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"mode":"local","vendor":null}}"#).expect("补丁应可解析");
    let (next, outcome) = apply_patch(&s, &patch).expect("应成功");
    assert_eq!(outcome, PatchOutcome::Updated, "清标签也是变更");
    assert_eq!(next.llm.mode, LlmMode::Local);
    assert_eq!(next.llm.vendor, None, "只清标签");
    assert_eq!(next.llm.vendors, before.vendors, "快照表原样保留");
    assert_eq!(next.llm.base_url, before.base_url, "当前值原样保留");
    assert_eq!(next.llm.api_key_env, before.api_key_env, "密钥绑定不清");
}

/// 段级显式清空（`llm: null`）= 整段回出厂形态 → 模式也回缺省 cloud。
#[test]
fn clearing_llm_section_resets_mode_to_cloud() {
    let mut s = cloud_with_vendor();
    s.llm.mode = LlmMode::Local;
    let patch: SettingsPatch = serde_json::from_str(r#"{"llm":null}"#).expect("补丁应可解析");
    let (next, outcome) = apply_patch(&s, &patch).expect("apply clear");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.llm.mode, LlmMode::Cloud);
}

// ───────────────────────── 不自动回退 / transport 不变

/// 模式**不参与** `resolve`：同一份地址 / 模型 / 密钥，local 与 cloud 解析出
/// 完全相同的 transport（同一条 OpenAI-compatible /chat/completions）。
/// 这是「local 失败不自动回退 cloud」的结构性保证：链路根本没有第二个端点。
#[test]
fn mode_is_not_part_of_transport_and_never_falls_back() {
    let cloud = cloud_with_vendor();
    let mut local = cloud.clone();
    local.llm.mode = LlmMode::Local;
    let lookup = |name: &str| (name == "DEEPSEEK_API_KEY").then(|| "sk-test".to_string());

    let rc = cloud.resolve_with(lookup).expect("cloud resolve");
    let rl = local.resolve_with(lookup).expect("local resolve");
    assert_eq!(rc.llm, rl.llm, "local / cloud 的 transport 必须完全相同");
    assert_eq!(rl.llm.base_url, "https://api.deepseek.com/v1");
    assert_eq!(rl.llm.model, "deepseek-flash");
    assert_eq!(
        rl.llm.api_key.as_ref().map(|k| k.expose_secret()),
        Some("sk-test"),
        "是否带鉴权仍只由 api_key_env 决定"
    );

    // 本地模式连不上时，`resolve` 也不会「切」出第二个端点：它仍只有一份 llm。
    let mut dead_local = local.clone();
    dead_local.llm.base_url = "http://127.0.0.1:1/v1".into();
    let rd = dead_local
        .resolve_with(no_keys)
        .expect("配置本身合法，仍可 resolve");
    assert_eq!(rd.llm.base_url, "http://127.0.0.1:1/v1");
    assert_eq!(dead_local.llm.mode, LlmMode::Local, "失败不改写配置");
}

/// 三态 JSON 里的 `"mode": null` 与缺键一样 = 不改（普通可选值，不是三态）。
#[test]
fn null_or_absent_mode_keeps_current_value() {
    let mut s = cloud_with_vendor();
    s.llm.mode = LlmMode::Local;
    for body in [r#"{"llm":{"mode":null}}"#, r#"{"llm":{}}"#] {
        let patch: SettingsPatch = serde_json::from_str(body).expect("补丁应可解析");
        let (next, outcome) = apply_patch(&s, &patch).expect("no-op");
        assert_eq!(outcome, PatchOutcome::NoChange, "{body}");
        assert_eq!(next.llm.mode, LlmMode::Local, "{body}");
    }
}
