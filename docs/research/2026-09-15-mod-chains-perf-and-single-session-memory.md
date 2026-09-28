# 五 Mod 实现链路 + 速度/可读性专项 + Mem0 单会话记忆调研（2026-09-15）

> **来源（2026-09-28 入库）**：本文原为旧 worktree `/home/skystar/Live2D-Ai` 的**未跟踪唯一副本**，
> 2026-09-28 判定**并入 0.2.0 线**（Mod 侧调研，属挂账）。
>
> **范围调整（用户 2026-09-15 指示）**：个人项目，**工程安全可以少做**；
> 本轮重点 = **① 速度 ② 可读性 ③ 五个 Mod 的实现链路 ④ Mem0 / 单会话记忆系统 ⑤ 各 Mod 同类项目**。
> 安全问题（token 真源、secret 抹除、越权等）已在上一份
> [五 Mod 实现审查](2026-09-15-five-mod-review.md) 记录，本轮只在「影响速度」时才提。
>
> **审查对象**：`mod/l1-product` worktree `/home/skystar/Live2D-Ai-l1`（HEAD `d2bdd4fd` + 未提交工作区）。
> **只读**，未改动被审查 worktree 任何文件。

---

## 1. 五条实现链路（文字版链路图）

标注含义：**P**=同步阻塞点、**L**=持锁、**IO**=磁盘/网络/进程、**T**=线程、
**×N**=每 N 次调用一次。主链 = LLM→TTS→口型→Live2D。

### 1.1 链路 A：聊天输入 → 会话注入（persona / memory 的挂点）

```text
[Flutter] ChatController.send(text, sessionId)
  → POST /api/v1/chat {text, session_id}                       (HTTP 线程)
  → SupervisorHandle::say_scoped → say_tx.try_send(cap=1)      (满 → 429 busy)
  ──────────────────────────────────────────────────────────── [supervisor 线程]
  出队 →
    ├ emit TurnStarted            (try_send, 非阻塞)
    ├ emit TurnPrompt ────────────► mod_event_sink               **L registry Mutex**
    │     └ dispatch_event_and_flush → try_send(channel 256)
    │         ──────────────────────────────────────────────── [mod-event-worker 线程]
    │         for each Running Mod (5 个，字典序): try_lock(runtime) + on_scoped_event
    │           • director       : 纯函数 derive + ledger push + info 日志
    │           • external-input : 本话题无操作（但它每 token 打日志，见下）
    │           • memory         : remember(全量读) + retrieve(全量读 + O(N) 分词)
    │                              → set_owned(owner=memory)             **IO ×2 + CPU**
    │           • persona        : 本话题无操作
    │           • voice-input    : 默认 no-op
    │         ← ack（等所有 Mod 处理完，recv_timeout 1s）         **P ≤1s**
    ├ session_scopes.prompt_for(session) → engine.set_system_prompt_override
    └ run_one_turn → LLM 流式
        每 token: emit TextDelta → try_send → worker 再扇出 5 个 Mod   **×tokens**
                  其中 memory/persona/director/external-input **各打 1 条 info（回显 token 原文）** → 4 行 / token
        TurnEnded → director 结项
```

**要点**：注入是**同轮生效**的（TurnPrompt 同步 flush 在决议之前）；代价是
① `TurnPrompt` 整段持 registry 锁（≤1s）；② 每个事件扇出到**全部 5 个** Running Mod
（订阅表不参与路由）；③ memory 的两次全量读 + O(N) 检索都在**开轮关键路径**上。

### 1.2 链路 B：external-input（HTTP 注入）

```text
[sidecar] POST /api/v1/external/chat {text, token?}
  → external_routes: Origin/loopback → Content-Type → gate(enabled, 停用=403) → token → 长度
  → 计数 record_accept/reject/busy
  → Mod 纯函数 render_from_config(prefix + text_template)      (HTTP 线程, 无 IO)
  → supervisor.say(text)  try_send                              (与聊天同一主链)
壳内「测试注入」→ POST /mods/external-input/command         **L registry Mutex**
  → dispatch_event? 否 → Mod command → 同一渲染纯函数 → say_tx  (无 token, Origin 保护)
```

### 1.3 链路 C：voice-input

```text
主路径（浏览器常驻听）：
[Flutter] VoiceListenController（voice/voice_listen_controller.dart，纯 Dart 可注入）
  → Web Speech 持续识别（speech_recognizer_web.dart）
  → 本地只判「有没有叫角色 + 后面有没有正文」→ 命中唤醒词进「已唤醒」窗口（wakeWindow）
  → POST /api/v1/voice/transcript {「唤醒词+正文」原文, token?}   **HTTP**
  → voice_routes: gate(enabled) → token → Mod gate::evaluate
      manual 闸 → 总闸(wake_phrase 空=关) → 唤醒词命中? → 剥短语
  → clean_transcript + normalize_for_locale → say_tx.say        (主链)
备路径（文件/一键 sidecar）：
[面板] → POST /mods/voice-input/command "run_sidecar"          **L registry + runtime 槽锁**
  → fs::metadata(audio) + Command::spawn(python sidecar…)       **IO/进程, 在 web_api 线程**
  → 后台 wait 线程收 stderr(全量) + exit code；sidecar 再 POST /voice/transcript 回流
```

> 注意：`listen_port` 是**提示值**，真实监听端口来自 `live2d-ai.toml` 的 `[web].port`（默认 18080），
> 本 Mod 不开 socket（`external-input/lib.rs:35`）。`voice-input` 同理，ASR 在浏览器或 sidecar。

### 1.4 链路 D：persona

```text
壳内导入（会话）→ command "import_card" {session_id, card_json|card_path}
  → 解析(JSON / PNG chara) → 合成 system_prompt
  → SessionArchive::insert（tmp+rename 落盘）                    **IO**
  → session_prompts.set_owned(owner=persona, session, composed)
  → 下一轮 prompt_for 组合（匿名 → persona → memory）
壳内导入（全局）→ write_imported_card → apply_settings(写 toml + supervisor.reload)  **IO + reload**
启动 start() → 读 card_json/card_path → apply_card；载入档案 restore() 灌回会话槽
停用 shutdown() → clear_owner(persona) + 还原全局基线
```

**要点**：persona 在**每轮热路径上零开销**（只写一次槽，之后每轮只是表读取）。
这是五条链里最健康的一条。

### 1.5 链路 E：director（预设表演）

