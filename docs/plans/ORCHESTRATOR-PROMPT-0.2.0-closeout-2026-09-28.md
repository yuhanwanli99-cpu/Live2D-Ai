# 调度提示词（Orchestrator）：0.2.0 收尾 —— rc.6 背景 → rc.7 技术债 →（按需 rc.8+）→ 0.2.0 末版

> 本文是**编排者**（你）的执行手册，**2026-09-28 经维护者确认口径后落盘**。
> 规划真源：`docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md`（Stage B 已按 rc.5 实测接地）。
> 实现提示词：`docs/plans/IMPL-PROMPTS-0.2.0-seal-2026-09-27.md`（Stage B 已接地；Stage C/D **待重新接地**）。
> 审查证据：`AUDIT-B/`（21 批，**为准**）+ `AUDIT/`（4 批，短跑快照）——尚未入库，见 §2。
> 本文**取代** `ORCHESTRATOR-PROMPT-0.2.0-seal-2026-09-27.md` 的**调度部分**；那份保留为 Stage A 的历史记录。

---

## 0. 维护者口径（2026-09-28，逐条照做，不要重新演绎）

1. **个人小项目，产物存本地即可**：不把「推远端 / 备份」当阶段目标。每阶段打**本地 tag / 留还原点**即可；
   但**不要**把「本地」理解成「可以丢」——破坏性 git 操作前一律先建 tag。
2. **rc.6 = 完成背景**：Stage B 全部内容（fit 四档 + `tileSize`、逐图样式、全局 enabled、DEC-1…DEC-7）
   **加上「背景域」的审计发现**（§3.1 判定表）。
3. **rc.7 = 收尾技术债**：Stage C（文档单一化 / 仓库卫生 / 结构拆分 / W4）+ **其余审计发现**（§3.2）。
4. **如需要可多版本可自取**：不要求把全部债务硬塞进 rc.7。rc.7 之后若仍有余量，**自行开 rc.8 / rc.9（或 rc.7.x）**
   继续，直到债务清零，再发 **0.2.0 末版**。宁可多一版，不要把不相关的风险挤进同一版。
5. **重要资产必须守护**（用户点名）：导演层接线的整套 Live2D 表演 + TTS 过滤（`PLAN` §3.1 的 A1–A8、§3.2 的六条红线）。
6. **不做的事**：不改后端来完成背景能力（舞台分区背景 / 舞台模糊 / 正文帧带 `sentence_seq` 只记录）；
   不自行裁决表演层「说话权归属」（V12/Q1）。

---

## 1. 环境事实（先读，别踩）

| 事实 | 值 |
|---|---|
| 工作树 | `/home/skystar/Live2D-Ai-fe` · 分支 `feat/frontend-redesign` · HEAD `5ef879f4` |
| 0.2.0 已提交基线（main） | `e4f139a8` **v0.2.0-rc.4**（2026-09-26） |
| 相对关系 | `feat/frontend-redesign` 领先 `main` **9 个提交**（Stage A rc.5 + 4 个 docs） |
| `origin/main` | `2d492447` **v0.2.0-rc.1**，落后本地 85 个提交——**按口径不管它**，但本地 tag 要打 |
| 仓库卫生 | **24 个 worktree**、1 个 stash；`docs/releases/v0.3.0.md` 与 rc 线矛盾 |
| 旧 worktree | `/home/skystar/Live2D-Ai`（`mod/persona-polish` @ `4e421993`，**main 的祖先**，落后 84）——里面有 **133 项脏改动**与一批**只此一份**的未跟踪文档 |
| 审计账本 | `/home/skystar/Live2D-Ai-fe/AUDIT/`、`AUDIT-B/`，**均未跟踪**；本会话已确认无 agent 在写 |
| rc.5 门禁基线 | cargo test **1457/0**、doc **3**、fmt clean、clippy **0 warning**、rust-ratio **97.3595% PASS**；flutter analyze **0 issue**、flutter test **1272**；build FRESH、gstatic **0/0**；`ignite.sh --check` **四项全 ok** |

