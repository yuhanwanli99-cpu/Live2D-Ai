# CONSOLIDATION-01 — BATCH-0001..0006 首批六批对账

日期: 2026-09-28 · 范围: `crates/live2d-ai-desktop/src/web_api/**` 前 6 批
本文件是对账，不是长文。每条都能回源码复核。

---

## ① 未关闭 P0/P1 回源码复核（不看笔记，从源码重新推导）

四条 P1 全部**仍成立**。复核方式与结论：

**F-0001-01（P1）前置路由四族零日志 —— 仍成立**
重跑 `grep -c 'tracing::' / 'println!|eprintln!'` 四个文件：`chat_routes 0/0`、
`external_routes 0/0`、`voice_routes 0/0`、`mods_routes 0/0`。BATCH-0004/0005 已逐行读完
`chat_routes.rs`(1086) / `external_routes.rs`(1062) / `voice_routes.rs`(498) 的生产段，
**确认这三个文件的每一个错误分支都只写进 HTTP 响应体，无一条落盘**。原判定的范围从
「grep 推断」升级为「逐行确认」——但结论未变，且新增证据：`voice_routes.rs:286-309`
三个闸门拒绝（`voice_manual_off` / `voice_gate_closed` / `wake_phrase_required`）同样零日志，
而这三类恰恰是 sidecar 排障时最需要看到的码。

**F-0001-02（P1）`model_root.rs` 两条测试不可能失败 —— 仍成立**
重读 `model_root.rs:108-120` 原文未变。复核时补了一条更强的反证：把断言里的
`root.join("bai/runtime/bai.model3.json").is_file()` 单独拎出来看——`ls -la assets/models/`
只有 `.gitkeep` 与 `README.md`，`bai/` 不存在，所以**两个析取项在当前工作树上同时为「不成立」与
「真」**，断言靠 `!is_dir()` 那一支过。改成任意 `<x>/assets/models` 仍然全绿。

**F-0002-01（P1）首次配置路径 file_watcher 永久不装 —— 仍成立**
重读 `cli_entry.rs:334`（`if let Some(sup) = &supervisor_opt`）与 `:360-362`（`else { None }`）未变。
补一条本轮才想清楚的因果：`supervisor_opt` 的两个 `None` 分支（`:155-161` 装配失败 /
`:163-166` 无配置文件）都发生在**启动期**，而 `ensure_supervisor_after_patch` 造出 supervisor 发生在
**首个 PATCH**（`supervisor_slot.rs:126-143`）。两者不在同一条控制流上，且全仓 `watch_config`
只有 cli_entry.rs:337 一个调用点 ⇒ 不存在补装路径。这与 BATCH-0004 读到的
`chat_routes` 会话表**有**多条写入路径形成对照：同一仓里「多条路径 ⇒ 状态源一致」做对了
（会话表），而 file_watcher 是「装配点只有一条且条件选错」——**结构不同，后果相同**。

**F-0002-02（P1）`supervisor_slot` 两条测试不可能失败 —— 仍成立**
重读 `supervisor_slot.rs:176-179`，测试体与批记录逐字一致（两条断言、零 `set`）。
毒化测试的自述失效原因（`:193-194`）同样未变。

---

## ② 覆盖率

- 队列总数 483（crates 266 / flutter lib 115 / flutter test 102）；已审 **18**。
- `web_api` 49 个 .rs / 18,915 行；已审 **18**（36.7%），行数 7,255。
- 剩余 web_api 31 个（models_routes 9、settings_routes 3、ws/audio_tests、各 tests_*.rs 等）。
- 队列外已覆盖：`cli_entry.rs` 部分、`mod_registry.rs` 三处片段、`dto.rs` 一处、
  `secrets.rs` 一处、`supervisor.rs` 一处、`session_scope` 未读。
- Phase 1 整体进度：web_api 是 7 个大项里的第 1 项，尚在第 1 项内。

---

## ③ 同根因归并

| 主发现 | 派生 | 归并理由 |
|---|---|---|
| **F-0001-01**（前置端点零日志） | F-0005-01 的「降级不可见」那一半 | 同一个根因：**web_api 里 2026-08-30 之后新增的模块都没接 `dispatch.rs` 的请求日志**。F-0005-01 的技术缺陷（门禁降级）是新的，但「降级为什么没人知道」这一层完全由 F-0001-01 解释。不重复计数，修 F-0001-01 会连带修掉 F-0005-01 的可观测性一半。 |
| **F-0001-02** + **F-0002-02** | （暂无派生） | 同根因：**测试体与被测契约脱节**。一个是析取式断言恒真，一个是测试名承诺的行为没写。合并为一条模式（见 ④），但保留两条独立发现——修法不同、文件不同。 |
| **F-0004-03**（CT/Origin 顺序与状态码两套） | B0005 确认前置族 3/3 内部自洽 | 不合并，但**范围收窄**：已从「两条路径不一致」精确为「两族之间不一致，族内自洽」。已在 F-0004-03 的验证段写明。 |

---

## ④ 跨批系统模式（本批对账最有价值的产出）

