# REGISTER — `memory`（Wave 3 C 轨 → 集成收束用）

> 本文件是交给主 agent 的**交付说明 + 待办清单**。协议
> [PARALLEL-WAVE3-2026-09-14.md](PARALLEL-WAVE3-2026-09-14.md) §1 的独占清单
> （`AVAILABLE_MOD_FACTORIES` / `mod_count_*` / `mod_factory_ids_match_expected` /
> 缺省 manifest / 全局版本 / 根 README / AGENTS / docs/README）**本轨一行未碰**。
>
> 分支 `mod/w3-memory`，基座 `118bd435`（0.2.0-rc.3 + Wave 3 基座）。
> 上一版：Wave 2 C 轨（分支 `mod/memory-v0` @ `429609f2`）起草。
> 设计文档：[memory-mod-v0.md](../../architecture/memory-mod-v0.md)。

## 1. 本分支已做完的

### 1.1 Wave 3 C 轨新增（本次）

| 项 | 位置 | 说明 |
| --- | --- | --- |
| 检索质量基线 | `tests/fixtures/quality_corpus.json` + `src/quality_tests.rs` | 固定语料 + 「查询 → 期望命中」表；7 条断言，含**运行时级「词面不重叠 → `last_hits=0` → 不注入」**；≥1 组中文；同义改写的「不命中」写进 fixture note |
| 条数上限**物理淘汰** | `src/store.rs::append_capped` / `rewrite` | 写入超 `max_records` 时同目录临时文件 + `rename` 原子重写，只留最新 N 条；`AppendOutcome{kept,evicted,bad_lines}`；取舍见 §3.3 |
| 物理性单测 | `src/store.rs`（5 条）+ `src/tests.rs`（1 条更新） | 「第 N+1 条写入后**裸读文件**只剩 N 行、最旧文本不在文件里」；cap=1 恒 1 行；重写清坏行；运行时从 `memory.jsonl` 原文断言 + `state_json.evicted` |
| `state_json` 四计数键 | `src/lib.rs::state_json` | **`writes` / `hits` / `injects` / `errors` 必在**；另加 `evicted` / `last_hits`；`remembered` / `injected` 是 Wave 2 兼容别名（同值） |
| 与 persona 共存 = last-writer-wins | `src/tests.rs` 两条对称回归 + 本文档 §3.2 + 设计文档 §5.1 | persona 先写 → memory 后拼，base 保留、块存在；memory 先拼 → persona 覆盖后块消失 → memory 下一轮在新 base 上重拼 |
| 配置面拆分 | `src/config.rs` | `MemoryConfig` + `memory_settings_spec` 从 `lib.rs` 拆出，保持「源码 ≤500 行」；行为不变 |
| 共享测试替身 | `src/test_support.rs` | `FakeHost` / `RecordingRegistrar` / `temp_dir` / `runtime`，`tests.rs` 与 `quality_tests.rs` 共用 |

### 1.2 Wave 2 已做完（保留，本次未改语义）

| 项 | 位置 | 说明 |
| --- | --- | --- |
| 新 crate | `crates/live2d-ai-mod-memory/` | `lib.rs`（Mod 集成）+ `strategy.rs`（纯逻辑）+ `store.rs`（JSONL） |
| workspace member | 根 `Cargo.toml` `members` | 一行（Wave 2 加，本次未动） |
| desktop path 依赖 | `crates/live2d-ai-desktop/Cargo.toml` | 一行（Wave 2 加，本次未动） |
| 静态 schema | `memory_settings_spec()` | `store_path` / `top_k`(缺省 3) / `max_records`(条数上限) / `enabled_injection`，**无第二个 `enabled`** |
| 纯函数检索 | `strategy.rs` | 中文 bigram + ASCII 词 + 停用词 + Jaccard + top-k（平局：时间新→旧）；零向量 / embedding / 网络依赖 |
| 本地 JSONL | `store.rs` | `append` 一行 / `load` 容忍坏行（跳过 + 计数，不 panic） |
| `TurnPrompt` 钩子 | `lib.rs` | remember → 排除 self-hit 检索 top-k → `apply_settings` 写 `persona.system_prompt`（marker 幂等，**只对下一轮生效**） |
| 停用清残留 | `shutdown` | 有 marker 就剥掉写回，不留残留 |

## 2. 集成收束待办

**注册面已由 Wave 2 收束落盘（本次只读确认，未改）**：

- `crates/live2d-ai-desktop/src/main.rs::AVAILABLE_MOD_FACTORIES` 已含
  `&live2d_ai_mod_memory::FACTORY`（6 个工厂）；
- `mod_count_is_six`（含清单字符串）与 `mod_factory_ids_match_expected`
  （`expected` 含 `"memory"`）已绿；
- `cli_entry` 缺省 manifest **不含** memory（缺省停用，有断言守）；
- `AGENTS.md` / `mod-product-chain.md` / `docs/releases/**` 由主 agent 维护，本轨不改。

