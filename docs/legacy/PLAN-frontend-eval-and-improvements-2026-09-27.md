# 前端评估与改进计划：背景系统 / 设置区 / 对话（2026-09-27）

> 🗄 **历史（归档，未采用）**：本文于 2026-09-28 从旧 worktree `/home/skystar/Live2D-Ai`
> 的**未跟踪唯一副本**移入 `docs/legacy/`；它属于 09-27 那次被用户叫停的前端会话，
> **未进入 0.2.0 线**。保留只为追溯「当时提过什么」。
>
> ⚠️ **本文件已被取代**（同日）：提出者随后解除了所有前端设计约束，
> 唯一红线只剩「不动后端」。现行计划见
> [`PLAN-frontend-redesign-2026-09-27.md`](./PLAN-frontend-redesign-2026-09-27.md)。
> 本文保留作为**取证与思路来源**（§2 的现状评估、§4 的证据表仍然有效）。

> **这份计划是给其他 worker agent 执行的。** 你不会看到提出者的上下文，
> 所以凡是「为什么」「不许做什么」「怎么算做完」都写在正文里。
> 遇到本文没写到的取舍：**先看 §1 红线，再按 §3 数据契约的形状推断**，
> 不要自己发明新字段。

---

## 0. 项目与代码位置

| 事实 | 值 |
| --- | --- |
| 仓库根 | `/home/skystar/Live2D-Ai`（WSL2，唯一开发环境） |
| 前端 | `shell/flutter/`（Flutter Web，Dart） |
| 设计令牌 | `shell/flutter/lib/design/`（`tokens.dart` 是四套配色的唯一真源） |
| 组件层 | `shell/flutter/lib/ui/`（`field_row.dart` = 8 个分区所有字段行） |
| 设置分区 | `shell/flutter/lib/settings/sections/` |
| 会话/消息模型 | `shell/flutter/lib/chat/` |
| 端口 | 服务在 `127.0.0.1:18080`，前端在 `http://127.0.0.1:18080/app/` |
| 点火 | `cd /home/skystar/Live2D-Ai && ./scripts/ignite.sh`（**另开终端**） |

---

## 1. 硬约束与红线（违反了就是做错，不是风格问题）

### 1.1 不动后端 —— 本轮**一行 Rust 都不改**

用户已明确：**不需要修改后端**。

| 禁止 | 说明 |
| --- | --- |
| 改 `crates/**` 任何文件 | 包括 wasm 渲染面 `l2d-wasm-demo` |
| 新增 / 修改 API 端点 | `web_api/**` 一行不改 |
| 新增 / 改 WS 帧类型 | `l2d-wasm-demo` 的 v1 协议不动 |
| 改 `live2d-ai.toml` / `.env` 的 schema | 配置面不动 |
| 改 `crates/live2d-ai-mod-*` | Mod 边界不动 |

**含义**：所有新能力必须**只靠已有通道**实现——
本机 `localStorage`（`DisplayPrefs` / `ChatSessionStore`）+
已有的渲染面消息（`stage-bg` 等，协议不变、只是**不发新的**）。

§7 有一份「**如果将来要后端配合**」的建议清单，**只记录，本轮不实施**。

### 1.2 前端门禁（本地必跑 = CI 必跑）

```bash
cd /home/skystar/Live2D-Ai/shell/flutter
export PATH="$HOME/flutter/bin:$PATH"
flutter analyze                       # 必须 0 issue
flutter test                          # 必须全绿（当前基线 853 通过）
flutter build web --release --base-href /app/ --no-web-resources-cdn
cd /home/skystar/Live2D-Ai && ./scripts/ignite.sh --check   # 四项全 ok
```

`--no-web-resources-cdn` **不是可选项**：缺省构建让 CanvasKit 指向
`fonts.gstatic.com` / `gstatic.com/flutter-canvaskit`，断网即白屏。
中文字体自托管在 `assets/fonts/`，**界面文案不得出现子集外的字符**
（`test/font_subset_test.dart` 守着）。

### 1.3 已有的用户裁决（改动与之冲突 = 需要先问人）