```text
TurnPrompt → derive(text)(纯函数, 词表扫描) → ledger（容量 20）→ state_json.latest.preset_id
TurnEnded  → ledger 结项（closed=true）
[Flutter] 每轮第一个 text_delta/text_fallback → GET /api/v1/mods/director/state  **HTTP ×1/轮**
  → seq 去重 → preset_id != none → live2d_bridge 'preset'
  → [iframe] l2d-wasm-demo preset.rs → 按表写 ParamMouthForm/ParamEye*/ParamBrow*/ParamAngle*
```

**要点**：crate 侧零下行通道调用、零网络、纯函数；唯一跨层成本是**每轮一次本地 HTTP**
（可改为随 WS 状态帧携带，省一次往返）。

---

## 2. 热路径量化与速度优化清单

### 2.1 每轮 / 每 token 的真实开销

| 时点 | 发生的事（file:line） | 量级 |
|---|---|---|
| **每 token** | `emit` 先 `ev.clone()`（AppEvent 分配）`supervisor.rs:445` → 投影再 `text.clone()` `:506` → **锁 mod_registry** + `payload.to_string()` + `try_send` `mod_registry.rs:597,635` → worker 对 5 个 runtime 各 `try_lock` `:659` → **4 条 info 日志、回显 token 原文**（memory `:505-509`、persona `:852-856`、director `:325-330`、external-input `:229-236`） | **≈6 次分配 + 1 把全局锁 + 5 把 runtime 锁 + 4 行日志** |
| **每轮开轮** | TurnStarted + TurnPrompt（同步等回执）+ TurnEnded | 3 次投递；flush **持 registry 锁 ≤1000ms**（`mod_registry.rs:633-642, 573-586`） |
| **每轮（memory 启用）** | `append_capped` = append + **全量 `load(0)`**（`store.rs:96-117`）；`retrieve_with` = **再一次全量 `load(max)`**（`:269`）；两次读的是**同一批记录**（文件已 cap 到 max_records）→ 第二次纯冗余；`rank_top_k` 逐条 `tokenize` + 每次重建 query 集合（`strategy.rs:216-242`） | **2 次整库读 + 2×N 行 JSON 解析 + ~N 次分词**；≈40–80 KB 读 |
| **每轮（memory 无会话）** | 额外 `apply_settings` → `read_settings` **每次重新读 toml + 解析 + JSON 序列化**（`cli_entry.rs:256-266`，无缓存；memory `lib.rs:326-335` 每轮调）→ 写 toml + rename + `reload()`（又重建 client） | **+1 次 toml 全量读 + 1 次写盘** |
| **每轮（director）** | `derive` 对 ~80 个关键词各扫一遍 ≤2000 字正文（`decision.rs:325, 202`） | ~80×N 字符扫描（纯 CPU） |
| **每轮 HTTP** | 前端 1 次 `GET /mods/director/state`（`main.dart:350-376`）；无其他 Mod 轮询定时器 | HTTP ×1/轮 |
| **每次 Mod 管理 POST** | `Box::leak(id)` 泄漏一个堆 String（`mods_routes.rs:162`）；enable/disable/config **全程持 registry 锁做磁盘 IO**（`:163` → `persist_manifest` `mod_registry.rs:245-282`），此时每 token 的 emit 都被这把锁挡住 | 锁内 IO |

> **结论**：默认参数（`max_records=200`）下不影响体感；**速度风险集中在三处**：
> ① **每 token 4 行日志**；② **memory 每轮两次全量读**（第二次纯冗余）；
> ③ **registry 锁跨 IO**（flush 与管理面互相阻塞）。三者与规模无关，**每一轮都在付**。

### 2.2 速度优化清单（按 收益/成本 排序）

| # | 问题（file:line） | 现开销 | 改法 | 收益 | 量 |
|---|---|---|---|---|---|
| **P-1** | 4 个 Mod 对每个 TextDelta 逐条 `info` 且回显 payload（memory `:505-509`、persona `:852-856`、director `:325-330`、external-input `:229-236`） | 每 token 4 次 `format!` + 落盘（200 字回复按 chunk 估 50–400 delta ⇒ 200–1600 行日志/轮） | 改 `trace!` 或只记 topic + 长度 | **大** | S |
| **P-2** | memory 每轮两次全量读**读同一批记录**（`store.rs:96-117, 269`） | 冗余的一次整库读 + 解析 | `append_capped` 把已载入 `records` 一并返回，`retrieve_with` 复用 | **大**（省 ~50% IO） | S |
| **P-3** | 订阅表只登记不路由（`mod_registry.rs:707-712`），worker 无差别扇出 5 个 Mod（`:657-669`） | 每 token 5 把 runtime 锁；无人订阅的话题也进 channel/分配 | 建 `topic → [id]` 表；无订阅者时 sink 直接 return | **大**（5 锁 → 1–2 锁） | M |
| **P-4** | `load(max)` 读完再 `drain` 旧行（`store.rs:198-230`），窗口不省 IO | N 行读 + N 次 JSON 解析 | `read_tail(path, n, max_bytes)`：反向 seek、丢半行、取末 n 行（实测 11.4ms → 0.23ms / 10k 行） | **大**（O(N)→O(k)） | M |
| **P-5** | `TurnPrompt` flush 持 registry 锁 ≤1s（`mod_registry.rs:633-642`） | 开轮期间所有 mods 路由/启停/每 token emit 被阻塞 | flush 不持锁（先 clone `event_tx`）+ 超时 warn；memory 写入移出自己的 worker | 中-大 | M |
| **P-6** | `rank_top_k` 每条重建 query 集合 + 重复分词（`strategy.rs:200-242`） | O(N·L) | query 集合只算一次；按 id 缓存分词 | 中 | S |
| **P-7** | `read_settings` 每次全量读 toml + JSON 序列化（`cli_entry.rs:256-266`），memory 无会话时每轮调 | 每轮 1 次 toml 读+解析+序列化 | 复用 `StatusContext` 内存快照 | 中 | M |
| **P-8** | registry 锁跨 IO：`mods_routes.rs:163` 持锁调 enable/disable/config → 读盘/写盘/reload | 「管理面一点、流式就卡」 | 锁内只改内存状态，锁外写 manifest | 中 | M |
| **P-9** | memory 无会话路径每轮 `apply_settings`（toml 写+rename+reload，`lib.rs:464-480`） | 每轮写盘 + 重建 client | 给「无会话」一个固定匿名槽，消灭每轮写盘 | 中 | M |
| **P-10** | director 对 ~80 词各扫一遍正文（`decision.rs:325`） | ~80×N | 单趟扫描 + 匹配表（或 Aho-Corasick） | 中 | M |
| **P-11** | 前端每个 text_delta 都调 `_applyDirectorPreset`（`main.dart:329-331`），且有「慢 GET 跨轮复位」竞态（`:334, :374`） | 每 token 一次调用（被 bool 挡成每轮 1 次 HTTP）+ 偶发漏表演 | 首个 delta 置位后不再调用；修跨轮竞态 | 小 | S |
| **P-12** | `Box::leak(id)`（`mods_routes.rs:162`） | 每次带 id 的管理请求永久泄漏 | registry 接受 `&str`（`is_enabled` 已是先例） | 小 | S |
| **P-13** | memory `state_json` 全量读盘（`lib.rs:601-623`） | 面板每次展开读整库 | 回缓存计数 | 小 | S |
| **P-14** | 命令/状态面在 web_api 线程做 IO/spawn（`mods_routes.rs:210`、voice `commands.rs:266,292`） | 一次慢 IO 卡整个 HTTP 循环 | 命令移 worker/后台任务 | 中（稳定性） | M |
| **P-15** | `dropped_events` 恒 0、丢事件无日志（`mod_registry.rs:589-599`） | 队列满时 TextDelta 可能挤掉 TurnEnded（256 容量、FIFO、无背压） | 丢弃计数上报；轮末事件走「必达」通道 | 中（正确性） | M |

