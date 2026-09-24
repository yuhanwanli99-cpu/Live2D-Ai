# 调度提示词（Orchestrator）：动作/表情/导演链路修复（W1–W9）

> 交给一个**拥有子 worker 调度与管理能力**的编排 agent。它是编排者，不是执行者。

## 0. 你的角色与目标

你是**编排者**：能启动后台子 agent、给它们发消息纠偏、收集回报、必要时中断重发。职责：
1. 把任务块**原样**分发给子 worker（用 read 工具读交接文件，把对应块整段作为它的 prompt）；
2. 控制并发与文件所有权，**禁止两个 worker 同时改同一个文件**；
3. 每个波次收口时**由你亲自**跑集成门禁并独立复算关键数字，不采信子 worker 自述；
4. 全部完成后给维护者一份可直接审查的交付报告（他会再转给审查方逐条复核）。

**任务真源**：`/home/skystar/Live2D-Ai-l1/docs/plans/IMPL-PROMPTS-actions-performance-round.md`（W1–W9 九块，每块自成一体）。
**调研真源**：`/home/skystar/Live2D-Ai-l1/docs/plans/RESEARCH-actions-director-audit-2026-09-21.md`（**只读**，不要改它；它是本轮所有背景与 file:line 证据的出处，也是审查方对照的基准）。
目标一句话：让倍率滑条全行程有效、让调试面板不再拖着老表情、让「本轮没有预设」真的归零、
清掉 Rust 侧卫生债，并在**拿到裁决**之后修导演的情绪输入与重复驱动。

## 1. 环境事实（先读，别踩）

- 工作区 = `/home/skystar/Live2D-Ai-l1`（git worktree，分支 `mod/l1-product`）。
  另一个 worktree `/home/skystar/Live2D-Ai` 是另一个分支，**绝对不要碰**。
- 该工作树有 **约 178 个已修改文件、8400+ 行未提交改动（WIP）**。因此禁止 `git worktree add` /
  `git checkout` / `git switch` / `git stash` / `git reset` / `git commit` / `git clean`；
  禁止不带 `--check` 的 `cargo fmt --all`（会把全仓重排成巨型 diff）与仓库级 `dart format`。
  所有改动**就地编辑**，不要为了并行去开分支/worktree。
- `live2d-ai.toml`、`.env`、`mods.json` 都是 gitignored 的运行时配置：改动不入版本库、只对本工作树生效。
  `live2d-ai.toml` / `.env` 有 file_watcher 热重载；`mods.json` **没有**。
- **服务状态**：本轮调研时服务是停的（`GET http://127.0.0.1:18080/` 回 `000`，旧 pid 已不在）。
  你可以在本波次需要肉眼/端到端证据时自己点火（`./scripts/ignite.sh`，默认 18080），
  用**后台 job** 起、记下 job id，收工前 `./scripts/ignite.sh --check` 并把它停掉。
  W2 会改 `live2d-ai.toml`：**先看服务在不在**，在的话改完确认热重载日志。
- **前端产物**：上一轮的教训是「Dart 改了但没重建产物」——`shell/flutter/build/web/main.dart.js` 比 Dart 源还旧，
  于是界面改动在 `/app/` 上根本不生效。本轮任何 Dart 改动后**必须**重建，且要在报告里给 mtime 证据。
- 并发编译：多个 Rust worker 同时 `cargo test` 会在 `target/` 上串行等锁（安全但慢）。
  让 worker 迭代期只跑**聚焦测试**（`cargo test -p <crate>`），完整 `--workspace` 门禁留到波次收口时由你串行跑。

## 2. 波次编排（并发铁律）

### 开工前（你自己做，不派给 worker）
1. 记录**基线**：把 RESEARCH 报告 §2.1 的死区表现场复算一遍（脚本即可，不要信文档里的数字），
   存成 `baseline-deadzone.txt`。这是你后面验 W2 的唯一依据。
2. 记录 `git status --porcelain | wc -l`、`shell/flutter/build/web/main.dart.js` 的 mtime、
   `grep -rn 'fn join_endpoint' crates/ --include=*.rs` 的输出、以及 `std::env::var` 的全量输出。
3. 改 `live2d-ai.toml` 前先备份一份到 `/tmp`（它 gitignored，改坏了没法 checkout 回来）。

