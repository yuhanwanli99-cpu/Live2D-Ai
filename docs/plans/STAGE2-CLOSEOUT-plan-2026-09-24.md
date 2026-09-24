# 阶段2 收口推进计划（2026-09-24 维护者独立复核版）

> 落盘人：顶层管理与代码审查（2026-09-24）。工作树 = `/home/skystar/Live2D-Ai-l1`
> （git worktree，分支 `mod/l1-product`，HEAD `d2bdd4fd`，**192 条未提交**）。
> **注意**：`/home/skystar/Live2D-Ai` 是另一个 worktree / 另一条分支，本计划不涉及。
>
> 上一版范围冻结：`docs/plans/STAGE2-WAVE2-plan-2026-09-22.md`（含 §8 暂停裁决）。
> 本文件 = **在 2026-09-24 复核后对阶段2 剩余工作的重排**：**代码已完成，本轮不再重做，只做收口取证 + 剩余证据 + git 门控**。

---

## 0. 一句话结论

阶段2 的**代码面已经落盘且我亲跑为绿**（W7 幅度下发通道整修 + flaky 根因修复），
上一版 §8.1 记录的「`flutter analyze` 4 issues ⇒ 当前树不可 checkpoint」**已不成立**。
**阶段2 现在缺的不是代码，是证据与门控**：收口门禁未按新口径串行全跑、`ignite --check` 未跑、
浏览器肉眼未取证、flaky 修复后 20× 未定名、TTS 端到端因 8080 离线仍未取证、git 无任何 checkpoint。
⇒ 本轮 = **收口（T1–T7）**，代码改动只在门禁报红时由维护者确认后发生。

---

## 1. 维护者独立复核（2026-09-24 亲跑，不采信自述）

| 项 | 实测命令 | 结果 |
| --- | --- | --- |
| Flutter 静态检查 | `cd shell/flutter && flutter analyze` | **No issues found!**（§8.1 的 4 issues 已被清） |
| Flutter 测试 | `flutter test` | **+1049 All tests passed!** |
| Rust 全目标测试 | `cargo test --workspace --all-targets --no-fail-fast` | **exit 0**，26 组 `test result: ok`，0 failed |
| Web 产物 | `stat build/web/main.dart.js` | **2026-09-22 21:08:45**（晚于最后 lib `.dart` 20:51） |
| 断网红线 | `grep -c gstatic.com/flutter-canvaskit build/web/main.dart.js` | **0** |
| wasm 产物 | `crates/l2d-wasm-demo/dist/` | 2026-09-21 22:19；此后无 `l2d-wasm-demo/**` 改动 ⇒ 不算陈旧 |
| 模型资产 | `assets/models/bai/runtime/bai.model3.json` + `bai.moc3` | 在（可点火） |
| 服务 | `curl 127.0.0.1:18080` | **000（已停）**；binary `target/debug/live2d-ai-desktop` 2026-09-21 22:21 |
| TTS | `curl 127.0.0.1:8080/v1/models` | **000（离线）** |

### 1.1 W7 闭环逐条核对（均已落盘）

| 编号 | 内容 | 落点证据 |
| --- | --- | --- |
| W7-1 三层优先级 | 临时覆盖 > 草稿 > 磁盘值，唯一出口 = `ActionScalesSyncer` | `action_scales_sync.dart`；`main.dart:292` 构造 syncer |
| W7-2 接上 `actionScales` | 构造点已传（选方案 (a)） | `main.dart:746 actionScales: _stageActionScales` |
| W7-3 `_applyPrefs` 尊重临时覆盖 | force 补发不得冲掉 pinned | `shell_prefs.dart` + 测试① |
| W7-4 面板文案 | 写明临时值何时被谁清掉 | `dev_tools_section.dart` |
| W7-5 新测试 | `test/action_scales_wiring_test.dart` **8 条** | ①改主题后临时值仍在 ②恢复产品设置 ③iframe 重建 ④不重复下发 ⑤B②标签表 ⑥ttl 表 ⑦A①文案 ⑧C③文案 |
| 欠账 A① | 「叠加基础表情」重新起算 2.6s | `dev_tools_section.dart:1693` |
| 欠账 B② | 标签表优先、常量兜底 | `live2d_stage.dart` ttl + `director_panel.dart` 通道标签 |
| 欠账 C③ | 表演层主路由 / staging 回退 文案 | `director_panel.dart:21 / :53 / :57` |
| 欠账 D④ | 陈旧注释 0.75/1.4/1.0 → 2.2 | `live2d_bridge.dart:205` = `[0.2, 2.2]`；stage/shell_prefs 旧数字 0 命中 |