### 2.3 实测（Python 微基准，**量级参考**）

10 000 条、每条约 30 字的 JSONL（1.4 MiB），复刻当前读法（Python；Rust 会更快，比例关系保持）：

```text
整库 load+parse（当前 load(0) 与 load(max) 各一次） : 11.4 ms  → 每轮 ×2 ≈ 22.8 ms
readlines()[-200:]（仍是全读，只是取尾部）           :  1.26 ms
反向 seek 尾部 200 行（P-4 的改法）                 :  0.23 ms   （≈ 50× 于整库读）
```

默认 `max_records=200` 时都不构成体感问题；但一旦调到几千/一万，**每轮 20ms+ 的纯读盘**
就发生在**持 registry 锁的开轮路径**上。P-2 直接省掉其中一半，P-4 把整项降到 O(k)。

---

## 3. 可读性评估

### 3.1 体积 / 注释 / 结构（实测，prod = 首个 `#[cfg(test)]` 之前）

| 文件 | 总行 / prod | 注释% | 最深嵌套 | 最长函数 |
|---|---|---|---|---|
| `mod_registry.rs` | 1318 / **721** | 29% | 7 | `make_services` 61 行 |
| `web_api/mods_routes.rs` | 958 / 500 | 19% | 7 | **`handle_mods_route` 266 行**（71–336） |
| `supervisor.rs` | 767 / 289 | **64%** | 3 | `run_forever` 171 行 |
| `persona/lib.rs` | 1005 / **998** | 29% | 7 | `persona_settings_spec` 55 |
| `external-input/lib.rs` | 832 / 362 | 33% | 4 | `external_input_settings_spec` 34 |
| `voice-input/lib.rs` | 408 / 404 | 36% | 4 | `voice_input_settings_spec` 71 |
| `memory/lib.rs` | 667 / 652 | 39% | 5 | `remember` 35 |
| `director/lib.rs` | 421 / 418 | 37% | 6 | `on_event` 53 |

**规范缺口**：AGENTS 要求源码 ≤500 行、>500 需头注豁免。`persona/lib.rs`（prod 998）、
`mod_registry.rs`（prod 721）、`mods_routes.rs`（958）**都没有行数豁免头注**；memory 有（`lib.rs:5-10`）。

### 3.2 读起来最费劲的 5 处（及具体改法）

1. **`mods_routes.rs:71-336 handle_mods_route`（一个函数 266 行）**：安全校验 + 4 个 action + 两次加锁交错。
   **改法**：拆 `handle_command_action` / `handle_config_action` / `handle_lifecycle_action` +
   `parse_id_action(path)`，顺手去掉 `Box::leak`。
2. **`mod_registry.rs:302-460` 的 6 个 `#[rustfmt::skip]` 函数**：单文件 13 处 `#[rustfmt::skip]`
   （`:302,350,413,423,454,651,679` …），把 `start_one`/`make_services`/`enable`/`disable` 压成
   一行链式 if-let——正好绕过「读起来有结构」那层。**改法**：删 `skip` 让 rustfmt 排版；
   四处 `e.status = Failed; e.last_error = …` 抽成 `mark_failed(&mut self, id, msg)`。
3. **`supervisor.rs:590-681`（say 分支 92 行干 6 件事）**：**改法**：抽 `begin_turn()` /
   `finish_turn()`；把 611–635 的 25 行历史注释压成一行「同轮生效：TurnPrompt 必须早于读表」。
4. **`persona/lib.rs:242-540`**：parse_bytes / 大小闸 / PNG chunk 遍历 / CardSource / AppliedPersona
   挤成一团。**改法**：拆 `card.rs` + `png.rs` + `runtime.rs`。
5. **`memory/lib.rs:391-481 handle_turn_prompt`**：会话/全局两套注入语义复述 3 遍，与头注 16–98 行重复；
   且 **`lib.rs:51` 注释是错的**（与 `:52` 及实现矛盾）。**改法**：函数体只留「语义见 `//!` 头注」，删错误注释。

### 3.3 其他可读性问题

- **注释解释「历史」而非「意图」**：`supervisor.rs:611-620`、`mod_registry.rs:564-571`、
  `memory/lib.rs:34-37`、`chat_panel.dart:28-29`。建议编年史移进 `docs/releases/`。
- **重复实现**：`store.rs:155-175` 的 `update_text`/`delete_by_id` 生产代码**零调用**，
  而 `commands.rs:127-190` 又抄了一遍 `load(0)→改→write_all`；persona/memory 各一份同构 `sessions.rs`。
- **做得好的（保留）**：模块头注回答「为什么 / 不做什么 / 边界」；纯函数与 IO 分离
  （`gate.rs`/`preset.rs`/`strategy.rs`/`sidecar.rs` 可原生单测）；owner/bucket/slot 词汇跨文件一致。

## 4. Mem0 调研 + 单会话记忆系统设计

### 4.1 Mem0 关键事实（一手文档 + 论文）

