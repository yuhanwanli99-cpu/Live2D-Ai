# HANDOFF：0.2.0 封口 —— 当前状态与未完成项（2026-09-27）

> 给下一个接手 agent。本文件是**状态快照**，不是新规划。
> 规划真源：`docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md`；
> 调度：`docs/plans/ORCHESTRATOR-PROMPT-0.2.0-seal-2026-09-27.md`；
> 实现提示词：`docs/plans/IMPL-PROMPTS-0.2.0-seal-2026-09-27.md`。

---

## 0. 一分钟上手

- **0.2.0 线** = `main`（已提交到 `v0.2.0-rc.4`）；**rc.5 已在 `feat/frontend-redesign` 上收口**（`01ea2af1`）。
- **下一步 = Stage B（rc.6）**：背景透传追平 shalldie/vscode-background v3.1.0。
- 工作树：`/home/skystar/Live2D-Ai-fe`（分支 `feat/frontend-redesign`）。
- 服务：`127.0.0.1:18080`（在 `-fe` 下用 `./scripts/ignite.sh` 起）。
- 已开工的 roadmap：`rc.5 资产+集成+P0` ✅ → `rc.6 背景透传` **← 下一步** → `rc.7 技术债+项目管理` → `0.2.0 末版`。

---

## 1. 已完成

### 1.1 Stage A · `0.2.0-rc.5`（已提交、已收口）

提交链：`e4f139a8`(main/rc.4) → `639a70db`(A1) → `99e04006`(lock) → `8bc1eaee`(A2 重放) →
`74b38706`(A3a/A3b) → `6f7a2069`(A3c/A3b2) → **`01ea2af1`(rc.5)**。每波都留了还原点。

**门禁（rc.5 实测）**：cargo test **1457/0**、doc **3**、fmt clean、clippy **0 warning**、rust-ratio **97.3595% PASS**；
flutter analyze **0 issue**、flutter test **1272**（main 1091 → +181）；build FRESH + gstatic **0/0**；`ignite.sh --check` **四项全 ok**。

**交付内容**：A1 资产守护网（19 条 + `docs/architecture/frontend-asset-inventory.md`）；A2 重放 112 个 Flutter 文件（29 冲突）；
背景 P0-1…P0-5 全修；P1/P2 清死代码与文档腐烂；15 条审计复现转正；**B4 偏离说明已提前完成**
（`docs/architecture/background-parity-vscode-background.md`，412 行）。

**最有价值的两条证据**（详见 `docs/releases/v0.2.0-rc.5.md` §1/§3）：
1. 守护断言的**红-绿双向演示**：注释掉 `main.dart` 的导演 cue 调用点 → 新断言变红（`+2 -2`），而**同一现场**下旧 `action_cue_test.dart` 仍全绿（`+11`）⇒「旧断言只查名字」被实证。
2. 测试计数账目分文不差：`1232 + 13 = 1245`、`1245 + 27 = 1272`，**无静默删除**。

### 1.2 Stage B 规划（本次落盘，尚未实施）

已把三份文档**基于 rc.5 实测重写**（只改 docs，未碰代码）：
- `PLAN-...md` §6 Stage B：新增 **B.0 前置裁决（DEC-1…DEC-7）** + **B.1 波次** + **B.2 收口判据**；
- `IMPL-PROMPTS-...md`：Stage B 换成接地版 **B-a / B-b / B-c / B-d**（原 B1–B4 已废弃，B4 已在 rc.5 完成）；
- `ORCHESTRATOR-PROMPT-...md`：Stage B 波次表同步为 B-a → B-b（B-c 可并行）→ B-d。

---

## 2. 未完成（下一步要做的）

### 2.1 Stage B · `0.2.0-rc.6`（**未开始**）

| 波次 | 任务 | 独占文件 | 并发性 |
|---|---|---|---|
| **B-a** | fit 四档＋`tileSize`、逐图样式字段、全局 `background.enabled`、DEC-1/DEC-5 | `settings/display_prefs.dart`、`ui/shell_backdrop.dart`、`design/background_item.dart` | 先做 |
| **B-b** | 铺法四档 UI、逐图样式编辑器、DEC-2/DEC-6/DEC-7、顺手抽背景块 | `settings/sections/appearance_section.dart`（＋新增 `appearance_background.dart`） | **等 B-a** |
| **B-c** | parity 文档回填（DEC-3 结论 /「已成现状」）、`docs/README.md` 索引 | `docs/**` | 可与 B-a/B-b 并行 |
| **B-d** | 门禁＋Win 肉眼＋rc.6 说明 | 无 | 最后 |