### 1.2 flaky 修复（已落盘，比「定名」更强）

- 根因：`tests_ws.rs` 旧 `do_client_handshake` 读到 `\r\n\r\n` 后**丢弃** head 缓冲；
  而 `ws/connection.rs` 在 101 之后立即写 `subscribe_ack`，loopback 上两段落进同一 TCP 段 ⇒ 首帧被丢。
- 修复：新建 `crates/live2d-ai-desktop/src/web_api/tests_ws_client.rs`（`split_http_head` 保留残余 + `WsClient` 残余优先读）；
  `tests_ws.rs` 改用该 helper（49+/81-）。**产品协议一行未改，断言语义未放宽**。
- 确定性回归 **2 条**：`handshake_residual_bytes_are_retained`、`first_ws_frame_coalesced_with_101_is_not_lost`。

### 1.3 仍未完成 / 未取证

1. 收口门禁未按新口径串行全跑：`cargo test --doc` / `fmt --check` / `clippy -D warnings` / `rust-ratio ≥95%`。
2. `./scripts/ignite.sh --check --port 18080` 四项未跑（服务停）。
3. 浏览器肉眼：临时覆盖换主题后仍在 + 「恢复产品设置」写下去（STAGE2 §3.4）——未取证。
4. flaky **修复后** 20× 未定名（§6 要求的 20 次是在修复前/中口径）。
5. TTS 端到端（真实 PCM + `turn_state=completed`）——8080 离线，未取证。
6. git 无 checkpoint（Gate 0/1 未执行）。

---

## 2. 本轮范围（T1–T7，代码原则不动）

| # | 任务 | 量 | 归属 |
| --- | --- | --- | --- |
| **T1** | 收口门禁串行全跑（`--no-fail-fast` + tee 原始输出）：workspace all-targets / doc / fmt --check / clippy -D warnings / rust-ratio | S | 编排者 |
| **T2** | Flutter 门禁 + 产物三证据：`analyze` + `test`；**无 lib .dart 改动则只复核 mtime/gstatic，不强制重建** | S | 编排者 |
| **T3** | 点火与托管层：后台起服务 → `ignite.sh --check --port 18080` 四项 ok → 收工前停 | S | 编排者 |
| **T4** | 浏览器肉眼（两条原始读数）：临时覆盖 → 换主题/调音量 → 面板 + HUD `scale_diag` 仍在；点「恢复产品设置」写下去 | S | 编排者（无浏览器能力 → 交用户，出可照做的清单） |
| **T5** | flaky 修复后定名：20× `cargo test --no-fail-fast -p live2d-ai-desktop --bin live2d-ai-desktop -- --test-threads=1`（**勘误**：`--no-fail-fast` 属 cargo 参数，必须在 `--` 之前），并发跑 flutter test；禁止过滤输出 | M | 编排者（只跑命令，不改文件） |
| **T6** | TTS 端到端：先探 8080；200 → 真实 PCM + `turn_state=completed` + error 0；非 200 → 如实标注环境缺口 | S | 编排者 |
| **T7** | 只读复核：A①–D④ 闭环 diff 逐条、W7 8 条测试名、flaky 2 条回归名 + 根因 | S | 编排者 |

**明确不做**：W9 / W10 / W11（待重排，见 §6）；未被点名文件；任何 git 写操作；不改 RESEARCH。
**代码改动唯一入口**：T1/T2 报红 → 停下、把红输出交维护者 → 确认后由维护者指定单一 worker 修 → 重跑整组。

---

## 3. 验收判据（阶段2 收口的定义）

