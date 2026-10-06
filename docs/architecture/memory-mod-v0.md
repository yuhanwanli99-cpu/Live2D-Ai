# 本地记忆 Mod v0（`live2d-ai-mod-memory`）

> **状态**：2026-09-15 **L1 产品级（C 轨：memory 被动 + 主动 + 会话分桶）**——
> 每条记忆有稳定 **`id`**（JSONL 落盘；旧行按同一公式派生）；`ModRuntime::command`
> 新增 **`list` / `import` / `update` / `delete`**（主动管理面，§13）；被动路径与
> 注入**按会话分桶**（`sessions/<id>.memory.jsonl` + 会话级 `system_prompt` 覆盖，
> §14）；`state_json` 新增 `session_scoped` / `active_session` / `bucket_path`；
> 面板新增记忆列表（查看 / 编辑 / 删除）+ 导入一条 + 会话降级文案。
> **版本仍 `0.2.0-rc.3`**（不 bump / 不 tag）。`../legacy/plans/PRODUCT-L1-GOALS-2026-09-15.md`（已移出工作树，取回见 [文档索引](../REMOVED-docs-index-2026-10-06.md)）
> §3（被动 + 主动）与 §4（验收句式）是本轮范围真源。
>
> **2026-09-20（P1-5 补齐：真摘要，本波）**：`summary_*` 那组钩子**真的接了 LLM**
> ——桶内原文总字符越过「注入预算 × `summary_ratio`（缺省 0.75）」＋过冷却时，
> **后台线程**跑一次 OpenAI 兼容摘要（主链本轮不等它），产出落**旁车文件**
> （`<桶>.summary.json`，带版本、可回滚）；摘要作为「摘要」段进**本 conversation**
> 的注入块；失败 ≡ 无摘要（旁车一字不改，只记 `last_error`；规则裁 top-k 照常）。
> 新增命令 `summary`（只读状态）与 `summary_rollback`（丢当前版、原文一条不动）。
> `settings_spec` v2 追加 `summary_enabled` / `summary_base_url` / `summary_model` /
> `summary_api_key_env` / `summary_timeout_ms`（**独立于主链 `[llm]`**）。
> 详见 **§15**。
>
> **2026-09-21（Wave 3：摘要含助手侧，本波）**：Mod 事件新增
> **`AssistantReplied`**（`assistant_replied`，payload = JSON
> `{turn,role,text,interrupted}`）：supervisor 在轮末把**清洗后**的助手正文
> （与上屏 / 送 TTS 同源）**非阻塞**投递给订阅方；memory 据此把助手侧按
> conversation 分桶追加（`role=assistant`，与同轮用户记录**共用同一个
> turn**）。摘要的待压窗口因此**用户 + 助手交错**，注入行与摘要输入都带角色前缀
> （`[用户]` / `[助手]`），「保留最近 K 轮」改为按 `turn` 分组计数
> （4 条记录不再等于 4 轮）。无 conversation 时助手侧与用户侧一样：只落老桶、
> 不注入、不摘要。`state_json.writes` 拆出 `user_writes` /
> `assistant_writes`。详见 **§16**。
>
> **2026-09-16（P1-5，上一波）**：**删掉全局降级**——无 conversation 时记忆仍落老桶
> `memory.jsonl`，但**不再写全局 `persona.system_prompt`**（那是「两份真相」
> 的来源、会污染所有会话）；日志与面板如实说「无 conversation，本轮不注入」。新增
> 注入**字符预算**（`injection_budget_chars`，缺省 1600）：占用 >60% 按分数裁
> top-k；>75% 进入摘要判定（当时的「待摘要」钩子；**真摘要在 2026-09-20 补齐**）。
> `state_json` 新增 `conversation_id` / `injection_budget_chars` /
> `budget_*` / `summary_*` / `no_conversation_turns`。
>
> **上一版**：2026-09-14 **产品级加强波次**（memory 轨，worktree
> `Live2D-Ai-pg-memory` @ 基座 `72d1af17`）更新——`state_json` 新增
> **`records`（库里当前实际条数）**、实现 **`command("clear")`**（原子重写 JSONL 为空）
> + 产品面板（条数 / 命中 / 注入 / 淘汰 / 上轮命中 / 清空按钮 / 注入开关与 persona
> 策略说明）+ §12 逐条可复制验收步骤。**版本仍 `0.2.0-rc.3`**（不 bump / 不 tag）。
> 上一版：2026-09-14 **Wave 3 C 轨**（分支 `mod/w3-memory` @ 基座 `118bd435`）——
> 补检索**质量基线 fixtures**、**条数上限物理淘汰**、`state_json` **四个计数键**，
> 并把与 persona 的 **last-writer-wins**（**全局路径**）用两向对称测试钉死；
> 会话路径改为**来源槽叠加**（L1，2026-09-15，见 §5 的提示框与 §14）。
> 再上一版：Wave 2 C 轨（分支 `mod/memory-v0` @ `429609f2`）起草。
> **注册已完成**（Wave 2 收束落盘）：memory 是 `AVAILABLE_MOD_FACTORIES` 的 6 个工厂之一，
> **缺省停用**（`cli_entry` 缺省 manifest 不含 memory）；本轨**不改注册面**。
> 上层协议：[PARALLEL-WAVE3-2026-09-14.md](../plans/parallel-mods/PARALLEL-WAVE3-2026-09-14.md) §3C；
> 集成说明：[REGISTER-memory-v0.md](../plans/parallel-mods/REGISTER-memory-v0.md)；
> Mod 通用契约：[mod-product-chain.md](mod-product-chain.md)。

## 1. 定位

本 Mod 只做一件事：**把这一轮说过的话（用户侧 + 助手侧）记在本机，并在下一轮
把最相关的几条放回 system_prompt**。它不接管 LLM 客户端、不新增对话通道、
不做改写；摘要是**自己的**压缩（P1-5，§15），只在**本 conversation** 的注入槽里
生效。

五个刻意选择：

| 选择 | 理由 |
| --- | --- |
| 检索 = 纯函数词元重叠（中文 bigram + ASCII 词） | 无向量 / embedding / 网络依赖；可脱机单测；行为可解释（为什么这条被检索到，能指着词元说） |
| 存储 = 本地 JSONL，追加写、逐行读 | 记忆只增不改，JSONL 与它天然匹配；坏行局部化；不引数据库依赖 |
| 注入 = 经 `ModServices.apply_settings` 写既有 `persona.system_prompt` | 主链只有一个 system 入口；Mod 不另开通道（与 persona 同一条纪律） |
| **淘汰 = 条数上限，写入超限时物理删除最旧**（Wave 3） | 文件有界、检索/注入不随历史无限变慢；数据丢失的取舍写死在 §6.1 |
| **质量基线 = 固定 fixtures + 命中/不命中断言**（Wave 3） | 质量口径可回归，不靠手感（§4.1） |

## 2. 时序（**同轮生效**，2026-09-15 起）

`TurnPrompt` 在 supervisor 提交 turn 时、**本轮请求体构建之前**发出
（`crates/live2d-ai-desktop/src/supervisor.rs` 的提交点，紧随 `TurnStarted`），
并且 host 对**这一个话题**走**同步投递**（`mod_event_sink` →
`ModRegistry::dispatch_event_and_flush`：入队后等 worker 回执）。
supervisor 收到回执**之后**才决议本轮 `system_prompt`——所以本 Mod 在这里
写进会话注入槽的内容，**本轮请求体就会带上**：

