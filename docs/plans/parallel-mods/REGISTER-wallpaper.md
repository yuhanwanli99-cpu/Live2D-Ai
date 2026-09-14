# REGISTER — `wallpaper`（Wave 1 → 集成 PR 用）

> ⚠️ **已过时（2026-09-14 Wave 2 起）**：本文件记的是 Wave 1 的**注册待办**，
> 注册早在 `0.2.0-rc.2` 完成；下面 §3 的「落点未接线 / `apply_decision` 是明文占位」
> 也已被 Wave 2 B 轨取代（`apply_decision` 函数已从 crate **删除**，决策现在经
> `state_json` 的 `prefs_patch` 落到 `DisplayPrefs`）。**现行契约与接线清单看
> [`REGISTER-wallpaper-wire.md`](REGISTER-wallpaper-wire.md) 与
> [`../../architecture/wallpaper-mod-v0.md`](../../architecture/wallpaper-mod-v0.md) §5**。
> 本文件仅作历史留档。

> 本文件**不是**注册动作本身，而是交给集成 PR 的**待办清单 + 接线说明**。
> Wave 1 worker 按 [`PARALLEL-PROTOCOL-2026-09-14.md`](PARALLEL-PROTOCOL-2026-09-14.md) §3
> **被禁止**碰 FACTORIES / `mod_count_*` / 缺省 manifest / 全局版本——这些全部留在这里。
>
> 分支 `mod/wallpaper` @ `2d492447`。范围真源 [`PLAN-wallpaper.md`](PLAN-wallpaper.md)，
> 设计见 [`../../architecture/wallpaper-mod-v0.md`](../../architecture/wallpaper-mod-v0.md)。

## 1. 本分支已做完的（worker 侧）

| 项 | 位置 | 说明 |
| --- | --- | --- |
| 新 crate | `crates/live2d-ai-mod-wallpaper/` | `lib.rs`（Mod 集成）+ `strategy.rs`（纯策略） |
| workspace member | 根 `Cargo.toml` `members` | 追加 `"crates/live2d-ai-mod-wallpaper"` 一行（协议允许） |
| desktop path 依赖 | `crates/live2d-ai-desktop/Cargo.toml` | 追加 `live2d-ai-mod-wallpaper = { path = ... }`，**未**注册 |
| 静态 schema | `wallpaper_settings_spec()` | `mode`（off/follow_stage/interval）+ `interval_secs`，无第二个 `enabled` |
| 纯策略 + 单测 | `strategy.rs` | 状态机 + 15 条纯逻辑测试；runtime 侧 7 条，共 **22 passed** |
| 架构文档 | `docs/architecture/wallpaper-mod-v0.md` | 模式/决策/落点/缺口 |
| 本文件 | `docs/plans/parallel-mods/REGISTER-wallpaper.md` | 集成清单 |

## 2. 集成 PR 待办（按 `mod-product-chain.md` §3 勾选表）

- [ ] `crates/live2d-ai-desktop/src/main.rs` 的 `AVAILABLE_MOD_FACTORIES` 追加
      `&live2d_ai_mod_wallpaper::FACTORY`（放在 `persona` 之后，保持可读顺序）
- [ ] 数量断言 `mod_count_is_three`（`main.rs:436`）→ **同步改名与数字**
      （Wave 1 若同时合入 `voice-input`，则 3 → 5，并改名 `mod_count_is_five`；
      只合 wallpaper 时 3 → 4）
- [ ] `mod_factory_ids_match_expected`（`main.rs:445`）的 `expected` 追加 `"wallpaper"`
      （同时合入 voice-input 则再追加 `"voice-input"`）
- [ ] **不改缺省 manifest**：wallpaper **缺省停用**（`mode` 缺省 `off`），
      不进 `cli_entry::default_mods_manifest`；也不进 `AGENTS.md` 的「缺省只启用」句
- [ ] `docs/architecture/mod-product-chain.md` §5 表加一行（`wallpaper` / off / Rust / 一句说明）
- [ ] `AGENTS.md` 的 Mod 段（注册数 3 → 4/5）与「增强能力」句同步
- [ ] 不得改全局版本号（协议 §3）——版本由发布方统一 bump

与 [`REGISTER-voice-input.md`](REGISTER-voice-input.md) 的**冲突点只有一个**：
`mod_count_*` 的数字与 `expected` 数组。两个都合就一次改到位，**不要**两次各自改又互相覆盖。

## 3. 唯一需要做决定的集成点：决策往哪落

crate 产出 `WallpaperDecision::{None, SyncStage, Advance{index}}`，
但 **Mod API 没有壁纸写入口**（`ModServices` 只有 say / action(休眠) / event / logger /
apply_settings / settings / config_path）。`apply_decision` 因此是**明文占位**：
`None` → `true`，其余 → warn + `false`（见 `lib.rs` 该函数 doc 与架构文档 §5）。

集成方**必须复用既有通道，禁止新增 wasm / framebuffer 路径**：

| 决策 | 落点 |
| --- | --- |
| `SyncStage` | `DisplayPrefs.copyWith(syncShellStageBg: true)`（`effectiveShellImage` 随即 = `stageImage`） |
| `Advance { index }` | 播放列表第 `index` 张 → `prefs.copyWith(stageImage: dataUrl)` → 既有 `sendStageBg` 下发 |

节拍也要集成方给（Mod 事件主题里没有时钟）：主循环 timer，
或在 `VoiceEnded` / `TurnStarted` 这类已有事件上做粗粒度节拍。
**不要**在 Mod 里起线程、不要往 `l2d-wasm-demo` 加计时器。

## 4. 红线复述

- **不改** `crates/l2d-wasm-demo/`（含 `stage_bg.rs`）/ framebuffer / `LoadOp` 预通道；
- **不做**大轮播 / 图库 / 在线拉图 / 轮播 UI；
- **不唤醒** `action_tx`（自 rc.2 休眠）；
- **不注册** `local-llm`（0.2.0-rc.1 废除，禁止挂回）。

## 5. 验收证据（本分支实测）

```text
cargo test -p live2d-ai-mod-wallpaper        # 22 passed; 0 failed
cargo fmt -p live2d-ai-mod-wallpaper -- --check   # clean
cargo clippy -p live2d-ai-mod-wallpaper --all-targets -- -D warnings  # 0 warning
```

集成后应再跑全量门禁（`cargo test --workspace --all-targets` / `--doc` / fmt / clippy /
`xtask rust-ratio`），并确认 `mod_count_*` 与 `mod_factory_ids_match_expected` 仍绿。