### 模式 A：新增模块没接既有的横切关注点（本轮 3 条发现同源）
`dispatch.rs:241-258` 的 `log_request_outcome` 是仓里唯一的请求级日志，2026-09-11 补的。
但它**只挂在 dispatch 末尾**。此后新增的 6 个前置/独立模块（`chat_routes` / `external_routes` /
`voice_routes` / `mods_routes` / `flutter_app` / `wasm_assets`）全部在 dispatch **之前**
`respond` + `continue`。结果：仓库里**越新的端点越不可观测**。这与项目 2026-09-11 那次
「后端出错无具体错误代码、看日志什么都没有」的复盘结论方向完全相反——那次修好了老路径，
新路径没继承。**判据**：`git log` 里 2026-08-30 之后新增的 web_api 模块，一个都没进 dispatch。
→ 建议单独立项（不属任何单条发现）：把「respond + 日志」收口成一个包装函数，让所有返回
`Some(resp)` 的前置路由都必须经过它。

### 模式 B：假绿灯的两种固定写法（各命中一处，都是新写的测试）
1. **析取式断言恒真**：`assert!(A || !B)`，当 B 在目标环境里不成立时，A 是什么都无所谓。
   （`model_root.rs:111`）
2. **测试名/头注承诺的契约没进测试体**：`slot_lifecycle_set_get_take` 承诺 set→try_get→take，
   体里只有构造器语义；`slot_poisoned_does_not_panic` 毒化的是 `RwLock<Option<Arc<()>>>`，
   与被测的 `SupervisorSlot` 不是同一个对象，头注自己承认了。
   （`supervisor_slot.rs:172-204`）
两条的共同点：**注释写得比测试更认真**，所以读代码的人会被注释说服而跳过测试体。
**对照组**：`flutter_app.rs:349-357` `repo_root_skips_the_crate_manifest` 与
`chat_routes.rs:845-917` `stop_returns_to_the_session_baseline`（真 broadcaster + 读真实帧原文 +
`rx.try_recv().is_err()` 断言「不补帧」）是**真测试**。说明团队写得出来，只是没每处都这么写。

### 模式 C：装配条件与运行时事实错开（本轮 1 条，但影响面大）
`file_watcher` 挂在**启动期的局部变量** `supervisor_opt` 上，而 supervisor 的真值在**运行期的槽位**
`supervisor_slot` 里。两者只在「启动即成功」时相等。BATCH-0004 读到的会话表是**反例**：
`handle.session_scopes()` 挂在 `SupervisorHandle` 上，多个写入方（`say_scoped` /
`set_active_session` / host API）都通过它，谁都不会错开。
**判据**：任何「装配某样东西」的条件，若读的是启动期快照而不是运行期槽位，就是这条模式的候选。

### 模式 D：护栏注释与实际装配脱节（F-0001-05 / F-0003-03 / F-0004-02 同族）
`security.rs:94` 说 `check_ws_origin`「当前未在 mod.rs 装配」（实际在 ws.rs:95 装配）；
`mod.rs:350` 说 `with_security`「由 cli_entry 装配时调用」（实际只有测试调）；
`ws.rs:227` 说 426 是「tungstenite::accept 失败」（实际是缺 Sec-WebSocket-Key）；
`chat_routes.rs:52` 说「本文件当前约 560 行」（实际 1086）。
**共同点**：都是**注释描述的过去状态**没有随代码更新。四处里两处（`check_ws_origin` /
`with_security`）还带 `#[allow(dead_code)]`——这让「安全函数失联」不再产生任何编译信号。
这是可维护性 P2/P3，但**频率异常高**（6 批里 4 条），值得单独立一条纪律：改装配点时必须同步改头注。

---

## ⑤ 质量自评（诚实）

- 六批共 **23 条**发现，全部带 `file:line` + 原文摘录。分布：P0 0 / P1 4 / P2 10 / P3 9。
- **有反证构造的 P1/P2：11 条**（其余 P2/P3 为事实陈述型，如行数、状态码口径，不需要反证）。
- **进候选池未进 FINDINGS 的：5 条**（C-1…C-5）。
- **主动推翻自己的假设：4 次**（BATCH-0003 两次：音频帧热路径、慢订阅者泄漏；
  BATCH-0005 两次：voice 空 token、Mod config 缺失）。其中「慢订阅者泄漏」那条如果不下手查
  `mpsc::SyncSender` 的所有权，就会写成一条**看似有据、实则错误的 P1**。
- **已知不足**：
  1. 本轮 6 批全在 `web_api` 一个目录内，**跨目录的接缝一次都没看过**（前端 Dart ↔ Rust 契约、
     runtime ↔ supervisor、Mod crate ↔ host 都未进入）。INDEX 的覆盖面因此是「广度」而非「链路」。
  2. 所有 P1/P2 的**用户可见性**都缺浏览器/门禁实测支撑（本任务禁跑），已在每条的「验证」段如实标注。
  3. 未读 `AVAILABLE_MOD_FACTORIES` 装配面，故 F-0005-01 的「生产不可达」仍是文档采信。
- **下一步优先级判断**（据 ④ 的模式）：
  1. 先做完 `web_api` 剩余 31 个文件，把模式 A/B 在本目录内收口；
  2. 然后**立刻**转 Phase 3 端到端链路（候选 ①发送→LLM→句子→TTS→音频→口型），
     因为模式 C/D 都是单点缺陷，而跨目录接缝才可能出现真正的 P0；
  3. Phase 4 对抗日对本轮 4 条 P1 逐条构造反例（已在本文件 ① 做过一轮，Phase 4 再做一次，
     刻意不看本文件）。