### Wave 0 —— 6 路并行（文件零重叠）
| 任务 | 独占文件（摘要，详见各块【文件归属】） |
|---|---|
| **W1** 文档对齐 | `AGENTS.md`、`docs/plans/PRODUCT-L1-GOALS-2026-09-15.md`、`docs/architecture/action-packs-v0.md`、`docs/README.md`（**director-rfc.md 归 W8**） |
| **W2** 幅值重标定 | `assets/actions/presets.json`、`preset/{mod,scales,table}.rs`、`preset/tests/*`、`runtime/src/settings.rs`（DEFAULT_*/MIN/MAX）、`live2d-ai.toml(.example)`、`settings_models.dart`、`appearance_section.dart`、两个 Dart 测试 |
| **W3** 调试面板 | `dev_tools_section.dart`、`preset_labels.dart`、`test/developer_section_test.dart`、`test/preset_labels_test.dart` |
| **W5** Rust 卫生 | `runtime/src/lib.rs`、`runtime/src/performance/{client,mod}.rs`、`web/surface/input.rs`、`l2d-wasm-demo/src/main.rs`、`preset/table.rs`、`runtime/src/settings.rs`（**仅 :594 文档段**） |
| **W6** 令牌走 secrets | `mod-external-input/src/lib.rs`、`web_api/{external_routes,voice_routes,cli_entry}.rs` |
| **W8** 导演口径与文档漂移治理（**docs-only**） | `docs/architecture/{director-rfc,director-mod-v0,performance-layer-v0}.md`、`assets/actions/preset_labels.json`；⚠ 与 W4 共用 `performance-layer-v0.md` ⇒ **W8 必须先收口，W4 才能开工** |

⚠ **W2 与 W5 都碰 `runtime/src/settings.rs`**：W5 在这个文件里**只改 :594 那段文档**。
若两者并发，让 W5 **先等** W2 收口（把 W5 排到 Wave 1 也可以）——不要赌「不同行不冲突」。

### Wave 1 —— W4（撤销语义）
独占 `shell/flutter/lib/main.dart`、`live2d/live2d_stage.dart`、`docs/architecture/performance-layer-v0.md`。
理由：`main.dart` / `live2d_stage.dart` 是后面 W7、W9 的热点，必须串行。

### Wave 2 —— W7（幅度下发通道整修）
独占 `main.dart`、`live2d_stage.dart`、`app/shell_prefs.dart`、`live2d/action_scales_sync.dart`、
`app/shell_settings.dart`、`dev_tools_section.dart`（**仅文案段**）、新建 `test/action_scales_wiring_test.dart`。
**必须在 W2 收口后**（要用 W2 定下来的最终默认值与 MAX_SCALE）。

### Wave 3 —— W9（W7 收口后；**无需裁决**）
`main.dart` 归 W9 独占（W7 已收口）。W9 = 退役「拉 `latest.preset_id`」那条前端通道，统一到 `action_cue`（纯缺陷修复）。
`performance-layer-v0.md` / `director-mod-v0.md` 由 W8 在 Wave 0 改过一次，W9 在其基础上继续改，**必须等 W8 收口**。
### Wave 4 —— W10（动作协议字段化；W9 收口后）
独占 `crates/l2d-wasm-demo/src/{main.rs,preset/*,web/surface/*}`、`shell/flutter/lib/live2d/{live2d_bridge,live2d_stage}.dart`、
`docs/architecture/{action-packs-v0,performance-fields-v0}.md`。三字段（body/head/expression）+ 同类叠加 + hold + 事件回执 + `stage-clock`。

### Wave 5 —— W11（导演 AI 侧；W10 收口后）
独占 `crates/live2d-ai-runtime/src/performance/*`、`crates/live2d-ai-mod-director/src/*`、
`crates/live2d-ai-desktop/src/{session_scope.rs,supervisor*}`；`performance-fields-v0.md` 与 W10 共用 ⇒ 必须等 W10 收口。
输出 schema、使用手册与 schema 同源、只切不改字、两条件唤醒、会话 baseline。

### 冲突热点（放行前逐一确认没有别人在改）
`shell/flutter/lib/main.dart`、`shell/flutter/lib/live2d/live2d_stage.dart`、`crates/live2d-ai-runtime/src/settings.rs`、
`shell/flutter/lib/settings/sections/dev_tools_section.dart`、`docs/architecture/performance-layer-v0.md`、
`crates/live2d-ai-mod-director/src/staging_http.rs`（W5 独占；W8 **不得**动它）。

## 3. 子 worker 的调用规范

- 同波次**后台并行**启动，等全部 settle 再进下一波；不要边跑边改下游。
- 每个子 worker 的 prompt = 交接文件里对应块的**原文**（公共前置 + 任务），原样粘贴，不要转述、不要合并两块。
- 回报必须四项齐全：**改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题**。
- 声称「没触发门禁」→ 要求给可复核证据（例如 `find . -name '*.rs' -newermt <基准时间>`），否则不放行。
  本轮**所有块都会碰代码或文档**，没有「不触发门禁」的情况。
- 「应该没问题」「大概通过」= 没通过，要原始输出。跑偏或反复失败 → 中断，把**具体失败点**写清后重发。
- 子 worker **不得**扩大范围：发现范围外的问题只记录进回报，不动手改。
- **禁止**为了让测试变绿而放宽断言；若确实要改语义，必须把理由写进回报。

