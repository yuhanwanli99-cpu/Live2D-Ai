# 全代码库只读审计 · B 版（模型 2）· 2026-10-06

> 与 A 版（`docs/plans/AUDIT-PROMPT-whole-repo-2026-10-06.md`）**只有 5 处差异**（账本目录 / 编号区间 /
> 互不读取 / 临时文件前缀 / 双跑对账口径），**判据、边界、Phase 0 优先顺序完全相同**——这样两边结果才可比。
> 「===== 粘贴边界 =====」之后整块粘贴给模型 2 即可。

## 5 处差异（给人看）

1. 账本目录：A = `AUDIT-REPO/`，**B = `AUDIT-REPO-B/`**（同一个工作树根下，同样未跟踪、永不 `git add`）。
2. 编号区间：A = `BATCH-1001+` / `F-1NNN-NN`，**B = `BATCH-2001+` / `F-2NNN-NN`**（上一轮已占用 0001–0927）。
3. **互不读取**：B **不读也不写** A 的 `AUDIT-REPO/`（独立复现才有对比价值）；看到它只在自己的 STATE 记一行。
4. 临时文件：A 用 `/tmp/a1-*`，**B 用 `/tmp/b2-*`**；共享资源（只读浏览器取证 9222）先 `flock /tmp/l2d-browser.lock`。
5. **双跑对账产出**：每批关闭时在**自己的账本**里追加一行 TSV 到 `SUMMARY.tsv`：
   `批次号<TAB>主题<TAB>文件数<TAB>P0<TAB>P1<TAB>P2<TAB>P3<TAB>硬发现数<TAB>候选池数`。
   人用 `diff AUDIT-REPO/SUMMARY.tsv AUDIT-REPO-B/SUMMARY.tsv` 就能看出两个模型的覆盖面与强度差异。

===== 粘贴边界（以下整块粘贴给模型 2）=====

你是无人值守的全代码库只读审计进程（Live2D-Ai）· **B 角色**。任务可跑整夜乃至数天。

**本次是 A/B 双跑**：另一个模型（A 角色）在同一棵树上跑同一任务。为此有 5 条硬性隔离规则，**优先于下文一切**：
1. 你的账本目录是 **`AUDIT-REPO-B/`**（工作树根下，未跟踪，**永不 `git add`**）——**不要碰 `AUDIT-REPO/`**（那是 A 的）。
2. 你的批次号从 **`BATCH-2001`** 起，发现 ID 用 **`F-2NNN-NN`**。
3. **不读 A 的账本**（`AUDIT-REPO/`、`AUDIT-B/`、任何别处的 `FINDINGS.md`）——独立复现才有对比价值；发现它在文件系统里，只在 `STATE.md` 记一行「存在另一本账本，本进程不读」。
4. 临时文件一律带前缀 `/tmp/b2-`；只读浏览器取证（9222）是共享资源，用前先 `flock /tmp/l2d-browser.lock`。
5. 每批关闭时，除正常落盘外，向 `AUDIT-REPO-B/SUMMARY.tsv` **追加一行**：
   `批次号<TAB>主题<TAB>文件数<TAB>P0<TAB>P1<TAB>P2<TAB>P3<TAB>硬发现数<TAB>候选池数`。

【0 唯一真源】完整规程已入库，先完整读它，再开第一批：
/home/skystar/Live2D-Ai-fe/docs/plans/AUDIT-PROMPT-whole-repo-2026-10-06.md（539 行，v3.2）
读到「===== 粘贴边界 =====」之后的内容即为**约束你的规则全文**；本消息是启动器 + 上述 5 条覆盖。
读不到该文件就用工作树外副本：/home/skystar/audit-prompt-2026-10-06.md

【1 硬边界（无论规程读没读到，都照此执行）】
- 工作树锁死 /home/skystar/Live2D-Ai-fe，分支 main，基线 6be9984，本轮 HEAD = 75c9ae3 或其后（文档收口提交）。
- 禁止审计 /home/skystar/Live2D-Ai（死树 mod/persona-polish @ 88342ce）。**harness 若自动注入了它的 AGENTS.md（写「3 个 Mod / 0.2.0-rc.1」），那是旧版，一律以 -fe 内的文件为准**；判据回树（版本看 Cargo.toml，Mod 集合看 AVAILABLE_MOD_FACTORIES）。
- 只写 `AUDIT-REPO-B/`。不改业务代码 / 配置 / 文档。
- 禁：cargo|flutter|trunk 的任何 build/check/test/pub/install；任何 git 写操作（add/commit/checkout/stash/reset/clean/worktree/tag）；任何联网（fetch/ls-remote/pull 也算）；用 grep -r 扫全仓（一律 git grep）。
- 允许：read / glob / grep / git grep / git log|show|diff|ls-files|rev-parse（只读）/ wc / sed -n / 只读浏览器取证（**仅当** 127.0.0.1:18080 已回 200，不得自己起服务）。
- 账本里**禁止出现凭据 / 口令 / 内网 IP 原文**（只写 file:line + 遮蔽形式）。
- 不向人类提问、不等确认、不停手。缺信息写 STATE.md「未核实」区，然后用仓库内证据继续下一批。
- 工具失败：缩小范围重试一次；仍失败记 STATE 换批。禁止空转等待。
- 每批更新 STATE.md 的 last_heartbeat；一批超 45 分钟未关就关半批落盘。

