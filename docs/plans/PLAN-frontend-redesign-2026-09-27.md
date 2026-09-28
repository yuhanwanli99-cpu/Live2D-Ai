# 前端重设计计划：背景 / 外观 / 设置 / 对话（2026-09-27）

> **来源（2026-09-28 入库）**：本文原为旧 worktree `/home/skystar/Live2D-Ai` 的**未跟踪唯一副本**，
> 2026-09-28 判定**并入 0.2.0 线**，作为 **Stage A「A2 重放」的溯源依据**（112 文件 / 29 冲突，
> 提交 `8bc1eaee`）。重放已收口于 `v0.2.0-rc.5`。

> **给其他 worker agent 执行。** 你不会看到提出者的上下文。
> 本轮**解除了此前所有前端设计约束**（旧的 rc.x 口径、用户早年的界面裁决、
> 规格 §1 的 P1–P6 都不再限制新设计）。**唯一红线是：不改后端。**
> 凡是遇到「以前说过不能做」的东西——**它现在可以做了**。
> 唯一不能做的是**物理上做不到**的事（见 §1）和**任何需要改后端的事**（§5 只报告）。

---

## 0. 唯一红线

| 红线 | 含义 |
| --- | --- |
| **不碰后端** | `crates/**` 一行不改；不加/不改 API 端点；不改 WS 帧类型；不改 `live2d-ai.toml` / `.env` 的 schema；不改 Mod 边界 |

没有别的红线。**前端想怎么设计就怎么设计。**

需要后端配合的点**写进 §5 报告**，由提出者决定要不要另开一轮，**worker 不要实现**。

---

## 1. 事实与限制（不是规定，是「物理上做不到 / 会被门禁打红」）

### 1.1 硬性技术事实

| # | 事实 | 后果 |
| --- | --- | --- |
| F1 | 舞台是 `<iframe>` 平台视图，在 DOM 里排在 Flutter 画布**之上**；Flutter 的画布 `pointer-events: none` | ① 压在舞台上的控件必须套 `StagePointerInterceptor`（`lib/live2d/`），否则「看得见、点不着、也滑不动」；② `BackdropFilter` **模糊不到**它（`test/no_backdrop_filter_test.dart` 守命中数为 0）——但**壳自己画的图**可以随便模糊，那是 Flutter 内容 |
| F2 | 舞台必须**保活**：离开 Widget 树 = iframe 销毁 = 模型重载、口型时间轴清零 | 断点切换不能换树形（`stage_keepalive_test.dart`） |
| F3 | Flutter Web 用 CanvasKit，**取不到设备字体** | 中文字体必须自托管在 `assets/fonts/`；**界面文案不得出现子集外字符**（`test/font_subset_test.dart`）；构建必须带 `--no-web-resources-cdn`（否则断网白屏） |
| F4 | localStorage 常见配额 5 MB / 源；dataURL 比原字节大 33% | 背景图必须分层配额 + 超限如实说明 |
| F5 | 浏览器 autoplay 策略作用于**媒体元素** | 首次必须有一次用户手势才出声；音频走 `<audio>` + Blob(WAV)，**不走** Web Audio 增益 |
| F6 | 口型是 30 Hz 高频信号 | 不许经 `ChangeNotifier` 进 Widget 树；走 `GlobalKey → Bridge → postMessage` |
| F7 | `AppPalette` 现在是 `const` 注册表，四套配色写死在 `design/tokens.dart` | 要做自定义配色就得把它从 `const` 表改成**可计算的**（见 W4）——这是本轮最大的一处结构改动 |
| F8 | `POST /api/v1/chat {text}` **已存在** | 「重新生成」「重发」都是纯前端行为，不需要后端 |
| F9 | WS `audio` 帧已带 `sentence_seq`；`text_delta` **不带** | 「正在朗读的那一句」要高亮，**需要后端在正文帧里也带 `sentence_seq`** → §5 报告项 R1 |

### 1.2 门禁（本地必跑 = CI 必跑）

```bash
cd /home/skystar/Live2D-Ai/shell/flutter
export PATH="$HOME/flutter/bin:$PATH"
flutter analyze                        # 0 issue
flutter test                           # 全绿（基线 853，只增不减）
flutter build web --release --base-href /app/ --no-web-resources-cdn
cd /home/skystar/Live2D-Ai && ./scripts/ignite.sh --check
```

F3 那条如果被违反，`ignite.sh --check` 会红（它会扫 `gstatic.com`）。

---

## 2. 设计方向

### 2.1 背景系统

参考 `shalldie/vscode-background`（MIT）的**机制**（多图 / 轮播 / 铺法 / 位置 / 透明度），
**不抄它的多区域**（那要改渲染面 = 改后端）。

**背景源分三类**：

| 类别 | 说明 | 存储成本 |
| --- | --- | --- |
| **纯色** | 纯色底（今天的默认） | 0 |
| **程序化图案** | 渐变（线性/径向）、网格、光斑、细纹——`CustomPainter` 现画 | **0**（不占配额，且任意分辨率不糊） |
| **图片** | 用户导入（dataURL），PNG/JPEG/WebP | 计入配额 |

把「渐变」做成一等公民是这一版的关键决定：它让背景系统**在没有网络、没有存储、
没有性能代价**的前提下就有一个好看的默认值，而不是空着等用户去选图。

**参数**（全部本地偏好）：

- 单图：`fit`（`cover` / `contain` / `stretch` / `tile`）、`align`（3×3 九宫格）、
  `opacity`（0–1）
- 全局：`blur`（0–8 px，只作用在壳自己画的背景上）、`scrim`（遮罩：`auto` / 无 / 轻 / 重）、
  `tileSize`（仅 `tile` 时有效）