```text
用户输入 ──► supervisor：TurnStarted（turn id）
             └─► supervisor：TurnPrompt（本轮正文）        ◄── memory 在这里干活
                  ├─ 1. remember(正文, now, turn)            → 追加 memory.jsonl 一行
                  │      └─ 超过 max_records → 物理淘汰最旧（写临时文件 + rename）
                  ├─ 2. load(窗口 max_records) + rank_top_k  → 排除刚写的那条（self-hit）
                  └─ 3. 命中非空 → apply_settings            → 写 live2d-ai.toml + reload
             └─► 构建本轮请求体（用的仍是**旧** system_prompt）
             └─► supervisor 决议本轮 system_prompt（读到的就是刚写的注入）
                  └─► 构建本轮请求体（带记忆块）
```

**历史（为什么曾经是「只对下一轮生效」）**：旧实现把 `TurnPrompt` 投递排在
`set_system_prompt_override` **之后**，而 `dispatch_event` 只是 `try_send`——
Mod 写表与 supervisor 读表是两个线程，谁先跑到读表点是竞态，于是注入实际上
落到下一轮。2026-09-15 改成「**先投递（同步）→ 再决议**」，并用回执把时序钉死。
**文档、REGISTER 与注释不得再写「只对下一轮生效」**（回归：
`supervisor::tests_loop::memory_written_on_turn_prompt_lands_in_the_same_turn_request`）。

## 3. 配置契约（`settings_spec` v1）

`MemoryFactory::settings_spec()` 是**静态**的（未启用也拿得到，rc.4 M2 语义），
`start` 注册同一份。

| key | kind | 语义 | 缺省 |
| --- | --- | --- | --- |
| `store_path` | string | JSONL 路径；空 = 配置文件同目录的 `memory.jsonl` | `""` |
| `top_k` | number | 每轮注入几条，钳在 `1..=10` | `3` |
| `max_records` | number | **条数上限（count cap）**：文件里最多保留这么多条，写入超限**物理淘汰**最旧；同时也是检索窗口。钳在 `1..=10000` | `200` |
| `enabled_injection` | bool | 是否注入（false = 只记不注入） | `true` |
| `injection_budget_chars` | number | 注入块字符预算（`200..=20000`） | `1600` |
| `trim_ratio` / `summary_ratio` | number | 越过 → 裁 top-k / 触发摘要 | `0.60` / `0.75` |
| `summary_enabled` … `summary_timeout_ms` | mixed | **真摘要**的独立 LLM 配置（见 §15.4） | 关闭 / `8000` |

> `settings_spec` 当前 **v2**（P1-5 追加 `summary_*` 一组；v1 是上表前四项）。

**没有第二个 `enabled`**：Mod 启停的唯一真源是 manifest 的 `enabled`
（与 `external-input` / `persona` 同口径，见 [mod-product-chain.md](mod-product-chain.md) §4）。
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

### 4.1 质量基线（Wave 3 新增，可跑证据）

固定语料：`crates/live2d-ai-mod-memory/tests/fixtures/quality_corpus.json`
（`include_str!` 进 `src/quality_tests.rs`）。语料是**数据**，断言分两层：

| 样例 | 查询 | 期望 |
| --- | --- | --- |
| `zh-weather-shares-bigrams` | 今天天气怎么样 | 命中「今天天气很好，我们去公园散步」「今天下雨了」（按分数序） |
| `zh-movie-shares-one-bigram` | 电影票在哪里买 | 命中「周末打算去看电影」（共享 `电影` 一个 bigram） |
| `zh-disjoint-no-hit` | 帮我写一首关于星空的诗 | **0 命中** → `last_hits=0` → **不注入**（运行时级断言：零 patch、主链提示词不变） |
| `zh-paraphrase-no-lexical-overlap` | 晴朗的日子适合出门走走 | **0 命中**——同义改写无共享词元，这是 v0 的**明文边界**（不是回归） |
| `ascii-token-overlap` | remember the API2 token rotation | 命中「API2 token rotation policy is documented」（大小写不敏感） |

断言直接打 `strategy::rank_top_k` 与运行时 `state_json`，**不设相似度阈值**：
命中与否由「词元交集是否为空」决定，可逐条复算。要动检索算法，先改 fixtures
的期望，再改实现——不许只改一边。

## 5. 注入形态与 `persona` 的边界

> **2026-09-15（L1）两条路径，别混**：
> - **全局路径**（调用方不带会话 id，写 `persona.system_prompt`）：**仍是
>   last-writer-wins**，下面 §5.1 一字未改；
> - **会话路径**（带会话 id，写宿主会话表的 **memory 来源槽**）：**不再互相覆盖**。
>   宿主表按 owner 分槽（`persona` / `memory` / 匿名），读时按固定顺序
>   `匿名 → persona → memory` 用空行连接 —— 所以「persona 的卡」与「memory 的块」
>   是**叠加**关系，谁先写谁后写都一样；停用任一方只清**自己那一槽**。
>   详见 [session-scope-l1.md](session-scope-l1.md) 与 §14。

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

### 5.1 边界（**仅全局路径**）：`persona` 与 memory 同时写 `persona.system_prompt`

> 适用范围：调用方**不带** `session_id` 的全局路径（裸 `POST /api/v1/chat`、终端壳）。
> 带会话 id 时走上面的「来源槽」组合，**没有**覆盖问题。

两者都可能写 `persona.system_prompt`，规则是 **last-writer-wins，没有仲裁**：

| 事件顺序 | 结果 |
| --- | --- |
| `persona` 后写 | 它的合成结果覆盖整个 `system_prompt`，**记忆块被冲掉** |
| memory 后写（下一轮） | 它在 persona 的合成结果之上重新拼上记忆块（base 一字不改） |
| memory 停用 | `shutdown` 检查 marker，有就剥掉写回——**不留残留** |

这是**已知取舍**，不是 bug：主链只有一个 system 入口，加一套优先级表就是
在核心里埋第二个产品。要「两个都生效」必须先论证仲裁规则（见 §8 非目标）。

**可执行建议**：想稳定用人设（persona 卡）就**别同时开 memory 注入**——两者同时
写同一份 `system_prompt` 时，后写者会覆盖前写者；要记忆能力就要接受「人设可能被
记忆块叠加、且 persona 后写会冲掉记忆块」这个取舍。面板上的同一条说明就在同一个
位置（`memory_panel.dart`）。

### 5.2 清空记忆库与提示词残留（产品级加强波次）

`command("clear")` **只清 JSONL 记忆库**（§9.1），**不写** `persona.system_prompt`
——**不「顺手」清 persona 的提示词**，更不做第二套「连提示词一起清」的行为
（那正是 §5.1 禁止的第二个仲裁/合并机制）。清空前若注入过记忆块，残留按 crate
**既有 `strip_residue` 生命周期**处理，没有新路径：

| 时间点 | 残留块的下场 |
| --- | --- |
| 停用 Mod | `shutdown` 调 `strip_residue`：按 marker 剥掉块，base 一字不改 |
| 下一轮**有非空命中** | `compose_injection` 先剥旧块再拼新块（幂等） |
| 清空后**没有命中**（库里还没新记录） | 旧块**不会**被自动剥掉，会留在 `system_prompt` 里 |

`clear` 的返回带 `residue: true/false` **如实报告**「提示词里现在是否还有注入块」，
面板据此提示「停用 Mod 会按既有语义剥离」（§9.1 / §12 第 6 步）。