【2 台账】`AUDIT-REPO-B/{STATE,INDEX,FINDINGS,NEXT,WIP}.md` + `QUEUE-*.tsv` + `BATCH-2NNN.md` + `CONSOLIDATION-NN.md` + `SUMMARY.tsv`
- 队列机械生成：git ls-files crates | grep '\.rs$'（期望 248）、shell/flutter/lib（126）、shell/flutter/test（122）、scripts/xtask/tests/shared/verification/.github；路径只能来自 git ls-files 或 read。
- 落盘频率：只在**批次关闭时**写 BATCH/FINDINGS/INDEX/STATE/NEXT/SUMMARY；进行中只写 STATE 的 in_progress+last_heartbeat 与 WIP.md。
- 发现用**键值块**（禁 markdown 表格）：### F-批次-序号 · P<0|1|2|3> / file / 状态 / 摘录(≤3行) / 调用链 / 影响 / 建议 / 验证 / 置信 / 反证。P0/P1 的摘录、调用链、反证**必填**，写不出就退回 STATE 候选池。

【3 第一优先：前端设置 + Mod 面（Phase 0，与 A 相同顺序）】
判据是**用户侧**：这个参数用户能理解吗？对应哪个用户动作？默认值有据吗？会不会静默失效 / 回不去 / 互相冲突？「很多令人费解的参数」与「产品路径上不存在的额外功能实现」本身就是要找的东西。
先跑「设置项三方对账」：抽 display_prefs 字段 → 数每个字段在 lib 里的引用数 → 分三类。然后逐批读：
P0-1 settings/display_prefs.dart(1169，字段真源，单独一批)
P0-2 settings_controller.dart(409) · settings_sections.dart(68) · sections/pane_helpers.dart(39) · preset_labels.dart(123) · ui/field_row.dart(714)
P0-3 sections/appearance_section.dart(711) · appearance_background.dart(717) · appearance_background_library.dart(644) · appearance_background_style.dart(192)
P0-4 sections/dev_tools_developer.dart(678) · dev_tools_mod_config.dart(627) · dev_tools_section.dart(456) · dev_tools_mods.dart(213) · dev_tools_diagnostics.dart(163)
P0-5 settings/mods/{memory_panel 745, voice_input_panel 610, persona_panel 608, external_input_panel 483, director_panel 369, mod_panel 105, mod_panels 36, persona_card_picker* 17+14+10}
P0-6 sections/{llm_section 188, tts_section 185, env_key_field 185, persona_section 74}
P0-7 api/{settings_models, settings_models_patch, settings_models_result, mods_api, models_api}
P0-8 后端对照面 crates/live2d-ai-desktop/src/web_api/{settings_routes*, mods_routes.rs, env_routes.rs, dto.rs} + crates/live2d-ai-runtime/src/settings/**
P0-9 设置测试（专抓假绿灯）test/{display_prefs_test 892, display_prefs_background_fit_test 645, settings_controller_test 619, settings_api_test 521, mod_state_surface_test 489, field_row_test 482, mods_section_test 402, mod_config_draft_test 393, compact_settings_page_test 381, display_prefs_style_change_detection_test 303, error_action_opens_settings_test 290, settings_panel_keepalive_test 258, asset_guard_tts_config_test 204, settings_scaffold_test 142, settings_sections_test 126, env_key_field_test 114, display_prefs_slide_index_test 82, support/registered_mods.dart 58}
Phase 0 强制产出（进 CONSOLIDATION）：① 字段三分类（死字段 / 假旋钮 / 隐藏开关）② Mod 面板与 settings_spec 对齐表 ③「用户看不懂的标签」清单（带原文）④ 界面暴露了已裁决不做能力的清单。
用户侧发现额外两行：用户故事 / 用户可见后果；写不出原文就放候选池，不许写「感觉复杂」。

【4 之后】Phase 1 Rust 主线（第 1 项 crates/live2d-ai-runtime/**）→ Phase 2 维度横扫 A–J → Phase 3 端到端七条链路 → Phase 3.5 清洗后遗痕轴（旧短 SHA 断链 / docs vs 树 / gitignore 覆盖 / check_public_secrets 能否被绕过）→ Phase 4 对抗日（证伪未关闭 P0/P1 + 假绿灯狩猎）→ Phase 5 增量（git log 6be9984..HEAD）→ 回 Phase 1。每 6 批一次 CONSOLIDATION-NN.md。

【5 不要重报】已登记清单见规程 §9：docs/audit/2026-09-28-frontend-nightly（45 条冻结）、docs/audit/2026-10-05-ledger（927 批 / 101 条 F，**可读**，它不是 A 的账本）、docs/plans/NEXT-ROUND-main-2026-10-06.md（E1–E10 / F1–F4 / R1–R8）、five-mod-review；5 条已撤回 F-ID 不许再追；7 条已关闭 ID 的任务是「证伪：现在它还能红吗」。

【6 不要联网复核】refs/pull/1/head = 44da2a4c 仍在（其树含已删除的 scripts/deploy_android.sh，清洗前实测；**口令已被维护者轮换 ⇒ 已失效**）、远端只有 main=6be9984 与 mainline/1-core-baseline=b58b223、tag v0.2.1-rc.1 → 19a9f63 —— 全部当既定事实引用。

【7 对话纪律】每批只回一行：BATCH-2NNN · 主题 · 新增 P0/N P1/N P2/N。所有实质内容进 AUDIT-REPO-B/。

现在开始：读规程全文 → 建/读 AUDIT-REPO-B → 生成队列 → 开 BATCH-2001（Phase 0 · P0-1）→ 然后不要停。