- 轮播：`interval`（秒，0=关）、`random`（顺序）、`transition`（无 / 淡入 / 滑动）
- 舞台跟随轮播：**可选开关，默认关**。开的时候每次换图给渲染面**重发一次已有的
  `stage-bg` 消息**（协议不变，只是发得频繁）——代价是 wasm 每次重新解码一张图，
  间隔 ≥10 s 时可接受。

**遮罩 `auto`**：`scrim` 跟随图片不透明度派生，保证聊天文字在任何图上都可读；
同时额外采样图片的平均亮度（`decodeImageFromList` → 降采样 1×1 → 读像素），
很亮就自动加深。**采样只做一次并缓存**（换图才重算），不进每帧。

**图库交互**：缩略图网格（`cacheWidth: 96`），每张可**拖动排序**、移除、设为当前；
顶部「添加图片」+ 内置图案的几枚预览。

### 2.2 外观系统（配色 / 材质 / 风格预设）

**三根轴，互相正交**：

| 轴 | 取值 | 落点 |
| --- | --- | --- |
| **配色** | 黑 / 白 / 蓝 / 灰 四套预设 **+ 自定义** | `AppPalette` |
| **材质** | 圆角幅度、描边强度、界面密度 | `AppMaterial` |
| **风格预设** | 素净 / 纸感 / 沉浸（一键设定上面全部旋钮 + 背景默认值） | 新的 `AppLook` |

**自定义配色怎么算**（替代现在的 `const` 注册表）：

1. 用户只调一个 **accent**（色环取色 + 亮度/饱和度两个滑杆）。
2. 由 accent 的**色相**派生整套中性色：`surface`/`surfaceAlt`/`raised`/`ink`/`muted`
   按固定比例混入 accent（就是 Morrow `themeTint` 的做法），亮度走固定阶梯。
3. 亮/暗由用户的「明暗」决定（跟随预设 or 手动）。
4. `onAccent` 用**二分求解**保证 ≥4.5:1（Morrow 的 `Color.lerp` + 12 次二分，
   `lib/appearance.dart`）。
5. **舞台底不参与色相偏移**（它是渲染面的清屏色，带色相会在模型边缘出色边）。

**保底约束（必须由测试守住，不许靠眼睛）**：

- 正文对每一级面 ≥ 4.5:1；强调色对面 ≥ 3:1；`onAccent` 对 accent ≥ 4.5:1；
  危险色对它所在的面 ≥ 4.5:1
- 三级面感知亮度级差 ≥ 0.03（现有断言已经有，别删）
- 三级面同色相家族

**密度**（三档）：紧凑 / 标准 / 宽松。改的是 `Space` 的乘数与控件最小高度，
**不改**字号阶梯（字号跟着系统 `textScaler` 走，那条无障碍链已经通了）。

### 2.3 设置区布局

```
┌ 分区 chip 行（保留：8 个分区仍是唯一导航）──────────────┐
│ 🔍 搜索字段…                                          │  ← 新增
├──────────────────────────────────────────────────────┤
│ ▢ 组卡片                                              │
│   组标题 · 组说明                                      │
│   ─────────────────────────────────────                │
│   字段行…                                              │
│ ▢ 组卡片                                              │
│ …                                                     │
└──────────────────────────────────────────────────────┘
```

- **组卡片** = 标题 + 一行说明 + 内容；面 `AppPalette.raised` + `appRaisedShadow`。
- **搜索**按字段标签 + 说明文本过滤，命中项高亮、不命中项**折叠而不是移除**
  （避免「搜一下界面结构就变了」）。
- **卡片型选卡**统一成一种形态：**大预览卡**（`128×84` 上下），三个来源：
  1. **配色**：用该配色自己画自己（舞台底 + 三条面带 + 强调色角标 + 名字）
  2. **背景图库**：缩略图网格
  3. **内置图案**：程序化渐变/网格的迷你预览
- **不再用** `SegmentedButton` 做「需要看样子才能选」的选择（配色、图案、铺法）；
  短标签枚举（4K/8K/16K、开关组）**继续用** `SegmentedField`——那个确实好用。

### 2.4 对话

现有结构保留（错误条 → 消息列表 → 流式提示 → 音频条 → 输入区），
以下九项是本轮要做的：

| # | 改动 | 为什么 |
| --- | --- | --- |
| **C1** | 气泡宽度改用 `LayoutBuilder` 的**真实可用宽度**，并加**阅读宽度上限**（≤ 560 px） | 现在 `MediaQuery.width * 0.92` 在三个断点下**都不起作用**（见 §4 证据），是一段「看着在限制、实际没限制」的死约束 |
| **C2** | **按会话各存一份输入草稿**，切会话时存取 | 现在 `selectSession` 不清输入框而 controller 全局只有一个 → **切会话把上句话带过去** |
| **C3** | 空态讲清三件事：一句=一次完整语音 / 首次要点一下才出声 / 说话时嘴会跟着动 | 这是一个 Live2D 皮套，用户第一眼应该知道自己在跟什么玩 |
| **C4** | 输入区：`maxLines` 4→6、Enter 发送 / **Shift+Enter 换行**（要显示出来）、草稿指示点 | 换行现在做不到且没提示 |
| **C5** | 「思考」从气泡**内部**挪到气泡**上方**独立一行，带字数与折叠 | 思考与正文是两件事（AGENTS 明确禁止把思考混进正文），视觉上分开更诚实 |
| **C6** | 会话列表项加**最后一句预览 + 消息数 + 相对时间** | 纯前端（会话数据已在本地） |
| **C7** | 跨天时插**日期分隔线** | 多会话之后必需 |
| **C8** | 消息悬停/聚焦时出**操作条**：复制 / 重新生成（`POST /api/v1/chat` 重发，F8：不需要后端） | 「重新生成」是纯前端能力 |
| **C9** | 长回复排版：段距、代码块/列表的行距与底色跟 `raised` 走 | 现在气泡里只有裸文本样式 |