**这条规则同时写在**
[REGISTER-memory-v0.md](../plans/parallel-mods/REGISTER-memory-v0.md) §3.2（逐字一致），
并由 `src/tests.rs` 的两条对称回归守住：

- `last_writer_wins_persona_base_survives_memory_injection`（persona 先写 → memory 后拼，base 保留、块存在）；
- `last_writer_wins_persona_overwrite_then_memory_reinjects_on_new_base`（memory 先拼 → persona 覆盖后块消失 → memory 下一轮在新 base 上重拼）。

## 6. 本地存储（JSONL）

实现：`crates/live2d-ai-mod-memory/src/store.rs`。一行一条：

```json
{"text":"今天天气很好","ts":1789000000,"turn":12}
```

- `append` 追加一行（父目录缺失则创建）；`load` 逐行读；
- **坏行纪律**：空行跳过**不计数**；非 JSON / 缺 `text` / `text` 空白 → 跳过并计入
  `LoadOutcome::bad_lines`（上层写 warn 日志——「跳过了几条」必须可见）；
  文件不存在 → 空结果（首次启用本来就没有文件）；其它 IO 错误 → 由上层决定；
- `load(max_records)` 仍只保留最新 N 条（检索窗口，对坏行/外部手改的兜底）；
- `ts` / `turn` 缺失按 `0`（旧文件向前兼容）。

### 6.1 条数上限 = 物理淘汰（Wave 3 选定，含取舍）

`JsonlStore::append_capped(record, max_records)` 是**写入路径**（运行时只用它）：

1. 先追加一行；
2. 重新载入整个文件（`load(0)`）；
3. 有效记录 ≤ `max_records` → **文件一字不改**（返回 `AppendOutcome{evicted:0}`）；
4. 超过 → 保留**最新 N 条**，写同目录临时文件 `<store>.jsonl.tmp` + `rename`
   原子替换；返回 `evicted = 被删条数` 与 `kept = N`。

`max_records == 0` = 不淘汰（与 `load(0)` 对称；配置钳位保证运行时 ≥ 1）。

**取舍（必须写清，不许含糊）**：

| 得 | 失 |
| --- | --- |
| 文件**有界**（默认 ≤ 200 条），检索/注入不随历史无限变慢 | **没有备份语义**：被淘汰的记录不可恢复，删了就没了 |
| 保留的是**最新**的 N 条（与检索平局的「时间新→旧」同向） | 想留长历史必须显式把 `max_records` 调大（最大 10000），**没有**软删除/导出 |
| 重写顺带清掉坏行（`bad_lines` 归零） | 坏行也会在这次重写里一起消失（这通常是好事，但要知道） |
| 临时文件 + `rename`，崩溃最多留一个 `.jsonl.tmp`，正式文件不会被截半 | 每次超限写入多一次全量读 + 一次全量重写（默认 200 条，量级可忽略） |

**单测证明物理性**（不是「只把窗口调窄」）：`store.rs` 的
`append_capped_truncates_file_to_cap_keeping_newest` 断言「第 5 条写入后**裸读文件**
只剩 3 行、最旧两条文本不在文件里」；`append_capped_cap_one_keeps_only_the_last_write`
断言 cap=1 时文件恒 1 行；运行时级 `tests.rs::max_records_is_a_physical_count_cap_not_just_a_window`
从 `memory.jsonl` 原文断言同一条，并断言 `state_json.evicted` 计数。

**为什么选「物理淘汰」而不是「显式 delete API」**（协议 §3C 二选一）：
count cap 是**自动、确定、可测**的边界，日常用不需要用户手动清理；delete API 需要
一套管理 UI/端点，而 v0 明确不做记忆管理面（§8）。代价是无备份，已在上面写明。

## 7. 失败纪律（与 persona 刻意不同）

persona 是「接管主链人设」，坏配置 → `Failed`；memory 是**增量**能力：

| 情形 | 结局 |
| --- | --- |
| 存储路径不可解析 / 追加失败 / 读取失败 | warn + 本轮 no-op + `errors += 1`；Mod 仍 `Running` |
| `apply_settings` 被拒（配置不可写 / busy） | warn + 本轮 no-op + `errors += 1`；不把 turn 打挂 |
| 坏行 | 跳过 + 计数（见 §6）；**不计入 `errors`**（有自己的信号） |
| 空正文 | 不记不注入（warn/info 一行），**不计入 `errors`** |

理由：记忆写不进去不该打断一轮对话。**不 panic、不静默**——每条失败路径都有一行
可搜的日志，并且被 `state_json.errors` 汇总。

## 8. 非目标（v0，明文）

1. **没有备份 / 撤销 / 导出语义**：条数上限淘汰是物理删除（§6.1）；
2. 不做向量 / embedding / 语义检索 / 外部记忆服务 / 记忆云
   ——同义改写在词面口径下**不命中**，这是边界不是 bug（§4.1）；
3. 不接管 LLM 客户端，不新增对话通道，不写 `persona` 之外的键
   （`max_history_pairs` 本轮**未用**，留给 v1）；
4. 不做跨会话去重 / 冲突消解 / 摘要（同一句话说两次就是两条记录）；
5. 不做 `persona` / memory 的仲裁优先级（§5.1）；
6. ~~不做记忆查看 / 编辑面~~ **L1（2026-09-15）起改为做**：面板可查看 / 导入 /
   编辑 / 逐条删除 / 清空整库（§13），不依赖聊天口令「记住 / 忘掉」。
   **仍不做**：导出、备份、软删除、撤销——淘汰与删除都是物理删除（§6.1 / §13.4）。

## 9. 运行态快照（`state_json`）

实现基座 Wave 2 的 `ModRuntime::state_json`，**只读、不写盘**；其中 `records`
需要**一次本地全量读**（`JsonlStore::load(0)`，见 §9.1），其余键仍是纯内存计数
（`state_json` 在 web_api 线程上被调用，「不阻塞」指不等待网络 / 锁）：

```json
{"store_path":"…/memory.jsonl","bucket_path":"…/sessions/s-1.memory.jsonl",
 "session_scoped":true,"active_session":"s-1",
 "records":12,"top_k":3,"max_records":200,
 "enabled_injection":true,"turns_seen":12,
 "writes":12,"user_writes":7,"assistant_writes":5,
 "hits":17,"injects":4,"errors":0,"evicted":2,"last_hits":2,
 "summary":{"enabled":true,"client":"openai","version":1,"covers_upto":4,
            "bucket_ratio":0.82,"pending":false,"last_error":null,"text":"…"},
 "remembered":12,"injected":4}
```

**Wave 3 必备四键**（协议 §3C 4）：

| 键 | 语义 |
| --- | --- |
| `writes` | 成功落盘的记忆条数（每次成功 `append_capped` +1）；Wave 3 起**含助手侧**，拆分为 `user_writes` + `assistant_writes`（§16） |
| `hits` | **累计**检索到的记忆条数（跨轮累加；`last_hits` 是最近一轮） |
| `injects` | 成功注入主链提示词的次数（`apply_settings` 接受） |
| `errors` | 失败路径次数：路径不可用 / 追加失败 / 读取失败 / 注入或剥离被拒（**不含**坏行） |

附带键：`evicted`（被物理淘汰的条数）、`last_hits`（最近一轮命中数）、
`turns_seen`；`remembered` / `injected` 是 Wave 2 的**兼容别名**，由同一字段
派生（与 `writes` / `injects` 恒同值，不会漂移）。

