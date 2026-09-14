# 本地记忆 Mod v0（`live2d-ai-mod-memory`）

> **状态**：2026-09-14 Wave 2 C 轨（分支 `mod/memory-v0` @ 基座 `429609f2`）起草。
> 本分支**不注册** `AVAILABLE_MOD_FACTORIES`——注册（5 → 6，缺省停用）由主 agent
> 按 [`../plans/parallel-mods/REGISTER-memory-v0.md`](../plans/parallel-mods/REGISTER-memory-v0.md)
> 收束时一次改到位。
> 上层协议：[`../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md`](../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md) §3C；
> Mod 通用契约：[`mod-product-chain.md`](mod-product-chain.md)。

## 1. 定位

本 Mod 只做一件事：**把用户说过的话记在本机，并在下一轮把最相关的几条放回
system_prompt**。它不接管 LLM 客户端、不新增对话通道、不做摘要/改写——
只是「记忆的存取」。

三个刻意选择：

| 选择 | 理由 |
| --- | --- |
| 检索 = 纯函数词元重叠（中文 bigram + ASCII 词） | 无向量 / embedding / 网络依赖；可脱机单测；行为可解释（为什么这条被检索到，能指着词元说） |
| 存储 = 本地 JSONL，追加写、逐行读 | 记忆只增不改，JSONL 与它天然匹配；坏行局部化；不引数据库依赖 |
| 注入 = 经 `ModServices.apply_settings` 写既有 `persona.system_prompt` | 主链只有一个 system 入口；Mod 不另开通道（与 persona 同一条纪律） |

## 2. 时序（**只对下一轮生效**，不是缺陷）

`TurnPrompt` 在 supervisor 提交 turn 时、**本轮请求体构建之前**发出
（`crates/live2d-ai-desktop/src/supervisor.rs` 的提交点，紧随 `TurnStarted`）。
因此本 Mod 在这里做的 `apply_settings` 写回**只对下一轮生效**：

```text
用户输入 ──► supervisor：TurnStarted（turn id）
             └─► supervisor：TurnPrompt（本轮正文）        ◄── memory 在这里干活
                  ├─ 1. remember(正文, now, turn)            → 追加 memory.jsonl 一行
                  ├─ 2. load(窗口 max_records) + rank_top_k  → 排除刚写的那条（self-hit）
                  └─ 3. 命中非空 → apply_settings            → 写 live2d-ai.toml + reload
             └─► 构建本轮请求体（用的仍是**旧** system_prompt）
下一轮 ────► 构建请求体（这时才带上上一轮注入的块）
```

**不得**在任何文档或注释里写「当轮生效」——本轮请求体已经带着旧
system_prompt 上路了，那是假承诺。**先检索、再注入下一轮**正是这个时序的
预期用法（`topics.rs::TurnPrompt` 头注）。

## 3. 配置契约（`settings_spec` v1）

`MemoryFactory::settings_spec()` 是**静态**的（未启用也拿得到，rc.4 M2 语义），
`start` 注册同一份。

| key | kind | 语义 | 缺省 |
| --- | --- | --- | --- |
| `store_path` | string | JSONL 路径；空 = 配置文件同目录的 `memory.jsonl` | `""` |
| `top_k` | number | 每轮注入几条，钳在 `1..=10` | `3` |
| `max_records` | number | **检索窗口**条数，钳在 `1..=10000` | `200` |
| `enabled_injection` | bool | 是否注入（false = 只记不注入） | `true` |

**没有第二个 `enabled`**：Mod 启停的唯一真源是 manifest 的 `enabled`
（与 `external-input` / `persona` 同口径，见 [`mod-product-chain.md`](mod-product-chain.md) §4）。
`enabled_injection` 是「要不要注入」，不改变「记忆照记」。

**宽容口径**：越界只钳（`0` → 下限，不是关掉）；类型不对 / 缺失 → 用缺省；
配置坏**从不**让 Mod 失败。

**路径解析**（`strategy::resolve_store_path`，单测钉死）：

1. `store_path` 非空 → 用它；相对路径相对**配置文件所在目录**解析
   （避免「cwd 不同 → 记忆写去别处」），绝对路径原样；
2. `store_path` 空 → 配置文件同目录的 `memory.jsonl`（`ModServices.config_path`）；
3. 两者都不可得（无 host 注入的测试环境）→ `None` → **no-op + warn**，不 panic、不猜 cwd。