1. T1/T2 全绿且**原始输出**齐全（cargo 分组数 + passed 合计、doc、fmt clean、clippy 0 warning、rust-ratio 百分比、analyze 无问题、flutter passed 数）。
2. T3 `ignite.sh --check` 四项 ok（`GET /`=302、`GET /app/`=200、无 gstatic、产物在位）。
3. T4 两条原始读数（不可用单测替代）。
4. T5 20/20 记录 + 风险清单；复现则立即停。
5. T6 有真实 PCM 证据，或**明确**标注「TTS 离线，未取证」。
6. 报告每节含：命令原文 / 原始输出 / 文件与测试名 / 未决问题；自述无输出者退回。

---

## 4. git 门控协议（**只由维护者执行**）

| 门 | 时机 | 动作 |
| --- | --- | --- |
| **Gate 0** | 编排者交回 T1–T3 全绿证据 + 我独立抽查后 | 在 `mod/l1-product` 落 checkpoint：按程序主题分组提交（L1 产品化 / 记忆 P1-5 / 动作回合 Wave 0–2 含 W7 / 表演层 v0 / flaky 测试端修复），**落前把分组清单报用户确认** |
| **Gate 1** | T4/T5/T6 证据齐 + 我独立复核后 | 选择性提交 W7 文件集 + `fix(w7): 幅度下发通道收敛 + 欠账 A–D`；flaky 修复单独一条 |
| **Gate 2** | 阶段2 全部验收后 | 是否把 `mod/l1-product` 推进到 `main` —— **需用户点头**；不 push / 不 tag / 不 bump，除非明确授权 |

worker 与编排者**一律禁止** git 写操作；交付物 = 「可被维护者直接 `git add` 的文件集合 + 原始门禁证据」。

---

## 5. 阶段3 预览（不在本轮放行，供用户决策）

| 阶段 | 候选 | 前置 |
| --- | --- | --- |
| 阶段3 | 退役「拉 `latest.preset_id`」前端通道，统一到 `action_cue`（原 W9）——大部分已被表演层 v0 门控覆盖 | 用户确认是否只做「删死代码 + 文档」 |
| 阶段4 | 动作协议字段化（body/head/expression + add + hold + 回执 + 音频时钟，原 W10）**并入** `performance-layer-v0.md` 的 speak/cues 契约 | 先重写目标，再放行 |
| 阶段5 | 导演 AI 侧 schema / 手册同源 / 只切不改字 / 两条件唤醒 / 会话 baseline（原 W11） | 同上（合并而非重做） |
| 债务 | `dev_tools_section.dart` 1792 行拆分；WIP git 历史整理 | 与 W7 冲突，W7 收口后单独排 |

---

## 6. 诚实标注

- 本复核为 **WSL2 静态 + 单元/集成门禁**；`ignite --check`、浏览器肉眼、TTS 端到端**本轮尚未取证**。
- 192 条未提交横跨至少 4 个程序，`main.dart` / `settings/view.rs` / `mod_registry.rs` 被多个程序先后改过 ⇒ Gate 0 只能按 §8.3 的「以文件归属为主 + 交叉文件归最后一次改动程序」执行，不承诺逐 hunk 干净。
- 行号会漂移；本文件所有引用按符号/文案定位。

---

## 7. 2026-09-24 收口报告核验与裁决（维护者亲核）

### 7.1 报告可信度（逐条复核）

- **T1**：日志实数与报告一致 —— `/tmp/stage2-close-t1-workspace.log` 26 组 `ok`、0 `FAILED`；doc 3 passed；fmt 0 B；clippy 0 warning；rust-ratio **97.1886%**（90847/93475）**PASS**。
- **T5**：`/tmp/flaky-close-1..15.log` 各 `595 passed / 0 failed`，`16` = `594 passed / 1 failed`；失败用例与行号与报告逐字一致。失败点 `mod_registry.rs:1087-1101` 确为 **10×50ms 轮询 + try_lock** —— 属测试端时钟假设，**报告属实**。
- **T3/T4/T7**：ignite 四项输出与 A①–D④ 源码逐条对得上；T4 两条原始读数（HUD `scale` 行）齐全。
- **勘误（维护者责任，非 worker 之过）**：本计划 §2 的 T5 与上一版提示词把 `--no-fail-fast` 写在 `--` **之后**，libtest 拒绝（`Unrecognized option: 'no-fail-fast'`）。正确：`cargo test --no-fail-fast -p <pkg> --bin <bin> -- --test-threads=1`。已修正上表。
- **计数差 1（192→193）**：维护者本轮新增的 `docs/plans/STAGE2-CLOSEOUT-plan-2026-09-24.md`（未跟踪）；工作树内容零改动，worker「零写操作」成立。