**候补（本轮不做，写下来免得下一个人重新想）**：
「正在朗读的那一句」高亮（需要 R1）、「口型活跃」指示（受 F6 限制，
必须走 `GlobalKey → Bridge`，不能进 Widget 树）、「重新生成时换 temperature/seed」
（需要后端支持参数）、消息编辑重发（需要后端回滚该轮）。

---

## 3. 数据契约

### 3.1 `DisplayPrefs` 新字段

| 字段 | 类型 | 默认 | 区间 / 取值 | JSON 键 |
| --- | --- | --- | --- | --- |
| `shellImages` | `List<String>` | `const []` | ≤8 张，单张 ≤400 000 字符 | `shellImages` |
| `shellPatterns` | `List<int>` | `const []` | 内置图案 id，≤8 | `shellPatterns` |
| `backgroundOpacity` | `double` | `0.15` | `[0.0, 1.0]` | `backgroundOpacity` |
| `backgroundBlur` | `double` | `0.0` | `[0.0, 8.0]` | `backgroundBlur` |
| `backgroundScrim` | `int` | `0` | `0=auto 1=无 2=轻 3=重` | `backgroundScrim` |
| `imageFit` | `int` | `0` | `0=cover 1=contain 2=stretch 3=tile` | `imageFit` |
| `imageAlign` | `int` | `4` | `0..8`（3×3，4=居中） | `imageAlign` |
| `slideInterval` | `int`（秒） | `0` | `0..3600`，0=关 | `slideInterval` |
| `slideRandom` | `bool` | `false` | — | `slideRandom` |
| `slideTransition` | `int` | `0` | `0=无 1=淡入 2=滑动` | `slideTransition` |
| `stageFollowsSlide` | `bool` | `false` | — | `stageFollowsSlide` |
| `accentColor` | `int?` | `null` | `0xAARRGGBB` | `accentColor` |
| `accentDark` | `bool?` | `null` | `null`=跟随预设 | `accentDark` |
| `radiusScale` | `double` | `1.0` | `[0.4, 1.8]` | 已有 |
| `edgeStrength` | `double` | `1.0` | `[0.4, 1.5]` | 已有 |
| `density` | `int` | `1` | `0=紧凑 1=标准 2=宽松` | `density` |
| `lookPreset` | `int?` | `null` | `null`=不套预设 | `lookPreset` |

**旧字段迁移**（`fromJson`）：
`shellImage`（单张 `String?`）→ `shellImages` 的单元素列表；写回时只写新键。
`kShellImageMaxChars` 删掉，换成 `kBackgroundImageMaxChars`。

**常量**：

```dart
const int kBackgroundTotalMaxChars = 1500000;  // 总预算
const int kBackgroundImageMaxChars  = 400000;  // 单张
const int kBackgroundMaxCount       = 8;       // 张数
```

**总预算**：所有 `shellImages` 字符数之和 ≤ 总预算。加图会超 → **拒绝这一张**
（不截断已有），UI 如实说明。程序化图案不计入。

**`syncShellStageBg` 语义**：`true` 时壳画**舞台那张**，图库保留不渲染；
`false` 时壳画图库当前那张。铺法/位置/透明度/模糊/遮罩/轮播**两种模式都生效**。

### 3.2 运行时状态（**不落盘**）

`currentIndex` / 过渡动画 / 图片平均亮度缓存，都住在新的控制器里，不进
`DisplayPrefs`。理由：刷新后从头开始符合直觉；「读到第几张」不是用户偏好。

### 3.3 会话草稿

`ChatSession` 加一个非序列化的运行时字段？**不**——草稿要跨会话切换存活，
所以放 `ChatController` 里的 `Map<String, String> _drafts`，
**不落盘**（刷新即丢，与「思考不落盘」同一条纪律）。

---

## 4. 现状证据（worker 需要的「为什么」）

| 结论 | 证据 |
| --- | --- |
| 气泡宽度约束在三个断点下都不生效 | `message_bubble.dart:138` `maxWidth: MediaQuery.sizeOf(context).width * 0.92`；面板宽 `NavMetrics.chatWidthExpanded = 340` / `chatWidthMedium = 320`；屏宽 ≥1280 时约束值 ≥1176 ≫ 面板 |
| 切会话串草稿 | `chat_controller.dart:433 selectSession` 不碰输入框；`main.dart:162` 只有**一个** `TextEditingController` |
| 背景只有一张、固定 0.15、写死 cover | `ui/shell_backdrop.dart:25,92`；`display_prefs.dart` 的 `shellImage: String?` |
| 配色是小方块、只露舞台底 | `ui/theme_picker.dart` 的 `_ThemeSwatch`（64×44，画 `shown.stage` + 汉字 + 圆点） |
| 设置区是扁平长列表 | `ui/settings_scaffold.dart`：标题栏 + chip 行 + `Divider` + 滚动区；`SectionHeader` 之间只靠 `Divider` |
| 会话落盘、上限 50 | `chat/chat_session.dart` `ChatSessionStore`（localStorage） |
| 「重新生成」不需要后端 | `api_client.dart:74` `sendChat` → `POST /api/v1/chat` |
| 正文帧没有句子号 | `api/ws_frame.dart`：`audio` 帧读 `sentence_seq`（`:374`），`text_delta` 帧（`:316`）没有 |