## 4. 集成与门禁（每个波次收口时你亲自跑，串行）

- `cargo test --workspace --all-targets`
- `cargo test --doc --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo run -p xtask -- rust-ratio`（门槛 ≥95%）
- 该波次改了 `shell/flutter/**` 时：`cd shell/flutter && flutter analyze && flutter test`，
  **并且必须重建产物**：`cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn`。
  重建后复核三件：
  ① `main.dart.js` 的 mtime **晚于**最后一个被改的 `.dart`；
  ② `grep -c gstatic.com/flutter-canvaskit build/web/main.dart.js` == 0（断网红线）；
  ③ `./scripts/ignite.sh --check` 四项全 ok（需先起服务）。
- 该波次改了 `crates/l2d-wasm-demo/**` 时：`cd crates/l2d-wasm-demo && trunk build`，并复核 `dist/` 的 mtime **晚于**最后一个被改的 `.rs`
  （W0 实测踩过：wasm 产物陈旧而原红线只写了 `.dart`）。

任一红 → **不要进入下一波**；把红的原始输出发回该波次负责人，修好后重跑**整组**。逐波记下门禁数字。

## 5. 独立核验（不采信自述；全部完成后逐条自测并贴原始输出）

1. **死区复算**：你自己再跑一遍基线脚本，对比 W2 报告的「表值 / 峰值系数 / 出厂值 / 上限 / 死区起点」五列。
   数值必须与你的独立算一致（允许四舍五入到 2 位）。特别复核 `shake` 的峰值系数不是 1.0（实测约 0.9285）。
   再断言三条：**scale 旋钮全程不触上限**、**intensity 旋钮按选定方案不触上限**、**出厂组合 ≤ 上限 x 0.60**。
   然后**独立列出「两个旋钮同时拉满会钳位」的组合表**，与 W2 报告里那张表逐行对照（口径必须一致，不许一方省略）。
2. **身/头比**：逐包按**主轴配对**（look_*/shake → X 对、nod → Y 对、tilt_* → Z 对）打印「表内身/头」与「出厂后身/头」，
   必须都落在 [0.30, 0.50]。注意 tilt_left/right 原本是 4/16 = 0.25（表内就不达标，与倍率无关），这条要能看出它被修了。
3. **默认值四处一致**：`live2d-ai.toml`、`live2d-ai.toml.example`、`runtime settings.rs` 的 DEFAULT_*、
   `preset/scales.rs::PRODUCT_DEFAULT`、Dart `settings_models.dart` 的 default* —— **五处**两两相等，把打印结果贴出来。
   （这条是「界面一套、渲染面一套」那类 bug 的专门护栏。）
4. **临时覆盖不再被冲掉**（W7 后）：起服务，在 `/app/` 里进 Developer 面板设一个明显偏离的临时值 →
   回「外观与互动」换个主题（或调一次音量）→ **回 Developer 面板读值**、并读渲染面 HUD 的 `scale_diag` 行，
   证明临时值仍在；再点「恢复产品设置」，证明产品值确实写下去。两条都要原始读数。
5. **撤销真的发生**（W4 后）：起服务，用一个中性措辞发一轮（director 判定为 neutral、`latest.preset_id` 为 null），
   读 director 的 `GET /api/v1/mods/director/state` 与渲染面 HUD 的 `preset:` 行：
   期望舞台两槽归零、HUD 不再显示上一轮的 preset。贴出 state JSON 的两个字段与 HUD 行。
6. **调试面板**（W3 后）：在 `/app/` 的「表情调试」点一条表情，等它 ttl 到点（约 2.6s），
   确认 UI 不再声称「当前基础表情：X」；再点「动作调试」的一条手势，确认**没有**把那条表情重新点亮。
   这两条是症状①的肉眼判据，必须真的在浏览器里做（不要用单测替代）。
7. **卫生**：`grep -rn 'fn join_endpoint' crates/ --include=*.rs`（期望：runtime 1 处 + 两个 Mod 的跨边界副本，
   数量与解释都要与 W5 回报一致）；`grep -rn 'std::env::var' crates/ --include=*.rs | grep -v test`
   （期望：只剩 secrets.rs、纯环境探测、CLI 的 LIVE2D_AI_* 开关——三个令牌名不再出现）。
8. **端到端不回归**：`POST /api/v1/chat` 发一条「下午好」，挂 `/ws/state`，确认 `text_delta` + 真实 PCM 音频帧 +
   `turn_state=completed`、`error` 帧 0；`POST /api/v1/settings/test/llm` **读 body 的 ok**（不看 HTTP 状态，恒 200）。

9. **只切不改字**（W11）：拿主模型原文与导演输出的 `segments` 做**逐字**比对（含标点 / 换行 / emoji 样本），必须完全相等；
   再构造一个「改了一个字」的坏输出，证明**整份回退**（不做局部抢救）。