**`records`（产品级加强波次新增）**：**当前生效桶**里的实际条数——逐行读 JSONL
数出来的有效记录数（坏行不计），**不是** `writes`（累计写入，清空也不重置）。
文件不存在 → `0`；路径不可解析 / 读取失败 → `null`（面板显示「—」）并把失败计入
`errors`——「读不到」不能谎报成「0 条」。代价是一次**全量读**（与每轮两次全量读
同一取舍，见 §11.2）；「不阻塞」指不等待网络 / 锁，本地顺序读不在其列。

**三个会话键（L1 新增，现有键一个不删）**：

| 键 | 语义 |
| --- | --- |
| `session_scoped` | 当前生效桶是不是**会话桶**（= 最近一轮 `TurnPrompt` 有没有带会话） |
| `active_session` | 最近一轮的会话 id；`null` = 裸 HTTP / 终端壳（无会话） |
| `bucket_path` | 当前生效桶的路径（会话桶或老路径）；不可解析 → `null` |

`store_path` **保持全局（老路径）含义不变**——排障时用它区分「配置指向哪」与
「现在实际读写哪个桶」（`bucket_path`）。`records` 跟随 `bucket_path`，而
`bucket_path` 跟的是**最近一轮 `TurnPrompt` 的会话**（`state_json` 不带参数）：
用户刚切换会话但还没说话时，概览可能仍是上一个会话的条数——面板的**列表**用
`ctx.activeSessionId` 显式带 `session_id`（§13.1），那一份才是权威。

路由由基座提供：`GET /api/v1/mods/memory/state`（404 = 不在注册表；
503 `state_unavailable` = 在册但未启用 / worker 正持锁）。

**`summary` 子对象（P1-5，2026-09-20 起）**——真摘要的可观察面，键表见 §15：

| 键 | 语义 |
| --- | --- |
| `enabled` / `client` / `has_api_key` / `endpoint` | 客户端是否可用、种类（`disabled` / `openai` / `injected`）、有没有解析出 key（**布尔可以出门，值不可以**）、端点 |
| `note` | 「开了总闸但没配齐」的可读原因（缺端点 / key 查不到）；未启用则 `null` |
| `pending` / `idle` | 后台线程是否在跑（`idle = !pending && 无回执通道`） |
| `version` / `covers_upto` / `text` | 当前生效版号（0 = 无摘要）、覆盖到桶内第几条、摘要正文（无 → `null`） |
| `evicted` / `pending_records` / `kept_recent` | 已淘汰条数（位点换算用）、待压条数、保留最近几轮原文 |
| `cooldown_left` / `marks` | 距下次可触发还有几轮、累计触发次数（含后台失败的那几次） |
| `bucket_chars` / `bucket_ratio` | 桶内全部原文字符数、与注入预算的比值（**触发判据**） |
| `last_error` | 上次失败的**可读原因**（成功后清空）；失败 ≡ 无摘要 |
| `sidecar` | 旁车文件路径（排障用） |

平铺的旧键 `summary_pending` / `summary_marks` / `summary_cooldown_left` /
`summary_keep_recent_turns` **一个未删**（同值派生，兼容别名）。

### 9.1 清空记忆库（`command("clear")`，产品级加强波次）

HTTP：`POST /api/v1/mods/memory/command`，body `{"command":"clear","args":{}}`
（`args` 可省）。面板按钮「清空记忆库」走 `onCommand('clear', args)`，`args` 带
`session_id`（有会话时；见 §13.1）——**只清那个会话的桶**。

```json
// args（可省；缺 session_id → 老路径全局桶）
{"session_id": "1757890123456789-3"}
```

- **只动 JSONL**：`JsonlStore::clear()` = 同目录 `<store>.jsonl.tmp` + `rename`
  **原子重写为空**，**不是** `remove_file`（理由见 `store.rs::clear` 头注）；
  文件本来不存在也会创建一个空文件（「空库」在盘上可见）；
- **不写 `persona.system_prompt`**：残留按既有 `strip_residue` 生命周期处理（§5.2）；
- 返回（`200 {"ok":true,"result":{…}}`）：

| 键 | 语义 |
| --- | --- |
| `cleared` | 恒 `true`（执行成功才有 200） |
| `records` | **清空后**现存条数，恒 `0`（与 `state_json.records` 同口径） |
| `removed` | 本次清掉的条数（清空前的有效记录数） |
| `residue` | **该桶对应提示词**里当前是否仍有注入块（有会话看该会话覆盖，无会话看全局；只读探测，供面板如实提示，§5.2） |
| `session_id` / `bucket` | 回显本次操作的目标会话（`null` = 全局）与桶路径，供面板核对 |

- **累计计数不重置**：`writes` / `hits` / `injects` / `evicted` 是生命周期计数，
  `records` 才是「现在库里有多少」；
- 失败码：`409 unsupported_command`（不认识命令）/ `409 command_failed`
  （路径不可配置 / 读失败 / 重写失败，且 `errors += 1`）/
  `503 command_unavailable`（未启用或 Mod worker 正持锁，**可重试**）。

## 10. 门禁与测试

- `CARGO_INCREMENTAL=0 cargo test -p live2d-ai-mod-memory` → **117 passed / 0 failed**
  （Wave 2 起 44 → Wave 3 +17 → 产品级加强波次 +8 → L1 +25 → **P1-5 真摘要 +23**）；
- `cargo test --workspace --all-targets` → **1313 passed / 0 failed**；
- `cargo test --doc --workspace` → 3 passed；
- `cargo fmt --all -- --check` clean；
- `cargo clippy --workspace --all-targets -- -D warnings` → 0 warning；
- `cargo run -p xtask -- rust-ratio` → **96.9784% PASS**；
- `cd shell/flutter && flutter analyze` 无问题 + `flutter test` → **1001 passed**
  （含 `memory_panel_test.dart` 的 29 条：渲染 / list 两行 / 导入 / 编辑 /
  删除二次确认 / 会话降级文案 / 清空 / 带码文案 / **摘要状态与回滚按钮** / 说明）。

L1 新增/更新的测试（Rust，见 `commands_tests.rs` 与 `store.rs` / `strategy_tests.rs`）：

- **id**：`fnv1a` 固定向量与稳定性、`make_id` 形状/敏感性、落盘带 id、
  旧行两次载入 id 相同（`id_derivation_is_stable_across_loads` 同款断言）；
- **list**：最新在前 + `total` + 两次载入 id 相同；limit 钳位与非法值回落缺省；
- **import/update/delete**：round-trip（tempdir，含原子重写与无 `.tmp`）；
  空 text / 缺 id / 不存在 id 的可读错误且无副作用；
- **会话分桶**：`sessions/<id>.memory.jsonl` 真落地、无会话保持老路径、
  A/B 桶不互见、清 A 不动 B、非法会话 id 可读错误、事件里非法 id 退回全局；
- **按会话注入**：写会话覆盖而非全局、可叠在 persona 的会话卡上、宿主无能力时
  绝不退化成全局、`state_json` 三个会话键、`shutdown` 清空会话覆盖。

产品级加强波次新增/更新的 8 条（Rust）：

- **`records` 计数**（`tests.rs` 2 条）：现存条数 = 文件有效行数、取快照只读不改盘
  （无 `.tmp`）；路径不可解析 → `null`（不是 0 条、不谎报 `errors`）；