**最容易踩的四个坑**
1. **不要在 `/home/skystar/Live2D-Ai` 上开发**——它是旧基线；它的脏副本是重设计的旧源，**已被重放到 -fe 并提交**（A2 `8bc1eaee`）。
2. **不要从 `-Ai` 重新快照那三份封口文档**——-fe 里的是接地版，旧副本会把它覆盖回去（`HANDOFF` §3.1）。
3. **`AUDIT/` 与 `AUDIT-B/` 是唯一副本**，Stage C 的「未跟踪清零」会误删它们——**先入库再清零**（§2.1）。
4. **只要改 `.dart`，必须重建前端**才能在 `/app/` 看到（`flutter build web --release --base-href /app/ --no-web-resources-cdn`）；改 `crates/l2d-wasm-demo/**` 必须 `trunk build`。

---

## 2. 开工前（你亲自做，不派 worker）

### 2.1 固定证据（先于一切代码改动）
1. **审计入库**：把 `AUDIT/` 与 `AUDIT-B/` 归并为 `docs/audit/2026-09-28-frontend-nightly/`：
   - `AUDIT-B/` 全量入（21 批 + 2 份 CONSOLIDATION + FINDINGS + INDEX + STATE + NEXT + WIP）；
   - `AUDIT/` 保留为 `short-run/` 子目录，并在 README 注明「短跑快照，以 AUDIT-B 为准」（两者 0001–0004 字节相同）；
   - 提交（docs-only），此后审计目录**从「未跟踪」转为「已跟踪」**，Stage C 清零才安全。
2. **旧 worktree 唯一副本入库**：逐份判定下面 5 份**只存在于 `-Ai` 未跟踪**的文档——
   - `PLAN-visual-substance-2026-09-27.md`（其实施已在 rc.5 前进入重设计）→ 并入 0.2.0 线或 `docs/legacy/`；
   - `PLAN-frontend-redesign-2026-09-27.md`（重设计的源计划，A2 依据）→ **建议并入** `docs/plans/` 作为 A2 的溯源；
   - `PLAN-frontend-eval-and-improvements-2026-09-27.md`（文件头已自标「已被取代」）→ `docs/legacy/`；
   - `docs/audit/2026-09-15-five-mod-review.md` + `docs/research/2026-09-15-mod-chains-perf-and-single-session-memory.md`（属于 0.2.0 线的 Mod 侧审查/调研）→ **并入** `docs/audit/`、`docs/research/`。
   判定原则：**能接进 0.2.0 线的就并入，不能的进 `docs/legacy/` 并加「历史」标注**；不要停在未跟踪状态。
3. **本地还原点**：给 `feat/frontend-redesign` 当前 HEAD 打本地 tag（如 `checkpoint/rc5-pre-rc6`）；rc.6/rc.7 每版收口各打 `v0.2.0-rc.N`。
4. **文档小修正**：`PLAN-0.2.0-seal-and-cleanup-2026-09-27.md` 的 Stage C 标题**重复出现两次**（242–243 行），删掉一行。

### 2.2 计划复核
- 读 `PLAN` §3（资产）、§4（债务分级）、§6（Stage B 接地版 + B.0 的 DEC-1…DEC-7）；
- 读 `IMPL-PROMPTS` 的 **B-a / B-b / B-c / B-d** 四块（Stage B 用这四块，旧的 B1–B4 已废弃）；
- 读 `HANDOFF-2026-09-27-stageB-plan-and-status.md` §2（未完成）与 §3（已知坑）。

### 2.3 记录对照基线
- 在干净 HEAD 上跑一遍完整门禁（§6 的全部命令），把原始输出存进 `docs/releases/v0.2.0-rc.6.md` 草稿；**rc.6 的门禁数字不得低于 rc.5**。

---

## 3. 审计发现分派（Triage 规则）

**规则一句话：域属「背景」→ rc.6；其余 → rc.7。** 依据是维护者「rc.6 完成背景」的口径，
而不是严重度——同一批文件一次改完，不要把冲突留给自己。**P0 = 0**（审计 7 批 58 源文件 + 34 测试文件，未见数据丢失/权限绕过/离线红线被破/注入面）。