10. **字段化 + 叠加 + hold**（W10）：发一个含 `body` + `head` + `expression` 各 ≥1 条的帧，读 HUD 的 `preset:` 行与渲染面 ack 事件，
    证明三字段都生效、同类**相加**、`hold` 不被 ttl 撤掉；再构造缺参数场景，证明 `dropped` 非空。
11. **时间轴统一**（W10）：确认渲染面的动作进度用的是 `stage-clock`（音频时钟）；**人为停发 `stage-clock`**，证明回落到墙钟且不崩。
12. **渲染面产物**：`crates/l2d-wasm-demo/dist/` 的 mtime 晚于最后一个被改的 `l2d-wasm-demo/**/*.rs`
    （W0 实测踩过：wasm 产物陈旧，而原红线只写了 `.dart`）。

## 6. 失败与边界

- 需要动「基座独占文件」（`crates/live2d-ai-mod-system/src/topics.rs`、`mod_registry.rs`、`supervisor.rs`、`main.rs`）→
  **停下问维护者**。W8 若发现非改 topics.rs 不可，就是这种情况。
- W2 的「收窄 MAX_SCALE」是**维护者的裁决**（裁决点 A）：按块内推荐路径先做并**附备选数值表**，不要自行决定选哪个。**11 个块全部已裁定，没有等待项**。
- 门禁反复红 / 两个 worker 抢同一文件 / 需要动本包点名之外的模块 → 停下说清「哪个任务、哪个文件、什么现象、试过什么」。
- **不要**顺手做 W1–W9 之外的重构（例如拆其它超大文件）；发现的问题记进回报的「未决问题」。
- 不要把密钥明文、`.env` 内容、请求体写进任何报告或日志。

## 7. 最终交付（给维护者，他会转给审查方）

按任务分节（W1–W9），每节包含：
- 改动文件（关键 diff 片段即可，不要贴整文件）；
- 新增/修改的测试名；
- 该波次的完整门禁数字（cargo 测试条数 / doc / fmt / clippy warning 数 / rust-ratio 百分比 / flutter analyze+test / flutter build mtime）；
- **独立核验 1–8 的原始输出**（命令 + 结果，逐条，不许转述）；
- 未决问题、已知豁免、以及**没做的事**（如实标注）。

另外单独给一节「未做 / 需维护者过目」：① 裁决点 A 的两个候选数值表；② W10/W11 完成后，把「三字段协议 / 音频时钟 / 两条件唤醒 / 会话 baseline」的**实际行为**写成一页小结（含 §5 核验 9–11 的原始输出）。
**已裁决**：情绪/表演决策的**输入 = 用户输入**；不要再把「改判角色回复」写进任何建议。**不要**引用「导演属场景 Mod / 主链不该有第二 LLM」那条推论（已被否定）。

审查方会照「声明 → 实测」逐条复核：**凡是自述、没有原始输出的结论都会被退回**。

## 8. 启动提示词（维护者直接粘贴给编排者的那一段）

