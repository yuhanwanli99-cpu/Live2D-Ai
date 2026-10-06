# 背景透传：相对 `shalldie/vscode-background` v3.1.0 的偏离说明

> **任务**：0.2.0 封口路线 **Stage B 的 B4**（把「不采纳」写成明文，docs-only）
> ＋ **R6-c 回填**（2026-09-28，docs-only：§5.2 补 DEC-3 结论、§6 加「实施状态」列、
> §7 逐条落裁决、同步 `docs/README.md` 索引）。
> **参考真源**：[shalldie/vscode-background @ `eef5ddb651501ef5f7d386e06e684f75cfc1a8aa`（v3.1.0, MIT）](https://github.com/shalldie/vscode-background/tree/eef5ddb651501ef5f7d386e06e684f75cfc1a8aa)，
> 本地浅克隆在 `/tmp/vscode-bg`（HEAD 已核对 = 上述 commit；**只读**）。
> **我方真源**：工作树 `/home/skystar/Live2D-Ai-fe` @ `feat/frontend-redesign`，
> `shell/flutter/lib/settings/display_prefs.dart`、`shell/flutter/lib/ui/shell_backdrop.dart`、
> `shell/flutter/lib/design/background_item.dart`、`shell/flutter/lib/app/shell_prefs.dart`。
> **范围真源**：`docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md` §5（§5.1 参考模型 / §5.2 差距表 / §5.3 短期目标）、
> `docs/plans/IMPL-PROMPTS-0.2.0-seal-2026-09-27.md` §B1–B4
> ——**这两份（及本文下文引用的 `ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md`）已于 2026-10-06
> 的 E8 文档减量中移出工作树**，正文引用**按原样保留**（不改历史），取回见
> [已移出工作树的文档索引](../REMOVED-docs-index-2026-10-06.md)。
> **改动史**：B4 时**只新增文档**（不改代码、不改任何既有文件，`docs/README.md` 索引原留给 Stage C · C1）；
> **2026-09-28 R6-c** 按任务回填 §0 / §5.2 / §5.3 / §6 / §7 并同步 `docs/README.md` 索引（§7.2 第 3 条关闭）。
> **两波都未改任何代码。**
>
> 行号说明：参考侧行号对应 `eef5ddb` 的原文，可复核；我方行号是**写作时的工作树快照**——
> 同一工作树上 **R6-a / R6-b** 正在改 `display_prefs.dart` / `shell_backdrop.dart` /
> `background_item.dart` / `appearance_section.dart`，落盘后行号会漂移，文件与符号名不会。
>
> **R6-c 的实跑基线**：HEAD `31d06cb4`（`feat/frontend-redesign`）；测量前后
> `git hash-object` 三个被测文件（`display_prefs.dart` / `shell_backdrop.dart` / `display_prefs_test.dart`）
> 逐一致 ⇒ 测量时工作树未漂移。证据口径见 §6 末。

---

## 0. 一句话结论

- **机制层采纳**：把背景当成「**分区 × 有序图列表 × 一组渲染参数**」来配置（这是参考的模型，不是它的实现）。
- **实现层不采纳**：参考靠**改写 VS Code 安装目录里的 `workbench.html`**（注入内联 `<script>`、必要时 `sudo` 覆盖只读文件、用 CSS 屏蔽 integrity 警告）来实现。
  我们不是编辑器扩展，不碰宿主安装目录（§4 D0）。
- **能力层明确不采纳 6 条**：在线 https 图 / 本地文件夹 / `~` 与环境变量展开 / 任意 CSS `style` /
  `editor` 的 `useFront` / 跨渲染面的多区域（舞台分区）。每条在 §4 给出「参考怎么做 → 为什么不做 → 红线依据」。
- **多区域分两层**：**壳内子区域技术可做，但已裁决本轮【不做】**（DEC-3：无用户诉求 +
  避免堆砌 +「舞台是主角」；记 Stage C backlog，§5.2）；
  **舞台分区 = 改渲染面 = 后端口径**（wasm + 版本化协议），不在前端做（§5）。
- **离线优先是硬依据**：`AGENTS.md`「前端层（Flutter）」的两条离线约束 + `scripts/ignite.sh --check`
  对产物里 `gstatic.com/flutter-canvaskit` 的探测。**注意其字面覆盖范围**（只扫 CanvasKit，不扫任意 URL），
  见 §4 D1 与 §8。

---

## 1. 参考真源与核实方式

- 本地克隆 HEAD：`eef5ddb651501ef5f7d386e06e684f75cfc1a8aa`（`release: v3.1.0`），`package.json:4` 版本 `3.1.0`。
- 逐条「参考怎么做」的出处：`package.json`（contributes 的**唯一真源**）、`README.zh-CN.md`、
  `package.nls.zh.json`（配置项描述文案）、`src/background/**`（实际怎么改 HTML）、`src/utils/patchTargets.ts`（改哪个文件）。
- 核实方式：**通读上述文件并摘原文行号**。**没有安装、没有运行**这个扩展 →
  一切「运行时会发生什么」都是源码推断，不当作实测（§8）。

### 1.1 参考的机制（一段话）

扩展激活后（`package.json:312-314` `onStartupFinished`）把配置编译成一段 JS，然后：

1. 定位 VS Code 的 `workbench.html`（`src/utils/patchTargets.ts:30-40`：桌面 `electron-browser`/`electron-sandbox`，Web/code-server 走 `browser`）；
2. 先给 CSP 的 `script-src` 补 `'unsafe-inline'`，再把 `<script>…</script>` 内联到 `</html>` 之前
   （`src/background/PatchFile/PatchFile.html.ts:5-32`）；
3. 写文件失败（安装目录只读）时提示「Retry with Admin/Sudo」，用 `@vscode/sudo-prompt` 提权 `mv`
   （`src/background/PatchFile/PatchFile.base.ts:72-116`）；
4. 附带用一个 CSS 规则把多语言的「installation appears to be corrupt」integrity 提示**藏起来**
   （`src/background/PatchGenerator/PatchGenerator.checksums.ts:8-57`，注释自称 "fix checksums with css. LOL"）；
5. 这段 JS 在**每次 VS Code 前端加载时**运行，往 `document.head` 插 `<style>`、按区域选择器铺背景。

**这就是「参考机制、不抄实现」的分界线**：1–4 是宿主适配（VS Code 专属、有副作用成本），
5 里的**区域选择器模型**才是值得借鉴的机制。

---

## 2. 参考特性模型（逐条落到 `package.json` 行）

| 能力 | 出处（`package.json`） | 默认 / 细节 |
|---|---|---|
| 全局开关 `background.enabled` | `:86-90` | boolean，默认 `true` |
| `background.editor` | `:91-145` | 见下 |
| ├ `useFront` | `:106-110` | boolean，默认 `true`：图片在代码**上方**还是下方 |
| ├ `style`（任意 CSS 对象） | `:111-119` | 默认 `{background-position:"100% 100%", background-size:"auto", opacity:0.6}` |
| ├ `styles[]`（**逐图**样式） | `:120-124` | 对象数组，按索引与 `images[]` 对齐 |
| ├ `images[]` | `:125-132` | 字符串数组 |
| ├ `interval` | `:133-137` | 秒，默认 `0` = 不轮播 |
| └ `random` | `:138-142` | boolean，默认 `false` |
| `background.fullscreen` | `:146-200` | `images[]` / `opacity`(默认 .1，**min 0 max 0.6**) / `size`(默认 `cover`) / `position`(默认 `center`) / `styles[]` / `interval` / `random` |
| `background.sidebar` | `:201-236` | 全部字段 `$ref` 到 fullscreen |
| `background.auxiliarybar` | `:237-272` | 同上 |
| `background.panel` | `:273-308` | 同上 |

**`images[]` 支持哪些形态**（`README.zh-CN.md:79-110` 与 `package.nls.zh.json:13` 的示例 + 实现）：

| 形态 | 示例（README 原文） | 实现落点 |
|---|---|---|
| 在线图 | `"https://hostname/online.jpg"` | `PatchGenerator.base.ts:62`：前缀 `http` / `data:` 直接放行 |
| data URL | `"data:image/*;base64,<base64-data>"` | 同上 |
| 本地文件 | `file:///`、`/home/…`、`C:/…`、`D:\\…` | `base.ts:68-70` → `normalizeImageUrls` |

| 形态（续） | 示例 | 实现落点 |
|---|---|---|
| `~` / 环境变量 | `"~/Pictures/img.png"`、`"${HOME}/Pictures/img.png"` | `base.ts:107-123` `expandPathVariables` |
| **文件夹** | `"/home/xie/images"` | `base.ts:133-146` `fast-glob` 递归抓 10 种扩展名 |

**区域选择器**（§5 会用到）：`fullscreen` = `body::after`（`PatchGenerator.fullscreen.ts:16`）、
`sidebar` = `.split-view-view > .part.sidebar::after`（`…sidebar.ts:6`）、
`auxiliarybar` = `.split-view-view > .part.auxiliarybar::after`（`…auxiliarybar.ts:6`）、
`panel` = `.split-view-view > .part.panel::after`（`…panel.ts:6`）；
`editor` 走 Monaco 的 `::after`/`::before`（`…editor.ts:42-87`），`useFront` 决定用哪个伪元素。

---

## 3. 总表：采纳 / 收窄 / 不采纳

| 参考能力 | 结论 | 我们这边 |
|---|---|---|
| 分区独立配置的**模型** | **采纳（机制）** | 壳 / 舞台两级来源 `backgroundSource`；壳内子区域见 §5 |
| `images[]` 有序列表 | **采纳** | `backgrounds[]`（判别联合：图片 / 内置图案） |
| `opacity` | **采纳（区间不同）** | `backgroundOpacity` 0–1，默认 1.0（参考 fullscreen 0–0.6，默认 0.1，越界**回 0.1**） |
| `size`（CSS `background-size`） | **收窄 → 短期扩档** | 现状仅 `cover/contain`；短期目标 `cover/contain/stretch/tile`（§6） |
| `position`（任意 CSS） | **收窄（有意）** | 9 宫格 `imageAlign`（比任意 CSS 更可点选；规划 §5.2 保留） |
| `style` + `styles[]` | **部分采纳** | 任意 CSS 不做（D4）；逐图 `opacity/fit/align` 覆盖为短期目标（B3） |
| `interval` / `random` | **采纳（区间收窄）** | `slideInterval`（0 或 5–300 秒）/ `slideRandom` |
| `background.enabled` | **短期目标** | 现状无全局开关；B3 排期 |
| 在线 https 图 | **不采纳** | D1 |
| 本地文件夹 | **不采纳** | D2 |
| `~` / 环境变量展开 | **不采纳** | D3 |
| 任意 CSS `style` | **不采纳** | D4 |
| `editor.useFront` | **不采纳** | D5 |
| 跨渲染面的多区域（舞台分区） | **不采纳** | D6 / §5 |
| 改写宿主 `workbench.html` + sudo + 屏蔽 integrity 提示 | **不采纳（实现层）** | D0 |

---

## 4. 逐条偏离（参考怎么做 → 我们为什么不做 → 红线依据）

### D0 · 机制层：改写宿主安装目录里的 `workbench.html`

**参考怎么做**：定位 `workbench.html`（`src/utils/patchTargets.ts:30-40`），给 CSP 补 `'unsafe-inline'`
后内联 `<script>`（`PatchFile.html.ts:5-32`）；写不动就用 `sudo` 提权覆盖
（`PatchFile.base.ts:72-116`，含 `@vscode/sudo-prompt` 依赖，`package.json:37`）；
再用一条 CSS 把 integrity 的「安装似乎损坏」提示 `display:none`
（`PatchGenerator.checksums.ts:8-57`，含 14 种语言文案）。

**我们为什么不做**：我们是**自带服务 + 自带前端**的应用（Rust `--web` + Flutter `/app/`），
没有「宿主的安装目录」可改，也不需要修改任何第三方二进制；引入「改安装目录 / 提权 /
屏蔽完整性警告」这三件事的成本与风险（杀软、自动更新覆盖、用户信任）远超收益。

**红线依据**：`AGENTS.md`「工作区与点火纪律」与「前端层」——
唯一入口是 `GET / → 302 → /app/`，前端产物由仓库自身构建并与服务同源托管；
`AGENTS.md:13`「**不做复杂上层**（实现保持最小）」。

**我们怎么做**：所有渲染都在我们自己的进程 / 页面内完成——壳背景是 Flutter 自己画的
（`shell_backdrop.dart:1-22`），舞台背景走渲染面版本化协议 `stage-bg`。

---

### D1 · 在线 `https` 图

**参考怎么做**：`images[]` 里任何以 `http` 开头的串**原样透传**
（`PatchGenerator.base.ts:60-64`：`if (['http','data:'].some(prefix => img.startsWith(prefix))) return [img];`），
最终变成 CSS `background-image: url(https://…)`；README 示例写明
`"https://hostname/online.jpg"`（`README.zh-CN.md:91-93`），`package.nls.zh.json:13` 的说明是
「在线图片，只允许 `https` 协议」。
（**实现与文案不一致**：代码判据是 `startsWith('http')`，`http://` 也放行。见 §8。）

**我们为什么不做**：

1. **离线优先**——远端图 = 运行时对外部 HTTP(S) 的硬依赖，断网/被墙即「用户选了图，界面空白」，
   而背景图恰恰是「本地优先桌宠」最不该联网的那类资产；
2. **我们没有「URL 即内容」的概念**——背景图是**用户手势选文件 → dataURL → 本地字节库**：
   背景库字节进 IndexedDB（`app/shell_prefs.dart:139-150`、`design/background_item.dart:86-113`），
   舞台那张走 localStorage + WS `stage-bg` 帧（同上）；
3. 一旦接受远端 URL，就要连带设计超时 / 重定向 / 混合内容 / SSRF / 缓存失效 / 隐私（每开一次界面就对外发一次请求）
   —— 那是参考作为「编辑器装饰扩展」可以不做的事，我们是产品。

**红线依据**：`AGENTS.md:370-376`（构建必须 `--no-web-resources-cdn`，产物**不得**依赖 Google CDN：
缺省构建把 CanvasKit 指向 `https://www.gstatic.com/flutter-canvaskit/<rev>/`，「**断网即白屏**」）
+ `AGENTS.md:377-389`（中文字体自托管，缺字去 `fonts.gstatic.com` → 断网豆腐块）
+ `scripts/ignite.sh:88-101`（`--check` 对 `/app/index.html` 与 `/app/main.dart.js`
**探测 `gstatic.com/flutter-canvaskit`**，命中即 FAIL「断网会白屏」）。
**边界要说清**：这条探针**只扫 CanvasKit**，它不会（也无法）扫出任意 https 图片 URL；
「在线图违反离线红线」是**同一条原则的直接推论**，不是这条探针的字面结论。
事实层面的佐证：`shell/flutter/lib` 里没有任何网络图加载路径——唯一的 `Image.network` 出现在注释里
（`api/models_api.dart:42`，在解释「为什么不用它」）。

**替代**：本地文件导入（单张 ≤ 24 MB，`display_prefs.dart:28` `kBackgroundImageMaxBytes`；
库 ≤ 8 项，`:15` `kBackgroundMaxCount`）+ 程序化内置图案（零存储、零网络，`background_item.dart:12-14`）。

---

### D2 · 本地文件夹（递归导入）

**参考怎么做**：`images[]` 里不以扩展名结尾的串被当成**文件夹**，
`fast-glob` 按 `folder/**/*.@(svg|png|jpg|jpeg|gif|bmp|webp|mp4|otf|ttf)` 递归抓文件
（`PatchGenerator.base.ts:133-146`），激活 / 重载时重新展开成一个静态列表。

**我们为什么不做**：

1. **浏览器里没有「路径」**——Flutter Web 拿不到用户本地路径，唯一入湖口是文件选择器手势
   （`app/shell_prefs.dart:156-157` `pickImageDataUrl()`）。「文件夹」不是一个我们收得到的输入；
   `showDirectoryPicker` 是 Chromium 专属且仍需授权，引入它等于为一条边缘功能增加平台分支（**未核实**，§8）；
2. **语义不成立**：参考是「每次进编辑器时**重新 glob**」→ 文件夹新增图片自动出现；
   我们没有那个「重算时刻」——只能做成一次性快照，那就**不是文件夹支持**，只是「一次选很多张」；
3. **预算模型冲突**：库有 8 项 / 单张 24 MB 的上限（`display_prefs.dart:15,28`）。
   对一个文件夹做无界批量导入，必然撞上限——而按参考的语义，撞限的项会**静默消失**，
   这正是本项目 P4 明令禁止的「静默失效」。

**红线依据**：离线优先（同 D1）+ `AGENTS.md:13`「不做复杂上层（实现保持最小）」+
规划 P4「禁止静默失效」（`PLAN-0.2.0-seal-and-cleanup-2026-09-27.md` §0/§4 的既有纪律）。

**替代**：一次导入一张，`canAddBackground`（`display_prefs.dart:693-700`）逐项判上限，
满了**如实告诉用户是哪一关拦的**（`shell_prefs.dart:152-155` 的「两条结果都要如实说」）。

---

### D3 · `~` 与环境变量展开（`${HOME}`、`$ENV`）

**参考怎么做**：`expandPathVariables`（`PatchGenerator.base.ts:107-123`）在**扩展宿主进程**里
把 `~/` 换成 `homedir()`、把 `${ENV}` 与 `$ENV` 换成 `process.env[name]`。

**我们为什么不做**：

1. **前提不存在**：这是**进程文件系统路径**的处理。Flutter Web 既没有 `homedir`，也读不到环境变量；
   我们从来就不接受「路径」这个输入（同 D2 第 1 条）；
2. 硬做只能是「在 Dart 里假装解析一段路径字符串」，然后仍然没有进程 FS 去打开它——
   一层只为兼容而存在的死代码，且会让用户以为「写了 `~/Pictures` 就能用」→ **误导性 UI**。

**红线依据**：`AGENTS.md:193-194`（前端治理红线：前端只做展示与调度）；
`AGENTS.md:13` 最小实现。

**替代**：没有路径输入框，因而**不存在**需要展开的东西。

---

### D4 · 任意 CSS `style`（以及 `styles[]` 里的任意属性）

**参考怎么做**：`style` 可以是任意 CSS 声明对象（`package.json:111-119`），
经 `stylis` 编译（`PatchGenerator.base.ts:156-158`）后拼进 CSS 规则。
参考自己也知道这是把双刃剑：`serializeStyle` **始终剔除** `pointer-events` 与 `z-index`
（`base.ts:169-176`，注释：避免用户样式破坏覆盖层的点击穿透与层级）。

**我们为什么不做**：

1. **我们没有 CSS**——壳是 Flutter widget 树。要做「任意 CSS」= 在 Flutter 里实现一个 CSS 子集
   解释器，这是**重造一个平台**而不是做功能；
2. **参考的实现本身就证明了任意 CSS 需要护栏**——它必须硬编码排除 `pointer-events`/`z-index`
   才敢放行其余属性。我们若做，护栏清单只会更长，且 Flutter 的对应物（布局、命中测试、层叠）
   与 CSS 不同构，**没有可移植的映射**；
3. 真正被用户使用的诉求是「**这一张图**我想换个铺法 / 位置 / 透明度」，那是**有类型**的参数字段，
   不需要任意字符串。

**红线依据**：`AGENTS.md:13`「不做复杂上层（实现保持最小）」；
`AGENTS.md:390-391`（前端与渲染面只走**版本化消息**，新增字段必须向后兼容——
任意 CSS 字符串是「不可校验的载荷」，与这条协议纪律相反）。

**替代**：短期目标 = **逐图样式覆盖**（`perItemStyle`：每张图可单独 `opacity/fit/align`，回落全局），
UI 放在背景库的单项编辑里——这正是 `styles[]` 的**语义**，但没有任意 CSS 的风险
（Stage B · B3，`IMPL-PROMPTS-0.2.0-seal-2026-09-27.md` §B3）。

---

### D5 · `editor` 的 `useFront`（图在代码上方）

**参考怎么做**：`useFront=true` 时用 `.…::after` 且 `z-index: 99`、
并**必须**同时设 `pointer-events: none`（否则整个编辑器点不动）；`false` 时用 `::before`
（`PatchGenerator.editor.ts:42-87`：`z-index: ${useFront ? 99 : 'initial'}`、
`pointer-events: ${useFront ? 'none' : 'initial'}`）。

**我们为什么不做**：

1. **我们本来就没有「代码」这个前景层**：壳的层级是**有意**自下而上
   底色 → 图案 → 图片 → 可读性遮罩 → `child`（`shell_backdrop.dart:79-92`）。
   「把图压到内容之上」只有两种结果：压住文字与控件（可用性崩），
   或加一层 `IgnorePointer`（= 参考的 `pointer-events:none`），而后者在 Flutter 里
   又是一套新层级语义（浮层、设置面板、toast 与它的先后关系都要重排）；
2. **可读性是我们的硬约束**：现在保证可读的旋钮是遮罩 + `uiTransparency`
   （`display_prefs.dart:193-217`、`shell_backdrop.dart:149-174`）。图跑到前景会直接绕开这套护栏；
3. 规划 §5.2 已裁决短期**不做**，只列为「可选项评估」
   （`PLAN-0.2.0-seal-and-cleanup-2026-09-27.md:172`：「压住面板会影响可读性」）。

**红线依据**：`AGENTS.md:13` 最小实现；项目 P4「禁止静默失效 / 可读性下限」
（同类纪律见 `display_prefs.dart:362-367`：面板不透明度**有下界**，不许归零）。

**替代**：背景**恒在内容之下**。要「更显眼的图」就调 `backgroundOpacity` / `backgroundBlur` / `backgroundScrim`。

---

### D6 · 跨渲染面的多区域（把舞台再分区）

**参考怎么做**：五个区域各自独立配置（`package.json:91-308`），
每个区域只是一条 CSS 选择器上的一个伪元素（§2 末尾）；`fullscreen` 甚至就是 `body::after`
盖住整个窗口（`PatchGenerator.fullscreen.ts:16`）。它之所以能「一个区域 = 一行选择器」，
是因为**整个 VS Code 界面是同一个 DOM**。

**我们为什么不做（舞台分区）**：我们的界面**不是同一个渲染面**——它是两套：

| 渲染面 | 我们怎么画 | 分区要动什么 |
|---|---|---|
| **壳**（AppBar / 聊天 / 侧栏 / 设置） | Flutter widget 树，`ShellBackdrop` 在壳根铺一层 | **纯 Flutter，可做**（§5） |
| **舞台**（Live2D） | `<iframe>` 平台视图 + wasm canvas，背景走 `stage-bg` 消息在 **framebuffer** 里画 | **改渲染面 = 改后端** |

舞台背景不是在 DOM 上贴 CSS，而是在 wasm 渲染面里：背景预通道 `LoadOp::Clear(<stageColor>)` +
全屏三角形贴纹理，模型通道 `LoadOp::Load` 叠上去（`AGENTS.md` 的 rc.5 修复三记录；
`shell_backdrop.dart:1-22` 明确「舞台走渲染面协议 `stage-bg`，两者管道完全不同，不要合成一条路」）。
在那上面再切「舞台的侧区 / 下区 / 全屏」= 要在渲染面里按区域再做合成与布局，
即**改 wasm + 扩版本化协议** ⇒ **后端口径**，不是前端能单方面完成的事；
而且它会把「舞台背景」和「模型渲染」的职责搅在一起，违反 §4 D4 同款的「版本化消息」纪律。

**红线依据**：`AGENTS.md:392-397`（舞台是 iframe 平台视图、画布 `pointer-events:none`、
指针进 iframe 文档——跨面交互必须显式垫层，说明两套渲染面是真边界）；
`AGENTS.md:390-391`（前端 ↔ 渲染面只走版本化消息，新增字段向后兼容）；
`AGENTS.md:398-409` 同类纪律：**前端不得复制核心逻辑**（状态机 / 仲裁 / 协议）。

**替代**：保留「壳 / 舞台」两级**来源**（`backgroundSource`，`display_prefs.dart:141-157,344-355`）：
它回答的是「**用哪张图**」，不是「**在哪个矩形里画**」——这是我们在没有第二个渲染面的前提下
能忠实表达的部分。

---

## 5. 多区域专节：边界、可做项、为什么没做

**结论一句话**：**壳内子区域可做（纯 Flutter）；舞台分区 = 改渲染面 = 后端口径。**

### 5.1 舞台分区（不做）

见 §4 D6。技术判据三条，缺一不可：

1. 舞台背景**不在 DOM/CSS 里**，在 wasm framebuffer 里（`stage-bg` 协议）；
2. 分区 = 渲染面按区域重新合成 ⇒ **改 wasm 代码 + 扩协议**，属后端；
3. 跨面交互已有既有教训（`StagePointerInterceptor`，`AGENTS.md:392-397`），
   说明「看起来是一个界面」的两套渲染面，任何跨越都是显式工程，不是配置。

### 5.2 壳内子区域（**裁决：本轮不做** · DEC-3）

**裁决（2026-09-28，按 `PLAN-0.2.0-seal-and-cleanup-2026-09-27.md` §6 Stage B · B.0 的建议默认）：不做。**
不再写「可做，未排期」——那个悬空状态已终结（§7.2 第 2 条问的就是它）。

**技术边界仍然成立**（备查）：壳是**一个** Flutter widget 树，所以
「聊天 / 侧栏 / 设置面板各自一张背景」在原理上成本可控：把 `ShellBackdrop` 从「壳根一层」
推广成「按子区域各铺一层」即可（现结构：`shell_backdrop.dart:79-107` 单层 + `Stack`）。
**能做 ≠ 该做。**

**不做的理由（三条，逐条独立）**：

1. **无用户诉求**：现有反馈里没有任何一条要求「聊天区一张、侧栏另一张」；用户能感知的一直是
   「我的背景图有没有显示、好不好看」。
2. **避免堆砌**：规划 §5.3 第 3 条的要求是「至少壳的『聊天/侧栏』与『设置面板』可分辨
   （**若判定为堆砌，写明不做**）」（`PLAN-0.2.0-seal-and-cleanup-2026-09-27.md:181`）。
   为一个没有诉求的能力增加分区配置项，正是「堆砌」的定义——本轮**判定为堆砌**，因此写明不做。
3. **「舞台是主角」**：产品主面是 Live2D 舞台，壳背景只是衬托；把注意力预算花在「壳里再切三块」上，
   与「舞台是主角」的产品口径相反。

**记 Stage C backlog**：若将来有明确诉求，恢复路径是「`ShellBackdrop` 推广成按子区域各铺一层」；
评估时**必须**先回答「谁来维护多出来的分区配置项」，并同步 §5.3 的对照表。

**本文件的口径**：技术边界保留在此备查；**产品裁决已完成（不做）**，全文不再出现「未排期」这种悬空表述。

### 5.3 与参考的对照

| 参考区域 | 我们的对应物 | 能否做 |
|---|---|---|
| `background.fullscreen`（`body::after`，整窗） | 壳根 `ShellBackdrop`（已有） | 已有 |
| `background.editor` | **无对应**（我们没有代码编辑器） | 不做 |
| `background.sidebar` | 壳的侧栏子区域 | 纯 Flutter，可做；**裁决不做**（DEC-3，§5.2） |
| `background.panel` / `.auxiliarybar` | 壳的设置面板 / 聊天面板 | 纯 Flutter，可做；**裁决不做**（DEC-3，§5.2） |
| 舞台内部再分区 | wasm 渲染面 | **不做（后端口径）** |

---

## 6. 部分采纳 / 短期目标（**短期目标 ≠ 现状；没跑过测试就不得写「已经支持」**）

> **状态口径（2026-09-28 R6-c）**：「短期目标」列是**目标**，不是现状；
> 「实施状态」列才是判断。**凡是写「已支持 / 已成现状」的，必须附 R6-c 亲自跑过的测试名**——
> 拿不到测试佐证的一律写「**裁决：做（实施中，待 R6-d 以测试佐证回填）**」。

| 参考能力 | 现状（写作时快照） | 短期目标（Stage B） | 实施状态（2026-09-28 R6-c） |
|---|---|---|---|
| `size`（CSS `background-size`） | 仅 **cover / contain**：`maxImageFit = 1`（`display_prefs.dart:322`）、`boxFitFor` 只有两条分支（`shell_backdrop.dart:67-70`）。`fitName` 与 int 注释里**已存在** `stretch/tile` 两个名字（`:390-395`、`:181`）但 clamp / UI / 实现都到不了 | **扩到 `cover/contain/stretch/tile`（+ `tileSize`）**，删掉或写明 `maxImageFit=1` 这个假上限的理由（旧 B1 → 现 **R6-a**） | **裁决：做（实施中，待 R6-d 以测试佐证回填）**——R6-a 改 `display_prefs.dart` / `shell_backdrop.dart` |
| `styles[]` 逐图样式 | 无 | **逐图 `opacity/fit/align` 覆盖**，回落全局（旧 B3 → 现 **R6-a** 加字段 / **R6-b** 做编辑器） | **裁决：做（实施中，待 R6-d 以测试佐证回填）** |
| `background.enabled` 全局开关 | 无 | 做（**DEC-4**，参考有；迁移默认 `true`） | **裁决：做（实施中，待 R6-d 以测试佐证回填）**——R6-a |
| `interval` / `random` | 有，但区间收窄：`0`（关）或 `5–300` 秒（`display_prefs.dart:341-342`） | 规划 §5.2 只要求「区间/clamp 一致」；**DEC-1** 进一步裁决为**端点夹持**（>300→300、<5→5） | **裁决：做（实施中，待 R6-d 以测试佐证回填）**——R6-a；改动**前**的实测见下注 ① |
| `opacity` | 0–1，默认 **1.0**（`display_prefs.dart:303-305`） | 保留（我们允许不透明；参考 fullscreen 锁在 0–0.6） | **已支持（保留）**——R6-c 实跑：`shell_backdrop_test.dart` ›「缺省值：加了图就该看得见 › 不透明度 1.0，模糊 0，遮罩 auto」；`display_prefs_test.dart` ›「背景库（2026-09-27：有序图库 + 内置图案 + 轮播参数）› 缺省就是「没有背景」——与本轮之前「没选图」的观感一致」（断言 `backgroundOpacity == 1.0`）；另 `… › 越界 / 非有限数一律 clamp 到区间（模糊不许变负）` |
| `position` | 9 宫格 `imageAlign`（`display_prefs.dart:184,324-326,373-384`） | 保留（比任意 CSS 更可点选） | **已支持（保留）**——R6-c 实跑：`display_prefs_test.dart` ›「背景库… › 缺省就是「没有背景」…」（断言 `imageAlign == defaultImageAlign`）、`… › 枚举字段越界**回落默认**而不是夹到端点`（`imageAlign:-3` → 默认）。**边界**：9 个取值的枚举完整性本轮无人断言；DEC-7a 只改「位置控件的显隐条件」，不动取值 |

**证据口径与未佐证清单（R6-c）**：

- 上表「现状」是**写作时快照**；「实施状态」列是 **2026-09-28 R6-c** 的判断。**B1/B2/B3 是旧波次名**
  （Stage B 现行波次 = **R6-a / R6-a2 / R6-b**，见 `docs/plans/ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md` §4）。
- R6-c 的实跑（唯一证据来源）：HEAD `31d06cb4` 上
  `cd shell/flutter && flutter test test/display_prefs_test.dart test/shell_backdrop_test.dart test/background_logic_test.dart test/display_prefs_slide_index_test.dart`
  ⇒ **`+112: All tests passed!`（exit 0）**；测量前后 `git hash-object` 一致（工作树未漂移）。
- **拿不到测试佐证的项一律写「裁决：做（实施中，待 R6-d 以测试佐证回填）」**：R6-a / R6-b 正在同一批
  文件上实施，R6-c **不代替它们宣布结果**，也**没有**为它们跑过任何测试。
- 注 ①（`interval`/`random` 改动**前**的实测，**仅供对照，不是 DEC-1 判决后的语义**）：
  R6-c 实跑 `display_prefs_slide_index_test.dart` ›「轮播间隔：存储 clamp 与滑杆共用同一上界（P1-6 回归）
  › 存储上界 = 滑杆上界（300）：不再有第二套上界 3600」**通过** ⇒ 当时现状为
  「`slideInterval: 3600` 读回 `0`（静默关掉轮播）」；DEC-1 裁决改成端点夹持后，**该测试由 R6-a 同步改写**。

---

## 7. 未决项与裁决结果（2026-09-28 R6-c 回填；原标题：未决 / 留给维护者（只记录，不裁决））

> **状态变化**：本节原先「只记录，不裁决」。编排者已按
> `docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md` §6 Stage B · B.0 的**建议默认**逐条裁决，
> R6-c 把结果回填在此。**七条裁决全部落在实施侧（R6-a / R6-b）或「不做」；
> R6-c 不代替实施波次宣布「已完成」**——凡未拿到测试佐证的，一律记「待 R6-d 以测试佐证回填」。

### 7.1 DEC-1…DEC-7 裁决表

| # | 问题（出处） | 裁决结果 | 实施波次 |
|---|---|---|---|
| **DEC-1** | `slideInterval` 越界（301–3600）与存量 1–4 秒回落 `0`＝**静默关掉轮播**（rc.5 §9.1；§6「区间/clamp 一致」） | **做：端点夹持**（>300→300，<5→5；`0` 仍＝关）。新增专用 `_clampIntToRange`；**不改** `_clampInt`（`scrim` 依赖「越界回落默认」） | **R6-a** |
| **DEC-2** | **两套轮播并存**：`stagePlaylist`（舞台单图，走 `stage-bg` 帧，rc.5 §9.2 记**零测试覆盖**）vs 背景库轮播（Flutter 层）；本文件原 §7 第 1 条 | **不合并**（合并要改后端）；改为**分工 + 改名 + 按 `backgroundSource` 互斥显示**（「舞台单图轮播」/「壳背景轮播」）；**保留 `stagePlaylist` 则先补它的守护测试** | **R6-b** |
| **DEC-3** | 壳内子区域是否堆砌（§5.2；规划 §5.3 第 3 条） | **不做**，并写明理由：无用户诉求 + 避免堆砌 +「舞台是主角」（已写入 §5.2） | **不做**（记 Stage C backlog） |
| **DEC-4** | 全局 `background.enabled`（§6 当时无此开关） | **做**（低成本，参考有；迁移默认 `true`） | **R6-a** |
| **DEC-5** | C4：坏 dataURL 仍 `isRenderable=true`（rc.5 §9.6） | **收紧**到真 dataURL 形态（`data:image/…` + 逗号 + 非空 payload）；坏图要能被 UI 告知 | **R6-a** |
| **DEC-6** | C3：`currentItemIsImage` 只看第 0 项（rc.5 §9.6） | **修**：把 AppShell 已有的运行时 `_backgroundIndex`（`app_shell.dart:181`，由 `ShellSlideshow.onAdvance` 驱动）传进外观区判「当前项」；**不持久化**运行时索引 | **R6-b** |
| **DEC-7** | D3 位置显隐条件与注释相反；D4 来源＝舞台那张时轮播控件空转且文案误导（rc.5 §9.6） | **都修**（显隐对齐；空转改「禁用＋说明」或隐藏，P4 禁止静默失效） | **R6-b** |

**落地状态（写此文档时）**：**只有 DEC-3 已由本文件 §5.2 写明「不做」并闭环**；其余六条均在实施波次中，
**R6-c 未为它们跑过任何测试 ⇒ 一律记「待 R6-d 以测试佐证回填」**，本节不预告结果。

### 7.2 原记录（三条，逐条已裁决）

1. **`stagePlaylist` 与背景库轮播语义重叠**：一边是舞台单图轮播列表
   （`DisplayPrefs.stagePlaylist`，`display_prefs.dart:219-229` + `appendToStagePlaylist` / `removeStagePlaylistAt` /
   `moveStagePlaylist`，`:900-1006`；预算 `kStagePlaylistMaxItems`/`kStagePlaylistMaxChars`；
   通道 = `stage-bg` 帧），另一边是背景库轮播
   （`backgrounds[]` + `slideInterval`/`slideRandom`；预算 `kBackgroundMaxCount`/`kBackgroundImageMaxBytes`；
   通道 = Flutter 层）。**两套列表、两套预算、两条下发通道**，用户可见的语义却是同一句
   「背景轮播」。谁是真源、是否合并（或明确分工：舞台单图 vs 壳背景库），**需产品裁决**——
   本文件当时只记录这个重叠，**不裁决**。
   ⇒ **已裁决 = DEC-2**（§7.1）：**不合并**；改为「分工 + 改名 + 按 `backgroundSource` 互斥显示」，
   且**保留 `stagePlaylist` 的前提是先补它的守护测试**（该列表零覆盖）。**实施波次 R6-b。**
2. **壳内子区域是否堆砌**（§5.2）：技术可做，产品未定。规划 §5.3 第 3 条要求
   「若判定为堆砌，**写明不做**」——那句「不做」当时还没人写。
   ⇒ **已裁决 = DEC-3**（§7.1）：**判定为堆砌，不做**，理由与恢复路径已写进 §5.2。
   **实施波次：不做**（记 Stage C backlog）。**本条已闭环。**
3. **`docs/README.md` 索引未更新**：B4 的文件归属是「新增 `docs/**`（不碰代码）」，
   且当时任务明令不碰既有文件；索引维护原归 Stage C · C1（`IMPL-PROMPTS-0.2.0-seal-2026-09-27.md` §C1 第 2/4 条）。
   本文件当时**只能靠路径被发现**。
   ⇒ **已关闭（2026-09-28 R6-c）**：`docs/README.md`「架构与契约（当前）」已补本文件条目；
   并顺带在该索引「代码审计」下补了审计账本
   [`docs/audit/2026-09-28-frontend-nightly/`](../audit/2026-09-28-frontend-nightly/README.md)
   （长跑 21 批 45 条为准 + short-run 快照）。**R6-c 全程未改任何代码。**

---

## 8. 未核实清单（**没核实的都在这里，不做成断言**）

| # | 事项 | 状态 |
|---|---|---|
| 1 | 参考的**运行时**行为：CSP 是否真的拦 / 放内联脚本、`http://` 图在 VS Code 里是否真能显示、code-server 下文件夹 glob 的行为、sudo 路径在三大平台的表现、integrity 提示出现的真实频率 | **未核实**——只读了 `eef5ddb` 的源码与文档，**没有安装、没有运行**该扩展 |
| 2 | `README.zh-CN.md:91`（「只允许 `https` 协议」）与 `PatchGenerator.base.ts:62`（`startsWith('http')`，`http://` 也放行）**文案与实现不一致** | 不一致**已核实**（读代码）；运行时到底谁生效**未核实** |
| 3 | `ignite.sh --check` 探针的**字面覆盖范围** | 已核实：`scripts/ignite.sh:88-101` 只扫 `/app/index.html` 与 `/app/main.dart.js` 中的 `gstatic.com/flutter-canvaskit`，**不会**扫任意 https 图片 URL。「在线图违反离线红线」是该原则的**推论**，已在 D1 写明 |
| 4 | Flutter Web 的 `showDirectoryPicker`（目录选择）可用性与代价 | **未核实**——D2 的理由不需要它（我们不需要这条能力），若将来有人想论证「文件夹可做」，必须先核实这一条 |
| 5 | 我方行号 | 快照：工作树有 **R6-a / R6-b** 未提交改动（`display_prefs.dart` / `shell_backdrop.dart` / `background_item.dart` / `appearance_section.dart`）；文件与符号名为准，行号会漂移 |
| 6 | 参考的 `images` 是否支持 `http://`（非 https）**并且**被 VS Code 的 CSP 放行 | **未核实**（见 #1/#2） |
| 7 | 参考在 Web/code-server 模式下的 `vscode-file://` 归一化（`base.ts:86-102`）是否仍必需 | **未核实**——只记录它存在的理由（v1.51.1 后的 file 协议限制） |
| 8 | **R6-c 的「已支持」判定（本表新增）** | 已在 HEAD `31d06cb4` 实跑 §6 列的 4 个测试文件（`+112 All tests passed!`，exit 0）；**该基线早于 R6-a / R6-b 的落盘** ⇒ 结论只对「当时那份树」有效，实施波次落地后必须由 **R6-d** 重跑并回填 |

---

## 9. 参考出处索引（全部指向 `eef5ddb`）

- `package.json`：`:4` 版本、`:54-311` contributes、`:86-90` enabled、`:91-145` editor、`:146-200` fullscreen、
  `:201-236` sidebar、`:237-272` auxiliarybar、`:273-308` panel、`:312-314` activationEvents
- `README.zh-CN.md`：`:52-75` 配置项总表、`:79-110` editor 示例（在线 / 本地 / `~` / 环境变量 / 文件夹 / data URL）、
  `:112-128` 其余区域表、`:130-163` 其余区域示例、`:173-175` 「本插件是通过修改 vscode 的 html 文件的方式运行」
- `package.nls.zh.json`：`:13` `images` 说明、`:18` opacity 越界回 0.1、`:19-20` size/position
- `src/utils/patchTargets.ts:30-40`：`workbench.html` 定位
- `src/background/PatchFile/PatchFile.html.ts:5-32`：CSP `unsafe-inline` + 内联 `<script>`
- `src/background/PatchFile/PatchFile.base.ts:72-116`：只读文件的 sudo 提权重试
- `src/background/PatchGenerator/PatchGenerator.base.ts:56-75`（`images` 分派）、`:86-102`（`vscode-file://` 归一化）、
  `:107-123`（`~` / 环境变量展开）、`:133-146`（文件夹 glob）、`:156-158`（stylis）、`:169-176`（剔除 `pointer-events`/`z-index`）、
  `:216-240`（`<style>` 注入 + IIFE）
- `src/background/PatchGenerator/PatchGenerator.editor.ts:30-40`（逐图样式）、`:42-87`（`useFront` / `::after`/`::before`）、`:89-139`（轮播）
- `src/background/PatchGenerator/PatchGenerator.fullscreen.ts:16`（`body::after`）、`:24-29`（opacity clamp）、`:32-51`（样式）、`:69-107`（轮播）
- `…sidebar.ts:6` / `…auxiliarybar.ts:6` / `…panel.ts:6`：分区选择器
- `src/background/PatchGenerator/PatchGenerator.checksums.ts:8-57`：用 CSS 屏蔽 integrity 提示
- `src/background/PatchGenerator/index.ts:19-38`：生成器编排（checksums + theme + 5 区域，terser 压缩）

**我方出处**：`shell/flutter/lib/settings/display_prefs.dart`、
`shell/flutter/lib/ui/shell_backdrop.dart`、`shell/flutter/lib/design/background_item.dart`、
`shell/flutter/lib/app/shell_prefs.dart`、`AGENTS.md`（「工作区与点火纪律」/「前端层（Flutter）」）、
`scripts/ignite.sh`、`docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md` §5、
`docs/plans/IMPL-PROMPTS-0.2.0-seal-2026-09-27.md` §B1–B4。
