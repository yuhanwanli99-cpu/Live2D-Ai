# REGISTER — `memory`（Wave 2 C 轨 → 集成收束用）

> 本文件**不是**注册动作本身，而是交给主 agent 的**待办清单 + 接线说明**。
> Wave 2 worker 按 [`PARALLEL-WAVE2-2026-09-14.md`](PARALLEL-WAVE2-2026-09-14.md) §1
> **被禁止**碰 `AVAILABLE_MOD_FACTORIES` / `mod_count_*` / `mod_factory_ids_match_expected` /
> 缺省 manifest / 全局版本——这些全部留在这里。
>
> 分支 `mod/memory-v0`，基座 `429609f2`（Wave 2 基座：`TurnPrompt` 主题 +
> `ModRuntime::state_json` + `GET /api/v1/mods/{id}/state`）。
> 设计见 [`../../architecture/memory-mod-v0.md`](../../architecture/memory-mod-v0.md)。

## 1. 本分支已做完的（worker 侧）

| 项 | 位置 | 说明 |
| --- | --- | --- |
| 新 crate | `crates/live2d-ai-mod-memory/` | `lib.rs`（Mod 集成）+ `strategy.rs`（纯逻辑）+ `store.rs`（JSONL） |
| workspace member | 根 `Cargo.toml` `members` | 追加 `"crates/live2d-ai-mod-memory"` 一行（协议允许） |
| desktop path 依赖 | `crates/live2d-ai-desktop/Cargo.toml` | 追加 `live2d-ai-mod-memory = { path = ... }`，**未**注册进 FACTORIES |
| 静态 schema | `memory_settings_spec()` | `store_path` / `top_k`(缺省 3) / `max_records` / `enabled_injection`，**无第二个 `enabled`** |
| 纯函数检索 | `strategy.rs` | 中文 bigram + ASCII 词 + 停用词 + Jaccard + top-k（平局：时间新→旧）；零向量 / embedding / 网络依赖 |
| 本地 JSONL | `store.rs` | `append` 一行 / `load` 容忍坏行（跳过 + 计数，不 panic）/ `max_records` = 检索窗口 |
| `TurnPrompt` 钩子 | `lib.rs` | remember → 排除 self-hit 检索 top-k → `apply_settings` 写 `persona.system_prompt`（marker 幂等，**只对下一轮生效**） |
| 停用清残留 | `shutdown` | 有 marker 就剥掉写回，不留残留 |
| 运行态快照 | `state_json` | 纯内存计数（`remembered` / `injected` / `last_hits` / `turns_seen`），无磁盘 IO |
| 单测 | 4 个测试模块 | `cargo test -p live2d-ai-mod-memory` → **44 passed / 0 failed** |
| 架构文档 | `docs/architecture/memory-mod-v0.md` | 设计 / 边界 / 非目标 / 时序图 / 已知缺口 |
| 本文件 | `docs/plans/parallel-mods/REGISTER-memory-v0.md` | 集成清单 |

## 2. 集成收束待办（按 `mod-product-chain.md` §3 勾选表）

- [ ] `crates/live2d-ai-desktop/src/main.rs` 的 `AVAILABLE_MOD_FACTORIES` 追加
      `&live2d_ai_mod_memory::FACTORY`（Wave 2 五轨合并后的位置，保持可读顺序与注释）
- [ ] 数量断言 `mod_count_is_five`（`main.rs:444`）→ **改名 `mod_count_is_six` + 数字 5 → 6**
      （字符串里的清单也加上 `memory`）
- [ ] `mod_factory_ids_match_expected`（`main.rs:453`）的 `expected` 追加 `"memory"`
      ——**如果同期合入 D 轨 director-rfc**：D 轨按设计**不注册**，`expected` 仍只加 `memory`
- [ ] **不改缺省 manifest**：memory **缺省停用**，不进
      `cli_entry::default_mods_manifest`；也不进 `AGENTS.md` 的「缺省只启用」句
- [ ] `docs/architecture/mod-product-chain.md` §5 表加一行（`memory` / off / Rust /
      一句说明 + 指向 [`memory-mod-v0.md`](../architecture/memory-mod-v0.md)）
- [ ] `AGENTS.md` 的 Mod 段（注册数 5 → 6）与「增强能力」句同步
- [ ] **不改全局版本号**（协议 §3）——版本由发布方统一 bump 到 `0.2.0-rc.3`
- [ ] 收束后跑全量门禁（`cargo test --workspace --all-targets` / `--doc` / fmt /
      clippy / `xtask rust-ratio`），并确认 `mod_count_is_six` 与
      `mod_factory_ids_match_expected` 仍绿

与其它轨的**唯一冲突点**是 `mod_count_*` 的数字与 `expected` 数组：Wave 2 五轨
一次改到位，**不要**分批各改一次又互相覆盖。

## 3. 边界与取舍（集成方必读）

### 3.1 注入点已钉死，不得更换

订阅 `ModEventTopic::TurnPrompt`（payload = 本轮输入正文）→ `apply_settings`
写既有 `persona.system_prompt`。**只对下一轮生效**（请求体在本事件后立刻构建）。
**不要**改成「当轮生效」的表述，也不要为 memory 新增事件主题 / LLM 客户端钩子。

### 3.2 与 `persona` 的边界 = last-writer-wins（无仲裁）

两者都可能写 `persona.system_prompt`：memory 每次注入前先用固定 marker 剥旧块，
再从 `ModServices.settings` 读到的 base 重拼。谁后写谁赢；persona 后写会冲掉记忆块，
memory 下一轮会重新拼上。**不要**在收束时「顺手加一条优先级仲裁」——那需要单独的
论证与文档（见 `memory-mod-v0.md` §5.1 / §8）。

### 3.3 门禁命令（worker 实测）

```text
cargo test -p live2d-ai-mod-memory                              # 44 passed; 0 failed
cargo fmt --all -- --check                                      # clean
cargo clippy -p live2d-ai-mod-memory --all-targets -- -D warnings  # 0 warning
```

`cargo test --workspace --all-targets` 的实跑结果见最终汇报（基座文件未改，
desktop 只因新增 path 依赖参与编译）。

## 4. 红线复述（本分支遵守）

- **不改**基座文件 `topics.rs` / `factory.rs` / `mod_registry.rs` / `mods_routes.rs` / `supervisor.rs`；
- **不改** `main.rs::AVAILABLE_MOD_FACTORIES` / `mod_count_*` / `mod_factory_ids_match_expected` / 缺省 manifest / 版本号；
- **不改**主链人设字段**结构**（只写既有 `persona.system_prompt`；`max_history_pairs` 本轮未用）；
- **不引入**向量 / embedding / 语义检索 / 外部记忆服务依赖；
- **不接管** LLM 客户端，不新增对话通道；
- **不碰** Flutter（`shell/flutter/**`）。