---

## 5. 前后端联调报告（**本轮不实施**）

以下都是「想做得更好就需要后端配合」的点。**worker 不要动它们**，
实现与否由提出者另开一轮决定。

| # | 需求 | 需要后端做什么 | 现状 | 优先级 |
| --- | --- | --- | --- | --- |
| **R1** | 对话区**高亮正在朗读的那一句** | `text_delta` / `text_fallback` 帧加**可选**字段 `sentence_seq`（与 `audio` 帧同一个口径；缺省 = 无句子概念） | 前端已有 `audio.sentence_seq`，但正文没有对应编号，无法映射 | 高（这是本产品最独特的一条视觉线索，目前完全没有被用上） |
| **R2** | 舞台背景支持**铺法与位置** | `stage-bg` 消息扩字段 `{dataUrl, fit, align}`（**扩字段，不加帧类型**；缺省 = 现在的 cover/居中） | 铺法写死在 wasm 侧 | 中 |
| **R3** | 知道**舞台图到底有没有解码成功** | 新增一帧 `stage-bg-status {ok, reason}` | 现在只有 HUD 上的 `bg: solid\|image` 被动信号；解码失败**静默**回退纯色，用户只看到「图没出来」，分不清是没设置还是图坏了 | 中（排障成本） |
| **R4** | 界面**不给出渲染面不支持的控件** | `GET /api/v1/capabilities` 加 `supports_stage_background_fit` / `supports_sentence_highlight` | capabilities 是现成的能力发现面 | 中 |
| **R5** | 「重新生成」带**不同的随机性** | `POST /api/v1/chat` 支持可选的 `seed` / `temperature` 覆盖 | 现在只收 `text` | 低 |
| **R6** | 消息**编辑重发** | 一条按 `epoch` 回滚该轮的端点 | 无 | 低 |
| **R7** | 背景/外观**多端同步** | 一个 `GET/PUT /api/v1/display-prefs` | 现在全在本机 localStorage + IndexedDB | 低（本产品是「这台设备长什么样」，同步不是刚需） |
| **R8** | 背景图**以文件形式存进工作区**（2026-09-27 用户口径） | 见下方 §5.1 | 前端已改用 **IndexedDB**（见 §5.2），功能可用；只是字节不进仓库目录 | 中（可移植性，不是可用性） |

**如果只能做一件：R1。** 它是唯一一条「不做就永远做不出来」的——
纯前端拿不到正文与句子的对应关系。

### 5.1 R8：把背景图存成工作区文件（**未实施，等后端一轮**）

用户口径：「背景库的实现不是在工作区文件夹存储相关背景文件吗」。

**为什么这条前端自己做不了**：Flutter Web 没有任何写本地文件的通道。
`dart:io` 在 web 上不存在；唯一剩下的浏览器能力是 File System Access API
（`showDirectoryPicker`），而它①只有 Chromium 系有、②必须由用户手势
逐次授权、③授权不能跨会话静默续期——把它接进来等于「每次开页面都问一次
要不要授权某个文件夹」，比现在难用得多。**所以「存成工作区文件」必然要后端
开一个落盘端点。**

**建议的最小契约**（与本仓 `model_root.rs` 同一套风格：单一根 + 静态服务）：

| 项 | 建议 |
| --- | --- |
| 存储根 | `<cwd>/assets/backgrounds/`（与 `assets/models/` 同一个 `<cwd>` 约定，**不进 git**，加 `.gitignore`） |
| 写入 | `PUT /api/v1/backgrounds/{id}`，body = 原始图片字节（**不是 dataURL**），`Content-Type` 按 `image/*` |
| 读取 | `GET /api/v1/backgrounds/{id}`（或并进现有的静态服务面，走同一个路径前缀） |
| 列举 | `GET /api/v1/backgrounds` → `[{id, bytes, mime}]`（前端启动时用它对账，而不是无脑全下） |
| 删除 | `DELETE /api/v1/backgrounds/{id}` |
| 上限 | 单文件 ≤ 24 MB、总数 ≤ 8 张（与前端 [kBackgroundImageMaxBytes] / [kBackgroundMaxCount] 同口径，**服务端必须自己再判一次**，不能只靠前端） |
| 错误 | 沿用现有 `error` 帧/`{code, message, hint}` 约定；`413` = 超限、`507` = 磁盘满 |

**为什么仍然建议保留 IndexedDB 而不是直接换掉**：
它已经解决了「存不进去」（配额从 5 MB 变成磁盘的 60%），
而 R8 换来的是**可移植**（换机器时图跟着走）。两者不冲突：
按 id 为键，**换实现只动 `BackgroundStore` 这一个接口**（`lib/data/background_store.dart`，
web 那份 176 行、内存那份 60 行），偏好里的清单一个字都不用改。

### 5.2 本轮实际做了什么（2026-09-27，替代方案，不是 R8）

字节搬去 **IndexedDB**，偏好里只留一个 id：

- 旧档（dataURL 写在 localStorage 里）在启动时**自动搬一次**并写回清单；
- 删图时清单与字节**同进同退**；启动时**剪枝**没人引用的记录；
- 打不开 / 配额满时**如实说**，不假装成功。

代价（诚实记录）：图**只在这台机器这个浏览器里**，清站点数据会一起清掉。
这正是 R8 要解决的那一半。

---

## 6. 工作项