## 4. 检索口径（v0，明文钉死）

实现：`crates/live2d-ai-mod-memory/src/strategy.rs`（纯逻辑，不碰 IO）。

- **中文按字 bigram**：连续汉字 `c0 c1 … cn` → `c0c1, c1c2, …`；单字成段 →
  该字本身（否则一个字的查询什么都检索不到）；
- **ASCII 按词**：`[A-Za-z0-9_-]+` 小写化；长度 < 2 或命中英文停用词 → 丢弃；
- **去停用词**：ASCII 整词；中文停用词**只在单字成段时**丢弃（bigram 一律保留，
  否则含常用字的实词会被误杀）；
- **打分**：查询词元集与记录词元集的 **Jaccard** `|∩| / |∪|`，恒在 `[0, 1]`；
  重复词元不抬高分数（按集合去重）；
- **top-k**：分数为 `0` 的记录**不进结果**（「没有相关记忆」与「有一条毫不相关的
  记忆」必须区分：前者 no-op）；`k = 0` → 空；`k > 命中数` → 全部；
- **平局**（选定策略）：`score` 降序 → **`ts` 新→旧** → `turn` 大→小 →
  `index` 大→小（后两者是同一批数据的稳定兜底）。即**时间新的优先**。
- **self-hit 排除**：本轮刚 `remember` 的那条不参与本轮检索——否则它必然以
  `1.0` 自命中，白白占掉一个 k 的名额。

## 5. 注入形态与 `persona` 的边界（last-writer-wins）

marker 是固定常量（`MEMORY_MARKER_BEGIN` / `MEMORY_MARKER_END`，HTML 注释形态）：

```text
<base>

<!-- live2d-ai:memory-v0:begin -->
- 记忆一
- 记忆二
<!-- live2d-ai:memory-v0:end -->
```

`strategy::compose_injection`：**先剥旧块，再从 base 重拼**（幂等）。

- 有一条记忆是单行、按 `MAX_MEMORY_LINE_CHARS = 200` 截断（`…` 收尾）、
  空白折叠、**相同文本只出现一次**；
- `memories` 全空/空白 → 返回**不带 marker** 的 base（不做空块）；
- 重拼结果与当前提示词逐字相同 → `injection_patch` 返回 `None`：**不写盘**。

### 5.1 边界：`persona` 与 memory 同时写 `system_prompt`

两者都可能写 `persona.system_prompt`，规则是 **last-writer-wins，没有仲裁**：

| 事件顺序 | 结果 |
| --- | --- |
| `persona` 后写 | 它的合成结果覆盖整个 `system_prompt`，**记忆块被冲掉** |
| memory 后写（下一轮） | 它在 persona 的合成结果之上重新拼上记忆块 |
| memory 停用 | `shutdown` 检查 marker，有就剥掉写回——**不留残留** |

这是**已知取舍**，不是 bug：主链只有一个 system 入口，加一套优先级表就是
在核心里埋第二个产品。要「两个都生效」必须先论证仲裁规则（见 §8 非目标）。

## 6. 本地存储（JSONL）

实现：`crates/live2d-ai-mod-memory/src/store.rs`。一行一条：

```json
{"text":"今天天气很好","ts":1789000000,"turn":12}
```

- `append` 追加一行（父目录缺失则创建）；`load` 逐行读；
- **坏行纪律**：空行跳过**不计数**；非 JSON / 缺 `text` / `text` 空白 → 跳过并计入
  `LoadOutcome::bad_lines`（上层写 warn 日志——「跳过了几条」必须可见）；
  文件不存在 → 空结果（首次启用本来就没有文件）；其它 IO 错误 → 由上层决定；
- `max_records` 是**检索窗口**：`load` 只保留最新 N 条，**不物理删除历史**
  （compaction 属 v1，且必须先有备份语义）；
- `ts` / `turn` 缺失按 `0`（旧文件向前兼容）。

## 7. 失败纪律（与 persona 刻意不同）

persona 是「接管主链人设」，坏配置 → `Failed`；memory 是**增量**能力：