- 📌 **许可 / 语言**：Apache-2.0，约 65k stars；只有 Python / Node 实现，强依赖
  LLM + embedder + 向量库，**Rust 无 1:1 等价物**（同类的 ai-memory / mcpmem / rustmem
  等都是小项目，不建议直接依赖）。
- 📌 **经典管线**（论文 arXiv:2504.19413）：抽取＝LLM 把新对话压成候选事实；
  更新＝每条候选取 top-10 向量相似旧记忆，交给 LLM function-call 自选
  **ADD / UPDATE / DELETE / NOOP**。
- 📌 **现行 OSS v3 = 单遍 ADD-only**：`add()` 只做 1 次 LLM 抽取，不再 UPDATE/DELETE，
  关联靠 `linked_memory_ids`；迁移文口径 LoCoMo 71.4→91.6、LongMemEval 67.8→93.4，
  抽取延迟约减半。
- 📌 **`add()` 的真实开销**：`infer=True` 时 = 取同 scope 近 10 条 → embed 新消息 →
  向量搜 top-10 → **1 次 LLM 抽取** → 逐条 embed → md5 去重 → 批量写库 → 写 history；
  **`infer=False` 跳过 LLM、原样存（仍 embed）**——这是延迟敏感场景的逃生门。
  Platform 的 `/v3/memories/add/` 更是**默认异步**（返回 `event_id` 轮询）。
- 📌 **延迟（论文 Table 2，LOCOMO，秒）**：Full-context 26031 token、
  **p50 9.87 / p95 17.1**；Mem0 1764 token、search p95 0.20、**总 p95 1.44**；
  Mem0g（图）3616 token、总 p95 2.59。相对 full-context **p95 −91%、token −>90%**。
- 📌 **作用域**：OSS 只有 `user_id` / `agent_id` / `run_id`（**没有 `session_id`**），
  `run_id` 就是「会话 / 任务 / 线程」；`app_id` 仅 Platform。
  **单会话隔离 = `run_id=<session>`，原生支持**；search/get_all/delete_all 至少要给一个 id。
- 📌 **存储**：OSS 缺省 = 本地 Qdrant + history SQLite + `text-embedding-3-small`；
  可换 30+ 种向量库（pgvector/chroma/…）。**OSS v3 已把 Graph memory 删掉**
  （`enable_graph`/`graph_store`/Neo4j 约 4000 行移除），图只剩 Platform always-on。
- 📌 **省延迟三件套**：`infer=False`、异步 API、rerank 默认关（`threshold=0.1`）。
- 🧠 **对实时对话的含义**：无论新旧算法，`infer=True` 的 `add()` 每轮至少 1 次 LLM
  ⇒ **绝不能放在回合关键路径**；本项目 `TurnPrompt` 同步投递上限 1s，光抽取就能吃满。

### 4.2 记忆类项目对照（调研摘要）

