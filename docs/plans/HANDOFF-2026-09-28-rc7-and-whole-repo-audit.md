# HANDOFF（2026-09-28 晚）：rc.5/rc.6/rc.7 已收口 · 下一步 rc.8 + 全代码库审计

> **给下一个接手 agent**。本文是**状态快照 + 待办**，不是新规划。
> 规划/调度真源：`docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md`、
> `docs/plans/ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md`、
> `docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md`、`docs/plans/BACKLOG-0.2.0-closeout-2026-09-28.md`。
> 全库审计提示词：`docs/plans/AUDIT-PROMPT-whole-repo-2026-09-28.md`（本轮新增，可直接粘贴）。
> **本轮（2026-09-28 晚）由只读复核会话产出**：未改业务代码，只新增本文与本轮审计提示词两份文档。
>
> **补记（2026-09-28 22:50，编排者并发提交 `dde6b855`）**：
> ① 本文 §2 的 **6 条文档债已全部修复**（AGENTS 当前版本 → rc.7 + 补 rc.5/rc.6/rc.7 三条变更历史；TRIAGE §1 状态列回填、45 行标题重生成且截断根因查明＝生成器正则 `[^\n]` 在 ERE 里被当成「非反斜杠且非 n」；CHANGELOG 加停用声明；v0.3.0 不再写死 rc.5；BACKLOG 语音 WIP 改判为已入库）。
> ② **待裁决 ① 的归档部分已完成**：`docs/legacy/-Ai-dirty-tracked-2026-09-28.patch`（136 项脏改动）+ `docs/legacy/-Ai-untracked-2026-09-28.txt`（23 项未跟踪，含 sha256）；`-Ai` worktree 本身**仍在**，删除仍待维护者点头。
> ③ 本轮新增的两份文档 + `docs/README.md` 索引**尚未提交**（见文件末尾「本次未提交」）。

---

## 0. 一分钟上手

- **唯一开发线**：`/home/skystar/Live2D-Ai-fe` · 分支 `feat/frontend-redesign` · HEAD **`dde6b855`**（= rc.7 `d0e22f12` → 挂账 `932ea5d4` → 复核修正 `dde6b855`）。
- **0.2.0 线状态**：`main` = `e4f139a8`（v0.2.0-rc.4）；`feat/frontend-redesign` **领先 14、落后 0**（即 main 是其祖先 ⇒ 末版可 **`--ff-only`** 合流）。
- **已打 tag**：`v0.2.0-rc.1 … v0.2.0-rc.7` + `checkpoint/rc5-pre-rc6`（rc.2/rc.3/rc.5 已补齐）。
- **仓库卫生**：worktree **3**（`-fe` / `-Ai` / `-product`）、本地分支 **12**、stash **0**、`-fe` 未跟踪 **0**、磁盘 **37G 可用**。
- **门禁基线（rc.7 实测）**：cargo test **1457/0**、doc 3、fmt clean、clippy 0、rust-ratio **97.3263% PASS**；
  flutter analyze **0 issue**、flutter test **1378**；#build FRESH、`gstatic.com/flutter-canvaskit` **0/0**；`ignite.sh --check` 四项 ok。
- **下一步**：`rc.8-a`（正确性与诚实性）→ `rc.8-b`（结构）→ **0.2.0 末版**；与一条**并行只读全库审计轨**。

---

## 1. 本轮（只读复核会话）做了什么