- **`clear` 命令**（`tests.rs` 4 条）：清空后 `records == 0` 且**文件真被重写**
  （仍在、内容为空、无 `.tmp` 残留）、报告 `removed`、累计计数不重置、清空后再记
  一轮 `records` 回到 1；`clear` **不碰** `persona.system_prompt`（并如实报
  `residue`）；未知命令 → `UnsupportedCommand` 且无副作用；无路径 → `command_failed`
  + `errors = 1`；
- **`JsonlStore::clear`**（`store.rs` 2 条）：原子重写为空且不留临时文件；文件不存在
  时创建空文件（「空库」在盘上可见）。

Wave 3 新增/更新的 17 条：

- **质量基线**（`quality_tests.rs`，7 条）：fixture 结构与引用完整性、正向命中序列、
  负向零命中、同义改写边界（含 note 断言）、ASCII 大小写、**运行时级「不重叠 →
  `last_hits=0` → 不注入」**、运行时级命中注入期望记忆；
- **物理淘汰**（`store.rs`，5 条 + `tests.rs` 1 条更新）：超限截断保留最新 /
  未超限不淘汰 / cap=0 不淘汰 / 重写清坏行 / cap=1 只留最后一条 /
  `max_records` 是物理上限（运行时从文件原文断言）；
- **计数与共存**（`tests.rs`，5 条新增）：四计数键形状与别名同值、
  `errors` 两条失败路径、无相关记忆零命中不注入、last-writer-wins 两向对称。

原有覆盖面（分词 / 打分 / top-k / 平局 / marker / 路径 / JSONL / 生命周期 /
self-hit / 幂等 / 停用清残留）一条未删。

涉及文件：

- `crates/live2d-ai-mod-memory/src/lib.rs`（Mod 集成：descriptor / runtime / factory / 计数 / 物理淘汰接线）
- `crates/live2d-ai-mod-memory/src/config.rs`（`MemoryConfig` + 静态 `settings_spec`；Wave 3 从 lib.rs 拆出）
- `crates/live2d-ai-mod-memory/src/strategy.rs`（纯逻辑：分词 / 打分 / top-k / marker / 路径）
- `crates/live2d-ai-mod-memory/src/store.rs`（JSONL 读写 + `append_capped` / `rewrite`）
- `crates/live2d-ai-mod-memory/src/summary_store.rs`（**P1-5 新增**：旁车版本栈 + 原子写 + 回滚）
- `crates/live2d-ai-mod-memory/src/summary.rs`（**P1-5 新增**：触发/切片/归一化/解析的纯逻辑 + 客户端 trait）
- `crates/live2d-ai-mod-memory/src/summary_http.rs`（**P1-5 新增**：OpenAI 兼容摘要客户端 + loopback mock 回归）
- `crates/live2d-ai-mod-memory/src/summary_flow.rs`（**P1-5 新增**：触发 / 后台线程 / 收结果 / 回滚 / 状态）
- `crates/live2d-ai-mod-memory/src/test_support.rs`（两测试模块共用替身）
- `crates/live2d-ai-mod-memory/tests/fixtures/quality_corpus.json`（固定语料）
- `crates/live2d-ai-mod-memory/src/{quality_tests,strategy_tests,summary_tests,tests}.rs`（单测）
- `shell/flutter/lib/settings/mods/memory_panel.dart`（产品面板：计数 / 清空 / **摘要状态 + 回滚** / 说明）
- `shell/flutter/test/memory_panel_test.dart`（面板单测：渲染 / 清空 / 带码文案 / **摘要**）

## 11. 已知缺口 / 下一步

1. **没有备份 / 撤销**：物理淘汰删了就没了（§6.1）。真要做，先定义备份位置、
   保留策略与恢复入口——不是「顺手加个 `.bak`」；
2. **全量读盘**：每轮 `append_capped` 计数读一次 + 检索读一次；每次取 `state_json`
   （面板展开 / 刷新运行态）再读一次算 `records`。默认 200 条可接受；窗口放大前
   先做「读尾部 N 行」或索引；
3. **检索质量只覆盖 fixtures**：fixtures 是回归基线，不是「质量分数」；Jaccard 无 idf，
   长记录会被稀释。要改先加样例再改实现；
4. ~~无逐条管理面~~ **L1（2026-09-15）起有**：查看 / 导入 / 编辑 / 逐条删除 /
   清空（§13）；**仍无导出 / 备份 / 撤销**——淘汰与删除都是物理删除，删了就没了
   （§6.1 / §13.4）；
5. **与 persona 的边界**：全局路径 last-writer-wins（§5.1），会话路径按来源槽叠加
   （§14）；两种都不做仲裁、不做合并；
6. `max_records` 同时是上限与窗口：语义重叠是刻意的（上限生效后窗口必然不溢出），
   但**改默认值前要知道它现在会删数据**；
7. **摘要是「累积式」而非「滚动式」**：每版只压「尚未覆盖的原文」，所以最新版覆盖的
   区间是单调变大的。好处是可预测、可回滚；代价是**上一版摘要的内容会随新版本一起
   被替换**（你看不到 v1 的全文，除非它在 `versions` 栈里）。真要做「增量 + 合并」
   得先定义合并冲突语义——不属于「最小」；
8. **摘要仍会读到被淘汰的原文**：旁车记的是位点，桶头部被物理淘汰后，摘要里那段
   历史是**唯一**的副本（原文已删）。这是刻意的（摘要的价值正在此），但意味着
   「删原文」不等于「删记忆」——想彻底忘掉一段历史，得同时清空桶与旁车（`clear` 做得到）。

## 12. 「相关内容下一轮会被提起」逐条可复制验收

前置：`memory` **缺省停用**——先在「设置 → Mod → 本地记忆」卡片标题行把它
启用（状态变「运行中」）。运行态读数是卡片展开后的「运行态（只读）」块，
或 `GET /api/v1/mods/memory/state`（§9）。

1. **看空库**：展开卡片 → `当前条数 0`、`写入条数 0`、`上轮命中 0`。
2. **记第一句**：聊天框发 `记住：我叫星梦，喜欢薄荷`，等这一轮回复结束
   （停止键变回发送键）。
3. **验写入**：点「刷新运行态」→ `当前条数 1`、`写入条数 1`、`注入轮数 0`
   （本轮请求体在这句入库**之前**就已发出，见 §2）。
4. **第二次提问**：发 `我叫什么？喜欢什么？`。memory 在这一轮检索到第 2 步的
   那句话；再点「刷新运行态」，应看到 `上轮命中 ≥ 1`、`累计命中 ≥ 1`、
   `注入轮数 1`。**注意**：这一轮的请求体里**还没有**注入块（注入只对下一轮
   生效，§2），所以此轮回复**不要求**出现「薄荷 / 星梦」——别把它当失败。
5. **验下一轮真被提起**：再发一句相关的话（例：`再说一次我喜欢什么`）。这一轮
   的请求体已经带着第 4 步注入的块，回复里应出现 `薄荷` / `星梦`（或明确表示记得）。
6. **清空**：点「清空记忆库」→ 文案 `已清空记忆库：清掉 N 条，现存 0 条`；
   刷新运行态 → `当前条数 0`。若提示「仍留着上一轮注入的记忆块」，那是 §5.2 的
   既有语义：停用 Mod 时 `strip_residue` 会剥掉（`clear` 本身不碰提示词）。
7. **复跑**：清空后重复第 2–5 步应得到同样结果（幂等，不残留旧记忆）。

排查（对应错误面，都带码）：