| 裁决 | 出处 | 对本计划的约束 |
| --- | --- | --- |
| 舞台是主角，UI 都是客人（P1） | `docs/design/web-ui-spec-v3.md` §1 | 新增常驻 UI 必须回答「吃掉多少舞台面积」 |
| 一个品牌色，其余全派生（P2） | 同上 | 配色只认 `AppPalette.accent`；不引入第二个强调色 |
| 静默失效是最危险的失败模式（P4） | 同上 | 加字段必须同步 `copyWith`/`lerp`/`toValuesMap`/相等性，并有结构枚举测试 |
| 高频信号不进 Widget 树（P6） | 同上 | 口型 30Hz 继续走 `GlobalKey → Bridge → postMessage` |
| 刻意不做功能堆砌 | `AGENTS.md` v0.1.0-rc.1 §13.2② | 任何新能力先问「这是不是堆砌」；堆砌一律走 Mod 方向 |
| 尽量少用图片，用文字做按钮 | v0.5.0 用户裁决 | 选卡里不要塞图标当主要区分手段 |
| 发送 = 圆形上箭头 | 同上 | 不要改成带「发送」文字的按钮 |
| 不用 `BackdropFilter` 毛玻璃 | 规格 §12-7 + `test/no_backdrop_filter_test.dart` | 舞台是 `<iframe>` 平台视图，模糊不到它；命中数必须为 0 |
| 源码 ≤500 行（豁免 ≤1000 需**头注**写理由） | `AGENTS.md` | 新文件要小；超 500 行必须在头注说明为什么不能拆 |

### 1.4 已完成、不要重做的前置工作（2026-09-27 同日）

- `AppPalette` 四套配色已换成**有色相**的中性色，并新增第三级面 `raised`
  （`design/tokens.dart`）；三级面**感知亮度级差 ≥ 0.03**、同色相家族，由
  `test/theme_palette_test.dart` 钉死。
- 新增 `AppMaterial`（圆角幅度 / 描边强度）两个材质旋钮 + `DisplayPrefs`
  的 `radiusScale`/`edgeStrength`。
- 新增 `appRaisedShadow(AppPalette)`（只给**我们自己构建**的浮层用）。
- 计划书：`docs/plans/PLAN-visual-substance-2026-09-27.md`（含改动前后的量化对照）。

---

## 2. 现状评估

### 2.1 背景图

**参考**：`shalldie/vscode-background`（MIT）。它的配置面是
「**多区域** × **多张图** × `interval` 轮播 / `random` 随机 ×
`size` / `position` / `opacity`」，每个区域（编辑器 / 侧边栏 / 面板 / 全屏）
各自一套。

**我们的现状（已读代码确认）**：

| 事实 | 位置 |
| --- | --- |
| 壳背景**只有一张**图 | `DisplayPrefs.shellImage: String?`（`lib/settings/display_prefs.dart`） |
| 透明度**固定 0.15**，明文「不做 opacity 滑条」 | `lib/ui/shell_backdrop.dart:25` `kShellBackdropOpacity` |
| 铺法写死 `BoxFit.cover`、位置写死居中 | `shell_backdrop.dart:92` |
| 无轮播、无随机、无多图 | — |
| 存储预算：单图 1.5M 字符（≈1.1 MB 二进制），超了**不写盘但本次会话生效** | `kStageImageMaxChars` / `kShellImageMaxChars` |
| 舞台背景走**渲染面** `stage-bg`（wasm 在 framebuffer 里画） | `DisplayPrefs.stageImage` |

**结论**：我们抄它的**机制**（多图 / 轮播 / 铺法 / 位置），
**不抄「多区域」**（分区背景要改渲染面 = 改后端，见 §1.1）。
舞台那张图**保持现状**（本轮只动壳侧），见 §5「明确不做」。

### 2.2 设置区

**现状（已读代码确认）**：

- 布局：`SettingsScaffold` = 标题栏 + **一行 chip 行**（分区导航）+ `Divider` + 内容滚动区。
  内容是一条**扁平长列表**：`SectionHeader` + 一串 `SliderField`/`ToggleField`/…
- 卡片型选卡**只有一处**：`ThemePickerField`（`lib/ui/theme_picker.dart`）——
  4 个 `64×44` 的圆角矩形，每个画那套配色的**舞台底** + 一个汉字 + 选中圆点。
- 其余枚举走 `SegmentedField`（`SegmentedButton`），例：渲染档位 4K/8K/16K。
- Mod 列表走 `AdminRow` / `_ModConfigTile`（一行一开关 / 一张配置卡）。

