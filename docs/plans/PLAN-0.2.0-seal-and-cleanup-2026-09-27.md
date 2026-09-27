# 0.2.0 封口路线图：资产守护 / 背景透传 / 技术债 / 末版发布（2026-09-27）

> **来源**：会话归档 `dsh-session-session-60624ae2-….zip`（前端审计：背景 / 透传 / 设置链路 / 审美）。
> 该会话被用户叫停，两个子审计被 `interrupt_agent` 掐断、**收尾消息为空**——但它们的结论**在
> `subagents/*/session.v3.jsonl` 里**，本文已把它们捞回并逐条独立复核。
>
> **维护者 2026-09-27 追加口径（本文据此改写，取代上一版）**：
> 1. **0.2.0 线继续开 `rc.x`**（不跳到 0.3.0），最后收在 **0.2.0 末版**；
> 2. **导演层接线的整套 Live2D 表演 + TTS 过滤 = 本版本重要资产**，必须守护（§3）；
> 3. **背景图片透传**以 [shalldie/vscode-background @ `eef5ddb`](https://github.com/shalldie/vscode-background/tree/eef5ddb651501ef5f7d386e06e684f75cfc1a8aa)（v3.1.0）
>    为**短期实现目标**（§5）；
> 4. 之后再**清理技术债 + 项目管理**（§7），最后发布 0.2.0 末版（§6 Stage D）；
> 5. 本文**只做规划与项目审查**，实施交给其他 agent worker；worker 提示词见
>    `docs/plans/IMPL-PROMPTS-0.2.0-seal-2026-09-27.md` 与 `docs/plans/ORCHESTRATOR-PROMPT-0.2.0-seal-2026-09-27.md`。

---

## 0. 一页结论（TL;DR）

1. **审计干净、代码不干净**：`flutter analyze` 0 issue、`flutter test` 970 全过，但掩盖了 **4 个 P0/P1**，
   其中一条会让**用户背景图永久丢失**（§4 P0-1）。
2. **封口的最大风险不是 bug，是拓扑**：本轮前端重设计（09-27，112 个 Flutter 文件）**未提交**，长在
   **09-14 的 rc.1 旧基线**上；而 0.2.0 线已到 **`v0.2.0-rc.4`（09-26）**，`crates/` 领先 **150 文件 / +39908 行**。
   两侧 Flutter 有 **29 个文件重叠**，**其中多个正是「重要资产」的前端接线点**（§3.3）。
3. **重放 ≠ 覆盖**：重设计文件里有 `main.dart`（`_applyDirectorCueForSeq`）、`live2d/live2d_stage.dart`（`applyPreset`）、
   `settings/sections/tts_section.dart`——**直接覆盖就会把资产打断**。Stage A 的第一要务是给资产加**守护断言**。
4. **有一条测试在给缺陷背书**：`background_store_test.dart:122` 把"存储挂掉→偏好退回没有图"当契约钉死。
5. **有一份临时审计文件在库里**：`shell/flutter/test/zz_audit_tmp_test.dart`（13.7 KB，"用完即删"），含 15 条复现，
   当前**混在 970 的计数里**。
6. **路线**：`rc.5 资产+集成+P0` → `rc.6 背景透传追平参考` → `rc.7 技术债+项目管理` → **0.2.0 末版**。

---

## 1. 会话信息总结（审计本身）

- 任务（`seq 10`）：**审计前端设计的背景 / 透传 / 设置链路 / 审美**；Live2D 渲染、对话、后端**豁免**。
- 中途换模型，随后用户 `停止工作 总结目前的发现返回`。
- **已核实（第一手）**：透明度收敛到单一真源 `uiTransparency → panelAlpha = 1 - 0.45t`（下界 0.55）；
  `hasBackgroundAt()` 单一判据；`Scaffold.backgroundColor` 的 `null` 分支写反已修；`scrimAlphaFor` 的亮度项已删。
- **明确没审到**（被叫停）：审美本体量化、1039 行设置区、IndexedDB 迁移/剪枝的数据丢失风险。
- **从归档恢复的两个子审计（各 31–32 步、95 次工具调用）**：
  - **子审计 A（审美/令牌）**：四套配色**确实已带色相**（black `surface #11121B / alt #1D1E2A / raised #252634`）；
    **W4 五项（`solve`/自定义 accent/`density`/`lookPreset`/搜索框）确认静默丢弃**（代码 0、测试 0、**减记 0**）；
    `onAccent ≥4.5` **数学下界 4.58 ⇒ 不可能失败**；同色相测试只比 `sign(b-r)`、**缺 `ink vs raised`**；白套 ink 色度仅 **1/255**。
  - **子审计 B（设置链路）**：挖出 **P0 数据丢失三段链**（§4 P0-1）、**迁移不检查 put**（P0-2）、
    **拖动排序双重减 1**（P0-3）、**缺陷被测试背书**（P0-4）、内存 store 假成功、成功文案盖失败、
    滑杆读数不夹持、**≥2 项时预览不可达**。

---

## 2. 现状盘点（可复现）

| 项 | 值 |
|---|---|
| 当前 worktree | `/home/skystar/Live2D-Ai` @ `mod/persona-polish` @ `4e421993`，**提交日期 2026-09-14 21:10** |
| 与 main 关系 | **main 的祖先**（`merge-base --is-ancestor` = true）⇒ 落后 **84** 提交、不领先任何提交 |
| 0.2.0 线 HEAD | `main` = `e4f139a8` **`v0.2.0-rc.4`**（2026-09-26，「0.2.0 线最后一个 RC」，但按新口径**继续开 rc.x**） |
| `origin/main` | `2d492447` `v0.2.0-rc.1`（09-14）——本地 main 领先远端 **85** 个未推提交 |
| 代码差距 | `crates/` = **150 文件 / +39908 / -991**；`shell/flutter` = **82 文件**；`AGENTS.md` 差 **263 行** |
| 未提交工作区 | **133** 项（113 已跟踪 + 20 未跟踪）；113 files **+5349 / -2055**；**`crates/**` 零改动** ✅ |
| 重叠文件 | **29** 个（重设计 × main）⇒ 重放必冲突 |
| 门禁（实测） | `flutter analyze` **0 issue**；`flutter test` **970 通过**（含 15 临时 ⇒ 真实 955） |
| 复现验证 | `flutter test test/zz_audit_tmp_test.dart` = **15/15 通过** ⇒ 缺陷稳定复现 |
| 仓库卫生 | **24** 个 worktree、**1** 个 stash、`docs/releases/v0.3.0.md` 与 rc.4 版本线矛盾 |

---

## 3. 必须守护的资产：导演层接线的 Live2D 表演 + TTS 过滤

> 这是维护者点名的**本版本重要资产**。定义与真源逐条列在 main 的
> `AGENTS.md`（「动作与表演的**现行状态**」5 条 + 「导演可观测」+ 「每皮套动作幅度」）与
> `docs/architecture/performance-protocol-v1.md`（**表演协议 v1 的唯一真源**）。

### 3.1 资产清单（现行产品路径）

| # | 能力 | 真源 |
|---|---|---|
| A1 | **9 条动作包 + `none` 哨兵**经 `preset` 帧直接驱动渲染面参数（`PresetSlot::Face/Gesture` 两槽，到点撤销） | `assets/actions/presets.json`、`assets/actions/preset_labels.json`、`crates/l2d-wasm-demo/src/preset/{mod,table,scales}.rs`、`crates/l2d-wasm-demo/src/main.rs`（`"preset" =>`） |
| A2 | **`[action]` 幅度倍率** + **每皮套 `[action.models.<id>]`**（三键各自回落全局），按通道钳位（`ParamAngle*`≤30 / `ParamBodyAngle*`≤10 / 五官≤4） | `live2d-ai.toml`、`crates/live2d-ai-runtime/src/settings.rs`（`ActionSettings`）、`crates/l2d-wasm-demo/src/preset/scales.rs`、`shell/flutter/lib/live2d/action_scales_sync.dart` |
| A3 | **director Mod 已接线，唯一驱动 = WS `action_cue`**；`preset_id=="none"` = 该句音频开始时撤销两槽；前端通道 `_applyDirectorCueForSeq`（旧的 `latest.preset_id` 拉取通道**已退役**） | `crates/live2d-ai-mod-director/src/{lib,presets}.rs`、`crates/live2d-ai-desktop/src/web_api/cli_entry.rs`、`mods_routes.rs`、**`shell/flutter/lib/main.dart`** |
| A4 | **表演协议 v1**：`segments`（**只切分、逐字不变**，拼接 == 原文）+ `cues`（`body`/`head`/`expression` 三族，add 合成，`hold`/`ttl_ms`，`at: now/seg:N/after_prev`） | `docs/architecture/performance-protocol-v1.md`、`crates/live2d-ai-runtime/src/performance/{client,plan,prompt}.rs`、`conversation/engine.rs` |
| A5 | **TTS 过滤 `clean_for_tts`**：**上屏 == 送 TTS == `clean_for_tts(段)`**；只拆标记、不改句界 | `crates/live2d-ai-runtime/src/tts.rs`、`conversation_engine_tts_flow.rs` |
| A6 | **音频时钟基准** `stage-clock`（30ms）+ **事件级 ack**（`preset-applied/expired/dropped`、`segment-ended` 含最终轴值/`clamped`/`degraded`/`reason`） | `crates/l2d-wasm-demo/**`、`shell/flutter/lib/live2d/render_events.dart` |
| A7 | **会话 baseline** 绑会话（停止/新消息 = 清动作+表情+TTS待播+回 baseline，不补帧） | `docs/architecture/performance-protocol-v1.md`、runtime |
| A8 | **导演可观测** 四栏（dev_mode 才渲染，零后端、**不落盘**、不新增 WS 帧） | `shell/flutter/lib/settings/sections/director_observer_section.dart`、`director_panel.dart` |

### 3.2 守护红线（Stage A/B 一律适用）

1. **不许删帧、不许改既有帧字段名**（V11：只增不改）。`action_cue` / `preset_id` / `speak` **一律不删**。
2. **不许把 `clean_for_tts` 旁路**：任何送 TTS 的文本必须过它；上屏与送 TTS 必须同源。
3. **不许让前端镜像每帧状态流**；口型/表演高频信号仍走 `GlobalKey → Bridge → postMessage`。
4. **不许动 `mod_count_is_five` 与五个 Mod 的 id 集合断言**；`wallpaper`/`pet-desktop` 保持封存。
5. **重设计重放时，asset 接触点必须逐个保留语义**（§3.3），并**新增守护断言**（见下）。
6. 表演层「说话权归属」是**未裁决项**（V12/Q1）——worker **不得**自行裁决。

### 3.3 重放冲突热点（重设计 × main 的 29 个重叠文件里，**碰资产**的那些）

| 文件 | 资产 | 风险 |
|---|---|---|
| `shell/flutter/lib/main.dart` | A3 `_applyDirectorCueForSeq` | **重放会覆盖导演 cue 接线** |
| `shell/flutter/lib/live2d/live2d_stage.dart` | A1 `applyPreset`、A2 `set_scales` | **动作包下发丢失** |
| `shell/flutter/lib/live2d/live2d_bridge.dart` | A6 协议桥、`stage-bg` | 协议帧丢失/错位 |
| `shell/flutter/lib/settings/sections/tts_section.dart` | A5 TTS 配置面 | 配置项被旧版覆盖 |
| `shell/flutter/lib/chat/turn_liveness.dart` | A7 baseline/回合状态 | 回合语义回退 |
| `shell/flutter/lib/state/ui_state_tracker.dart` | error/ack 帧消费 | 可观测性回退 |

**守护手段**：Stage A 必须先产出一份
`docs/architecture/frontend-asset-inventory.md`（上表 + 每个接触点的 file:line + 现有测试名），
并为每个接触点**补一条会失败的守护断言**（例：`main.dart` 必须仍含 `_applyDirectorCueForSeq` 调用点；
`live2d_stage.dart` 必须仍发 `preset` 帧）。**先把网架起来，再重放。**

---

## 4. 技术债清单（分级）

### P0 —— 数据丢失 / 界面在骗人

| # | 项 | 证据 |
|---|---|---|
| **P0-1** | **背景图永久丢失（三段式）**：瞬时 IDB 读失败 → 摘项 + `migrated=true` → 写回（清单空）→ 下次启动 `items.isEmpty` → `retainOnly({})` 真删字节 | `background_hydration.dart:48-52, 71-78, 82` |
| **P0-2** | **迁移不检查 `put` 成功**：IDB 写失败时项留在偏好、字节两处皆无 | `background_hydration.dart:67` |
| **P0-3** | **拖动排序双重减 1**：`onReorderItem` 已调整 newIndex（Flutter 3.47.3 SDK 明文），应用再 `-1` ⇒ 向下拖早一格、**向下拖一格完全没反应** | `shell_prefs.dart:304` + `appearance_section.dart:698` + SDK `reorderable_list.dart:82-84` |
| **P0-4** | **缺陷被测试背书** | `background_store_test.dart:122` |
| **P0-5** | **遮罩文案与实现相反**（"随图变亮自动加强" vs 亮度项已删） | `appearance_section.dart:455,475` vs `background_logic.dart:85-97` |

### P1 —— 静默失效 / 死代码 / 假绿灯

`kShellSurfaceAlpha` 死常量+空转测试（`shell_backdrop.dart:43`、`shell_backdrop_test.dart:172-173`、`chat_panel.dart:92`）；
≥2 项时**预览不可达**（`appearance_section.dart:666`）；悬空引用**无提示**（浏览器实测 `bg0ee3027d`）；
内存 store **假成功**；**成功文案盖失败**（`shell_prefs.dart`）；**滑杆读数不夹持**（`field_row.dart:194` vs `:208`，
存量 `slideInterval=3600` vs 滑块 300）；**W4 五项静默丢弃**；**`onAccent` 零检验力**；**白套 ink 色度 1/255**。

### P2 —— 文档腐烂 / 未接线 / 弱测试

三处仍讲已删的 `syncShellStageBg`（`shell_backdrop.dart:7`、`app_shell.dart:457-461`、`chat_panel.dart:92`）；
`shell_backdrop.dart:12` 宣称 `stretch/tile` 而 `display_prefs.dart:285 maxImageFit=1`；
`AppMaterial.toString()` 漏 `uiTransparency`；`app_shell_background_test.dart:44-63` 是**源码字符串扫描**非行为断言；
`effectiveBackground` 恒第 0 项 vs 轮播运行时索引（形状已被复现 C3 钉住）。

### P3 —— 结构与仓库卫生

`appearance_section.dart` **1039 行**（超 `≤500`，连 `≤1000` 豁免带都不满足）；临时测试入库风险；
**24 worktree + 1 stash**；6 份未跟踪文档（含**属于 0.2.0 线**的五 Mod 审查/调研）；**两份互相矛盾的 `AGENTS.md`**。

---

## 5. 背景图片透传：短期实现目标（参考 vscode-background v3.1.0）

> 参考：[shalldie/vscode-background @ `eef5ddb`（v3.1.0, 2026-09-11）](https://github.com/shalldie/vscode-background/tree/eef5ddb651501ef5f7d386e06e684f75cfc1a8aa)。
> **参考它的机制，不抄它的实现**（它是 VS Code 扩展，改的是 workbench HTML）。

### 5.1 参考特性模型（`package.json` contributes 实测）

- **多区域**：`background.editor` / `.fullscreen` / `.sidebar` / `.auxiliarybar` / `.panel`，各自独立配置；
- 每区域：`images[]`（支持 **在线 https / 本地路径 / 文件夹 / `~` 与环境变量 / data URL**）、
  `opacity`、`size`（CSS `background-size`）、`position`（CSS `background-position`）、
  `style`（任意 CSS）+ `styles[]`（**逐图**样式）、`interval`（秒，0=关）、`random`；
- `editor` 额外 `useFront`（图在代码**上方或下方**）；
- 全局 `background.enabled`。

### 5.2 我们已有 vs 参考（差距表）

| 参考能力 | 我们现在 | 结论 |
|---|---|---|
| 多区域（5 个） | **2 个**：壳 / 舞台（`backgroundSource`） | **短期只做壳内子区域**（纯 Flutter）；跨到渲染面=改后端 ⇒ 记录不做 |
| `images[]` 列表 | ✅ `backgrounds[]`（image + pattern） | 有；但**拖动排序坏**（P0-3）、**预览不可达**（P1-2）⇒ 先修 |
| `opacity` | ✅ `backgroundOpacity` | 已有 |
| `size` | ⚠️ 仅 `cover/contain`（`maxImageFit=1`）× 9 宫格 `position` | **主要缺口**：补 `stretch/tile/auto/百分比`（§5.3） |
| `position` | ✅ 9 宫格 `imageAlign` | 已有（比 CSS 更可点选，保留） |
| `interval` / `random` | ✅ `slideInterval`/`slideRandom` | 有；但**区间/clamp 不一致**（P1-6）⇒ 修 |
| `style` 逐图样式 | ❌ | 短期做**逐图 `opacity/fit/align` 覆盖**（纯 CSS 字符串不可移植到 Flutter，不做） |
| `useFront` | ❌（背景恒在内容之下） | 短期**不做**；列为可选项评估（压住面板会影响可读性） |
| `background.enabled` 全局开关 | ❌ | 可低成本补 |
| 在线图 / 文件夹 / 任意 CSS | ❌ | **明确不采纳**：本项目**离线优先红线**（`ignite.sh --check` 扫 `gstatic.com`）；写进偏离说明 |

### 5.3 短期（rc.6）目标定义

1. **`imageFit` 扩到参考同集的子集**：`cover` / `contain` / `stretch` / `tile`（`tile` 需 `tileSize`），
   在 Flutter 侧用 `BoxFit` + `CustomPainter` 实现；**删除 `maxImageFit=1` 这个假上限**或给它写理由。
2. **逐图样式覆盖**（`perItemStyle`）：每张图可单独 `opacity/fit/align`，回落全局。
3. **壳内子区域**：至少壳的「聊天/侧栏」与「设置面板」可分辨（若判定为堆砌，**写明不做**）。
4. **轮播语义对齐**：`effectiveBackground`/`currentItemIsImage` 必须反映**运行时当前索引**，与 UI 一致。
5. **背景库交互修复**：管理/预览模式分离（预览可达）、拖动排序正确。
6. **偏离说明落盘**：在线图/文件夹/任意 CSS/多区域跨渲染面——逐条写「为什么不做」。

---

## 6. 0.2.0 封口路线图（四阶段）

> 口径：**0.2.0 线继续开 `rc.x`**，最后收在 **0.2.0 末版**。每阶段一个 rc，阶段内可并发 worker。

### Stage A · `0.2.0-rc.5` —— 资产守护 + 重设计并入 + 背景 P0

**目标**：把 09-27 的前端重设计**并入 0.2.0 线**，且**一行资产都不许断**。

1. 从 `main` 切 `feat/frontend-redesign`（**新 worktree**，不要在本 worktree 直接干）。
2. 先产出 `docs/architecture/frontend-asset-inventory.md` + **资产守护断言**（§3.3）。
3. 重放 112 个 Flutter 文件；29 处冲突逐个解；**资产接触点逐个对照 §3.1 复核**。
4. 修背景 P0：P0-1/2/3/5（P0-4 的测试期望同步改）。
5. 测试治理：15 条临时复现转正式（P0 类先红后绿）；`background_store_test.dart:122` 改期望。
6. 门禁 + **Win 肉眼**（背景选图→可见→清图→刷新仍在；拖动排序；预览）。

**收口判据**：`flutter analyze` 0 + `flutter test` 全绿且**不含 `zz_`**；资产守护断言全绿；
`cargo test/fmt/clippy/rust-ratio` 全绿；`ignite.sh --check` 四项 ok。

### Stage B · `0.2.0-rc.6` —— 背景透传追平参考（§5.3）· **基于 rc.5 实测重写**

> rc.5 已交付：A1 守护网（19 条）、重放（112 文件 / 29 冲突）、P0-1…P0-5、P1/P2、复现转正（flutter test 1272）。
> **B4「偏离说明」已提前完成**（`docs/architecture/background-parity-vscode-background.md`，412 行）。
> 因此 Stage B 只剩三件事：**扩档（fit/tile）**、**逐图样式**、**把 rc.5 §9 的 7 条未决项落地**。

#### B.0 前置裁决（rc.5 §9 + parity §7 提出；编排者未擅自改）

| # | 问题 | 建议（默认按此执行，除非维护者否决） | 依据 |
|---|---|---|---|
| **DEC-1** | `slideInterval` 越界（301–3600）回落 **0＝关轮播** 而非 300；存量 1–4 秒同理 | **改成端点夹持**（>300→300，<5→5；`0` 仍＝关）。新增专用 `_clampIntToRange`，**不改** `_clampInt`（scrim 依赖「越界回落默认」） | 回落 0＝静默关掉用户已开的功能；夹持落在合法区间 |
| **DEC-2** | **两套轮播并存**：`stagePlaylist`（舞台单图，走 stage-bg 帧，**零测试覆盖**）vs 背景库轮播（走 Flutter 层） | **不合并**（合并要改后端）；改为**分工 + 改名 + 按来源互斥显示**：来源＝舞台那张时只显示舞台块，＝背景库时只显示壳块。**保留即先补 `stagePlaylist` 守护测试** | parity §7.1；两者通道不同 |
| **DEC-3** | 壳内子区域（parity §5.2） | **本轮不做，写明理由**（无用户诉求 + 避免堆砌 + 「舞台是主角」），记 Stage C backlog | 规划 §5.3 第 3 条要求「写明不做」 |
| **DEC-4** | 全局 `background.enabled` | **做**（低成本，参考有） | parity §6 |
| **DEC-5** | C4：坏 dataURL 仍 `isRenderable=true` | **收紧**到真 dataURL 形态（`data:image/…` + 逗号 + 非空 payload），坏图要能被 UI 告知 | rc.5 §9.6 |
| **DEC-6** | C3：`currentItemIsImage` 只看第 0 项 | **修**：AppShell 已有运行时 `_backgroundIndex`（`app_shell.dart:181`，由 `ShellSlideshow.onAdvance` 驱动），把它传进外观区判「当前项」；**不持久化**运行时索引 | rc.5 §9.6 |
| **DEC-7** | D3 位置显隐条件与注释相反；D4 来源＝舞台时轮播控件空转且文案误导 | **都修**（显隐对齐；空转改「禁用＋说明」或隐藏） | rc.5 §9.6 |

#### B.1 任务波次（按文件所有权切分）

| 波次 | 任务 | 独占文件 | 并发性 |
|---|---|---|---|
| **B-a** | 模型/渲染：fit 四档＋`tileSize`、逐图样式字段、全局开关、DEC-1/DEC-5 的 clamp 与 dataURL | `settings/display_prefs.dart`、`ui/shell_backdrop.dart`、`design/background_item.dart` | 先做 |
| **B-b** | UI：铺法四档、逐图样式编辑器、DEC-2/DEC-6/DEC-7 的显隐与预览、顺手抽出背景块 | `settings/sections/appearance_section.dart`（＋新增 `appearance_background.dart`） | **等 B-a** |
| **B-c** | docs：parity 补 DEC-3 结论与「已成现状」、`docs/README.md` 索引 | `docs/**` | 可与 B-a/B-b 并行 |
| **B-d** | 收口：门禁＋肉眼（四档逐档 / tile / 预览 / 拖动 / 两轮播分工）＋ rc.6 说明 | 无 | 最后 |

#### B.2 收口判据

- `imageFit` 四档＋`tileSize` 有测试且 **Win 逐档可见**；逐图样式可覆盖且缺省回落全局；
- 轮播「控件说的 ＝ 画面做的」；预览在**任意库大小**可达；拖动排序正确（rc.5 已修，回归保留）；
- **DEC-1…DEC-7 逐条落地或写明否决**；`stagePlaylist` 若保留则已有守护测试；
- 门禁数字**不低于 rc.5**（cargo 1457 / flutter 1272）；`ignite.sh --check` 四项 ok。

---

### Stage C · `0.2.0-rc.7` —— 技术债 + 项目管理
### Stage C · `0.2.0-rc.7` —— 技术债 + 项目管理

**收口判据**：§4 的 P1/P2/P3 逐条关闭或**写明不做 + 理由**；`AGENTS.md` 单一化；worktree/stash/未跟踪文档清零；
`v0.3.0.md` 版本线矛盾消除；W4 五项有裁决。

### Stage D · `0.2.0` 末版发布

版本号三处同步（`Cargo.toml` / `pubspec.yaml` / `README`）；release note；tag；推 `origin/main`；
跑一遍完整门禁 + 肉眼 checklist 并落盘。

---

## 7. 项目管理清理（Stage C 主体）

- **AGENTS.md 单一化**：以 **main 版为底**（含「动作与表演现行状态 / 导演可观测 / 每皮套动作幅度」），
  补一节「前端显示层（背景库/材质）」；**不要**再用本 worktree 那份"动作不存在"的旧 doctrine。
- **worktree / 分支**：24 个 worktree 逐个判定——`git branch --merged main` 的**移除 + 删分支**，
  未并入的**写明为什么活着**；`stash@{0}` 解包归类或删。
- **未跟踪文档 6 份**：并入 0.2.0 线或移 `docs/legacy/`。
- **版本线矛盾**：`docs/releases/v0.3.0.md`（来自 `dev/integrity`）标注"历史草案，未发布"或归档。
- **门禁对齐**：把本轮的资产守护断言纳入 CI（`flutter-checks.yml`）。
- **`.gitignore`**：临时测试命名（`zz_*`）与 `build/`、`.dart_tool/` 明确排除。

---

## 8. 未来工作（0.2.0 之后）

- **W4 补齐或正式减记**（可计算配色 / 自定义 accent / 三档密度 / 风格预设 / 设置区搜索框）。
- **设置区重排**：`appearance_section.dart`（1039 行）拆分 + `GroupCard` 推广 + 大预览卡。
- **背景存储健壮性**：三态 store API、配额如实报错、悬空引用可见提示 + 一键清理。
- **审美量化闭环**：把 `docs/design/assets/visual-substance-2026-09-27/measure_chroma.py` 接进验收。
- **需要后端配合（只记录）**：正文帧带 `sentence_seq`（朗读高亮）；舞台模糊/分区背景（平台视图限制）；
  `wallpaper` Mod 是否晋升。

---

## 9. 执行清单（Stage → 勾选）

**Stage A（rc.5）**
- [ ] 从 main 切 `feat/frontend-redesign` 新 worktree
- [ ] `frontend-asset-inventory.md` + 资产守护断言
- [ ] 重放 112 文件、解 29 冲突、逐个复核 §3.1
- [ ] P0-1 / P0-2 / P0-3 / P0-5 修复；P0-4 测试期望改正
- [ ] 15 条复现转正式；删 `zz_audit_tmp_test.dart`
- [ ] 门禁 + Win 肉眼

**Stage B（rc.6）**
- [ ] `imageFit` 四档 + `tileSize`
- [ ] 逐图样式覆盖
- [ ] 轮播索引语义一致
- [ ] 背景库管理/预览分离
- [ ] 偏离说明落盘

**Stage C（rc.7）**
- [ ] P1/P2/P3 关闭或写明不做
- [ ] AGENTS.md 单一化
- [ ] worktree / stash / 未跟踪文档清零
- [ ] `v0.3.0.md` 版本线矛盾处理
- [ ] W4 五项裁决

**Stage D（0.2.0 末版）**
- [ ] 版本号三处同步
- [ ] release note + tag + 推 origin/main
- [ ] 全套门禁 + 肉眼 checklist 落盘

---

## 附录 A · 证据索引

| 结论 | 证据 |
|---|---|
| 拓扑落后 main 84 提交 | `git rev-list --left-right --count main...HEAD` = `84 0`；`merge-base --is-ancestor` = true |
| main = rc.4 | `git log -1 main` = `e4f139a8 2026-09-26` |
| 后端差 40k 行 | `git diff --stat HEAD main -- crates` = 150 files, +39908/-991 |
| 29 文件重叠 | `comm -12`(`git status` flutter 文件, `git diff --name-only HEAD main`) |
| 门禁 | analyze = No issues；test = 970（含 15 临时）；`zz_audit_tmp_test.dart` = 15/15 |
| 资产真源 | main `AGENTS.md`「动作与表演的现行状态 / 导演可观测 / 每皮套动作幅度」+ `docs/architecture/performance-protocol-v1.md` |
| 参考特性 | `/tmp/vscode-bg` @ `eef5ddb` 的 `package.json` contributes + `README.zh-CN.md` |
| P0-1/2 | `background_hydration.dart:48-52, 67, 71-78, 82` |
| P0-3 | `shell_prefs.dart:304` + `appearance_section.dart:698` + SDK `reorderable_list.dart:82-84` |
| P0-4 | `background_store_test.dart:122` |
| P0-5 | `appearance_section.dart:455,475` vs `background_logic.dart:85-97` |

> 归档恢复脚本：`/tmp/sess60624/dig.py`；参考仓库浅克隆：`/tmp/vscode-bg`。
