//! Wave 3 轨 E：`persona` 坏卡 E2E（`ModRuntime` 级启停循环）+ 与 `memory` 共存契约。
//!
//! 与 `src/tests.rs` 的分工：那边守 Wave 1/2 的覆盖面（卡解析纯函数、各条坏输入
//! 的**单点** `Err`、启停三条入口）；这里守 Wave 3 要求的两条新契约：
//!
//! 1. **坏卡 E2E**：`enable → Failed →（把卡/配置修好）→ 再 enable 真接管 →
//!    disable 回基线`。断言打在**主链 `system_prompt` 的值**上，而不是
//!    「`apply_settings` 被调了几次」——`Failed` 时值是逐字未动才算数。
//! 2. **与 memory 共存**：`docs/architecture/memory-mod-v0.md` §5.1 的
//!    last-writer-wins 三行。这里**直接调用 memory crate 的真实**
//!    `strategy::compose_injection` / `strip_memory_block`（不是本地复刻一份
//!    marker 规则）——两侧契约漂移会在这条测试上直接变红。
//!
//! 「假主链」：`apply_settings` 真的更新一份 `Arc<Mutex<String>>`，settings 快照
//! 读的也是同一份。于是「主链一字未动」是**值断言**，不是计数断言。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use live2d_ai_mod_memory::MEMORY_MARKER_BEGIN;
use live2d_ai_mod_memory::strategy::{
    compose_injection, contains_memory_block, strip_memory_block,
};

use super::*;
use crate::tests::{CARD_V2, MockRegistrar, TempDir, noop_services, png_with_chara, push_chunk};

/// 假主链的当前 `persona.system_prompt`。
type Chain = Arc<Mutex<String>>;

fn new_chain(initial: &str) -> Chain {
    Arc::new(Mutex::new(initial.to_string()))
}

fn prompt(chain: &Chain) -> String {
    chain.lock().unwrap().clone()
}

fn set_prompt(chain: &Chain, value: &str) {
    *chain.lock().unwrap() = value.to_string();
}

