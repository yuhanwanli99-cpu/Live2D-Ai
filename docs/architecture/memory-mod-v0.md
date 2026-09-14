# 本地记忆 Mod v0（`live2d-ai-mod-memory`）

> **状态**：2026-09-14 **产品级加强波次**（memory 轨，worktree
> `Live2D-Ai-pg-memory` @ 基座 `72d1af17`）更新——`state_json` 新增
> **`records`（库里当前实际条数）**、实现 **`command("clear")`**（原子重写 JSONL 为空）
> + 产品面板（条数 / 命中 / 注入 / 淘汰 / 上轮命中 / 清空按钮 / 注入开关与 persona
> 策略说明）+ §12 逐条可复制验收步骤。**版本仍 `0.2.0-rc.3`**（不 bump / 不 tag）。
> 上一版：2026-09-14 **Wave 3 C 轨**（分支 `mod/w3-memory` @ 基座 `118bd435`）——
> 补检索**质量基线 fixtures**、**条数上限物理淘汰**、`state_json` **四个计数键**，
> 并把与 persona 的 **last-writer-wins** 用两向对称测试钉死。
> 再上一版：Wave 2 C 轨（分支 `mod/memory-v0` @ `429609f2`）起草。
> **注册已完成**（Wave 2 收束落盘）：memory 是 `AVAILABLE_MOD_FACTORIES` 的 6 个工厂之一，
> **缺省停用**（`cli_entry` 缺省 manifest 不含 memory）；本轨**不改注册面**。
> 上层协议：[PARALLEL-WAVE3-2026-09-14.md](../plans/parallel-mods/PARALLEL-WAVE3-2026-09-14.md) §3C；
> 集成说明：[REGISTER-memory-v0.md](../plans/parallel-mods/REGISTER-memory-v0.md)；
> Mod 通用契约：[mod-product-chain.md](mod-product-chain.md)。

## 1. 定位

本 Mod 只做一件事：**把用户说过的话记在本机，并在下一轮把最相关的几条放回
system_prompt**。它不接管 LLM 客户端、不新增对话通道、不做摘要/改写——
只是「记忆的存取」。

五个刻意选择：

| 选择 | 理由 |
| --- | --- |
| 检索 = 纯函数词元重叠（中文 bigram + ASCII 词） | 无向量 / embedding / 网络依赖；可脱机单测；行为可解释（为什么这条被检索到，能指着词元说） |
| 存储 = 本地 JSONL，追加写、逐行读 | 记忆只增不改，JSONL 与它天然匹配；坏行局部化；不引数据库依赖 |
| 注入 = 经 `ModServices.apply_settings` 写既有 `persona.system_prompt` | 主链只有一个 system 入口；Mod 不另开通道（与 persona 同一条纪律） |
| **淘汰 = 条数上限，写入超限时物理删除最旧**（Wave 3） | 文件有界、检索/注入不随历史无限变慢；数据丢失的取舍写死在 §6.1 |
| **质量基线 = 固定 fixtures + 命中/不命中断言**（Wave 3） | 质量口径可回归，不靠手感（§4.1） |

## 2. 时序（**只对下一轮生效**，不是缺陷）

`TurnPrompt` 在 supervisor 提交 turn 时、**本轮请求体构建之前**发出
（`crates/live2d-ai-desktop/src/supervisor.rs` 的提交点，紧随 `TurnStarted`）。
因此本 Mod 在这里做的 `apply_settings` 写回**只对下一轮生效**：

```text
用户输入 ──► supervisor：TurnStarted（turn id）
             └─► supervisor：TurnPrompt（本轮正文）        ◄── memory 在这里干活
                  ├─ 1. remember(正文, now, turn)            → 追加 memory.jsonl 一行
                  │      └─ 超过 max_records → 物理淘汰最旧（写临时文件 + rename）
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
| `max_records` | number | **条数上限（count cap）**：文件里最多保留这么多条，写入超限**物理淘汰**最旧；同时也是检索窗口。钳在 `1..=10000` | `200` |
| `enabled_injection` | bool | 是否注入（false = 只记不注入） | `true` |

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
6. 不做记忆**查看 / 编辑**面；唯一的删除动作是**清空整库**（面板按钮 →
   `command("clear")`，§9.1）；API 面另有基座通用的
   `GET /api/v1/mods/memory/state`（只读计数，见 §9）。**不做**逐条删 / 导出。

## 9. 运行态快照（`state_json`）

实现基座 Wave 2 的 `ModRuntime::state_json`，**只读、不写盘**；其中 `records`
需要**一次本地全量读**（`JsonlStore::load(0)`，见 §9.1），其余键仍是纯内存计数
（`state_json` 在 web_api 线程上被调用，「不阻塞」指不等待网络 / 锁）：

```json
{"store_path":"…/memory.jsonl","records":12,"top_k":3,"max_records":200,
 "enabled_injection":true,"turns_seen":12,
 "writes":12,"hits":17,"injects":4,"errors":0,"evicted":2,"last_hits":2,
 "remembered":12,"injected":4}
