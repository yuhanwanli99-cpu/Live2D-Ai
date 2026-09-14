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
