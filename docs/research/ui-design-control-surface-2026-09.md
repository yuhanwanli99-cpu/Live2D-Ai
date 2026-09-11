# 虚拟形象「控制台 / 舞台」交互设计调研（2026-09）

> 委托调研。目标：为 Live2D-Ai 从零设计的 **Flutter Web 前端**给出**控制台与舞台的交互设计模式与结论**。
> 本文**只做交互与界面**，不写实现代码；**动作生成侧**（待机/idle/头部追踪/导演层）见相邻调研
> `docs/research/idle-motion-tracking-and-director-2026-09.md`，本文不重复。
>
> 纪律：每条非显然结论附来源；对本仓库的断言附 `文件:行`。
> 【已验证·官方一手】= 官方文档/官方源码/官方手册实读；
> 【已验证·本仓库】= 本仓库源码实读；
> 【二手】= 第三方转述/社区；
> 【推断】= 本人判断，无直接来源；
> 【未核到】= 取不到证据。
>
> 证据分级标注在每条结论后。**报告末尾给 10 条具体设计决策**。

---

## 0. 方法与基线

### 0.1 证据获取方式

- 外部：官方文档/官方源码直接抓取（VTube Studio 官方 API 文档与官方 Wiki、VRChat 官方创作者文档、
  Nielsen Norman Group、Google Conversation Design、Electron 官方 API 文档、winit 官方 rustdoc）。
- 内部：直接 grep/read 本仓库源码，附 `文件:行`。
- 三个并行子调研分别覆盖 ①VTS/VSeeFace/VRChat、②Warudo/Live2DViewerEX/Stream Deck/桌宠（VPet、Shimeji）、
  ③OBS/语音助手/自动-手动，**三路均已回传并已并入本文**。
  所有外部结论均可回溯到具体 URL 与原文引用；取不到的写入 §10【未核到】，**不以记忆填空**。

### 0.2 本仓库基线（撰写时快照）

- 仓库 HEAD：`28756088`；**工作区非干净**（`git status --porcelain` 显示 `AGENTS.md`、`CHANGELOG.md`、
  `crates/live2d-ai-desktop/src/web_api/{cli_entry,ws}.rs`、`shell/flutter/lib/{main,audio/audio_player,settings/display_prefs,ui/display_panel}.dart`
  等被并行改动）。**本文行号以撰写时工作区为准**；引用时同时给符号名，避免行号漂移误导。
- 前端主线：`shell/flutter/`（Dart）。旧 JS 前端（`crates/live2d-ai-desktop/src/web_api/app.js` 等）为遗留实现。
  【已验证·本仓库】`AGENTS.md` §分层与技术栈。
- 渲染面：iframe `/render`（Rust/wasm），协议 v1 `{version,type,payload}`。
  【已验证·本仓库】`shell/flutter/lib/live2d/live2d_bridge.dart:18-27`；`crates/l2d-wasm-demo/src/main.rs:277-300`。

### 0.3 后端已具备的「可暴露面」（控制台能挂什么）

| 能力 | 端点/DTO | 证据 |
|---|---|---|
| 能力快照：`actions`（6）、`action_sources`（3）、`strength_levels`[1,2,3]、`model_upload_supported`、`script_invoke_supported`、`runtime_ws`、`state_ws`、`ws_protocol_version` | `GET /api/v1/app/capabilities` | 【已验证·本仓库】`crates/live2d-ai-desktop/src/web_api/dto.rs:21-50`（`AppInfo`）、`dto.rs:268-290`（`capabilities_info()`） |
| 命令注册表（6 个基础动作，`params_schema` 仅 `strength` 1..3 默认 2，`allowed_sources` 恒 `["user_command"]`，`kind` 恒 `single`） | `GET /api/v1/commands` | 【已验证·本仓库】`crates/live2d-ai-desktop/src/web_api/command_registry.rs:71-96`（`CommandDescriptor`/`SINGLE_BASE_SCHEMA`）、`command_registry.rs:126-178`（`COMMANDS`）、`web_api/commands_routes.rs:62-95`（`CommandSummary`/`InvokeAccepted`） |
| 触发命令（**手动通道**）：经 `SupervisorHandle::make_user_action` → `ActionSource::UserCommand` | `POST /api/v1/commands/{id}/invoke` | 【已验证·本仓库】`web_api/commands_routes.rs:154-235`、`command_registry.rs:20-34`（头注：「都走 supervisor trigger_action 通道 → core 闸门」） |
| 动作仲裁（单 active；来源优先级 user 100 > llm 90 > rule 50；完全相同幂等拒绝；低优先级不可抢占；同优先级后来者胜） | core reducer | 【已验证·本仓库】`crates/live2d-ai-core/src/action/mod.rs:110-130`、`action/mod.rs:264-317`、`action/mod.rs:228-256`（`ActionDropReason`/`ActionEffect`） |
| 动作事件（**含来源** `user_command`/`llm_tool`/`rule_fallback`） | WS `/ws/state` 帧 `action_state` | 【已验证·本仓库】`crates/live2d-ai-desktop/src/web_api/ws/events.rs:112-138` |
| 回合/播放状态 | WS `turn_state`、`runtime_status`（`voice_started`/`voice_ended`/`new_epoch`/`shutdown_ready`）、`text_delta`、`error`、`heartbeat`、`subscribe_ack` | 【已验证·本仓库】`web_api/ws/events.rs:39-111`、`shell/flutter/lib/api/ws_client.dart:260-341` |
| 日志（dev_mode 门控）、级别列表 | `GET /api/v1/logs`、`GET /api/v1/logs/levels` | 【已验证·本仓库】`web_api/mod.rs:28-29,128-131`、`web_api/log_routes.rs:48-105` |
| 模型管理（列表/导入/激活/显示/删除） | `/api/v1/models*` | 【已验证·本仓库】`web_api/models_routes/mod.rs:71-75`、`web_api/mod.rs:135-137` |
| Mod 管理（列表 + enable/disable/restart/config） | `GET /api/v1/mods`、`POST /api/v1/mods/{id}/{action}` | 【已验证·本仓库】`web_api/mods_routes.rs:3-9,56-141` |
| 运行状态（服务器） | `GET /api/v1/app/status`（uptime/active_model_id/epoch/dev_mode/…） | 【已验证·本仓库】`web_api/dto.rs:52-95`、`web_api/app_routes.rs:150-186` |

**已有但没被前端用起来的三个关键信号**（本报告多处建议的基础）：

1. `action_state` 帧带 `source`，但 Flutter 客户端**忽略**它，且把 `data.action` 当字符串解析——
   而服务端发的是对象 `{action,strength,source}`。
   【已验证·本仓库】服务端 `web_api/ws/events.rs:121-131`；客户端 `shell/flutter/lib/api/ws_client.dart:68-80` 与 `ws_client.dart:303-310`（`_str(data['action'])` → 实际为 `null`）。
2. 渲染面**已实现** `action-state`（start/end + 强度档 0.6/1.0/1.35）、`stage-zoom`（in/out/reset）、
   `clickEnabled`、`stage-bg`，但 Flutter 桥只实现 `sync`/`stage`/`mouth`/`destroy`。
   【已验证·本仓库】wasm：`crates/l2d-wasm-demo/src/main.rs:334`（clickEnabled）、`:346-360`（stage-zoom）、`:379-406`（action-state）、`:344-376`（stage-bg）；
   Flutter：`shell/flutter/lib/live2d/live2d_bridge.dart:89-145`（仅 4 类下行）。
3. 渲染面上行 `stage-ack`（「未收 ACK 前父页不得提示已生效」）在 Flutter 端被**默认分支忽略**。
   【已验证·本仓库】wasm 注释 `crates/l2d-wasm-demo/src/main.rs:270-273`；Flutter `live2d_bridge.dart:249-250`。

---

## 1. 「动作/表情触发」的界面形态

### 1.1 同类产品怎么做（一手证据）