1. 复核了编排者 rc.6/rc.7 收口报告，**独立复算**以下事实均成立：
   - 提交链 `31d06cb4 → e1e05eb3(rc.6) → d0e22f12(rc.7) → 932ea5d4`、工作树 clean、tag 齐全；
   - 审计**冻结计数 45 条 · P0 0 / P1 12 / P2 19 / P3 14**（用账本自带命令复算一致）；短跑 17 条；两账本 BATCH-0001…0004 `cmp` 逐字节相同；
   - 我在 rc.7 树上**独立重跑**：`flutter analyze` = No issues found；`flutter test` = **+1378 All tests passed!**；`gstatic.com/flutter-canvaskit` = 0/0；产物新鲜（无 `.dart` 晚于 `main.dart.js`）；
   - 关闭的实现真实存在：背景解码备忘（`lib/data/background_decode_cache.dart`）、水合合并（`main.dart:202 applyBackgroundHydration`）、`sendStageBg` 去重（`live2d_bridge.dart:282`）、`F-0005-2` 的 `settingsRevision + _settingsTick` 缓存键（`app_shell.dart:118/393/641`）、`test/rebuild_scope_test.dart`；
   - 4 条假绿灯已改成**能失败**的断言（`no_backdrop_filter_test.dart:246-270` 剥注释 + 锚定 `build` 的 return；`semantics_test` 真泵 `StatePill`；`chat_bubble_labels_test` 断言"整个不画"；`chat_notice_test` 行为断言）。
2. 发现 **6 条文档债**（§2）与 **4 件待裁决**（§3），已写进本文。
3. 产出全库审计提示词（`docs/plans/AUDIT-PROMPT-whole-repo-2026-09-28.md`）。

---

## 2. 必须修的 6 条文档债 —— **已于 `dde6b855` 全部修复**（保留原文作记录）

1. **`AGENTS.md:15` 的「当前版本」仍是 `0.2.0-rc.6`**（代码/README 已是 rc.7），且**变更历史没有 rc.7 条目**（`grep rc.7 AGENTS.md` 零命中）。
   → Stage C1 范围应从"补 rc.2/3/4"扩为：**当前版本 → rc.7 + 补 rc.5/rc.6/rc.7 三条变更历史**。
2. **`TRIAGE-0.2.0-audit-45-2026-09-28.md` §1 的「状态」列 45 行仍全是 `⏳ 未开始`**，与 §5（rc.6 关 11 / rc.7 关 5 / 顺延 29）矛盾 → 回填，或在 §1 顶部加拉链"状态以 §5 为准（§1 未回填）"。
3. **同一文件 §1 约 19 行的「一句话」被截断**（`F-0001-5`→"AppShell.o"、`F-0002-3`→"mai"、`F-0003-4`→"E"、`F-0005-1`→"「舞台被 Repai"、`F-0006-2`→"`co"、`F-0012-2`→"`LlmSectio"、`F-0017-1`→"后端 `Setti"、`F-0021-1`→"…`fo" 等）。成因=含反引号/竖线的原文直接进表格。
   → **从 `docs/audit/2026-09-28-frontend-nightly/FINDINGS.md` 的完整标题重生成**（转义 `|`）。
4. **`CHANGELOG.md` 自 `v0.1.0-rc.4` 起停更**（`grep -c '0\.2\.0' CHANGELOG.md` = 0）。"版本三处（+zh README）"的表述没错，但读者会以为文档线全同步 → C1 里明确 CHANGELOG 的去留（停用并加头注 / 或补齐）。
5. **`docs/releases/v0.3.0.md` 的收尾文字仍指向 `docs/releases/v0.2.0-rc.5.md`** 作"现行版本" → 改指 `AGENTS.md` 首屏。
6. **`BACKLOG-0.2.0-closeout-2026-09-28.md` §2 说"mod/l1-product 的语音 WIP 从未入库、本会话未取得"——不成立**：
   我 `git ls-files` 核到全部在位：`shell/flutter/lib/api/voice_api.dart`、`lib/voice/{voice_listen_controller,speech_recognizer,speech_recognizer_web,speech_recognizer_stub}.dart`、
   `lib/settings/mods/voice_input_panel.dart`，以及 3 个测试（含与 Rust `DEFAULT_WAKE_PHRASE` 对账的 `voice_wake_default_consistency_test.dart`）
   → 该条降级为「**已入库**」，不必再考古。

---

## 3. 待维护者裁决（4 件，含我的建议）