**问题**：
1. 长列表没有「组」的概念——「外观」「舞台与口型」「互动」三组之间只靠
   `Divider` + `SectionHeader` 区分，字段一多就变成一堵表单墙。
2. 配色选卡信息量太低：只暴露「舞台底 + 名字」，而**决定观感的三级面和强调色
   一个都看不到**（用户点之前无法判断）。
3. 字段说明文案密度不均：有的两句话压在一个字段上（「壳背景」那段），
   有的没有。

### 2.3 对话

**现状（已读代码确认）**：

- `ChatPanel` 结构：头（仅设置/会话入口）→ 错误条 → 消息列表（`reverse: true`）
  → `StreamingIndicator` → 常驻 `AudioBar` → 输入区。
- 消息模型 `ChatMessage`：`role` / `text` / `reasoning`（思考，**不落盘**）/
  `failed`（未收尾兜底，**落盘**）/ `epoch`。
- 多会话：`ChatSessionStore` 落 localStorage，上限 **50 个会话**；
  与视图层 `kChatHistoryLimit = 500` **取同一个值**。
- 气泡宽度约束：`message_bubble.dart:138`
  `maxWidth: MediaQuery.sizeOf(context).width * 0.92`。
- 面板宽度：expanded = **340 px**（`NavMetrics.chatWidthExpanded`），
  medium = 320 px；compact 下聊天是整页。
- 空态：图标 + 「说点什么吧」+「回复会一边生成一边上屏，语音按句完整播放。」

**问题（逐条都验证过，不是猜）**：

| # | 问题 | 证据 | 影响 |
| --- | --- | --- | --- |
| C1 | **气泡宽度约束在三个断点下都不起作用** | 约束用**屏幕**宽 ×0.92：expanded 屏宽 ≥1280 → 1177 px ≫ 面板 340；medium 900–1280 → 828 ≫ 320；compact 虽然面板≈屏宽但那时它恰好生效 | 一段**无效约束**：读起来像「限制过宽」，实际从没限制过。改了也不会有可见变化——这正是 P4「静默失效」的形态 |
| C2 | **切换会话会把上一个会话的半句话带过去** | `ChatController.selectSession`（`chat_controller.dart:433`）只做 `_abandonTurn + sessions.select + _persist`，**不清输入框**；输入框 `TextEditingController _input` 由 app 根持有（`main.dart:162`），全局只有一个 | 切会话后草稿串味。要么清空，要么**按会话各存一份草稿**（后者更符合 IM 习惯） |
| C3 | 空态没说明**最关键的产品约定** | 空态只说「一边生成一边上屏」 | 「**一句 = 一次完整语音，不会在句中卡顿**」是本项目最重要的手感约定（`AGENTS.md` 语音输出约定），用户第一次用时最该知道；且首次要点一下才出声（浏览器 autoplay 限制），空态也没提 |
| C4 | 输入区没有「换行」提示 | `minLines: 1, maxLines: 4` + `TextInputAction.send` + `onSubmitted` | 用户不知道能不能换行；也没说清 Enter 的语义 |
| C5 | 思考区没有「耗时/规模」感 | 思考折叠区存在且不落盘 | 一轮长思考时除了转圈没有别的信号。**低成本**方案：折叠区标题带一句「思考中 / 思考了 N 字」，N 可由 `reasoning.length` 得出（不落盘，刷新即消失，与现状一致） |

---

## 3. 数据契约（**照抄，不要改字段名**）

以下字段全部落在 `DisplayPrefs`（`lib/settings/display_prefs.dart`）。
规则与现有字段完全一致：const 默认值 + 区间常量 + `clampXxx` + 序列化往返 + 相等性。

| 字段 | 类型 | 默认 | 区间 | JSON 键 | 备注 |
| --- | --- | --- | --- | --- | --- |
| `shellImages` | `List<String>` | `const []` | ≤ `kBackgroundMaxCount` 张，每张 ≤ `kBackgroundImageMaxChars` | `shellImages` | **取代** `shellImage`（见下方迁移） |
| `shellImageOpacity` | `double` | `0.15` | `[0.05, 0.40]` | `shellImageOpacity` | **默认值 = 今天写死的 0.15**，所以不改这一项时观感逐像素不变 |
| `shellImageFit` | `int` | `0` | `0..2` | `shellImageFit` | `0=cover 1=contain 2=stretch`，另存枚举名，别存字符串 |
| `shellImageAlign` | `int` | `4` | `0..8` | `shellImageAlign` | 3×3 九宫格索引，**0=左上 … 4=居中 … 8=右下** |
| `shellSlideInterval` | `int`（秒） | `0` | `0..3600` | `shellSlideInterval` | `0` = 不轮播 |
| `shellSlideRandom` | `bool` | `false` | — | `shellSlideRandom` | |