**VRChat（Expression Menu / Action Menu，径向菜单）**【已验证·官方一手】
[Expressions Menu and Controls](https://raw.githubusercontent.com/vrchat-community/creator-docs/main/Docs/docs/avatars/expression-menu-and-controls.md)：

- 「This allows you to play animations or change your avatar's appearance with VRChat's **radial menu**.」
- 容量：**「Up to 8 controls can be added to a single menu.」**并支持 Sub-Menu 嵌套。
- 控件类型（可直接搬进我们的动词语义）：`Button`（**「Cannot be held down」**，约一秒后自动复位）、
  `Toggle`、`Sub-Menu`、`Two-Axis Puppet`、`Four-Axis Puppet`、`Radial Puppet`（「kind of like a progress bar that you can fill」）。
- 参数语义：Sub-Menu 进出可设/清参数；puppet 退出后**保留**数值（可「freeze」）；Puppet 用 IK Sync、Button/Toggle 用 Playable Sync。

**VTube Studio（Hotkeys 面板 + 屏幕按钮 + 外部 API）**【已验证·官方一手】
[官方 Wiki：Expressions](https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/Expressions-(a.k.a.-Stickers-or-Emotes).md)、
[官方 API 文档](https://raw.githubusercontent.com/DenchiSoft/VTubeStudio/master/README.md)：

- 表达式的设置入口就是 **「Hotkeys」标签页**：「Expressions can be set up directly inside of VTube Studio on the **"Hotkeys"** tab.」
- 每个热键是结构化条目：`name / type / description / file / hotkeyID / keyCombination / onScreenButtonID`
  （`type` 例：`TriggerAnimation`、`ToggleExpression`、`ChangeIdleAnimation`、`ChangeVTSModel`、`MoveModel`、`ChangeBackground`、`Unset`）。
- **屏幕按钮也是 8 个，且形态明确**：「`onScreenButtonID` … **(1 (top) to 8 (bottom) or -1)**」；
  [Model Settings](https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/VTS-Model-Settings.md)：
  「**Up to 8 hotkeys can be set as on-screen buttons. They will be shown as semi-transparent white circles on the right of the main screen**」
  ⇒ 与 VRChat 的「8 controls per menu」形成**独立双源收敛**：**8 是表达槽位的隐性业界常数**，
  且**屏幕按钮的推荐形态是"半透明圆形浮标、贴主屏右侧"**（不是工具条）。
- 键盘绑定能力边界：「You can set combinations of **up to 2 keys**.」「Also included: **Mouse-Hotkeys** (right/left/middle click).」
  ；macOS 上「hotkeys will only work on macOS if the VTube Studio window is **focused**」（Windows 后台可用）。
- 热键名会进日志：「You can give hotkeys names that are also **shown in the logs** when the hotkey is activated.」
- **外部触发来源是显式字段**（对本项目 §2 极关键）：Events 负载含
  [「`hotkeyTriggeredByAPI`: false」](https://raw.githubusercontent.com/DenchiSoft/VTubeStudio/master/Events/README.md)
  ⇒ VTS 把"这次触发是不是 API（外部程序）干的"当作**一等公民信号**下发。
- 外部来源的绑定入口有独立 UI：每个热键旁有 Twitch 图标，
  [「click the Twitch icon next to the hotkey name. Hotkeys that have no active Twitch Triggers will have this icon **grayed out**.」](https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/Twitch-Hotkey-Triggers.md)
- 触发的执行侧有**冷却与队列**：「one specific hotkey can only be triggered **once every 5 frames**」，
  「You may send different hotkeys in quick succession, which will result in them getting **queued**」，
  队列容量 32，满则报错。
- 重名规则：「If multiple hotkeys have the same name, only the first one will be executed (order is the order they show up in the UI).」
- 键盘绑定**不对插件暴露**：「For security reasons, the `keyCombination` array will currently **always be empty**」。
- 官方一句硬约束（对控制台设计极重要）：
  「It's recommended to activate expressions **via hotkeys** since otherwise users could end up with
  **activated expressions they can't deactivate** because they don't have hotkeys set up for them.」
  ⇒ **任何"能被自动化打开"的状态，必须配一个用户可触达的关闭入口。**

**本仓库自身的历史先例（py-legacy）**【已验证·本仓库】
`docs/plans/node-d-py-asset-migration.md` 的迁移表：

- R11：`renderer/src/dev-console/dev-console.ts`「开发者控制台：动作目录可视化 + 手动触发」，
  「**保留**：动作列表渲染、强度 1/2/3 滑块、**release 按钮**、**accepted/rejected 计数**。**改**：去掉「原始动作触发接口」直连（Rust core 走 audit log）」。
- R10：`choreography.ts` 的 `ActionPriority`「**dev/user/timeline/fallback/idle**」5 级（现 core 只有 3 级来源优先级）。
- R13：`live-event.ts` 的 `LiveEventPayload`（`type/text/user/motion/emotion/strength/parameterOverrides`），
  外部直播事件只接 `danmaku/gift/command` 三类。
- 旧 JS 前端现状：设置页「动作与互动 → 基础动作试演」卡片网格（6 个 `.act-btn`）、
  可拖拽的**动作浮窗**（带 `actDrag` 拖拽头 + 强度 1/2/3 分段 + `actLog` 历史）、
  以及 `试演` 后写一行带时间戳的日志。
  【已验证·本仓库】`crates/live2d-ai-desktop/src/web_api/index.html:165-170,235-251`、`web_api/app.js:95-160,289-330`。

**VSeeFace（热键在 Expression settings，且热键与外部输入会打架）**【已验证·官方一手】
[官方开发者手册仓库](https://raw.githubusercontent.com/emilianavt/VSeeFaceManual/master/README.md)：

- 「you can assign a **hotkey in the `Expression settings`** to trigger it」；热键后台可用：
  「All configurable hotkeys also work while it is **in the background or minimized**, so the expression hotkeys,
  the audio lipsync toggle hotkey and the configurable **position reset hotkey**」。
- 设置分段是 **General / Expression / advanced / Light / VMC**；超出界面的项落 `settings.ini`。
- **冲突规则是"外部输入赢"**：「if received **blendshapes** are applied to a model, triggering expressions
  **via hotkeys will not work**.」⇒ 与"用户优先"相反的一种取舍（见 §8 反直觉）。
- 状态反馈：视场边缘蓝条（「you are close to the edge of your webcam's field of view」）、
  左上角更新提示、`Open logs` 按钮。

**Warudo（触发=节点图，镜头/缩放复位，Stream Deck 集成）**【已验证·官方一手】
[docs.warudo.app](https://docs.warudo.app/docs/blueprints/understanding-blueprints)、
[getting-started](https://docs.warudo.app/docs/tutorials/getting-started)、
[Stream Deck integration](https://docs.warudo.app/docs/blueprints/miscellaneous/streamdeck-integration)：

- 触发不是菜单而是**节点**：「Type "on keystroke" in the search bar, and you should see the **On Keystroke Pressed** node.」；
  连线规则：「A flow input can connect to multiple flow outputs, but a flow output can only connect to **one** flow input.」
- **"谁在触发"用动画表达**：「You can determine if a flow is being triggered … by checking if there's a **ball rolling on the connection**」
  ⇒ 把控制流做成**可见的运动**，是本报告见过最直接的"控制权可视化"手法（见 §2）。
- 官方对新手/高级的分层原话：「Many options are for **advanced users** and are not necessary for day-to-day usage.」
  「For beginners though, they could look **intimidating**, so we recommend leaving them alone for now.」
- 复位：「set the **Scale** values to **1**, or click on the **Reset** button」；「click **Reset Camera Transform**」。
- Stream Deck 集成：Toggle 动作的**状态用图标颜色表达**（「⚪ Grey … become 🟣 Purple」）。
- **未核到**：文档树里没有径向菜单（radial/径向 0 命中）——径向**不是行业必需**。

**Live2DViewerEX（"控制面板 + 快捷菜单"两级，浮球手势）**【已验证·官方一手】
[官方手册 live2d.pavostudio.com](https://live2d.pavostudio.com/doc/zh-cn/pc/manual/)：

- **两级控制台**：「快捷菜单是一个**精简版的控制面板**，通过快捷图标打开。」
  ⇒ 这是本报告推荐的"渐进披露"在同类产品里的直接实现（对照 NN/g）。
- 浮球的手势分工：「**可随意拖动，点击可展开**」；另有「开启之后**长按**助手模型会弹出文本框」
  ⇒ 同一个指针通道用**拖动/点击/长按**三种手势分流。
- 控制面板分 7 组（模型/背景/效果/滤镜/小部件/场景/设定）；最多 8 个模型槽；编辑面板**在模型前弹出**；
  小部件可鼠标移动 + 可锁定（「点击锁图标可以锁定」）。
- **未核到**：官方手册未见热键列表、滚轮缩放、点击穿透开关。

**Elgato Stream Deck（离散=键，连续=旋钮；键有状态）**【已验证·官方一手】
[docs.elgato.com](https://docs.elgato.com/stream-deck/profiles/getting-started/)、
[SDK · dials](https://docs.elgato.com/streamdeck/sdk/guides/dials/)、
[SDK · keys](https://docs.elgato.com/streamdeck/sdk/guides/keys/)、
[官方 dial stack 说明](https://www.elgato.com/eu/hu/explorer/products/stream-deck/what-is-a-dial-stack-for-stream-deck/)：

- 键位数量与旋钮：`Stream Deck +` **8 键 + 4 旋钮**（2×4），`XL` 32 键（4×8），`+ XL` 38 键 + 6 旋钮。
- 组织方式：「Profiles can also incorporate **page navigation, folders**, and organizational structure」；
  动作类型含「hotkeys, **multi-action keys, dial actions**, text actions, and plugin actions」。
- **旋钮专职连续量，按键专职离散事件**：manifest 示例 `"Push": "Play / Pause"`、`"Rotate": "Adjust Volume"`、
  `"LongTouch": "Skip Track"`；旋钮还有 **dial stack**：「letting **one dial handle multiple actions**.
  Then, by **pressing the dial**, you can cycle through each action in the stack」。
- 键的**状态反馈是内建设计**：「you can choose to configure **two states** within the manifest」、
  「the **state is toggled** when the user presses the key」、瞬时反馈 `showOk` / `showAlert`。
- **触发来源可见**（对 §2 有用）：「if (ev.payload.**isInMultiAction**) { … ev.payload.**userDesiredState** }」
  ⇒ 外部按键（复合动作）会把自己"是复合触发的、期望什么状态"告知插件。
- Loupedeck：官方仅把旋钮当作独立控件类；具体页面**未核到**。

**OBS Studio（直播控制台：混音器 + 二级对话框 + Studio Mode + 热键表）**【已验证·官方一手】
官方 KB [obs-studio-overview](https://obsproject.com/kb/obs-studio-overview)、
[kb/audio-mixer-guide](https://obsproject.com/kb/audio-mixer-guide)、
官方本地化文件 [frontend/data/locale/en-US.ini](https://github.com/obsproject/obs-studio/blob/master/frontend/data/locale/en-US.ini)（master；旧版在 `UI/data/locale/en-US.ini`）：

- **控制台形态**：主界面 = 混音器 + 场景/来源列表 + 底部控制；**"高级"的东西进二级对话框**
  （Audio Monitoring 在 `Edit → Advanced Audio Properties` 里，不在主界面）——两级，不是三级。
- **热键表有冲突检测**（值得直接抄）：
  `Basic.Settings.Hotkeys.FilterByHotkey="Filter by Hotkey"`、
  `Basic.Settings.Hotkeys.DuplicateWarning="This hotkey is shared by one or more other actions..."`。
  KB 列举的热键用途含 `Push-to-talk/Push-to-mute`——即**"静音"也是一种可绑定动作**。
- **Studio Mode = 模式感知的标准答案**：KB 原话
  「you will see the current **Live Scene (what your viewers see)** on the right while your **edit Scene** on the left」，
  且「**Viewers cannot see when Studio Mode is enabled or not**」；面板标签就是
  `StudioMode.Preview="Preview"` / `StudioMode.Program="Program"`。
- **诊断**：Help 菜单下 `Show Log Files` / `View Current Log` / `Upload Current Log File`
  （master 已把 `Upload &Last Log File` 改名为 `Upload &Previous Log File`，见 27.2.4 与 master 的 locale 差异）；
  上传对话框带两个按钮：`Copy Log URL` 与 `Analyze Log File`；另有网页版 Analyzer。
  **Stats 窗口**字段：`Dropped Frames (Network)`、`CPU Usage`、`Average time to render frame`、
  `Skipped frames due to encoding lag`、`Missed frames due to rendering lag`、`Reset Stats`
  ——**没有"复制统计信息"**（官方 locale 里只有 Reset，**未核到**复制入口）。
- 音频见 §6.2（Mute 与 Monitor 是**两个轴**）。

### 1.2 可迁移的模式对比

| 形态 | 典型容量 | 发现性 | 记忆负担 | 适合频率 | 一手依据 / 判断 |
|---|---|---|---|---|---|
| **常驻工具条 / 角标按钮** | 3–6 | 最高（永远可见） | 无 | **最高频（每轮对话都用）** | 【推断】工具条是「首屏即重要」的渐进披露读法 |
| **半透明屏幕浮标（半圆排列）** | 8 | 高（贴主屏边缘） | 低 | 高频、演出型 | VTS 官方：「semi-transparent white circles on the **right of the main screen**」【已验证·官方一手】 |
| **两级控制台（面板 + 精简快捷菜单）** | 精简 5–8 / 完整 7 组 | 高（快捷入口）+ 深 | 低 | 高频走快捷、低频进面板 | Live2DViewerEX 官方：「快捷菜单是一个精简版的控制面板」【已验证·官方一手】 |
| **浮球手势入口（拖/点/长按）** | 3 种手势 | 中（浮球可见） | 低 | 长期挂机、桌面宠物 | ViewerEX：「可随意拖动，点击可展开」「长按助手模型会弹出文本框」【已验证·官方一手】 |
| **快捷键面板（可配置热键表 + 试演按钮）** | 10–100 | 低（需打开面板才知道） | 高 | 高频（键盘用户）/ 演出型 | VTS「Hotkeys」标签页（每项带 `description` 供 UI 展示）、≤2 键组合 + 鼠标键【已验证·官方一手】；VSeeFace 在 Expression settings 里绑热键【已验证·官方一手】 |
| **卡片网格（面板内）** | 6–20 | 中 | 低 | 中频（偶发试演/排练） | 本项目旧 JS「基础动作试演」6 按钮网格【已验证·本仓库】；dev-console「动作目录可视化」【已验证·本仓库 R11】 |
| **径向菜单 / 表情轮盘** | ≤8/页（可嵌套） | 中（需知道开口存在） | 中（位置记忆） | 中低频、手势/摇杆/VR/单手 | VRChat 官方称 radial menu、8 控件/页，理由是「quickly perform those actions **one handed** and in a less obvious manner」【已验证·官方一手】 |
| **连续轴控制器（Puppet / 硬件旋钮）** | 1 参数/控件（可堆叠） | 中 | 低 | **调参数**（强度/角度/音量/进度），非触发 | VRChat `Two/Four-Axis Puppet`、`Radial Puppet`「像进度条」【已验证·官方一手】；Stream Deck +：`Rotate: Adjust Volume`、dial stack 一键多动作【已验证·官方一手】 |
| **节点图（Blueprint/触发器）** | 无上限 | 低（需搜索节点） | 高 | 高级用户/复杂编排 | Warudo 官方：「Type "on keystroke" in the search bar」；「Many options are for **advanced users**…could look **intimidating**」【已验证·官方一手】 |
| **外部触发（聊天/礼物/API/硬件）** | 无上限 | — | — | 由事件驱动，不进 UI 按钮区 | 本项目 R13（danmaku/gift/command）【已验证·本仓库】；VTS 的 Twitch 触发器 + API 热键【已验证·官方一手】 |
| **文字/关键词触发** | 无上限 | 低 | 高 | 低频、兜底 | 本项目 core 的 `rule_fallback` 来源即此类【已验证·本仓库】`action/mod.rs:117-118` |

### 1.3 结论

1. **6 个动作 + 3 档强度根本不需要径向菜单**。径向的价值在"小集合 + 位置肌肉记忆 + 手柄/VR 指针"；
   本项目是 Flutter **Web + 鼠标/触屏**，径向要自绘命中区、与 iframe 舞台叠层协调、触屏上收益进一步下降（【推断】）。
   该用「常驻工具条（3 个高频）+ 面板卡片网格（6 动作 × 强度）」。
2. **屏幕槽位按 8 设计**（VRChat 8/页、VTS 8 屏钮，双源收敛）——留 2 个空位给未来的 Mod 动作，
   不要一开始塞满 6 个就改布局。【已验证·官方一手】
3. **每个触发项必须自带"停止/释放"**（R11 的 `release` 按钮、VTS「否则用户会有打不开也关不掉的状态」）。
   本项目已有 `ActionCommand::Interrupt`/`Release`（`action/mod.rs:215-226`），但**没有任何 HTTP 端点暴露它**——
   控制台目前能给"开始"却不能给"停"。这是 §2 的核心缺口。
4. **重复点同一个动作不会重播**：core 对「动作+强度+来源完全相同」返回 `Dropped(AlreadyActive)`
   （`action/mod.rs:294-299`）。VTS 的成熟解法是**队列 + 冷却**（5 帧/次、队列 32），而不是丢弃【已验证·官方一手】。
   控制台要么给按钮加"正在演"的置灰/进度，要么把连点语义定义为"重新开始"（先 Release 再 Play）。
5. **键盘绑定不要下发到渲染面**（VTS 明确因安全不暴露 `keyCombination`）——快捷键属于壳层，
   渲染面只认语义动作 id。【已验证·官方一手】
6. **"径向菜单"不是行业共识，别把它当必然选项**：Warudo 官方文档里**完全没有**径向/屏幕操作菜单
   （触发一律是节点图），Live2DViewerEX 用「浮球 + 两级面板」。VRChat 用径向的官方理由是
   「**one handed** and in a **less obvious** manner」——即"单手 + 旁人不易察觉"，**不是"效率更高"**。
   ⇒ 本项目若不做径向，不构成落后；若要做，理由也只应是"单手/低干扰"。【已验证·官方一手 + 推断】
7. **离散动作给按钮、连续量给旋钮/滑杆——这条分工在硬件侧已被验证**：Stream Deck + 的
   `Rotate: Adjust Volume`（连续）与 `Push: Play / Pause`（离散）写在官方 manifest 示例里；
   本项目的"强度 1/2/3"是**离散三档**，应做成 segmented 按钮而不是滑杆，而
   "音量 / 缩放 / 口型灵敏度"是连续量，才用滑杆。【已验证·官方一手 + 本项目旧 JS 已是 segmented：`app.js` 的 `.seg-btn[data-v]`】
8. **热键/动作表要自带冲突检测**：OBS 的热键设置同时提供 `Filter by Hotkey` 与
   `DuplicateWarning`（「This hotkey is shared by one or more other actions…」）【已验证·官方一手】
   ⇒ 本项目若给动作加快捷键（VTS 的"每热键 ≤2 键 + 鼠标键"是同类），**必须提示重复绑定**，
   否则用户按下一个键会同时触发两个动作。
9. **"准备中"与"已上屏"值得区分**：OBS 用 Preview/Program 两栏表达
   「what your viewers see」vs 你正在编辑的场景，并明确「Viewers cannot see when Studio Mode is enabled or not」
   【已验证·官方一手】。本项目当前只有"用户看到的舞台"这一路输出；
   若将来加入直播/录制或"AI 自主演出"模式，**应显式区分"我在排练"与"已对外"**，
   且不要让这个模式切换对外可见（OBS 的原话正是"观众看不出你是否开了 Studio Mode"）。

---

## 2. 手动触发 vs 自动触发：界面如何表达「谁在控制」

### 2.1 本仓库已给出的**权威答案**（比任何同类产品都更直接可用）

core 的动作子系统已经是一个「单 active + 来源优先级」仲裁器：

- 来源与优先级：`UserCommand = 100`、`LlmTool = 90`、`RuleFallback = 50`。
  【已验证·本仓库】`crates/live2d-ai-core/src/action/mod.rs:110-130`。
- 判定顺序（注释原文）：**capability gate → 完全相同幂等 → 低优先级被拒 → 空闲起播/否则抢占**。
  【已验证·本仓库】`action/mod.rs:264-268`；实现 `action/mod.rs:280-317`。
- 抢占是**单一显式效果** `ActionEffect::Transition { from, to }`。
  【已验证·本仓库】`action/mod.rs:244-256`。
- 手动通道恒为 `user_command`：`POST /api/v1/commands/{id}/invoke` →
  `SupervisorHandle::make_user_action`（`commands_routes.rs:229-235`），
  LLM 工具通道恒为 `llm_tool`（`crates/live2d-ai-desktop/src/tool_adapter.rs:52-56`）。
  ⇒ **UI 不需要自己发明"谁优先"的规则**；UI 只需要**把它显示出来**。

**"谁在控制"的语义**（可直接写成 UI 规则）：

| 场景 | core 结果 | 建议 UI |
|---|---|---|
| 空闲 + 用户点动作 | `Start{user}` | 动作卡高亮 + 来源徽标「你」 |
| 空闲 + LLM 调动作 | `Start{llm}` | 徽标「AI」+ 可选一句 `reason`（工具参数里有 `reason`：R3 保留 3 参数 `action/strength/reason`）【已验证·本仓库】`docs/plans/node-d-py-asset-migration.md` R3 |
| AI 动作在演 + 用户点另一个 | `Transition{llm→user}`（用户抢占） | 徽标从「AI」切到「你」，并**短暂闪一次"已接管"** |
| AI 动作在演 + 用户点**同一个** | `Dropped(AlreadyActive)` | 按钮显示「正在演」，不要静默 |
| 用户动作在演 + 规则想插 | `Dropped(LowerPriority)` | 若在开发者面板，显示被压住的入参；普通用户不打扰 |
| 模型不支持该动作 | `Dropped(CapabilityMissing)` | 按钮**置灰**并给 tooltip（能力来自 `capabilities.actions`） |

### 2.2 同类产品的「人在场 = 自动化让位」模式（反直觉，值得抄）

**VTube Studio：面板打开时自动化被冻结**【已验证·官方一手】
[官方 API 文档](https://raw.githubusercontent.com/DenchiSoft/VTubeStudio/master/README.md)：
`ItemListResponse` 的 `canLoadItemsRightNow`「may be `false` if the user has certain menus or dialogs open in VTube Studio.
**This generally prevents actions such as loading items, using hotkeys and more.**」
⇒ 直觉会以为「设置面板是旁观者」，但业界把「用户正在操作界面」当作**暂停自动化的信号**。

**VTube Studio：自动化移动可被用户就地打断**【已验证·官方一手】
`ItemMoveRequest.userCanStop`：「If you want the user to be able to **stop the movement by clicking/dragging** the item
while it's moving, set `userCanStop` to `true`」（默认 `false`）。
⇒ 「手动接管」是**默认关闭、需显式开启**的能力，且接管手势就是**在角色身上按/拖**。

**VTube Studio：外部控制要用户先授权**【已验证·官方一手】
API Permissions：「Certain functionality … is locked behind additional **permissions** that have to be requested by the plugin
after authenticating … a popup is shown to the user explaining what the permission does. The user can then choose to
**grant or deny** it.」；拒绝后返回 `errorID 50`。可随时撤销。
⇒ 把「LLM/外部代理能控制模型」表达成**一次显式授权 + 可撤销**，而不是一个隐式开关。

**VRChat：Toggle 是"状态"，Button 是"一次性动作"**【已验证·官方一手】
「`Button` … usually after about a second. **Cannot be held down**」/「`Toggle` … resets when the toggle is turned off」。
⇒ UI 语汇要区分「瞬时动作」与「持续状态」，否则用户无法判断"现在谁在持有这个状态"。

**VTS：外部触发来源是下发的字段（本项目最该对标的先例）**【已验证·官方一手】
Events 负载带 [`"hotkeyTriggeredByAPI": false`](https://raw.githubusercontent.com/DenchiSoft/VTubeStudio/master/Events/README.md)
⇒ 同类产品把"这次动作是不是外部程序（AI 插件）干的"做成**协议字段**，UI 只需渲染。
本项目已有等价物：WS `action_state.data.action.source`（`ws/events.rs:121-131`）——**只是前端没读**。

**VTS：自动化运动会锁住手动（但可开"用户可打断"）**【已验证·官方一手】
- [API 文档](https://raw.githubusercontent.com/DenchiSoft/VTubeStudio/master/README.md)：
  「While the movement is going on, the user **cannot move the model manually by dragging it**.」
- 但同一 API 提供 `userCanStop`：「If you want the user to be able to **stop the movement by clicking/dragging** the item
  while it's moving, set `userCanStop` to `true`」（默认 `false`）。
⇒ 「手动接管」是一枚**默认关闭、可显式开启**的能力，且接管手势就是**在角色身上按/拖**。

**VSeeFace：也存在相反取舍——外部输入赢**【已验证·官方一手】
「if received **blendshapes** are applied to a model, triggering expressions **via hotkeys will not work**.」
⇒ 当外部驱动器（摄像头/外部程序）在持续写参数时，手动触发表情**直接失效**。
这是"来源优先级"的另一种答案（外部 > 手动），与本项目 core 的"用户 100 > LLM 90"**方向相反**。
⇒ **结论**：优先级方向不是普适常量，必须在 UI 上把"当前谁在持有"讲清楚；本项目可采用
"用户优先"（core 已如此），但必须用可见状态告诉用户"AI 正在演/被你压住了"。

**Warudo：把"谁在触发"做成屏幕上的运动**【已验证·官方一手】
「You can determine if a flow is being triggered … by checking if there's a **ball rolling on the connection**」
⇒ 控制流可见化的极端做法；对本项目的启示是：**归属提示不必是文字徽标，也可以是一段只在触发时出现的动效**。

**Stream Deck：外部按键会告知"我是复合触发"**【已验证·官方一手】
SDK keys 文档：`ev.payload.isInMultiAction` + `ev.payload.userDesiredState`
⇒ 触发来源与意图可以作为参数随事件传递，而不是靠 UI 猜。

**Nest 恒温器的「hold」（手动词的"模态"含义）**【已验证·官方一手】
子调研实读 [support.google.com/googlenest/answer/10120172](https://support.google.com/googlenest/answer/10120172)：
当前术语是 **hold**（不是 "temporary hold"）——
「When you set your thermostat to **hold** the temperature, it will maintain that specific temperature
for **as long as you want**」「**ignore scheduled or automatic temperature changes for a while**」；
且有一条**模态规则**：「**If a hold is active, you must end that hold first. Then, you can change the temperature.**」
（预设有 `up to 24 hours` 的自动到期上限。）
⇒ 两种可迁移的手动接管语义：
(a) **优先级式**（本项目 core 的做法：用户 100 直接抢占，无需先"结束"什么）；
(b) **模态式**（先结束 hold 才能改）——**更安全但更烦**。
本项目该选 (a)（与 core 一致），但**必须在 UI 上让"AI 的 hold"可见**，否则用户会疑惑
"为什么它自己在动/为什么我改了它还回去"。【已验证·官方一手 + 已验证·本仓库 + 推断】

**OBS Studio Mode（"我给观众看的" vs "我在准备的"）**【已验证·官方一手】
[kb/obs-studio-overview](https://obsproject.com/kb/obs-studio-overview)：
「you will see the current **Live Scene (what your viewers see)** on the right while your **edit Scene** on the left」；
面板标签即 `Preview` / `Program`；并明确「**Viewers cannot see when Studio Mode is enabled or not**」。
⇒ "谁在控制 / 谁看得见"可以靠**并列两个画面**表达，而不是靠文字说明；
且**模式本身对观众不可见**——这正好对应"AI 在后台怎么动，不该影响用户对前台状态的判断"。【已验证·官方一手】

**Amazon Alexa：barge-in 的定义**（官方 Amazon Science 博客，非开发者文档）
【已验证·官方一手】[amazon.science](https://www.amazon.science/blog/change-to-alexa-wake-word-process-adds-natural-turn-taking)：
「One key to natural turn-taking is handling **barge-ins**, or customer **interruptions of Alexa's output speech**」
「Alexa knows to **stop speaking and proceed with processing the new request**」「contextual barge-in」。
⇒ 打断的语义是"**立刻停 + 立刻接受新意图**"，而不是"停在原地等确认"。
本项目 core 的 `StopRequested`→epoch 推进→`new_epoch` 正是这个语义【已验证·本仓库】。
（`developer.amazon.com` 的 barge-in 文档页全部 404，官方细节**未核到**。）

### 2.3 结论（含本仓库的**可观测性缺口**）

1. **不做"自动/手动"总开关，做"手动优先 + 临时接管"**：优先级已在 core 里，UI 只需显示归属。
   总开关会与 core 的仲裁语义重复，还会制造"关了自动但 AI 仍然影响"的困惑。【推断 + 已验证·本仓库仲裁语义】
2. **归属必须落到最小可视单元**：动作卡/动作日志每条带来源徽标（你 / AI / 规则），
   AI 触发的用**不同色相 + 小图标**，并允许展开看 `reason`。
   数据现成：`action_state.data.action.source`【已验证·本仓库】`ws/events.rs:121-131`。
3. **补两个可观测缺口**（否则 §2 的承诺无法兑现）：
   - **`Dropped` 目前只进 tracing 日志**：`RootEffect::Action(ActionEffect::Dropped { reason }) => tracing::debug!(...)`
     【已验证·本仓库】`crates/live2d-ai-desktop/src/supervisor/handlers.rs:244-246`。
     建议投影成一条 `action_state{kind:"dropped", reason, incoming, active}` 的 WS 帧。
   - **Flutter 侧 `action_state` 解析是坏的**：`ActionStateEvent` 只有 `{epoch,kind,action:String?}`，
     而服务端 `data.action` 是对象 ⇒ `action` 恒为 `null`。【已验证·本仓库】对比 `ws_client.dart:68-80,303-310` 与 `ws/events.rs:121-131`。
4. **给外部/LLM 控制加"授权 + 范围"表达**：参照 VTS 的权限弹窗，第一次启用 `llm_tool` 触发动作时，
   在聊天/设置里给一次性说明卡（"AI 可以自己做这些动作：点头/摇头/…"，附关闭入口），
   而不是让它神秘地动。**任何 AI 能打开的状态都要有用户可点的关闭**（VTS 官方原话）。
5. **用户在操作面板时考虑降噪**：不用真去冻结 AI 动作（那会伤"陪伴感"），但**手动试演期间应压制
   AI/规则动作**（用户点下去到动作播完的窗口内，UI 可以先本地打"手动接管中"），这与 core 的
   优先级语义一致，且避免"我点歪头它却在惊讶"。【推断】
6. **"当前谁在持有"要有持续态，而不是只有一次性日志**：
   VRChat 用 Toggle 表达持续态、Stream Deck 用键的两态图标、Warudo 用连线上滚动的球。
   本项目可用「动作卡上的进度环 + 来源徽标」承载同一语义（进度环同时解释 `AlreadyActive` 的幂等丢弃）。
   【已验证·官方一手（三例）+ 推断（本项目映射）】

---

## 3. 参数字段的可视化调节：暴露 / 隐藏的判据

### 3.1 本项目现有的两个面板与它们的边界

- **Flutter `DisplayPanel`（外观与口型）**：模型缩放、口型灵敏度、主音量滑杆、
  「静音」（副标题「只关声音、保留口型——需要安静时用它」）、「口型同步」、「待机小动作」、「恢复默认」。
  【已验证·本仓库】`shell/flutter/lib/ui/display_panel.dart`（`缩放`:53-63、`口型灵敏度`:64-86、`主音量`:87-96、静音提示 `97-102`、`静音`:111-115、`口型同步`:120-124、`待机小动作`:129-135、`恢复默认`:137-146）。
- **持久化与 clamp 的单一真相在 `DisplayPrefs`**（scale/mouthSensitivity/lipSync/idleEnabled/muted/volume），
  纯逻辑、可 VM 单测。【已验证·本仓库】`shell/flutter/lib/settings/display_prefs.dart`。
- **渲染面可调参数**（协议 v1）：`scale`(0.5–2.0)、`dark`、`lipSync`、`mouthSensitivity`(0..`MAX_MOUTH_SENSITIVITY`)、
  `idleEnabled`、`tier`(4096/8192/16384)、`clickEnabled`、`model`。
  【已验证·本仓库】`crates/l2d-wasm-demo/src/main.rs:311-345`。
- **不存在的能力**：`sync.payload.paused` 由 Flutter 桥发出（`live2d_bridge.dart:94,104`），
  但 wasm 侧没有 `paused` 分支（`main.rs` 全文无该字段）⇒ **这是一个会静默失效的字段，不该做成 UI 控件**。
  【已验证·本仓库】

### 3.2 同类产品怎么组织参数

**VTube Studio：模型作者决定"隐藏"，用户按需"全显"**【已验证·官方一手】
[Expressions Wiki](https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/Expressions-(a.k.a.-Stickers-or-Emotes).md)：

- 参数/文件夹可被模型作者标 **「Hide in list」**：「Hidden items will **not show up** in the Expression Editor
  unless the user enables the **"Show All"** toggle. This is useful for **hiding internal or tracking parameters**
  that users of your model/asset don't need to see.」
- 支持**分组（文件夹）+ 自定义图标 + 「Add space after」分组留白 + 最多 10 种语言本地化**。
- 表达式参数有三种作用模式：**Overwrite（默认）/ Add / Multiply**，且「Add/Multiply **applied after everything else**」；
  多热键改同一参数时「the value of the **last activated** hotkey is used」。
- **⭐ Model Customization Expression**：标记后**不被 "Clear all Expressions" 清掉**，用于"发型/服装"这类用户配置。

**VRChat：参数类型即 UI 约束**【已验证·官方一手】
Expression Parameters：「`Int` 0–255」「`Float` **-1.0 to 1.0**」「`Bool`」；
`Saved`（跨世界/模型重置保留）、`Synced`（同步），以及 `Puppet` 走 IK Sync、Button/Toggle 走 Playable Sync。
另有一条**"高级=改文件"**的官方自认：[OSC Avatar Parameters](https://docs.vrchat.com/docs/osc-avatar-parameters)
「the config file system is a **stop-gap** … until we can integrate a proper in-client UI for OSC, and **may be removed**」
⇒ 用配置文件当"高级面板"是行业里被承认的临时方案，不是好设计（本项目 `live2d-ai.toml` 同理）。

**Warudo：官方的"新手 vs 高级"界线就是编辑面**
【已验证·官方一手】[getting-started](https://docs.warudo.app/docs/tutorials/getting-started)：
「Many options are for **advanced users** and are not necessary for day-to-day usage.」
「For beginners though, they could look **intimidating**, so we recommend leaving them alone for now.」
⇒ 分层判据不止"参数名难不难"，还包括"**这一整块编辑面是否面向作者而非用户**"。

**Live2DViewerEX：控制面板分 7 组（模型/背景/效果/滤镜/小部件/场景/设定）**
【已验证·官方一手】[官方手册](https://live2d.pavostudio.com/doc/zh-cn/pc/manual/)：
「控制面板 ¶ 应用主控制界面，大部分功能都在此控制」，并提供**精简版快捷菜单**打头。
⇒ 注意它**恰好是两层**（快捷菜单 = 精简的同一面板 + 完整面板），
这符合 NN/g 的"≤2 层"约束；**不要**把它读成"可以再加第三个入口"。

**Stream Deck：连续量交给旋钮**【已验证·官方一手】
SDK manifest 示例 `"Rotate": "Adjust Volume"`、`"Push": "Play / Pause"`；
[官方 dial stack](https://www.elgato.com/eu/hu/explorer/products/stream-deck/what-is-a-dial-stack-for-stream-deck/)：
「letting **one dial handle multiple actions** … by **pressing the dial**, you can cycle through each action」
⇒ **一个连续控件可以服务多个参数，靠"按下切换目标"**——这正好解决"参数一多，滑杆就塞爆面板"的问题。

**Nielsen Norman Group（渐进披露的官方定义、收益与**硬约束**）**【已验证·官方一手】
[Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/)：

- 「Progressive disclosure **defers advanced or rarely used features to a secondary screen**, making applications
  **easier to learn and less error-prone**.」
- 做法：「**Initially, show users only a few of the most important options.**」「**Offer a larger set of
  specialized options upon request.**」
- 「the very fact that something appears on the initial display tells users that **it's important**.」
- 收益：「improves 3 of usability's 5 components: **learnability, efficiency of use, and error rate**」；
  新手「avoid mistakes」、老手「avoid having to scan past a large list of features they rarely use」。
- ⚠️ **硬约束（本报告据此修正了 §9 决策 6）**：
  「there are two things you must get right … the right split between initial and secondary features …
  **It must be obvious how users progress**」；
  且 **「designs that go beyond 2 disclosure levels typically have low usability」**。
  ⇒ **"普通区 + 快捷入口 + 高级面板"已经是三层，属于官方点名的低可用性结构**；
  正确做法是**两层**（普通 / 高级），高频项直接留在普通层首屏。

### 3.3 判断准则（本报告的结论）

把候选参数逐个过四问，**四问全"是"才进普通区**：

1. **可见后果吗？** 拖动后 1 秒内能用肉眼看出差异（缩放、音量、口型灵敏度=是；`ParamAngleX` 上限=不一定）。
2. **需要模型内部知识吗？** 不需要知道参数名/量程/物理（口型灵敏度=否；"头部角度限制 ±30° vs ±45°"=是）。
3. **错了能一键回来吗？** 有确定默认值且可复位（有 → 普通区；会累积漂移/污染模型文件 → 高级区）。
4. **一分钟内会调第二次吗？** 会（音量/静音/缩放）→ 常驻；不会（显示档位 tier、物理档）→ 高级区。

映射到本项目（**层数上限 = 2**：普通区 / dev_mode 高级区；高频项不另开第三个入口）：

| 参数 | 建议位置 | 依据 |
|---|---|---|
| 主音量、静音、模型缩放 | **常驻/普通区**（已在） | 【已验证·本仓库】`display_panel.dart`；【已验证·官方一手】NN/g「initial display 即重要」 |
| 口型灵敏度 | 普通区**带一行解释与标定值**（已在：「1.00 为标定值；觉得嘴动得太小就调大」`display_panel.dart:79-85`） | 同上 |
| 口型同步 / 待机小动作 / 点击互动 | 普通区开关（前两者已在；`点击互动` 尚无 Flutter 入口，见 §4） | 【已验证·本仓库】wasm `clickEnabled` 已实现 |
| 显示档位 `tier`（4096/8192/16384） | **高级区**（dev_mode 才显示） | 性能/画质权衡需要"档位"知识；旧 JS 已把它放在外观区但带「本地预览（不写入后端）」说明【已验证·本仓库】`app.js:57-63,376-430` |
| 头部/身体角度上限、参数曲线、物理档 | **开发者区**（dev_mode + 单参数滑杆） | 需要模型知识；且当前 wasm 侧用硬编码 ±30/±10（`crates/l2d-wasm-demo/src/param_scale.rs`），
暴露未标定的滑杆会放大失真【已验证·本仓库，见相邻调研 §已交付项】 |
| 单参数实时滑杆（`ParamAngleX/Y/Z`、`ParamMouthOpenY`…） | **开发者区**，只读+可覆盖+显式"释放" | 参照 VTS「不再被任何来源修改时回落到动画/追踪/默认值」的语义【已验证·官方一手】；本项目 core 对应 `ActionCommand::Release`【已验证·本仓库】`action/mod.rs:219-225` |
| 每参数"隐藏/显示" | **模型侧元数据 + 用户「显示全部」** | 直接照抄 VTS 的 `Hide in list` + `Show All`【已验证·官方一手】 |

**两个补充结论**：

- **参数面板要给单位与量程，不要只给 0–1 归一化**。VTS 官方强调头可设为 ±45（相邻调研已引用），
  本项目 `param_scale.rs` 目前硬编码 ±30 ⇒ 面板显示"±30°"会与"±45°模型"矛盾。
  【已验证·本仓库（相邻调研 §已交付项 (b)）】
- **Add/Multiply 语义值得在 UI 上区分**：VTS 的 overwrite/add/multiply 三模式决定了
  "我的调整会不会被 AI 的动作覆盖"。本项目 PoseStack 目前是纯覆盖（相邻调研 §1 结论），
  因此**不要给用户一个会被下一次动作清掉的"表情"控件**——要么标成"一次性"，要么等加性混合落地。
- **"一块连续控件管多个参数"值得抄**：Stream Deck 的 dial stack（按下切换目标、旋转调值）
  比"N 个滑杆平铺"更省面板空间；本项目普通区参数 ≤7 时不必用，但**开发者区的单参数滑杆**
  可以直接采用"选择一个参数 → 一个滑杆 → 显式释放"的形态。【已验证·官方一手 + 推断】

---

## 4. 舞台本身的交互（拖拽 / 缩放 / 复位 / 穿透 / 置顶）

### 4.1 本仓库现状（先厘清三层，别混为一谈）

**第一层：窗口层（桌面壳）**——置顶与穿透

- `--pet-mode` 的初始窗口意图由纯函数给出，并有一条**安全规则**：
  「`tray_confirmed == false` 时**强制 `click_through = false`**——穿透 + 无托盘恢复入口 = **不可恢复**」，
  且会打印可见降级说明。【已验证·本仓库】`crates/live2d-ai-desktop/src/user_event.rs:52-95`。
- 托盘事件里可切换 `ClickThrough` / `AlwaysOnTop` / `Visible`（toggle 与 set 两种语义）。
  【已验证·本仓库】`user_event.rs:102-121`。
- 后端能力表当前对 X11/Wayland 都返回"支持穿透"与"支持拖动窗口"。
  【已验证·本仓库】`crates/live2d-ai-desktop/src/platform/window.rs:50-59`。

**第二层：页面/画布层（iframe `/render` 内）**——拖拽、滚轮缩放、双击复位

- `pointerdown`（左键）**立即**进入拖拽，无位移阈值；`pointermove` 累加 `movementX/Y`（回退 client 差值），
  位移写入 `offset_x/offset_y`（clamp ±1.5 NDC）。
  【已验证·本仓库】`crates/l2d-wasm-demo/src/main.rs:594-670`。
- `wheel`：`deltaY<0` 放大 ×1.1，否则 ÷1.1，scale clamp 0.5–2.0，并 `preventDefault()`。
  【已验证·本仓库】`main.rs:687-710`。
- `dblclick`：**复位** scale=1.0 / offset=0/0。【已验证·本仓库】`main.rs:712-730`。
- 另有 `stage-zoom` 消息（`in`/`out`/`reset`），供壳层做 +/−/复位按钮。
  【已验证·本仓库】`main.rs:346-360`（注释提到「UI 精修规格 A2：控制条 +/−/复位」）。
- `clickEnabled=false` 会**同时**禁用上述四项（拖拽/滚轮/双击复位）。
  【已验证·本仓库】`main.rs:602-603,636-637,691-692,716-717`。
- **没有模型命中测试**：没有对 ArtMesh/包围盒做 hit test，拖拽在整个 canvas 上生效。
  【已验证·本仓库】`main.rs` 全文无 hit_test/bounds；对照 VTS 有 `ArtMeshes at position` API【已验证·官方一手】。
- **没有"点击角色触发互动"**：`pointerup` 只结束拖拽，没有任何 click/tap 语义。
  【已验证·本仓库】`main.rs:672-685`。
- Flutter 侧当前**没有** stage 工具条：`Live2DStage` 只有 loading 徽标、错误遮罩与 FPS 徽标（右上）。
  【已验证·本仓库】`shell/flutter/lib/live2d/live2d_stage.dart:151-177`。

**第三层：桌面宠物语义**——点击穿透与"点角色"

- 当前 `/render` 内**没有** `pointer-events:none` 的动态切换；穿透是**整窗**能力，不是逐像素。
  【已验证·本仓库】`crates/l2d-wasm-demo/index.html`（58 行，无控制条/无穿透逻辑）。

### 4.2 业界能做到什么：穿透 API 是**整窗布尔**（反直觉的硬约束）

**winit（本项目的桌面壳）**【已验证·官方一手】
[`Window::set_cursor_hittest`](https://docs.rs/winit/latest/winit/window/struct.Window.html#method.set_cursor_hittest)：

- 「Modifies whether the window catches cursor events. If `true`, the window will catch the cursor events.
  If `false`, events are **passed through the window** such that any other window behind it receives them.
  **By default hittest is enabled.**」
- 平台差异：**「iOS / Android / Web / Orbital: Always returns an `ExternalError::NotSupported`」**
  ⇒ 在 Web 里根本没有这个能力；只有桌面壳有。

**Electron（同类桌面壳的标准做法）**【已验证·官方一手】
[`win.setIgnoreMouseEvents(ignore[, options])`](https://www.electronjs.org/docs/latest/api/browser-window#winsetignoremouseeventsignore-options)：

- 「Makes the window **ignore all mouse events**. All mouse events happened in this window will be passed to the
  window below this window, but if this window has focus, it will still receive keyboard events.」
- `forward`（macOS/Windows）：「If true, **forwards mouse move messages to Chromium**, enabling mouse related events
  such as `mouseleave`. **Only used when `ignore` is true.**」

⇒ 行业解决"穿透但仍要知道鼠标在哪"的唯一通用手段是**动态开关 + 转发鼠标移动**：

```
鼠标移动到角色区域 → 关闭穿透（可交互）
鼠标离开角色区域   → 打开穿透（点得到桌面）
```

**结论（对本项目）**：`穿透` 与 `点击角色` **不可能同时是静态配置**。
必须实现为**动态切换**，且必须有**不可丢失的恢复入口**（托盘 / 全局热键）。
本仓库已有正确的安全规则（`user_event.rs:54-58`），**UI 只需要把它表达出来**（见 4.3）。

### 4.3 结论与交互规则

1. **两套开关，别混用**：
   - 窗口层：「置顶」「点击穿透」——放在**托盘菜单 + 设置页的"舞台"分组**，并显示当前态；
     穿透开启时给**常驻提示条**（"已开启穿透：按住 `Ctrl+Alt+P` 或从托盘恢复"）。
   - 画布层：「互动（拖拽/缩放）」——现有 `clickEnabled` 的语义**不是**"点击互动"，建议 UI 文案写成
     「允许拖动与缩放」；真正的"点角色"另开一项。
   【已验证·本仓库】`main.rs:602-717`（`clickEnabled` 实际禁的是拖拽/滚轮/双击）
2. **拖拽 vs 点击的经典冲突——桌宠实现里有四条可选通道，本项目应先取"时间阈值 + 右键菜单"**：
   - **时间阈值（首选，有源码先例）**：VPet 用**长按时长**区分点击与长按，且时长是**用户可设置的**
     （`PressLength`）：`VPet-Simulator.Core/Display/Main.xaml.cs` 有 `/// 默认点击事件` / `/// 默认长按事件`，
     `Thread.Sleep(Core.Controller!.PressLength)` 后按是否仍在按分流；
     `PressLength` 是设置项（`ISetting.cs`）。
     【已验证·官方源码】[Main.xaml.cs](https://raw.githubusercontent.com/LorisYounger/VPet/main/VPet-Simulator.Core/Display/Main.xaml.cs)
   - **位移阈值（辅助）**：VPet 拖拽=按下时移动窗口，带 **1 px 死区**（`if (Math.Abs(x) < 1) x = 0;`）
     与 **20 px 状态切换测试**（`if (Math.Abs(x) + Math.Abs(y) > 20 && rasetype >= 1) rasetype = 0;`）。
     Shimeji 虽有 5 px 判定，但**只用于重置被拖拽动画计时**（`action/Dragged.java`），
     不是点击/拖拽分界；Shimeji 的拖拽在**按下的瞬间**就开始（`Mascot.java`：
     「Switch to the drag the animation when the mouse is down」）。【已验证·官方源码】
   - **右键专职菜单（两个独立产品收敛）**：VPet `MainGrid_MouseRightButtonDown` → `ToolBar?.Show()`；
     Shimeji `if (event.isPopupTrigger()) { showPopupLater(event); }`。【已验证·官方源码（双源）】
   - **长按专职互动**：ViewerEX 官方手册「开启之后**长按**助手模型会弹出文本框」；浮球则「可随意拖动，点击可展开」。
     【已验证·官方一手】
   ⇒ **建议组合**：左键 = 点击互动（短按）/ 拖拽（超过时长或位移阈值）；右键 = 菜单；
     长按 = 第二类互动（可留空）；**不要**把左键同时给"拖拽窗口"与"点击角色"。【推断，但有上列先例支撑】
3. **VTS 对"自动化进行中"的手动处理值得对照**：自动化移动期间用户**不能**手动拖动（API 文档原话），
   但可用 `userCanStop` 让用户"点/拖即打断"。本项目动作播放期间舞台拖拽（只改 offset）与动作
   （只改参数）互不冲突，**不需要互锁**；需要互锁的是"同一参数的写入来源"。
   【已验证·官方一手 + 已验证·本仓库 `main.rs:650-660`】
4. **滚轮缩放 + 双击复位要"可见"**：目前双击复位是隐藏手势（`main.rs:712-730`）。
   建议在舞台角标给 +/−/复位 三键（复用已存在的 `stage-zoom` 消息，`main.rs:346-360`），
   双击作为快捷方式保留，并在 tooltip 里写出来。
   Warudo 的官方教程同样是"显式 Reset 按钮 + 显式 Reset Camera Transform"，
   ViewerEX 也把「重置模型位置 / 模型重置」放进编辑面板。【已验证·官方一手】
5. **命中测试是"点角色"的前置，且业界有"逐部件排除"的粒度**：VTS 支持在 ArtMesh 的 UserData 里写
   [`vts_ignore_raycast`](https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/Item-Scenes-and-Item-Hotkeys.md)
   ——「Have `vts_ignore_raycast` somewhere in the UserData field for that ArtMesh」
   ⇒ 让模型的某些部件**不参与点选**（例如背景层、发饰）。
   本项目 wasm 侧没有命中测试，若要做"点脸/点头顶"，必须先有 ArtMesh 或包围盒命中，
   否则只能做"点在画面内就算"——那会和拖拽/穿透冲突得更厉害；
   建议同时支持一个**逐 ArtMesh 的"忽略点选"标记**（对应 VTS 的先例）。【已验证·官方一手 + 已验证·本仓库 + 推断】
6. **复位要覆盖可见与不可见两类状态**：scale、offset（已有）；建议把"复位"扩展为
   一次性恢复 `scale/offset`（舞台）与可选 `DisplayPrefs` 默认值（面板已有「恢复默认」）。
   【已验证·本仓库】`display_panel.dart:137-146`、`main.rs:712-730`

---

## 5. 状态与反馈：状态机 → 视觉映射

### 5.1 本项目**已有的状态源**（不要另造一套）

| 状态源 | 取值 | 证据 |
|---|---|---|
| core `Phase` | `Idle` / `Thinking` / `Speaking`（注释：「驱动 shell 决定是否展示思考/说话 UI」） | 【已验证·本仓库】`crates/live2d-ai-core/src/state.rs:15-24` |
| core `PlaybackState` | `NeverStarted` / `Playing` / `Drained` / `Cleared` | 【已验证·本仓库】`state.rs:37-60` |
| WS `runtime_status.event` | `voice_started` / `voice_ended` / `new_epoch` / `shutdown_ready` | 【已验证·本仓库】`web_api/ws/events.rs:52-111` |
| WS `turn_state.status` | `completed` / `failed` | 【已验证·本仓库】`ws/events.rs:42-51` |
| WS `action_state.kind` | `perform` / `cease`（+ 建议的 `dropped`） | 【已验证·本仓库】`ws/events.rs:112-138` |
| 渲染面桥 `Live2DBridgePhase` | `loading` / `ready` / `error` / `destroyed`（+`progress`、`fps`） | 【已验证·本仓库】`shell/flutter/lib/live2d/live2d_bridge.dart:8-9,221-251` |
| WS 连接 `WsStatus` | `idle` / `connecting` / `connected` / `disconnected` / `closed` | 【已验证·本仓库】`shell/flutter/lib/api/ws_client.dart:140` |
| 打断（interrupted） | `StopRequested` 推进 epoch；`new_epoch` 通知 | 【已验证·本仓库】`state.rs:1-6`（epoch 唯一 owner）、`ws/events.rs:101-107` |

Flutter 现状：`_WsBadge` 用 5 态 + 颜色圆点 + 文字；舞台只显示 loading/进度与 FPS。
【已验证·本仓库】`shell/flutter/lib/main.dart:248-271`、`live2d_stage.dart:155-177`。

### 5.2 推荐映射（五态 + 动作表演正交层）

| 对外状态 | 判据（信号） | 舞台视觉 | 动效建议 | 依据 |
|---|---|---|---|---|
| **离线** | `WsStatus != connected` **或** 桥 `error` | 舞台顶部窄横幅「后端未连接 · 点此重试」；角色保持但降饱和 | 横幅滑入（200 ms），不闪 | 【已验证·本仓库】已有 `_StageErrorOverlay`+`_WsBadge` 可复用；【推断】降饱和是"没坏、只是没连上" |
| **空闲 Idle** | `Phase::Idle` | 无额外 UI；只留待机小动作 | — | 【已验证·本仓库】`state.rs:17-20` |
| **思考中 Thinking** | `Phase::Thinking`（`StartThinking` 效果） | 舞台**边缘呼吸光**（1.2–1.6 s 周期）+ 聊天气泡里的三点指示；FPS 徽标不动 | 低振幅、无声音（见下"反直觉"） | 【已验证·本仓库】`state.rs:22`；【已验证·官方一手】Google「earcon 增加认知负担，不直观」 |
| **说话中 Speaking** | `Phase::Speaking` / `voice_started` | 口型由 PCM 驱动（已有）；麦克风/名字处脉冲 | 脉冲与 `volume` 同源，不要独立节拍器 | 【已验证·本仓库】`ws_client.dart:102-111`（服务端 `volume` 是口型唯一来源） |
| **被打断 Interrupted** | 用户 `stop` → epoch 推进 → `new_epoch` | 角色**一次性回正**（头/身回中），聊天流式气泡收口为"已停止" | 150–250 ms 回正 + 徽标闪一次；**不要**做"震惊"表情（那是 AI 语义，不是系统语义） | 【已验证·本仓库】`action/mod.rs:215-226`（Interrupt 回中）、`ws/events.rs:101-107` |
| **动作表演中（正交层）** | `action_state{k:"perform"}` + 来源 | 动作卡显示进度环（wasm 知道总时长：`choreography_total_ms`）与来源徽标 | 进度环天然解释"为什么再点没反应"（AlreadyActive） | 【已验证·本仓库】`main.rs:398-405`、`action/mod.rs:294-299` |

### 5.3 结论

1. **状态不要新造**：UI 的"思考/说话/被打断"直接映射 `Phase` + `runtime_status` + `epoch`，
   三段信号的组合就够，且都已在 WS 上。【已验证·本仓库】
2. **延迟反馈优先用视觉与"应答语"，不用提示音**。Google 官方对 earcon 的判据：
   「Earcons impose **cognitive load** because users have to learn what they mean; they're **not intuitive**」
   「If you feel like you have to **teach users** what an earcon means, **don't use an earcon**」。
   官方推荐的另一条路是 **Acknowledgements**（短应答语）：
   [acknowledgements](https://developers.google.com/assistant/conversation-design/acknowledgements)——
   「Acknowledgements **assure users that their input was received**」（"Okay/Sure/Alright/Got it"）。
   ⚠️ 但要注意官方同时提醒「**acknowledgements are not discourse markers**」——
   应答语只表示"收到了"，**不能代替内容**；本项目里它对应"聊天气泡先回一个正在处理的状态"，
   而不是让角色说"嗯"来填空。
   ⇒ 本项目"思考中"的音效**应默认不做**；若必须做，必须能被用户关掉。
   **补充：Google 官方文档里没有任何"超过 N 秒就播 filler"的秒数阈值**（子调研逐页核对导航后**未核到**），
   因此本报告**不给**这类阈值。【已验证·官方一手】[Google Conversation Design · Earcons](https://developers.google.com/assistant/conversation-design/earcons)
3. **打断要区分"系统打断"与"角色反应"**：`stop` 造成的回正是机械语义（回中、清动作、收气泡），
   不要渲染成角色的情绪反应。core 的 `CeasePerforming` 已经把两者分开。【已验证·本仓库】`ws/events.rs:132-138`
   业界对"打断"的定义是"**立刻停 + 立刻接受新输入**"（Alexa barge-in：
   「Alexa knows to **stop speaking and proceed with processing the new request**」，见 §2.2）
   ⇒ 打断后**不要**插入确认步骤或过渡动画阻塞新输入。【已验证·官方一手】
4. **"刚刚这个动作是 AI 自己做的"要有专门的可视通道**：
   舞台内一次性"来源徽标"（AI/规则/你）+ 舞台旁的动作历史条（时间戳 + 动作 + 强度 + 来源），
   旧 JS 的 `actLog` 已是雏形。【已验证·本仓库】`web_api/app.js:99-107`
   业界同类：VTS 在事件里直接下发 `hotkeyTriggeredByAPI`【已验证·官方一手】；
   Warudo 在连线上跑一个球表示"控制流正在被触发"【已验证·官方一手】；
   Stream Deck 用键的两态图标表示"当前处于什么状态"【已验证·官方一手】。
5. **诊断/状态面板在同类产品里已有很重的先例**：VRChat 官方 Debug Menu
   「It pops up a **text display** that shows you the current **animator state** of your avatar with details on
   **parameter values, tracking states, current motion states**」【已验证·官方一手】
   ⇒ 本项目 §7 的"诊断面板"不是发明，而是行业标准配置，值得直接放进去。
6. **通知类反馈的位置惯例是"角落、可取消"**：VTS 在**右上角**给通知且允许取消挂起的模型加载
   （「a notification will be shown in the **top right** that allows you to **cancel** the pending model load」），
   插件列表把已连接者标为 "Active"，控制器断连给警告指示。【已验证·官方一手】
   ⇒ 本项目"模型切换中/加载中"应给**可取消**的角落通知，而不是全屏遮罩。
7. **FPS 徽标这类诊断信息不要占舞台主视线**：现在在右上角小徽标（`live2d_stage.dart:169-174`），
   建议移入"开发者"区或做成 hover 才出现。【推断】

---

## 6. 音量控制：「静音但保留口型」怎么表达才不困惑

### 6.1 本项目**已有三层**音量/静音语义（这是最容易让用户困惑的地方）

| 层 | 谁控制 | 效果 | 证据 |
|---|---|---|---|
| **服务端强制静音** | 环境变量 `LIVE2D_AI_MUTE_AUDIO=1`（跑测试/无人值守）；旧开关 `LIVE2D_AI_UNMUTE_AUDIO=1` 优先级最高 | 下发**全零 PCM**，但 `volume` 仍是**原始样本**的 RMS ⇒ **不出声、口型照动**；任何客户端都绕不过 | 【已验证·本仓库】`crates/live2d-ai-desktop/src/web_api/ws.rs:475-479`（`resolve_muted`）、`ws.rs:512-546`（`volume` 永远取原始样本）、`web_api/cli_entry.rs:370-391` |
| **客户端静音（本地）** | UI 开关 `DisplayPrefs.muted` | 只把播放增益置 0，**不断开音频图**（保证口型到期释放不漂移） | 【已验证·本仓库】`shell/flutter/lib/settings/display_prefs.dart`（`muted`/`volume` 注释）、`shell/flutter/lib/audio/audio_player.dart:181-232`（`playbackGain(volume, muted)`）；`AGENTS.md` §语音输出约定（2026-09-10 v0.4.3） |
| **客户端主音量** | UI 滑杆 `volume` 0..1 | 与静音**正交**：静音不重置音量，取消静音恢复原值；非线性感知压缩 | 【已验证·本仓库】`display_panel.dart:87-102`、`display_prefs.dart` |

服务端**默认出声**（`Broadcaster::default()` 的 `muted=false`，「产品默认必须出声」），
并且 `muted` 会随每个音频帧下发（`data.muted`），客户端**已经解析**该字段。
【已验证·本仓库】`ws.rs:117-125`、`ws.rs:544`；`shell/flutter/lib/api/ws_client.dart:109-110`
（注释：「服务端是否处于静音输出（`data.muted`，观测/诊断用）」）。

### 6.2 同类产品的惯例（可用于组织层级）

- **VTube Studio**：音量在**通用设置**里（[Sound](https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/Sound.md)：
  「In the **general settings** tab, you can control the **playback volume** and global sound playback settings.」），
  同时**把"音量开关"也做成一种热键类型**：「You can use a hotkey of the type **`Toggle Model Volume`**
  to toggle the volume of a model or Live2D item on/off.」【已验证·官方一手】
  ⇒ 音量既要有常驻控件（设置里），又要能被**动作/快捷键系统**引用（对直播/演出场景很重要）。
- **Stream Deck**：官方把"音量"当作**旋钮的典型连续量**（manifest 示例 `"Rotate": "Adjust Volume"`）
  【已验证·官方一手】⇒ 音量是"连续调节"的教科书归属，不该做成离散档位。
- **OBS（最有价值的一例：把"听不听得到"拆成两个轴）**【已验证·官方一手】
  [kb/audio-mixer-guide](https://obsproject.com/kb/audio-mixer-guide)：
  「**Mute Button** - A speaker icon to **mute and unmute the source to your output**」；
  「**Monitor Button** - Allows playing the audio of the source back through your **monitoring device**」。
  再往下一层是 `Edit → Advanced Audio Properties` 的 **Audio Monitoring**，三档（31.1.2 官方 locale 原文）：
  `Monitor Off` / `Monitor Only (mute output)` / `Monitor and Output`；
  底层语义（[obsproject.com/docs/reference-sources.html](https://obsproject.com/docs/reference-sources.html)）：
  `OBS_MONITORING_TYPE_NONE - Do not monitor`、
  `OBS_MONITORING_TYPE_MONITOR_ONLY - Send to monitor device, no outputs`、
  `OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT - Send to monitor device and outputs`。
  另有全局 `Monitoring Device` 设置与混音器右键 `Enable/Disable Monitoring`。
  ⇒ **"输出"与"监听"是两个正交轴**，OBS 用**两个不同名字的控件**承载（Mute / Monitor），
  而不是一个"静音"开关承担两种含义。
  注意 master 已把第三档改名为 `Monitoring Enabled`（不再写 "Monitor and Output"），
  属于**官方自己承认旧标签有歧义**的迭代（见 §8 反直觉）。
- **通用惯例（推断）**：主音量用**滑杆**、静音用**图标按钮 + 状态**（点击图标 toggle，滑杆保留值），
  这是几乎所有播放器的共同约定；本项目已符合。

### 6.3 结论：用「两个开关 + 一个只读徽标」消除困惑

1. **主音量滑杆 + 静音开关放一起，静音时滑杆不置灰、不归零**（保留用户原值），
   并在静音时给一行提示"当前已静音——取消静音后按此音量播放"。
   【已验证·本仓库】`display_panel.dart:97-102` 已有此实现，**保留**。
2. **必须把"服务端静音"表达成只读状态，而不是另一个"静音"开关**：
   读 WS 音频帧的 `data.muted`，在音量行显示二级徽标，例如
   「**服务端已静音**（`LIVE2D_AI_MUTE_AUDIO=1`）· 口型仍按语音驱动 · 需在启动参数里关闭」。
   理由：**两个同名开关指向两个不同真相**，用户按了本机静音却发现根本没声音过，会以为 UI 坏了。
   【已验证·本仓库】两层的存在与语义；【推断】文案形式的建议。
3. **文案分工要写清"听不到" vs "没发声"，并且不要都叫"静音"**（OBS 因同类歧义改过两次名，见 §8）：
   - 本机开关 → 标签写成「**静音（本机播放）**」，说明「**你这台设备听不到**（口型保留）」
   - 服务端状态 → 只读徽标「**服务端静音中**（`LIVE2D_AI_MUTE_AUDIO=1`）」，说明「**服务端没有发出声音**
     （任何客户端都听不到，需在启动参数里关闭）」
   ⇒ 规则：**同一个词不能同时命名"我能听到"与"有没有声音被生产出来"这两个轴**。
     两个轴要给两个名字——这正是 OBS 把 `Monitor and Output` 改名 `Monitoring Enabled` 的教训。
4. **官方/产品默认出声**：`muted` 默认 false（`AGENTS.md` v0.4.3 的裁决：「『不出声』必须显式要求」），
   因此**不要**在 UI 上默认勾选任何静音；测试/无人值守用服务端 env。
   【已验证·本仓库】`ws.rs:117-125`、`AGENTS.md` §语音输出约定
5. **「静音但保留口型」不需要新开关**——它已经是静音的**定义**：静音=增益 0，口型来自
   `volume`/本地包络，二者正交。UI 只需**如实说明**（现有副标题「只关声音、保留口型」即达标）。
   【已验证·本仓库】`display_panel.dart:113`、`audio_player.dart:246-249`

---

## 7. 开发者 / 诊断面板

### 7.1 本仓库已有的诊断素材与门控

- `dev_mode` 门控日志端点：`GET /api/v1/logs` 与 `GET /api/v1/logs/levels`（头注明写 dev_mode 门控）。
  【已验证·本仓库】`web_api/mod.rs:28-29,128-131`
- `GET /api/v1/app/status`：`uptime_s`/`started_at`/`config_path`/`active_model_id`/`current_epoch`/`dev_mode`，
  以及 `llm`/`tts` 的 `has_api_key`/`base_url`/`model` 脱敏摘要。
  【已验证·本仓库】`web_api/dto.rs:52-95`、`app_routes.rs:150-186`
- **`status.audio` 目前是占位常量**：`backend: "none"`、`available: false`（硬编码）。
  【已验证·本仓库】`web_api/app_routes.rs:179-184`
  ⇒ **诊断面板绝不能把这两个字段当真展示**（会显示假信息）；要展示就得先接通真实后端探测。
- 旧 JS 的诊断雏形：日志区按 `dev_mode` 显隐、级别下拉（`/api/v1/logs/levels`）、
  行级过滤（`filterLogLines`，warn 保留 warn+error；无级别行始终保留）、
  以及设置页的**连接测试显示 `r.latency_ms`**。
  【已验证·本仓库】`web_api/app.js:249-263,744-790,766`、测试用例 `crates/live2d-ai-desktop/tests/webapp_behavior.rs:120-135`
- 渲染面已上报：`progress`（已 fetch/总数）、`fps`、`ready`/`loaded`/`error`。
  【已验证·本仓库】`live2d_bridge.dart:221-251`、`live2d_stage.dart:179-188`
- 桌面壳已有能力簿记（会话线索 / 声明能力 / 实测后端 / 运行时能力），并对"预期受限"给出可读降级说明。
  【已验证·本仓库】`crates/live2d-ai-desktop/src/platform/{capabilities,window}.rs`（`window.rs:50-59`）

### 7.2 同类产品的可迁移模式

**OBS（诊断：日志可上传、可复制 URL、可送分析器）**【已验证·官方一手】
官方 locale [en-US.ini](https://github.com/obsproject/obs-studio/blob/master/frontend/data/locale/en-US.ini)：

- 菜单项：`Show Log Files` / `View Current Log` / `Upload Current Log File`
  （`Basic.MainMenu.Help.Logs.*`；旧版 `27.2.4` 的对应键名是 `Upload &Last Log File`，
  master 已改为 `Upload &Previous Log File` —— **同一功能改过名**）。
- **上传对话框带两个按钮**：`LogUploadDialog.Buttons.CopyURL="Copy Log URL"` 与
  `LogUploadDialog.Buttons.AnalyzeURL="Analyze Log File"`；
  另有网页版 Analyzer（`obsproject.com/tools/analyzer`，页面为 JS 壳，**正文未核到**）。
- 这正是"**一键复制诊断信息**"的工业级做法：**先上传（得到可分享 URL）→ 再复制 URL / 或直接分析**，
  而不是让用户从日志里手抄。
- **Stats 窗口只有 `Reset Stats`，没有复制**（master locale 无对应字符串；**未核到**复制入口）
  ⇒ 反面教材：**能复制的只有日志，统计信息只能截图**。本项目应避免这种割裂。

**VRChat（Debug Menu = 文字状态面板）**【已验证·官方一手】
[docs.vrchat.com · Action Menu](https://docs.vrchat.com/docs/action-menu)：
「It pops up a **text display** that shows you the current **animator state** of your avatar with details on
**parameter values, tracking states, current motion states**」
⇒ 诊断面板的"第一屏"形态：**纯文本、可滚动、含参数当前值**。与本项目"单参数滑杆 + 当前值"完全同构。

**Live2DViewerEX（编辑面板开在模型前）**【已验证·官方一手】
官方手册：「会在**模型前**弹出编辑面板，按照提示进行模型编辑」
⇒ "调参"与"看效果"必须在同一视野内（本项目舞台与面板同屏，已满足）。

**Warudo（复位的显式化）**【已验证·官方一手】
「set the **Scale** values to **1**, or click on the **Reset** button」/「click **Reset Camera Transform**」
⇒ 开发者/高级面板里每个可调量都要有**显式复位**，而不是靠用户记住默认值。

**VTube Studio（相关性 ID —— 很值得抄）**【已验证·官方一手】
[官方 API 文档](https://raw.githubusercontent.com/DenchiSoft/VTubeStudio/master/README.md)：

- 每个请求可带 `requestID`：「This ID will also be used to **log the request in the VTube Studio logs along with any errors**.
  If anything goes wrong, you can use this as reference to check for any errors related to this request in the logs.」
  ⇒ **"一次操作一个可复制的追踪 ID"**是给用户报障的最低成本设计。
- 运行统计可查：`uptime`、`framerate`、`allowedPlugins`、`connectedPlugins`、`startedWithSteam`、窗口尺寸。
  ⇒ 正好对应"诊断面板第一屏"该放什么。

**VTS（日志敏感性）**：键盘绑定不对插件暴露（`keyCombination` 恒空，出于安全）。
⇒ 诊断导出必须**默认脱敏**；本项目红线亦然（密钥不进 GET/日志/WS/导出，见 `AGENTS.md` §核心层）。

**VTS（可读日志的另一半）**：热键名会写进日志
（[Model Settings](https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/VTS-Model-Settings.md)：
「You can give hotkeys names that are also **shown in the logs** when the hotkey is activated.」）
⇒ 日志里出现的是**人能读懂的语义名**，而不是内部 id；本项目动作日志同理应用中文名。

**VSeeFace（日志入口 + 溢出到 ini）**【已验证·官方一手】
[官方手册](https://raw.githubusercontent.com/emilianavt/VSeeFaceManual/master/README.md)：
`Open logs` 按钮；以及"界面放不下的项落 `settings.ini`"
（「You might be able to **manually enter** such a resolution in the `settings.ini` file.」）
⇒ 这是最省事的"高级区"实现，但**可发现性极差**，只该作为最后手段。

**Stream Deck（来源可见性）**：SDK 让动作知道自己是"被复合动作触发的、期望什么状态"
（`ev.payload.isInMultiAction` / `userDesiredState`）【已验证·官方一手】
⇒ 与本报告 §2 的主张一致：**触发的来源与意图应当是数据，而不是 UI 的猜测**。

### 7.3 结论：三层门 + 一键复制

1. **第一层（所有用户）「连接与延迟」**：
   WS 状态（`WsStatus` 5 态）、最近一次 `heartbeat` 的 `seq`/`ts`、渲染面 `progress`/`fps`、
   桥 phase、协议版本（`capabilities.ws_protocol_version`）、后端 `uptime_s`/`active_model_id`/`current_epoch`。
   【已验证·本仓库】各字段均已存在。
   **注意**：本项目 WS 是**单向事件流**（服务端不读客户端帧），所以**无法 ping/pong 测 RTT**；
   只能展示"事件时间戳与本地时钟的差"（同机场景有效）。
   【已验证·本仓库】`ws_client.dart:142-152`、`web_api/ws/connection.rs:225-238`
   ⇒ 文案用「**事件滞后**」而非「网络延迟」，避免假指标。
2. **第二层（dev_mode）「日志 + 单参数」**：日志查看/级别过滤（复用已 dev_mode 门控的端点）、
   单参数滑杆与覆盖释放、动作仲裁观测（建议新增的 `dropped` 帧）。
   【已验证·本仓库】`mod.rs:128-131`；【推断】后两项。
3. **第三层（危险操作，全部在 dev_mode 且二次确认）**：模型导入/激活/删除、Mod enable/disable/restart/config。
   【已验证·本仓库】`models_routes/mod.rs:71-75`、`mods_routes.rs:56-141`
   理由：这些操作的后果不在"显示"层（会改磁盘/热重载），按 NN/g 的判据属于"回不去"的一类，应后置。
4. **「一键复制诊断信息」**（强烈建议做，成本低收益高）：
   - 复制内容：`version`+`schema_version`+`uptime_s`+`epoch`+WS 状态与最近 seq/ts+FPS+桥 phase+
     协议版本+最近 N 条 `action_state`（含 source 与 drop 原因）+ 最近 N 条日志级别统计。
   - 形态：设置页「复制诊断信息」按钮（成功给 toast）；日志区提供"复制全部/复制筛选后"；
     渲染面/后端请求带可追溯 ID（对标 VTS `requestID`）；若将来要远程排障，
     参照 OBS 的"**上传 → 复制 URL / 一键分析**"两步式，而不是只给一堆文本。
   - 依据：VTS 的 `requestID` 先例（一次操作一个可追溯 ID）【已验证·官方一手】；
     OBS 的 `Copy Log URL`+`Analyze Log File`+网页 Analyzer 三件套【已验证·官方一手】；
     本项目已有 `seq`/`ts`/`epoch` 可直接进快照【已验证·本仓库】。
5. **不要做"看起来很专业"的假指标**：`status.audio` 是占位（`app_routes.rs:179-184`）、
   `sync.paused` 渲染面不认（`live2d_bridge.dart:94,104` vs `main.rs` 无该分支）——
   这类字段要么接通，要么**在 UI 上不存在**。【已验证·本仓库】
6. **诊断面板要能回答"谁触发了这个动作"**（不只是"发生了什么"）：
   建议在动作历史里同时给 `source`、强度、epoch 与（若为 LLM）`reason`，
   并允许"复制这一条"。先例：VRChat Debug Menu 展示 animator state / 参数值 / 追踪状态与动作状态；
   VTS 在下发事件里就带 `hotkeyTriggeredByAPI`。【已验证·官方一手 + 已验证·本仓库】

---

## 8. 反直觉发现汇总（本报告最重要的部分）

1. **"自动 vs 手动"的业界答案不是总开关，而是"用户永远优先 + 抢占可观测"**。
   VTS 的外部触发走**冷却+队列**（5 帧/次、队列 32）而不是丢弃；本仓库 core 用
   user 100 > llm 90 > rule 50 的优先级，同优先级"后来者胜"。
   【已验证·官方一手 + 已验证·本仓库】`action/mod.rs:110-130,280-317`、VTS API 文档。
2. **用户一旦打开界面/对话框，自动化会被冻结**（VTS `canLoadItemsRightNow`：
   「if the user has certain menus or dialogs open … This generally prevents actions such as loading items,
   **using hotkeys and more**」）。直觉把面板当"旁观者"，业界把它当"暂停自动化的信号"。【已验证·官方一手】
3. **"静音"在本项目里有两个互不相同的真相**：服务端 env 级静音（零 PCM + 真实 volume）与
   客户端增益静音（只影响本机）。两者**都**保留口型，且服务端默认是**出声**。
   一个滑杆两侧各有一个"静音"，若不区分会让用户以为控件坏了。【已验证·本仓库】
4. **8 是表达槽位的隐性常数**：VRChat「Up to **8** controls per menu」与 VTS
   `onScreenButtonID`「**1 (top) to 8 (bottom)**」两个独立产品撞在一起。【已验证·官方一手（双源）】
5. **"思考中"加提示音与直觉相反**：Google 官方明确说 earcon「impose **cognitive load**」「**not intuitive**」，
   并给出判据「If you feel like you have to **teach users** what an earcon means, **don't use an earcon**」。
   ⇒ 延迟反馈优先视觉/文案。【已验证·官方一手】
6. **点击穿透没有"逐像素"这一档**：winit `set_cursor_hittest` 是**整窗布尔**且 **Web 永远不支持**；
   Electron 的等价物 `setIgnoreMouseEvents` 也是全窗，只能靠 `forward` 转发鼠标移动来实现动态切换。
   ⇒ "只有角色透传、其它区域正常"这类需求在桌面壳里做不到，必须动态切换 + 保命恢复入口。
   【已验证·官方一手】；本项目已有正确安全规则【已验证·本仓库】`user_event.rs:54-58`
7. **"再点一次"不会重播**：core 对完全相同的动作返回 `Dropped(AlreadyActive)`（幂等），
   而 `Dropped` **只进 tracing 日志**、不上 WS ⇒ 用户点按钮"没反应且没有任何解释"。
   VTS 的做法恰恰相反（队列）。这是本项目控制台**最该先补的反馈闭环**。
   【已验证·本仓库】`action/mod.rs:294-299`、`supervisor/handlers.rs:244-246`
8. **能力快照已经存在，但 UI 若不用它就会自相矛盾**：`capabilities.actions` 是"这个模型支持什么"的
   唯一权威；动作按钮该由它驱动显隐/置灰，而不是硬编码 6 个。
   【已验证·本仓库】`dto.rs:21-50`、`dto.rs:268-290`；旧 JS 已按能力驱动（`resolveActions()`）【已验证·本仓库】`app.js:276-290`
9. **"这动作是谁触发的"在同类产品里是协议字段，不是 UI 的推断**：VTS 的 Events 负载直接给
   `hotkeyTriggeredByAPI: false/true`。【已验证·官方一手】
   ⇒ 本项目已经有等价字段（`action_state.data.action.source`），**却在前端解析时被丢掉**——
   这不是设计缺失，是**接线缺失**，修它比设计新机制更划算。【已验证·本仓库】
10. **"用户优先"不是普适常量**：VSeeFace 在收到外部 blendshape 时，**手动表情热键会失效**
    （外部驱动 > 手动）。所以"谁在控制"必须由**每个产品自己定优先级并显式展示**，
    不能假设用户永远赢。本项目 core 选择用户 100 > LLM 90 > rule 50，是**一个选择**，
    因此更需要把"AI 想动但被压住/正在演"画出来。【已验证·官方一手 + 已验证·本仓库】
11. **桌宠的"拖拽 vs 点击"主流解法不是位移阈值，而是时间阈值 + 右键菜单**：
    VPet 用可配置的 `PressLength` 长按时长分流（源码），Shimeji 按下即开始拖拽、右键弹菜单（源码）；
    位移阈值（1 px 死区 / 20 px 状态切换）只作辅助。【已验证·官方源码（双源）】
    ⇒ 本报告最初按"6 px 位移阈值"设想的方案应改为"**时间阈值为主、位移阈值为辅、右键专职菜单**"。
12. **"点击穿透"的反面思路：Shimeji 没有穿透开关，它维护一张"交互窗口白名单"**
    （`conf/settings.properties`: `InteractiveWindows=Notepad/Chat/Steam/Friends`）【已验证·官方源码】。
    ⇒ 与"让窗口忽略鼠标"相反：它是"**让桌宠只对特定窗口敏感**"。
    对桌宠场景（用户想一边用别的软件一边让角色不挡路），**白名单可能比全局穿透更符合意图**，
    而且没有"穿透后无法恢复"的风险——本项目已有托盘恢复入口，可两条路都留。【已验证·官方源码 + 推断】
13. **径向菜单不是行业共识**：VRChat 用它，理由是「quickly perform those actions **one handed** and in a
    **less obvious manner** to those around you」（**单手 + 不显眼**，不是"更快"）；
    Warudo 官方文档里**没有**径向/屏幕操作菜单（触发即节点图）；
    Live2DViewerEX 用"浮球 + 两级面板"。【已验证·官方一手（三源）】
    ⇒ 以"别人都做径向"为理由做径向是站不住的。【已验证·官方一手 + 推断】
14. **"一个词同时指两个轴"是公认的失败设计，而且大厂也在反复改**：
    OBS 的 `Monitor and Output` 在 master 被改名为 `Monitoring Enabled`，
    `Upload Last Log File` 被改成 `Upload Previous Log File`——**官方自己承认旧标签有歧义**
    【已验证·官方一手 locale 差异】。
    ⇒ 本项目 §6 的"静音"正是同一个坑（本机增益静音 vs 服务端源头静音），
    **必须在文案上分成两个名字**，不能指望用户从上下文推断。
15. **渐进披露不是"越能藏越好"，而是"超过 2 层就变差"**：
    NN/g 官方原文「**designs that go beyond 2 disclosure levels typically have low usability**」
    【已验证·官方一手】。⇒ 本报告因此**自我否决**了"普通区 + 快捷入口 + 高级面板"的三层方案（见 §9 决策 6）。
16. **手动接管有两种语义，其中一种更安全但更烦**：Nest 的 hold 是**模态**的——
    「**If a hold is active, you must end that hold first. Then, you can change the temperature.**」
    【已验证·官方一手】；而本项目 core 采用**优先级式**（用户 100 直接抢占，不需要先"结束"什么）
    【已验证·本仓库】。两者都不是错，但**用户完全无法从画面判断当前是哪种**——
    所以"AI 是否持有控制权"必须显式可见。
17. **"一键复制诊断"这件事连成熟产品也只做了一半**：OBS 的日志有 `Copy Log URL` + `Analyze Log File`，
    但 **Stats 窗口只有 `Reset Stats`，没有复制**（官方 locale 无对应字符串）【已验证·官方一手】
    ⇒ 本项目要把**统计与日志放在同一个可复制快照**里，别重蹈覆辙。
18. **"打断"的行业定义是"立刻停 + 立刻接受新输入"，而不是"停住等人确认"**：
    Alexa barge-in「Alexa knows to **stop speaking and proceed with processing the new request**」
    （含 "contextual barge-in"）【已验证·官方一手（Amazon Science）】。
    ⇒ 本项目的 `stop` 之后**不该**再加确认步骤/阻塞式过渡。

---

## 9. 给本项目的 10 条具体设计决策

> 每条 = 结论 + 依据 + 落点（前端/协议/后端），**不写实现代码**。

1. **触发形态定为「常驻工具条（3 个高频）＋ 面板卡片网格（6 动作 × 强度 1/2/3）」，不做径向菜单。**
   依据：VRChat 8 控件/页与 VTS 8 屏幕按钮确立"≤8 槽位"惯例，且 VTS 的屏幕按钮形态是
   **半透明圆形浮标贴主屏右侧**【已验证·官方一手】；而**径向并非共识**——Warudo 官方文档零径向、
   Live2DViewerEX 用"浮球 + 两级面板"、VRChat 的官方理由是"单手 + 不显眼"而非效率【已验证·官方一手（三源）】。
   **落点**：`shell/flutter/lib/live2d/live2d_stage.dart` 的 Stack 内新增舞台浮标/工具条（≤8 槽，先放 6）；
   动作面板沿用 `DisplayPanel` 的组织方式（分组 + 说明文案 + 恢复默认）。
2. **手动触发只走 `POST /api/v1/commands/{id}/invoke`，不新开"舞台直发 action-state"旁路。**
   依据：该通道恒定 `user_command`（优先级 100），是"谁在控制"的权威来源【已验证·本仓库】`commands_routes.rs:229-235`；
   旧 JS 的 `postToStage({type:"action-state"})` 属于绕过仲裁的直连，迁移时**不要再抄**。
3. **补齐"三态回执"并显示它**：`HTTP accepted(已排队)` → `action_state{perform}(core 已受理)` →
   `stage-ack(渲染面已生效)`。三者都要可见（至少 hover/日志可见），并对
   `Dropped(AlreadyActive/LowerPriority/CapabilityMissing)` 给出人话提示。
   依据：invoke 响应语义是"已入队"而非"已播放"【已验证·本仓库】`commands_routes.rs:79-95`；
   渲染面 ACK 设计已存在但被 Flutter 忽略【已验证·本仓库】`main.rs:270-273`、`live2d_bridge.dart:249-250`；
   `Dropped` 只进日志【已验证·本仓库】`handlers.rs:244-246`。
4. **动作归属做成"每条动作都带来源徽标"**（你 / AI / 规则），AI 触发的用不同色相并允许展开 `reason`；
   外部触发还应有**独立的绑定/授权入口**（对标 VTS 每个热键旁的 Twitch 图标与 API 权限弹窗）。
   **落点**：修 `shell/flutter/lib/api/ws_client.dart:68-80,303-310` 的解析
   （`data.action` 是对象 `{action,strength,source}`，不是字符串）。
   依据：服务端已下发 `source`【已验证·本仓库】`ws/events.rs:121-131`；
   VTS 在事件里直接下发 `hotkeyTriggeredByAPI`，证明"来源是协议字段"是行业做法【已验证·官方一手】；
   LLM 工具契约里已有 `reason`【已验证·本仓库】migration R3。
5. **不做"自动/手动"总开关；做"手动优先 + 临时接管"**：手动动作抢占 AI 动作时，UI 给约 5 s 的
   "手动接管中"徽标；同时把 `ActionEffect::Dropped` 投影成 WS 帧（AI 想动但被压住也要可见）。
   依据：core 优先级已定死（user > llm > rule）【已验证·本仓库】`action/mod.rs:110-130`；
   "所有自动化状态都要有用户可点的关闭入口"【已验证·官方一手】VTS 官方原话。
6. **参数分级严格两层：普通区 8 项（缩放 / 口型灵敏度 / 主音量 / 静音 / 口型同步 / 待机小动作 /
   允许拖动缩放 / 动作面板入口），其余（tier、头身角度上限、参数曲线、单参数滑杆、物理档）
   全部进 dev_mode 高级区；不要再造第三个入口。**
   判据 = NN/g 渐进披露四问：可见后果？需模型知识？能一键回？一分钟内会调第二次？
   依据：NN/g 官方在给出定义与收益的同时明确 **「designs that go beyond 2 disclosure levels
   typically have low usability」**【已验证·官方一手】——因此"普通区 + 快捷入口 + 高级面板"的三层方案
   **被本报告否决**；ViewerEX 的"快捷菜单/完整面板"恰好是两层、Warudo 官方
   「Many options are for **advanced users**…could look **intimidating**」也是两层【已验证·官方一手】；
   VTS 的 `Hide in list`+`Show All` 是模型侧的同类机制【已验证·官方一手】；
   本项目 `param_scale.rs` 硬编码 ±30/±10，暴露未标定滑杆会放大误差【已验证·本仓库（相邻调研）】。
   形态补充：**离散档（强度 1/2/3）用分段按钮，连续量（音量/缩放/灵敏度）用滑杆**；
   开发者区的多个单参数滑杆可借用 Stream Deck 的"dial stack"思路（选中参数 + 一个滑杆 + 显式释放/复位）。
   【已验证·官方一手（Stream Deck）+ 推断（本项目映射）】
7. **舞台交互分两层、四个开关，文案不许混**：
   ①窗口层「置顶」「点击穿透」（托盘 + 设置页，穿透时必须常驻恢复提示）；
   ②画布层「允许拖动与缩放」（即现有 `clickEnabled`，改名！）与「点击角色有反应」（新能力）。
   依据：`clickEnabled` 现在禁的是拖拽/滚轮/双击，**不是**点击互动【已验证·本仓库】`main.rs:602-717`；
   穿透是整窗布尔、Web 不支持【已验证·官方一手】winit/Electron。
8. **拖拽 vs 点击用「时间阈值为主 + 位移阈值为辅 + 右键专职菜单」分流，长按留给第二类互动。**
   依据：VPet 用可配置的 `PressLength` 长按时长分流、并保留右键给工具栏；Shimeji 右键弹菜单；
   VPet 的位移判定是 1 px 死区 + 20 px 状态切换（辅助而非主判据）【已验证·官方源码（双源）】；
   ViewerEX 用"浮球拖动/点击/长按"三手势【已验证·官方一手】。
   现状需改：本项目 `pointerdown` **立即**进入拖拽、无阈值【已验证·本仓库】`main.rs:594-620`；
   且拖拽只改 `offset_x/offset_y` 不改模型参数（**保持**）【已验证·本仓库】`main.rs:650-660`。
   配套：若要做"点角色"，先补 ArtMesh/包围盒命中与**逐部件"忽略点选"标记**
   （对标 VTS 的 `vts_ignore_raycast`）【已验证·官方一手】。
9. **状态用五态（离线/空闲/思考/说话/被打断）+ 动作表演正交层；延迟反馈不用提示音。**
   依据：状态源齐全（`Phase`、`PlaybackState`、`runtime_status`、`epoch`）【已验证·本仓库】；
   Google 官方反对 earcon【已验证·官方一手】；打断的回正属系统语义，不得渲染成角色情绪【推断+已验证·本仓库】。
10. **音量做「滑杆 + 本机静音开关 + 服务端静音只读徽标」三联，并明确文案分工**
    （"你这台设备听不到" vs "服务端没有发出声音"）；默认出声；静音=增益置 0、不断音频图。
    依据：三层语义已存在于代码【已验证·本仓库】`ws.rs:475-546`、`display_prefs.dart`、`audio_player.dart:181-232`；
    `AGENTS.md` v0.4.3「默认出声」与"音量/静音与口型正交"。

**额外一条（强烈建议，不计入 10 条）**：把「一键复制诊断信息」做进设置页，
快照包含 `version/schema_version/uptime_s/epoch/WS 状态与最近 seq,ts/FPS/桥 phase/协议版本/最近 N 条 action_state(含 source 与 drop 原因)`；
日志区提供"复制筛选后"。依据：VTS 的 `requestID` 可追溯性先例【已验证·官方一手】；
本项目已具备全部字段【已验证·本仓库】。

---

## 10. 未核到 / 待补清单

以下项在撰写时**取不到一手证据**，不做结论：

1. **OBS 的 Analyzer 正文与 Stats 复制**：`obsproject.com/tools/analyzer` 是 JS 壳（无正文）；
   master locale 里**没有** Stats 窗口的复制字符串（只有 `Reset Stats`）——
   即"是否存在复制入口"**未核到**（倾向为不存在）。
2. **Alexa 开发者文档**：`developer.amazon.com` 的 barge-in / AVS 文档页**逐一 404**（疑似已下线迁移），
   本报告只用 Amazon Science 官方博客的定义（已注明为博客而非文档）。
   Alexa 的"思考/处理中"状态与灯环颜色约定：**未核到**。
3. **Google 的延迟阈值与 barge-in 官方页**：逐页核对 Conversation Design 导航后，
   **没有**任何"N 秒后播 filler"的官方阈值；barge-in 也**未核到**官方专页
   （本报告因此不给阈值，见 §5.3）。
4. **Nest 的字面词 "temporary hold" / "manual mode"**：官方现用 **hold**（已实读），
   旧术语与"自动到期"的确切规则**未核到**（旧 answer ID 404）。
5. **SAE J3016 / NHTSA 的 takeover request**：`nhtsa.gov` 返回 **HTTP 403**，J3016 需付费，**未核到**。
6. **"径向菜单在鼠标/触屏下更慢"的定量证据**：**未核到**；本报告只用"三个产品里只有 VRChat 用径向"
   这一**定性**事实（见 §1.3、§8），其余判断标【推断】。
7. **VTS 的滚轮缩放与独立 advanced 分区**：官方文档未见，**未核到**；
   VTS 屏内"这次动作从哪来"的即时提示也**未核到**（但事件里有 `hotkeyTriggeredByAPI`）。
8. **VSeeFace 的剩余空白**：官方手册未见**径向菜单**、**模型拖拽/点击穿透**、
   **输出音量/静音**、以及**热键列表界面形态**（热键绑定只在 Expression settings 里提了一句）。
9. **Warudo 的剩余空白**：官方文档**没有** MIDI 设置页（全 docs 树只有一句示例）；
   **没有**任何径向/屏幕操作菜单。
10. **桌宠的剩余空白**：**Live2DPet** 未取源；**Live2DViewerEX** 官方手册未见热键列表、
    滚轮缩放、点击穿透开关（Steam 社区帖提到"点穿"但**未抓取**，故不引用）。
11. **Stream Deck 帮助中心**：`help.elgato.com` 两次 403（Cloudflare），
    "如何使用 Multi Action"的 explorer 页为 JS-only ⇒ 该文**未核到**；
    但 `docs.elgato.com` 的 profiles/dials/keys 三页已提供足够证据（见 §1.1）。

---

## 附录 A：引用清单（外部）

| 主题 | 来源 | 类型 |
|---|---|---|
| VRChat Expression Menu（8 控件/页、6 种控件、Puppet 语义） | https://raw.githubusercontent.com/vrchat-community/creator-docs/main/Docs/docs/avatars/expression-menu-and-controls.md | 官方一手 |
| VRChat Action Menu（三层 UI、桌面 R 键、Debug Menu 文字状态面板） | https://docs.vrchat.com/docs/action-menu | 官方一手 |
| VRChat OSC 参数（外部驱动同名参数、config 为 stop-gap） | https://docs.vrchat.com/docs/osc-avatar-parameters | 官方一手 |
| VRChat 设计意图（单手 + 不显眼） | https://docs.vrchat.com/docs/what-is-avatars-30 | 官方一手 |
| VTube Studio 表达式/Hotkeys/隐藏参数/分组/Add-Multiply | https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/Expressions-(a.k.a.-Stickers-or-Emotes).md | 官方一手 |
| VTube Studio 模型设置（8 屏幕按钮形态、≤2 键+鼠标键、输入→输出映射、热键名进日志） | https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/VTS-Model-Settings.md | 官方一手 |
| VTube Studio 公开 API（热键列表/触发、冷却与队列、权限、`canLoadItemsRightNow`、`userCanStop`、`requestID`、统计、自动化期间禁止手动拖动） | https://raw.githubusercontent.com/DenchiSoft/VTubeStudio/master/README.md | 官方一手 |
| VTube Studio Events（`hotkeyTriggeredByAPI`） | https://raw.githubusercontent.com/DenchiSoft/VTubeStudio/master/Events/README.md | 官方一手 |
| VTube Studio 音量（General settings + `Toggle Model Volume` 热键） | https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/Sound.md | 官方一手 |
| VTube Studio Twitch 触发器（每热键旁图标、未绑定置灰） | https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/Twitch-Hotkey-Triggers.md | 官方一手 |
| VTube Studio 逐 ArtMesh 忽略点选（`vts_ignore_raycast`） | https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/Item-Scenes-and-Item-Hotkeys.md | 官方一手 |
| VSeeFace 官方手册（Expression 热键、外部 blendshape 使热键失效、advanced 里的 VRChat OSC、Open logs、FOV 边缘条） | https://raw.githubusercontent.com/emilianavt/VSeeFaceManual/master/README.md | 官方一手 |
| Warudo Blueprints（节点=触发器、连线"滚动的球"、单向 flow 规则） | https://docs.warudo.app/docs/blueprints/understanding-blueprints | 官方一手 |
| Warudo 新手/高级界线、缩放与相机复位、动画节点与 Mask | https://docs.warudo.app/docs/tutorials/getting-started | 官方一手 |
| Warudo Stream Deck 集成（Toggle 用图标颜色表状态） | https://docs.warudo.app/docs/blueprints/miscellaneous/streamdeck-integration | 官方一手 |
| Live2DViewerEX 官方手册（两级控制台、浮球拖动/点击、模型长按、7 组面板、8 模型槽、重置位置、小部件锁定） | https://live2d.pavostudio.com/doc/zh-cn/pc/manual/ | 官方一手 |
| VPet 源码（长按时长 `PressLength` 分流、1 px 死区、20 px 状态切换、右键工具栏） | https://raw.githubusercontent.com/LorisYounger/VPet/main/VPet-Simulator.Core/Display/Main.xaml.cs | 官方源码 |
| Shimeji-ee 源码（按下即拖拽、右键弹菜单、5 px 仅重置动画、`InteractiveWindows` 白名单、置顶） | https://raw.githubusercontent.com/gil/shimeji-ee/master/src/com/group_finity/mascot/Mascot.java · .../action/Dragged.java · .../conf/settings.properties | 官方源码 |
| Stream Deck 设备与组织（键/旋钮数量、profiles/pages/folders/multi-action） | https://docs.elgato.com/stream-deck/profiles/getting-started/ | 官方一手 |
| Stream Deck 旋钮（dial + touch strip、Push/Rotate/LongTouch） | https://docs.elgato.com/streamdeck/sdk/guides/dials/ | 官方一手 |
| Stream Deck 键（两态、showOk/showAlert、`isInMultiAction`/`userDesiredState`） | https://docs.elgato.com/streamdeck/sdk/guides/keys/ | 官方一手 |
| Stream Deck dial stack（一键多动作、按下切换） | https://www.elgato.com/eu/hu/explorer/products/stream-deck/what-is-a-dial-stack-for-stream-deck/ | 官方一手 |
| Loupedeck（旋钮作为独立控件类） | https://support.loupedeck.com/loupedeck-device-features.html?hsLang=en | 官方一手（内容薄） |
| OBS 界面术语（日志菜单、日志上传对话框 `Copy Log URL`/`Analyze Log File`、Stats 字段、Audio Monitoring 三档、Studio Mode 标签、热键冲突警告） | https://github.com/obsproject/obs-studio/blob/master/frontend/data/locale/en-US.ini （旧版：`.../blob/31.1.2/frontend/data/locale/en-US.ini`、`.../blob/27.2.4/UI/data/locale/en-US.ini`） | 官方一手 |
| OBS 混音器（Mute 与 Monitor 两个轴） | https://obsproject.com/kb/audio-mixer-guide | 官方一手 |
| OBS 监听三档的底层语义 | https://obsproject.com/docs/reference-sources.html | 官方一手 |
| OBS Studio Mode（"观众看到的" vs "你编辑的"） | https://obsproject.com/kb/obs-studio-overview | 官方一手 |
| OBS 热键默认表 | https://obsproject.com/kb/keyboard-shortcuts | 官方一手 |
| 渐进披露定义、收益与 **"超过 2 层即低可用性"** | https://www.nngroup.com/articles/progressive-disclosure/ | 官方一手 |
| Google Conversation Design · Earcons（"earcon 增加认知负担"） | https://developers.google.com/assistant/conversation-design/earcons | 官方一手 |
| Google Conversation Design · Acknowledgements（短应答语） | https://developers.google.com/assistant/conversation-design/acknowledgements | 官方一手 |
| Google Conversation Design · Discourse markers | https://developers.google.com/assistant/conversation-design/discourse-markers | 官方一手 |
| Alexa barge-in 定义（官方博客） | https://www.amazon.science/blog/change-to-alexa-wake-word-process-adds-natural-turn-taking | 官方一手（博客） |
| Nest 恒温器 "hold" 语义与模态规则 | https://support.google.com/googlenest/answer/10120172 | 官方一手 |
| Electron `setIgnoreMouseEvents` + `forward` | https://www.electronjs.org/docs/latest/api/browser-window#winsetignoremouseeventsignore-options | 官方一手 |
| winit `set_cursor_hittest`（整窗布尔、Web NotSupported） | https://docs.rs/winit/latest/winit/window/struct.Window.html#method.set_cursor_hittest | 官方一手 |

## 附录 B：本报告引用的仓库文件（撰写时快照 HEAD `28756088`，工作区非干净）

后端：`crates/live2d-ai-desktop/src/web_api/{dto,command_registry,commands_routes,app_routes,log_routes,mods_routes,mod,ws,ws/events,ws/connection,cli_entry}.rs`、
`crates/live2d-ai-desktop/src/{tool_adapter,user_event,supervisor,supervisor/handlers}.rs`、
`crates/live2d-ai-desktop/src/platform/{window,capabilities}.rs`、
`crates/live2d-ai-core/src/{state,action/mod}.rs`、
`crates/l2d-wasm-demo/src/main.rs`（+ `index.html`）、
旧前端 `crates/live2d-ai-desktop/src/web_api/{app.js,index.html}`。
前端：`shell/flutter/lib/{main.dart,ui/display_panel.dart,settings/display_prefs.dart,audio/audio_player.dart,api/ws_client.dart,live2d/live2d_bridge.dart,live2d/live2d_stage.dart}`。
文档：`AGENTS.md`、`docs/plans/node-d-py-asset-migration.md`、`docs/research/idle-motion-tracking-and-director-2026-09.md`（相邻调研，仅引用不重复）。