| # | 事项 | 建议 |
|---|---|---|
| ① | `/home/skystar/Live2D-Ai`（`mod/persona-polish`，**136 项脏改动 + 23 未跟踪**）——**归档已完成（`dde6b855`）**，还剩「删不删 worktree」待裁决 | **归档 → 再删**：`git -C -Ai diff > docs/legacy/-Ai-dirty-2026-09-28.patch`，未跟踪 23 项按清单打包（其中 6 项已在 rc.6 前并入 `-fe`，见 `docs/audit/2026-09-28-frontend-nightly/README.md`），提交后再 `worktree remove`，**保留分支**。删前确认会话已切到 `-fe`（它曾是会话工作目录）。 |
| ② | `/home/skystar/Live2D-Ai-product`（1 份唯一文档 `../legacy/plans/PRODUCT-L1-GOALS-2026-09-15.md`，与 `-fe` 同名文件 `cmp` 不同） | 先 `diff` 两版 → 并入 `-fe` 或移 `docs/legacy/` → 再删 worktree。1 份文档不值得留一棵树。 |
| ③ | 音量滑杆"松手生效"是否可接受 | **可接受**，但拆成两件事：**(a) 拖动中读数必须跟手**（保留）；**(b) 落盘/下发去抖**（`onChangeEnd` 或 120–200ms）。并执行 `BACKLOG` §5 自己的提醒：若 `field_row.SliderField` 补 `onChangeEnd`，**删掉 rc.6 在 appearance 区自建的防抖层**，避免双层。 |
| ④ | rc.8 边界是否按 TRIAGE §5.1 切 | **同意**，但再切一刀：`rc.8-a = 正确性与诚实性`（3 条 P1 + 组 C/D/E/F），`rc.8-b = 结构`（C1 + C3 + W4）。理由与原口径一致：重构与修 bug 混版，出问题无法归因。 |

**仍未裁决的既有项**：表演层「说话权归属」（V12/Q1）——本轮**未遇到、未裁决**，不得由 worker 自行裁决。

---

## 4. 下一步计划（照做即可）

### Step 0 · 文档债（§2 六条）+ 热补丁（~1 小时）
- ~~§2 六条一次性 docs-only 提交~~ → **已由 `dde6b855` 完成**（见 §2 顶部补记；该提交逐字未碰代码，`git show --name-only` 无 `.dart/.rs/.toml/.yaml`）；
- **`F-0001-1`**（错误横幅"去设置"不开面板）：`main.dart` 的 `_gotoSection` 后补 `_shellKey.currentState?.openSettings()`（~3 行）+ 1 条可失败回归，独立 `fix(...)` 提交。成本极低，不必占 rc.8 波次。

### Step 1 · `rc.8-a` 正确性与诚实性（每条 P1 必须有红-绿双向）
| 波次 | 内容 | 文件归属 |
|---|---|---|
| R8a-1（独占） | `F-0008-1` 心跳看门狗 + `F-0007-1` 本地失败不回落相位（同一"幽灵态"两半，**同波次做**，判据抽零依赖纯函数） | `api/ws_client.dart`、`api/ws_liveness.dart`、`chat/chat_controller.dart`、`state/ui_state_tracker.dart` |
| R8a-2 | `F-0012-1` + `F-0003-2`（自检成败禁止扫 `contains('ok'/'ms')`；成功结果必须有渲染槽） | `settings/sections/llm_section.dart`、`tts_section.dart`、`ui/field_row.dart`、`main.dart` |
| R8a-3 | 组 C：`F-0005-4`（气泡 `excludeSemantics` 吃掉重试/复制）、`F-0003-6`（`_ModConfigTile` re-seed 吞草稿） | `ui/message_bubble.dart`、`settings/sections/dev_tools_section.dart` |
| R8a-4 | 组 D：`F-0006-2`（contentFaint 当文字色，白主题 3.96<AA）、`F-0006-3`、`F-0005-5`（字体门禁 | 运行时文本） | `design/tokens.dart`、`ui/theme.dart`、`test/design_tokens_lint_test.dart`、`test/font_subset_test.dart` |
| R8a-5 | 组 E：`F-0010-2` / `F-0001-4`（progress/fps 无出口）、`F-0004-1`（记忆面板跨桶） | `live2d/live2d_bridge.dart`、`live2d/live2d_stage.dart`、`settings/mods/memory_panel.dart` |