**新增常量**（同文件）：

```dart
/// 背景图**总**预算（字符）。单张超限只是丢一张图，整份偏好写不进去
/// 会连带丢掉主题、音量、口型设置——代价远大于「一张图没记住」。
const int kBackgroundTotalMaxChars = 1500000;   // = 今天的 kStageImageMaxChars
const int kBackgroundImageMaxChars = 400000;   // 单张上限
const int kBackgroundMaxCount = 8;             // 最多几张
```

**迁移规则（`fromJson`）**：

1. 读 `shellImages`（`List<dynamic>` 里的 `String`）。
2. **没有** `shellImages` 但**有**旧的 `shellImage`（`String?`）→ 迁移成单元素列表。
3. 逐张过 `_readImage(raw, kBackgroundImageMaxChars)`；非字符串/空串/超限一律**丢弃**。
4. 数量超 `kBackgroundMaxCount` → 截断到前 N 张（不做提示：这是历史存档，不是用户操作）。
5. 迁移后 `toJson()` **只写** `shellImages`，不再写 `shellImage`。
6. `DisplayPrefs.shellImage` 字段与 `kShellImageMaxChars` 别名**删除**，
   所有调用点改为 `shellImages`（当前调用点：`lib/app/app_shell.dart`
   的 `shellImage:` 形参、`lib/app/shell_prefs.dart` 的选图/清图、`lib/main.dart`）。

**总预算怎么用**：`shellImages` 全部字符数之和 ≤ `kBackgroundTotalMaxChars`。
加图时若会超总预算 → **拒绝这一张**（不截断已有列表），UI 如实说明
（沿用现有「本次不写盘」的诚实文案，例句：
「再加一张就超过本机存储上限（1465 KB），没加上」）。

**`syncShellStageBg` 的新语义**（必须改清楚，rc.5 的一条真相原则不能破）：

- `true`（默认）：壳画的是**舞台那张图**（单张），`shellImages` 保留在盘上但**不参与渲染**。
- `false`：壳画 `shellImages[currentIndex]`。
- **铺法 / 位置 / 透明度 / 轮播 / 随机**在两种模式下**都生效**——它们是渲染参数，不是图片来源。

**不落盘的运行时状态**：`currentIndex` **不进 `DisplayPrefs`**
（刷新后从头开始，这符合直觉，也避免一个「读到哪张」的持久化字段）。
它住在一个新的纯逻辑控制器里（见 W2）。

---

## 4. 工作项（可并行，注意依赖）

```
W1 存储层（DisplayPrefs + 纯逻辑 + 单测）        ← 无依赖，先做
W2 渲染层（ShellBackdrop + 遮罩 + 轮播控制器）    ← 依赖 W1
W3 对话：气泡宽度 + 会话草稿 + 空态 + 输入提示     ← 无依赖，可与 W1 并行
W4 分组卡片组件（GroupCard，新文件）              ← 无依赖，可并行
W5 配色选卡重设计（theme_picker 重写）             ← 无依赖，可并行
W6 各分区改用 GroupCard                          ← 依赖 W4
W7 外观分区接上新的背景字段                        ← 依赖 W1 + W2 + W4
W8 文档（AGENTS / 发布说明）                      ← 依赖全部
```

### W1 · 存储层

**文件**：`lib/settings/display_prefs.dart`、`test/display_prefs_test.dart`

1. 按 §3 加字段 / 常量 / `clampXxx` / `toJson` / `fromJson` 迁移 / `copyWith` / `==` / `hashCode` / `toString`。
2. 删 `shellImage` 与 `kShellImageMaxChars`，修所有调用点。
3. 加一个纯逻辑方法：
   ```dart
   /// 这张图能不能加进来（单张上限 + 总预算）。**纯函数，可 VM 单测。**
   static bool canAddImage(List<String> current, String dataUrl);
   ```