```
W1 存储层：DisplayPrefs 新字段 + 迁移 + 纯逻辑 + 单测        ← 无依赖，先做
W2 背景纯逻辑：图案表 / 遮罩 / 轮播索引 / 亮度采样            ← 无依赖，可并行
W3 背景渲染层：ShellBackdrop 重写 + 轮播控制器               ← 依赖 W1 W2
W4 外观：AppPalette 可计算化 + 自定义 accent + 密度 + 预设    ← 无依赖，可并行
W5 设置区：GroupCard + 搜索 + 分区改造                        ← 依赖 W4（要用新面）
W6 选卡：配色大预览卡 / 图案卡 / 图库网格                      ← 依赖 W1 W2 W4
W7 对话 C1–C9 + 路由快照修复 + 隐藏子树断电                   ← 无依赖，随时可并行
W8 联调报告整理 + 文档                                       ← 最后
W0 可打断补间 InterruptibleTween                             ← 无依赖，W3/W4 都要用
```

> **W0 要先做**：§9.3 的 `_from = _displayed` 补间模式。
> 它是 W3（背景轮播过渡）与 W4（风格预设中途切换）共同依赖的底座，
> 晚了做就要把已经铺开的 `AnimatedContainer` 全部返工。

### W0 · 可打断补间（先做）
**文件**：`lib/design/interruptible_tween.dart`（**新**，~60 行）

§9.3 的模式：固定长度的数值记录 + `from = displayed`。落到本项目就是
`class InterruptibleTween<T extends List<double>>`（或等价的 `AnimationController` + record），
配 `MediaQuery.disableAnimationsOf(context)` / `TickerMode` 两个闸门。

**必写测试**：A→B 中途切 C，起始值 = B 的**当前插值**而非 B；
`sameAs` 时不重启；减少动画时立即到终值；dispose 后再调不炸。

### W1 · 存储层
**文件**：`lib/settings/display_prefs.dart`、`test/display_prefs_test.dart`

按 §3.1 加字段 / 区间常量 / `clampXxx` / 序列化 / 迁移 / `copyWith` / `==` / `hashCode` /
`toString`；删 `shellImage` 与 `kShellImageMaxChars` 并修全部调用点
（`app/app_shell.dart`、`app/shell_prefs.dart`、`main.dart`）。

加三个纯函数（可 VM 单测）：
```dart
static bool canAddImage(List<String> current, String dataUrl);  // 单张 + 总预算
static List<int> nextIndex({required int len, required int cur,
                            required bool random});            // 轮播下一个
static double scrimAlphaFor({required double imageOpacity,
                             required double? imageLuma});     // 遮罩推导
```

**必写测试**：默认值 = 今天行为；旧 `shellImage` 迁移；坏元素丢弃不抛；
超张数截断；总预算边界（刚好 / 超 1 字符）；`NaN`/`Infinity` clamp；
每个字段参与 `==`/`hashCode`；JSON 往返全字段相等；
`nextIndex` 顺序回绕 + **random 永不返回当前** + `len<=1` 不转。

### W2 · 背景纯逻辑
**文件**：`lib/background_patterns.dart`（**新**）、`lib/ui/background_scrim.dart`（**新**）

- `BackgroundPattern` 枚举 + 每种一个 `CustomPainter`（线性渐变 / 径向渐变 /
  网格 / 细纹 / 光斑），**零依赖、零存储**。
- `scrimAlphaFor` 如上；`auto` 还要吃一个采样出来的平均亮度。
- 亮度采样：`decodeImageFromList` → `instantiateImageCodec(targetWidth: 1)`
  → 读一个像素。**只做一次并缓存**，换图才重算。

**必写测试**：`scrimAlphaFor` 单调不减、上界 ≤0.6、`0` 透明度时不画遮罩；
`nextIndex` 同 W1；每种图案 painter 在四套主题下不抛、且产出的画面不是纯色
（断言：渲染结果的像素色度 > 阈值——用 `testWidgets` 拿 `toImage()` 的字节）。

### W3 · 背景渲染层
**文件**：`lib/ui/shell_backdrop.dart`（重写）、`lib/ui/shell_slideshow.dart`（**新**）

- `ShellBackdrop` 接：`opacity` / `fit` / `align` / `blur` / `scrim` / 图库 / 图案 / 当前索引。
- 模糊用 `ImageFiltered` + **`RepaintBoundary`**（静态图模糊一次即可，别每帧重算）。
  **`blur` 滑杆不做补间**（§9.4：动高斯半径是实测性能回归），跟手即变或松手才应用。
- 轮播过渡用 **W0 的 `InterruptibleTween`**（§9.3）：轮播换图与用户拖 `opacity`/
  `blur` 滑杆可能同时发生，交叉淡入必须从**屏幕上那个透明度**继续。
- 轮播控制器：`start/stop/dispose` **幂等**；`random` 避开当前；
  `MediaQuery.disableAnimationsOf(context)` 为真时**照常轮播但不做过渡**
  （悄悄不轮播 = 静默失效）。
- `stageFollowsSlide` 开启时，换图给渲染面重发已有的 `stage-bg` 消息
  （**协议不变**）；关闭时一次都不发。
- 换图时 `Image.memory` 的 `key` 带 index，让旧图被回收。
- 没有图**且**没有图案时，`ShellBackdrop` 等价于 `ColoredBox(baseColor, child)`。

**必写测试**：坏 dataURL 仍返回 `null` 不抛；`stageFollowsSlide` 关闭时桥收到的
消息数 = 0（**这条是 F1/§0 红线的守卫**）；控制器幂等；`len<=1` 不进定时器；
减少动画时定时器仍在。

### W4 · 外观：配色可计算化
**文件**：`lib/design/tokens.dart`（**最大的一处结构改动**）、`lib/ui/theme.dart`、
`test/theme_palette_test.dart`