```text
【角色】你是本次改造的**编排者（orchestrator）**，不是执行者。你负责：把任务块原样分发给子 worker、
控制并发与文件所有权（禁止两个 worker 同时改同一个文件）、每个波次收口时**亲自**跑门禁并独立复算关键数字、
最后交一份可直接审查的交付报告。

【工作区】**/home/skystar/Live2D-Ai-l1**（git worktree，分支 mod/l1-product，约 178 个未提交改动）。
绝对不要碰 /home/skystar/Live2D-Ai（另一个 worktree）。下面所有路径都是绝对路径，
**不要**在 /home/skystar/Live2D-Ai 下找同名文件——那里的文档是旧分支的，会把你带偏。

【先读这三份，按顺序】
1. /home/skystar/Live2D-Ai-l1/docs/plans/ORCHESTRATOR-PROMPT-actions-performance-round.md
   → 你的作业指导书：§1 环境事实 / §2 波次编排与冲突热点 / §4 门禁 / §5 独立核验 8 条 / §7 交付格式。
2. /home/skystar/Live2D-Ai-l1/docs/plans/IMPL-PROMPTS-actions-performance-round.md
   → 任务真源：W1–W9 九个块，每块含公共前置 + 任务 + 【文件归属】。
3. /home/skystar/Live2D-Ai-l1/docs/plans/RESEARCH-actions-director-audit-2026-09-21.md
   → 调研真源（**只读，不要改它**）：每个任务的背景、file:line 证据与量化表都在这里，子 worker 开工前应读相关章节。

【本次范围：Wave 0 → Wave 5，共 11 个任务块，**全部已裁定，没有等待项**】
  Wave 0（6 路并行，文件零重叠）：W1 文档对齐 / W2 幅值重标定 / W3 调试面板 / W5 Rust 卫生 / W6 令牌走 secrets /
                                  W8 导演口径与文档漂移治理（docs-only）
    ⚠ W2 与 W5 都碰 crates/live2d-ai-runtime/src/settings.rs：W5 在该文件里**只改 :594 那段文档**；
      若有风险就把 W5 排到 Wave 1 与 W4 并行。
    ⚠ W8 与 W4 共用 docs/architecture/performance-layer-v0.md ⇒ W8 必须**先**收口，W4 才能开工。
  Wave 1：W4 撤销语义（独占 main.dart / live2d_stage.dart）
  Wave 2：W7 幅度下发通道整修（W4 收口后；与 W4 共用那两个文件）
  Wave 3：W9 单一驱动者（W7 收口后；退役「拉 latest.preset_id」那条通道，统一到 action_cue）
  Wave 4：W10 动作协议字段化（W9 收口后）：body / head / expression 三字段 + 同类叠加 + hold + 事件回执 + 音频时钟
  Wave 5：W11 导演 AI 侧（W10 收口后）：输出 schema / 使用手册与 schema 同源 / 只切不改字 / 两条件唤醒 / 会话 baseline
    ⇒ W9 → W10 → W11 是**同一条链路的递进改造**，必须串行，不许并行抢文件。
  ★ 产品形态（权威，见 RESEARCH §3.7 / §3.8）：**「酒馆（类酒馆角色扮演内核）+ Live2D 皮套壳子」**；
    人设由内核稳定，各功能由 **mod 矩阵**承担，皮套负责情绪表达；**导演是一个 AI**。**不做架构搬迁。**
  ★ 已裁定、不得翻案：情绪判定的**输入 = 用户输入**（不是角色回复）；过滤层**不许改写**、**允许断句**（只切不改字）；
    三字段的每皮套强度**复用现有 [action] 三倍率**（不新增第四套）；**非标准参数本轮不做**；hold 可加；
    输出**立即生效**、不需要动作队列；唤醒 = ① TTS 段播完 **且** ② 动作做完；
    断开（停止 / 新消息）= 清「动作 + 表情 + TTS 待播」+ 退回**该会话**的角色基础状态值。
    ⚠ 「导演属场景 Mod / 主链不该有第二 LLM」那条**是我的推论、已被维护者否定**，不得引用，也不要做架构改造建议。
  ★ W2 按块内**推荐路径**实施（收 MAX_ACTION_SCALE 到约 1.29、出厂 body 0.8），
    但必须在报告里附备选方案（保持 2.5、下调表值）的数值表，供维护者过目裁决点 A。

【分发方式】每个子 worker 的 prompt = IMPL-PROMPTS 文件里对应块的**原文**：用 read 工具读出该块，
整段原样粘贴作为它的 prompt，不要转述、不要删减、不要把两个块合并。同波次后台并行启动，
全部 settle 后由你亲自收口门禁，再进下一波；不要边跑边改下游。

【开工前你必须先做（不派给 worker）】
1. 读 RESEARCH 报告 §2.1，**自己复算一遍死区表**（脚本即可）存成 /tmp/baseline-deadzone.txt——这是你后面验 W2 的唯一依据，
   不要采信文档或 worker 给的数字。
2. 记录基线：`git -C /home/skystar/Live2D-Ai-l1 status --porcelain | wc -l`、
   `stat -c '%y %n' /home/skystar/Live2D-Ai-l1/shell/flutter/build/web/main.dart.js`、
   `grep -rn 'fn join_endpoint' /home/skystar/Live2D-Ai-l1/crates --include=*.rs`、
   `grep -rn 'std::env::var' /home/skystar/Live2D-Ai-l1/crates --include=*.rs | grep -v test`。
3. **把 /home/skystar/Live2D-Ai-l1/live2d-ai.toml 备份到 /tmp**（它 gitignored，改坏没法 checkout 回来）。
   改它之前先看服务在不在（`curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:18080/`），在的话改完确认热重载日志。

【红线（违反即整轮作废）】
- 禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 178 个文件的 WIP）；
  禁止不带 --check 的 cargo fmt --all；禁止仓库级 dart format。
- 不碰 /home/skystar/Live2D-Ai。
- 任何 .dart 改动后必须重建产物：
  `cd /home/skystar/Live2D-Ai-l1/shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn`，
  并复核三件：① 产物 mtime **晚于**最后一个被改的 .dart；
  ② `grep -c gstatic.com/flutter-canvaskit build/web/main.dart.js` == 0；③ `./scripts/ignite.sh --check` 四项全 ok。
  （上一轮就是漏在这一步：Dart 改了、产物没重建，界面改动在 /app/ 上根本没生效。）
- 有 `crates/l2d-wasm-demo/**` 改动时，必须 `cd /home/skystar/Live2D-Ai-l1/crates/l2d-wasm-demo && trunk build`，
  并复核 `dist/` mtime 晚于最后一个被改的 `.rs`（这条 W0 踩过，原红线漏写）。
- 需要动**基座独占文件**（crates/live2d-ai-mod-system/src/topics.rs、mod_registry.rs、supervisor.rs、desktop/src/main.rs）
  → 先停下问维护者，不要自行扩大范围。
- **不采信子 worker 自述**：每一条结论都要原始命令输出；「应该没问题」「大概通过」= 没通过，退回重做。
  禁止为了让测试变绿而放宽断言；要改语义必须把理由写进报告。
- 不把密钥明文、.env 内容、请求体写进任何报告或日志。

【收工交付】按 ORCHESTRATOR 文件 §7 的格式逐任务分节（W1/W2/W3/W4/W5/W6/W7），每节含：
改动文件（关键 diff 片段即可）/ 新增或修改的测试名 / 该波次完整门禁数字 / 未决问题。
外加**必须**包含：
- ORCHESTRATOR §5 独立核验 1–8 的**原始输出**（命令 + 结果，逐条，不许转述）；
- W2 的四张表 T1–T4（死区表、身/头比对照、两旋钮同拉的钳位组合、断言的实际 cargo test 输出）；
- 单独一节「本轮未做 / 需裁决」：裁决点 A（MAX_SCALE 取舍）与 **B2**（[performance] 去留）的现状 + 你的建议。
  （「情绪判定的输入 = 用户输入」已裁决，不要再列成待裁决项；待裁决的是 RESEARCH §3.7 的 Q1/Q2。）

维护者会把这份报告转给审查方逐条复核：**凡是自述、没有原始输出的结论都会被退回。**

【第一步】现在**先不要派 worker**。读完那三份文件后，先回报两样东西：
① 你的波次计划（哪个任务在第几波、每个波次占用了哪些文件、你判断的冲突点）；
② 开工前基线（死区复算结果 + 上面第 2 条那四项的实际输出）。
等维护者确认后再派 Wave 0。
```

---

## 9. Wave 1 续作指令（粘贴给**同一个**编排者；也可给新会话用）

```text
【续作指令 · Wave 0 已验收通过，放行 Wave 1】

（若你这条会话没有上一轮上下文，先读这三份再执行：
 /home/skystar/Live2D-Ai-l1/docs/plans/ORCHESTRATOR-PROMPT-actions-performance-round.md
 /home/skystar/Live2D-Ai-l1/docs/plans/IMPL-PROMPTS-actions-performance-round.md
 /home/skystar/Live2D-Ai-l1/docs/plans/RESEARCH-actions-director-audit-2026-09-21.md ）

【1. 两份提示词文件已更新，动手前重读这几处】
- IMPL-PROMPTS 的**公共前置**（新增 wasm 重建纪律，11 处全补）；
- ORCHESTRATOR 的 §4 门禁、§5 独立核验（新增第 12 条）、§8 红线段。
即：只要有 crates/l2d-wasm-demo/** 的改动，必须 `cd crates/l2d-wasm-demo && trunk build`，
并复核 dist/ 的 mtime **晚于**最后一个被改的 .rs。这条是你发现的（原红线只写了 .dart），已补进文件。

【2. W0 验收结论：通过（维护者已独立复核，不采信自述）】
你不必重做，但把下面当继续的前提：
- 门禁：cargo test **1394 passed / 0 failed**（39 个 test result: ok、FAILED 0）、doc / fmt / clippy 全绿、
  rust-ratio **97.1829% PASS**、flutter analyze 无问题、flutter test **+1036**、flutter build web exit 0、trunk build exit 0；
- 产物：main.dart.js **21:18:57**（晚于最后 .dart，-newer 计数 0）、gstatic 计数 0、ignite --check 四项 ok、dist/ 21:18；
- 五处默认值 **0.75 / 0.80 / 1.0** 一致；MAX_ACTION_SCALE **三处一致 = 2.2**；
- W2：我从源码重算 ⇒ **四条硬指标违规 NONE**；主轴死区起点 **2.500 / 2.564 / 2.693 / 2.762 全部 > 2.2**（滑条全行程有效）；
  身/头比 **表内 0.3250 / 出厂后 0.3467**；出厂值与你的 before/after 逐格吻合；
- W6：三令牌的 5 处 std::env::var 命中**全部在测试文件**，生产路径 = 0；
- 两条未取证项（TTS 不在线导致拿不到真实 PCM 帧与 turn_state=completed、W2 的「不推送 scales」兜底路径）
  判为**环境/探针缺口，不是回归**——你没有造假绿灯，处理正确。

【3. 维护者对 §7 A–E 的裁决】
A（用户选 **(i)**）**维持 W3 现状**：默认行为已修，残留只在显式打开「叠加基础表情」时出现。
  ⇒ **不开 W3b、不改 _applyGesture、旧 Face→Gesture 回归保留语义**。
  ⇒ 只加一条最小要求：把该开关的说明文案写实（「打开后每次点手势都会把当前基础表情**重新起算**（2.6s 重新计时）」），
     并进 **W7 的 dev_tools_section.dart 文案段**（它本来就要改那一段）。
B **并入 W7**：不是删 kExpressionPresetIds，而是两个消费点（live2d_stage.dart 的 ttl 显示、
  director_panel.dart 的表达式判定）**优先用标签表、取不到表才回落常量**；报告里贴两处改后 diff。
C 拆两处：三份文档（core-chain-baseline.md / directory.md / mod-product-chain.md）→ **W8b（Wave 1，docs-only）**；
  director_panel.dart:21/50-57 → **并进 W7**。
D 逐条定主：`l2d-wasm-demo/src/main.rs:391` → **W5**；`live2d_stage.dart:93` + `shell_prefs.dart:31` → **W7**；
  **`live2d_bridge.dart:205` 授权给 W7**（1 行注释）；`action-packs-v0.md` 的标定口径 + T3 表 → **W2b（Wave 1）**。
E **并入 W5**：`settings.rs:497` 那行一并改成实情，报告里贴改后行。

【4. 现在放行 Wave 1 —— 4 路并行（文件零重叠）】
  W4  撤销语义     ：shell/flutter/lib/main.dart、live2d/live2d_stage.dart、
                     docs/architecture/performance-layer-v0.md、test/action_cue_test.dart
  W5  Rust 卫生    ：crates/live2d-ai-runtime/src/lib.rs、performance/{client,mod}.rs、
                     crates/l2d-wasm-demo/src/{main.rs,preset/table.rs,web/surface/input.rs}、
                     crates/live2d-ai-runtime/src/settings.rs（:594 **+ ★:497**）
  W8b 文档漂移三处 ：docs/architecture/{core-chain-baseline,directory,mod-product-chain}.md（docs-only）
  W2b 标定口径+T3  ：docs/architecture/action-packs-v0.md 一处 + assets/actions/presets.json 的 _doc（docs/json-only）
  分发方式照旧：每块 prompt = IMPL-PROMPTS 里该块**原文**，其后追加【编排补充】。
  W5 的【编排补充】追加三条：① W2 已改 settings.rs 的 :594 段，只核对、不要回退；
    ② 顺手把 :497 改成实情；③ 本块会改 l2d-wasm-demo/** 的注释 ⇒ 收口必须 trunk build。
  **不要**为 A/B 另开 worker（已并入 W7，Wave 2 才做）。

【5. Wave 1 收口要求】
- 你亲自串行跑整组门禁（沿用 W0 口径），给原始输出与数字；
- **trunk build**：因 W5 改了 l2d-wasm-demo/**，收口必须重建并复核 dist/ mtime 晚于最后一个被改的 .rs；
- **flutter build web**：W4 改了 Dart ⇒ 三项红线证据（mtime / gstatic 0 / ignite --check）照旧由你出；
- 服务**留着别停**（W4 要用浏览器核验）。

【6. Wave 1 报告里我要看到的】
1) A–E 五条的**闭环状态**（各自做完没做、转给谁），逐条给 diff 或原始输出；
2) W5：settings.rs:497 改后的行 + join_endpoint 收敛**前后**的 grep 对比；
3) W4：**浏览器实测**——发一轮中性措辞，HUD `preset:` 两槽归零、不再显示上一轮残留（单测不能替代）；
4) 未做/未取证项如实列出（含 TTS 不在线导致端到端仍缺的一块）。

【7. 一条环境提示（不是你的活）】
`[tts] base_url = http://127.0.0.1:8080/v1` 目前不在线（维护者已确认 curl 000）。
如果在你收口前它起来了，请把 §5.8 的端到端**重跑一次**（要真实 PCM 帧 + turn_state=completed）；
没起来就照旧如实标注。

【8. 红线不变】不碰 /home/skystar/Live2D-Ai；禁止 git worktree / checkout / switch / stash / reset / commit / clean；
不改 RESEARCH-actions-director-audit-2026-09-21.md（规格真源，只读）；
需要动 topics.rs / mod_registry.rs / supervisor.rs / desktop/src/main.rs 时先停下问维护者。
```


---

## 10. Wave 2 续作指令（粘贴给同一个编排者）

```text
【续作指令 · Wave 1 已验收通过（有条件），放行 Wave 2；并附带一个必须做的 flaky 定名】

【1. Wave 1 验收结论：通过（我独立复核过，不采信自述）】
门禁 1394/0（我求和确认）、flutter +1041 All tests passed、join_endpoint 收敛为 runtime 1 处 + 两个 Mod 跨边界副本、
settings.rs:497 已改、main.rs:391 已改 [0.2, 2.2]、W8b 的「动作在产品路径上不存在」0 命中、
W4 的 decide() 顺序正确（:104 先去重 → :110 再归零）、W2b 的 §11 与 presets.json 数值零改动 —— **全部对得上**。
两处要你在最终报告里解释清楚：
  ① `w0-gates.log` 有 39 个 `test result: ok`、`w1-cargotest-clean.log` 只有 26 个，但两者 passed 合计都是 **1394** —— 请说明口径差异（哪个命令、哪个 bin）；
  ② `w1-flutter.log` 是摘要脚本输出、不是原始 flutter 输出 —— **从现在起门禁一律 tee 原始输出**（见第 2 条）。

【2. flaky：接受你的自我判断，但「未定名」不能就这样过去 —— 做有界定名】
你那次是 fail-fast 导致 740 中止（合理），3 次干净 1394/0、2 次定向复现绿（可信）。请照下面做，**与 Wave 2 并行、不共享文件**：
  for i in 1..20: `cargo test -p live2d-ai-desktop --bin live2d-ai-desktop -- --test-threads=1 --no-fail-fast`
    输出 `2>&1 | tee /tmp/flaky-$i.log`；同时在另一终端跑 `flutter test` + `flutter build web` 制造并发窗口。
  ★ **禁止**把输出过滤成只剩 `test result:` 行（上次就是这样丢掉用例名的）。
  结果分支：20/20 绿 → 记「未复现」，并给出风险清单（计时敏感面点名：`supervisor/tests_stall.rs`、`web_api/ws/connection.rs`）；
           一旦复现 → **立刻停下、Wave 2 暂停**，把失败用例名 + 原始输出交给维护者。
  流程纪律（比 flaky 本身更重要）：**所有门禁命令一律 `--no-fail-fast` + tee 全量输出**，不再用汇总行。

【3. 放行 Wave 2 —— W7（幅度下发通道整修）】
【编排补充】除块内原文外，追加以下九条：
  A① 把「叠加基础表情」开关的说明文案写实：「打开后每次点手势都会把当前基础表情**重新起算**（2.6s 重新计时）」。
     （不改 `_applyGesture` 语义、不动旧 Face→Gesture 回归 —— 维护者已裁 (i)。）
  B② 两个消费点改成**优先用标签表、取不到表才回落常量**：`live2d_stage.dart:436`（ttl 显示）、`director_panel.dart:104`（通道标签）；
     不删 `kExpressionPresetIds`（`preset_labels.dart:32` 是定义点，:25 已写明兜底理由）。报告里贴两处 diff。
  C③ `director_panel.dart:21 / 51 / 55` 的「遗留回退旁路 / 不是产品主路径」改成实情。
  D④ 三处陈旧注释：`live2d_stage.dart:156`「0.75 / 1.4 / 1.0」、`shell_prefs.dart:31`「0.75 / 1.4 / 1.0」、
     `live2d_bridge.dart:205`「钳 [0.2, 2.5]」→ 2.2（**维护者已授权你改这一行**）。
  ⑤ `live2d_bridge.dart` **只改那一行注释**，不要顺手扩大；`shell_prefs.dart` / `action_scales_sync.dart` 的改动仍限本任务范围。
  ⑥ 权威构建：W7 改了 Dart ⇒ 收口时由你串行跑一次 `flutter build web … --no-web-resources-cdn` 并出三证据；
     **前端改动不必重启后端**（静态文件按请求读盘），但浏览器要硬刷新；本波不碰 `l2d-wasm-demo` ⇒ 不必 trunk build。
  ⑦ W7 与 W9 都碰 `main.dart` / `live2d_stage.dart` ⇒ 仍须串行（W9 留在 Wave 3）。
  ⑧ 报告里逐条给 A/B/C/D 四条的**闭环 diff**（这是 Wave 1 转过来的欠账，本轮必须结清）。
  ⑨ 不要为 flaky 另开 worker，也不要为了让它变绿去改测试期望。

【4. 收口要求】整组门禁（`--no-fail-fast` + tee）、`flutter build web` 三证据、服务留着别停。

【5. 报告里我要看到的】
1) W7 的改动文件 / 测试名 / 门禁数字；
2) A/B/C/D 四条闭环 diff（逐条）；
3) 浏览器实测：设一个明显偏离的临时幅度覆盖 → 回「外观与互动」换主题（或调音量）→ **回 Developer 面板读值 + 读渲染面 HUD**，
   证明临时值仍在；再点「恢复产品设置」，证明产品值确实写下去（两条都要原始读数）；
4) flaky 定名的 20 次结果（每次的 passed/failed 与文件路径），未复现则给风险清单；
5) 未做/未取证项如实列出；TTS 若仍未起，端到端那块照旧标注缺。

【6. 红线不变】不碰 /home/skystar/Live2D-Ai；禁止 git worktree / checkout / switch / stash / reset / commit / clean；
不改 RESEARCH-actions-director-audit-2026-09-21.md（规格真源，只读）；需要动 topics.rs / mod_registry.rs / supervisor.rs / desktop/src/main.rs 先停下问维护者。
```