### 2.2 需要**维护者裁决**的 7 条（规划里已给建议，默认按建议执行）

- **DEC-1** `slideInterval` 越界语义：建议**端点夹持**（>300→300，<5→5；0 仍＝关）；**新增** `_clampIntToRange`，**不要改** `_clampInt`（scrim 依赖「越界回落默认」）。
- **DEC-2** 两套轮播（`stagePlaylist` vs 背景库轮播）：建议**不合并**，改「分工＋改名＋按来源互斥显示」；**保留即先补 `stagePlaylist` 守护测试**（rc.5 §9.2：目前零覆盖）。
- **DEC-3** 壳内子区域：建议**不做，写明理由**（避免堆砌），记 Stage C backlog。
- **DEC-4** 全局 `background.enabled`：建议**做**。
- **DEC-5** 坏 dataURL 仍 `isRenderable=true`：建议**收紧**到真 dataURL 形态。
- **DEC-6** `currentItemIsImage` 只看第 0 项：建议**用 AppShell 运行时 `_backgroundIndex` 判当前项**（不持久化）。
- **DEC-7** D3 位置显隐与注释相反 + D4 来源＝舞台时轮播控件空转：**都修**。

### 2.3 Stage C / Stage D 的提示词**尚未重新接地**

`IMPL-PROMPTS` 里的 Stage C（C1/C2/C3）与 Stage D 仍是 **rc.5 之前**写的版本。
Stage B 落地后需要重跑一遍「行号/文件名对账」，尤其：
- rc.5 §9.4 的**4 个超行数文件**：`appearance_section.dart` **1489**、`dev_tools_section.dart` **1874**、`main.dart` **1238**、`display_prefs.dart` **1002**；
- rc.5 §9.5 的 2 个测试文件超 800 行：`display_prefs_test.dart` **882**、`memory_panel_test.dart` **852**；
- rc.5 §9.3：`AGENTS.md` 的「当前版本」仍是 rc.4，归 Stage C1。

### 2.4 未验收项（**不伪造绿灯**）

1. **交互式背景 checklist**（选图→可见→清图→刷新仍在、拖动排序手感、预览可达）——Flutter CanvasKit 对 CDP 不暴露可交互无障碍树，只能人工在 Windows 浏览器走；
2. **资产回归**（皮套动作/表情、语音、口型、导演可观测四栏）——需 Live2D 模型（本仓不捆绑，实测 `/models/bai/runtime/bai.model3.json` **404**）＋ 活 TTS 端点（`live2d-ai.toml` 指向 `127.0.0.1:8080`，本机无服务）；
3. parity §8 的 **7 条未核实清单**（参考的运行时行为、`http://` 是否被 CSP 放行等）——只读了源码，**没有安装/运行**该扩展。

---

## 3. 已知坑（别踩）

1. **另一个 worktree `/home/skystar/Live2D-Ai`（`mod/persona-polish`，09-14 旧基线）里，有本次 3 份文档的旧未跟踪副本。**
   Stage A 的 A0 快照已把它们**提交进 `-fe`**；**不要再从 `-Ai` 快照一次**，会把 `-fe` 的接地版覆盖成旧版。
2. `stagePlaylist` **零测试覆盖**（rc.5 §9.2）。DEC-2 若选保留，必须**先补守护测试**，否则又多一条无护栏的产品路径。
3. `_clampInt`（`display_prefs.dart:724-727`）的「越界回落**默认值**」是 **scrim 的语义**（0=auto / 1=无），DEC-1 必须用**新函数**，不要改它。
4. parity 文档里的**行号是快照**，B-a/B-b 改完会漂（parity §8 第 5 条已自我声明）。
5. 只要改 `.dart` 就必须**重建前端**才能在 `/app/` 看到：`flutter build web --release --base-href /app/ --no-web-resources-cdn`。
6. 资产红线（规划 §3.2）：不删 WS 帧、不改既有帧字段名；`clean_for_tts` 不得旁路；表演层「说话权归属」是**未裁决项**，不得自行裁决。

---

## 4. 本次会话（规划方）的任务边界

- 本会话**只做规划 / 项目审查 / 产出阶段提示词**，**未修改任何业务代码**。
- 本次落盘内容：本 HANDOFF + 上述三份文档的 Stage B 接地修订（docs-only 提交）。
- 未完成的部分就是 §2：**Stage B 实施、7 条裁决、Stage C/D 提示词重新接地、3 类肉眼/运行时验收**。
