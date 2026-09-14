# REGISTER — `director`（Wave 3 G 轨 → 集成收束用）

> 本文件**不是**注册动作本身，而是交给**主 agent** 的**待办清单 + 接线说明**。
> Wave 3 worker 按 [PARALLEL-WAVE3-2026-09-14.md](PARALLEL-WAVE3-2026-09-14.md) §1/§3G
> **被禁止**碰 `main.rs::AVAILABLE_MOD_FACTORIES` / `mod_count_*` /
> `mod_factory_ids_match_expected` / `cli_entry::default_mods_manifest` / 全局版本号 /
> `docs/README.md` / `AGENTS.md` / `mod-product-chain.md` 的 Mod 清单表——
> 这些全部留在这里。
>
> 分支 `mod/w3-director`，基线 `118bd435`（Wave 3 基座：`ModEventTopic::TurnEnded`）。
> 实现契约：[../../architecture/director-mod-v0.md](../../architecture/director-mod-v0.md)；
> 上位契约：[../../architecture/director-rfc.md](../../architecture/director-rfc.md)。

## 1. 本分支已做完的（worker 侧）

| 项 | 位置 | 说明 |
| --- | --- | --- |
| 新 crate | `crates/live2d-ai-mod-director/` | `lib.rs`（Mod 集成）/ `decision.rs`（纯函数）/ `ledger.rs`（账本）/ `tests.rs`（回归） |
| workspace member | 根 `Cargo.toml` `members` | 追加 `"crates/live2d-ai-mod-director"` 一行（协议唯一允许的根 Cargo.toml 改动；**未**碰 version） |
| 描述符 | `DESCRIPTOR` | `id = "director"`、`api_version = MOD_API_VERSION` |
| 静态 schema | `director_settings_spec()` | `log_capacity`（Number，1..=200，缺省 20）/ `emotion_lexicon`（Select `builtin`/`strict`）；**无第二个 `enabled`** |
| 订阅面 | `start` | 正好两个主题：`TurnPrompt` + `TurnEnded` |
| 纯函数决策 | `decision::derive` | 正文 → `{emotion, intent, suggested_tts:{speed,pitch}}`；确定性、无 IO / 网络 / 时钟 |
| 决策账本 | `ledger::DecisionLedger` | 最近 `log_capacity` 条 + `turns_seen/turns_ended/decisions/silent/errors` |
| 运行态快照 | `state_json` | 上述计数 + `recent_decisions` + `delivered:false` + `channel:"none"`（纯内存，无 IO） |
| **零投递** | `lib.rs` | **从不**调用 `action_tx`、**从不**调用 `apply_settings`；回归用间谍 sender 断言两个计数都是 0 |
| 单测 | `src/tests.rs` | `cargo test -p live2d-ai-mod-director` → **28 passed / 0 failed** |
| 架构文档 | `docs/architecture/director-mod-v0.md` | 骨架范围 / 状态面 / 与休眠 `action_tx` 的关系 / 晋升门槛 |
| RFC 更新 | `docs/architecture/director-rfc.md` §0/§8/§9 | 写明 Wave 3 已推进到骨架、仍不投递、注册留给收束 |
| 本文件 | `docs/plans/parallel-mods/REGISTER-director-v0.md` | 集成清单 |

## 2. 集成收束待办（按 `mod-product-chain.md` §3 勾选表）

- [ ] `crates/live2d-ai-desktop/Cargo.toml` 追加 path 依赖
      `live2d-ai-mod-director = { path = "../live2d-ai-mod-director" }`
      （**G 轨没有该文件的所有权，故未改**）
- [ ] `crates/live2d-ai-desktop/src/main.rs` 的 `AVAILABLE_MOD_FACTORIES` 追加
      `&live2d_ai_mod_director::FACTORY`（保持可读顺序与注释；
      建议注明「Wave 3：导演最小骨架，缺省停用，**不投递**」）
- [ ] 数量断言 `mod_count_is_six`（`main.rs:454`）→ **改名 `mod_count_is_seven` +
      数字 6 → 7**（字符串里的清单加上 `director`）
- [ ] `mod_factory_ids_match_expected`（`main.rs:463`）的 `expected` 追加
      `"director"`
- [ ] **不改缺省 manifest**：director **缺省停用**，不进
      `cli_entry::default_mods_manifest`；也不进 `AGENTS.md` 的「缺省只启用」句
- [ ] `docs/architecture/mod-product-chain.md` §5 表加一行
      （`director` / off / Rust / 一句说明 + 指向
      [director-mod-v0.md](../architecture/director-mod-v0.md)）