### 3.1 → rc.6（背景域，与 Stage B 同文件批）

| ID | 等级 | 一句话 | 落点文件 |
|---|---|---|---|
| F-0001-2 | P2 | 轮播双索引漂移：预览不驱动 `jumpTo`；删/清空库不重置运行时索引 | `main.dart` / `app_shell.dart` / `shell_slideshow.dart`（与 DEC-6 合并） |
| F-0001-3 | P2 | 启动水合竞态：用旧 prefs 快照整体覆盖，吞掉窗口期改动 | `main.dart` `_hydrateBackgrounds` |
| F-0013-1 | **P1** | 水合窗口内导入的图「先说已加入、随后凭空消失」，删除/重排/偏好一并回滚（F-0001-3 的升级） | `main.dart` / `background_hydration.dart` |
| F-0002-2 | P2 | 早期加图写进占位内存库，「已记住」是假话 | `main.dart` 的 `_store` 接线 |
| F-0002-3 | P3 | `_openStore` 超时/退路在 web 上是死代码，注释与真实兜底不符 | `main.dart` |
| F-0002-1 | **P1** | 偏好一变就全量重发 stage-bg + 全量落盘；滑杆每像素一次 | `shell_prefs.dart` / `live2d_bridge.dart` / `audio_bar.dart` / `main.dart` |
| F-0003-1 / F-0006-1 | **P1** | 壳背景图在**每个 delta** 上 base64Decode + 整图重解码（MemoryImage 恒 cache-miss） | `shell_backdrop.dart` / `app_shell.dart` |
| F-0016-1 | P2 | 背景库缩略图每次 build 重解码（与 F-0006-1 同根因） | `appearance_section.dart` |
| F-0003-3 | P2 | 背景库批量删除按「下标」选删，重排后删错图 | `appearance_section.dart` |
| F-0001-5 | P3 | `AppShell.onBackgroundIndex/onBackgroundJump` 死参数 + 注释指向不存在的 `ShellBackgroundHost` | `app_shell.dart` |

**rc.6 的收口判据**（在 Stage B 原有判据上追加）：
- 背景域上述 10 条**逐条关闭或写明不做 + 理由**；两条 P1（F-0002-1、F-0006-1）**必须有可失败的回归**；
- 轮播「控件说的 = 画面做的」；预览在**任意库大小**可达；
- `DEC-1…DEC-7` 逐条落地；`stagePlaylist` 若保留则**先补守护测试**（rc.5 §9.2 为零覆盖）；
- 门禁数字 ≥ rc.5（cargo 1457 / flutter 1272）。

### 3.2 → rc.7（技术债 + 其余审计发现）

**rc.7 的第一优先级 = 审计的「主发现」**（`CONSOLIDATION-02` §3 判定的放大器）：

| 组 | 主发现 | 等级 | 说明 |
|---|---|---|---|
| A 重建放大链 | **F-0005-2** 每个 `text_delta` 重建整棵 AppShell 子树 | P1 | **最有价值的单点**——修它同时降 F-0006-1/F-0016-1/F-0006-4 的成本（背景那两条已在 rc.6 单独收） |
| B 假绿灯 | F-0005-1 / F-0005-3 / F-0005-6 / F-0005-7 | P1/P2 | 4 条「删掉实现测试仍绿」；**按测试修复处理，不按业务 bug**；给所有源码扫描式守卫加「判别力自证」 |
| C 接缝 | F-0007-1 / F-0005-4 / F-0003-6 | P1/P2 | 两侧组件各有真断言，接缝无人测；对策=判据抽零依赖纯函数 |
| D 门禁覆盖 | F-0006-2 / F-0006-3 / F-0005-5 | P2/P3 | 门禁覆盖面 < 规则适用范围 |
| E 派生值 | F-0010-2 / F-0001-4 / F-0004-1 | P1/P2 | 通知没人读 / 服务端快照不刷新 |
| F 文案猜语义 | F-0012-1 / F-0003-2 / F-0003-4 | **P1** | 自检成败扫 `contains('ok'/'ms')`；成功结果根本不渲染 |