- 第 4 步 `上轮命中` 仍 `0`：v0 是**词面重叠**检索，同义改写不命中（§4.1）；
  换一句与第 2 步**有共同词**的问题，或确认 `top_k ≥ 1`；
- 第 5 步回复没提起：确认 `enabled_injection` 没被关（面板「注入开关」说明它
  != Mod 启停，见 §3）；确认第 4 步 `注入轮数` 真的 +1；
- 清空失败：文案带码——`503 command_unavailable` 可重试（worker 正忙 / 未启用），
  `409 command_failed` 看服务端日志（路径 / 权限），`404 not_found` 说明 Mod 不在注册表。

> 时序真源 = §2 的时序图。**不要**期望「同轮就提起」。

## 13. 主动管理（面板命令契约，L1 2026-09-15）

「主动线」= 用户在 **设置 → Mod → 本地记忆** 面板里管理记忆，**不依赖**聊天口令
「记住 / 忘掉」。实现见 `crates/live2d-ai-mod-memory/src/commands.rs`；HTTP 一律
`POST /api/v1/mods/memory/command`，body `{"command":"<名>","args":{…}}`。

### 13.1 通用口径（每条命令都适用）

- **候选桶**：`args.session_id` 给了就用它（先过 `sanitize_session_id` 路径穿越闸）；
  **没给 → 老路径全局桶**。面板在 `activeSessionId == null` 时**不传** `session_id`
  ——与面板上「还没有会话：记忆会落到全局桶」那句话**逐字同源**；
- **失败码**（host 映射见 `web_api/mods_routes.rs`）：不认识命令 → `409
  unsupported_command`；参数坏 / 找不到 id / IO 失败 → `409 command_failed`
  （文案直接提示「先 list 拿 id」）；未启用或 worker 正持锁 → `503
  command_unavailable`（**可重试**）；
- **成功返回**统一 `200 {"ok":true,"result":{…}}`，下面写的是 `result` 的键；
- 命令**不碰** `persona.system_prompt`（`clear` 只**只读探测** `residue`）——注入残留
  仍按 §5.2 的 `strip_residue` 生命周期处理。

### 13.2 记录 id

每条记录有稳定 id **`<ts>-<turn>-<fnv1a(text) 8 位十六进制>`**
（`MemoryRecord::make_id`；FNV-1a 32 位自实现，5 行、无依赖）：

- **新写入**（被动 `remember` / 面板 `import`）按公式生成并**落盘** `id`；
- **旧行没有 `id`** → 载入时按同一公式**派生**（`parse_record_line`）。公式只吃
  `ts` / `turn` / `text` 三个已落盘字段，所以同一条老记录每次载入 id 相同
  （回归 `store.rs::legacy_line_without_id_derives_the_same_id_every_load`）；
- `import` 在同一秒导入同一句话时**递增 turn 直到 id 不冲突**——否则连点两次会得到
  两条同 id 的记录，`update` / `delete` 会一次命中两条；
- `update` **保持原 id**：编辑是「改这条记录」，不是「删了再建」。代价是改写后该行的
  id 不再等于按新正文现算的公式值——`id` 从此是**持久化身份**。

### 13.3 命令表

| command | args | `result` |
| --- | --- | --- |
| `list` | `{"limit"?: number, "session_id"?: string}` | `{ok, records:[{id,text,ts,turn}], total, bad_lines, limit, bucket, session_id}` |
| `import` | `{"text": string, "session_id"?: string}` | `{ok, id, records, total, evicted, bucket, session_id}` |
| `update` | `{"id": string, "text": string, "session_id"?: string}` | `{ok, id, records, total, bucket, session_id}` |
| `delete` | `{"id": string, "session_id"?: string}` | `{ok, id, removed, records, total, bucket, session_id}` |
| `clear` | `{"session_id"?: string}` | `{ok, records, cleared, removed, residue, bucket, session_id}` |

- **`list` 最新在前**（追加序的逆序）；`limit` 缺省 **50**、上限 **200**、非法值
  （非数 / 0 / 负数 / NaN）**回落缺省**；`total` 是桶里全部条数（不受 limit 影响）；
- **`import`** 走 `append_capped`：受 `max_records` 上限约束，超限与被动的写入一样
  **物理淘汰最旧**（返回 `evicted`）；`text` 去首尾空白后为空 → 可读错误；
- **`update` / `delete`** 用 `JsonlStore::write_all`（同目录 `.tmp` + `rename` 原子替换）。
  id 不存在 → 可读错误且**文件一字不改**；
- **`clear`** 语义同 §9.1，只是作用域变成「该桶」，并在返回里回显 `session_id` / `bucket`。

### 13.4 取舍（写清，不许含糊）

| 得 | 失 |
| --- | --- |
| 用户能看见、能改、能删——不再只能靠聊天口令或手改 JSONL | **没有撤销 / 备份**：`delete` / `clear` / 淘汰都是物理重写，删了就没了（§6.1） |
| `id` 让面板「刚选中那条」在刷新后仍指得中 | `update` 后 id 与公式不再对应，只能靠持久化的 `id` 字段识别 |
| 命令与被动路径共用同一个 JSONL 与同一条上限 | 每条命令一次全量 `load(0)`（+ 可能一次重写）——默认 200 条量级可忽略（§11.2） |

### 13.5 面板（`shell/flutter/lib/settings/mods/memory_panel.dart`）

- 进入面板 / 点「刷新」→
  `onCommand('list', {'limit': 50, 'session_id': ctx.activeSessionId})`（无会话则不带）；
- 每条：截断正文 + 「编辑」（对话框 `TextFormField` → `update`）+「删除」
  （`AlertDialog` 二次确认 → `delete`）；
- 「导入一条」：输入框 + 按钮 → `import`；空输入**不发命令**，直接给可读文案；
- 「清空记忆库」保留（带当前会话说明）；
- 每个成功动作后 `ctx.onRefreshState()` + `ctx.notifyChanged('已…记忆…')`；
  失败显示**带 code** 的文案（`command_unavailable` / `unsupported_command` /
  `command_failed` / `not_found` 各有分支）。

## 14. 会话分桶（L1 2026-09-15）

目标：**尽量与会话分桶对齐 persona**——A 会话记的东西不出现在 B 会话。

### 14.1 桶路径

| 情形 | 路径 |
| --- | --- |
| 有会话（`TurnPrompt` 带 session，或命令带 `session_id`） | `<老路径同目录>/sessions/<session>.memory.jsonl` |
| 无会话（裸 HTTP / 终端壳；命令不带 `session_id`） | **老路径** `<老路径同目录>/memory.jsonl` |

「老路径」= §3 的 `store_path` / `config_path` 解析结果（默认
`<配置目录>/memory.jsonl`）。会话桶跟着**已解析出的老路径的父目录**走，所以自定义
`store_path` 时桶也在那个库旁边。

**兼容红线**：无会话路径与升级前**逐字相同**——老记忆必须还能读到；`sessions/`
目录只会在真有会话写入时才被创建。

session id 要进文件名，一律过 `sanitize_session_id`（只允许 ASCII 字母数字与
`- _ . :`，≤128 字符）；非法 id 在**事件**里退回全局桶 + warn，在**命令**里回可读
错误（绝不静默改道，也绝不路径穿越）。

### 14.2 注入也按会话

