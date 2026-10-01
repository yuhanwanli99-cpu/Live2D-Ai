# 实现提示词 · 去臃肿轮（Live2D-Ai，2026-10-01）

> 用法：leader 把**公共前置 + 红线 + 验收协议**（见 `ORCHESTRATOR-PROMPT-debloat-round-2026-10-01.md` `6-`8）
> 与下面**对应的一块**拼起来，整块发给 worker。
> 每块含：文件归属 / 必做 / 禁做 / **验收测试（planner 指定）** / 证据 / 回报。

---

## W-S0 · 文档整理（docs-only，0 代码）

**文件归属**：`docs/**`、`AGENTS.md`、`CHANGELOG.md`（**不得碰** `.rs/.dart/.toml/.yaml`）

**必做**
1. 提交 planner 落盘的新文档 + `docs/README.md` 索引（`docs(chore): …`，docs-only，逐字无代码）。
2. **归档执行**（独立 commit）：按 `docs/legacy/plans-archive-candidates-2026-10-01.txt` 把 94 份
   `git mv` 到 `docs/legacy/plans/`，并改链（步骤见 `DOC-MAP.md` `4.1）。先 `git tag checkpoint/pre-doc-archive`。
3. `AGENTS.md` **单一化**：以 `-fe` 版为底；`-Ai` 旧 doctrine 按维护者裁决处理。
4. `CHANGELOG.md` 加停用头注**或**补齐（按裁决）；`docs/releases/v0.3.0.md` 标"历史草案，未发布"。

**禁止**：移动或改写任何源码；一次性改超过 1 个主题。

**验收测试 / 校验（planner 指定）**
- `grep -rn "docs/plans/<归档名>" docs/ AGENTS.md README.md CONTRIBUTING.md` 含**归档名**者 -> **0 命中**；
- `docs/README.md` 里所有相对链接指到的文件都存在（脚本核，逐条输出缺失项）；
- `git show --name-only <commit>` 里**没有** `.rs/.dart/.toml/.yaml`。

**证据**：归档前后文件数（115 -> ?）；改链涉及文件数；断链核验脚本输出。

---

## W-D0 · 度量门禁（**先做，否则精简没有刻度**）

**文件归属**：`xtask/**`、`.github/workflows/**`

**必做**
1. `xtask` 新增子命令 `code-stats`：逐 crate 输出 **prod / inline-test / 集成测试** 行数、
   Dart `lib/test` 行数、`docs` 行数、**超限文件清单**（`src .rs >500/1000`、`Dart >800`）、
   Cargo 依赖计数、产物体积（`build/web`、`wasm dist`）。
2. `code-stats --check`：任一硬门禁超限 -> **退出码非零**（供 CI 用）。
3. CI（`pr-checks.yml` / `flutter-checks.yml`）接三条：行数门禁、`>1000 行 = 0`、依赖上限。

**禁止**：改任何业务 crate 的源码；为了让数字好看而放宽阈值（阈值只在计划里改，且要提 planner）。

**验收测试（planner 指定）**
- `xtask` 单测：给定 fixture 目录树 -> 期望的 prod/test/超限计数（含一个 `#[cfg(test)]` 内联用例的边界）；
- `--check` 红绿双向：临时造一个 `>1000` 行文件 -> 必须红；删除 -> 必须绿；
- 用本文件基线自检：`Rust prod 67,806 / Dart lib 33,476 / docs 69,733`（差异要解释，±2% 内）。

**证据**：`code-stats` 原始输出 + 三条 CI 的 `run:` 原文。

---

## W-R8a-1 · "幽灵态"两半（**独占波次，必须同波做**）

**文件归属**：`shell/flutter/lib/api/ws_client.dart`、`api/ws_liveness.dart`、
`chat/chat_controller.dart`、`state/ui_state_tracker.dart`

**必做**
1. `F-0008-1` **心跳看门狗**：心跳被解析/下发后要有消费者；半开连接可检测（抽成**零依赖纯函数**判据）。
2. `F-0007-1` **本地失败不回落相位**：`POST /api/v1/chat` 本地失败必须清"思考中"与"停止本轮"。

**禁止**：与 R8a-2…5 同时改同一文件（由 leader 保证）。

**验收测试（planner 指定）**
- 纯函数单测：给定"最后心跳时间 + now + 阈值" -> 期望 offline/online 三态（边界 ±1ms）；
- 控制器级：构造 `POST` 失败 -> 状态胶囊**不停在思考中**、发送键**不永变停止**；
- **红-绿**：修复前这两条必须红（写进 GROUNDING）。

**证据**：两条测试的"修复前红 / 修复后绿"原始输出。

---

## W-R8a-2 · 自检"文案猜语义"

**文件归属**：`settings/sections/llm_section.dart`、`settings/sections/tts_section.dart`、
`ui/field_row.dart`、`main.dart`

**必做**
1. `F-0012-1` / `F-0003-2`：成败判定改用 `TestOutcome.ok`（**禁止**扫 `ok/ms/毫秒` 文案）；
2. **成功结果必须有渲染槽**（`F-0003-2` 根因是成功无处显示）。

**验收测试（planner 指定）**
- `ok=true` 且 message 含 "ms" -> **判成功且上屏**；`ok=false` 且 message 含 "ok" -> **判失败**；
- 成功注入 -> `find` 到结果槽（真实泵，不用字符串扫描）。

---

## W-R8a-3 · 无障碍 + re-seed

**文件归属**：`ui/message_bubble.dart`、`settings/sections/dev_tools_section.dart`

**必做**
1. `F-0005-4`：失败轮"重试 / 复制"对读屏**可达**（去掉整条 `excludeSemantics` 的副作用）；
2. `F-0003-6`：`_ModConfigTile` 的 `config` 身份变更**不吞草稿**。

**验收测试（planner 指定）**
- `semantics_test` 风格：真实泵 `MessageBubble` -> 语义树里能找到重试/复制；
- 输入草稿 -> 外部 `config` 变化 -> `controller.text` 保持。

---

## W-R8a-4 · 令牌 / 对比度 / 字体门禁

**文件归属**：`design/tokens.dart`、`ui/theme.dart`、`test/design_tokens_lint_test.dart`、`test/font_subset_test.dart`

**必做**
1. `F-0006-2`：`contentFaint` 不得承载文字（三处换令牌）；白主题对比度 **>= 4.5**；
2. `F-0006-3`：断点 lint 覆盖非字面量写法；
3. `F-0005-5`：字体门禁覆盖**运行时文本来源**（后端文案 / Mod 运行态值），至少形成来源清单 + 检查。

**验收测试（planner 指定）**
- 对比度纯函数单测（白/黑主题各一组，给期望比值）；
- 字体门禁：构造一个含子集外字符的**运行态来源** -> 必须红。

---

## W-R8a-5 · 派生值出口

**文件归属**：`live2d/live2d_bridge.dart`、`live2d/live2d_stage.dart`、`settings/mods/memory_panel.dart`

**必做**
1. `F-0010-2` / `F-0001-4`：`progress` / `fps` 通知必须触发重建（不再显示陈旧值）；
2. `F-0004-1`：记忆面板随**会话桶**切换刷新（跨桶 id 操作必须消失）。

**验收测试（planner 指定）**
- 推一次 progress -> 重建计数 +1 / 文本更新；
- 切桶 -> 列表内容变化（断言具体条目，不是"不为空"）。

---

## W-D1 · 休眠资产裁决（**需维护者裁决后才开工**）

**文件归属**：`crates/live2d-ai-desktop/src/app`、`tray.rs`、`repl.rs`、`benchmark`、`model_smoke`、
`crates/live2d-ai-mod-local-llm`、`-wallpaper`、`-pet-desktop`、
`crates/live2d-ai-core/src/action`、`src/performance`、`Cargo.toml`、`cli_entry.rs`、`main.rs`、
`docs/architecture/ARCHIVED-*.md`

**必做**（按裁决三选一，逐项记录理由）
- ① 删：先打 tag + 归档分支/文档；② 移出 workspace（crate 保留不编译）；③ feature-gate。
- 同步 `mod_count` 断言与文档台账；更新 `AGENTS.md` 休眠台账。

**禁止**：删 `IdleState`；删表演资产 A1-A8；顺手把 `crates/live2d-ai-desktop` 的 `web_api` 一起动。

**验收测试（planner 指定）**
- `mod_count` 断言更新后仍能守住 Mod 集合；
- `cargo test --workspace --all-targets` 全绿 + `--doc` 全绿；
- 休眠行数统计（`code-stats`）前后对比；"显式冻结"路径要有台账文件 + 不参与构建的证据。

---

## W-D2 · 结构拆分（纯搬运）

**文件归属**：`mod_registry.rs`、`web_api/chat_routes.rs`、`web_api/external_routes.rs`、
`live2d-ai-mod-persona/src/lib.rs`、`dev_tools_section.dart`、`appearance_background.dart`、
`main.dart`、`display_prefs.dart`、`app_shell.dart`、`settings_models.dart`、`tokens.dart`

**必做**：只搬运 + 可见性调整；目标 `src .rs <=500`、`Dart <=600`；**行为零变化**。

**禁止**：顺手改逻辑、改公共 API、改测试语义。

**验收测试（planner 指定）**
- 拆分前后 **测试条数与测试名集合一致**（脚本 diff）；
- `cargo test` / `flutter test` 全绿；`code-stats` 超限清单满足 `>1000 = 0`。

---

## W-D3 · 重复消除

**文件归属**：新增的共享 helper 文件 + 引用点（`formatWsError` x3、`decodeDataUrl` x4、
`redact` x4、`effective_token` x2；测试侧 `stripCommentsAndStrings` x8）

**必做**：单一实现 + 全量替换；**禁止**改变对外行为与错误码文案。

**验收测试（planner 指定）**
- helper 单测（含原副本各自的边界用例）；
- **反复制门禁**：一条测试/脚本断言这些 helper 只有一个定义点（`grep -c` 断言）。

---

## W-D4 · 依赖 / 产物瘦身

**文件归属**：`Cargo.toml`、`crates/*/Cargo.toml`、`shell/flutter/pubspec.yaml`、wasm 构建配置

**必做**：按 D1 结果删依赖；给 `build/web` 与 `wasm dist` 定体积预算并记录前后。

**禁止**：改任何功能代码；为了减依赖去重写实现。

**验收测试（planner 指定）**
- `cargo tree` 校验依赖存在性；
- `code-stats` 输出依赖数 + 体积前后对比；目标 `desktop <=22`。

---

## W-D6 · 测试治理（**不砍覆盖**）

**文件归属**：`crates/**/tests/**`、`shell/flutter/test/**`

**必做**
1. 假绿灯清零：空转断言、源码字符串扫描式测试、恒真守卫（每条给出"改错会红"的证明）；
2. 重复夹具 / helper 合并；`>800` 行测试拆分。

**禁止**：为降行数删测试；改测试来迁就实现。

**验收测试（planner 指定）**
- **判别力自证**：每个被改的断言，故意改错一次 -> 必须红（记录到 GROUNDING）；
- 测试条数 **不减**（拆分产生的更多条允许）。

---

## W-VERIFY · 独立复核（非实施者）

**文件归属**：**只读** + 写 `docs/audit/2026-10-01-debloat/<wave>/GROUNDING.md`

**必做**（每波）
1. 在干净 HEAD 上重跑该波的全部门禁，保存原始输出；
2. 逐条读 diff，确认没有"顺手改"的范围外改动；
3. 对每个 P1 构造**至少一个反例**尝试推翻修复；
4. 检查红线（`ORCHESTRATOR` `7）逐条未被触碰。

**输出**：`GROUNDING.md` = 命令原文 + 原始输出 + 结论 + **未核实栏**。
**禁止**：修改任何源码来"让门禁变绿"。

---

## 附：planner 的测试所有权说明

- 本轮的**测试规格**（上表"验收测试"块）由 planner 会话提供；worker **实现**它们，`W-VERIFY` **确认**它们。
- 任何"这条测不了"的结论，必须回 planner 改规格，或明确标注**不可自动化 + 缺什么前提**，不许静默省略。