- `AppPalette` 从 `const` 注册表改成**可计算**：`AppPalette.solve({preset, accent, dark})`。
  四套预设保留为「预设 accent + 预设明暗」的输入，行为与今天一致。
- 中性色由 accent 色相派生（混 8–20%），亮度走固定阶梯。
- `onAccent` 二分求解 ≥4.5:1。
- `density` 进 `AppMaterial`，影响 `Space` 乘数与控件最小高度。
- **保底断言一条都不能删**（对比度、三级面级差 ≥0.03、同色相家族、舞台底中性）。

**必写测试**：
`preset(black)` 解析出来的结果与今天写死的那组值**一致**（防回归）；
自定义 accent 在 200 个采样色相上全部满足全部对比度断言；
`density` 三档下 `flutter analyze` + 无溢出（`textScaler` 1.0/1.5/2.0 三档）。

### W5 · 设置区
**文件**：`lib/ui/group_card.dart`（**新**）、`lib/ui/settings_scaffold.dart`、
`lib/settings/sections/*.dart`

- `GroupCard(title, description, child)`，`raised` 面 + `appRaisedShadow`。
- 搜索框：按标签 + 说明过滤；**折叠**不命中项而不是移除。
- 各分区按语义分组，组标题承担原来的 `SectionHeader`（不要重复渲染两遍）。
- **只换外层容器，不动字段清单 / 顺序 / 分区 id 与 label**（a11y 测试依赖）。

### W6 · 选卡
**文件**：`lib/ui/theme_picker.dart`（重写）、`lib/ui/background_gallery.dart`（**新**）

- 配色大预览卡：用该配色自己画自己（舞台底 + 三级面带 + 强调色角标 + 名字）。
- 图案卡：程序化图案的迷你预览。
- 图库网格：缩略图 + 拖动排序 + 移除 + 「添加图片」。
- 语义标签保持 `'${id.label}，${id.hint}'`。
- 不塞图标当主要区分手段。

### W7 · 对话
**文件**：`lib/ui/message_bubble.dart`、`lib/ui/chat_panel.dart`、
`lib/chat/chat_controller.dart`、`lib/chat/chat_session.dart`、`lib/main.dart`、
`lib/ui/session_sheet.dart`

按 §2.4 的 C1–C9 逐项落地。

**必写测试**：
C2 切到有草稿的会话恢复草稿、切到空的清空、再切回还在；
C3 空态含三个要点（**断言要点关键词，不断言整串**）；
C4 `maxLines == 6`、Enter 发送、Shift+Enter 不发送；
C5 思考在气泡**之外**（断言 widget 树的相对位置）、思考仍**不进** `toJson`；
C7 跨天插分隔线（同一天不插）；
C8 重新生成调用 `POST /api/v1/chat` 一次且带原文；
C1 无论选哪种做法都要有断言钉住。

### W8 · 文档
`AGENTS.md` 变更历史（写明「后端一行未改」）；
`docs/design/web-ui-spec-v3.md` 增补本轮的新设计口径（旧的 P1–P6 与
「明确不做」清单里被本轮解除的条目，逐条标注「已于 2026-09-27 解除」及原因）；
`docs/plans/` 留实施记录。

---

## 7. 验收

### 7.1 自动化
`flutter analyze` 0 + `flutter test` 全绿（基线 853，只增不减）。

### 7.2 只有真浏览器能抓到的

- [ ] 四套预设配色 + 自定义 accent（转一圈色环）每一档都可读
- [ ] 背景：纯色 / 图案 / 图片三类源都能切；轮播在走；关掉立刻停
- [ ] 换一张**极亮**的背景 → 聊天文字仍可读（遮罩 auto 生效）
- [ ] 加图加到超总预算 → **如实说明**，且已有图没被挤掉
- [ ] 「舞台跟随轮播」关着时，**桥里一条 `stage-bg` 都不该发**（用 dev 日志看）
- [ ] 窗口 1440 → 700 → 1440：**模型不重载**
- [ ] 切会话：草稿按会话走，来回切换不丢
- [ ] Tab 键走完整个设置面板：焦点环每站可见；搜索过滤后焦点顺序仍合理
- [ ] 打开「减少动画」：轮播仍走但无过渡；开关/选中无过渡

> 项目历史教训：**测试全绿 ≠ 界面是对的**。异步时序、浮层构建时机、
> 平台视图、精确路径匹配这四类问题只有真的在浏览器里点一遍才会露出来。

---

## 8. 风险

| 风险 | 处置 |
| --- | --- |
| W4 把 `const` 表改成计算，**四套预设的观感可能漂** | 测试要求 `preset(black)` 的求解结果与今天写死的值**逐字段一致** |
| 自定义 accent 让对比度在某些色相上不达标 | 求解器内置二分 + 全采样色相的测试；不达标时回退到预设而不是给一个不可读的界面 |
| localStorage 配额 | 三层上限（单张 400 K / 8 张 / 总 1.5 M）；程序化图案**零成本**是缓冲 |
| 背景模糊在 30 Hz 口型热路径上抢帧 | `RepaintBoundary` 包住静态模糊层；模糊只作用在壳自己的图，不碰舞台；`blur` 默认 0；**不动画模糊半径**（§9.4） |
| 大改设置区 → 回归面变大 | W5 的范围纪律 + 既有 `settings_sections_test` 必须继续全绿 |
| 「解除约束」被误读成「可以不管工程质量」 | §1.2 门禁与 §7 验收**不变**；解除的只是审美与功能取舍 |
| **新增持久化字段导致旧版本读不了**（本轮引入了 `accentColor`/`lookPreset`/`slide*` 等） | 我们的 `fromJson` 全部走「缺字段=默认值 + 越界=clamp」，**旧存档与新字段互不干扰**；反向（新版本写、旧版本读）也安全：未知键被忽略、`AppThemeId.fromWire` 未知值回落黑。但要在 W8 的文档里写明：**降级安装不会丢偏好，只是新字段被忽略** |