### Step 2 · `rc.8-b` 结构
- C1：AGENTS 单一化（含 §2 第 1 条）+ `v0.3.0.md` 收尾 + `asset_guard_director_cue_test` 头注漂移行号；
- C3：拆 4 个 >1000 行文件（`dev_tools_section` 1874 / `appearance_background` 1551 / `main` 1344 / `display_prefs` 1157）+ 2 个 >800 行测试；
  ★ `main.dart` 拆分时**同波次**做 `F-0005-2` 的"改法 B"（外壳不再整体订阅 `_chat`），**但必须两个 commit**（重构与行为分开，保证可归因）；
- W4 五项裁决（可计算配色 / 自定义 accent / 三档密度 / 风格预设 / 设置区搜索框）：要么做，要么**正式减记**。

### Step 3 · 并行只读全库审计轨（**与实施者分离**）
- 提示词：`docs/plans/AUDIT-PROMPT-whole-repo-2026-09-28.md`（整块粘贴即可，弱模型可用）；
- 账本：`AUDIT-REPO/`（`-fe` 根下，未跟踪，**永不 `git add`**）；
- 优先目标：`crates/**`（291 文件 / 97.5k 行，**从未被系统审计**）→ `v0.2.0-rc.5..HEAD` 的前端增量（rc.6/rc.7 从未审过）→ 前端存量未覆盖项 → 端到端链路 → 对抗日。

### Step 4 · `0.2.0` 末版（Stage D）
```bash
# main 是 feat/frontend-redesign 的祖先 ⇒ 可快进合流，无需解冲突
cd /home/skystar/Live2D-Ai-fe && git checkout main && git merge --ff-only feat/frontend-redesign
```
版本三处（`Cargo.toml` / `pubspec.yaml` / `README`×2）同步 → 完整门禁 → **维护者肉眼 8 项**（见 §5）→ tag → release note。**按维护者口径：不推远端。**

### Step 5 · 已与编排者对齐的 rc.8 形态（2026-09-28 22:55，逐条拍板）

1. **Stage D 第一条 = `main` ff-only 合流**（`main` 是 `-fe` 祖先，零冲突路径）；
2. `F-0001-1` 做成**独立 `fix(...)` 热补丁**，tag 跟 rc.8（`errorActionsFor` 路径此前零覆盖，顺手补上）；
3. **`F-0008-1` + `F-0007-1` 必须同波次**——只修一半会把症状从「永久思考中」变成「永久无提示」；
4. **rc.8 再切 a/b**：`rc.8-a` 正确性与诚实性、`rc.8-b` 结构（接受"修 bug 与重命名也应分开"的理由）；
5. **并行只读审计轨**（全库提示词 + BATCH-0022 缺口矩阵 + Phase 3/4）**审计者与实施者分离**；
6. **肉眼验收落成 `docs/verification/*checklist*.md` 可勾选文件**（含逐档 `fit/tile`），并截图归档——不要再把结论只写在发布说明里；
7. **磁盘检查变成可执行**：把「worktree 数 / `df` 余量」加进 `scripts/ignite.sh --check`（`REPO-HYGIENE` 里的文字纪律不算闭环）。

**仍未开始**：`rc.8-a` / `rc.8-b` / 0.2.0 末版。3 条 P1 **没有被"顺手做掉"**——按本文件 §4 计划，它们是 rc.8 第一波，且必须带红-绿双向与非实施者复核（这符合维护者「主计划 + 审查」双轨口径）。

---

## 5. 未验收 / 不可自动化的部分（**不许伪造绿灯**）