- [ ] `AGENTS.md` 的 Mod 段（注册数 6 → 7）与「增强能力」句同步；补一句
      「director 是**只记日志、不投递**的骨架」
- [ ] **不改全局版本号**（Wave 3 协议 §0）——版本由发布方统一处理
- [ ] 收束后跑全量门禁（`cargo test --workspace --all-targets` / `--doc` / fmt /
      clippy / `xtask rust-ratio`），并确认 `mod_count_is_seven` 与
      `mod_factory_ids_match_expected` 仍绿
- [ ] 注册后补一次**活服务**验收：`GET /api/v1/mods/director/state` = 200（启用后）
      / 503（未启用）；`GET /api/v1/mods` 里 director 的 `settings_spec` 与本文档 §1 一致
- [ ] `docs/README.md` 第 33 行的 director 条目仍写着「契约先行，未注册（0.2.0-rc.3，
      只交文档）」——收束时改成「已推进到最小骨架（缺省停用、不投递），收束注册为第 7 个」
      并补一条指向 [director-mod-v0.md](../architecture/director-mod-v0.md) 的链接
- [ ] 可选但推荐：`docs/architecture/core-chain-baseline.md` §3.2 的休眠台账里
      「director」一行若提到「已删除 / 未注册」，收束时补一句**当前状态**——
      **注意**：该文件属基座独占清单，只能由主 agent 改

与其它轨的**唯一冲突点**是 `main.rs` 的 `mod_count_*` / `expected` 数组与
`desktop/Cargo.toml`：Wave 3 七轨一次改到位，**不要**分批各改一次又互相覆盖。

## 3. 边界与取舍（集成方必读）

### 3.1 骨架 = 「零投递」，不是「投递了但被拒」

`director` **从不调用** `ModServices::action_tx`，也**从不调用**
`ModServices::apply_settings`。因此：

- 它**不改** `live2d-ai.toml`、**不写** `persona.system_prompt`、
  **不碰** `[tts]` / `[llm]` / 壁纸偏好；
- `suggested_tts` 目前**没有消费者**（只进日志 + `state_json`）；
- 回归 `tests::action_tx_and_apply_settings_are_never_called` 用间谍 sender
  断言两个调用计数都是 `0`。**收束时不要**为了「让骨架有用」而给它接上通道——
  那要过 `director-rfc.md` §5.1 第 0/1 条（授权 + 用户可见演示）。

### 3.2 为什么缺省停用而不是启用

它当前**不改变任何产品行为**（零投递），启用后唯一可见差异是多一份运行态快照。
缺省停用是刻意的：避免「GET /api/v1/mods 里有一条启用了却什么都不会发生的条目」，
也避免稀释计数断言的护栏语义（`director-rfc.md` §8）。

### 3.3 与 memory 的节拍同源、输出面不重叠

两者都挂在 `TurnPrompt`，但 memory 动「说什么」（`system_prompt`）、
director 动「怎么说」（`voice` 的**建议**）。两者**不共用**通道、互不等待。
**不要**把 director 的 issue 记到 memory 的字段上。

### 3.4 门禁命令（worker 实测）

```text
cargo test -p live2d-ai-mod-director                                  # 28 passed; 0 failed
cargo fmt -p live2d-ai-mod-director -- --check                        # clean
cargo clippy -p live2d-ai-mod-director --all-targets -- -D warnings   # 0 warning
```

## 4. 红线复述（本分支遵守）

- **不改**基座文件 `topics.rs` / `factory.rs` / `mod_registry.rs` / `mods_routes.rs` /
  `supervisor.rs`；
- **不改** `main.rs::AVAILABLE_MOD_FACTORIES` / `mod_count_*` /
  `mod_factory_ids_match_expected` / 缺省 manifest / 版本号 / `pubspec.yaml` /
  `AGENTS.md` / `docs/README.md` / `docs/releases/**` / `README*` /
  `mod-product-chain.md` 的 Mod 清单表；
- **不复活** Action 真通道：`action_tx` 保持休眠，core `action/` / `performance/` 不碰；
- **不实现**动作库 / 编舞 / WS 帧 / wasm 协议；
- **不投递**任何动作或 TTS 参数；
- **不碰** Flutter（`shell/flutter/**`）与其它轨的 crate / 文档。

## 5. 维护者（RFC §5.1 第 8 条：必须点名）

- **owner**：Wave 3 G 轨执行 agent（`mod/w3-director` 分支作者）；
  收束后由**主 agent** 作为集成方接手注册与后续演进。
- **评审入口**：任何「让 director 真的投递」的改动必须先改
  [director-rfc.md](../../architecture/director-rfc.md) §5.1 / §5.2 并拿到授权记录，
  再动代码。