/// services：`apply_settings` 改的就是这份假主链，settings 快照读的也是它。
fn chain_services(dir: &TempDir, chain: &Chain) -> ModServices {
    let read = Arc::clone(chain);
    let write = Arc::clone(chain);
    noop_services()
        .with_apply_settings(ModSettingsApplier::new(move |patch| {
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

/// `CARD_V2` + 关掉纪律模板时应逐字写入主链的整段提示词。
///
/// 用**字面量**而不是复用实现里的 `compose_system_prompt`：断言不能与被断言者
/// 共用同一条公式，否则两边一起错也会全绿。
const NEKO_WITHOUT_DISCIPLINE: &str = "[角色] NEKO\n\n猫娘";

fn runtime(dir: &TempDir, chain: &Chain, config: serde_json::Value) -> PersonaRuntime {
    PersonaRuntime::new(
        chain_services(dir, chain),
        PersonaConfig::from_value(&config),
    )
}

// ------------------------------------------------- 坏卡 E2E（enable → Failed → 修好 → 启用）

/// 坏 JSON：enable → Failed（主链**值**一字未动、无基线快照）→ 修好 card_json →
/// 再 enable 逐字接管 → disable 回基线。
#[test]
fn bad_json_then_fixed_card_takes_over_then_disable_restores() {
    let dir = TempDir::new("e2e-bad-json");
    let chain = new_chain("BASE");

    let mut bad = runtime(
        &dir,
        &chain,
        serde_json::json!({"card_json": "{ not json", "include_discipline": false}),
    );
    let mut mock = MockRegistrar::default();
    let err = bad
        .start(&mut mock)
        .expect_err("坏 JSON 必须 Failed，而不是假装 Running");
    assert!(err.to_string().contains("card_json"), "err = {err}");
    assert!(mock.specs.is_empty(), "失败路径不得留下半个副作用");
    assert_eq!(prompt(&chain), "BASE", "Failed 时主链一字未动");
    assert!(
        !dir.path().join(BASE_STATE_FILE).exists(),
        "没接管就不该有基线快照"
    );

    let mut good = runtime(
        &dir,
        &chain,
        serde_json::json!({"card_json": CARD_V2, "include_discipline": false}),
    );
    good.start(&mut MockRegistrar::default())
        .expect("修好 card_json 后应启用成功");
    assert_eq!(
        prompt(&chain),
        NEKO_WITHOUT_DISCIPLINE,
        "修好后必须逐字接管主链"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join(BASE_STATE_FILE)).unwrap(),
        "BASE",
        "首次成功接管才落基线快照"
    );

    good.shutdown().unwrap();
    assert_eq!(prompt(&chain), "BASE", "disable 应回基线");
}

/// `card_json` 是合法 JSON 数组（不是角色卡对象）：同样 enable → Failed，
/// 修成对象卡后接管、停用回基线。
#[test]
fn card_json_array_then_fixed_card_takes_over_then_disable_restores() {
    let dir = TempDir::new("e2e-array-json");
    let chain = new_chain("BASE");

    let mut bad = runtime(
        &dir,
        &chain,
        serde_json::json!({"card_json": "[1,2,3]", "include_discipline": false}),
    );
    let err = bad
        .start(&mut MockRegistrar::default())
        .expect_err("数组不是角色卡，必须 Failed");
    assert!(
        err.to_string().contains("不是可识别的角色卡"),
        "err = {err}"
    );
    assert_eq!(prompt(&chain), "BASE");
    assert!(!dir.path().join(BASE_STATE_FILE).exists());

    let mut good = runtime(
        &dir,
        &chain,
        serde_json::json!({"card_json": CARD_V2, "include_discipline": false}),
    );
    good.start(&mut MockRegistrar::default())
        .expect("修好后应启用");
    assert_eq!(prompt(&chain), NEKO_WITHOUT_DISCIPLINE);
    good.shutdown().unwrap();
    assert_eq!(prompt(&chain), "BASE");
}

/// PNG 无 `chara` 块：enable → Failed；**把同一路径的文件换成带 chara 的 PNG** →
/// 再 enable 成功（证明「修好输入」与「重新启用」是同一条真实路径）→ 停用回基线。
#[test]
fn png_without_chara_then_replaced_with_a_real_card_png() {
    let dir = TempDir::new("e2e-png-no-chara");
    let chain = new_chain("BASE");

    let mut no_chara = PNG_SIGNATURE.to_vec();
    push_chunk(&mut no_chara, b"IEND", &[]);
    let path = dir.write("card.png", &no_chara);
    let config = serde_json::json!({
        "card_path": path.display().to_string(),
        "include_discipline": false,
    });

    let mut bad = runtime(&dir, &chain, config.clone());
    let err = bad
        .start(&mut MockRegistrar::default())
        .expect_err("PNG 无 chara 块必须 Failed");
    assert!(err.to_string().contains("没有角色卡数据"), "err = {err}");
    assert_eq!(prompt(&chain), "BASE", "Failed 时主链一字未动");

    // 修好：同一路径换成内嵌 base64 chara 的真卡。
    let b64 = base64::engine::general_purpose::STANDARD.encode(CARD_V2.as_bytes());
    dir.write("card.png", png_with_chara(&b64));

    let mut good = runtime(&dir, &chain, config);
    good.start(&mut MockRegistrar::default())
        .expect("换成真卡后应启用成功");
    assert_eq!(prompt(&chain), NEKO_WITHOUT_DISCIPLINE);
    good.shutdown().unwrap();
    assert_eq!(prompt(&chain), "BASE");
}

/// 非法字符（既不是 PNG 也不是 UTF-8 的字节）：enable → Failed；换成合法卡 → 接管。
#[test]
fn illegal_bytes_card_then_fixed_card_takes_over_then_disable_restores() {
    let dir = TempDir::new("e2e-illegal-bytes");
    let chain = new_chain("BASE");

    let path = dir.write("card.bin", [0xFF, 0xFE, 0x00, 0x01]);
    let config = serde_json::json!({
        "card_path": path.display().to_string(),
        "include_discipline": false,
    });

    let mut bad = runtime(&dir, &chain, config.clone());
    let err = bad
        .start(&mut MockRegistrar::default())
        .expect_err("非法字节必须 Failed");
    assert!(
        err.to_string().contains("不是 PNG 也不是 UTF-8"),
        "err = {err}"
    );
    assert_eq!(prompt(&chain), "BASE");
    assert!(!dir.path().join(BASE_STATE_FILE).exists());

    dir.write("card.bin", CARD_V2);
    let mut good = runtime(&dir, &chain, config);
    good.start(&mut MockRegistrar::default())
        .expect("换成合法卡后应启用成功");
    assert_eq!(prompt(&chain), NEKO_WITHOUT_DISCIPLINE);
    good.shutdown().unwrap();
    assert_eq!(prompt(&chain), "BASE");
}

// ------------------------------------------------- 与 memory 共存（§5.1 last-writer-wins）

/// §5.1 第 1+2+3 行：
/// persona 后写 → 记忆块被冲掉；memory 后写（下一轮）→ 在 persona 结果之上重拼；
/// memory 停用 → 剥掉 marker 写回，persona 的合成结果保留。
///
/// memory 侧的每一步都调用 **memory crate 的真实纯函数**（`compose_injection` /
/// `strip_memory_block` 正是 `MemoryRuntime::injection_patch` / `strip_residue`
/// 用的那两个函数）。
#[test]
fn persona_write_wipes_memory_block_then_memory_reinjects_on_top() {
    let dir = TempDir::new("coexist-persona-then-memory");
    let chain = new_chain("BASE");

    // 上一轮 memory 已经注入过（base + 记忆块）。
    let injected = compose_injection(&prompt(&chain), &["用户喜欢猫".to_string()]);
    set_prompt(&chain, &injected);
    assert!(contains_memory_block(&prompt(&chain)));
    assert!(injected.contains(MEMORY_MARKER_BEGIN));

    // persona 后写：整段替换 → 记忆块被冲掉。
    let mut persona = runtime(
        &dir,
        &chain,
        serde_json::json!({"card_json": CARD_V2, "include_discipline": false}),
    );
    persona.start(&mut MockRegistrar::default()).unwrap();
    let after_persona = prompt(&chain);
    assert_eq!(after_persona, NEKO_WITHOUT_DISCIPLINE);
    assert!(
        !contains_memory_block(&after_persona),
        "persona 后写必须把记忆块冲掉: {after_persona}"
    );

    // memory 后写（下一轮）：在 persona 的合成结果之上重拼，base 一字不动。
    let reinjected = compose_injection(&after_persona, &["用户喜欢猫".to_string()]);
    assert!(
        reinjected.starts_with(after_persona.as_str()),
        "base 必须保留"
    );
    assert!(contains_memory_block(&reinjected));
    assert!(reinjected.contains("- 用户喜欢猫"), "{reinjected}");
    set_prompt(&chain, &reinjected);

    // memory 停用：剥掉 marker 写回，persona 的合成结果保留、不留残留。
    let stripped = strip_memory_block(&prompt(&chain));
    assert!(!contains_memory_block(&stripped), "停用后不得留残留");
    assert_eq!(stripped, after_persona, "剥离应回到 persona 的合成结果");
}

/// §5.1 的对称面：**主链已有记忆块时，persona 坏卡 Failed 也不得动它**
/// （「没真的接管，就不许留下痕迹」与共存的交叉断言）。
#[test]
fn failed_persona_start_leaves_an_existing_memory_block_untouched() {
    let dir = TempDir::new("coexist-failed-keeps-block");
    let chain = new_chain("BASE");
    let injected = compose_injection(&prompt(&chain), &["旧记忆".to_string()]);
    set_prompt(&chain, &injected);

    let mut bad = runtime(
        &dir,
        &chain,
        serde_json::json!({"card_json": "[1,2,3]", "include_discipline": false}),
    );
    bad.start(&mut MockRegistrar::default())
        .expect_err("坏卡必须 Failed");
    assert_eq!(prompt(&chain), injected, "Failed 不得动主链（记忆块原样）");
    assert!(!dir.path().join(BASE_STATE_FILE).exists());
}

/// persona 的基线快照是**启用那一刻的整段** `system_prompt`：若当时含记忆块，
/// disable 会把它原样还回来（persona 不解析 memory 的 marker——这正是
/// last-writer-wins「没有仲裁」的直接推论；memory 下一轮会幂等重拼 / 停用时剥离）。
#[test]
fn persona_disable_restores_the_snapshot_even_if_it_contained_a_memory_block() {
    let dir = TempDir::new("coexist-disable-restores-block");
    let chain = new_chain("BASE");
    let injected = compose_injection(&prompt(&chain), &["用户养了一只猫".to_string()]);
    set_prompt(&chain, &injected);

    let mut persona = runtime(
        &dir,
        &chain,
        serde_json::json!({"card_json": CARD_V2, "include_discipline": false}),
    );
    persona.start(&mut MockRegistrar::default()).unwrap();
    assert!(
        !contains_memory_block(&prompt(&chain)),
        "启用期间是 persona 整段接管"
    );

    persona.shutdown().unwrap();
    assert_eq!(
        prompt(&chain),
        injected,
        "停用还回的是快照（整段），不是剥过块的 base"
    );
}

// ------------------------------------------------- L1 会话绑定（2026-09-15）
//
// 这一组守的是 PRODUCT-L1-GOALS 的 persona 验收句：「角色卡绑定当前会话；
// 换会话不串卡；回原会话仍在；停用还原」。它们跨「Mod 内存 / 宿主会话表 /
// 档案文件 / 进程重启」四个面，所以放在 e2e 文件里（tests_command.rs 守的是
// 单条命令的形状，且已接近 800 行）。

/// 另一张**好卡**（与 `CARD_V2` 的 name 不同，用来断言「哪个会话拿了哪张」）。
const OTHER_CARD: &str =
    r#"{"spec":"chara_card_v2","data":{"name":"OTHER","description":"另一张卡"}}"#;

/// **假会话表**：按来源槽（owner）真的记录 set / clear / clear_owner。
///
/// 为什么不用 `NoSessionPrompts`：`disabled()` 的 `set` 是 no-op，
/// 「绑上了」与「没绑」在断言里长得一模一样——L1 的证据必须打在**内容**上。
/// 也必须真的分槽：本次修复的验收是「清掉 persona 的槽后 memory 的贡献还在」，
/// owner 不分槽就验不到。
#[derive(Clone, Default)]
struct FakeSessions {
    inner: Arc<Mutex<FakeSessionsState>>,
}

#[derive(Default)]
struct FakeSessionsState {
    /// 会话 id → (owner → 文本)。
    slots: BTreeMap<String, BTreeMap<String, String>>,
    active: Option<String>,
    sets: Vec<(String, String)>,
    clears: Vec<String>,
    clear_owners: Vec<String>,
}

impl FakeSessions {
    fn new_enabled() -> Self {
        Self::default()
    }

    /// 组合结果（= supervisor 的读点）。
    fn prompt_of(&self, session: &str) -> Option<String> {
        SessionPromptSink::get(self, session)
    }

    /// **单个来源槽**的文本（验收「只碰自己的槽」用）。
    fn slot_of(&self, owner: &str, session: &str) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .slots
            .get(session)?
            .get(owner)
            .cloned()
    }

    fn bound(&self) -> Vec<String> {
        self.inner.lock().unwrap().slots.keys().cloned().collect()
    }

    fn set_calls(&self) -> Vec<(String, String)> {
        self.inner.lock().unwrap().sets.clone()
    }

    fn clears(&self) -> Vec<String> {
        self.inner.lock().unwrap().clears.clone()
    }

    fn clear_owners(&self) -> Vec<String> {
        self.inner.lock().unwrap().clear_owners.clone()
    }

    fn set_active(&self, session: Option<&str>) {
        self.inner.lock().unwrap().active = session.map(str::to_string);
    }
}

impl SessionPromptSink for FakeSessions {
    fn get(&self, session: &str) -> Option<String> {
        let s = self.inner.lock().unwrap();
        let slots = s.slots.get(session)?;
        live2d_ai_mod_system::compose_session_prompt(
            slots
                .iter()
                .map(|(owner, text)| (owner.as_str(), text.as_str())),
        )
    }

    fn set_owned(&self, owner: &str, session: &str, prompt: &str) {
        let mut s = self.inner.lock().unwrap();
        s.sets.push((session.to_string(), prompt.to_string()));
        s.slots
            .entry(session.to_string())
            .or_default()
            .insert(owner.to_string(), prompt.to_string());
    }

    fn clear_owned(&self, owner: &str, session: &str) -> bool {
        let mut s = self.inner.lock().unwrap();
        let mut removed = false;
        let mut drop_session = false;
        if let Some(slots) = s.slots.get_mut(session) {
            removed = slots.remove(owner).is_some();
            drop_session = slots.is_empty();
        }
        if drop_session {
            s.slots.remove(session);
        }
        removed
    }

    fn clear_owner(&self, owner: &str) {
        let mut s = self.inner.lock().unwrap();
        s.clear_owners.push(owner.to_string());
        let ids: Vec<String> = s.slots.keys().cloned().collect();
        for id in ids {
            let mut drop_session = false;
            if let Some(slots) = s.slots.get_mut(&id) {
                slots.remove(owner);
                drop_session = slots.is_empty();
            }
            if drop_session {
                s.slots.remove(&id);
            }
        }
    }

    fn contributions(&self, session: &str) -> Vec<(String, String)> {
        let s = self.inner.lock().unwrap();
        let Some(slots) = s.slots.get(session) else {
            return Vec::new();
        };
        let mut out: Vec<(String, String)> = slots
            .iter()
            .map(|(owner, text)| (owner.clone(), text.clone()))
            .collect();
        out.sort_by(|a, b| {
            live2d_ai_mod_system::owner_merge_rank(&a.0)
                .cmp(&live2d_ai_mod_system::owner_merge_rank(&b.0))
        });
        out
    }

    fn set(&self, session: &str, prompt: &str) {
        let mut s = self.inner.lock().unwrap();
        s.sets.push((session.to_string(), prompt.to_string()));
        s.slots
            .entry(session.to_string())
            .or_default()
            .insert(String::new(), prompt.to_string());
    }

    fn clear(&self, session: &str) -> bool {
        let mut s = self.inner.lock().unwrap();
        s.clears.push(session.to_string());
        s.slots.remove(session).is_some()
    }

    fn clear_all(&self) {
        let mut s = self.inner.lock().unwrap();
        s.slots.clear();
    }

    fn sessions(&self) -> Vec<String> {
        self.bound()
    }

    fn active(&self) -> Option<String> {
        self.inner.lock().unwrap().active.clone()
    }

    fn set_active(&self, session: Option<&str>) {
        self.inner.lock().unwrap().active = session.map(str::to_string);
    }
}

/// 假主链 + 假会话表（config_path 由 `chain_services` 注入）。
fn session_services(dir: &TempDir, chain: &Chain, sink: &FakeSessions) -> ModServices {
    chain_services(dir, chain).with_session_prompts(ModSessionPrompts::new(sink.clone()))
}

fn session_runtime(
    dir: &TempDir,
    chain: &Chain,
    config: serde_json::Value,
    sink: &FakeSessions,
) -> PersonaRuntime {
    PersonaRuntime::new(
        session_services(dir, chain, sink),
        PersonaConfig::from_value(&config),
    )
}

fn archive_path(dir: &TempDir) -> PathBuf {
    dir.path().join(sessions::SESSION_CARDS_FILE)
}

fn card_value(text: &str) -> serde_json::Value {
    PersonaCard::parse_json(text)
        .expect("测试卡必须能解析")
        .to_json()
}

fn import_for(rt: &mut PersonaRuntime, card: &str, session: &str) -> serde_json::Value {
    rt.command(
        "import_card",
        &serde_json::json!({"card_json": card, "session_id": session}),
    )
    .expect("会话导入应成功")
}

/// **换会话不串卡 + 绝不写全局**：A 的卡只进 A 的桶，B 是 None，主链一字未动。
#[test]
fn session_import_binds_only_the_target_session_and_leaves_the_global_chain_alone() {
    let dir = TempDir::new("l1-session-bind");
    let chain = new_chain("BASE");
    let sink = FakeSessions::new_enabled();
    let mut rt = session_runtime(
        &dir,
        &chain,
        serde_json::json!({"include_discipline": false}),
        &sink,
    );
    rt.start(&mut MockRegistrar::default()).unwrap();

    let result = import_for(&mut rt, CARD_V2, "A");
    assert_eq!(result["ok"], true);
    assert_eq!(result["scope"], "session");
    assert_eq!(result["session_id"], "A");
    assert_eq!(result["card_name"], "NEKO");
    assert_eq!(result["card_format"], "v2");

    assert_eq!(
        sink.prompt_of("A").as_deref(),
        Some(NEKO_WITHOUT_DISCIPLINE),
        "A 会话拿到的必须正好是这张卡的合成结果"
    );
    assert_eq!(sink.prompt_of("B"), None, "换会话不串卡");
    assert_eq!(prompt(&chain), "BASE", "会话导入绝不写全局主链");
    assert!(
        !dir.path().join(IMPORTED_CARD_FILE).exists(),
        "会话导入不该同时写全局导入卡"
    );

    let s = rt.state_json().unwrap();
    assert_eq!(s["session_bound"], true);
    assert_eq!(s["sessions"], serde_json::json!(["A"]));
    assert_eq!(s["scope"], "global", "没有活动会话时作用域是全局");
}

/// **清一个会话不碰另一个**；幂等；全局主链不受影响。
#[test]
fn clearing_one_session_leaves_the_other_session_bound() {
    let dir = TempDir::new("l1-session-clear-one");
    let chain = new_chain("BASE");
    let sink = FakeSessions::new_enabled();
    let mut rt = session_runtime(
        &dir,
        &chain,
        serde_json::json!({"include_discipline": false}),
        &sink,
    );
    rt.start(&mut MockRegistrar::default()).unwrap();

    import_for(&mut rt, CARD_V2, "A");
    import_for(&mut rt, OTHER_CARD, "B");
    assert!(sink.prompt_of("A").unwrap().contains("[角色] NEKO"));
    assert!(sink.prompt_of("B").unwrap().contains("[角色] OTHER"));

    let cleared = rt
        .command("clear_import", &serde_json::json!({"session_id": "A"}))
        .expect("清除一个会话应成功");
    assert_eq!(cleared["ok"], true);
    assert_eq!(cleared["scope"], "session");
    assert_eq!(cleared["cleared"], true);
    assert_eq!(sink.prompt_of("A"), None, "清除的会话必须真的没了");
    assert!(
        sink.prompt_of("B").unwrap().contains("[角色] OTHER"),
        "另一个会话不得被顺手清掉"
    );
    assert_eq!(prompt(&chain), "BASE", "会话清除不碰全局主链");
    assert_eq!(
        rt.state_json().unwrap()["sessions"],
        serde_json::json!(["B"])
    );

    // 幂等：再清一次是 cleared=false，不是错误。
    let again = rt
        .command("clear_import", &serde_json::json!({"session_id": "A"}))
        .expect("重复清除应幂等");
    assert_eq!(again["cleared"], false);
}

/// **本次修复的验收**：persona 清掉自己的会话槽之后，另一个 owner（memory）
/// 在同一会话里的贡献**还在**——这正是旧实现 clear_all 会误伤的场景。
#[test]
fn clearing_a_persona_session_leaves_another_owners_contribution() {
    let dir = TempDir::new("l1-session-owner-isolation");
    let chain = new_chain("BASE");
    let sink = FakeSessions::new_enabled();
    let mut rt = session_runtime(
        &dir,
        &chain,
        serde_json::json!({"include_discipline": false}),
        &sink,
    );
    rt.start(&mut MockRegistrar::default()).unwrap();
    import_for(&mut rt, CARD_V2, "A");
    // 另一个来源（memory）在同一个会话里写自己的槽。
    sink.set_owned(SESSION_PROMPT_OWNER_MEMORY, "A", "用户喜欢薄荷");

    let cleared = rt
        .command("clear_import", &serde_json::json!({"session_id": "A"}))
        .expect("清除 persona 的会话卡应成功");
    assert_eq!(cleared["ok"], true);
    assert_eq!(cleared["cleared"], true);
    assert_eq!(
        sink.slot_of(SESSION_PROMPT_OWNER_PERSONA, "A"),
        None,
        "persona 的槽必须真的被清掉"
    );
    assert_eq!(
        sink.prompt_of("A").as_deref(),
        Some("用户喜欢薄荷"),
        "persona 清除不得误伤别的来源（memory 的贡献仍在）"
    );
    assert!(
        sink.bound().contains(&"A".to_string()),
        "会话条目仍在（还有别的来源的槽）"
    );
}

/// **回原会话仍在**：档案文件 round-trip——同一目录新建 runtime + start，
/// 宿主会话表被重新灌回。
#[test]
fn session_cards_survive_a_restart_through_the_archive_file() {
    let dir = TempDir::new("l1-session-restart");
    let chain = new_chain("BASE");
    let config = serde_json::json!({"include_discipline": false});

    let first_sink = FakeSessions::new_enabled();
    let mut first = session_runtime(&dir, &chain, config.clone(), &first_sink);
    first.start(&mut MockRegistrar::default()).unwrap();
    import_for(&mut first, CARD_V2, "A");

    // 档案形状是契约：{"version":1,"sessions":{"A":{<规范化 V2 卡>}}}
    let raw = std::fs::read_to_string(archive_path(&dir)).expect("会话档案应落盘");
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("档案必须是 JSON");
    assert_eq!(doc["version"], 1);
    assert_eq!(doc["sessions"]["A"]["data"]["name"], "NEKO");

    // 「进程重启」：新的 runtime、**空的**宿主表、同一个目录。
    let second_sink = FakeSessions::new_enabled();
    let mut second = session_runtime(&dir, &chain, config, &second_sink);
    second.start(&mut MockRegistrar::default()).unwrap();
    assert_eq!(
        second_sink.prompt_of("A").as_deref(),
        Some(NEKO_WITHOUT_DISCIPLINE),
        "重启后回到会话 A，人设必须还在"
    );
    assert_eq!(second_sink.prompt_of("B"), None, "恢复也不许串到别的会话");
}

/// **停用还原**：shutdown 清空宿主表的**全部**会话，同时保留老的全局基线还原。
#[test]
fn shutdown_clears_every_session_binding_and_still_restores_the_global_baseline() {
    let dir = TempDir::new("l1-session-shutdown");
    let chain = new_chain("BASE");
    let sink = FakeSessions::new_enabled();
    let mut rt = session_runtime(
        &dir,
        &chain,
        serde_json::json!({"card_json": CARD_V2, "include_discipline": false}),
        &sink,
    );
    rt.start(&mut MockRegistrar::default()).unwrap();
    // 全局卡已经接管主链（老语义），再给 A 绑一张会话卡。
    assert_eq!(prompt(&chain), NEKO_WITHOUT_DISCIPLINE);
    import_for(&mut rt, OTHER_CARD, "A");
    import_for(&mut rt, OTHER_CARD, "B");

    rt.shutdown().unwrap();
    assert!(
        sink.bound().is_empty(),
        "停用即还原：persona 的会话槽必须清空"
    );
    assert_eq!(
        sink.clear_owners(),
        vec![SESSION_PROMPT_OWNER_PERSONA.to_string()],
        "只清自己的 owner，不许 clear_all"
    );
    assert_eq!(prompt(&chain), "BASE", "老的全局基线还原也必须在");
    assert!(
        archive_path(&dir).exists(),
        "停用不删档案：下次启用要能灌回来"
    );
}

/// **非法 session_id 不写任何地方**：不落档案、不碰宿主表、不动全局主链。
#[test]
fn an_invalid_session_id_is_rejected_without_writing_anywhere() {
    let dir = TempDir::new("l1-session-invalid");
    let chain = new_chain("BASE");
    let sink = FakeSessions::new_enabled();
    let mut rt = session_runtime(&dir, &chain, serde_json::json!({}), &sink);
    rt.start(&mut MockRegistrar::default()).unwrap();

    for bad in ["../x", "a/b", "会话", "a b"] {
        let err = rt
            .command(
                "import_card",
                &serde_json::json!({"card_json": CARD_V2, "session_id": bad}),
            )
            .expect_err("非法 session_id 必须失败");
        assert!(err.to_string().contains("session_id"), "err = {err}");
        let err = rt
            .command("clear_import", &serde_json::json!({"session_id": bad}))
            .expect_err("非法 session_id 的清除也必须失败");
        assert!(err.to_string().contains("session_id"), "err = {err}");
    }
    assert!(sink.bound().is_empty());
    assert!(sink.set_calls().is_empty(), "非法 id 不得触发任何 set");
    assert!(sink.clears().is_empty(), "非法 id 不得触发任何 clear");
    assert!(!archive_path(&dir).exists(), "非法 id 不得落档案");
    assert_eq!(prompt(&chain), "BASE", "非法 id 不得动全局主链");

    // 空串 = 「没给」：老语义（全局导入），不是错误——这是与非法 id 的关键区别。
    let global = rt
        .command(
            "import_card",
            &serde_json::json!({"card_json": CARD_V2, "session_id": "  "}),
        )
        .expect("空 session_id 按全局处理");
    assert_eq!(global["scope"], "global");
    assert!(prompt(&chain).contains("[角色] NEKO"));
}

/// 本环境没有会话能力时，会话导入**明确失败**，不静默降级成全局写回。
#[test]
fn session_import_without_a_host_sink_or_a_config_path_fails_readably() {
    // ① 有会话表、没有 config_path（单测宿主）：档案无处落，明确报 config_path。
    let sink = FakeSessions::new_enabled();
    let mut no_path = PersonaRuntime::new(
        noop_services().with_session_prompts(ModSessionPrompts::new(sink.clone())),
        PersonaConfig::from_value(&serde_json::json!({})),
    );
    no_path.start(&mut MockRegistrar::default()).unwrap();
    let err = no_path
        .command(
            "import_card",
            &serde_json::json!({"card_json": CARD_V2, "session_id": "A"}),
        )
        .expect_err("没有落点必须失败");
    assert!(err.to_string().contains("config_path"), "err = {err}");
    assert!(sink.bound().is_empty(), "失败不许悄悄绑上");

    // ② 有 config_path、没有会话表：报「没有会话绑定能力」，且不落档案。
    let dir = TempDir::new("l1-session-nosink");
    let chain = new_chain("BASE");
    let mut no_sink = runtime(&dir, &chain, serde_json::json!({}));
    no_sink.start(&mut MockRegistrar::default()).unwrap();
    let err = no_sink
        .command(
            "import_card",
            &serde_json::json!({"card_json": CARD_V2, "session_id": "A"}),
        )
        .expect_err("没有会话表必须失败");
    assert!(err.to_string().contains("会话"), "err = {err}");
    assert!(!archive_path(&dir).exists());
}

/// **坏档案 = warn + 忽略，不是 Mod Failed**（它是派生缓存，不是用户配的真源）。
#[test]
fn a_corrupt_or_oversized_session_archive_is_ignored_not_fatal() {
    let dir = TempDir::new("l1-session-corrupt");
    let chain = new_chain("BASE");
    let config = serde_json::json!({"card_json": CARD_V2, "include_discipline": false});

    for bad in ["{ not json", r#"{"version":9,"sessions":{}}"#] {
        dir.write(sessions::SESSION_CARDS_FILE, bad);
        let sink = FakeSessions::new_enabled();
        let mut rt = session_runtime(&dir, &chain, config.clone(), &sink);
        rt.start(&mut MockRegistrar::default())
            .expect("坏档案不得让 Mod Failed");
        assert!(sink.bound().is_empty(), "坏档案不得灌进任何会话");
        assert_eq!(prompt(&chain), NEKO_WITHOUT_DISCIPLINE, "全局卡照常接管");
    }

    // 会话数超上限（51 > 50）→ 整份忽略。
    let mut many = serde_json::Map::new();
    for i in 0..51 {
        many.insert(format!("s{i}"), card_value(CARD_V2));
    }
    dir.write(
        sessions::SESSION_CARDS_FILE,
        serde_json::json!({"version": 1, "sessions": many}).to_string(),
    );
    let sink = FakeSessions::new_enabled();
    let mut rt = session_runtime(&dir, &chain, config, &sink);
    rt.start(&mut MockRegistrar::default())
        .expect("超限档案也只是忽略");
    assert!(sink.bound().is_empty());
}

/// 会话数上限在**导入时**也要把住：新增第 51 个 → 可读错误，档案不被写坏。
#[test]
fn the_session_cap_is_enforced_on_import_with_a_readable_error() {
    let dir = TempDir::new("l1-session-cap");
    let chain = new_chain("BASE");
    let mut many = serde_json::Map::new();
    for i in 0..50 {
        many.insert(format!("s{i}"), card_value(CARD_V2));
    }
    dir.write(
        sessions::SESSION_CARDS_FILE,
        serde_json::json!({"version": 1, "sessions": many}).to_string(),
    );

    let sink = FakeSessions::new_enabled();
    let mut rt = session_runtime(
        &dir,
        &chain,
        serde_json::json!({"include_discipline": false}),
        &sink,
    );
    rt.start(&mut MockRegistrar::default()).unwrap();
    assert_eq!(sink.bound().len(), 50, "启动应恢复满 50 个");

    // 覆盖**已有**会话不算新增：必须仍然成功。
    import_for(&mut rt, OTHER_CARD, "s0");
    assert!(sink.prompt_of("s0").unwrap().contains("[角色] OTHER"));

    let err = rt
        .command(
            "import_card",
            &serde_json::json!({"card_json": OTHER_CARD, "session_id": "s50"}),
        )
        .expect_err("新增第 51 个会话必须失败");
    assert!(err.to_string().contains("上限"), "err = {err}");
    assert_eq!(sink.prompt_of("s50"), None);
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(archive_path(&dir)).unwrap()).unwrap();
    assert_eq!(
        doc["sessions"].as_object().unwrap().len(),
        50,
        "失败的导入不得写坏档案"
    );
}

/// `state_json.scope` 跟着宿主的**活动会话**走（面板要据此显示「当前会话已绑定」）。
#[test]
fn state_scope_follows_the_hosts_active_session() {
    let dir = TempDir::new("l1-session-scope");
    let chain = new_chain("BASE");
    let sink = FakeSessions::new_enabled();
    let mut rt = session_runtime(
        &dir,
        &chain,
        serde_json::json!({"include_discipline": false}),
        &sink,
    );
    rt.start(&mut MockRegistrar::default()).unwrap();
    import_for(&mut rt, CARD_V2, "A");

    sink.set_active(Some("A"));
    let s = rt.state_json().unwrap();
    assert_eq!(s["active_session"], "A");
    assert_eq!(s["scope"], "session", "活动会话已绑定 → 会话作用域");

    sink.set_active(Some("B"));
    assert_eq!(
        rt.state_json().unwrap()["scope"],
        "global",
        "活动会话没绑定 → 回落全局"
    );

    sink.set_active(None);
    let s = rt.state_json().unwrap();
    assert!(s["active_session"].is_null());
    assert_eq!(s["scope"], "global");
}

/// **只列 persona 自己的槽**：宿主会话表是**共享的**（memory 也往里写），
/// `state_json.sessions` 若直接取 `sessions()`，会把「只有 memory 绑定过的会话」
/// 也算成「persona 已绑定」——那是界面在说谎，也会让面板的「已绑定 N 个会话」对不上。
#[test]
fn state_sessions_lists_only_personas_own_slots() {
    let dir = TempDir::new("l1-session-own-slots");
    let chain = new_chain("BASE");
    let sink = FakeSessions::new_enabled();
    let mut rt = session_runtime(
        &dir,
        &chain,
        serde_json::json!({"include_discipline": false}),
        &sink,
    );
    rt.start(&mut MockRegistrar::default()).unwrap();
    import_for(&mut rt, CARD_V2, "A");

    // 另一个来源（memory）在**别的会话**里写了内容。
    sink.set_owned(
        live2d_ai_mod_system::SESSION_PROMPT_OWNER_MEMORY,
        "B",
        "记忆块",
    );

    let s = rt.state_json().unwrap();
    assert_eq!(
        s["sessions"],
        serde_json::json!(["A"]),
        "只应列出 persona 自己有贡献的会话；memory-only 的 B 不该被算进来：{s}"
    );
}