4. **测试**（`test/display_prefs_test.dart`，沿用文件里既有的 group 风格）：
   - 默认值 = 今天的行为（`shellImages` 空、`opacity` 0.15、fit 0、align 4、interval 0、random false）
   - **旧存档 `{shellImage: "data:..."}` 迁移成单元素列表**；写回后不再有 `shellImage` 键
   - 坏元素（数字/空串/超单张上限）被丢弃且**不抛**
   - 数量超上限被截断到 8
   - `canAddImage`：刚好到总预算为 true、超 1 字符为 false
   - 越界 / `NaN` / `Infinity` 一律 clamp（`opacity` 夹到 0.05–0.40，**不许归零**）
   - 每个新字段参与 `==` / `hashCode`
   - JSON 往返（`toJson → fromJson` 全字段相等）

**门禁**：`flutter analyze` 0 + `flutter test` 全绿。

---

### W2 · 渲染层

**文件**：`lib/ui/shell_backdrop.dart`（改）、`lib/ui/shell_slideshow.dart`（**新**）

**W2-a `ShellBackdrop` 扩展**

| 参数 | 默认 | 说明 |
| --- | --- | --- |
| `opacity` | `0.15` | 由 `DisplayPrefs.shellImageOpacity` 传入 |
| `fit` | `BoxFit.cover` | |
| `alignment` | `Alignment.center` | |
| `scrim` | 派生 | 见下 |

**可读性遮罩（新）**：图越亮/越不透明，越需要压一层。
纯函数，先在 VM 单测里钉住：

```dart
/// 遮罩强度：由图片不透明度推出一个**永远盖得住文字**的下限。
/// 0 透明度 → 不画遮罩（与今天逐像素一致）。
double scrimAlphaFor(double imageOpacity);
```

实现取向：**竖直渐变**（上 0 / 中 max / 下 0 的两层，或单层线性），
用 `palette.stage` 压（暗主题是黑、亮主题是白——与 `AppColors.glassScrim`
已有的「遮罩与背景反向」纪律一致）。**不要**写死黑色。

**W2-b 轮播控制器（新文件，纯逻辑 + 一个 timer）**

```dart
/// 轮播：纯算的部分与打表分开，好测。
class ShellSlideshow {
  int get index;
  int get length;
  int next({required bool random});   // 纯函数：下一个索引（random 时避开当前）
  void start(Duration interval, {required bool random});  // 内部 Timer.periodic
  void stop();
  void dispose();
}
```

- **`random` 时必须避开当前那张**（否则会出现「连着两轮同一张」的观感故障）。
- **减少动画**（`MediaQuery.disableAnimationsOf(context)`）时：**照常轮播，但不做过渡**。
  不允许「因为开了减少动画就悄悄不轮播」——那是 P4 的静默失效。
- 切图后 `Image.memory` 的 `key` 要带上 index，让旧图被回收
  （8 张 400 KB dataURL 同时驻留 ≈ 3 MB，能接受，但不该无限缓存）。
- 换图时**不发** `stage-bg`（本轮舞台不动，见 §5）。

**测试**：`test/shell_backdrop_test.dart`（已有文件，扩）+ 新建 `test/shell_slideshow_test.dart`

- `decodeDataUrlBytes` 既有行为不回退（坏 dataURL 仍返回 `null`，不抛）
- `scrimAlphaFor(0) == 0`；`scrimAlphaFor` 单调不减；上界 ≤ 0.6
  （**不许**把图压到看不见）
- `next()` 顺序轮播回绕正确；`random` 永不返回当前 index；`length <= 1` 时不转
- `start/stop/dispose` 幂等（连调两次不炸）
- 没有图时 `ShellBackdrop` **等价于** `ColoredBox(baseColor, child)`（与今天一致）

**门禁**：同 W1。

---

### W3 · 对话（本轮只做 UI/交互，零后端）