```

**Wave 3 必备四键**（协议 §3C 4）：

| 键 | 语义 |
| --- | --- |
| `writes` | 成功落盘的记忆条数（每次成功 `append_capped` +1） |
| `hits` | **累计**检索到的记忆条数（跨轮累加；`last_hits` 是最近一轮） |
| `injects` | 成功注入主链提示词的次数（`apply_settings` 接受） |
| `errors` | 失败路径次数：路径不可用 / 追加失败 / 读取失败 / 注入或剥离被拒（**不含**坏行） |

附带键：`evicted`（被物理淘汰的条数）、`last_hits`（最近一轮命中数）、
`turns_seen`；`remembered` / `injected` 是 Wave 2 的**兼容别名**，由同一字段
派生（与 `writes` / `injects` 恒同值，不会漂移）。

**`records`（产品级加强波次新增）**：记忆库**当前实际条数**——逐行读 JSONL 数出来
的有效记录数（坏行不计），**不是** `writes`（累计写入，清空也不重置）。文件不存在
→ `0`；路径不可解析 / 读取失败 → `null`（面板显示「—」）并把失败计入 `errors`
——「读不到」不能谎报成「0 条」。代价是一次**全量读**（与每轮两次全量读同一取舍，
见 §11.2）；「不阻塞」指不等待网络 / 锁，本地顺序读不在其列。

路由由基座提供：`GET /api/v1/mods/memory/state`（404 = 不在注册表；
503 `state_unavailable` = 在册但未启用 / worker 正持锁）。

### 9.1 清空记忆库（`command("clear")`，产品级加强波次）

HTTP：`POST /api/v1/mods/memory/command`，body `{"command":"clear","args":{}}`
（`args` 可省）。面板按钮「清空记忆库」走 `onCommand('clear')`（§12 第 6 步）。

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
| `residue` | 提示词里当前是否仍有注入块（只读探测，供面板如实提示，§5.2） |

- **累计计数不重置**：`writes` / `hits` / `injects` / `evicted` 是生命周期计数，
  `records` 才是「现在库里有多少」；
- 失败码：`409 unsupported_command`（不认识命令）/ `409 command_failed`
  （路径不可配置 / 读失败 / 重写失败，且 `errors += 1`）/
  `503 command_unavailable`（未启用或 Mod worker 正持锁，**可重试**）。

## 10. 门禁与测试

- `cargo test -p live2d-ai-mod-memory --all-targets` → **69 passed / 0 failed**
  （Wave 2 起 44 → Wave 3 +17 → 产品级加强波次 +8）；
- `cargo fmt -p live2d-ai-mod-memory -- --check` clean；
- `cargo clippy -p live2d-ai-mod-memory --all-targets -- -D warnings` → 0 warning；
- `cd shell/flutter && flutter analyze` 无问题 + `flutter test` 全绿（新增
  `test/memory_panel_test.dart`：渲染 / 清空 / 带码文案 / 说明）。

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
- `crates/live2d-ai-mod-memory/src/test_support.rs`（两测试模块共用替身）
- `crates/live2d-ai-mod-memory/tests/fixtures/quality_corpus.json`（固定语料）
- `crates/live2d-ai-mod-memory/src/{quality_tests,strategy_tests,tests}.rs`（单测）
- `shell/flutter/lib/settings/mods/memory_panel.dart`（产品面板：计数 / 清空 / 说明）
- `shell/flutter/test/memory_panel_test.dart`（面板单测：渲染 / 清空 / 带码文案）

## 11. 已知缺口 / 下一步

1. **没有备份 / 撤销**：物理淘汰删了就没了（§6.1）。真要做，先定义备份位置、
   保留策略与恢复入口——不是「顺手加个 `.bak`」；
2. **全量读盘**：每轮 `append_capped` 计数读一次 + 检索读一次；每次取 `state_json`
   （面板展开 / 刷新运行态）再读一次算 `records`。默认 200 条可接受；窗口放大前
   先做「读尾部 N 行」或索引；
3. **检索质量只覆盖 fixtures**：fixtures 是回归基线，不是「质量分数」；Jaccard 无 idf，
   长记录会被稀释。要改先加样例再改实现；
4. **无逐条管理面**：唯一的删除动作是清空整库（§9.1）；编辑 / 导出 / 逐条删除仍只有
   手工改 JSONL；
5. **与 persona 的边界只有 last-writer-wins**（§5.1），无仲裁、无合并；
6. `max_records` 同时是上限与窗口：语义重叠是刻意的（上限生效后窗口必然不溢出），
   但**改默认值前要知道它现在会删数据**。

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