**其余按序**（逐条判定「修 / 明确不做 + 理由」）：
F-0001-1（错误横幅不开面板，P1）· F-0007-2（打断了不重发）· F-0008-1（无心跳看门狗，P1）·
F-0004-1（记忆面板不随会话桶，P2，**可能跨桶误删**）· F-0004-2 / F-0004-3 · F-0003-5 ·
F-0006-4 · F-0009-1 · F-0010-1 · F-0011-1 · F-0012-2（`LlmSection` 零覆盖）· F-0015-1 / F-0015-2 ·
F-0017-1（后端 `performance` 段前端零解析，P2）· F-0018-1 · F-0019-1 · F-0021-1（行内码无 `fontFamilyFallback`，红线 L 破口）·
以及 `AUDIT/` 独有的编号（若与 AUDIT-B 不同）。

**再叠加 Stage C（既有计划）**：
- **C1** 文档单一化：`AGENTS.md` 以 main 版为底（含「动作与表演现行状态 / 导演可观测 / 每皮套动作幅度」），版本号更新到当前线；`docs/releases/v0.3.0.md` 标注「历史草案，未发布」或归档；Stage C/D 提示词**重新接地**（行号已漂：`appearance_section` 1489、`dev_tools_section` 1874、`main.dart` 1238、`display_prefs` 1002；测试 `display_prefs_test` 882、`memory_panel_test` 852）。
- **C2** 仓库卫生（**只有你能做，破坏性 git**）：24 个 worktree / 1 个 stash / 未跟踪文档逐个判定——`git branch --merged main` 的移除+删分支，未并入的写明为什么活着；清零前确认 §2.1 已入库。
- **C3** 结构：`appearance_section.dart` 拆分（≥1489 行，超 `≤500` 且不满足 `≤1000` 豁免带）+ `GroupCard` 推广 + **W4 五项裁决**（可计算配色 / 自定义 accent / 三档密度 / 风格预设 / 设置区搜索框）。

**rc.7 收口判据**：§4 的 P1/P2/P3 **逐条关闭或写明不做 + 理由**；4 条假绿灯**已改为可失败断言**（红-绿双向演示）；结构文件回到 `≤500`（或按豁免头注 ≤1000）；worktree/stash/未跟踪清零；门禁数字 ≥ rc.6。

### 3.3 版本不够就继续开
若 rc.7 同时扛「审计债务 + Stage C 三项」超载，按**主题**拆版（推荐顺序）：
- `rc.7` = 正确性与诚实性（组 A/B/C/D/E/F）+ 4 条假绿灯；
- `rc.8` = 结构与仓库卫生（C1/C2/C3 + W4 + 超长文件）；
- 仍有余量则 `rc.9`……直到清零。**每版都必须完整收口（门禁 + 肉眼）再进下一版**，不留「半版」。

---

## 4. 波次编排（并发铁律：文件零重叠才可并发）

### rc.6（背景）
| 波次 | 任务 | 独占文件 | 并发性 |
|---|---|---|---|
| **R6-0** | §2 开工前（你） | — | 单独 |
| **R6-a** | Stage B 模型/渲染：fit 四档+tileSize、逐图样式字段、全局 enabled、DEC-1/DEC-5 | `display_prefs.dart`、`shell_backdrop.dart`、`background_item.dart` | 先做 |
| **R6-a2** | 背景域审计 P1：F-0002-1（去重+防抖）、F-0006-1（解码备忘化）、F-0013-1（水合/store 窗口） | `shell_prefs.dart`、`main.dart`、`background_hydration.dart` | **等 R6-a**（同文件族） |
| **R6-b** | Stage B UI + DEC-2/DEC-6/DEC-7 + F-0001-2/F-0003-3/F-0016-1 | `appearance_section.dart`（＋新 `appearance_background.dart`） | **等 R6-a2** |
| **R6-c** | parity 文档回填 + `docs/README.md` 索引（docs-only） | `docs/**` | 可并行 |
| **R6-d** | 收口：门禁 + Win 肉眼（四档逐档 / tile / 预览 / 拖动 / 两轮播分工）+ rc.6 说明 | — | 最后 |