| 项目 | 记忆表示 | 写入开销 | 检索 | 作用域 | 许可 | 备注 |
|---|---|---|---|---|---|---|
| **Mem0** | 事实＋向量 | 每轮 1 次 LLM（p95 ~1.4s）；`infer=False` 可免 | 语义+BM25+实体 | user/agent/**run** | Apache-2.0 | v3 改 ADD-only；OSS 已无图 |
| **Letta / MemGPT** | 可自编辑 memory blocks + recall/archival | 工具调用改块；**sleep-time 后台整理** | 常驻块＋向量 | agent（跨会话） | Apache-2.0 | 「后台离线整理」形态值得抄 |
| **Zep / Graphiti** | 时序知识图（valid_at/invalid_at） | 每 episode 1 次 LLM 抽实体/边 | 语义+BM25+图遍历 | group_id | Apache-2.0 | 价值在**事实随时间变更**，单会话用不上 |
| **LangMem / LangGraph** | 摘要＋profile（严格 schema）/collection | hot path 或 **background manager** | 向量/自建 | namespace | MIT | 官方自陈：抽取过多降精度、过少降召回 |
| **A-MEM** | Zettelkasten 笔记＋链接＋向量 | 每条 ≥1 次 LLM+embedding | 向量＋链接近邻 | 进程内 | MIT | 2025-12 后停更 |
| **MemoRAG** | 7B 记忆模型压缩长文 | 一次性编码（需 GPU） | 线索→检索 | 数据集 | Apache-2.0 | 端侧常驻不现实 |
| **cognee** | 关系+向量+图三存储 | pipeline 批量 | 图+向量 | dataset | Apache-2.0 | 重 |
| **basic-memory** | **Markdown 即真相**＋SQLite 索引 | **无每轮 LLM** | 文本+向量混合 | project | **AGPL-3.0** | 「文件是真相、索引可重建」最优雅；但 AGPL 不能链进 Rust |
| **engram (HBarefoot)** | 事实＋向量（SQLite＋内置 embedding） | LLM 蒸馏 | BM25+向量 | agent | MIT | 进程内/离线，最本地友好 |
| **SQLite FTS5** | 原文索引 | 0（无 LLM） | BM25/前缀/NEAR/trigram | 库 | public domain | 中文用 trigram；**零新依赖** |
| **sqlite-vec** | 向量表 | 0 | 向量近邻 | 库 | Apache-2.0 | 官方 Rust crate、可静态链；**pre-v1 会破坏性变更** |

📌 **一条关键外部证据**：AgentIR（arXiv:2605.25092）实测 LoCoMo 上 **BM25 单路即最强单系统**，
级联可 100% 跳过 dense 通道（快 132×）；LongMemEval 跳过 63% 而精度持平。
Mem0 也把 BM25 作为多路信号之一，Graphiti 以「不依赖 LLM 摘要」为卖点。

🧠 **「一轮摘要 + 关键词检索」2026 年够不够**：**单会话、单人、几千条以内 → 完全够**（注入数毫秒）。
阈值大致：< ~2k 条 摘要+FTS5 足够；2k–50k 条开始漏同义改写，加 embedding 做**混合**（保留 BM25）；
> 50k 条或跨会话/多人 才必须向量索引；**只有**「事实随时间变更/矛盾消解」才需要图。
瓶颈不是「够不够先进」，而是「有没有跨会话」——本项目单会话正好在朴素方案的甜点区。

### 4.3 单会话记忆系统设计（推荐）

**核心洞察**：注入本来就是「**下一轮**才被 LLM 看到」，因此**回合内根本不需要同步做完
append+检索+注入**。把整条读写链路移进 Mod 自己的 worker 线程，`on_event` 只做一次
`try_send`，就同时解决了速度（不再持 registry 锁/不再全量读）与可读性
（删掉为「同轮生效」存在的一堆时序说明与降级分支）。

#### 数据模型

```json
{"id":"...","text":"...","ts":123,"turn":45,"kind":"raw|fact"}
```

- 单条 `text` ≤ 1 KiB（截断 + 标记），整库 ≤ 2 MiB，条数沿用 `max_records`；
- P0 只落 id/text/ts/turn/kind；`score` 只在内存；`embedding` 到 P2 才落盘（可重建的缓存）；
- 单会话路径：`<config_dir>/memory/<session_id>.jsonl`；会话 id 收紧字符集
  （禁 `:`、禁 `..`、禁点开头）。

#### 链路（全部移出主链）

```text
[TurnPrompt]  on_event: try_send(Job::Prompt{text, session}) → 立即返回
                     └─[memory worker] append_line(O(1))
                                       cache.sync()（只读新增字节）
                                       rank（内存，<1ms/200 条）
                                       build_patch(owner=memory) → set 会话槽
[TurnEnded]   try_send(Job::End)      →（可选）计数 / 空闲合并
[启动]        read_tail(n, max_bytes)：seek 到 len-budget、丢半行、取末 n 行
[面板 state]  直接回缓存计数（不读盘）
```

#### 延迟预算（主链 0 阻塞）

| 阶段 | 分值 | 额外延迟 | 降级 |
|---|---|---|---|
| P0 | Jaccard（bigram + ASCII） | < 1 ms / 200 条 | 无网络、无 |
| P1 | + 时间衰减 + 近重复合并 | 同量级 | 无 |
| P2 | α·cos + (1−α)·jaccard（`/embeddings`） | 本机 30–200 ms，远端 100–600 ms | 超 150 ms 或报错 → 本轮只用 Jaccard，向量异步补 |

失败链固定：**embedding → Jaccard → 空结果 no-op**（绝不注入坏块）。

#### 存储选型：P0/P1 **不引入 SQLite**

JSONL 正好匹配这个场景：追加 O(1)、尾部读、坏行局部化、纯文本可调、已有 tmp+rename 原子纪律、
零新依赖。`rusqlite` 的索引/事务/并发/FTS5 在「单进程 / ≤1 万条 / 单会话」下用不上，
代价是 C 依赖 + 构建时间 + 失去可读性。**迁移触发条件**：跨会话共享库 / 多进程写 / 需要 FTS 时，
再上 `rusqlite`（bundled）+ FTS5（中文 trigram）。向量同理——只在 >2k 条或明显漏同义改写时上
`sqlite-vec`，且把向量表当**可重建缓存**。

#### 抄什么 / 不抄什么

| | 内容 | 理由 |
|---|---|---|
| ✅ 抄 | **写读分离**：写侧后台、读侧多信号（Mem0/Graphiti 共识） | 保证回合延迟 |
| ✅ 抄 | **单遍 ADD-only** 的简化思想（降级成本地规则，不调 LLM） | 少一次 LLM、少一类竞态 |
| ✅ 抄 | **内容 hash + 近重复去重**（Jaccard ≥ 0.85 跳过/就地替换） | 廉价且可解释 |
| ✅ 抄 | **`infer=False` 逃生门**：P0 原样存，不抽取 | 与 Mem0 同一条延迟取舍 |
| ✅ 抄 | **history 式审计**（update/delete 留痕）+ 显式 clear | 面板要能如实说「这条哪来的」 |
| ✅ 抄 | **原文是真相、索引可重建** | 坏索引可重算，不怕迁移 |
| ❌ 不抄 | 每轮 LLM 判 ADD/UPDATE/DELETE/NOOP | 延迟/成本/依赖三杀；单会话短史用不上 |
| ❌ 不抄 | Graph / decay / dream / 多租户 | OSS 自己都在 v3 删了图；单会话无事实时效问题 |
| ❌ 不抄 | 以向量库为唯一存储 | 引入常驻服务，违背「少依赖、读得懂」 |

#### 模块布局与关键签名（可直接照写）

```text
crates/live2d-ai-mod-memory/src/
  lib.rs      生命周期；on_event 只入队
  config.rs   MemoryConfig / settings_spec（+ max_line_chars / file_bytes / embed_*）
  record.rs   MemoryRecord { id, text, ts, turn, kind }
  store.rs    JsonlStore { append_line, read_tail, append_capped, clear }
  cache.rs    SessionCache { sync, records, tokens }
  retrieve.rs tokenize / overlap / rank_top_k / hybrid_rank
  embed.rs    EmbeddingsClient { embed_batch }
  inject.rs   build_block / strip_block（marker 转义）
  worker.rs   spawn_worker(rx)  ← 全部 IO / 检索 / 注入
```

```rust
pub fn append_line(&self, r: &MemoryRecord) -> std::io::Result<()>;
pub fn read_tail(&self, n: usize, max_bytes: usize) -> std::io::Result<LoadOutcome>;
impl SessionCache {
    pub fn open(p: &Path, c: &MemoryConfig) -> std::io::Result<Self>;
    pub fn sync(&mut self) -> std::io::Result<usize>;   // 只读新增字节
}
pub fn hybrid_rank(q: &str, qv: Option<&[f32]>, c: &SessionCache, cfg: &MemoryConfig) -> Vec<Hit>;
pub fn build_patch(hits: &[MemoryRecord]) -> Option<serde_json::Value>;  // owner=memory
pub fn spawn_worker(rx: Receiver<Job>, svc: ModServices) -> std::thread::JoinHandle<()>;
```

#### 分阶段实施

| 阶段 | 内容 | 改动量 | 收益 |
|---|---|---|---|
| **P0** | `on_event` 只入队 + worker；`read_tail` + `SessionCache` 消全量读；单条/文件字节闸；marker 转义；`state_json` 读缓存；会话级路径 + owner 槽 | M（半天–1 天） | 修 P-3/P-4/P-5；每轮 ≈0 读盘、主链 0 阻塞 |
| **P1** | 时间衰减、近重复合并、可选「离线合并」（旧事实标 `superseded_by`，默认关） | M（1–2 天） | 记忆质量上一台阶，仍不碰回合延迟 |
| **P2** | `/embeddings` + 向量落盘 + 混合打分 + 超时降级；可选 rusqlite + FTS5 | L（按需） | 大规模/同义改写召回 |

> **来源**：[Mem0](https://github.com/mem0ai/mem0) ·
> [Mem0 论文 arXiv:2504.19413](https://arxiv.org/abs/2504.19413) ·
> [Mem0 Add Memories（V3 异步 / infer=false）](https://github.com/mem0ai/mem0/blob/main/docs/api-reference/memory/add-memories.mdx) ·
> [Mem0 Entity-Scoped Memory（run_id）](https://github.com/mem0ai/mem0/blob/main/docs/platform/features/entity-scoped-memory.mdx) ·
> [Letta sleep-time compute](https://www.letta.com/blog/sleep-time-compute) ·
> [MemGPT arXiv:2310.08560](https://arxiv.org/abs/2310.08560) ·
> [Zep arXiv:2501.13956](https://arxiv.org/abs/2501.13956) · [Graphiti](https://github.com/getzep/graphiti) ·
> [LangMem 概念指南](https://github.com/langchain-ai/langmem/blob/main/docs/docs/concepts/conceptual_guide.md) ·
> [A-MEM](https://github.com/agiresearch/A-mem) · [MemoRAG](https://github.com/qhjqhj00/MemoRAG) ·
> [cognee](https://github.com/topoteretes/cognee) · [basic-memory](https://github.com/basicmachines-co/basic-memory) ·
> [sqlite-vec](https://github.com/asg017/sqlite-vec) · [SQLite FTS5](https://www.sqlite.org/fts5.html) ·
> [AgentIR arXiv:2605.25092](https://arxiv.org/abs/2605.25092)

## 5. 各 Mod 同类项目调研（实现链路对照）

### 5.1 external-input（弹幕/礼物 → 清洗 → 注入）

| 同类项目 | 语言 / 许可 | 关键做法 | 可借鉴 |
|---|---|---|---|
| **blivedm** | Python / MIT | WS 事件表 `DANMU_MSG` / `SEND_GIFT(_V2)` / `GUARD_BUY` / `SUPER_CHAT_MESSAGE` / `INTERACT_WORD_V2`，回调分发 | 事件表 + 未知 cmd 显式计数 |
| **bilibili-api-python** | Python | 反向 REST+WS 全覆盖；**已因 B 站侵权告知函停维护并关停** | 反向协议的法律风险——别进主仓 |
| **bilibili-live-ws** | JS / MIT | 轻量弹幕 WS 客户端 | 心跳 / 重连 |
| **BiliBili-AI-Danmaku** | Electron+Python | 关键词/正则 → 固定回复优先 → AI 队列（待发/已发/跳过）+ 发送间隔 + 队列上限 | **优先级队列 + 节流** |
| **神奇弹幕 MagicalDanmaku** | JS | 弹幕姬 + 答谢 + 回复 + 点歌，可编程 | 插件化 |
| **social_stream** | JS | 多平台 chat 聚合 overlay，消息排队/精选 | 去重与排队 |

**现状链路**（`external-input` + `external_routes.rs` + `docs/examples/bilibili-sidecar`）：
Mod 只有 settings_spec v2（`listen_port`/`token`/`text_template`/`prefix`）+ 纯函数渲染 +
进程级计数 + `test_inject`/`reset_counters`，**不含任何 B 站协议**；sidecar（blivedm）只处理
`DANMU_MSG`+`SEND_GIFT`，`clean_text` 去控制符/压空白/截 500，Throttle 全局最小间隔 1s 后
**直接丢弃**，`SEND_GIFT_V2` 未识别则上报 `v2_ignored`。
**缺口**：无事件去重、无礼物聚合、无 SC/舰队优先、无热度排序、无重连退避。
**改进**：① sidecar 加 `(uid, content)` 滑动窗去重 + 礼物聚合（同 uid 同 gift 3s 内累加 `num` 后只注入一条）；
② 全局 1s 硬丢改为**有界优先级队列**（`SUPER_CHAT`/`GUARD_BUY` 插队，普通弹幕按 uid 复现与长度降权），
保留 200+`ok:false` 的退避语义；③ **重连不要自己写**——blivedm 的 WS 客户端已内建心跳（默认 30s）
与重连循环（默认策略是固定 1s），正确动作是**配置退避**：
`client.set_reconnect_policy(blivedm.utils.make_linear_retry_policy(1, 2, 30))`（或自定义指数 + jitter），
并把 `retry_count` 与 `v2_ignored` 同款自报进 `state_json`（界面能看见「正在重连 / 已重连 N 次」）；
心跳保持默认，不要自行加（过密会被限流、过疏会被判掉线）。

### 5.2 voice-input（转写 → 清洗 → 唤醒闸 → 注入）

| 同类项目 | 语言 / 许可 | 关键做法 | 可借鉴 |
|---|---|---|---|
| **sherpa-onnx** | C++/Py/**Rust** / Apache-2.0 | 流式/离线 ASR、VAD、KWS、标点，**有 Rust 绑定** | 可免 Python sidecar |
| **FunASR** | Python / MIT | Paraformer 实时中文 | 中文强 |
| **whisper.cpp** | C/C++ / MIT | 本地整段 + 量化 | 离线简单 |
| **faster-whisper** | Python / MIT | CTranslate2，比 openai/whisper 快至 4× | 整段 + VAD |
| **RealtimeSTT** | Python / MIT | VAD+唤醒词+流式；草稿+终稿两段式 | 实时草稿 + 权威终稿 |
| **Vosk** | C++/Py/**Rust** / Apache-2.0 | 50MB 小模型、零延迟流式 | 低配端侧 |
| **Silero VAD** | Python/ONNX / MIT | 轻量端点检测 | 切句 |
| **openWakeWord** | Python / Apache-2.0 | 开源唤醒词、可自训、**无需密钥** | 本地唤醒首选 |
| **Porcupine** | 多语言 / 代码 Apache-2.0 + 商用条款 | 高精度 KWS；**运行必须 AccessKey**，个人模型限教育/研究/爱好且 30 天过期 | 对外分发选 openWakeWord |

📌 **可照抄的参数**：Silero VAD 默认 `threshold=0.5` / `min_speech_duration_ms=250` /
`min_silence_duration_ms=100` / `speech_pad_ms=30`；sherpa 端点三规则默认
「静音 2.4s 无条件结束 / 解码出内容后 1.2s 结束 / 话语 20s 上限」。
📌 **延迟量级**：faster-whisper 13min 音频 CPU int8 1m42s、GPU large-v2 1m03s（RTF≈0.08–0.13）；
whisper.cpp base 编码 220ms / small 685ms（M1 Pro 8 线程）；FunASR Paraformer CPU RTF 0.0976(fp32)/0.0484(int8)，流式 chunk≈600ms。

**现状链路**：`POST /api/v1/voice/transcript`（推模式，Rust `opens_network=false`）→ 纯函数
`gate::evaluate`（manual→总闸→含唤醒词→剥词）→ `clean_transcript` + locale 归一化 → `say_tx`；
一键 sidecar 用 `Command` 逐参 spawn + 后台 wait。
**关键缺口**：**唤醒词是「对已转写文本的子串匹配」，没有音频级 VAD/端点，也没有音频唤醒词**；
ASR 是整段文件一次性、非流式；sidecar 一次性、无重连。
**改进**：① **唤醒前移**——sidecar 用 openWakeWord 做音频级唤醒，命中后才启动 ASR 与推送，
Rust 文本闸保留为二道防线（兼容 fake/mock）；② 用 Silero VAD 做端点切句，并评估 **sherpa-onnx 的
Rust 绑定**（其 VAD/KWS 可在不违反「Rust 不开 socket」的前提下做成可选本地能力）；
③ 退避可观察：三类失败分开（加载失败=指数+jitter+降级小模型 / 设备占用=退避重试并提示 /
识别超时=丢弃本轮+重置 VAD），把计数与最近错误写进 `state_json`；若保留 Python 通道，
IPC 用 JSON Lines（`-u`/PYTHONUNBUFFERED，行长上限）避免块缓冲。

### 5.3 persona（角色卡 → system_prompt）

| 同类项目 | 语言 / 许可 | 关键做法 | 可借鉴 |
|---|---|---|---|
| **Character Card V2 规范** | 文档 | V1 六字段包进 `data`，新增 `creator_notes`/`system_prompt`/`post_history_instructions`/`alternate_greetings`/`character_book` | 字段→槽位成文契约 |
| **Character Card V3 规范** | 文档 / MIT | V2 超集：`nickname`/`assets`/`group_only_greetings`；Lorebook 增 `use_regex` | 版本探测、保留未知字段 |
| **SillyTavern** | JS / AGPL-3.0 | Prompt Manager 固定槽位；世界书 before/after/@depth；PNG 只扫 tEXt，ccv3 优先 | **槽位顺序与 token 预算** |
| **chub.ai** | TS / 专有 | V2 卡 + Characterbook（scan_depth/token_budget/递归） | 世界书落地 |
| **JanitorAI** | Web / 专有 | 最小字段集、无 lorebook | 无卡兜底 |

**现状链路**：解析 V1 扁平 / V2 嵌套；PNG 只取 tEXt 与未压缩 iTXt 的 `chara`（**无 ccv3**）；
`PersonaCard` 只有 6 字段；`compose_system_prompt` 写死拼接；写回主链唯一入口 `apply_settings`
（整段替换 `persona.system_prompt`）+ 基线快照还原；会话档案 50 上限；与 memory 是 last-writer-wins。
**缺口**：无 `mes_example`/`post_history_instructions`/`character_book`/宏/槽位/token 预算；无 V3/ccv3。
**改进**：① 解析补 V3 与 ccv3（PNG 顺序 ccv3→chara、keyword 小写比较），把新字段读进来（先存后用），
未知字段只读不毁；② 合成从写死拼接改为**槽位表**（system→world before→description→personality→scenario→post_history），
缺省对齐 ST Prompt Manager，并实现极简世界书（constant+关键词+insertion_order+字符预算）；
③ persona 与 memory 的共存改为**标记块 + 槽位**（memory 已用 `live2d-ai:memory-v0` 标记），
persona 保留该块而非整段覆盖，消除 last-writer-wins 冲掉记忆的取舍。

### 5.4 director（情绪/意图 → 表情/短动作预设）

| 同类项目 | 语言 / 许可 | 关键做法 | 可借鉴 |
|---|---|---|---|
| **soullink-emotion-sdk** | TS / MIT | VAD 连续情绪 + FACS/AU + 分层 MotionMixer + ModelProfile 自动适配降级 | 连续情绪、参数 ownership、profile 扫描 |
| **Open-LLM-VTuber** | Python / MIT | LLM 出 `[joy]` 标签 → emotionMap → 表情序号 | 零额外算力 |
| **VTube Studio API** | 文档 / MIT | `HotkeyTriggerRequest` / `ExpressionActivationRequest(fadeTime)` | 淡入淡出、现成通道 |
| **GoEmotions** | Python / Apache-2.0 | 27+neutral 细标签 | 细标签收敛成表情类 |
| **distilroberta-emotion** | Python | 7 类 Ekman，单句概率 | 用概率设阈值 |
| **Chinese-Emotion-Small** | Python / MIT | mDeBERTa 8 类中文 | 中文落点 |
| **Ikaros-521/AI-VTuber** | Python / GPL-3.0 | 弹幕→LLM→Live2D 全链 | 注意 GPL |

**现状链路**：订阅 TurnPrompt/TurnEnded → 纯函数 `derive`（内置中英词表、否定前缀、ASCII 词边界）→
`{emotion(6+neutral), intent(7), suggested_tts, preset_id}` → 硬编码 preset 表 → `state_json.latest.preset_id`
→ 前端拉取 → 渲染面参数。
**缺口**：无置信度、无冷却/限频、无优先级/不可打断、无强度/时长/缓动、无参数级合成。
**改进**：① 预设表**外置**为 TOML/JSON（`params[{Id,Value,Blend}]`、`duration_ms`、`fade`、
`priority`、`cooldown_ms`、`confidence`），默认全表关闭，未知/低置信 → `none` 且不动待机；
② 分级与冷却：规则命中给**弱置信**，可选 sidecar 小模型（Chinese-Emotion-Small，MIT）出概率，
低于阈值或 top1−top2 过小则回落 neutral；优先级 `reaction > speech > emotion > idle`，
进行中动作不可打断；③ 语义升级：把 `preset_id` 从「选一条」扩成**参数层合成**，经版本化 WS 帧
`{version,emotion,variant,intensity,priority,ttl_ms}` 交渲染面，且表情**不得覆盖 `IdleState`**（呼吸/眨眼）。

> **许可提示**：本节表格里 **AGPL/GPL**（SillyTavern、Ikaros-521）只能读不能链进 Rust 二进制；
> MIT/Apache-2.0（blivedm、sherpa-onnx、Vosk、openWakeWord、soullink-emotion-sdk、Chinese-Emotion-Small）
> 才可作为实现参考。



---

> **§5 来源**：
> [blivedm handlers.py](https://github.com/xfgryujk/blivedm/blob/master/blivedm/handlers.py) ·
> [blivedm ws_base.py（心跳/重连）](https://github.com/xfgryujk/blivedm/blob/master/blivedm/clients/ws_base.py) ·
> [blivedm utils.py（retry policy）](https://github.com/xfgryujk/blivedm/blob/master/blivedm/utils.py) ·
> [bilibili-api-python（已关停）](https://github.com/Nemo2011/bilibili-api) ·
> [B 站开放平台协议](https://open-live.bilibili.com/document/657d8e34-f926-a133-16c0-300c1afc6e6b) ·
> [BiliBili-AI-Danmaku](https://github.com/xgxdmx/BiliBili-AI-Danmaku) ·
> [faster-whisper](https://github.com/SYSTRAN/faster-whisper) · [RealtimeSTT](https://github.com/KoljaB/RealtimeSTT) ·
> [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) · [sherpa endpoint.h](https://github.com/k2-fsa/sherpa-onnx/blob/master/sherpa-onnx/csrc/endpoint.h) ·
> [vosk-api](https://github.com/alphacep/vosk-api) · [silero-vad](https://github.com/snakers4/silero-vad/blob/master/src/silero_vad/utils_vad.py) ·
> [openWakeWord](https://github.com/dscripka/openWakeWord) · [Porcupine 许可](https://github.com/Picovoice/porcupine/issues/292) ·
> [whisper.cpp](https://github.com/ggml-org/whisper.cpp/issues/89) · [FunASR](https://github.com/modelscope/FunASR) ·
> [Character Card V2 规范](https://github.com/malfoyslastname/character-card-spec-v2/blob/main/spec_v2.md) ·
> [Character Card V3 规范](https://github.com/kwaroran/character-card-spec-v3/blob/main/SPEC_V3.md) ·
> [SillyTavern Prompt Manager](https://docs.sillytavern.app/usage/prompts/prompt-manager/) ·
> [SillyTavern Tokenizer](https://docs.sillytavern.app/usage/prompts/tokenizer/) ·
> [soullink-emotion-sdk](https://github.com/nanlingyin/soullink-emotion-sdk) ·
> [Live2D 标准参数表](https://docs.live2d.com/4.2/zh-CHS/cubism-editor-manual/standard-parametor-list/) ·
> [CubismSpecs exp3.json](https://github.com/Live2D/CubismSpecs/blob/master/FileFormats/exp3.json.md)

---

## 6. 建议路线图（速度/可读性优先）

### P0 — 半天到一天，全是最小改动、收益最直观

| 项 | 内容 | 量 |
|---|---|---|
| **P-1** | 4 个 Mod 的每 token `info` 改 `trace!`（或只记 topic+长度） | S |
| **P-2** | memory 两次全量读合成一次（`append_capped` 返回 records） | S |
| **P-3** | 事件按订阅路由，无订阅者不进 channel | M |
| **P-11** | 前端首个 delta 触发一次 + 修跨轮复位竞态 | S |
| **P-12** | 去掉 `Box::leak(id)` | S |

### P1 — 一到两天，把「锁 + 全量 IO」从热路径清出去

| 项 | 内容 | 量 |
|---|---|---|
| **P-4** | memory `read_tail` + `SessionCache`（消全量读） | M |
| **P-5** | flush 不持 registry 锁 + 超时 warn | M |
| **P-8** | registry 锁不跨 IO（锁内只改内存） | M |
| **P-7** | `read_settings` 复用内存快照 | M |
| **P-9** | memory 无会话路径走匿名槽，消灭每轮写盘+reload | M |
| **P-6 / P-13** | 检索预计算 + `state_json` 读缓存 | S |

### P2 — 可读性（按「读起来最费劲」顺序）

| 项 | 内容 | 量 |
|---|---|---|
| **R-1** | 拆 `mods_routes.handle_mods_route`（266 行 → 按 action 四个函数） | M |
| **R-2** | `mod_registry`：删 13 处 `#[rustfmt::skip]`、抽 `mark_failed`、拆 worker/manifest/services | M |
| **R-3** | 拆 `persona/lib`（card/png/runtime）；精简 `handle_turn_prompt`；删 `memory/lib.rs:51` 错误注释 | M |
| **R-4** | 给 >500 行的文件补豁免头注或真拆；命令层复用 store 方法（删重复实现） | S |

### P3 — 单会话记忆（§4 设计）

| 阶段 | 内容 | 量 |
|---|---|---|
| P3-1 | `on_event` 只入队 + memory 自己的 worker；`read_tail` + `SessionCache`；字节闸；marker 转义；缓存计数 | M（半天–1 天） |
| P3-2 | `TurnEnded` 后台抽取 + 近重复合并 + 可选摘要层 | M（1–2 天） |
| P3-3 | 可选 `/embeddings` + 向量落盘 + 混合打分与超时降级；可选 rusqlite + FTS5 | L（按需） |

### P4 — 各 Mod 功能（§5 建议，按需）

| 项 | 内容 | 量 |
|---|---|---|
| external | `(uid,content)` 去重 + 礼物聚合 + 有界优先级队列（SC/舰队插队）+ **配置** blivedm 退避 | S/M |
| voice | openWakeWord 音频级唤醒前移 + Silero VAD 端点 + 评估 sherpa-onnx Rust 绑定 | L |
| persona | 补 ccv3/V3 解析 + 槽位表合成 + 极简世界书 + 与 memory 用标记块共存 | M/L |
| director | 预设表外置 + 置信度/冷却/优先级 + 参数层合成（不覆盖 `IdleState`） | M/L |

---

## 7. 盲区与建议的实测补充

- 本报告为**只读静态审查 + Python 量级微基准**；未跑 Flutter `analyze/test`，未在活服务上量真实
  turn 延迟。建议补三个 `tracing` 计时点作为后续基线：① `dispatch_event_and_flush` 实际耗时；
  ② memory `append_capped` / `retrieve_with` 各自耗时；③ 每轮 Mod 日志行数（可直接验证 P-1）。
- 被审查 worktree 处于未提交状态（core-next 三件套），行号基于该快照，提交后可能漂移。
- §5 的同类项目结论来自联网调研（URL 经抓取）；其中 blivedm 的重连行为已按源码勘误为
  「配置退避，而非自行实现」。