| # | 改动 | 文件 | 说明 |
| --- | --- | --- | --- |
| C1 | 气泡宽度约束改成**基于真实可用宽度** | `lib/ui/message_bubble.dart` | 用 `LayoutBuilder` 拿气泡容器的 `maxWidth`，再乘 0.92；**或者**干脆删掉这个约束并在头注写明「面板本身就是约束」。**二选一，但必须留一条注释说明为什么**（P4） |
| C2 | **按会话各存一份草稿** | `lib/chat/chat_controller.dart` + `lib/main.dart` | 切会话时：旧会话草稿存下、新会话草稿取回。`TextEditingController` 仍由 app 根持有，切换时**显式 `text = ...`**，不要在控制器里藏第二个 controller |
| C3 | 空态补两句 | `lib/ui/chat_panel.dart` `_ChatEmptyState` | ①「一句 = 一次完整语音，不会在句中卡顿」；②首次要点一下画面才出声（浏览器 autoplay 限制）。**不要**加「试试这些 prompts」——那是功能堆砌 |
| C4 | 输入区补换行提示 | 同上 | Enter 发送 / Shift+Enter 换行，写成输入框下方一行 `labelSmall`；`maxLines` 从 4 提到 6（更实用的上限） |
| C5 | 思考折叠区标题带规模 | `lib/ui/message_bubble.dart` | 折叠标题从「思考」改成「思考 · N 字」（`N = reasoning.length`）。**不落盘**（与现状一致），刷新即消失 |

**测试**（`test/message_bubble_test.dart` / `test/chat_session_test.dart` / 新增断言）：

- C2：切到有草稿的会话 → 输入框恢复；切到没草稿的会话 → 清空；
  再切回来 → 草稿还在
- C3：空态文本包含「完整播放」与「点一下」两个要点（**断言文案要点，不是断言整串**，
  否则改个错别字就红）
- C4：`maxLines == 6`；Enter 仍触发 `onSend`；Shift+Enter **不**触发
- C5：思考折叠标题含字数；`reasoning` 仍**不进** `toJson`
- C1：无论选哪种做法，都要有断言钉住（选了删约束就断言气泡在窄容器里不溢出）

**门禁**：同 W1。

---

### W4 · 分组卡片组件（新文件）

**文件**：`lib/ui/group_card.dart`（**新**，≤200 行，头注写清职责）

```dart
/// 设置分区里的**组卡片**：标题 + 一行说明 + 内容。
/// 面 = AppPalette.raised，抬起 = appRaisedShadow。
class GroupCard extends StatelessWidget {
  const GroupCard({required this.title, this.description, this.icon, required this.child, ...});
}
```

- 圆角 `AppRadius.lg`，`hairline` 描边，`appRaisedShadow`。
- `title` 用 `titleSmall`，`description` 用 `bodySmall` + `contentMuted`。
- **内边距只用 `Space.*`**，不许写字面量（`test/design_tokens_lint_test.dart` 扫）。
- 卡片之间 `Space.s3`；卡片内字段之间 `Space.s2`。

**测试**：新建 `test/group_card_test.dart`

- 四套主题下渲染都不溢出（含 `textScaler` 1.0 / 1.5 / 2.0 三档）
- `description == null` 时不占位
- 语义树里标题是可读文本（不是纯装饰）

---

### W5 · 配色选卡重设计

**文件**：`lib/ui/theme_picker.dart`（重写）、`test/theme_picker_test.dart`（改）

每张卡（宽 ~120、高 ~78）**用那套配色自己画自己**：

```
┌──────────────┐   ← 底 = shown.stage
│ ▓▓▓▓▓▓▓▓▓▓▓ │   ← 三条面带：surface / surfaceAlt / raised（从上到下更亮）
│ ▓▓▓▓▓▓▓▓▓▓▓ │   ← 右上角一个 accent 小方块
│      黑      │   ← 名字（shown.ink）
└──────────────┘
```

- 选中态：**2 px 不透明 `accent` 描边 + 一个实心圆点**（沿用今天的约定，
  颜色在这里已被承载语义，不能同时表示「选中」）。
- 宽屏 4 张一行；窄屏（compact 侧板约 320 px）`Wrap` 换行成 2×2。
- **语义标签逐字保持** `'${id.label}，${id.hint}'`——现有测试与读屏都依赖它。
- 不加图标（用户裁决「尽量少用图片」）。

**测试**：

- 四套各自用**自己**的 `AppPalette` 渲染（断言卡片底色 = `AppPalette.of(id).stage`）
- 选中态描边宽度 2、不透明
- 320 px 宽下 2×2 不溢出
- 语义标签与今天逐字相同

---

### W6 · 各分区改用 GroupCard

**文件**：`lib/settings/sections/*.dart`、`lib/ui/settings_scaffold.dart`

**范围纪律**：**只换外层容器，不动字段清单、不动字段顺序、不动分区 id/label/description**
（分区元数据被 `SettingsSection` 与 a11y 测试依赖）。
`SectionHeader` 在改用 `GroupCard` 后**由卡片标题承担**，不要再重复渲染一遍。