### rc.7（技术债）
| 波次 | 任务 | 并发性 |
|---|---|---|
| **R7-0** | Stage C/D 提示词重新接地（docs-only） | 单独，先做 |
| **R7-a** | 审计主发现 **F-0005-2**（外壳订阅收窄） | **独占**（放大器，先修） |
| **R7-b** | 假绿灯四条 + 判别力自证 | 可与 R7-a 并行（多为 test/） |
| **R7-c1/c2/c3** | 正确性与接缝按文件切分（F-0007-1/F-0008-1/F-0012-1+F-0003-2／F-0001-1/F-0004-1／F-0017-1/F-0021-1） | 文件零重叠可 3 路 |
| **R7-d** | Stage C1 文档单一化（docs-only） | 可与 c 组并行 |
| **R7-e** | Stage C3 结构拆分 + W4 | **等 R7-c 收口**（都碰 settings/） |
| **R7-f** | Stage C2 仓库卫生（**只有你能做**）+ 收口 | 最后 |

> 若按 §3.3 拆 rc.8：R7-f 与 C1/C2/C3 移到 rc.8，rc.7 只留 R7-a…d。

---

## 5. 子 worker 的调用规范

- 每个 worker **一块提示词**（IMPL-PROMPTS 整块粘贴），**不要把多块塞给同一个 worker**。
- 每个 worker 开工前必须读规划里对应那一节；提示词里已写明，你只需确认。
- worker **不得**做 git 破坏性操作（建/删 worktree、tag、reset、stash）；需要时**回报给你**，由你做。
- worker 回报必须含：**改动文件清单 / 测试名 / 门禁原始数字 / 未决问题**，缺任一项打回。
- 同一波次不要在同一棵工作树上并发改同一个文件；每个任务块末尾都写了【文件归属】，按它分派。
- 源码 **≤500 行**（豁免 ≤1000 需头注理由）；**禁止**仓库级 `dart format`。

---

## 6. 集成与门禁（每个波次收口时你亲自跑，串行）

```bash
cd /home/skystar/Live2D-Ai-fe/shell/flutter
export PATH="$HOME/flutter/bin:$PATH"
flutter analyze                          # 必须 0 issue
flutter test                             # 必须全绿，计数 ≥ rc.5 的 1272；不含 zz_
flutter build web --release --base-href /app/ --no-web-resources-cdn
cd /home/skystar/Live2D-Ai-fe && ./scripts/ignite.sh --check    # 四项全 ok

# 只有碰过 Rust 才跑（本路线预期不碰 crates/**）：
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio         # ≥95%
```

**产物新鲜度**（本项目踩过两次）：
- 有 `.dart` 改动 → `build/web` 必须重建，且 `main.dart.js` 的 mtime **晚于**最后一个被改的 `.dart`；
- 有 `crates/l2d-wasm-demo/**` 改动 → `trunk build`，且 `dist/` 的 mtime 晚于最后一个被改的 `.rs`。

---

## 7. 独立核验（不采信 worker 自述，逐条贴原始输出）

1. **资产守护断言**：故意注释掉 `main.dart` 里一处 `_applyDirectorCueForSeq` 调用，确认守护断言**变红**，再还原。**这是整轮最重要的核验。**
2. **背景 P1 回归**：F-0006-1——同一 dataUrl 两次解码得到不同 bytes ⇒ `MemoryImage` 恒 miss 的最小证明 + 备忘化后增量为 0；F-0002-1——注入假 transport，`_updatePrefs(volume)` 两次断言 stage-bg 增量为 0（现状 2）；F-0013-1——水合中改音量，断言 prefs **原样**、`retainOnly` **未被调用**。
3. **假绿灯的判别力自证**：删掉被测实现，确认对应测试**变红**（F-0005-1/3/6/7 四条各做一次）。
4. **测试计数账目**：确认没有静默删除（rc.5 的 `1232+13=1245`、`1245+27=1272` 是可复制的对账方法）。
5. **grep 死代码**：rc.6/rc.7 声称删掉的符号全仓零引用。
6. **肉眼**：Win 浏览器按 checklist 走一遍（背景选图→可见→清图→刷新仍在；四档 fit/tile 逐档；拖动排序；预览可达；两轮播分工；动作/语音/口型/导演可观测）。