1. **Win 浏览器肉眼 8 项**：背景选图→清图→刷新仍在 / 四档 fit 逐档（`tile` 只有算术与结构级断言，**无像素级回归**）/ 逐图样式 / 轮播第 2 张 / 预览可达 / 拖动排序 / 两轮播分工 / 动作·语音·口型·导演四栏。
2. 服务已在 `127.0.0.1:18080`（`GET /app/` 200）。
3. **本机无 CDP/Chrome 可交互取证**、**无 Live2D 模型**（`bai.model3.json` 404）、**无 TTS 端点** ⇒ 资产回归只能人工。
4. 审计侧仍标「未核实/中」的幅度类结论（`F-0005-2` 幅度、`F-0006-1` 耗时、`F-0005-5` 触发条件、`F-0009-1` 概率）**不得当实测值引用**。
5. **验收清单文件尚不存在**（见 Step 5 第 6 条）：需要在 rc.8 里落 `docs/verification/v0.2.0-rc.8-checklist.md`（背景 8 项 + 四档 fit/tile 逐档 + 资产四栏），否则每版都会重述一遍「未做肉眼」。

---

## 6. 已知坑（别踩）

1. **不要在 `/home/skystar/Live2D-Ai` 上开发**——旧基线、136 脏；它的唯一副本已在 2026-09-28 并入 `-fe`（`31d06cb4`），**不要再从它快照**。
2. `-fe` 的 `build/web` 是**产物**：只要改 `.dart` 必须 `flutter build web --release --base-href /app/ --no-web-resources-cdn`；改 `crates/l2d-wasm-demo/**` 必须 `trunk build`。
3. **门禁命令会写盘**：`cargo`/ `flutter` 会写 `target/`、`.dart_tool/`、`build/`。本轮曾撞 **ENOSPC**（147G 只剩 48M），根因是 24 个 worktree 的 `target/`；现已 3 个 worktree / 37G 可用。**大构建前先 `df -h`。**
4. 审计账本分两处，别混：前端既有账本 `docs/audit/2026-09-28-frontend-nightly/`（**已入库、封口只读**）；新全库账本 `AUDIT-REPO/`（未跟踪）。
5. 破坏性 git 操作前**先打 tag**；worker 不得做 git 破坏性操作。
6. 表演层"说话权归属"（V12/Q1）未裁决；不删 WS 帧、不改既有帧字段名；`clean_for_tts` 不得旁路。

---

## 7. 关键文件索引

| 用途 | 路径 |
|---|---|
| 规划真源（Stage B 已接地） | `docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md` |
| 调度真源（2026-09-28 版） | `docs/plans/ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md` |
| 45 条分派对账（注意 §2 的状态列与截断） | `docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md` |
| 挂账清单 | `docs/plans/BACKLOG-0.2.0-closeout-2026-09-28.md` |
| 前端审计账本（封口只读） | `docs/audit/2026-09-28-frontend-nightly/` |
| rc.6/rc.7 施工接地（行号/文件） | `docs/audit/2026-09-28-rc7/GROUNDING.md` |
| 仓库卫生对账 | `docs/audit/2026-09-28-rc7/REPO-HYGIENE.md` |
| 发布说明 | `docs/releases/v0.2.0-rc.6.md`、`v0.2.0-rc.7.md` |
| **全库审计提示词（本轮新增）** | `docs/plans/AUDIT-PROMPT-whole-repo-2026-09-28.md` |
| Mod 侧审查 / 调研（0.2.0 线） | `docs/audit/2026-09-15-five-mod-review.md`、`docs/research/2026-09-15-mod-chains-perf-and-single-session-memory.md` |
| 动作/语音/记忆选型（未实施） | `docs/plans/PLAN-actions-voice-memory-2026-09-15.md` |

---

## 8. 本次未提交（接手第一件事：docs-only 提交）

```
 M docs/README.md                                              # 索引新增两条（指向下面两份）
?? docs/plans/AUDIT-PROMPT-whole-repo-2026-09-28.md            # 全代码库审计提示词（整块粘贴用）
?? docs/plans/HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md    # 本文
```

建议提交信息：`docs(handoff): 全库审计提示词 + rc.7 接手状态（只读复核会话产出）`。
**提交后 `-fe` 才回到 clean**；在此之前不要在这些文件上做破坏性 git 操作。