### 7.2 裁决

| # | 事项 | 裁决 |
| --- | --- | --- |
| **D1** | T5 `mod_registry::tests::event_delivers_to_running_mod` 复现 | **授权改 `mod_registry.rs` 测试段**：改成**确定性同步**（channel / Condvar / 信号），**不许只调大超时**；断言文本不变；顺手审计 desktop 测试同类「轮询等待」写法；≥30× 并发压测证明不再复现。若确定性修复被迫改产品代码 → 停下问维护者。 |
| **D2** | T4：重进 Developer 分区后面板 0.75 / HUD 1.50 | **授权 W7b（Dart）**：有 pin 时面板以 pin 为初值 + 显示「临时覆盖生效中」；文件限 `dev_tools_section.dart` + `shell_settings.dart`，扩展/新建测试；重建 web 三证据。 |
| **D3** | T6 TTS 端到端 | ✅ **已取证（2026-09-24 第二轮）**：8080=200；`verify_core_chain.py` **18/18 跳**；WS 探针 **277 audio 帧 / 264960 B 真实 PCM / 24000 Hz / `turn_state=completed` / error 帧 0**；`settings/test/llm` body `ok:true`。日志 `/tmp/stage2-close-s5-*.log`。合并收口时复核一次即可。 |
| **D4** | 首屏 `main.dart.js` pending | 记 backlog（托管层/keep-alive 时序），不在阶段2 收口内。 |
| **D5** | 未跟踪运行时产物 `memory.jsonl` | Gate 0 写入 `.gitignore`；**永不提交**。 |
| **D6** | 阶段2 收口状态 | **被 D1 挡住**：D1 修好 + T5 复跑 20/20 + D2 构建三证据齐，才做 Gate 1。 |

### 7.3 下一轮并发（文件零重叠）

- **Worker A（Rust 测试端）**：`crates/live2d-ai-desktop/src/mod_registry.rs`（**仅测试段**）+ 如需新建 test helper。
- **Worker B（Dart W7b）**：`shell/flutter/lib/settings/sections/dev_tools_section.dart`、`shell/flutter/lib/app/shell_settings.dart`、`shell/flutter/test/action_scales_wiring_test.dart`（扩展）。
- 两者可并行；合并收口门禁（Rust 全量 + Flutter analyze/test + build web 三证据 + ignite --check）在双方 settle 后**由编排者串行跑一次**。

### 7.4 Gate 0 预置

- 当前 193 条 = 4 个程序 + 未跟踪 `memory.jsonl` + `docs/plans/*` + 维护者新计划。
- **建议方案 A（单条 checkpoint）**：目的只是让 D1/D2 的 diff 可独立审阅；§7.1.3 已允许「主题太混则降级为合并提交」。若坚持方案 B（按程序拆 4–5 条），需先做逐文件归属，成本高且仍有交叉。

### 7.5 流程缺口（2026-09-24 第二轮暴露）

第一轮只把「收口续作」提示词交给了编排者，而 D1/D2 是两个**独立执行 worker**——从未被派发
（`porcelain=0`、D1/D2 零改动；编排者全域搜索后按 S1 正确停下，未用基线绿灯冒充合并通过）。
**根因是粘贴目标错位，不是 worker 失败。** 修法：把「派 A/B」与「收口 S1–S6」合并为单一真源
`docs/plans/STAGE2-CLOSEOUT2-worker-prompts-2026-09-24.md`，维护者只交编排者一个入口。
Gate 0（`a7952412`）仍是有效基线，不受影响。