| 分区 | 卡片划分 |
| --- | --- |
| `AppearanceSection` | 外观 / 舞台与口型 / 互动 |
| `LlmSection` | 连接 / 生成参数 / 人设（若已有分组就照搬） |
| `TtsSection` | 同上原则 |
| `ModelsSection` | 模型库 / 导入 |
| `PersonaSection` | 按现有结构 |
| `ModsSection` | Mod 列表 / 插件与服务 |
| `DiagnosticsSection` | 状态 / 日志 / 快照 |

**测试**：`test/settings_sections_test.dart` 等既有文件**必须继续全绿**
（这本身就是「没有改坏」的证据）；另加一条：每个分区**至少一张** GroupCard。

---

### W7 · 外观分区接上背景新字段

**文件**：`lib/settings/sections/appearance_section.dart`、`lib/app/shell_prefs.dart`、`lib/app/app_shell.dart`、`lib/main.dart`

「背景」卡片（**取代**现在那两段长说明的「舞台背景图 / 壳背景」）：

1. **图库**：一个可增删的缩略图网格（每张缩略图一个「移除」按钮）。
   缩略图本身用 `Image.memory(bytes, cacheWidth: 96)`，**不要**用原图尺寸渲染。
2. **铺法**：`SegmentedField`（`铺满 / 完整 / 拉伸`）。
3. **位置**：3×3 九宫格按钮组（九个小方块，中心那个是当前值）——
   **不要**九宫格 + 下拉两层。
4. **透明度**：`SliderField`，两端标 `淡` / `浓`。
5. **轮播**：`ToggleField` + 间隔滑杆（两端 `慢` / `快`）+ `ToggleField 随机顺序`。
6. 「与舞台同步」开关**保留**（语义见 §3），并把文案改成新语义。

**诚实文案要求**（沿用现有纪律）：超单张上限 / 超总预算 / 浏览器读文件失败，
三种都要有**各自的**文案，且失败用警告色不是静默。

**测试**：`test/appearance_section_test.dart` + `test/display_prefs_test.dart`

- 每根滑杆读 `DisplayPrefs` 的区间常量（不写字面量——已有同款断言，扩名单）
- 超出总预算的加图被拒绝且 UI 显示原因
- 缩略图网格：0 张时显示空态 + 「添加图片」按钮
- 「与舞台同步」开着时，壳渲染的是**舞台那张**（不是 `shellImages`）

---

### W8 · 文档

- `AGENTS.md` 变更历史加一条（日期 2026-09-27，内容：背景系统 v2 + 设置区分组卡片
  + 对话 5 项修正；**明确写出「后端一行未改」**）。
- 发布说明：写到 `docs/releases/v0.2.0-rc.2.md`（**若你无权切版本线，
  就先只写 `docs/plans/` 里的实施记录，并在 AGENTS 里注明「未切版本线」**）。
- `docs/design/web-ui-spec-v3.md` 增补：§12「明确不做」里补上
  「分区背景（要改渲染面）」「在线拉图」「视频/GIF 背景」三条及其理由。

---

## 5. 明确不做（附理由，避免下一个 worker 又做一遍）

| # | 不做 | 理由 |
| --- | --- | --- |
| 1 | **分区背景**（舞台一块、侧栏一块、面板一块各自配） | 参考插件的核心卖点，但实现要改渲染面 = 改后端（§1.1） |
| 2 | **舞台跟着轮播** | 每换一张要重发 `stage-bg`，wasm 里重新解码一张图；且属于协议侧改动面 |
| 3 | **在线拉图 / URL 背景** | 断网红线（本地优先、离线可用）。`AGENTS.md` 明确禁止依赖 CDN |
| 4 | **视频 / GIF 背景** | 解码常驻 CPU/GPU；本项目舞台是 30 Hz 口型驱动的热路径 |
| 5 | **背景模糊 / 毛玻璃** | `BackdropFilter` 模糊不到 `<iframe>` 平台视图；`test/no_backdrop_filter_test.dart` 守命中数为 0 |
| 6 | **把渲染档位 / 所有枚举都改成卡片** | 4K/8K/16K 三个短标签，`SegmentedButton` 更好用；卡片化会让「一眼扫到」变成「逐张读」。**只重做「需要看样子才能选」的选卡**（配色、背景图库） |
| 7 | **消息落盘的思考内容** | 思考比正文长 5–20 倍，落盘会让会话存档膨胀一个数量级（现状已刻意不落盘） |
| 8 | **输入框里的「试试这些」推荐语** | 功能堆砌（§1.3） |