- **有会话**：命中写进
  `services.session_prompts.set(session, compose_injection(base, memories))`；
  base = `session_prompts.get(session)`（该会话当前覆盖，可能是 persona 的卡），
  没有才回落全局 `persona.system_prompt`。**不写**全局 `persona.system_prompt`
  ——否则 A 会话的记忆会串到 B；
- **无会话**（P1-5 起）：记忆仍落**老路径** __T__memory.jsonl__T__，但**不注入**——
  **绝不**再写全局 __T__persona.system_prompt__T__（旧「降级」已删除）。日志与面板
  如实说「无 conversation，本轮不注入」；
- 宿主没注入会话能力（`session_prompts.enabled() == false`）时，有会话就**不注入**
  （warn + `errors += 1`），**绝不**退回全局——那等于把 A 的记忆广播给所有人；
- `shutdown`：`session_prompts.clear_all()` + 老的 `strip_residue()`。前者的已知取舍
  （共享覆盖表会被一起清空）写在 `lib.rs::shutdown` 头注。

### 14.3 时序（2026-09-15 起）：**同轮生效**

会话分桶只改「记去哪个桶、注入写到哪个覆盖」，不改时序（§2）。有会话时同样在
`TurnPrompt`（本轮请求体构建之前、且**同步投递**）发生，因此：

> 注入在**本轮的请求体里**就已生效。用户这一句能命中检索，本轮回复就用得上。

面板导入 / 编辑 / 删除后同理：**下一条命中它的用户话，本轮就会带上**——面板
成功文案里逐字写了这一点。

### 14.4 面板的降级语义

`ctx.activeSessionId == null` 时面板显示：

> 还没有会话：记忆会落到全局桶（与所有会话共享）。发一条消息后，面板会切到该会话自己的桶。

此时面板**不传** `session_id`，list / import / update / delete / clear 全部落在全局桶
——UI 说的就是命令实际做的。

---

## 15. 真摘要（P1-5 补齐，2026-09-20）

一句话：**注入预算被撑大时，把更早的历史压成一段短摘要**，而不是继续整条塞原文。

### 15.1 触发（全中才起后台）

| 条件 | 判据 |
| --- | --- |
| 总闸开 | `summary_enabled=true`（缺省 **false**，显式开启） |
| 客户端可用 | `summary_base_url` + `summary_model` 都非空、URL 合法（见 15.4） |
| 桶够大 | **桶内全部原文的字符数** > `injection_budget_chars` × `summary_ratio`（缺省 0.75） |
| 待压够多 | `summary::build_request` 能切出 ≥2 条（见 15.2） |
| 过冷却 | 距上次**真的发出请求** ≥ `summary_cooldown_turns` 轮（缺省 8） |

为什么判据是**桶的总字符**而不是「本轮注入块占用」：本轮注入块只有 top-k 几条、
永远很小；真正在膨胀的是**库**。按注入块判会让阈值永远不触发——那正是
「标记了待摘要却从没有摘要」的现场。`state_json.summary.bucket_ratio` 就是它。

**非阻塞**（硬要求）：`TurnPrompt` 走同步投递（supervisor 等 Mod worker 回执），
所以 HTTP **不在 worker 上跑**——`std::thread::spawn` 一个后台线程，主链本轮只
`try_recv` 收**已经跑完**的结果。收不到就继续用「无摘要 + top-k 原文」。

### 15.2 输入与输出

```text
桶里 n 条原文：  [0 .. covers_upto)                 已被当前摘要覆盖（不再重复喂）
                 [covers_upto .. kept_start)       <- 本次摘要的输入（用户 / 助手交错）
                 [kept_start .. n)                 最近 keep_turns 轮（缺省 4）
```

- **保留最近 K 轮原文**（`summary_keep_recent_turns`，缺省 4）：最近几轮还在上下文
  窗口里，压进摘要既浪费 token 又会让「刚刚说的话」变成二手转述；
- **「轮」= 记录的 `turn` 字段**（Wave 3，2026-09-21）：助手侧入桶后，同轮的
  user + assistant 两条共用一个 turn、一起保留——按「最后 K 条」算会让 4 条记录只
  等于 2 轮（§16）；
- 每条按**角色前缀**进 user 消息（`- [用户] …` / `- [助手] …`）：摘要压的是
  **双方对话**的要点，模型必须知道哪句是谁说的；
- 输入有字符上限（8 000）：超出从**最旧**一侧丢，`covers_upto` 只推进到真正喂进去的
  那几条——没喂进去的下轮再来；
- **输出**：`covers_upto`（覆盖到桶内第几条，**位点**）+ `covers_turn` + `text`；
- 正文归一化：压平空白、剥 `摘要：` / `- ` 之类包装、超 600 字截断；归一化后为空按
  **失败**处理（绝不写空摘要）。

### 15.3 落盘 / 版本 / 回滚

旁车文件**跟着桶走**（`x.memory.jsonl` → `x.memory.summary.json`）：

```json
{
  "versions": [
    {"version": 2, "covers_upto": 9, "covers_turn": 9, "created_at": 0, "text": "…"},
    {"version": 1, "covers_upto": 4, "covers_turn": 4, "created_at": 0, "text": "…"}
  ],
  "generated": 2,
  "last_error": null,
  "last_attempt_at": 0
}
```

- `versions` **最新在前**（`versions[0]` = 当前生效），栈上限 10；
- 写盘 = 同目录 `.tmp` + `rename`（与 JSONL 同一条原子纪律）；
- **回滚**（`command("summary_rollback")`）= 丢掉 `versions[0]`：新头的 `covers_upto`
  更小 → 被那版覆盖的原文**重新变回「未被覆盖」**、又能进 top-k；
- **回滚不删原文**——原文从头到尾都在 JSONL 里，这就是「回滚 = 用回原文」；
- 物理淘汰会让桶头部左移：有效位点 = `covers_upto - 已淘汰条数`（钳到 `0..=len`），
  回归 `summary_tests::uncovered_start_accounts_for_physical_eviction`；
- `command("clear")` **连带删除**旁车（桶空了，摘要描述的就是不存在的历史）。

### 15.4 LLM 配置（**独立于主链 `[llm]`**，写清为什么）

memory 是 Mod，拿到的是 host 的**脱敏**设置快照——里面没有主链 `base_url`、更没有
`api_key_env` 变量名（这是密钥纪律，不是遗漏）。所以摘要用自己的四个键：

| key | 语义 | 缺省 |
| --- | --- | --- |
| `summary_enabled` | 总闸 | `false` |
| `summary_base_url` | OpenAI 兼容 base URL（空 = 不摘要） | `""` |
| `summary_model` | 模型名（空 = 不摘要） | `""` |
| `summary_api_key_env` | API key 的**环境变量名**（空 = 不鉴权；值走 `.env` / 进程环境） | `""` |
| `summary_timeout_ms` | 单次请求超时（500–20000） | `8000` |

**想复用主链那把 key**：把同一个变量名填进 `summary_api_key_env` 即可（`.env` 只有
一份）。想复用主链端点：把 `[llm].base_url` / `model` 抄进来——**不自动继承**，
因为「两个真相」比「多填两格」贵。

### 15.5 注入（五段里的「摘要」段）

摘要在**本 Mod 自己的会话槽里**打头：

```text
<!-- live2d-ai:memory-v0:begin -->
- [摘要] 用户自称星梦，喜欢薄荷，养了一只叫煤球的猫…
- 我明天要早起
<!-- live2d-ai:memory-v0:end -->
```

- 保持同一个 owner 槽（不新开一个 owner）：新开槽会让摘要块排到 persona 之后、
  历史之前，与 §0 定下的段序对不上；