**本轨给主 agent 的收束动作**：只有 merge（`cargo test --workspace --all-targets` 等
全量门禁由主 agent 跑）。**不需要**改 FACTORIES / `mod_count_*` / 缺省 manifest / 版本号。
若合并后 `state_json` 有前端消费方，新增的 `writes/hits/injects/errors` 是**追加**键，
Wave 2 的 `remembered` / `injected` / `last_hits` 仍在，向后兼容。

## 3. 边界与取舍（集成方必读）

### 3.1 注入点已钉死，不得更换

订阅 `ModEventTopic::TurnPrompt`（payload = 本轮输入正文）→ `apply_settings`
写既有 `persona.system_prompt`。**只对下一轮生效**（请求体在本事件后立刻构建）。
**不要**改成「当轮生效」的表述，也不要为 memory 新增事件主题 / LLM 客户端钩子。

### 3.2 与 `persona` 的边界 = last-writer-wins（无仲裁）

> 以下文字与设计文档 [memory-mod-v0.md](../../architecture/memory-mod-v0.md) §5.1 **逐字一致**。

两者都可能写 `persona.system_prompt`，规则是 **last-writer-wins，没有仲裁**：

| 事件顺序 | 结果 |
| --- | --- |
| `persona` 后写 | 它的合成结果覆盖整个 `system_prompt`，**记忆块被冲掉** |
| memory 后写（下一轮） | 它在 persona 的合成结果之上重新拼上记忆块（base 一字不改） |
| memory 停用 | `shutdown` 检查 marker，有就剥掉写回——**不留残留** |

memory 每次注入前先用固定 marker 剥旧块，再从 `ModServices.settings` 读到的 base 重拼。
**不要**在收束时「顺手加一条优先级仲裁」——那需要单独的论证与文档（见设计文档 §5.1 / §8）。

两向对称回归（`src/tests.rs`）：

- `last_writer_wins_persona_base_survives_memory_injection`；
- `last_writer_wins_persona_overwrite_then_memory_reinjects_on_new_base`。

### 3.3 淘汰 = 条数上限物理删除（Wave 3 选定，含数据丢失取舍）

`max_records` 现在**不只是检索窗口**：写入超限时 `store::append_capped` **物理重写**
JSONL，只留最新 N 条（临时文件 + `rename` 原子替换）。**没有备份语义**——被淘汰的
记录不可恢复；默认 200 条即上限，要留长历史就把 `max_records` 调大（最大 10000）。
**没有**显式 delete API / 软删除 / 导出（v0 非目标，见设计文档 §6.1 / §8）。

选「物理淘汰」而不是「delete API」的理由：count cap 自动、确定、可测；delete API
需要一套管理面，而 v0 明确不做记忆管理 UI。取舍表见设计文档 §6.1。

### 3.4 `state_json` 四个必备计数键

`writes`（成功落盘条数）/ `hits`（累计命中条数）/ `injects`（成功注入次数）/
`errors`（失败路径：路径不可用 / 追加失败 / 读取失败 / 注入或剥离被拒；**不含**坏行）。
`remembered` / `injected` 是 Wave 2 兼容别名，与 `writes` / `injects` 恒同值。
断言在 `src/tests.rs::state_json_exposes_the_four_required_counters` 与
`errors_counter_counts_unresolved_path_and_rejected_inject`。

### 3.5 质量基线 fixtures（改动检索前先看）

`tests/fixtures/quality_corpus.json` 是**数据真源**：命中/不命中的期望写在那里，
`src/quality_tests.rs` 逐条断言。要改检索算法，**先改 fixtures 期望再改实现**；
不允许只改一边（否则基线变成事后追认）。同义改写不命中是**明文边界**，不是回归。

### 3.6 门禁命令（worker 实测）

```text
cargo test -p live2d-ai-mod-memory                              # 61 passed; 0 failed（Wave 2 为 44）
cargo fmt -p live2d-ai-mod-memory -- --check                    # clean
cargo clippy -p live2d-ai-mod-memory --all-targets -- -D warnings  # 0 warning
```

（`CARGO_TARGET_DIR=/home/skystar/Live2D-Ai-integrate/target`，共享 target，未另建。）

## 4. 红线复述（本轨遵守）

- **不改**基座文件 `topics.rs` / `factory.rs` / `mod_registry.rs` / `mods_routes.rs` / `supervisor.rs`；
- **不改** `main.rs::AVAILABLE_MOD_FACTORIES` / `mod_count_*` / `mod_factory_ids_match_expected` / 缺省 manifest / 版本号 / 根 README / AGENTS / docs/README / docs/releases；
- **不改**主链人设字段**结构**（只写既有 `persona.system_prompt`；`max_history_pairs` 未用）；
- **不改** `persona` crate（E 轨的）；
- **不引入**向量 / embedding / 语义检索 / 外部记忆服务依赖；
- **不接管** LLM 客户端，不新增对话通道；
- **不碰** Flutter（`shell/flutter/**`）。