---

## 6. 验收

### 6.1 自动化（每个工作项自己跑）

`flutter analyze` 0 + `flutter test` 全绿（基线 853，只增不减）。

### 6.2 只有真浏览器能抓到的（本项目被这四类问题咬过）

在 `http://127.0.0.1:18080/app/` 手工过一遍：

- [ ] 加 3 张背景图 → 刷新 → **还在**（localStorage 往返）
- [ ] 轮播开着 → 看着它换 → 关掉 → **立刻停**（不是等下一个周期）
- [ ] 换一张**极亮**的背景图 → 聊天文字仍可读（遮罩有效）
- [ ] 把图加到超总预算 → UI **如实说明**，且**已有图没被挤掉**
- [ ] 窗口从 1440 拖到 700 再拖回来 → **模型不重载**（`StagePointerInterceptor`
  / `stage_keepalive_test` 守的保活，回归重点）
- [ ] 切会话 → 输入框按草稿走 → 再切回来 → 草稿还在
- [ ] Tab 键走一遍设置面板：焦点环每一站都看得见
- [ ] 打开「减少动画」→ 轮播仍走但无过渡；开关/选中不再有过渡

### 6.3 不许只看测试

> 项目历史教训（`AGENTS.md` v0.5.1）：**测试全绿 ≠ 界面是对的**。
> 异步时序、浮层构建时机、平台视图、精确路径匹配这四类问题
> 只有真的在浏览器里点一遍才会露出来。

---

## 7. 前后端接口建议（**只记录，本轮不实施**）

如果将来要让舞台背景 / 分区背景成立，需要后端配合。按「最小改动」排序：

| # | 建议 | 现状 | 为什么值得做 |
| --- | --- | --- | --- |
| I1 | `stage-bg` 消息扩成 `{dataUrl, fit, align}`（**扩字段，不加帧类型**） | 现在只有 dataUrl，铺法写死 `cover` 居中 | 现在是「一次能力一次字段」，第三次要改时就会变成协议考古；`missing field = 默认值` 已经是不破坏兼容的约定 |
| I2 | 加一帧 `stage-bg-status {ok: bool, reason: string}` | 现在只有 HUD 上的 `bg: solid\|image` 被动信号 | Flutter 侧需要知道 **wasm 解码成功没有**。现在解码失败会静默回退到纯色底，用户只看到「图没出来」却不知道是图坏了还是没设置 |
| I3 | `GET /api/v1/capabilities` 加 `supports_stage_background_fit: bool` | capabilities 是现成的能力发现面 | 渲染面没实现 fit/align 时，界面应当**隐藏**对应控件，而不是让用户调一个不生效的旋钮（这正是 P4） |
| I4 | 背景图走 `PUT /api/v1/display-prefs` 而非 localStorage | 现在全在本机 | 只在「用户想在多台机器间同步外观」时才有必要。**现在不要做**——本项目偏好明确是「这台设备看起来是什么样」 |

**这四条都不属于本轮范围。** 写在这里是为了：将来有人提「让舞台也用新背景」时，
不用重新推一遍。

---

## 8. 风险

| 风险 | 处置 |
| --- | --- |
| localStorage 配额（5 MB/源，dataURL 比原字节大 33%） | §3 的三层上限（单张 400 K / 8 张 / 总 1.5 M）；**已有**的「超限不写盘但本次会话生效 + 如实说明」沿用 |
| 轮播与舞台**不同步**（壳换图、舞台不变） | **本轮明确接受的取舍**。开关「与舞台同步」开着时壳本来就是画舞台那张，语义自洽 |
| 8 张图同时在内存 | ≈3 MB，可接受；靠 `Image.memory` 的 `key` 随 index 变化让旧图被回收 |
| 设置区大改 → 回归面变大 | §4 W6 的范围纪律（只换外层容器，不动字段清单与分区元数据）+ 既有 `settings_sections_test` 必须继续全绿 |
| 有人把 `AppPalette` 的新色值「清理」回中性灰 | 已有断言（色度 ≥ 3/255、同色相家族、级差 ≥ 0.03）会红 |