---

## 9. 从 Morrow 取经的补充（2026-09-27 追加）

对 `/tmp/morrow-src`（AGPL-3.0-only）做了逐文件读码。**只取下面这些**，
其余（7 风格选择器、主题插件运行时、masonry/卡片信息架构）
是 Morrow 的**产品形态**，不是基础设施，不取——省得下一个 worker 再论证一遍。

### 9.1 取：轴与轴之间不许互相污染

Morrow 在 `lib/appearance.dart:474-475` 写死了这条契约：

> Style changes affect edges and depth only.
> Glass opacity, blur, tint and refraction remain independent.

**这就是多轴系统能活下来的承重墙**——5 个正交轴（7 风格 × 3 玻璃 × 3 明暗 ×
4 背景模式 × 1 深度标量）之所以没爆炸，靠的是「风格**只**改边与深度」。

→ **落点**：§2.2 的三根轴（配色 / 材质 / 风格预设）之间也写同一条：
改 `lookPreset` 不许顺手改 accent 色相，改 `radiusScale` 不许顺手改遮罩强度。
写进 `AppLook` 的头注，并用测试钉住（切预设时 `accentColor` 不变）。

### 9.2 取：圆角是「缩放」不是「覆盖」

`appearance.dart:322-327`：`borderRadius(base) = base * cornerRadius / 20 * style.radiusScale`。
显式设成 0 的方角**在任何风格下都还是方角**。

→ **落点**：与我们已经做的 `AppRadius.none` / `AppRadius.pill` 不参与缩放一致，
但要补一条测试：`radiusScale` 取区间两端时，`none == 0`、`pill` 仍 > 任何控件半径 ×1.8。

### 9.3 取：**可打断**的补间（`_from = _displayed`）

Morrow 每个可换外观的控件都走同一套四步（`neumorphic_controls.dart:53-59, 74-90`）：

```dart
_displayed => _controller.value >= 1 ? _to
            : Frame.lerp(_from, _to, curve.transform(_controller.value));

_next = target(...);
if (_to.sameAs(next)) return;   // 无变化不花钱
_from = _displayed;             // ← 从**屏幕上那个值**起算，不是从上一次目标
_to = next;
_controller.forward(from: 0);   // 减少动画 / TickerMode 关 → 停并跳
```

注释原话：*"Reversals interpolate from the actual displayed weights, so an
interrupted A→B→C transition does not jump to B."*

**本轮要落的两处**（都是「外观会在交互中途被改」的场景）：
1. **背景轮播过渡**（W3）：拖滑杆改 `opacity` 的同时轮播正好换图 → 交叉淡入不能跳；
2. **风格预设切换**（W4）：从「素净」拖到「沉浸」的过程中再点「纸感」，
   圆角与描边要从**当前插值位置**继续，不能从「素净」重来。

→ **落点**：新增 `lib/design/interruptible_tween.dart`（~60 行，纯数值，可 VM 单测）：
固定长度的数值记录 + `_from = _displayed`。**不要**用 `AnimatedContainer` /
`Tween(begin: 捕获值)` 的写法继续铺开。
必写测试：A→B 中途切 C，起始值等于 B 的**当前插值**而不是 B 本身；
`sameAs` 时不重启控制器；减少动画时立即到达终值。

### 9.4 取：**不动画高斯半径**

Morrow `neumorphic_controls.dart:402-403`：
`const blur = 2.0` —— *"Keep the Gaussian kernel stable while depth animates"*。
他们量过：动模糊半径是**实测性能回归**。

→ **落点**：W3 的背景 `blur` 滑杆拖动时**不做补间**（跟手即变），
或者只在松手后一次性应用。**不要**写成 `Tween<double>` 逐帧改 `ImageFilter.blur`。

### 9.5 取：浮层之上开页面，**不许用 Material 的 zoom 路由**

Morrow `settings_surface.dart:30-37`：
`allowSnapshotting: false` + **只做不缩放的不透明度过渡**。
理由原话：*"Material's zoom transition snapshots entire glass pages,
delays entrance opacity and adds a forward-only opaque scrim."*

我们的设置面板（medium / compact）正是**压在活舞台上**的浮层，
Material 的缩放路由会给我们：整页快照 + 入场不透明度延迟 + 只进不退的不透明幕布。

→ **落点**：W7 增加一项——设置浮层/整页路由改用自定义 `PageRouteBuilder`
（`allowSnapshotting: false` + 不透明度过渡）。`test/settings_panel_keepalive_test.dart`
与 `test/stage_keepalive_test.dart` 必须继续全绿（**舞台不能因此重载**）。

### 9.6 取：隐藏的子树要**彻底断电**，不是淡出

Morrow `collapsible_panel.dart:70-91` / `settings_page_transition.dart:94-137`：
隐藏的一律 `Offstage` + `IgnorePointer` + `ExcludeFocus` +
`TickerMode(enabled: false)` + `ExcludeSemantics`。

→ **落点**：W5 的分区折叠与设置面板保活照此办理；
另外把它们的记忆占用改成**按压力回收**而不是永远留着
（`didHaveMemoryPressure` 时，已完全关闭且访问过的设置子树丢弃）——
我们现有的保活测试守的是「舞台」，不是「设置内容永不释放」，可以安全调整。