- 摘要行**先占位**再裁 top-k：预算不够时先丢原文命中，不丢摘要；
- 已被摘要覆盖的原文**不进 top-k**（否则同一个意思出两次，压缩等于没做）；
- 有会话才注入；无会话不摘要、不注入、**不写全局 persona**（15.6）。

### 15.6 边界（写死，测试守）

| 情形 | 行为 |
| --- | --- |
| 无 conversation（裸 HTTP / 终端壳） | **不摘要**（没有注入面；老桶跨会话共享，压了就是第二个全局真相） |
| 客户端 disabled / 没配齐 | 越阈值也不排队（`pending=false`）、不建旁车；原文照常 top-k 注入 |
| 超时 / 非 2xx / 坏 JSON / 只有思考 | `None` → **旁车 versions 一字不改**、只记 `last_error`；等同「无摘要」 |
| A / B conversation | 各自旁车 + 各自注入槽，互不可见（回归 `conversations_do_not_leak_summaries_into_each_other`） |
| 停用 Mod | 清本 Mod 的会话槽（`clear_owner`）——旁车**保留**（重新启用还能回滚） |

### 15.7 命令面

| command | args | `result` |
| --- | --- | --- |
| `summary` | `{session_id?}` | `{ok, summary:{…}, bucket, session_id}`（与 `state_json.summary` 同源） |
| `summary_rollback` | `{session_id?}` | `{ok, rolled_back:{version,covers_upto,text}, version, covers_upto, text, bucket, session_id}` |

没有可回滚的版本 → `ModError::Other`（host 409 `command_failed`），**不谎报成功**。

### 15.8 逐条可复制验收

前置：memory 已启用；`.env` 里那把 key 的变量名填进 `summary_api_key_env`；
`summary_enabled=true` + `summary_base_url` + `summary_model`。

1. **看状态**：`GET /api/v1/mods/memory/state` → `summary.client="openai"`、
   `summary.version=0`、`summary.bucket_ratio` 随聊天增长；
2. **触发**：连着聊到 `bucket_ratio > 0.75`（或把 `injection_budget_chars` 调到下限
   200 快速复现）→ 日志出现「memory 桶内原文 N% 越过摘要阈值：后台起摘要」；
3. **落地**：下一轮（或刷新状态）→ `summary.version=1`、`summary.text` 非空、
   桶旁边多出 `*.summary.json`；
4. **进注入**：再聊一句相关的话 → 注入块里出现 `- [摘要] …`；原文命中紧随其后；
5. **回滚**：`POST /api/v1/mods/memory/command` body
   `{"command":"summary_rollback","args":{"session_id":"…"}}` → `version` 退回 0
   （或上一版）；下一轮注入不再含该摘要，被它覆盖的原文重新出现在命中里。
   **原文条数不变**。


---

## 16. 助手侧记忆（Wave 3，2026-09-21）

一句话：**一轮收口时把「助手已说出 / 已上屏的正文」也按 conversation 记进同一桶**，
摘要的待压窗口因此是**用户 + 助手交错**的对话，而不是只有用户话。

### 16.1 事件（host 侧唯一入口）

| 项 | 值 |
| --- | --- |
| 主题 | `ModEventTopic::AssistantReplied`（稳定 id `assistant_replied`） |
| payload | JSON `{"turn":<turn id>,"role":"assistant","text":"<清洗后正文>","interrupted":<bool>}` |
| 发点 | supervisor 空闲态、`turn::run_one_turn` 返回之后、**`TurnEnded` 之前** |
| 会话 | 与同轮 `TurnPrompt` 同一个 session（记忆据此分桶；`None` → 老桶） |
| 投递 | **非阻塞**（`try_send`）；host 只对 `TurnPrompt` 走同步投递 |
| 文本 | **确定性清洗后**的上屏口径（与送 TTS 同源，见 §16.4） |

为什么不用已有主题：`TurnPrompt` 只有用户侧正文；`SentenceReady` 是按句锚点
（一轮多条、且语义是「即将送 TTS」）。记忆要的是「一轮的助手侧正文」，缺它
就只能永远压用户话——这正是上一波 §11 / 15.1 记下的缺口。

### 16.2 入桶（与用户侧完全对称）

- **有 conversation** → `sessions/<id>.memory.jsonl`；**无 conversation** → 老路径
  `memory.jsonl`（只记）；
- `role=assistant` 落盘（JSONL 的 `"role"` 字段）；**老行没有该字段 → 解析回
  `user`**（升级前它们全是用户输入，老 id 派生公式也不变）；
- 记录 id：user 沿用 `<ts>-<turn>-<hash8>`；assistant 为 `<ts>-<turn>-a<hash8>`
  ——同秒、同轮、同一句话也**不会撞 id**（否则面板 delete 会把两条一起删）；
- 写失败 / payload 坏 / 空正文 / 非法会话 id → **warn + 跳过**，绝不 panic、
  绝不打断主链（记忆是增量能力）；
- `state_json.writes` 现在是总数，新增拆分 `user_writes` / `assistant_writes`。

### 16.3 注入与摘要的角色区分

- 注入行：`- [用户] …` / `- [助手] …`（`MemoryRecord::line()` 是唯一格式化处，
  与摘要行 `- [摘要] …` 同一套括号风格）；
- 摘要输入同上（`summary::build_request` 复用 `line()`）；摘要 system 提示明确
  要求「[用户] / [助手] 的归属不要搞反」；
- 检索仍按词元重叠，**不区分角色**——助手说过的事实也能被用户后续提问检索到。

### 16.4 与确定性清洗的关系（本波第 3 件）

助手正文交给 Mod 前经过 `live2d_ai_runtime::dialogue::clean_for_tts`（纯函数）：
剥成对全 / 半角括号动作描写、成对星号舞台指示、多余 Markdown；不改语义、不缩写、
不重排句边界。清洗的**调用点在上屏之前**（引擎切句之后），因此「送 TTS 的文本 =
上屏的文本 = 交给记忆的文本」——一份产物，三处消费。

### 16.5 验收（可复制）

1. 启用 memory；发一句（例：`记住：我喜欢薄荷`），等回复结束；
2. `GET /api/v1/mods/memory/state` → `writes` / `assistant_writes` ≥ 1；
3. 打开 `sessions/<会话 id>.memory.jsonl` → 一轮有**两行**：
   `{"role":"user",…}` 与 `{"role":"assistant",…}`，`turn` 相同；
4. 再发一句相关的话 → 注入块里同时出现 `- [用户] …` 与（若相关）`- [助手] …`；
5. 摘要在同一批记录上触发 → `summary.covers_upto` 数到**助手行**，摘要请求体里
   出现 `- [助手] …`，且最近 K 轮的用户 + 助手一起被排除。

### 16.6 边界（写死）

| 情形 | 行为 |
| --- | --- |
| 无 conversation | 助手侧仍落老桶（只记）；**不注入、不写全局 persona、不摘要** |
| 助手正文为空 | 不投递事件（host 侧判空）；收到的空白 text 也不落盘 |
| 清洗后为空（整句都是动作描写） | 该句走静音句路径、不进 TTS；整轮 text 相应为空则不投递 |
| `/stop` 中断 | 事件仍投递（`interrupted=true`），消费方自决；memory 照记 |
| Mod 未启用 / 停用 | 不收到事件；停用只清本 Mod 的会话槽，桶文件与旁车保留 |