| 情形 | 结局 |
| --- | --- |
| 存储路径不可解析 / 追加失败 / 读取失败 | warn + 本轮 no-op；Mod 仍 `Running` |
| `apply_settings` 被拒（配置不可写 / busy） | warn + 本轮 no-op；不把 turn 打挂 |
| 坏行 | 跳过 + 计数（见 §6） |
| 空正文 | 不记不注入（warn/info 一行） |

理由：记忆写不进去不该打断一轮对话。**不 panic、不静默**——每条失败路径都有一行
可搜的日志。

## 8. 非目标（v0，明文）

1. **不做物理 compaction**：`max_records` 只约束检索窗口，JSONL 只增不删；
2. 不做向量 / embedding / 语义检索 / 外部记忆服务 / 记忆云；
3. 不接管 LLM 客户端，不新增对话通道，不写 `persona` 之外的键
   （`max_history_pairs` 本轮**未用**，留给 v1）；
4. 不做跨会话去重 / 冲突消解 / 摘要（同一句话说两次就是两条记录）；
5. 不做 `persona` / memory 的仲裁优先级（§5.1）；
6. 不做记忆管理 UI（查看 / 编辑 / 删除）——API 面只有基座通用的
   `GET /api/v1/mods/memory/state`（只读计数，见 §9）。

## 9. 运行态快照（`state_json`）

实现基座 Wave 2 的 `ModRuntime::state_json`，**纯内存计数、不做磁盘 IO**
（它跑在 web_api 线程上）：

```json
{"store_path":"…/memory.jsonl","top_k":3,"max_records":200,
 "enabled_injection":true,"turns_seen":12,"remembered":12,"injected":4,"last_hits":2}
```

路由由基座提供：`GET /api/v1/mods/memory/state`（404 = 不在注册表；
503 `state_unavailable` = 在册但未启用 / worker 正持锁）。

## 10. 门禁与测试

- `cargo test -p live2d-ai-mod-memory` → **44 passed / 0 failed**；
- `cargo fmt --all -- --check` clean；
- `cargo clippy -p live2d-ai-mod-memory --all-targets -- -D warnings` → 0 warning。

覆盖面：分词（bigram / ASCII / 停用词 / 单字 / 空与纯标点）、打分（Jaccard 边界 /
重复词元 / 空侧）、top-k（排序 / 零分排除 / k 钳位 / 四种平局键）、marker
（拼装 / 幂等 / 替换旧块 / 半截块 / 去重 / 压平截断）、JSONL（往返 / 缺文件 /
坏行计数 / 窗口 / 建父目录 / 行形状）、路径解析三态、配置缺省与钳位、
生命周期（schema / 订阅 / 非 `TurnPrompt` 忽略）、一轮的记→检索→注入、
self-hit 排除、幂等不重写、注入关闭仍记、空正文、路径缺失、`apply_settings`
被拒、停用清残留、`state_json` 形状。

涉及文件：

- `crates/live2d-ai-mod-memory/src/lib.rs`（Mod 集成：descriptor / schema / config / runtime / factory）
- `crates/live2d-ai-mod-memory/src/strategy.rs`（纯逻辑：分词 / 打分 / top-k / marker / 路径）
- `crates/live2d-ai-mod-memory/src/store.rs`（JSONL 读写）
- `crates/live2d-ai-mod-memory/src/strategy_tests.rs`、`src/tests.rs`（单测）
- 根 `Cargo.toml`（members 一行）、`crates/live2d-ai-desktop/Cargo.toml`（path 依赖一行）

## 11. 已知缺口 / 下一步

1. **注册未做**（本分支刻意不做）：`FACTORIES` 5 → 6、`mod_count_is_*`、
   `mod_factory_ids_match_expected`、`mod-product-chain.md` §5、`AGENTS.md`——
   步骤见 [`REGISTER-memory-v0.md`](../plans/parallel-mods/REGISTER-memory-v0.md)；
2. **每轮一次全量读盘**：`load` 读整个 JSONL 再排序。`max_records ≤ 10000`
   时是可接受的 v0 口径；若未来窗口放大，先加「读文件尾部 N 行」或索引；
3. **检索质量未做基线评测**：Jaccard 无 idf，长记录会被稀释。要改先加
   一组固定的「查询 → 期望命中」样例做对照，别凭手感调；
4. **无记忆管理面**：删除 / 编辑 / 导出只有手工改 JSONL；
5. **与 persona 的边界只有 last-writer-wins**（§5.1），无仲裁、无合并。