---

## 8. 失败与边界

- **撞到未裁决项就停**：表演层「说话权归属」（V12/Q1）**不得自行裁决**，写清问题交维护者。
- **两行意图不兼容**（重设计 vs main 的新能力）→ 停，记录，**不要发明第三种语义**。
- **门禁红** → 不许把红说成「已知问题」蒙过去；修，或明确记录为阻塞项并回报。
- **任何需要改后端才能完成的背景能力**（舞台分区背景 / 舞台模糊 / 正文帧带 `sentence_seq`）→ **只记录**为未来工作。
- **审计发现与实现冲突时**：回到源码重读再判定，**不辩护、不硬套**（审计自己的纪律：笔记会腐烂，代码在变）。
- **审计里标「未核实/中」的幅度类结论**（F-0005-2 幅度、F-0006-1 耗时、F-0005-5 触发条件、F-0009-1 概率）→ 能在真机量就量，不能就保留标注，**不要伪装成已实测**。

---

## 9. 最终交付（给维护者）

1. 各 rc 的交付报告：改了什么 / 门禁原始数字 / 肉眼结论 / 未决问题。
2. 资产守护断言与 4 条假绿灯的**红-绿双向**演示结果。
3. 审计 ~45 条的**分派对账表**（每条：rc.6 / rc.7 / rc.8 / 不做+理由 / 状态）。
4. 仓库卫生前后对比（worktree 数、stash、未跟踪文档数）。
5. **0.2.0 末版**的 release note + 本地 tag + 完整门禁 + 肉眼 checklist。

---

## 10. 启动提示词（维护者直接粘贴给编排者的那一段）

```
你是本轮的**编排者**（Orchestrator），不要自己写业务实现。
必读（按顺序）：
  1) docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md（Stage B 已接地）
  2) docs/plans/IMPL-PROMPTS-0.2.0-seal-2026-09-27.md（Stage B 用 B-a/B-b/B-c/B-d）
  3) docs/plans/ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md（本文）
  4) docs/plans/HANDOFF-2026-09-27-stageB-plan-and-status.md
维护者口径（2026-09-28）：个人小项目、产物存本地即可（不推远端，但每版打本地 tag）；
rc.6 = 完成背景（Stage B + 背景域审计发现）；rc.7 = 收尾技术债（Stage C + 其余审计发现）；
如需要可多开版本（rc.8/rc.9 自取），宁多一版不硬塞。
第一步（你亲自做，不要派）：按本文 §2 固定证据——把 AUDIT/ 与 AUDIT-B/ 入库到
  docs/audit/2026-09-28-frontend-nightly/（AUDIT-B 为准，AUDIT 标注短跑快照）、把 -Ai 里 5 份
  唯一未跟踪文档判定并入、给当前 HEAD 打本地还原点、修掉 PLAN 里重复的 Stage C 标题；
  然后记录 rc.5 门禁基线（cargo 1457 / flutter 1272）。
第二步：rc.6 按 R6-a → R6-a2 → R6-b（R6-c 可并行）→ R6-d 推进，主题只有一个词：背景。
第三步：rc.6 收口后再进 rc.7：先 R7-a（F-0005-2 独占），再假绿灯与正确性分派，最后 Stage C。
每个波次收口你亲自跑门禁并做 §7 的独立核验（尤其「故意弄红」那条）。
硬约束：worker 禁止 git 破坏性操作；源码 ≤500 行；禁止仓库级 dart format；
不删 WS 帧/不改既有帧字段名；clean_for_tts 不得旁路；不自行裁决表演层说话权归属。
遇到未裁决项或两行意图不兼容 → 停下记录，不要自己发明语义。
```