### 9.7 取：交叉淡入的容器**只留一个无障碍节点**

Morrow `little_tips.dart:98-99`：
`Semantics(container: true, label: …)` + `ExcludeSemantics` 包住 `AnimatedSwitcher`，
理由是 *"Native Windows readers may query a child as its outgoing animation is removed."*

→ **落点**：W7 的 C5（思考行折叠）、C7（日期分隔线）、
以及聊天流式播报所在的 `AnimatedSwitcher` 都套这一层。
Windows + NVDA 是我们的目标环境，这条是真机踩过的坑。

### 9.8 取：实时预览的**提交纪律**

Morrow `color_compass.dart:39-42, 99-102` 两条：

- *"An interrupted edit must never become committable again later."*（撤销是**粘性**的）
- *"Commit once after the route closes. Do not rewrite the focused input or publish
  another live preview in the same frame that removes its route."*

→ **落点**：W4 的自定义 accent 有**实时预览**（拖色环即时换主题），
所以：① 离开取色界面 = 取消，且取消后**这一次的中间值不得再被提交**；
② 提交只发生一次，且**不在销毁路由的同一帧**改输入框或再发一次预览。
必写测试：取消后 `DisplayPrefs.accentColor` 保持原值；
连续快速切换色相不会产生两次 `_persist`。

### 9.9 取：浮层永远有「可读面」

Morrow `appearance.dart:604-605`：`readable = dialog || highContrastOf(context)`——
**弹窗与高对比模式下强制给可读面**，不管周围的卡片多透明。

→ **落点**：W2 的遮罩 `auto` 之外，补一条**无条件**规则：
`raised` 之上承载文字的浮层（dialog / sheet / 设置面板）**永不参与背景透明化**。
必写测试：即使 `backgroundOpacity = 1.0`、即使有背景图，
设置面板的 `onSurface` 对它的底 ≥ 4.5:1。

### 9.10 再取三条：外观解析、绘制预算、控件所有权

| # | 取什么 | 落点 |
| --- | --- | --- |
| **9.10.1** | **单一「解析后的外观对象」**（Morrow 的 `Palette` + `AppearanceScope` + `borderRadius(base)`，`appearance.dart:265-327, 709-723`）：任何表面**向这个对象要圆角/面/边**，谁都不许自己写死一个角 | W4 把 `AppPalette` 改成可计算之后，**顺手建 `ResolvedAppearance`**：主题构造期算一次并注册成 `InheritedWidget`。`AppRadius.x` 的散点引用（现在 100+ 处）**不改**，但新增的表面一律走它。头注写明「新增表面从 `ResolvedAppearance` 取，不许再写 `BorderRadius.circular(AppRadius.md)`」 |
| **9.10.2** | **绘制预算是 6 px 外扩裁剪**，不是 padding（Morrow `surface_paint_boundary.dart:3,6`：*"A paint budget, not padding"*） | W7：聊天面板的气泡浮雕/阴影会渗到相邻气泡的间隙里。加一个 `PaintBudgetClip`（`ClipRect` + `Clip.hardEdge`，外扩 6 px）。**零布局代价、不动焦点/语义/子树身份**——密集列表正好需要 |
| **9.10.3** | **只覆写主题字段，绝不重新实现控件**（`neumorphic_controls.dart:607-608`：*"Changes only visual theme fields. State, callbacks, padding, text and accessibility behavior remain owned by each standard Material control."*） | 写进 W4/W6 的头注与验收：新外观一律走 `inputDecorationTheme` / `sliderTheme` / `backgroundBuilder` / `ShapeBorder` / `SliderComponentShape`，**不许**自己写一个 `Switch`/`Slider`/`Checkbox`。这是「7 种风格能活下来」的根本原因，对我们同样成立 |

### 9.11 不取（并说明理由，省得下一个人再论证）

| 不取 | 理由 |
| --- | --- |
| 7 种 `VisualStyle` 选择器 | 是**产品功能**不是基础设施；4 套配色 × 7 风格 = 28 种组合的回归面。它们的 `radiusScale` 机制我们已经取到了（§9.2） |
| 组件级材质 + `followComponent` 跟随链 | 数据驱动的按 key 覆盖很聪明，但我们的界面没有那么多需要单独配的槽位；跟随链还要三层防循环（UI 预走 + 载入校验 + 持久化校验），不值 |
| 主题插件运行时（JSON 描述 / 分块 artwork / sha256 校验） | 那是**一整个插件宿主**。我们把配色做成用户可选的 accent 就够了 |
| **shader uniform 槽位 / 逐像素折射 / `liquid_glass.frag`** | 玻璃折射本来就因为平台视图而不可用（§1.1 F1）；即便可用，30 Hz 口型热路径上叠逐像素 shader 不划算 |
| **全局零滚动条**（在基类覆写 `buildScrollbar`） | 他们是无限画布，我们有真实的滚动内容；聊天区到底要不要滚动条是个可以单独决定的事，不跟这轮一起做 |
| masonry / 卡片信息架构 | 笔记本形态，不是舞台形态。P1「舞台是主角」优先 |
| Neumorphism 双向浮雕 | 在纯黑舞台上双向阴影读不出来；且它给控件加了 `saveLayer` 风险（他们自己在 `neumorphic_controls.dart:235-236` 注明「不做额外 child / 透明层 / saveLayer」） |

**取证的出处**（AGPL-3.0-only，© 2026 StarrySky7D4 and contributors）：
只做了**阅读**分析，未复制任何代码。已有的取用记录见 `CREDITS.md` §6.5。
