# AI 伴侣 / Live2D 桌宠 —— 视觉设计语言与信息架构调研（2025–2026）

> 调研员：UI 设计调研员（子代理）
> 日期：2026-09-10
> 范围：**只提取视觉设计语言与信息架构**（布局 / 配色 / 圆角间距 / 字号阶梯 / 阴影 / 设置分区 / 关键交互 / 动效 / 缺陷），**不写功能清单**。
> 产出用途：为本项目（Live2D-Ai，Flutter Web 前端）从零重做前端提供可直接落地的设计决策。
> **不重复**已有调研：`competitive-analysis-report.md`（功能矩阵）、`neko-ui-alignment-and-gaps.md`（对已归档 Python 前端的对标）、`ux-benchmark.md`、`benchmark-neko-vs-neurosama.md`。
>
> **证据纪律**：本文所有断言分四级。
> - `【已验证 file:line】` / 行内反引号 `文件:行` —— 直接读到源码，附路径与行号。**这是本文最主要的标注方式**：绝大多数事实性断言以行内 `文件:行` 形式给出，括号标签只用于**聚合计数、截图结论或推断**这类"证据类型不明显"的地方。
> - `【已验证 截图】` —— 来自官方截图（源码内文档截图 / 商店页官方截图），附来源 URL；**像素级数值不据此断言**。
> - `【推断】` —— 显式标注的推理，**不作数值断言**（全文 27 处）。
> - `未核到` —— 该维度没有拿到一手证据。**宁可留空，不用常识补全。**（汇总见 §11：**13 条按维度 + 8 条按样本，共 21 条**；全文共 27 处标注。）
>
> 所有被调研仓库均为 zip 快照（本机 `git clone` 因 git 级代理配置失效不可用，改用 `codeload` 归档），因此**无法给出 commit hash**；引用一律用 `文件:行`。
> 所有被引用仓库的源码树保留在 `/tmp/uisurvey/`（未写入本仓库）；本报告只新增本文件一个产物。

---

## 结论先行

### 最值得抄的 5 个设计决策

1. **把主题收成一个标量：色相（hue）** —— AIRI 的全部品牌色只由**一个数**决定（**OKLCH 色相角**，默认 `220.44`，`packages/stage-ui/src/stores/settings/theme.ts:2,5`），可在设置页手动拖动，也可**从模型/背景图里采样自动提取**（`theme-color.ts:97-101` 用 html2canvas 采样背景顶部 140px）。派生出**一条 chroma 公式 + 11 档系数**（`packages/ui/src/fallback.css:1-15`：`0.18 + cos(hue·π/180)·0.04`，`50:×0.3 … 600:×1.15 … 950:×0.5`），最终经 `color-mix(in srgb, oklch(95% …), oklch(50%|90% 0 360))` 落地（`App.vue:80-100`）。换色不改任何组件，只改一个 CSS 变量。→ **我们应该**：Design Token 的“品牌色”只定义**一个色相基准**，衍生色全部由它算出来；不要让组件认识具体色值。（对照：Ghost Vessel 用**只换 `bg + accent` 两个值**的 12 变量主题达成同样效果，§7.9.1——两条路都可行，选一条并坚持。）

2. **设置首页做“图标 + 标题 + 说明”的卡片菜单，而不是侧栏** —— AIRI 的设置首页是一列大卡片（**大号低对比图标水印** + 标题 + 一句人话说明），点进去才是详情页（官方截图 `manual-settings-window.avif`）。分区只有 8 个且**每项都有一句解释**（`settings.yaml:462-467,517-519,529,1495-1497,1688-1690`）。→ **我们应该**：设置首屏用“说明型卡片菜单”，用户不需要先学会产品黑话才能找到开关。

3. **状态用“一个 Pill + 一个颜色语义 + 一个节奏”表达，且映射集中在一处** —— Meuxe 把 `listening / speaking / thinking` 三个状态映射集中在一个计算块里并配固定语义色（`src/App.tsx:471-477`），迷你窗**复用同一映射**；Soul of Waifu 更进一步做到**四态四色并同步到渲染面**（`sow_system_signals.py:213-218,240`）；Nexus 则**按状态换动画速度**（`breathe 5.8s / think 1.7s / listen 2.4s / speak 0.72s`，`App.css:2840-2862`）。→ **我们应该**：把“连接 / 思考 / 说话 / 聆听 / 离线”做成**一个**组件、一份状态枚举 → 一份“色 + 字 + 节奏”映射表，禁止各页面自造状态样式（OLV-Web、my-neuro、Live2DPet、N.E.K.O、Nexus、Amica **六个项目都栽在这里**）。

4. **桌面壳的“可见性”必须是显式契约：透明 + 无边框 + 置顶 + 不抢焦点 + 点击穿透（带热点豁免）** —— 五个项目独立收敛到几乎相同的窗口参数：AIRI 主窗 **450×600** `transparent/frame:false/hasShadow:false`（`windows/main/index.ts:81-97`、`windows/shared/window.ts:39-46`）；OLV-Web **900×670** 同款并额外 `titleBarOverlay`（`window-manager.ts:56-78`）；Meuxe 迷你窗 **280×420** `transparent/decorations:false/always_on_top/resizable:false/skip_taskbar`（`src-tauri/src/window.rs:19-28`）；Nexus pet **320×460** + `setAlwaysOnTop(true,'floating')`；Super-agent-party 的 VRM 窗 **540×960**。→ **我们应该**：把这一组参数写进桌面壳的一处常量，规定**气泡/字幕窗必须 `focusable:false`**（Live2DPet `window-manager.js:184,229`），并让穿透逻辑**带热点豁免**（`clickThrough && !petHotspotActive`，Nexus `petWindowInstances.js:88-91`）。

5. **用“令牌 + 主题包”换风格，而不是写两套组件** —— my-neuro 的 WebUI 自述“组件样式只引用变量，做到主题无关”（`webui/static/new/css/themes.css:1-6`），并且它把**形状**也令牌化（`--clip-tab` 与 `--br-tab` 成对，`themes.css:59-72`），于是 cyber 主题能把胶囊按钮整形成切角多边形（`themes.css:179`）。→ **我们应该**：圆角/切角/发光/动效曲线与颜色一起进令牌。

### 最该避开的 5 个坑

1. **令牌引用静默失效（本次调研出现 4 次）** —— Amica 丢掉了 ChatVRM 的 `charcoal-ui` preset，导致 `typography-16`、`rounded-4`、`w-col-span-6` 等约 14 处类名**渲染为零样式**，`bg-rose/90` 让四个消息卡标题栏变成透明底白字（`tailwind.config.js` 无 `presets` 键）；Meuxe 自己的自曝机制抓到 `hover:bg-clay-600` 未定义；SAP 的 `var(--primary-color)` **用了 9 次却从未定义**（菜单 hover/active 主色实际无效）、`var(--el-primary-color)` 拼错 5 次；Live2DPet 的 `showStatus(..., '')` 撞上 `.status{display:none}` 让“合成中”**永远渲染不出来**。→ **我们应该**：令牌表由**我们自己的构建产物**保证存在，并加一个“被引用但未定义即失败”的**枚举式校验测试**（这正是 Meuxe 抓到自己 bug 的方法）。

2. **两个（或更多）并列强调色** —— 本次调研里**出现频率最高的缺陷，横跨 8 个项目**：OLV-Web 全仓库只有一处硬编码品牌色 `#7C5CFF` 却混用 Chakra 默认 `blue.500/green.500/yellow.500/red.500`；Amica 至少 6 个色族并存；N.E.K.O 在 9k 行 CSS 里 `#40c5f1` **171 次** vs `#22b3ff` **39 次**（两个蓝）；Live2DPet 主色 `#7c4dff` 旁边有页脚 `#58a6ff`；my-neuro 单页最多 4 种；Soul of Waifu **6 套**；Nexus **6 套**；AIRI 的进度条粉色**游离在色相体系之外**。→ **我们应该**：1 个品牌色 + 3 个语义色（成功/警告/危险），**其余一律从品牌色派生**。

3. **状态/连接指示缺失或只有文字** —— OLV-Web 把 6 个 AI 状态全部塞进**同一个恒定紫色胶囊**，只有文字变，且“思考/说话”在状态机层面就是**同一个值** `thinking-speaking`；N.E.K.O 把承载连接文本的 `#status` 用 `display:none !important` **主动隐藏**，只剩 2 秒 toast；my-neuro 与 Live2DPet 的思考态**完全没有 UI**；Amica 的 `chatSpeaking` 创建了却**从未渲染**；Nexus 的离线态只靠 `opacity: 0.72`。→ **我们应该**：每个状态至少有“色 + 形 + 字”三个通道里的两个，并且**连接态必须常驻可见**。

4. **把整块 UI 塞进透明全屏窗、所有浮层共用一个 z-index 战场** —— my-neuro 把桌宠窗设成**所有显示器 bounds 的并集**（`live-2d/main.js:443-466`），聊天框 / 字幕 / 气泡 / 控件都是这个窗口内部的 `fixed` 图层，z-index 出现 `10000 / 9000 / 1001 / 999` 混用。Nexus 则在单个 panel 窗里同时塞了绝对定位舞台 + **全屏覆盖式聊天层**（`chat-sheet-v2.css:16-24`），且全仓 z-index 无尺度（`10000×3 / 9999 / 120` 与 0–40 并存）。→ **我们应该**：要么严格“1 OS 窗 = 1 职责”（Meuxe / Live2DPet / Ghost Vessel 路线），要么在单窗内先定义**分层的 z-index 令牌表**。

5. **把体积大、可读性差的控件直接铺在设置首屏** —— AIRI 的“服务来源”页把 5 类 provider 连同价格/部署筛选条/卡片列表全部堆在一页（官方截图 `image-8.png`），该页源码 326 行、i18n 文案约 540 行；SAP 则是**一级导航 11 项**。→ **我们应该**：设置首屏只放“分区入口”（≤8 项），二级页面才放列表，列表才放筛选；一级导航 ≤3。

---

## 0. 本次调研的对象、取源方式与许可边界

### 0.1 取源方式（可复现）

本机 `git clone` **不可用**（git 全局配置指向失效代理 `http://172.18.96.1:10808`），因此全部改用 GitHub `codeload` 归档 + `python3 -m zipfile` 解包：

```
curl -sL -o X.zip "https://codeload.github.com/<owner>/<repo>/zip/refs/heads/<branch>"
python3 -c "import zipfile; zipfile.ZipFile('X.zip').extractall('x_X')"
```

元数据（star 数、license、最后推送时间）走 `https://api.github.com/repos/...`，查询日期 **2026-09-10**。

### 0.2 许可概览（决定“能不能抄代码”）

**这是本报告里唯一需要逐字核对的表**：本项目自有代码为 **AGPL-3.0-only**，可复用范围受此约束。

| 项目 | 许可（一手核对） | 与本项目复用兼容性 |
| --- | --- | --- |
| AIRI (`moeru-ai/airi`) | **MIT**（`LICENSE`；GitHub API `spdx_id: MIT`） | ✅ 兼容，署名即可 |
| Open-LLM-VTuber 上游（Python 主仓） | **MIT**（`LICENSE:1-3`「MIT License / Copyright (c) 2025 Yi-Ting Chiu」） | ✅ 主仓代码兼容 |
| Open-LLM-VTuber-Web（**当前前端**） | ⚠️ **不是标准 Apache-2.0**：GitHub API 报 `NOASSERTION`；`LICENSE` 为「Apache-2.0 + Additional Conditions」自定义许可，附加条款限制付费访问/托管、商业再分发换牌、商业嵌入 | ⚠️ **不可当 Apache-2.0 直接搬**；仅可参考设计，不宜复制代码 |
| ↑ 其内置 `src/renderer/WebSDK/` | Live2D Cubism SDK 官方样例（`WebSDK/Core/LICENSE.md` = **Live2D Proprietary Software License**；`WebSDK/Framework/LICENSE.md` 要求年营收 ≥1000 万日元企业取得 Cubism SDK Release License） | ❌ 与 upstream 代码许可无关，需独立评估 Live2D EULA |
| N.E.K.O (`Project-N-E-K-O/N.E.K.O`) | **Apache-2.0**（`LICENSE:2-4` + `NOTICE:7`）——注意：**不是** AGPL-3.0 | ✅ 兼容，保留 NOTICE 即可 |
| Meuxe (`meet447/Meuxe`) | **MIT**（`LICENSE:1-3` + `package.json:4`） | ✅ 兼容 |
| my-neuro (`morettt/my-neuro`) | **MIT**（`LICENSE:1-3`） | ✅ 兼容 |
| Live2DPet (`x380kkm/Live2DPet`) | **MIT**（`LICENSE:1-3`）；第三方 VOICEVOX/VVM/Open JTalk **不随仓库分发**，产物侧有署名义务 | ✅ 代码兼容；产物需署名 |
| ChatVRM (`pixiv/ChatVRM`) | **MIT**（`LICENSE:1-3`） | ✅ 兼容 |
| Amica (`semperai/amica`) | **MIT**（`LICENSE:1-3`「Semper AI, pixiv Inc.」） | ✅ 兼容 |
| super-agent-party (`heshengtao/super-agent-party`) | **AGPL-3.0**（GitHub API） | ✅ **同许可**，最无摩擦 |
| 其余（Soul of Waifu / Nexus / ghost-vessel / Agent-LLM-Live2D / LunaMate） | 见 §11「未能核到证据的清单」 | — |

> **一条硬结论**：本次调研里**设计系统最成熟、最值得抄的两个样本（AIRI、Meuxe）都是 MIT**，因此“参考它们的令牌结构”没有任何许可障碍。反过来，**当前 upstream 的前端（OLV-Web）许可最不友好**，虽然它是我们最直接的同类，但只能看设计、不能搬代码。

---

## 1. 横向对比：布局 / 信息架构 / 视觉系统一览

> 每格都有一手证据，详见第 2 章起的逐项目分解。`未核到` 表示无证据。

| 项目 | 舞台与聊天/控制的位置关系 | 设置导航形态 | 是否有集中式 design token | 品牌色 |
| --- | --- | --- | --- | --- |
| **AIRI**（桌面） | 舞台占满 **450×600** 透明置顶窗；控制与聊天是**浮在角色上的浮层**（控制岛 + 独立聊天窗 + 字幕浮窗，`windows/` 下 14 种窗口） | 设置首页 = **8 张图标卡片菜单** → 二级页；系统页另有 4 个子页 | ✅ **有**：`uno.config.ts` theme + `presetChromatic({baseHue:220.44})` + 单一 `themeColorsHue` 标量 | 由 hue 生成（默认 220.44，青蓝） |
| **N.E.K.O** | 角色常驻右侧，聊天是**左侧浮层面板**（毛玻璃），另有字幕浮窗 + 独立“插件管理”窗口 | 独立窗口内 **左侧侧栏 + 分段 Tab + 卡片/列表双视图** | ❌ **无颜色令牌**：`static/css/*.css` 合计 9k+ 行，293 个 CSS 变量里**没有一个颜色变量**，颜色全是裸 hex | 青蓝 `#40c5f1`（171 次）与 `#22b3ff`（39 次）**并存** |
| **Open-LLM-VTuber-Web**（当前前端） | 舞台绝对定位铺满；聊天是**左侧 440px 可折叠面板**（收起留 24px 把手）；底栏 120px | 设置 = **左侧 Drawer(440px) + Drawer 内 6 个横向 Tab** | ❌ **无主题定制**：`extendTheme/semanticTokens` 全仓库 0 命中，直接用 Chakra v3 `defaultSystem` | 唯一硬编码 `#7C5CFF`，只用在 AI 状态胶囊上 |
| **Meuxe** | **三栏并列**：64px 图标轨道 + `flex-1` 舞台卡片 + **380px 会话面板**；另有 280×420 透明迷你窗 | 居中模态 sheet（`max-w-4xl`）+ **左侧 `w-60` 分组导航**（Companion / You 两组） | ✅ **有且最强**：`@theme` 显式禁用 Tailwind 默认色板（`--color-*: initial`） | pastel amber `#dea03a` + 语义色族 |
| **my-neuro** | 桌宠窗 = **全虚拟桌面的透明穿透层**，聊天框/字幕/气泡是窗口内 `fixed` 图层；设置是**独立无边框窗**（1080×760） | 控制窗 = **180px 侧栏 + 10 页**；WebUI = **顶部 10 个横向 Tab**；**同名功能两套叫法** | ✅ WebUI 有双主题令牌（galgame 粉 / cyber 青）；❌ 桌宠层与控制窗 **0 个令牌、209 个硬编码色** | WebUI `#e75480` / 桌宠层 `#7eb3e0` / 控制窗橄榄灰 `#7d7362` |
| **Live2DPet** | **三个完全独立的 OS 窗**：设置窗 480×600 / 桌宠窗 300×300 透明 / 气泡窗 250×80 不可聚焦 | 顶部 **5 个等宽 Tab**（Settings / Model / Emotion / TTS / Prompt）+ 单列卡片 | ❌ **零 CSS 文件、零令牌**：三个 HTML 内联样式，`:root` 与 `var(--` 命中 0 | 紫 `#7c4dff`（另有页脚 `#58a6ff`） |
| **ChatVRM** | 舞台 `absolute w-screen h-[100svh] -z-10` 铺满，面板全是**绝对定位浮层** | **一个扁平滚动页**（5 个小节，无 Tab 无面包屑） | ❌ 依赖外部 `charcoal-ui` preset（未随仓库分发，**数值未核到**） | 依赖 charcoal token |
| **Amica** | 同 ChatVRM，但“chat mode”是**把渲染分辨率减半 + 右移 65%** 的 hack | 11 项主菜单 + 嵌套子页 + 面包屑 | ❌ 同上，且丢了 preset → 约 14 处类名失效 | 至少 6 个色族并存 |
| **Soul of Waifu** | 主窗（1350×734，**最小=默认**）内三栏；**舞台是 200–800px 可拖右栏**；另有 400×600 置顶桌宠窗 + 独立字幕窗 | `QListWidget` 行 + `QStackedWidget`；设置是**二级侧栏（230px）+ 16 张玻璃卡** | ❌ **无 token**：335 个不同 hex / 902 次 / 1,196 处内联 `setStyleSheet` | 六套并列（`#4BB8FF`/`#82CDFF`/`#C49A38`/`#A855F7`/`#8ab4f8`/`#6C86FF`） |
| **Nexus** | **两个独立无边框透明 OS 窗**：pet 320×460 / panel 460×660（折叠 380×92 是**真窗口缩放**）；panel 内舞台是绝对定位浮层、聊天是全屏覆盖层 | 模态抽屉 + 左侧边栏（`minmax(152px,184px)`）+ **组内二级 tab**；**另有整套 V1 IA 残留** | ⚠️ **两个中心源打架**：`--radius-*` 四档同值且全仓用 1 次 vs 硬编码 `border-radius:8px` **223 处**；`var(--shadow-*)` 用 **0 次** vs 151 个 shadow 字面值 | 六套并列（`#8274c9`/`#7d67d9`/`#333333`/`#A88BFF`/`#8fa8ff`/青+琥珀） |
| **super-agent-party** | 主窗**左右各 50%**（`.chat-area 50%` / `.side-panel 50%`）；**形象不在主窗内**——VRM 是独立 always-on-top 透明窗（默认 **540×960**） | 一级导航 **11 项**；系统设置 **4 节**（通用/外观/快捷指令/账户管理） | ⚠️ token 完备（9 套主题 × 60+ 变量）但与 **362 个硬编码 hex** 长期并存；`!important` **1,514 处** | 亮 `#17827a` / 暗 `#c8815a` |
| **Ghost Vessel** | **双窗**：形象窗 360×640 + 对话窗 400×560（均无边框/透明/置顶/无阴影） | **无设置页**，全部塞在 **280px 浮层**（语言/暗/亮/系统/麦克风/语音引擎） | ✅ 全 UI 仅 **12 个 CSS 变量**；12 套主题**只换 `bg + accent`** | 由主题提供（MIT，可吸收） |
| **VTube Studio**（商业标杆） | 角色占满主窗，**左侧一列彩色圆形图标 rail**；配置是弹出面板 | 图标 rail → 各配置面板 | 未核到（闭源） | 每功能一色（红/绿/黄/蓝/粉/橙） |
| **Warudo**（商业标杆） | **双窗口分离**：main window 做演出，editor window 做配置 | Editor 内 **Assets / Blueprints 两个顶层 Tab** + Settings 菜单 | 未核到（闭源） | 未核到 |

### 1.1 四条跨项目的结构性观察

1. **“舞台 + 聊天”的主流解法是浮层，不是分栏。** 10 个开源样本里只有 Meuxe 做真正的并列分栏（`App.tsx:480,505` + `HistoryDrawer.tsx:18`）、**super-agent-party 做严格的 50/50**（`styles.css:4068,4523`）、OLV-Web 做 440px 可折叠侧栏（`sidebar-styles.tsx:35-51`），其余全是浮层或独立窗口。**原因**：桌宠场景下舞台必须能占满透明窗，任何“分栏”都会切掉角色的可用面积。→ 【推断】对一个**以角色为卖点**的产品，默认形态应是“舞台满幅 + 聊天浮层”，分栏是“工作/复盘模式”的变体。
2. **最彻底的解法是“控制台与形象分离成两个 OS 窗”。** super-agent-party 的做法最激进——**主窗只做对话，VRM 形象在一个独立 always-on-top 透明窗里**（默认 540×960，`main.js:2017-2022`）；Warudo 也是 main/editor 双窗（§6）；Ghost Vessel 是形象 360×640 + 对话 400×560 双窗。→ 【推断】这条路线**天然回避了“面板遮挡角色”的一整类问题**，并且与本项目「Rust 桌面壳 + Web 前端」的分层完全对应。
3. **设置导航形态与设置规模非线性相关。** AIRI 有 8 个分区但用**卡片菜单**（`settings/index.vue:29-56`）；my-neuro 有 10 项就用**侧栏**（`control-refined.css:48`）；OLV-Web 只有 6 项却用**Drawer + 横向 Tab 两层**（`setting-ui.tsx:98-164`）；Meuxe 8 页用**模态 + 分组侧栏 + `PAGE_META` 单点枚举**（`Settings.tsx:52,61-94`）；SAP 用 **11 项一级导航 + 4 节系统设置**。→ 决定导航形态的是**分区之间的语义距离**，不是数量。
4. **“有没有设计令牌”与项目成熟度强相关，但**不是**与 star 数相关。** 49k star 的 AIRI 有完整令牌链；13.7k star 的 OLV-Web 完全没有；1358 star 的 my-neuro 只有 WebUI 那一层有；而只有 79 star 的 Meuxe 拥有本次调研中最干净的一套。→ 令牌体系是**团队决策**，不是规模产物；我们现在补上完全来得及。

---

## 2. AIRI（moeru-ai/airi）—— 本报告最重要的参考样本

> 规模：⭐ **49,010**（2026-09-10 查询），`pushed_at` 2026-09-10T10:27Z，TypeScript，**MIT**。
> 一手证据：仓库内官方文档截图（`docs/content/zh-Hans/docs/manual/tamagotchi/setup-and-use/assets/*.avif`，对应版本 **AIRI-0.12.0-beta.5**，见 `setup-and-use/index.md:19`）+ 完整源码（`packages/stage-layouts/`、`packages/stage-pages/`、`packages/ui/`、`apps/stage-tamagotchi/`）。

### 2.1 整体布局：舞台占满小窗，一切 UI 都是浮层

- **主窗尺寸 450×600**，并且**尺寸/位置会被持久化**（`windows/main/index.ts:81-84` 的 `width: mainWindowConfig?.width ?? 450.0, height: ... ?? 600.0`；`:188-224` 在 `resize`/`move` 时写回配置）。托盘菜单里也把 **“推荐 (450x600)”** 作为一等选项（`i18n/zh-Hans/tamagotchi/electron/tray.yaml`）。
- **窗口形态**：`transparentWindowConfig()` = `{ frame:false, titleBarStyle: isMacOS?'hidden':undefined, transparent:true, hasShadow:false }`（`windows/shared/window.ts:39-46`）；`setVisibleOnAllWorkspaces(true)`、mac 下 `setFullScreenable(false)` + `setWindowButtonVisibility(false)`、`setWindowAlwaysOnTop(window, true)`（`windows/main/index.ts:220-228`）。
- **关窗 = 隐藏，不退出**：`window.on('close')` 里 `event.preventDefault(); window.hide()`（`windows/main/index.ts:211-218`）。
- **拖拽**：非 Linux 平台引入 `electron-click-drag-plugin` 做原生拖拽（`windows/main/index.ts:240-252`）；UnoCSS 里还专门定义了 `drag-region` 规则映射到 `app-region: drag`（`uno.config.ts:205`）。
- **浮层而非分栏**：官方截图（`manual-main-window.avif`）显示角色铺满 450×600 窗，**右下角只有 4 个竖向小方按钮**（展开 `⌃` / 听觉 `🎤` / 扬声器 / 移动 `✥`）；点“展开”后弹出一个**深色毛玻璃 3×3 圆角图标岛**（官方截图 `image-2.png`）。
  - 控制岛的 CSS：`'absolute right-0 translate-y-[-100%]'` + `max-w-full overflow-y-auto overscroll-contain px-3 py-3 font-sans scrollbar-none` + **`transition-[height] duration-250 ease-out`**，溢出时用 `mask-image: linear-gradient(to bottom, transparent 0, black 1rem, black calc(100% - 1rem), transparent 100%)` 做上下渐隐（`MobileInteractiveArea.vue` 域内，见 digest 行 52）。
  - **控制岛高度上限**：键盘不可见时 = `visibleHeight - messageComposerHeight`；键盘可见时 = `Math.min(availableHeight, visibleHeight * 0.45)`（**≤ 视口 45%**）。移动端聊天历史高度 = **视口 35%**（`use-mobile-interactive-area-layout.ts:61,68`）。
- **Web 端（`apps/stage-web`）的聊天是 absolute 悬浮层，不是固定侧栏**：`h 85dvh` / `right 1rem` / **`max-width 500px`** / `min-width 30%`（`apps/stage-web/src/pages/index.vue:236`）。→ 这三条数值是可直接抄的“桌面悬浮聊天面板几何”。
- **聊天是独立窗口**，不是浮层：`pages/chat.vue` 走 Electron 独立 BrowserWindow；截图 `manual-chat-window.avif` 显示它有自己的原生标题栏（File/Edit/View/Window 菜单），输入框是**浅粉紫圆角块**（颜色来自当前主题色相）。
- **字幕是独立悬浮窗**：`windows/caption/` 专门一个窗口，托盘里可 “打开字幕 / 字幕浮窗 / 跟随窗口 / 重置位置”（`tray.yaml`）。字幕窗宽度按屏幕分辨率分档：720p 取屏宽 0.9 且 clamp 到 `[280,560]`，1080p 取 0.5 且 clamp 到 `[320,640]`（`windows/caption/index.ts:76-84`）。
- **共 14 类窗口**：`main / inlay / editor / devtools / beat-sync / chat / onboarding / notice / dashboard / about / caption / desktop-overlay / spotlight / widgets / settings`（`apps/stage-tamagotchi/src/main/windows/` 目录列举）。这是“1 窗 = 1 职责”的极致版本。
- **移动端断点**：`stage-layouts` 为移动端单独准备了 `MobileHeader.vue`、`MobileInteractiveArea.vue`、`mobile-settings-drawer.vue`（`packages/stage-layouts/src/components/Layouts/`），说明它接受“同一功能两套布局”，但不是靠响应式魔法，而是**显式两套组件**。

### 2.2 视觉语言：单一色相驱动的完整令牌链

**（a）颜色 = 一个标量（而且是 OKLCH 色相角，不是 HSL）。**

```ts
// packages/stage-ui/src/stores/settings/theme.ts:2,5,7-8
import { converter } from 'culori'
export const DEFAULT_THEME_COLORS_HUE = 220.44
const convert = converter('oklch')
const getHueFrom = (color?: string) => color ? convert(color)?.h : DEFAULT_THEME_COLORS_HUE
```
```ts
// uno.config.ts:158-164
presetChromatic({ baseHue: 220.44, colors: { primary: 0, complementary: 180 } })
```
（`constants/theme.ts:1` 另有同值的 `chromaticHueDefault = 220.44`。）

- 设置项落盘为 `settings/theme/colors/hue` 与 `settings/theme/colors/hue-dynamic` 两个 localStorage 键（`theme.ts:11-12`）。
- **派生链（仓库内可读的最完整算法证据）**：`packages/ui/src/fallback.css:1-15` 保留了一份手写 fallback，给出 **chroma 生成公式与 11 档系数**，且它经 `packages/ui/src/main.css:5` + 各 app 的 `styles/main.css` 全局导入 → **这组变量在应用里始终存在**：

```css
--chromatic-hue: 220.44;
--chromatic-chroma: calc(0.18 + (cos(var(--chromatic-hue) * 3.14159265 / 180) * 0.04));
/* 分档系数 */
50:×0.3  100:×0.5  200:×0.6  300:×0.75  400:×0.85
500:var(--chromatic-chroma)  600:×1.15  700:×1.1  800:×0.85  900:×0.7  950:×0.5
```

- **语义色如何进渲染**（`apps/stage-web/src/App.vue:80-100`）：

```ts
primary   = `color-mix(in srgb, oklch(95% var(--chromatic-chroma-900) calc(var(--chromatic-hue) + 0))   {dark:70%, light:90%}, oklch(50%|90% 0 360))`
secondary = 同上但色相 +180
tertiary  = 同上但色相 +60
colors    = [primary, secondary, tertiary, isDark ? '#121212' : '#FFFFFF']
```

  即：**明度硬编码 95%，深浅只由 `color-mix` 的百分比（暗 70% / 亮 90%）控制**；色相 `0/180` 与 `uno.config.ts:157-160` 的 `primary:0`/`complementary:180` 一致，`+60` 是页面转场附加色。
- **色相可动画**：`App.vue:199-218` 注册 `@property --chromatic-hue { syntax:'<number>' }` + `@keyframes hue-anim { from:0 to:360 }` + `.dynamic-hue { animation: hue-anim 10s linear infinite }`（`App.vue:112-114` 在 `watch(themeColorsHue)` 里 `setProperty('--chromatic-hue')`，`immediate:true`）。
  → **「动态主题色」= 色相 10 秒转一圈的无缝轮转**（不是别的）。
- **⚠️ `primary-50..950` 的确切 hex/oklch：未核到。** `presetChromatic` 的实现体是外部 npm 包 `@proj-airi/unocss-preset-chromatic@1.1.4`，仓库已剔除 `node_modules`，`pnpm-lock.yaml:10569` 只有空对象 `{}`，`docs/uno.config.ts:146` 引用的 `dist/*.d.mts` 不在磁盘。上文 `fallback.css` 的公式是**本地能读到的最接近算法的证据，但它是 fallback，不等于 preset 最终输出**（相关变量名与出现次数已核实：`--chromatic-hue` 51 次 / `--chromatic-chroma-900` 19 次 / `--chromatic-chroma` 16 次 / `--chromatic-bri` 与 `--chromatic-sat` 各 1 次，后者在 `workbench.vue:108` **只被读取、从未定义**）。
- **动态取色**：`themeColorsHueDynamic` 开启时，色相从**背景图采样**得到（`theme-color.ts:sampleBackgroundColor` 用 `html2canvas` 采样背景顶部 140px 区域，`sampleHeight:20, sampleStride:10, scale:0.5`），或对 wave 背景取 `.widgets.top-widgets .colored-area` 的实际 `background-color`（`theme-color.ts:97-101`）。
- 用户可见的 “配色方案” 页提供 **6 套预设**（`packages/stage-pages/src/pages/settings/system/color-presets.json`）：`default`（`color-1: null`，即不覆盖）、`morandi`、`monet`、`japanese`、`nordic`、`chinese`，每套 **8 个 hex**。例：`morandi` = `#A5978B / #D8CAAF / #B8B4A7 / #C4BCB1 / #E5DED8 / #9A8F7D / #BEB5A7 / #C9C0B6`；`chinese` = `#E4C6D0 / #A61B29 / #5D513C / #789262 / #1C0D1A / #F7C242 / #62A9DD / #8C4B3C`。
  - ⚠️ **重要修正**：`color-presets.json` 是**纯展示色卡，不参与主题计算**（它只提供 `color-N` 的 hex，用于设置页渲染色块）；真正生效的是 `themeColorsHue` 这个标量。并且**6 套预设目前只读不可点**（属未完成交互）。
- **色相会渗透到 UI 装饰上**：header 的 AIRI logo 用 `filter: hue-rotate(calc(var(--chromatic-hue,0) * 1deg))` 跟随主题（`HeaderLink.vue:29-33`）；连 `meta[name=theme-color]` 都在跟随背景采样值更新，wave 背景每 250ms 刷新一次（`theme-color.ts:104-110`）。路由加载条 NProgress 也用同一套 `color-mix(in srgb, oklch(95% var(--chromatic-chroma-900) …) 70%, oklch(50% 0 360))`（`main.css:45`）。
  - **⚠️ 一处可直接核到的 CSS bug**：`main.css:56` 把暗色 NProgress 条写成 `` background: `color-mix(...)`; ``（**反引号模板字符串**），在纯 CSS 里非法 → `.dark #nprogress .bar` **实际不生效**（对比同文件 `:45` 的正确写法）。

**（b）字体：一个“主体 + 一个可爱体 + 一堆可换装字体”。**

`uno.config.ts:207-214` 定义 5 条字体栈，全部走 `@unocss/preset-web-fonts`，provider 可切 `fontsource` / `none`（本地自托管，离线可用）：

| token | 值 |
| --- | --- |
| `sans` | `"DM Sans Variant", "DM Sans", ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", …` |
| `sans-rounded` | `"Comfortaa Variable", "Comfortaa", "DM Sans", …` |
| `cute` / `cuteen` / `cutejp` | `"Nunito Variable","Nunito","ChillRoundM","Kiwi Maru","Comfortaa Variable", …` |

- 桌面壳的根元素**强制用 `cute` 栈**：`<main h-full font-cute>`（`packages/stage-layouts/src/layouts/stage.vue:6`）。
- 另有 5 个独立字体包作为 workspace 依赖：`font-chillroundm` / `font-cjkfonts-allseto` / `font-departure-mono` / `font-xiaolai`（`packages/` 目录）——**CJK 是被当作一等公民处理的**，有中文专用字体包与 `quanlai: 'cjkfonts AllSeto'`、`xiaolai: 'Xiaolai SC'` 两个 `provider:'none'`（本地）字体（`uno.config.ts:135-144`）。

**（c）圆角 / 玻璃 / 阴影。**

聊天容器是**这一套语言的浓缩样本**：

```html
<!-- packages/stage-layouts/src/components/Widgets/ChatContainer.vue:2-7 -->
<div
  flex="~ col"
  border="solid 4 primary-200/20 dark:primary-400/20"
  h-full w-full rounded-xl
  bg="primary-50/50 dark:primary-950/70" backdrop-blur-md
>
```

即：**4px 的半透明强调色描边 + `rounded-xl` + 50%~70% 半透明底色 + `backdrop-blur-md`**。这是可直接抄的“毛玻璃卡片”配方，而且描边用的是 `primary` 的**浅档**（`primary-200/20`），不是灰色——所以整卡会随主题变色的同时保持低对比。

**（d）背景策略与 `--bg-color`。**

```css
/* apps/stage-tamagotchi/src/renderer/styles/main.css:14-27 */
html { --bg-color-light: rgb(255 255 255); --bg-color-dark: rgb(18 18 18);
       --progress-bar-color: rgb(244 114 182); --bg-color: var(--bg-color-light); }
html.dark { --bg-color: var(--bg-color-dark); }
html.dark { color-scheme: dark; }
```

- **深色底 `rgb(18 18 18)` / 浅色底 `rgb(255 255 255)`**；进度条用粉色 `rgb(244 114 182)`（**第三个强调色**，且不在色相体系里 —— 见 §2.6）。
- `settings` 布局还动态改 `meta[theme-color]` 为 `dark:'rgb(18 18 18)'` / `light:'rgb(255 255 255)'`（`layouts/settings.vue:70`）——**桌面壳会跟随主题改窗口标题栏色**。
- 安全区：所有布局都把 `env(safe-area-inset-*)` 写进内联 style（`layouts/default.vue:9-16`、`home.vue:9-16`、`settings.vue:88-95`）。
- 系统页 `settings.vue:110-116` 有 **`2xl:max-w-screen-2xl`** 的居中上限 + `mx-auto`，即设置内容在大屏不会无限拉宽。

**（e）动效令牌：命名化 + 统一缓动。**

`uno.config.ts:225-283` 定义了一整套**命名动画**，并给每个都指定了时长与缓动：

| 动画 | 时长 | 缓动 | 用途（推断自命名） |
| --- | --- | --- | --- |
| `overlayShow` / `overlayHide` | 300ms | `cubic-bezier(0.16, 1, 0.3, 1)` | 遮罩淡入淡出 |
| `contentShow` / `contentHide` | 150ms | 同上 | 弹层 `scale(.96)→1` + 位移 |
| `slideUpAndFade` / `slideRightAndFade` / `slideDownAndFade` / `slideLeftAndFade` | 400ms | 同上 | 四向滑入（2px 位移 + 淡入） |
| `fadeIn` / `fadeOut` | 200ms | `ease-in-out` | 通用淡变 |

> 缓动曲线 `cubic-bezier(0.16, 1, 0.3, 1)` 是 **Radix/shadcn 系的“出场曲线”**（快出慢收）。AIRI 在 `uno.config.ts:170-176` 的注释里明确引用了 `hyoban/unocss-preset-shadcn`。

另有一个**页面级过场**：`.slide-away-*` = `transform 0.15s ease-in-out, opacity 0.15s ease-in-out`，位移 ±10px（`apps/stage-tamagotchi/src/renderer/styles/transitions.css:1-18`）。

- **加载态**：聊天容器顶部有一条 **2px 扫描条**（`h-1` 容器 + `w-1/3` 的 `bg-primary-500` 子元素 + 自定义 `@keyframes scan` 2s linear infinite，从 `translateX(-100%)` 到 `translateX(400%)`），只用 `bg-primary-500/20` 做底（`stage-layouts/.../InteractiveArea.vue:62-68, 93-103`）。→ **这是“正在加载历史消息”的轻量可视化，值得直接抄。**
- **页面转场是可关的**：设置页路由有 `meta.stageTransition.name: slide`，并提供 `settings.usePageSpecificTransitions` 与 `disableTransitions` 两个开关（`settings/index.vue:23-34`；i18n `settings.yaml:5,9`：`是否开启舞台切换动画` / `是否使用页面特定过场动画`）。→ **无障碍/性能优先的显式选项**。

### 2.3 信息架构：设置 = 卡片菜单 + 8 个分区，且每项都有一句人话说明

**（a）首页是“图标卡片菜单”，不是侧栏。**

```html
<!-- packages/stage-pages/src/pages/settings/index.vue:41-56 -->
<RippleGrid :items="settings" :get-key="item => item.to" :columns="1" :origin-index="lastClickedIndex" …>
  <template #item="{ item }">
    <IconItem :title="item.title || ''" :description="item.description" :icon="item.icon" :to="item.to" />
  </template>
</RippleGrid>
```

- 条目由**路由 meta 自动派生**并按 `meta.order` 排序（`settings/index.vue:29-40`）。这是一个 **IA 单点真相** 的好模式：新增设置页 = 新增一个带 meta 的路由文件。
- 官方截图 `manual-settings-window.avif` 显示实际观感：**整宽卡片，左侧标题（16px 级）+ 一行说明（次要色），右侧一枚巨大的、极低对比度的图标水印**（约 60–80px），卡片间距约 12px。**图标水印不是装饰，是分类记忆点。**

**（b）8 个分区（按 `meta.order`，含原始 order 值）**

| order | 路由 | 中文标题 | 说明原文 | 图标（Solar 图标集） |
| --- | --- | --- | --- | --- |
| 1 | `settings/airi-card` | AIRI 角色卡 | 使用 AIRI 角色卡预设 | `i-solar:emoji-funny-square-bold-duotone` |
| 2 | `settings/modules` | 机体模块 | 思维，视觉，言语综合，游戏等 | `i-solar:layers-bold-duotone` |
| 3 | `settings/scene` | 场景 | 配置角色所在环境 | `i-solar:armchair-2-bold-duotone` |
| 4 | `settings/models` | 角色模型 | 切换角色的 Live2D、VRM 模型 | `i-solar:people-nearby-bold-duotone` |
| 5 | `settings/memory` | 记忆体 | 存放记忆的地方，以及策略 | `i-solar:leaf-bold-duotone` |
| 6 | `settings/providers` | 服务来源 | LLM，语音合成，语音识别服务来源等 | `i-solar:box-minimalistic-bold-duotone` |
| 7 | `settings/data` | Data | 管理存储 AIRI 数据、导出和重置 | `i-solar:database-bold-duotone` |
| 8 | `settings/connection` | 连接 | 配置 WebSocket 服务器连接 | `i-solar:wi-fi-router-bold-duotone` |

来源：各页 `<route lang="yaml">` 的 `meta` 块（如 `scene/index.vue:227-239`、`connection/index.vue:9-20`）+ `packages/i18n/src/locales/zh-Hans/settings.yaml:462-467, 517-519, 529, 1495-1497, 1688-1690`。

**（c）两层信息架构的细节**

- “机体模块”下挂 14 个子页：`artistry / beat-sync / consciousness / gaming-factorio / gaming-minecraft / hearing / mcp / memory-long-term / memory-short-term / messaging-discord / speech / vision / web-search / x`（`packages/stage-pages/src/pages/settings/modules/` 目录）。
- “服务来源”下按**能力**分 tab：Chat / Vision / Speech / Transcription / Artistry（官方截图 `image-8.png`），并在列表上方给出 **3 个筛选维度**：`价格：全部/免费/付费`、`部署：全部/本地/云端`（截图可见），卡片上是 provider 名 + 一行解释 + 徽章（`推荐` / `付费` / `云端`）。
- **系统页有 4 个子页**（`general` / `color-scheme` / `窗口快捷方式` / `开发者`），且 `general`、`color-scheme` 的 route meta **没有 `settingsEntry: true`**（`system/general.vue:9-16`、`system/color-scheme.vue:227-234`）→ 它们不能从设置首页直达，只能从系统页进。这是**刻意的层级隔离**。
- 分层导航用 `PageHeader` 统一渲染标题 + 副标题：**副标题恒为 `settings.title`（即“设置”）**，主标题是当前页（`settings.yaml` 中每页 `subtitleKey: settings.title`；渲染见 `layouts/settings.vue:41-48, 118-122`）。→ 截图里就是“设置 / AIRI 角色卡”上下两行 + 一个返回箭头。

**（d）角色卡页的列表模式**（官方截图 `manual-airi-card.avif`）：搜索框（`搜索角色卡…`）+ `排序方式：名称 (A-Z)` 下拉 → 虚线**上传拖放区**（图标 + “上传” + “点击或拖拽文件到此处上传”）→ **“+ 创建新角色卡”** 同尺寸卡片 → 角色卡列表（标题 + 铅笔 + 选中勾，正文是截断的人设文本，底部一行 `v1.0.0` + 两个 `default` 徽章 + 右下角圆形主色勾）。

### 2.4 关键交互

- **模型切换**：`settings/models` 页负责（`title: 角色模型` / `description: 切换角色的 Live2D、VRM、Spine 模型`，`settings.yaml:517-519`）。i18n 里能读到它包含 `模型选择器`（`zh settings.yaml:114`）、`导入前检查`（`:121`，其下再分 `模型 / 资源 / 问题`，`:126,133,142`）、`更换模型`（`:159`）、`编辑动作映射`（`:161`）、`映射动作`（`:164`）。→ **导入前有一次显式校验分诊**，这是“用户导入模型”类产品的好模式。
- **Live2D 专属设置**（`settings.yaml:165-194`）：`Live2D 设置` 下含 `缩放与位置`、`想切换至3D虚拟形象？`、`从模型提取主题颜色`（带一个 `提取` 按钮，`:179-181`）、`动画` → `动作驱动器` → `通用 / MAGIC / MAGIC 配置`。→ **“从模型提取主题颜色”是一个把角色和 UI 绑定的产品级动作**，不是普通设置项。
- **状态呈现**：主窗控制岛里有独立按钮（屏听/扬声器），状态不是常驻文字。聊天区顶部有可切换的 `PageHeader` 与 streaming 指示。
- **停止朗读**：TTS 播放中时输入区出现**“停止朗读”按钮**，且**只停当前语音、不取消已生成的文字回复**（`setup-and-use/index.md:188` 附近的说明；实现见 `stage-layouts/src/composables/useStopSpeakingButton.ts`）。→ 这是“语音与文本解耦”的正确姿势。
- **快捷操作浮窗（Spotlight）**：托盘 `打开快速操作...` → 一个浮动输入框，输入短请求回车后**窗口自动隐藏、结果以系统通知呈现**，Esc 取消（`tray.yaml` + `setup-and-use/index.md`）。→ 一个不打断桌面的“快速指令”通道。
- **小部件窗（Widgets）**：地图 / 天气 / 艺术创作 / 扩展小部件集中在一个窗口里（`tray.yaml: open_widgets`）。

### 2.5 动效

- **面板/弹层**：一律走 §2.2(e) 的命名动画，缓动统一 `cubic-bezier(0.16,1,0.3,1)`。
  - ⚠️ 但**新聊天消息用的不是上面任何一条具名动画**：`history-message-frame.vue:46-53` 是 `'opacity-0 transition-opacity duration-200 ease-out motion-reduce:transition-none'` + `isVisible ? 'opacity-100'`，由 `useElementVisibility`（`:26-29`）触发。→ **纯透明度、200ms、`ease-out`、无位移/缩放**；【推断】因为消息列表用了虚拟滚动（`virtua/vue` 的 `<Virtualizer>`，`history.vue:197-203`），位移动画在虚拟滚动时会抖。相关常量：`CHAT_HISTORY_OVERSCAN = 600`、顶部渐隐 `topFadeRatio = variant==='mobile' ? 0.2 : 0`（`history.vue:54-55,109`）、滚动条淡入 150ms ease-out / 淡出 180ms ease-in（带 `prefers-reduced-motion` 兜底）。
  - 滑动回复：`opacity = swipe.progress`，图标 `translate3d(-min(offset*0.18,10)px, -50%, 0) scale(0.72 + progress*0.28)`，达阈值 `triggerHaptic('medium')`（`history-message-frame.vue:33-42,60`）。
- **可折叠/可拖拽的输入区**：`stage-layouts/src/components/AdaptiveInput/` 是一整套 `area / root / viewport / context` 组件 + `adaptive-input-geometry.ts`（且有单测 `adaptive-input-geometry.test.ts`）→ **自适应输入框的几何计算是被单元测试覆盖的**。
- **页面级**：`.slide-away`（±10px + opacity，150ms，`transitions.css`）。
- **主题切换**：`html { transition: all 0.3s ease-in-out }`（`styles/main.css:12-14`）→ 换主题/换色时整页颜色平滑过渡。**这是一个极低成本的高级感细节。**
- **状态切换**：麦克风音量指示器单独一个组件（`IndicatorMicVolume.vue`），说明“聆听”有**实时电平反馈**而不只是图标变色。
- **移动端设置抽屉用 `vaul-vue`**：AIRI 只覆写 `DrawerOverlay 'fixed inset-0 z-[9999] bg-black/35'`、`DrawerContent '… max-h-[90dvh] rounded-t-[32px] shadow-xl pb-[max(1rem,env(safe-area-inset-bottom))]'` + `motion-reduce:animate-none`；抽屉**串接**靠 `nextPanel` + `@after-close` + `@close-auto-focus`（先等前一个关完再开下一个，避免焦点/滚动锁双主）。→ **`vaul-vue` 内部时长/缓动：未核到**（源码不在本地）。
- **鼠标交互**：`presetIcons({ scale: 1.2 })`（`uno.config.ts:147`）→ 图标默认放大 20%。

### 2.6 明确的设计缺陷

1. **第三个强调色游离在色相体系之外**：进度条 `--progress-bar-color: rgb(244 114 182)`（粉）写死在 `styles/main.css:15-24`，**不参与 `--chromatic-hue` 色相旋转**，而 logo 会跟着转（`HeaderLink.vue:29-33`）。→ 【推断】主题换成绿色时，进度条仍然是粉的。
2. **`settings` 页的 8 项排序号在 8 个文件里各写一遍**（`order: 1..8` 分散在 `scene/`、`providers/`、`connection/`、`memory/`、`models/`、`data/`、`modules/`、`airi-card/` 各自的 route meta）。改一次顺序要动 8 个文件，且**重号不会报错**（`settings/index.vue:33` 只做 `Number(a.meta?.order ?? 0) - Number(b.meta?.order ?? 0)`）。→ 这是“IA 单点真相”的一个反例：它只在**读取侧**是单点，**写入侧**是分散的。
3. **`Data` 页 title 是英文**（`settings.yaml:466`：`title: Data`，`description: 管理存储 AIRI 数据、导出和重置`）—— 在一个全中文界面里单独一个英文词，截图 `image-14.png` 已复现。同类还有截图里的 `v1.0.0`、`default` 徽章、`排序方式` 右侧的 `名称 (A-Z)`。
4. **服务来源页信息密度过高**：一页里同时有 5 个能力 tab + 2 组筛选 chip + provider 卡片列表（官方截图 `image-8.png`）；`providers/index.vue` 本身 **326 行**，i18n 的 `settings.providers.*` 从 `settings.yaml:954` 一直延续到 `:1494`（**约 540 行文案**）。→ 【推断】这是最难维护的一页。
5. **浅色主题下卡片对比度过低**：官方截图 `manual-settings-window.avif` 中，浅灰背景上的卡片几乎与背景同色，图标水印与说明文字对比度都很低；这是**浅色主题被当作二等公民**的迹象（深色截图 `image-14.png` 的层次明显更好）。
6. **桌面端设置窗残留原生标题栏**：截图 `image-10.png`/`image-14.png` 顶部仍是 mac 的“红黄绿 + Settings”原生标题栏，而主窗是无边框透明窗 → 两套窗口装饰语言并存。
7. **开发者页承担了不该承担的产品设置**：`是否开启舞台切换动画`、`是否使用页面特定过场动画` 出现在“开发者”页（官方截图 `image-10.png`），而 i18n 里 `settings.yaml:5,9` 确认这两个 key 就在通用设置下——**“动效开关”被归类到开发者页**，普通用户找不到。
8. **字体族在同一产品内不统一**：舞台根是 `font-cute`（`stage.vue:6`）且两侧气泡都是 `font-cute`（`user-item.vue:63`、`assistant-item.vue:95`），但**移动端控件岛是 `font-sans`**（`MobileInteractiveArea.vue:130`）→ 控件与消息不同族。
9. **`font-mono` 用了 45 次却从未在 `theme.fontFamily` 声明** → 落入未定义工具类的静默失效（与 §7.4 Amica 的缺陷同族）。
10. **具名动画定义了却从未使用**：`overlayShow/overlayHide` 等 **0 命中**；`--chromatic-bri`/`--chromatic-sat` 在 `workbench.vue:108` 被读取却**从未定义**。
11. **`cute` / `cuteen` / `cutejp` 三个字体键值完全相同**（`uno.config.ts:207-214`）→ 键名承诺了差异，实现没有。
12. **另有一条坏 CSS**：`main.css:56` 用反引号模板字符串写 `background: \`color-mix(...)\``，在纯 CSS 里非法 → 暗色 NProgress 条不生效（§2.2a）。
13. **半成品路由**：`/v2/settings` 可路由但不可达；模块分类 `categorizedModules` 是死代码。→ 说明这个项目的 IA 存在**未清理的历史分支**（与 Nexus 的 V1/V2 双 IA 同病，§7.5.6）。

### 2.7 可迁移的具体设计决策

1. **因为** AIRI 用**一个 OKLCH 色相标量**（默认 `220.44`）驱动全部品牌色，且支持从背景图/模型采样自动提取（`theme.ts:8,11`、`uno.config.ts:158-164`），**所以我们应该**：Flutter 侧定义 `AppHue` 单值（默认取一个基准色），语义色全部由它派生；并预留“从当前皮套立绘采样色相”的入口。
2. **因为** AIRI 的玻璃卡片配方是固定的四件套「**4px 半透明强调色描边 + rounded-xl + 50–70% 半透明底 + backdrop-blur**」（`ChatContainer.vue:2-7`），**所以我们应该**：把它做成一个 `GlassPanel` 组件，**描边用派生色的浅档而不是灰色**，这样换主题时卡片整体协调。
3. **因为** AIRI 的设置首页是**自动从路由 meta 生成的卡片菜单**，每项强制带 `title + description + icon`（`settings/index.vue:29-56`），**所以我们应该**：设置分区用一个**声明式清单**驱动（而不是手写导航），并对“缺少说明文案”做编译期检查。
4. **因为** AIRI 把**加载态**做成聊天卡顶部的 2px 扫描条（`InteractiveArea.vue:62-68, 93-103`），**所以我们应该**：所有“异步拉取”的容器都复用同一扫描条组件，而不是放 spinner 居中。
5. **因为** AIRI 在 `html` 上挂了 `transition: all 0.3s ease-in-out`（`styles/main.css:12-14`），换主题时整页平滑过渡，**所以我们应该**：主题切换必须有全页过渡，且过渡时长进令牌表。
6. **因为** AIRI 把“页面转场”做成**两个用户可关的开关**（`settings.yaml:5,9`），**所以我们应该**：所有非必要动效都要有一个统一的“减少动效”总开关，且默认尊重系统 `prefers-reduced-motion`。
7. **因为** AIRI 的窗口尺寸会被**持久化 + 托盘里提供“推荐 / 全高 / 半高 / 全屏 + 5 个对齐位”**（`windows/main/index.ts:188-224`、`tray.yaml`），**所以我们应该**：桌宠窗提供**一组尺寸/位置预设**而不是让用户手动拖到合适——这比自由缩放更符合“陪伴”定位。
8. **因为** AIRI 用 `focusable:false` 类窗口承载字幕，并让字幕窗可选择“跟随主窗 / 独立位置 / 重置位置”（`tray.yaml`），**所以我们应该**：字幕浮窗的定位策略做成 3 个显式枚举，并把“跟随”做成默认。

---

## 3. N.E.K.O（Project-N-E-K-O/N.E.K.O）—— 视觉语言取自官方商店页截图 + 本仓 CSS

> 规模：⭐ **2,823**，`pushed_at` 2026-09-10，Python，**Apache-2.0**，已在 Steam 发行（`store.steampowered.com/app/4099310`）。
> 一手证据：Steam 官方商店截图（经 Steam `appdetails` API 取得 `screenshots[].path_full` 后下载，见 §0.1 方法）+ 仓库内 `static/css/*.css`（9,036 行）。

### 3.1 视觉语言：青蓝玻璃 + 大圆角 + 白色描边（这是“萌系 AI 桌宠”最成熟的现成配方）

**（a）官方截图给出的配方（`ss1` / `ss3`）**

- **聊天面板**：位于屏幕**左侧**，占宽约 35–40%，是**半透明浅青白毛玻璃**（背景景色透出但被明显虚化），**大圆角**（约 16–20px），**1px 近白描边 + 极轻外阴影**。
- 面板头部：一个 `✦` 星形图标 + 标题 `对话`，右侧依次是 `👥`、`⤴`、**青色圆形主操作按钮**、`×`。
- 消息：**圆角大气泡**，助手气泡是半透明白，用户气泡是更实的白；**气泡内没有头像**，头像只在消息行左侧；行内带**小字号时间戳**（`YUI 12:24:14`）。
- **推荐回复**：A/B/C 三条**圆角胶囊按钮**（浅青底 + 深青字），直接列在输入框上方（`ss1`/`ss3` 均可见）。
- 输入框：**居中占位文字** `回车发送，Shift+回车换行`，下方一排**圆角方块工具栏**（约 6 个图标 + 一个**紫色圆形发送按钮**），其中一个图标带**绿色未读角标 `15`**。
- **内嵌播放器卡片**：聊天流中间可以插一张音乐播放卡（左侧青色 `MA` 封面块、曲名 `easy hiphop`、进度条、`00:36 / 01:59`、来源 `VibeDeport`、三个圆形控制键）。→ **“富消息卡片”是这个产品的一等公民**，不是纯文本流。
- **副窗（消息 2/3）**：字幕浮窗是一块**青蓝实色（近 `#40c5f1`）圆角卡**，白字居中，右上角一个齿轮；下方紧跟一块**设置小卡**（`目标语言：日本語` 下拉、`不透明度 100%` 滑杆、`整体拖动` 开关、`大小：小/中/大` 三选一，选中项为青色实底）。
- **插件管理窗（`ss2`）**：**独立 OS 窗口**，浅色主题，顶部一条**青色渐变标题栏**（带斜向玻璃反光），左侧 `w-56` 级侧栏（Logo + 5 项：`仪表盘 / 插件管理 / 运行记录 / 服务器日志` + 分组标题 `适配器` → `MCP 适配器`），主区是**面包屑 + 图标按钮组 + 居中分段 Tab（`插件(12) / 适配器(1) / 扩展(0)`）+ 右侧视图切换（`列表 / 单排 / 双排 / 紧凑`）+ 卡片网格**。卡片内容：标题 + 状态点 + `已停止` + `手动启动`、一行描述、底部 `v1.0.0` + `入口点: 42`。
- **模型/表情配置页（`ss4`）**：一列**青色胶囊按钮**，每个带圆形图标：`🗂 导入`、`Live2D`、`yui-origin`、`neutral3`（带 ▶）、`无表情`（带 ▶）、`选择常驻表情`；下方一个**选择列表**（`001 / by / bzy / sbx / slb / swz / syh8`）。**这是本次调研里唯一直接看到 Live2D 表情/动作选择 UI 的样本。**

**（b）CSS 侧的量化事实【已验证】**

| 指标 | 值 | 证据 |
| --- | --- | --- |
| `static/css/*.css` 总行数 | **9,036** | `wc -l index.css dark-mode.css subtitle.css avatar-reaction-bubble.css` = 4597/2903/1033/503 |
| 定义的自定义属性总数 | **293** | 对 `static/css/*.css` grep `^\s*--[a-z0-9-]+:` 去重 |
| **颜色类**自定义属性 | **0**（293 个变量里没有一个是颜色） | 同上，逐条检视全部为 `--neko-*` 布局/几何/窗口控件类 |
| 最频繁 hex | `#fff`(175) / **`#40c5f1`(171)** / `#ffffff`(114) / `#e0e0e0`(67) / `#b3e5fc`(61) / `#96e8ff`(55) / `#4aa3df`(49) | 全量 hex 频次统计 |
| **第二个蓝** | `#22b3ff`(39) 与 `#44b7fe`(39) 同时存在 | 同上 |
| `backdrop-filter` 使用文件数 | **15 个文件**，其中 `index.css` **38 处**、`model_manager.css` 23 处 | `grep -c backdrop-filter` |
| 根元素透明与穿透 | `html, body { background: transparent !important; pointer-events: none; }`（注释：`允许鼠标事件穿透`） | `index.css:12-25` |
| 模型过渡令牌 | `--neko-model-opacity-transition: opacity 280ms ease-in`；`--neko-model-transform-transition: transform 400ms cubic-bezier(0.22,1,0.36,1)`；`--neko-model-visibility-delay: 400ms` | `index.css:6-10` |

- 有一条**明确的“暗色模式单独文件”策略**：`dark-mode.css`（2,903 行，占全部 CSS 的 1/3）。→ 【推断】深色不是通过变量切换，而是**另写一份覆盖表**；这与它“无颜色令牌”的事实一致。
- `#status-toast` 用的是**贴图背景**而不是 CSS 圆角卡：`background-image: url('/static/icons/toast_background.png'); background-size: 100% 100%;` + `padding-left: 50px`（为左上角猫爪留位）+ 白字 + `text-shadow: 0 1px 3px rgba(0,0,0,0.35)`（`index.css:30-66`）。→ 【推断】“贴图当卡片”是它视觉风格的一部分（换取更自由的造型），代价是**尺寸/深色模式/多语言都要重新出图**。

**（c）全仓唯一“令牌化彻底”的模块是字幕 —— 这是可以直接抄的样板。**

`static/css/subtitle.css` 是 26 个 CSS 文件里唯一做到彻底变量化的：**116 个 `--subtitle-*` 变量** + **`[data-subtitle-color-scheme="red|orange|…"]` 属性选择器配色档（7 套）**【已验证 `subtitle.css:13-42, 127-176`】。配套事实：

| 事实 | 值 | 证据 |
| --- | --- | --- |
| 字幕字号（离散档位，非比例） | **16 / 21 / 26 / 34 / 44 px**，默认 26 | `templates/subtitle.html:55-59` |
| 字幕面板内边距 | 用 `clamp()` 跟自身宽高联动 → **全仓唯一“比例化”间距** | `subtitle.css:40-42` |
| 字幕面板显隐曲线 | `0.4s cubic-bezier(0.34,1.56,0.64,1)`（**过冲/弹性**） | `subtitle.css:249` |
| 主面板折叠 / 展开 | `transform 0.35s cubic-bezier(0.4,0,0.2,1)` / `0.32s cubic-bezier(0.2,0,0,1)` | `index.css:1263,1270` |
| 尺寸过渡 | `width/height/max-height/padding .3s ease, box-shadow .2s, transform .32s ease` | `index.css:1258` |

**（d）三套并行的强调色 + 一个“贴图气泡”。**

- 管理页共用青 `#40C5F1`，共 **171 次**（`character_card_manager` 66 / `voice_clone` 28 / `card_maker` 24 / `api_key_settings` 24），**但在 `index.css` 里 0 次**；`index.css` 自身最高频是蓝 `#44b7fe`（16 次）；弹窗 token `--neko-popup-accent: #2a7bc4`（`dark-mode.css:65`）；plugin-manager 主色是 Element **`#409EFF`**（`variables.css:7`）；焦点环固定 `#40C5F1`（`base.css:16`）。**全仓 636 个不同 hex / 2,345 次出现。**
- 表情气泡的外壳也是**贴图**：`--bubble-shell-image: url('/static/icons/chat_bubble.png')`（`avatar-reaction-bubble.css:3`）。
- **三种文本面并存**：① 页内字幕条 `#subtitle-display`（`templates/index.html:152`）；② **独立 `/subtitle` 窗口**，可切**弹幕模式**（有独立 lane 层与边缘渐隐，`subtitle.css:336-346`）；③ 表情气泡 `#avatar-reaction-bubble`（贴图壳）。

### 3.2 关键交互（从截图可直接读出）

- **模型切换**：`ss4` 的下拉式胶囊清单 + **每个动作/表情右侧一个 ▶ 试播按钮**（`neutral3 ▶`、`无表情 ▶`）→ **可试播是标配**（对比 Live2DPet 的“只有列表、不能试播”，见 §7.4）。
- **常驻表情**：单独的 `选择常驻表情` 入口 + 一个多选列表（`001 / by / bzy / …`）。→ **“待机表情”是一个独立概念**，与“对话中触发的表情”分开管理。
- **字幕浮窗自带设置**：语言 / 不透明度 / 跟随拖动 / 大小三档，直接贴在字幕卡下方（`ss3`）。→ **字幕的调节项就近放在字幕上**，而不是埋进设置里。
- **推荐回复**：A/B/C 胶囊按钮直接可点（`ss1`/`ss3`）。
- **多窗口**：主舞台（角色 + 聊天浮层 + 字幕）与“插件管理”是两个独立窗口（`ss1/ss3` vs `ss2`），后者是标准的“设置型窗口”。

### 3.3 信息架构：**没有全站侧栏/标签**，导航是「桌宠浮动按钮 → popup 层级菜单」

- **浮动按钮列**：**48px × 5 个，gap 12**（`static/live2d/live2d-ui-buttons.js:22-24`：`LIVE2D_FLOATING_BUTTON_SIZE = 48` / `…_GAP = 12` / `…_COUNT = 5`）；按钮文案如 `屏幕分享` / `Agent工具` / `设置`（`static/avatar/avatar-ui-buttons/methods-buttons.js:269,280,301`）。
  → **这与 VTube Studio 的左侧圆形图标 rail 是同一个模式**（§5），但 N.E.K.O 把它放在桌宠上，且按钮尺寸 48px 比 VTS 更大。
- **设置 popup 是「跳转 + 悬浮侧面板」的层级弹层**：`通用设置`→`/character_card_manager`、`模型管理`→`/model_manager`、`声音克隆`→`/voice_clone`，另有 `API密钥` / `声纹身份` / `记忆浏览`，以及 `角色设置` 的 **hover 侧面板**（`avatar-ui-popup-config.js:56-58` + `avatar-ui-popup.js:3133-3135`）。
- **模型切换不在主界面**：popup 只做跳转，真正切换在独立的 `/model_manager` 页（`id="live2d-model-select-btn"` / `id="mmd-model-select-btn"`）（`avatar-ui-popup-config.js:57` + `templates/model_manager.html:86-87,118-120`）。
- **几何权威值在 JS 而非 CSS**：默认宽 430 / 可拖最窄 180 / 视口内边距 16 / 对话条默认高 64（`static/app/app-react-chat-window/bootstrap-state-and-geometry.js:460,467,469,473`）→ 这是一个正例（常量集中一处），但该仓库自己也因 **host clamp 与 React 侧不同步**而告警。

### 3.4 状态反馈的缺口（三家共有，值得单独标记）

- **连接态没有常驻指示器**：承载连接文本的 `#status` 元素被 JS 强制 `display:none !important`（`static/app/model-display.js:387-389`），只剩 **2 秒后消失的 toast**。→ 网络断了用户不知道。
- 这与 my-neuro（思考态直接剥离 `<thinking>`）、Live2DPet（思考完全无 UI）构成**同一个跨项目缺陷**（§8 #3）。
- 本仓库内该缺陷的**正面样板仍是字幕模块**：`subtitle.css` 用变量 + `[data-*-scheme]` 做到可换肤（§3.1c）。

### 3.5 明确的设计缺陷

1. **两个蓝并存且频率都极高**：`#40c5f1`(171) 与 `#22b3ff`(39) / `#44b7fe`(39) / `#5cb8f0`(29) / `#4aa3df`(49) 同时散落在 9,036 行 CSS 里【已验证 hex 频次】。→ 【推断】同一个产品里至少存在 5 个近似蓝，视觉上表现为“哪里都差不多但就是不一致”。
2. **293 个变量里 0 个是颜色**：颜色无法主题化，换色要改全仓 hex。→ 与 §2.2(a) 的 AIRI 形成最强烈的对照。
3. **深色模式靠一份 2,903 行的覆盖表**，且原表 `index.css` 有 38 处 `backdrop-filter` → 【推断】维护成本随组件数线性增长，且新组件极易漏写深色分支。
4. **`pointer-events: none` 写在 `html, body` 上**（`index.css:21-22`）→ 【推断】所有可交互元素都必须**逐个**显式打开 `pointer-events: auto`，漏一个就点不动。这是一个高风险的全局默认值。
5. **状态 toast 是贴图 + 白字**，`#status-toast` 的 `border-radius: 12px` 与贴图本身的圆角是两套（`index.css:60` + `background-size:100% 100%` 拉伸）→ 【推断】贴图会被拉变形（与 Live2DPet 的 `object-fit: fill` 是同一类缺陷，见 §7.7.6.3）。
6. **连接状态被主动隐藏**（`model-display.js:387-389`），只有 2s toast（§3.4）。
7. **三（五）套并行强调色互不引用**（见 §3.1d 的完整清单）——`#40C5F1` 在管理页 171 次、在 `index.css` 0 次，是最典型的“两个子系统各长各的”。

### 3.6 可迁移的具体设计决策

1. **因为** N.E.K.O 的聊天面板是一块**左侧毛玻璃浮层 + 大圆角 + 近白描边**，角色完整地在右侧（官方截图 `ss1`），**所以我们应该**：默认形态采用“**角色满幅 + 左侧半透明对话浮层**”，并把浮层的毛玻璃强度做成可调（它在截图中明显能透出背景但保留可读性）。
2. **因为** N.E.K.O 把**富消息卡片**（音乐播放器）直接插进对话流（`ss1`），**所以我们应该**：消息模型从一开始就设计成“卡片列表”而不是字符串，避免以后加播放器/图片/工具结果时改协议。
3. **因为** N.E.K.O 的字幕浮窗**自带语言/不透明度/大小设置**（`ss3`），**所以我们应该**：字幕的样式控制就近放在字幕上（或一个悬浮小面板），不要放进设置页。
4. **因为** N.E.K.O 的模型/表情列表**每项都有 ▶ 试播**（`ss4`），**所以我们应该**：动作/表情选择器必须支持即时预览；这需要一个“从 UI 直接注入动作”的通道（属于 core 仲裁范围内的合法指令）。
5. **因为** N.E.K.O 把“**常驻/待机**表情”与“对话触发表情”分成两个入口（`ss4`），**所以我们应该**：把“idle 组”与“事件组”在模型配置里显式分成两类，而不是混在一个列表。
6. **因为** N.E.K.O 的 293 个变量里 0 个颜色、并出现 5 个近似蓝（`index.css` hex 频次），**所以我们应该**：第一件事就把颜色变量补齐——**至少要有一个“主色”变量被所有组件引用**。
7. **因为** 它的导航是「**桌宠上的 48px 浮动按钮列（5 个，gap 12）→ popup 层级菜单**」，而不是常驻侧栏（`live2d-ui-buttons.js:22-24`），**所以我们应该**：陪伴态用「浮动按钮列 → 层级 popup」这套零占屏导航；按钮命中区 ≥ 48px（比 VTS 的 rail 更适合触屏/近视操作）。
8. **因为** 它的几何常量集中在**一个 JS 模块**里（`bootstrap-state-and-geometry.js:460-473`），但**仍因 host clamp 与 React 侧不同步而告警**，**所以我们应该**：几何常量集中一处只是必要条件，还要让**渲染侧从同一处读取**（Flutter 侧即：一个常量文件 + 所有尺寸引用它，而不是 Dart 与 Rust 各写一份）。
9. **因为** `subtitle.css` 是它唯一变量化彻底的模块（**116 个 `--subtitle-*` + 7 套 `[data-subtitle-color-scheme]` 配色档**，`subtitle.css:13-42,127-176`），**所以我们应该**：**每个可换肤浮层都走「语义变量 + `[data-*-scheme]` 配色档」**，并把字幕作为第一个按此规范实现的组件（它的字号离散档 16/21/26/34/44 与 `clamp()` 内边距也可直接借用）。
10. **因为** 它把字幕做成**独立 `/subtitle` 窗口并支持弹幕模式**（独立 lane 层 + 边缘渐隐，`subtitle.css:336-346`），**所以我们应该**：字幕组件从一开始就抽象成“一个可渲染 N 行、可换 lane、可换宿主（页内/独立窗）”的组件，而不是写死在舞台里。
11. **因为** 它的连接态被隐藏（`model-display.js:387-389`）、my-neuro 与 Live2DPet 也没有思考态（§3.4），**所以我们应该**：**常驻**的连接/思考/说话状态位是硬需求（我方 §10 P0-6 已列为必须）。

---

## 4. Open-LLM-VTuber：当前上游前端（Open-LLM-VTuber-Web）—— 我们最直接的同类，但要小心抄

> ⚠️ **重要事实核正**：本仓库归档的 Python 前端（`neko-ui-alignment-and-gaps.md` 的对标对象）对应的上游是 `Open-LLM-VTuber/Open-LLM-VTuber-Web`。
> - 主仓 `Open-LLM-VTuber/Open-LLM-VTuber` 的 `frontend/` 目录**是空的**，它是一个 **git submodule**：`.gitmodules` → `url = https://github.com/Open-LLM-VTuber/Open-LLM-VTuber-Web`，`branch = build`【已验证 `.gitmodules:1-3`】。
> - 因此“上游当前前端”= `Open-LLM-VTuber-Web` 的 **`main` 分支源码**（`package.json` version `1.2.1`）；`build` 分支只有编译产物。
> - 主仓本身 `pushed_at` 停在 **2026-05-15**，README 明确 “**v2.0 Development** … early discussion and planning phase”（`README.md:14`）→ **v2.0 前端尚不存在，当前前端就是这一版**。

### 4.1 布局：舞台绝对定位铺满 + 440px 可折叠侧栏 + 120px 底栏

顶层是 `<Live2D 容器>` 与 `<UI>` 作为**兄弟节点**，靠手写绝对定位与 `zIndex` 排序（`App.tsx:94-155`）。

| 元素 | 数值 | 证据 |
| --- | --- | --- |
| 应用容器 | `100vw`；Electron 下 `height: calc(100vh - 30px)` 且 `mt: 30px`（为原生标题栏让位）；`bg gray.900`、`color white`、`overflow hidden`；`flexDirection: base=column / md=row` | `layout.tsx:12-23` |
| 侧栏 | `width: base=100% / md=440px`；`bg gray.800`；`borderRight: 1px solid whiteAlpha.200` | `layout.tsx:24-34` |
| 侧栏内壳（**第二套定位**） | `position:absolute; left:0; width:440px`；折叠 = `translateX(calc(-100% + 24px))`；`transition: all 0.3s cubic-bezier(0.4,0,0.2,1)` | `sidebar-styles.tsx:35-51` |
| 折叠把手 | 整条 **24px 竖边**可点（`width:24px; height:100%`） | `sidebar-styles.tsx:52-67` |
| 舞台 | `position:absolute; top:30px; height:calc(100% - 30px); zIndex:5; left: md = (侧栏可见 ? 440px : 24px); width: md = calc(100% - 440px|24px)` | `App.tsx:61-82` |
| 舞台（宠物模式） | `top:0; left:0; width:100vw; height:100vh; zIndex:15` | `App.tsx:85-92` |
| 底栏 | `height: base=100px / md=120px`；折叠后 `base=20px / md=24px` | `layout.tsx:53-60` |
| 字幕条 | `position:absolute; bottom: 折叠?39px:135px; left:50%; translateX(-50%); width:60%; zIndex:10` | `App.tsx:127-136` |
| 连接徽标 | `position:absolute; top:20px; left:20px; zIndex:10` | `App.tsx:124-126` |
| 宠物模式浮窗 | `bottom:120px; left:50%; translateX(-50%); zIndex:1000`；宽 `400px`；`rounded xl`；`bg blackAlpha.700`；`backdropFilter: blur(8px)` | `electron-style.tsx:4-27` |

- **响应式只有两级**：Chakra 的 `base` / `md`（`base` 纵向堆叠，`md` 左右分栏）；另有一处 **UA 嗅探** `navigator.userAgent` 正则 `/Mobi|Android/i` 直接切 `window.innerHeight`（`layout.tsx:4-6`）→ 不是响应式而是特判。
- **440px 与 24px 这两个数在 3 个文件里各写一遍**（`layout.tsx:26-27`、`App.tsx:115`、`App.tsx:76-80`、`sidebar-styles.tsx:40-43`）→ 改宽度要动 3 处。

### 4.2 Electron 窗口外壳（本报告最可直接复用的一节）

```ts
// window-manager.ts:56-78
width: 900, height: 670, show: false,
transparent: true, backgroundColor: '#ffffff',
autoHideMenuBar: true, frame: false,
...(isMac ? { titleBarStyle: 'hiddenInset' } : {}),
hasShadow: false, paintWhenInitiallyHidden: true
// index.ts:82-88 追加：
titleBarOverlay: { color: '#111111', symbolColor: '#FFFFFF', height: 30 }
```

- `alwaysOnTop` / `skipTaskbar` / `resizable` / `minWidth` / `minHeight` **构造期全部未设置**，`minWidth/minHeight` 全仓库无命中 → **窗口最小尺寸未设**【已验证】。
- **模式切换不重建窗口，只改属性**，两阶段 + 500ms 延时：
  - 窗口模式：`setAlwaysOnTop(false)` → `setIgnoreMouseEvents(false)` → `setSkipTaskbar(false)` → `setResizable(true)` → `setFocusable(true)` → **再次** `setAlwaysOnTop(false)` → `setBackgroundColor('#ffffff')` → 恢复 bounds（无记录则 `setSize(900,670)` + `center()`）（`window-manager.ts:152-185`）。
  - 宠物模式：存 `windowedBounds` → `setBackgroundColor('#00000000')` → `setAlwaysOnTop(true,'screen-saver')` → `setPosition(0,0)` → 取所有显示器并集**铺满虚拟桌面** → `setResizable(false)` → `setSkipTaskbar(true)` → `setFocusable(false)` → **点击穿透**：mac `setIgnoreMouseEvents(true)`、其他平台 `(true,{forward:true})`（`window-manager.ts:187-241`）。
  - 握手：主进程 `setTimeout(500)` → 渲染侧**双层 `requestAnimationFrame`** 回 ack → 主进程才 `setOpacity(1)`（`index.ts:27-37`、`window-manager.ts:39-41`）→ 【推断】存在 **≥500ms 的整窗不可见期**，慢机器会闪。
- **细粒度点击穿透（设计亮点）**：渲染侧通过 `update-component-hover(componentId, isHovering)` 上报，主进程维护 `hoveringComponents: Set<string>`，`shouldIgnore = size === 0`（`window-manager.ts:284-307`）。注册方只有两处：Live2D 模型 canvas 命中测试（componentId `'live2d-model'`）与宠物浮窗（componentId `'input-subtitle'`）。另有托盘级**强制穿透开关** `toggle-force-ignore-mouse`（`window-manager.ts:310-332`）。
- 【推断】**缺陷**：`setFocusable(true)` 在悬浮时被设置，但鼠标离开只从 Set 里删、**不恢复 `setFocusable(false)`**（`window-manager.ts:290-295`）→ 碰过一次浮窗后，透明置顶窗会长期抢焦点。
- **关窗 = 隐藏**：`close` 一律 `preventDefault() + hide()`（`index.ts:91-97`）；真正退出只能走托盘 `Exit`（`menu-manager.ts:97-102`）。

### 4.3 视觉语言：**没有主题，等于把深色写死在每个 style 对象里**

- **`extendTheme` / `defineConfig` / `createSystem` / `semanticTokens` 全仓库 0 命中**【已验证】；`App.tsx:160` 直接 `<ChakraProvider value={defaultSystem}>`。
- **唯一的品牌色是硬编码 `#7C5CFF`（紫），只出现在 AI 状态胶囊**（`footer-styles.tsx:88`）；`src/renderer/src/**` 里 grep `#hex` **只命中这一处**。
- 其余全走 Chakra v3 默认步进：`gray.900`(应用底/侧栏/抽屉)、`gray.800`(侧栏外框/底栏/输入)、`gray.700`(聊天输入/气泡)，白色系一律 `whiteAlpha.50…900`，语义态用 `blue.500` / `green.500` / `yellow.500` / `red.500`。
- **浅色链路是死代码**：`next-themes` 全套封装存在（`components/ui/color-mode.tsx:12-16` + `:42-67` 的 `ColorModeButton`），但**唯一入口 `components/ui/provider.tsx` 没有任何导入者**（`grep ui/provider` refs=0），`App.tsx` 绕过了它 → `next-themes` 是**死依赖**，实际配色硬编码为深色，无 light 分支。
- **强制注入的 Chakra v3 默认 token**（从编译产物 `assets/main-CKgUHFa9.js` 抠出，**非项目自定义**）：
  - `fonts.heading` = `fonts.body` = `` `Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif, "Apple Color Emoji", …` ``
  - `fontSizes` = `2xs .625rem / xs .75rem / sm .875rem / md 1rem / lg 1.125rem / xl 1.25rem / 2xl 1.5rem / 3xl 1.875rem / 4xl 2.25rem / 5xl 3rem`
  - `radii` = `none 0 / 2xs .0625rem / xs .125rem / sm .25rem / md .375rem / lg .5rem / xl .75rem / 2xl 1rem / 3xl 1.5rem / 4xl 2rem / full 9999px`
- **第三套视觉系统：chatscope 覆写**（`sidebar-styles.tsx:483-561` 的一个 emotion 模板字符串，通过 `<Global styles={...}/>` 注入）。覆写**依赖 Chakra 运行时注入的 `--chakra-colors-*` / `--chakra-radii-*` 变量**，而编译产物的静态 CSS 里 **grep 不到任何 `--chakra-colors-*` 定义** → 【推断】**这层覆写是否生效取决于运行时变量是否注入；变量缺失时 `!important` + `var()` 一起失效**。关键覆盖值：
  ```
  .cs-message-list   background: var(--chakra-colors-gray-900) !important; padding: var(--chakra-space-4)
  .cs-message        margin: 12px 0
  .cs-message__content  background: var(--chakra-colors-gray-700) !important;
                        border-radius: var(--chakra-radii-md); padding: 8px !important;
                        font-size: 0.95rem !important; line-height: 1.5 !important; margin-top: 4px
  .cs-message--outgoing .cs-message__content  background: var(--chakra-colors-gray-600) !important
  .cs-chat-container  border: 1px solid var(--chakra-colors-whiteAlpha-200);
                      border-radius: var(--chakra-radii-lg); padding: var(--chakra-space-2)
  .cs-avatar          background: var(--chakra-colors-blue-500) !important; width/height 28px; border-radius: 50%
  .cs-message--outgoing .cs-avatar  background: var(--chakra-colors-green-500) !important
  .cs-message__sender  position:absolute; top:0; left:36px; font-size:.875rem; font-weight:600
  .cs-message__content-wrapper  max-width: 80%; margin: 0 8px
  ```
- **字体在同一屏上是两套**：聊天区用 chatscope 自带的 `Helvetica Neue, Segoe UI, Helvetica, Arial, sans-serif`，其余 UI 用 Chakra 的 `Inter, …`（`assets/main-QEkl09-0.css` 中 `.cs-message{font-family:…}`）。
- **CJK 字体：未核到**。全项目无中文/日文字体栈、无 `@font-face`、`index.html` 只有 `<meta charset>` + `<title>`（`src/renderer/index.html:1-12`）；`i18n.ts:57` 只把 `document.documentElement.lang` 设为当前语言。→ 【推断】**中文字形完全依赖 OS 回退链**，三平台三套字形。

### 4.4 字号实况（源码字面量统计）

| 值 | 次数 | 用例 |
| --- | --- | --- |
| `sm`（=0.875rem） | 12（JSX）+ 6（style 对象） | 面板正文 |
| `lg`（=1.125rem） | 4 | 面板标题、抽屉标题、设置 Tab 标题 |
| `xs`（=0.75rem） | 3+1 | 聊天气泡文本、工具调用文本、宠物浮窗状态文本 |
| `14px` | 2 | 字段标签、WS 状态徽标 |
| `18px` | 1 | 底栏输入框 |
| `12px` | 1 | AI 状态胶囊 |
| `1.5rem` | 1 | 字幕正文 |
| `0.95rem` / `0.875rem` | 各 1 | chatscope 覆写 |

- **结论**：字号体系是 **Chakra 的 rem 阶梯 + 5 处裸 px 补丁** 的混合体；`fontFamily` 只用了 2 处，都是 `'mono'`。

### 4.5 信息架构：Drawer + 6 个横向 Tab

- **形态 = 左侧 Drawer（浮层，`maxWidth:440px`，`bg gray.900`，`height: Electron? calc(100vh-30px) : 100vh`，`borderLeft: 1px solid whiteAlpha.200`）+ Drawer 内 `Tabs.Root`**（`setting-ui.tsx:98-164`、`setting-styles.tsx:72-78`）。
- 6 个 Tab 的 **EXACT 中文标签**（`locales/zh/translation.json:10-17`）：

| value | 中文 | 英文 |
| --- | --- | --- |
| `general` | 常规 | General |
| `live2d` | Live2D | Live2D |
| `asr` | 识别 | ASR |
| `tts` | 合成 | TTS |
| `agent` | 代理 | Agent |
| `about` | 关于 | About |

- 抽屉底栏只有 `取消`（`colorPalette=red`）+ `保存`（`colorPalette=blue`），**两者都调 `onClose()`**（`setting-ui.tsx:166-173`）→ 【推断】「取消」并不回滚，与“常规页即时生效”的链路冲突。
- **`合成`（TTS）Tab 是空的**：`tts.tsx:1-5` 组件体就是 `return <Box> </Box>;`（7 行文件），zh/en **都没有** `settings.tts.*` 任何 key → **可见 Tab 点进去空白**。
- **`Live2D` Tab 只有 2 项**：`鼠标交互`（默认 `false`）、`启用滚轮缩放`（默认 `true`）（`locales/zh/translation.json:33-36`、`live2d.tsx:36-50`）。
- **「常规」Tab 的项目顺序**（即渲染顺序）：`语言` → `使用摄像头背景` → `显示字幕` → `背景图片`（仅关闭摄像头背景时出现）→ `或输入自定义背景URL` → `角色预设` → `WebSocket地址` → `基础URL` → `图片压缩质量` → `图片最大宽度`（`general.tsx:81-170`，后两项的 placeholder/help 是**硬编码英文**）。
- 侧栏头部是 **5 个纯图标按钮、无文字、无 aria-label**：设置(⚙) / 群组管理(👥) / 聊天历史(🕐) / 新建会话(+) / 模式菜单(▤，`Live Mode` / `Pet Mode` **英文硬编码**，非 Electron 时 Pet 项 disabled)（`sidebar.tsx:60-109`）。
- 侧栏底部 `BottomTab` 3 个**带文字**的 Tab：`摄像头` / `屏幕` / `浏览器`，面板固定 `240px` 高（`bottom-tab.tsx:13-46`、`locales/zh/translation.json:74-76`）。

### 4.6 关键交互与状态呈现

- **AI 状态 = 6 态枚举，1 种视觉**：枚举（`ai-state-context.tsx:17-49`）与中文文案（`locales/zh/translation.json:123-130`）为 `空闲 / 思考说话中 / 已打断 / 加载中 / 聆听中 / 等待中`；但 `AIStateIndicator` 只渲染 `<Text>{t('aiState.'+aiState)}</Text>`，**无 Spinner、无图标、无颜色分支**，底色恒为 `#7C5CFF`，尺寸 `110×30px`，`borderRadius:12px`，`fontSize:12px`（`ai-state-indicator.tsx:12-14`、`footer-styles.tsx:86-104`）。
  → **“思考”和“说话”在状态机层面就是同一个值 `thinking-speaking`**（`ai-state-context.tsx:27`）。同一文件里 toaster 和 button 都用到了 `Spinner`，说明作者会写 spinner，只是状态胶囊没用。
- **连接状态**：`OPEN → green.500 + 已连接`、`CONNECTING → yellow.500 + 连接中`、其它 → `red.500 + 点击重新连接`（可点，`reconnect()`）；容器 `borderRadius:20px; padding:8px 16px; fontSize:14px`（`use-ws-status.ts:20-44`、`canvas-styles.tsx:55-72`）。
- **音量 / 静音：完全不存在**。`grep volume|mute` 在 `src/renderer/src` 只命中 `<video muted>`（摄像头预览）与**服务端下发的口型幅度数组** `volumes`（`websocket-handler.tsx:162`）。音频播放是裸 `HTMLAudioElement`：`const audio = new Audio(dataUrl); audio.play()`（`use-audio-task.ts:150-179`），**没有 GainNode、没有 AudioContext、没有 volume、没有 mute 状态**。
  → ⚠️ **本项目在这一点上明确优于上游**：我方 AGENTS.md 已裁决「主音量是客户端 `GainNode` 增益、与口型正交、静音置增益 0 但保持音频图活着」。**上游连音量层都没有，不可参考其实现。**
- **口型驱动是 monkey patch**：`const lipSyncScale = 2.0`，并覆盖 `model._wavFileHandler.update`，把 `_lastRms` 乘 2 后 clamp 到 2.0（`use-audio-task.ts:165, 182-194`）。
- **三套并存的“文本面”**：
  1. **字幕条**（窗口模式）：`background rgba(0,0,0,0.7)`、`padding 15px 30px`、`borderRadius 12px`、`minWidth 60%`、`maxWidth 95%`、`fontSize 1.5rem`、`lineHeight 1.4`、`whiteSpace pre-wrap`；渲染条件是 `isLoaded && subtitleText && showSubtitle` 三者同时为真，否则 `return null`（**无淡出**）（`canvas-styles.tsx:39-54`、`subtitle.tsx:22-33`）。
  2. **聊天气泡**（chatscope）。
  3. **宠物模式浮窗**：只显示**最后一条 AI 消息**（`messages.filter(role==='ai').slice(-1)[0].content`，`use-input-subtitle.ts:26-29`），下面一行 `LuBell` + **裸 `{aiState}`**（未走 `t()`，所以宠物模式显示英文枚举 `idle`/`thinking-speaking`，而窗口模式显示中文）（`input-subtitle.tsx:124-126`）。
- **字幕是“逐句覆写”，无队列、无时长控制**：`subtitle-context` 只有 `subtitleText / setSubtitleText`，字幕会一直停在最后一句直到下一条 WS 消息（`use-subtitle-display.ts:4-15`）。→ 与「一句一单元」不冲突，但**没有任何“随播放结束清空/对齐音频”的逻辑**。
- **打断按钮是双语义隐藏按钮**：`aiState==='thinking-speaking'` 时点击 = `interrupt()`（可能再 `startMic()`）；否则若 `settings.allowButtonTrigger` 为真 = `sendTriggerSignal(-1)`（举手请求发言）。**外观完全不变**（恒 `yellow.500`，`aria-label="Raise hand"`）（`use-footer.tsx:35-44`）→ 用户无法知道这个按钮此刻是“打断”还是“请求发言”。
- **工具调用内联为一行**（不是气泡）：`FaTools`(blue.300, 14px) + 文本 + 状态图标（running→`Spinner`、completed→`FaCheck`(green.300)、error→`FaTimes`(red.300)），容器 `pl:'44px'`（对齐头像）、`minHeight:24px`；文案**硬编码英文** `` `${msg.name} is using tool ${msg.tool_name}` ``（`chat-history-panel.tsx:58-96`、`sidebar-styles.tsx:445-480`）。
- **模型切换两条路径**：设置里 `角色预设` Select（`general.tsx:121-127`）；或宠物模式**右键菜单 `Switch Character`**（仅宠物模式可见，`menu-manager.ts:155-164`）。切换动作顺序：`setSubtitleText('New Character Loading...')` → `interrupt()` → `stopMic()` → `setAiState('loading')` → `setModelInfo(undefined)`（`use-switch-character.tsx:18-36`）——**字幕里写死的英文未走 i18n**，而 zh 里其实有 `notification.characterLoaded = 新角色已加载`（只用在服务端 `config-switched` 之后）。

### 4.7 动效

- **`framer-motion` 是死依赖**：`package.json:46` 有 `^11.14.4`，但 `grep -rln "framer-motion"` **只命中 `package.json` 与 `package-lock.json`**；`AnimatePresence|motion.div|useMotionValue` 在 `src/` **0 命中**。→ **没有 variants / duration / easing / layout 动画**，动效 100% 是 CSS transition。
- 实际动效值（逐条）：

| 对象 | 值 | 证据 |
| --- | --- | --- |
| 侧栏展开/收起 | `all 0.3s cubic-bezier(0.4,0,0.2,1)` + `translateX(calc(-100% + 24px))` | `sidebar-styles.tsx:42-49` |
| 底栏折叠 | 同上曲线；`translateY(calc(100% - 24px))`；`bg` `gray.800↔transparent`；`borderTopRadius` `lg↔none` | `footer-styles.tsx:21-30` |
| 侧栏外框 | `all 0.2s`（**与上面的 0.3s 不一致**） | `layout.tsx:33` |
| Live2D 容器 | `all 0.3s ease-in-out`（**第三种曲线**） | `App.tsx:64` |
| 主内容/画布/底栏 | `all 0.3s cubic-bezier(0.4,0,0.2,1)` | `layout.tsx:41,49,56` |
| 宠物浮窗拖拽 | `isDragging ? 'none' : 'transform 0.1s ease'` | `use-draggable.ts:76,94` |
| 模型缩放 | rAF 插值，`EASING_FACTOR=0.3`、`WHEEL_SCALE_STEP=0.03`、`MIN 0.1 / MAX 5.0 / DEFAULT 1.0` | `use-live2d-resize.ts:11-15,96-135` |
| 摄像头/屏幕/浏览器面板 | `all 0.2s` | `sidebar-styles.tsx:262,296,330` |
| 滚动条 | `width:4px`；track `whiteAlpha.100`；thumb `whiteAlpha.300`；`borderRadius full` | `sidebar-styles.tsx:6-18` |
| 摄像头“直播”脉冲点 | `animation:'pulse 2s infinite'` —— **全项目无 `@keyframes pulse` 定义** → 不生效 | `camera-panel.tsx:15` |

- **消息出现动效：无**。`.cs-message` 只设 `margin: 12px 0`，无 enter/exit/keyframe（`sidebar-styles.tsx:489-492`）。

### 4.8 明确的设计缺陷

1. **无主题、无令牌、无 CJK 字体**（§4.3）：一个 13.7k star 的同类项目，设计系统层几乎是空白。
2. **一个可见 Tab 点进去是空白页**（`tts.tsx:1-5`）。
3. **6 个 AI 状态共用 1 种视觉**，且“思考/说话”在状态机里本就是 1 个值（§4.6）。
4. **三套动效曲线**并存（`0.3s cubic-bezier(0.4,0,0.2,1)` / `0.3s ease-in-out` / `0.2s`）且**一个 animation 的 keyframes 根本没定义**（`camera-panel.tsx:15`）。
5. **`framer-motion` 与 `next-themes` 都是死依赖**（§4.3、§4.7）→ 说明这个项目的依赖管理没有清理机制。
6. **跨库覆写用“框架私有 CSS 变量 + `!important`”**（`sidebar-styles.tsx:483-561`），且这些变量在静态产物里根本不存在 → 【推断】脆弱。
7. **同一个数写 3 遍**（440px / 24px，§4.1）。
8. **宠物模式状态显示英文枚举裸值**（`input-subtitle.tsx:124-126`），切角色提示写死英文（`use-switch-character.tsx:26`），工具调用文案写死英文（`chat-history-panel.tsx:72`），图片压缩两项 placeholder/help 写死英文（`general.tsx:133,140`）→ **中英混排的系统性位置**。
9. **一个按钮两种语义、外观不变**（§4.6 打断/举手）。
10. **字幕不随音频结束清空**（§4.6）。
11. **窗口最小尺寸未设**、`contextIsolation: true` 同时 `nodeIntegration: true` + `sandbox: false`（`window-manager.ts:71-73`）→ 安全取舍与“透明置顶宠物窗”这种高风险窗口不相称。

### 4.9 可迁移的具体设计决策

1. **因为** OLV-Web 的侧栏是「**440px 面板 + 24px 常驻把手 + `translateX` 收起**」，收起后仍留一条可点细边（`sidebar-styles.tsx:35-67`），**所以我们应该**：聊天面板收起时**保留一个明确的“拉出”把手**，而不是完全消失——这是“随时能说话”的低摩擦设计。
2. **因为** 它的**舞台 `left/width` 会跟随侧栏状态联动**（`App.tsx:76-80`），角色永远居中于剩余空间，**所以我们应该**：舞台尺寸必须由**容器实测宽度**驱动（Flutter 用 `LayoutBuilder`），而不是从面板宽度反算硬编码值。
3. **因为** OLV-Web 用**整条 24px 竖边**作折叠把手而不是 12px 图标（`sidebar-styles.tsx:52-67`），**所以我们应该**：把可点区域做得比视觉元素大（至少 24px 命中区）。
4. **因为** 它的宠物模式浮窗用 `blackAlpha.700 + backdropFilter: blur(8px)`（`electron-style.tsx:4-27`），**所以我们应该**：字幕/输入浮层统一用「70% 黑 + 8px 模糊」，这是经过验证的“深色背景上可读、浅色背景上也不脏”的配方。
5. **因为** 它做了**细粒度点击穿透**（按组件注册 hover 集合，`window-manager.ts:284-307`），**所以我们应该**：穿透不要做全局开关，而是**按“可交互区域”注册**——但要修掉它“离开后不恢复 `focusable:false`”的 bug。
6. **因为** 它的**关窗 = 隐藏、真退出只能走托盘**（`index.ts:91-97`、`menu-manager.ts:97-102`），**所以我们应该**：桌宠默认“关闭即最小化到托盘”，并在首次关闭时提示这一点。
7. **因为** 它把 `titleBarOverlay` 设成 `#111111/#FFFFFF` 而应用底色是 `gray.900`（`index.ts:82-88`），两者不是同一个值，**所以我们应该**：窗口装饰色必须引用**同一个令牌**，否则原生标题栏与内容底色会在深色模式下微妙地不匹配。
8. **因为** 它的 6 个 AI 状态只有 1 种视觉、而状态机里“思考/说话”本就是同一个值（§4.6），**所以我们应该**：先把状态枚举收敛到**互斥且视觉可区分**的最小集合（建议：`离线 / 空闲 / 聆听 / 思考 / 说话 / 错误`），每个状态在“色 + 形 + 字”里至少占两个通道。
9. **因为** 上游完全**没有音量/静音层**（`use-audio-task.ts:150-179`），**所以我们应该**：把“主音量（客户端 GainNode，与口型正交）”当作差异化的必备件（我方 AGENTS.md 已裁决），并在 UI 上给它一级入口。

---

## 5. VTube Studio（闭源商业标杆）—— 只借“控制台布局模式”

> 一手证据：Steam 官方商店截图（`store.steampowered.com/app/1325860`，经 `appdetails` API 取 `path_full` 后下载；`t=1750309780`）。闭源，**无源码**，故视觉数值一律不断言。

**截图可见的布局模式（`v2` / `v4`）**：

- 角色铺满整个窗口，背景是一张实景/插画场景；**UI 完全不占据角色区域**。
- **左侧一列竖向圆形图标 rail**，每个图标一个**饱和的语义色圆底**（红=人物/profile、绿=相机/表情、黄=锁、蓝=设置组、粉=齿轮、橙=文件夹、红色 ✕=退出）。
- 顶部有**一条居中的胶囊提示条**（截图为 `Models not included in the app. You need to import your own models!`），带一个兔子图标。
- 右下角有**社交账号水印块**（Twitter 图标 + 白色圆角条的 `twitter.com/<name>`）。
- **没有常驻聊天框、没有设置侧栏、没有状态胶囊。**

**可迁移的设计决策**

1. **因为** VTube Studio 把**全部配置收进左侧一列图标**，角色区域永远干净（截图 `v2`/`v4`），**所以我们应该**：桌宠形态下**默认零常驻 UI**，只保留一个**可收起的图标 rail**；“配置”不应该在陪伴态占据屏幕。
2. **因为** 它的每个图标用**不同的语义色圆底**来区分功能（截图），**所以我们应该**：图标 rail 用“色 + 形”双编码，让用户在收起文字的窄栏里也能凭颜色定位；但这要求**颜色语义在全局唯一**（不能像 N.E.K.O 那样出现 5 个近似蓝）。
3. **因为** 它把一次性提示做成**顶部居中胶囊**而不是弹窗（截图顶部提示条），**所以我们应该**：非阻塞提示用顶部胶囊 toast，不要用模态对话框打断陪伴。
4. **因为** 它把**水印/账号信息固定在右下角**（截图），**所以我们应该**：如果要做“角色名牌/来源信息”，固定在一个不干扰角色的角落，且默认低对比度。

---

## 6. Warudo（闭源商业标杆）—— 借“双窗口 + 标签页 + 节点图”的控制台范式

> 一手证据：官方手册 `https://docs.warudo.app/docs/tutorials/getting-started`（zh/ja/ko/es 多语，`Last updated on 2026.01.13`）。

**官方文档明确描述的结构**：

- **开两个窗口**：“when you open Warudo, you should see two windows: the **main window** and the **editor window**”；“The editor is used to configure your character, environment, etc., while the main window is where everything is in action. You can close the editor window at anytime and reopen it by pressing **Esc** while the main window is focused.”（原文摘录）
- **Editor 有两个顶层 Tab**：**Assets**（场景中一切皆 asset：角色/相机/道具/动捕设备/灯光…）与 **Blueprints**（节点图，“当某事发生时要做什么”）。
- **Asset 列表 → 选中 → 属性面板**的三段式；属性可重置（“click the **Reset** button that appears when you hover over the **Scale** values”）。
- **表达式可绑定热键**：onboarding 问“if your model contains expressions, Warudo can help you import them and **assign hotkeys** to them”，并弹出“a list of expressions just imported along with their hotkeys”。
- **onboarding 是问答式向导**（问“你有哪些设备” → 推荐一套方案 → 确认），而不是配置表单。
- **输出设置集中在 `Settings → Output`**（Virtual Camera / NDI / Spout / 窗口捕获四选）。
- **明确承认“选项太多”**：文档 FAQ 自述 “Help! There are so many options in Warudo, and I don't know what they do!” → 官方回答是“很多选项是给高级用户的，日常不需要懂”。

**可迁移的设计决策**

1. **因为** Warudo 把“演出”和“配置”**拆成两个 OS 窗口**，配置窗可随时关闭、按 Esc 再打开（官方文档原文），**所以我们应该**：**桌面壳分为「舞台窗」与「控制台窗」**，且控制台窗**不阻塞**舞台窗；这正是我方 Rust 桌面壳 + Web 前端的天然对应（Web 端可作为控制台，舞台由 wasm 渲染面承担）。
2. **因为** Warudo 的 Editor 只有两个顶层 Tab（**Assets / Blueprints**）就覆盖了全部配置，**所以我们应该**：控制台的一级导航**不超过 3 个**——「外观/形象」「行为/触发」「系统」，每个之下再分。
3. **因为** Warudo 的属性面板支持**悬停即出现 Reset**（官方文档），**所以我们应该**：每个可调项都要有“恢复默认”，且入口就近（悬停/长按出现），不要集中到一个“重置全部”。
4. **因为** Warudo 的导入流程**会询问用户设备并推荐配置**（问答式 onboarding），**所以我们应该**：首次启动用**问答式向导**而不是设置表单；对“模型导入”尤其如此（AIRI 的“导入前检查”也是同思路，见 §2.4）。
5. **因为** Warudo 官方在文档里**主动承认“选项太多”并给出“不用全懂”的安抚**，**所以我们应该**：对一个功能会持续膨胀的产品，**“默认隐藏高级项 + 显式高级模式”**要作为 IA 原则写进设计约束，而不是等用户抱怨。

---

## 7. 其余开源项目

> 本节各项目的一手取证方式与维度覆盖见对应小节；**无法核到的维度已逐条标注**（汇总见 §11）。

### 7.1 样本成熟度排序（基于可量化的一手证据）

| 排名 | 项目 | 判定依据（一句话 + 证据） |
| --- | --- | --- |
| — | **AIRI** | 唯一有**完整令牌链**（色相标量 → chroma 公式 → `color-mix` 语义色）且**用单测覆盖几何计算**的项目（§2） |
| 1 | **Meuxe** | 唯一**把规范写成会自曝的代码**：`--color-*: initial` 清空默认色板后白名单式定义，越界色阶静默失效（`index.css:21` + `docs/DESIGN.md:34`） |
| 2 | **Nexus** | 有 token 源但**零采纳**：`var(--radius-*)` 全仓 **1 次** vs 硬编码 `border-radius: 8px` **223 处**；`var(--shadow-*)` **0 次** vs **151 个不同 box-shadow 字面值** |
| 3 | **Soul of Waifu** | 无 token 源：**335 个不同 hex / 902 次出现 / 1,196 处内联 `setStyleSheet`**；圆角 **35 种取值** |
| 4 | **my-neuro** | 三套设计系统并存，只有 WebUI 那层有真令牌（§7.4） |
| 5 | **Live2DPet** | 极限反例：**0 个 `.css` 文件 / 0 个 `:root` / 0 个 `var(--)`**（§7.5） |
| 6 | **ChatVRM / Amica** | 依赖外部 preset 且 Amica 把它弄丢了，导致约 14 处类名零样式（§7.6） |

**【推断】** 该排序主要由「**是否把设计规范写进代码并被工具强制**」决定，而非语言/框架：Meuxe 让越界色阶**静默失败**，Nexus 让字面值**静默成功**——后者才是设计系统失控的根因。

---

### 7.2 Soul of Waifu（jofizcd/Soul-of-Waifu）—— Python + PyQt6，UI 是真实实现但设计系统几乎不存在

> ⭐ 1,305，`pushed_at` 2026-08-28，Python，**GPL-3.0**（`LICENSE:1-2` 为逐字 GPLv3 全文；仓内 `grep "or later"` 在 `.md/.py` **0 命中**，`LICENSE:639-643` 的 "or later" 属 FSF 模板附录样板且含 `<year> <name of author>` 占位符；源码无 license 头）。
> ⚠️ **许可含义**：GPL-3.0 代码**可以并入** AGPL-3.0 项目（单向兼容），但它**不能反向吸收 AGPL 代码**，且 copyleft 会传染 → **本项目只能借鉴其设计思路与结构，不可复制代码**。
> 规模证实（非 stub）：`app/gui/sowInterface.py` = 7,375 行、`app/gui/custom_widgets.py` = 10,084 行。

#### 7.2.1 布局：主窗内三栏 + 两个额外独立置顶窗；舞台是可拖宽的右栏

- 主窗**无边框 + 透明底**，且**最小尺寸 = 默认尺寸 = 1350×734**：`MainWindow.resize(1350, 734)` / `setMinimumSize(QSize(1350, 734))` / `setWindowFlags(FramelessWindowHint | Window)` / `setAttribute(WA_TranslucentBackground)`（`sowInterface.py:43-47`）→ 【推断】**小屏不可用**，且**窗口几何完全不持久化**（`QSettings` 全仓 0 命中）。
- 三栏是 **QGridLayout**（不是可拖整体分割）：`addWidget(self.menu_bar, 0, 0, 1, 3)`（自绘标题栏高 25）+ 左栏 + 中栏 + 右栏（舞台）（`sowInterface.py:275,108-109,5066,4477,13810`）；中央聊天区最小 800×648（`:277`）。
- 左导航**定宽 190 且可折叠**（`setMinimumSize(190,648)` / `setMaximumSize(190,∞)`，`sowInterface.py:4480-4481`；折叠目标宽在 `interface_signals.py:413-416`：`new_width = 190` else `0`）。
- **Live2D/VRM 舞台 = 主窗内可拖拽调宽面板**（不是 QSplitter、不是独立窗）：`self.avatar_resizer = QFrame(...)` + `setCursor(SplitHCursor)` + `setFixedWidth(3)`（`interface_signals.py:13582-13584`）；拖拽区间 `max(200, min(800, start_width + delta))`（`:13620`）→ `expression_widget.setFixedWidth(...)`（`:13621`）；下限 `setMinimumSize(QSize(320,0))`（`:13640`）。
  → **舞台宽度：3px 抓边 / 200–800px 可调**。
- **另有独立 OS 级置顶桌宠窗**：`resize(400,600)` + `FramelessWindowHint | WindowStaysOnTopHint | Tool` + `WA_TranslucentBackground`（`sow_system_signals.py:3068-3076`）；点击穿透 `flags |= WindowTransparentForInput`（`:3142`）；三档尺寸 `200×300 / 400×600 / 600×900`（`:3578,3580,3582`）。
- **字幕是第三个独立置顶窗**（`WindowDoesNotAcceptFocus` + 鼠标穿透，`sow_system_signals.py:4244-4251`）。
- **无 dock**：`QDockWidget` / `addDockWidget` 全仓 **0 命中**；`QSplitter` 仅 3 处且主舞台与聊天之间**没有**。
- 舞台模式枚举 4 个：`Live2D Model` / `Expressions Images` / `VRM` / `Nothing`（`interface_signals.py:13673,13689,13700,13566`）。

#### 7.2.2 视觉语言：深色单主题，**无中央 token 源**，硬编码与常量分叉极重

- 深色单主题（无浅色 / 无切换）：`self._BG = "#070709"`（`sowInterface.py:601,629`）；`custom_widgets.py:4152` 的 `_BG = "#0C0C10"`；`interface_signals.py:4188` 的 `_BG = "#070709"`。
- **量化事实（基数：全部 `app/gui/*.py` + `main.py`）**：hex 字面值 **902 次 / 335 个不同值**；`setStyleSheet` 内联 **1,196 处**（sowInterface 235 / custom_widgets 326 / interface_signals 308 / soul_stage_page 187 / 其余 140）。
- **常量重复且值分叉**：类内 `_BG` 定义 **12 处**（8 处 `#0C0C10` + `custom_widgets.py:9017` 的 `#0D0D12` + 3 处 `#070709`）；`_TEXT` **24 处** / `_BORDER` **25 处** / `_ACCENT` **7 处**。
- 局部「伪 token」类互不共享且值不一致：`class C`（`sowSystem.py:11-25`）`ROOT="#08080e" / PANEL="#0c0c0c" / T_PRIMARY="#e8e8f0"` vs `character_ai_assistant.py:43-62` 的 `TEXT="#E6E8EE" / ACC="#A855F7" / BLUE="#4BB8FF"`。
- 唯一接近 token 的运行时主题表**只覆盖背景/滚动条 6 个槽**：`THEMES` = `default(27,27,27) / darker(15,15,15) / slate(22,28,36) / warm(30,24,18) / violet(20,18,30) / forest(18,26,20)`（`interface_signals.py:580-587`）。
- **强调色跨 view 六套并存**：`#4BB8FF`(38) / `#82CDFF`(25) / `#C49A38`(`custom_widgets.py:4161 _ACCENT`) / `#A855F7`(`character_ai_assistant.py:51`) / `#8ab4f8`(`sowInterface.py:1951`) / `#6C86FF`(`soul_stage_page.py:637`) / `#00E676` / `#E2B34C`。
- **圆角无 scale**：`border-radius: Npx` 共 **35 个不同值**（8px 141 / 6px 110 / 10px 83 / 4px 48 / 12px 33 / 3px 22 / 16px 17 / 2px 15 / 15px 14 / 20px 13 / 5px 10 / 14px 10 … 最大 70px）。
- 字体栈：`Inter Tight` / `Inter Tight Medium` / `Inter Tight SemiBold`（`QFont("Inter Tight Medium", 10)` **19 次**，`sowInterface.py:49,52,55`）；字幕/logo 用 `Comfortaa`（`sow_system_signals.py:4270`）；等宽 `Cascadia Code/Consolas`（`custom_widgets.py:3007`）。
- **字号单位混用**：QSS `font-size: Npx` **184 处**（13px 54 / 12px 50 / 11px 28 / 14px 14 / 10px 10…）vs `setPointSize(N)` **25 处**。
- **阴影 41 处 `QGraphicsDropShadowEffect`，blur 无分级**：20(10) / 15(10) / 28(5) / 25(5) / 18(4) / 10(3) / 8(2) / 35,32,14(各 1)。
- **间距无网格**：`setSpacing` 取值 0(92) / 10(79) / 12(64) / 8(40) / 6(26) / 15(26) / 4(22) / 16(22) / 20(18) / 5(15) / 14(13)。
- 玻璃拟态：Qt 侧 `rgba(...)` + `qlineargradient`（`sowInterface.py:4504`）；Web 侧 `backdrop-filter: blur(20px)`（`app/web_client/style.css:41-42`）与 blur 10/25/30px。

#### 7.2.3 信息架构：`QListWidget 行 + QStackedWidget.setCurrentIndex`，**无 tab / 无 dock / 无抽屉 / 无 modal 设置**

- 一级左栏 6 项**精确名**（`main.py:230-235`）：`" Main"` / `" Characters Gateway"` / `" Models Hub"` / `" RP Editors"` / `" Soul Stage"` / `" Options"`（**前导空格是字符串本身的一部分**）。
- 页面注册 9 个：`no_characters:382` / `main_characters:599` / `create_character:1422` / `charactersgateway:1644` / `options:3657` / `chat:4144` / `modelshub:4343` / `rp_editors:4470` / `soul_stage:4474`。
- **Options（设置）二级侧栏**：定宽 230、标题 `"SETTINGS"`、**5 项精确名**（`sowInterface.py:1683,1651,1749-1753`）：`"API & Providers"` / `"System & UI"` / `"Local LLM"` / `"SoW Modules"` / `"Tool Calling & MCP"`；第 6 项 `"Appearance"` 运行时追加（`:3649-3652`）。
- 设置页内 `create_glass_card` 玻璃卡共 **16 个标题**：`Localization & Translation` / `Audio Devices` / `Text-to-Speech Rules` / `Hardware Specifications` / `Conversation Provider` / `API Configuration` / `General Generation Parameters` / `Hardware & Backend` / `Prompting & Formatting` / `Advanced Local LLM Sampling` / `Native AI Capabilities` / `Model Context Protocol (MCP)` / `"Soul of Waifu System"` / `Visualizations (Live2D / VRM)` / `Local Web Server` / `Sub-Modules`（`sowInterface.py:1893,1971,2042,2074,2113,2161,2182,2357,2447,2656,2690,2815,3013,3091,3151,3247,3354`）。
- 其他二级导航：角色编辑器侧栏定宽 220、标题 `"NAVIGATION"`、6 项（`General Info / Personality & Scenario / Dialogues / Advanced & Lore / Variables & State / Export / Utils`）；Characters Gateway rail 定宽 220、4 项（`Soul Gateway / Chub AI Hub / World Lorebooks / Soul Stage Scenarios`）；Soul Stage 4 视图。
- **Web 客户端是并行维护的第二套 IA**：`app/web_client/index.html:44-45` 的 `#main-content` 内 `#avatar-container` + `:75` 的 `#chat-container`，`:107` 的 `#main-menu` 是全屏遮罩模态选角色。

#### 7.2.4 关键交互

- **角色/模型切换时销毁重建舞台**：`if widget.objectName() == "expression_widget": widget.deleteLater()`（`interface_signals.py:13571-13573`）。
- **动作/表情触发**：`play_motion_safely(self, target_group)` → `live2d_model.StartMotion(group_name, no, priority_val)`（`custom_widgets.py:302,295`）；表情 `live2d_model.SetExpression(current_emotion)`（`:215`）；右键菜单动态列 motion group（`sow_system_signals.py:3448`）。
- **音量 / 静音：未核到** —— `grep -rn "setVolume|self.volume|\.mute|is_muted"` 在 `app/gui/*.py`、`app/configuration/*.py`、`app/web_client/*.js` **零命中**，`settings.json` 亦无 volume/mute 键。只有**输入侧**波形 `audio_worker.volume_signal.connect(ui.waveform_widget.push_volume)`（`sow_system_signals.py:252`）。
- **字幕有完整的句读/逐词揭示引擎**（这是全项目最值得借鉴的部分）：
  - 节奏：`char_time = len(clean_word) * 45` + `base_gap = 100`；句读 `+= 450`；逗号 `+= 200`；clamp `MAX_INTERVAL = 700` / `MIN_INTERVAL = 80`（`sow_system_signals.py:4288-4300`）。
  - 淡入淡出：`FADE_IN_MS = 85` / `FADE_OUT_MS = 380`；自适应字号 `FONT_MAX_PX = 40` / `FONT_MIN_PX = 18`（`:4225-4232`）。
  - ⚠️ 但它**硬绑在 companion 窗上**（`class CompanionSubtitleOverlay`，`:4224`），不可复用。
- **字幕 vs 气泡是两条路**：字幕走 overlay 窗，聊天走气泡流（`add_message(...)`，`sow_system_signals.py:893`）。
- **状态机与配色是单点映射（正例！）**：`"STOPPED": ("rgb(70,70,70)", "Ready")` / `LISTENING rgb(46,204,113) "Listening..."` / `PROCESSING rgb(241,196,15) "Thinking..."` / `SPEAKING rgb(52,152,219) "Speaking..."`（`sow_system_signals.py:213-218,177`），写入 `voice_indicator.setStyleSheet`（`:223`）与 `status_label.setText`（`:229`），**并同步到 VRM 渲染面** `runJavaScript("window.setAppState('{state}')")`（`:240`）。
  → **四态四色，色 + 字双通道**，是本次调研里除 Meuxe 外唯一做对状态映射的项目。
- 连接态（Web 端）：`.status-online { color:#4ade80 !important }` / `.status-offline { color:#f87171 !important }`（`style.css:77-78`）。

#### 7.2.5 动效：Qt `QPropertyAnimation` 为主，时长/缓动均无令牌

- 用量：`QPropertyAnimation` 74 / `setDuration` 67 / `QEasingCurve` 63 / `QTimer` 86 / `setWindowOpacity` 9 / `QGraphicsOpacityEffect` 6。
- **时长无梯度**：`300`×12、`200`×12、`350`×11、`220`×6、`250`×4、`180`×4、`160`×4，另有 `500/450/280/1400` 各 1。
- **缓动分布**：`OutCubic`×22、`InOutCubic`×13、`OutQuad`×11、`InCubic`×9（**无 spring / OutBack**）。
- 具体：侧栏开关用 `QPropertyAnimation(SideBar_Left, b"minimumWidth")` + `setDuration(300)` + `InOutQuart`（`interface_signals.py:418-422`）；卡片 hover 三动画并行 350/350/300ms `InOutCubic`；Toast geometry 250ms `OutCubic`；输入框高度 `QParallelAnimationGroup` duration 100 + 4 条 `OutQuad`。
- Web 侧：`transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1)`（`style.css:604`）；keyframes `fadeIn 0.3s` / `slideUp 0.3s` / `bounce 1.4s infinite`。

#### 7.2.6 明确的设计缺陷

1. **无单一 token 源**：335 个不同 hex / 902 次出现 / 1,196 处内联 `setStyleSheet`。
2. **常量重复且值分叉**：`_BG` 12 处（三种值）/ `_TEXT` 24 处 / `_BORDER` 25 处 / `_ACCENT` 7 处。
3. **同一字段两个默认值且互相矛盾**：`settings.json:59-65` 的 `text_color "#E8E8E8"` / `border_radius 20` / `quote_color "#E8A040"` / `italic_color "#A0A0A0"` **vs** `interface_signals.py:437-443` 的 `"#DCDCDC"` / `15` / `"#FFA500"` / `#a3a3a3`。→ **代码常量与配置文件各说各话**。
4. **圆角 35 种取值**，同文件内 14/15/16/20px 混用。
5. **强调色跨 view 六套冲突**（§7.2.2）。
6. **Web 端强调色自相矛盾**：`--accent-gold: rgba(255,200,80,0.15)`（`style.css:5`）全文件仅用 1 次（`:200`），实际 hover 用硬编码 `rgba(255,157,0,0.08)`（`:609`）与 `border-color:#ff9d00`（`:623`）。
7. **emoji 当图标：201 处**（`👁`13 / `✕`10 / `🎬`9 / `💬`8 / `⚠`7 / `🧠`6 / `🗨`6 / `⚡`6 / `📌`5 …），含托盘 `tray_menu.addAction("👁🗨 Toggle Click-Through")`（`sow_system_signals.py:3115`）；另有字形按钮 `≡` / `✓` / `★` / `◈`（`sowInterface.py:132` 等）。
8. **缺加载态构件**：思考态靠 **emoji 文案占位** —— `await self.add_message(self.character_name, "💭 Thinking...", ...)`（`sow_system_signals.py:893`），**无 skeleton / spinner**。
9. **px 与 pt 混用**；**主窗最小尺寸 = 默认尺寸**（1350×734）；**窗口几何不持久化**（QSettings 0 命中）。

#### 7.2.7 可迁移的设计决策

1. **因为** 全仓 335 个不同 hex 与 1,196 处内联样式，`_BG`/`_TEXT` 各重复 12/24 次，**所以我们应该**：建立单一 token 模块（色/圆角/间距/字阶/时长/窗口高度）并**禁止业务文件出现字面色值**。
2. **因为** 它的状态机→配色是**单点映射且色+字双通道**（`sow_system_signals.py:213-218`），**所以我们应该**：直接采用这套四态四色（灰 Ready / 绿 Listening / 黄 Thinking / 蓝 Speaking）的语义分工，并照它的做法**同步到渲染面**（`window.setAppState(...)`）——这与我们 Rust 状态机的边界天然吻合。
3. **因为** 它的字幕/逐词节奏是一套**可复用的量化参数**（45ms/字、句读 +450ms、逗号 +200ms、clamp 80–700ms、淡入 85ms / 淡出 380ms、字号 18–40px 自适应），**所以我们应该**：把这组数值作为字幕时序的**起点基线**做 A/B，而不是从零猜。
4. **因为** 这份字幕引擎**硬绑在 companion 窗上不可复用**（`class CompanionSubtitleOverlay`），**所以我们应该**：字幕从一开始就做成**独立组件 + 可换宿主**（页内 / 独立浮窗），而不是长在某个窗口里。
5. **因为** 同一设置字段在 JSON 与代码里各有一套互相矛盾的默认值（§7.2.6.3），**所以我们应该**：**代码常量是唯一默认真源**，配置文件降级为纯用户覆盖层。
6. **因为** 圆角 35 值 / `setSpacing` 13 值 / 字号 px 与 pt 并存，**所以我们应该**：把这三类先收敛为固定档位（如 radius 4/8/12/16/20、spacing 4/8/12/16/24），再用 lint/测试拒绝越界字面值。
7. **因为** 舞台存在**两套互不共享的实现**（主窗内 200–800px 可拖右栏 vs 独立 400×600 置顶 Tool 窗），**所以我们应该**：抽出统一 `StageHost` 抽象（尺寸策略 / 嵌入模式 / 置顶模式），让不同渲染后端共用容器契约。
8. **因为** 它的导航是**左栏 `QListWidget` + `QStackedWidget`，设置是二级侧栏（230px）+ 玻璃卡**，**所以我们应该**：`≤6 个一级项 + 每个一级项内 ≤6 个二级项` 的分组上限可以作为我们的初始约束（它是被实际使用验证过的规模，而不是理论值）。

---

### 7.3 Meuxe（meet447/Meuxe）—— 本次调研中设计系统最干净的样本

> ⭐ 79，`pushed_at` 2026-09-09，Tauri 2 + React 19 + TS，**MIT**（`LICENSE:1-3` + `package.json:4`）。
> 证据：本地源码（`src/`、`src-tauri/`、`docs/DESIGN.md`）。

#### 7.3.1 布局：三栏并列，舞台是“卡片”

- 外壳：`<div className="flex h-screen gap-3 bg-canvas p-3 font-sans text-ink">`（`src/App.tsx:480`）→ 横向 flex + **12px 外边距**，面板之间靠 `canvas` 底色 + `gap` 分隔，**不使用分割线**。
- 左导航：`<nav className="flex w-16 shrink-0 flex-col items-center py-2">`（`Sidebar.tsx:34`）→ 固定 **64px 图标轨道，不可折叠**（无折叠状态变量）。
- 中舞台：`<main className="squircle relative min-h-0 min-w-0 flex-1 overflow-hidden rounded-panel bg-surface shadow-soft">`（`App.tsx:505`）→ `flex-1` 自适应 + `rounded-panel`(20px) 卡片 + `squircle` 圆角；画布 `absolute inset-0` 铺满（`App.tsx:524`）。
- 右会话面板：`<aside className="squircle flex w-[380px] shrink-0 flex-col overflow-hidden rounded-panel bg-surface shadow-soft animate-rise-in">`（`HistoryDrawer.tsx:18`）→ **固定 380px 并列面板**；`open=false` 时 `return null`（`:15`）→ 可开关但**无宽度过渡**（瞬时挂载/卸载）。
- 设置：居中模态 sheet：`fixed inset-0 z-[120] … animate-fade-in items-center justify-center p-4 sm:p-6`（`Settings.tsx:624-635`）+ `h-[min(760px,92vh)] w-full max-w-4xl animate-pop-in`，内层左侧 `w-60` 导航（`Settings.tsx:637`）。
- **独立 OS 窗（桌宠）**：主窗 **1200×800，min 800×600**（`src-tauri/tauri.conf.json:16-19`）；迷你窗 **280×420**，`transparent(true) / decorations(false) / always_on_top(true) / resizable(false) / skip_taskbar(true)`（`src-tauri/src/window.rs:19-28`）；开迷你窗时**隐藏主窗**（`window.rs:71 hide_main_window(&app);`），两窗通过 `app:mode-changed` 事件同步（`window.rs:64-78`）。
- 迷你窗尺寸预设 **S/M/L/XL = 260×400 / 300×460 / 340×540 / 380×620**，运行时 `setSize(new LogicalSize(...))`（`MiniWidget.tsx:34-39,129`）。

#### 7.3.2 视觉语言：唯一 token 源 + **显式禁用 Tailwind 默认色板**

- token 源 = `src/index.css` 的 `@theme`（`:20`），并且：

```css
/* src/index.css:21 */
--color-*: initial;
```

  文档写明用意：`docs/DESIGN.md:34` —— “The default Tailwind palette is disabled (`--color-*: initial`). If a class like `bg-slate-100` slips in it will silently render nothing”。**没有 `tailwind.config` 文件**（Tailwind v4 CSS-first）。
  → ⚠️ 这是一个**双刃**决策：它确实防住了“随手用默认色”，但**失败模式是静默的**（正是 §7.6 Amica 那个 bug 的另一面）。**若我们要抄，必须配一个构建期检查**。
- **亮色单主题，无深色模式**（`grep prefers-color-scheme|darkMode|data-theme` 在 `index.css` 与 `App.tsx` 零命中）。
- 表面/墨色令牌：`surface-2 #ffffff`、`surface #fcfcfc`、`canvas #f4f4f5`、`well #f0f0f2`、`well-2 #e6e6e9`、`line #ebebee`、`line-2 #dcdce0`、`ink #1b1b1e`、`ink-2 #58585e`、`ink-3 #8b8b92`、`ink-4 #bbbbc2`（`index.css:27-41`）。
- 强调色是 **pastel 而非饱和主色**：amber `accent-300 #f3cd78` / `accent-500 #dea03a`（`index.css:44-51`）；另有 rose `peach-100 #fbe4e9`、lemon `honey-*`、mint `sage-*`、coral `clay-*`（`index.css:54-84`）。
- 字体：`--font-sans: "Figtree", …`、`--font-mono: "JetBrains Mono", …`，**自托管经 `@fontsource`**（`index.css:3-11,87-88`）→ **无 CDN 依赖**（对离线桌宠很关键）。
- **圆角 5 档令牌**：`--radius-control 0.75rem(12px)` / `--radius-field 0.75rem` / `--radius-card 1rem(16px)` / `--radius-panel 1.25rem(20px)` / `--radius-sheet 1.5rem(24px)`，并封进 `Surface` 的 `radius` 枚举（`index.css:91-95`、`ui/Surface.tsx:14-20`）。
- **阴影本质是 1px 描边而不是投影**：`--shadow-soft: 0 0 0 1px rgba(20,20,25,0.07)`、`--shadow-float`（…0.09）、`--shadow-pop: 0 0 0 1px … , 0 12px 32px -16px rgba(20,20,25,0.12)`、`--shadow-inset: none`（`index.css:101-104`）。设计原则原文：`docs/DESIGN.md:11` —— **“Flat, not floating. Nothing casts a drop shadow.”**
- 玻璃感**极克制**：只有舞台顶部名牌与字幕用 `backdrop-blur` + 90~95% 半透明表面（`App.tsx:538` `bg-surface-2/90 … backdrop-blur`；`MiniWidget.tsx:209` `bg-surface-2/95 … shadow-float backdrop-blur`）。
- **字号没有 token**：`@theme` 里无任何 `--text-*`，全部写成任意值 `text-[Npx]`。实测分布（基数：`src/**/*.tsx` 全量，共 52 处）：`11px`×15、`15px`×11、`13px`×11、`12px`×8、`22px`×2、`14px`×2、`10px`×2、`28px`×1；字号规范**只存在于文档**（`docs/DESIGN.md:40`）。
- **间距无成体系 token（未核到）**，靠 Tailwind 默认 4px 阶梯的 `gap-3 / p-3 / p-5 / px-8`。

#### 7.3.3 信息架构：模态 sheet + 左侧分组导航 + **单一类型枚举**

- **8 个页面**集中在**一个类型定义**里：`type SettingsPage = "profile" | "llm" | "tts" | "privacy" | "updates" | "expressions" | "memory" | "avatar";`（`Settings.tsx:52`）。
- 侧栏**分两组**，组标题为 **`Companion`** 与 **`You`**（`Settings.tsx:643,650`）：
  - Companion：**`Agent` / `Voice` / `Avatar on screen` / `Expressions` / `Memory`**（`Settings.tsx:98-104`）
  - You：**`Profile` / `Privacy & data` / `App updates`**（`Settings.tsx:106-110`）
- **代码 id 与用户可见术语已脱钩**：`SettingsPage` 的 id 是 `llm`/`tts`，显示名却是 `Agent`/`Voice`（`Settings.tsx:61-69 PAGE_META`）→ 迁移成本，但**页面标题+描述集中在 `PAGE_META` 是单点真相**（IA 可枚举、可测）。
- 主导航（非设置）：左轨道入口 `Conversation / Companions / Mini mode / Framing(full↔half) / Settings`（`Sidebar.tsx:39-83`）。
- **首次运行 onboarding = 7 步单列流程**：步骤标签 `["Start","You","Look","Personality","Voice","Connect"]` + 完成页（`OnboardingShell.tsx:6-16`，第 7 项标题 "See you on the desktop"）；顶部 6 个进度点（`:65-74`）；内容列宽 560px（第 3 步 640px）（`:83`）。
- 添加角色是**独立大模态**（`max-w-5xl`，两列网格，左表单右预览）：`h-[min(860px,92vh)] max-w-5xl` + `lg:grid-cols-[minmax(0,1fr)_minmax(260px,360px)]`（`AddCharacterModal.tsx:245-260`）。

#### 7.3.4 关键交互

- **模型切换**：`Companions` 弹出 `w-72` 浮层（`fixed left-20 top-20`，`elevation="pop"`）（`CharacterSelect.tsx:34-41`）；切换时**整体重置会话态**：`setMessages([])` + `clearQueue()` + 表情归 neutral + `setZoom(1.1)`（`App.tsx:348-358`），并靠 `key` 强制重挂画布避免状态串味：`key={\`vrm-${selectedCharId}\`}` / `key={\`l2d-${selectedCharId}\`}`（`App.tsx:77/80`）。
- **表情触发走 LLM 回复里的行内标记**，再从展示文本剥离：`cleanExpressionTags` 同时清 `<<...>>` 与 `[...]` / `[expression:xxx]`（`useChat.ts:78-80`）；表情经句子事件下发（`useChat.ts:375`），并挂在消息上供气泡旁显示（`useChat.ts:212-221`）。
- **音量 / 静音：未核到 UI 控件**（`grep volume|mute` 只命中 `getAudioLevels()` 的口型回传，`Live2DCanvas.tsx:20`）→ Meuxe **没有用户可调音量/静音**。
- **三套文本面且分场景**：① 舞台底部**浮动字幕卡**显示正在朗读的那一句（`spokenCaption` 取 `speakingSentence`，`App.tsx:412-417`；渲染于 `FloatingChatInput.tsx:64-71`）；② 迷你窗同一句字幕悬在输入区上方（`MiniWidget.tsx:207-216`）；③ 历史面板里是**只读消息气泡**（右侧 dock，`hideInput`，`App.tsx:619-621`）。
- **状态显示（单点计算 + 固定语义色）**：`listening → peach "Listening"`、`speaking → accent "Speaking"`、`isStreaming || (speechSessionActive && !speaking) → honey "Thinking"`，渲染为带呼吸点的小 Pill（`App.tsx:471-477, 538-547`）。迷你窗复用**同一映射**但换到右上角，且说话时若有字幕则让位给字幕（`MiniWidget.tsx:167 const showThinkingStatus = !caption && (...)`）。
- **工具确认（写操作）在迷你窗内就地弹出 honey 色审批卡（Allow / Deny）**（`MiniWidget.tsx:219-253`）。
- 输入：底部**圆角胶囊** `rounded-full`，麦克风左、ink 发送键右（`FloatingChatInput.tsx:74,90-93`）；流式中发送键变**停止键**（clay 底）（`:90-98`）。

#### 7.3.5 动效：纯 CSS，**无 framer-motion**

- 动效**令牌化在 `@theme`**：`--animate-fade-in .3s` / `rise-in .35s` / `pop-in .3s var(--ease-spring)` / `breathe 4s infinite` / `blink 5s` / `float 3.5s` / `dot 1.2s` / `pulse-soft 1.8s` / `listen-ring 1.6s`，配 **8 个 `@keyframes`**（`index.css:110-156`）；缓动 `--ease-soft: cubic-bezier(.4,0,.2,1)`、`--ease-spring: cubic-bezier(.2,.9,.3,1.2)`（`index.css:107-108`）。
- 面板开合：右面板 `animate-rise-in`（位移 10px + 淡入）（`HistoryDrawer.tsx:18`）；设置 sheet `animate-fade-in` 遮罩 + `animate-pop-in`（scale .96→1）（`Settings.tsx:624,635`）。
- 消息出现：`animate-in fade-in slide-in-from-bottom-1 duration-200`（`ChatPanel.tsx:166`）。
- 迷你窗底栏**悬停才出现**：`transition-opacity duration-200` + `opacity-0/100`（`MiniWidget.tsx:279-281`）。
- 拖拽阈值 **6px** 才 `startDragging()`（`MiniWidget.tsx:114`）；**无障碍**：吉祥物呼吸/眨眼均加 `motion-safe:`（`Mascot.tsx:45,65`）。

#### 7.3.6 明确的设计缺陷（本节此前缺证，现已补齐）

1. **引用不存在的色阶 → hover 静默失效（它自己的机制抓到的真实 bug）**：`hover:bg-clay-600` 出现 2 次（`FloatingChatInput.tsx:92`、`ChatComposer.tsx:153`），而 `@theme` 只定义 `clay-50/100/200/400/500/700`（`index.css:79-84`）且默认色板已被 `--color-*: initial` 清空（`:21`）→ 该 hover **不产生任何样式**。全仓枚举「被引用但未定义的 token 类」**只命中这一处**。
2. **字号无 token**：52 处 `text-[Npx]`，规范只在文档 → 无工具可强制。
3. **任意圆角绕过令牌**：`rounded-[9px]` / `[12px]`（`ToolCallBubble.tsx:132,151`）、`[6px]`（`ChatPanel.tsx:103`、`VRMCanvas.tsx:131`、`Live2DCanvas.tsx:141`）、`[7px]`（`ui/Kbd.tsx:8`）、`[10px]`（`ui/Button.tsx:19,67` 等）、`[13px]/[15px]/[18px]`（`agents/AgentPresetIcon.tsx:20-23`）、气泡尾 `rounded-tr-[10px]`（`ChatPanel.tsx:172-178`）—— 与 5 档圆角令牌并存，**无区分规则**。
4. **死代码造出第二套配色**：`ChatPanel` 有 `appearance: "light" | "dark"` 分支（`ChatPanel.tsx:35,267`），dark 用 `bg-white/10 text-white/90`（`:175`），但**唯一调用点写死 `appearance="light"`**（`App.tsx:621`）→ 整套深色气泡永不渲染。
5. **文档与实现互相矛盾**：用户气泡实际为 `bg-peach-100`（`ChatPanel.tsx:173`），而设计文档规定 `bg-accent-300 text-ink`（`docs/DESIGN.md:28`）。
6. **调色板外硬编码 hex 漏进组件**：`const previewBg = "#eef1f6";`（`CompanionAvatarPreview.tsx:17`）；把 canvas 值抄成字面量 `"#f4f4f5"`（`AvatarViewportSettings.tsx:6`）。
7. **emoji 残留为死数据**：`CompanionVibePack.emoji` 仍存 6 个 emoji（`lib/companionVibes.ts:15,23,31,39,47,55`），但渲染已换描边图标组件且该字段在 `.tsx` 中**零引用**（`ui/VibeGlyph.tsx:14` 注释 "replaces emoji"）→ **图标缺陷已修，数据未清**。
8. **手写动画工具类与 Tailwind 同名重复**（`index.css:253-268`，自述 "Legacy animation helpers"）：它改的是 `animation-duration` 而 Tailwind 原生 `duration-*` 管 `transition-duration`，**语义重叠、易误读**。
9. **正面结论（对照组）**：空态/加载态**不缺** —— `StageEmptyState` 区分「助手未就绪 / 可开聊」两态（`StageEmptyState.tsx:11-37`）；聊天空态含吉祥物+文案+ASCII 纹理（`ChatPanel.tsx:369-375`）；画布懒加载有 Suspense 文案（`App.tsx:70-74`）；配置加载有吉祥物+Dots（`App.tsx:443-450`）。

#### 7.3.7 可迁移的设计决策

1. **因为** Meuxe 用 `--color-*: initial` **显式禁用了 Tailwind 默认色板**，任何“随手用默认色”都会静默失效（`index.css:21`、`docs/DESIGN.md:34`），**所以我们应该**：在 Flutter 侧同样**不暴露框架默认色**，只暴露语义令牌；但**必须加构建期守卫**（因为它的失败模式是静默的）。
2. **因为** Meuxe 的阴影是 **1px 描边而非投影**（`--shadow-soft: 0 0 0 1px rgba(20,20,25,0.07)`），设计原则原文 “Flat, not floating. Nothing casts a drop shadow.”（`index.css:101-104`、`docs/DESIGN.md:11`），**所以我们应该**：深色 UI 上优先用**描边分层**而不是投影分层。
3. **因为** Meuxe 把**动效也做成令牌**（9 个 `--animate-*` + 2 条缓动曲线，`index.css:107-156`），**所以我们应该**：动效时长/曲线全部进令牌表，禁止在组件里写裸时长。
4. **因为** Meuxe 的状态映射集中在 `App.tsx:471-477` 一处、被主窗与迷你窗复用，**所以我们应该**：状态 → 颜色/文案的映射写成一个纯函数 + 单测。
5. **因为** Meuxe 切角色时**整体重置会话态并靠 `key` 强制重挂画布**（`App.tsx:77-80,348-358`），**所以我们应该**：模型切换必须显式重置所有与角色绑定的 UI 状态与渲染资源生命周期，不能靠“刷新一下就好”。
6. **因为** Meuxe 的迷你窗在**说话有字幕时让状态指示让位**（`MiniWidget.tsx:167`），**所以我们应该**：小尺寸形态下**同一位置只显示一个信息**，并规定明确优先级（字幕 > 状态 > 其他）。
7. **因为** Meuxe 的舞台是被 `gap` 与底色分隔的 `rounded-panel` **卡片**而非靠分割线（`App.tsx:480,505`），**所以我们应该**：让舞台作为“一张卡”参与布局，用**背景与间距**承担层级。
8. **因为** Meuxe 把「正在朗读的那一句」独立成浮动字幕卡、与历史气泡彻底分离（`App.tsx:412-417`），**所以我们应该**：区分「实时朗读字幕」与「历史消息」两种载体——避免流式字幕塞进消息列表引发重排与滚动抖动。
9. **因为** Meuxe 的 `SettingsPage` 枚举把代码 id 与显示名脱钩（`llm` vs `Agent`），**所以我们应该**：从一开始就让**导航项 id = 用户可见语义**。
10. **因为** Meuxe **没有音量/静音 UI**，**所以我们应该**：把它作为我方明确要领先的一处（我方 AGENTS.md 的“默认出声 + 主音量客户端设置”正好补这个缺口）。

---

### 7.4 my-neuro（morettt/my-neuro）—— “三套 UI 三套设计语言”的典型反例

> ⭐ 1,358，`pushed_at` 2026-09-09，JavaScript，**MIT**（`LICENSE:1-3`「Copyright (c) 2025 xxxiu」）。证据：本地源码（`live-2d/`、`webui/`）。
> ⚠️ **许可注意**：其 `live-2d/libs/live2dcubismcore.min.js` 是**随仓库分发的 Live2D Cubism Core 二进制**，仓库内**没有对应 notice 文件** → 该文件的许可声明**未核到**（与 §7.3 的 WebSDK 同理，Cubism 许可是独立一层）。

#### 7.4.1 布局：桌宠 = 全虚拟桌面透明穿透层

- 桌宠窗尺寸 = **所有显示器 bounds 的并集**（`minX/minY/totalWidth/totalHeight`），窗口选项：`transparent: true, frame: false, alwaysOnTop: true, backgroundColor: '#00000000', hasShadow: false, type: 'desktop', skipTaskbar: true, maximizable: false`（`live-2d/main.js:443-466`）；创建后还要反复重申并集边界以抵消被系统夹到 workArea（`:476-477`）。
- 鼠标穿透 `win.setIgnoreMouseEvents(true, { forward: true })`（`:468`）；置顶级别 `setAlwaysOnTop(true, 'screen-saver')`（`:467`）。
- 舞台层 `.avatar-container { position: fixed; inset: 0; width:100%; height:100% }`，body 透明 + `overflow:hidden`（`live-2d/css/styles.css:1-15`）。
- **聊天框是舞台内的固定悬浮块**：`#text-chat-container { position: fixed !important; bottom: 50px; right: 390px; width: 350px !important; max-height: 400px !important; z-index: 10000 !important }`，默认 `display:none; opacity:0` 由 UIController 决定何时显示以免闪现（`styles.css:112-127`）。
- **副屏感知布局**：副屏在主屏左侧时聊天框翻边：`body.screen-left #text-chat-container { right: auto; left: 20px }`（`styles.css:128-132`）。
- 字幕：`#subtitle-container { position: fixed; bottom: 20px; left: 70%; transform: translateX(-50%); max-width: 80%; max-height: 300px; border-radius: 10px; z-index: 1000 }`（`styles.css:79-95`）。
- 气泡：`#bubble-container { position: fixed; top:0; left:0; z-index:999; transition: none }`，位置由 JS 每帧写入（`styles.css:647-657`）。
- 控件面板：`#model-controls { position: fixed; z-index: 1001 }`，折叠用 `max-height: 0 → 320px` 过渡（`styles.css:134-138,151-158`）；圆形按钮 44×44 + `backdrop-filter: blur(10px)`（`:163-172`）。
- **设置窗是独立无边框窗**：`width:1080, height:760, minWidth:860, minHeight:620, frame:false, transparent:true, opacity:0, backgroundColor:'#00000000'`（`live-2d/control-main.js:579-590`），壳层 `border-radius:25px`（`control.html:13`）。
- 第三个窗口：72×72 的“换装加载中”跟随窗，`focusable:false, skipTaskbar:true, resizable:false, hasShadow:false`，位置按相对坐标（默认 `x=0.65, y=0.38`）换算（`control-main.js:42-59`）。
- WebUI 容器宽 `max-width: 1600px`（`webui/static/new/css/style.css:34-39`）；服务卡网格 `repeat(3, 1fr)` → 1200px 转 2 列 → 768px 转 1 列（`:273-281`）。
- 内置 Live2D 预览工作台是**左右分栏**：`grid-template-columns: minmax(320px, 520px) 1fr; gap: 18px`，900px 以下塌成单列（`:1472-1477,1538-1542`）；舞台 `min-height: 420px`。

#### 7.4.2 视觉语言：三套并存的系统（本项目最重要的一手发现）

**（a）WebUI 有真正的集中 token 系统，且是双主题。**

- 文件自述：「style.css 中的组件样式只引用这里的变量，做到主题无关」（`webui/static/new/css/themes.css:1-6`）。
- galgame（默认，浅色）：`--bg-primary:#fff6f8; --surface: rgba(255,255,255,0.6); --text-primary:#4d323d; --accent:#e75480; --accent-light:#ff9eb8`（`themes.css:11-23`）；追加 `--accent-2:#a78bfa; --grad-accent: linear-gradient(135deg,#ff8fab 0%,#e75480 55%,#a78bfa 115%); --ease-spring: cubic-bezier(0.34,1.56,0.64,1)`（`:105-111`）。
- cyber（深色）：`--bg-primary:#050814; --surface: rgba(10,15,31,0.7); --text-primary:#e0f2fe; --accent:#00f0ff`；圆角收紧 `--radius-md:8px`；字体换 `'Orbitron','Rajdhani'`（`:126-137,149,158,179`）。
- 语义色：`--green:#2ba471; --red:#e5484d; --blue:#4a86e8; --yellow:#d9a400; --orange:#ef8b3a`；**圆角四档** `--radius-sm:8px / -md:12px / -lg:18px / -xl:22px`；**阴影三档** `--shadow-sm/md/lg` + `--shadow-glow`；动效 `--transition: 0.2s cubic-bezier(0.2,0.8,0.2,1)`、`--transition-slow: 0.4s`（`themes.css:24-41`）。
- **字体栈令牌化**：`--font-heading / --font-body / --font-mono`（`themes.css:42-44`）。
- **把“形状”也令牌化**（少见）：`--clip-tab:none; --br-tab:999px; --clip-modal:none; --br-modal:20px`（`themes.css:59-72`）；cyber 里 `--clip-btn: polygon(8px 0,100% 0,100% calc(100% - 8px),calc(100% - 8px) 100%,0 100%,0 8px)`（`:179`）。
- 玻璃拟态：`.tab-content` 用 `backdrop-filter: blur(20px)` + `background: var(--surface)`（`new/css/style.css:175-187`）；卡片 `blur(12px)`、按钮 `blur(15px)`（`:292-293,318-319`）。
- **不彻底之处**：`style.css` 中 `var(--…)` 出现 **617 次**，但**仍有 28 个残留硬编码色**（`#f59e0b`×4、`#2563eb`×4、`#16a34a`×3 …）；圆角 `var(--radius-sm)`×18 / `var(--radius-md)`×16 旁边还有 `6px`×13、`8px`×3、`3px`×3、`2px`×2、`20px`×2 裸写；字号 **11/12/13/14/15/16/18/22px 与 `2.5em/1.3em/1em/0.8em` 混用**（13px 出现 39 次、12px 31 次、14px 21 次）。

**（b）Electron 桌宠叠加层（`css/styles.css`）：完全没有 token。**

`var(--…)` 出现 **0 次**，56 个不同硬编码色。四个子系统的语言互不相干：

| 子系统 | 语言 | 证据 |
| --- | --- | --- |
| 对话气泡 | 蓝灰卡通：`linear-gradient(145deg,#ffffff,#f0f4f8,#e3edf7)` + `2.5px solid #7eb3e0` + `border-radius:20px` + 三层阴影 + 文字 `#2c5aa0 14px` | `styles.css:658-687` |
| 字幕 | 白字黑描边 `30px`、`font-family:'Patrick Hand'`、`font-weight:900`、8 向 `text-shadow` | `styles.css:98-111` |
| 换装 spinner | 米褐色 `border-top-color:#8e8473` | `styles.css:47-56` |
| 快捷面板 | 深灰胶囊 `rgba(40,40,40,0.85)` + `border-radius:20px` + 白字 13px，**激活态却是绿色** `rgba(76,175,80,0.45)` | `styles.css:899-970` |
| 字幕拖拽态 | Material 蓝 `border: 2px dashed #2196F3` | `styles.css:994-1006` |

**（c）Electron 控制窗（`css/control-refined.css`）：也没有 token，且是“拼装件”。**

- `:root { color:#413a31; background: transparent }` 只设色不设令牌（`:1`）；文件内**自定义属性定义数 = 0**，出现的 15 次 `var(--…)` 全是**组件局部状态**（`--active` 索引、`--count`、`--height`、`--radius`），不是设计令牌（`:210-218,222-224,237-244`）；**209 个不同硬编码色**。
- 主色是**橄榄灰**：`.primary { color:#fff; background:#7d7362 }`、侧栏激活 `background:#817767`（`:87,:54`）。
- 文件里留有 7 处“借来的组件”出处注释：`Uiverse.io by TCdesign-dev`(:98)、`arthur_6104`(:106)、`bandirevanth`(:118)、`RaspberryBee`(:138)、`gharsh11032000`(:169)、`ilkhoeri`(:187)、`Admin12121`(:209)。

#### 7.4.3 信息架构：**同名功能两套叫法**

- **WebUI（默认新版）= 顶部 10 个横向 Tab**：`服务控制 / 基础配置 / 对话配置 / 人格设置 / LLM 配置 / 云端配置 / Live2D设置 / 声音克隆(非云版才显示) / 广场 / 插件管理`（`webui/templates/index_new.html:91-132`）。
  - 服务控制页内含：系统信息栏（WebUI 版本/运行时间/一键启动/一键停止）→ 服务卡片（`Live2D 主服务 / ASR 语音识别 / TTS 语音合成 / 记忆系统 / RAG 服务 / BERT 服务`）→ 总览看板（`对话管道`＝输入→上下文→LLM→输出、`最近动态`、`插件速览`）→ `LLM 耗时/Token`、`错误/警告趋势`、`CPU/内存` 三联图 → 多日志面板（系统日志/桌宠日志/工具调用/ASR/TTS/记忆/对话历史）（`index_new.html:136-381`）。
  - `Live2D设置` 页用**二级 Tab**：`UI 设置 / 动作 / 表情`（`:874-877`）；小节名逐字为 `🎵 唱歌控制`(:859)、`预览工作台`(:880)、`皮套形态与模型`(:904)、`动作与表情模式`(:930)、`动作绑定区域`(:992)、`表情绑定区域`(:1064)。
- **Electron 控制窗 = 左侧 180px 侧栏 + 10 页**：`.workspace { grid-template-columns: 180px 1fr }`（`control-refined.css:48`）；导航逐字为 `启动 / 模型配置 / 对话记录 / 插件 / 对话设置 / 表情与动作 / 终端控制室 / 工具屋 / 云端配置 / UI设置`（`control.html:61-72`）。
- **同名功能三套叫法**：控制窗叫“模型配置 / 表情与动作 / UI设置”，WebUI 叫“LLM 配置 / 动作 / 表情 / Live2D设置”（`control.html:64,68,72` vs `index_new.html:107,875-876,115`）。

#### 7.4.4 关键交互

- **模型切换是两步式**：先选**皮套形态**（`Live2D（2D 皮套）/ VRM（3D 模型）/ MMD（3D 模型 + 物理）/ PNGTuber（图片皮套）`）→ 应用形态 → 再选“该形态下的模型” → 应用模型；界面自述“Live2D 与 3D 形态之间切换时桌宠窗口会自动重载（约 20 秒）”（`index_new.html:904-925,918`）。
- **动作/表情三种运行模式**：`自动表情 + AI 编舞（推荐）/ 自动表情 + 传统动作 / 仅 AI 编舞` + 风格预设 `natural/lively/calm/shy`（`index_new.html:930-942,945-951`）；动作绑定/表情绑定各占一个二级 Tab。
- **待机动作组/表情**：预览工作台里两个下拉 + `预览待机` / `保存待机`（`:882-893`）。
- **字幕位置可交互调整**：`调整字幕位置` / `复位字幕位置` 按钮 + 拖拽调整态样式（`:926-928`、`styles.css:994-1006`）。
- **连接/运行状态**：服务卡标题前置状态点，running/stopped/waiting 的发光阴影**被令牌化**：`--shadow-status-running: 0 0 6px var(--green-glow)` / `-stopped: none` / `-waiting: 0 0 6px rgba(239,139,58,0.3)` + `--pulse-anim: statusPulseSoft 1.5s ease-in-out infinite`（`themes.css:91-99`）。
- **思考态：`未核到`**。反向证据是产品选择**不暴露思考过程**：`<thinking>` 块在展示前被直接删掉（`js/ai/llm-client.js:369-370`、`js/ai/llm-handler.js:31`）；桌宠层 CSS 内无 thinking/speaking/listening 类名。
- **音量/静音：`未核到` UI 控件**。WebUI 两个模板与 `control.html` 中“音量/静音/volume”命中 **0 处**；只在配置层存在于字段（`webui/config_manager.py:1123`、`js/voice/tts-request-handler.js:41`）。

#### 7.4.5 动效

- 面板切换：`.tab-content.active { display:block }` + `animation: fadeIn 0.4s ease both`（`translateY(8px)`→0）（`new/css/style.css:175-215`）。
- 页头入场 `fadeSlideDown 0.5s`（`:46-52`）。
- 换装：`#avatar-transition` `opacity .18s` + `transform .25s cubic-bezier(.2,.8,.2,1)`，模型 `opacity 0 / blur(9px) / scale(.97)`，`avatar-reveal .5s`（`styles.css:17-45`）。
- 气泡常驻漂浮 `animation: bubble-float-smooth 3s ease-in-out infinite`（`styles.css:686`）。
- 折叠用 `max-height` 过渡（`styles.css:151-158`）；快捷面板 `transition: max-height .3s ease, opacity .25s ease`，齿轮 `#quick-gear.open { transform: rotate(90deg) }`（`:918-925,984-989`）。
- **控制窗用原生 View Transitions API**：`.content { view-transition-name: page-content }` + `::view-transition-old(page-content) { animation: page-leave .16s }` + `::view-transition-new(page-content) { animation: page-view-enter .3s }`（`control-refined.css:55,65-74`），并**尊重 `@media (prefers-reduced-motion: reduce)`**（`:75-78`）。

#### 7.4.6 明确的设计缺陷（控制窗最严重）

1. **同一文件里 7 个不同作者的外部组件拼装**，直接解释 209 个硬编码色（出处注释见 §7.4.2c）。
2. **一个页面加载三份互相覆盖的样式表**：`control.css` + `control-simple.css` + `control-refined.css`，再加 22 行内联 `<style>`（`control.html:7-9,10-31`）。
3. **同屏最多 4 种强调色**：refined 主色橄榄灰 `#7d7362/#817767`，但借来的启动按钮是蓝灰新拟态 `#fcfcfd/#d6d6e7/#36395a`、保存按钮脏态是**明黄** `#eab308/#facc15`、rocker 开关勾选态是**亮蓝** `#0084d0`（`control-refined.css:99,107,108,129,87,54`）。
4. **硬编码 `#000/#fff` 当“反色”**：`.tab-button.active { color:#000 }` + `svg { stroke:#000 }`，在 cyber 深色主题下靠反向补丁 `html[data-theme="galgame"] .tab-button.active svg { stroke:#fff }` 覆盖（`new/css/style.css:153-160` vs `themes.css:506`）。
5. **基础层带深色假设、浅色主题靠高特异性覆盖**：基类滚动条是白 12% `rgba(255,255,255,0.12)`，galgame 再覆盖（`new/css/style.css:28-31` vs `themes.css:512-513`）。
6. **字幕定位可疑**：`left: 70%` 写死居中锚点，`max-width: 80%` 在窄屏会溢出；同文件另有 `overflow-y: hidden /* 可能的丢包问题 */`（把缺陷写进注释）（`styles.css:79-95`）。
7. **emoji 当图标**：`🎵 唱歌控制`、`🐂 云端肥牛配置`、`🔊 云端 TTS 配置`、`🎤 百度流式 ASR 配置`（`index_new.html:859,547,587,690`），而 Tab 图标是 SVG → **同界面两套图标语言**。
8. **页面 h1 被 CSS 隐藏**：HTML 里写了 9 个 h1，但 `.page-head > div { display:none }` 把标题容器整体隐藏，留下 `.page-head h1 { font-size:25px }` 这类作用在隐藏元素上的死规则（`control.html:91-133` vs `control-refined.css:79-82`）。
9. **状态变量映射重复两份**：`--active` 的 nth-child 映射既在 CSS（`.chat-tabs:has(button.active:nth-child(n))`）又在页面内联 style 里再写一遍（`control-refined.css:223-224` vs `control.html:18-20`）。
10. **魔法偏移**：主启动按钮 `left:-24px; grid-column:2; width:250px; height:56px`（`control-refined.css:99`）。
11. 令牌化不彻底（§7.4.2a 末尾）。

#### 7.4.7 可迁移的设计决策

1. **因为** my-neuro 用“覆盖全虚拟桌面的透明点击穿透窗”承载舞台，从而躲开了多显示器窗口拼接（`main.js:443-477`），**所以我们应该**：在“单窗全屏穿透层”与“多窗口”之间先做显式决策——前者布局简单且天然跨屏，代价是**所有 UI 共用一个 z-index 战场**（它已经出现 10000/9000/1001/999 混用）。
2. **因为** 它把**形状也令牌化**（`--clip-*` 与 `--br-*` 成对，`themes.css:59-72,179`），**所以我们应该**：主题令牌包含圆角/切角/发光/字距，才能“换主题不改组件”。
3. **因为** 三套 UI 用了三套色系与三套命名（WebUI 粉 `#e75480` / 桌宠层蓝 `#7eb3e0` / 控制窗橄榄灰 `#7d7362`；同名功能三种叫法），**所以我们应该**：显式规定“唯一 UI 宿主”，遗留第二前端只读不写——这与本项目 AGENTS.md 把原生 JS 前端降级为遗留的判断一致。
4. **因为** 引入 7 个外部组件带来 209 个硬编码色与 4 个并列强调色，**所以我们应该**：任何外部组件必须“过令牌化”才允许合入，禁止直接粘贴带死色的样式。
5. **因为** 折叠面板用 `max-height: 0 → 320px` + `transition` 即可展开收起，无需 JS 测高（`styles.css:151-158`），**所以我们应该**：小面板优先用高度/透明度过渡，把 JS 留给状态。
6. **因为** 音量/静音在 my-neuro 只存在于配置字段而**没有 UI**，**所以我们应该**：主音量/静音必须是一级可见控件（我方 AGENTS.md 已裁决）。
7. **因为** 它把服务状态做成“状态点 + 令牌化发光阴影”（`themes.css:91-99`），**所以我们应该**：连接/思考/说话态共用同一套“点 + 发光令牌”。

---

### 7.5 Live2DPet（x380kkm/Live2DPet）—— “零令牌”的极限样本

> ⭐ 81，`pushed_at` 2026-06-22，JavaScript，**MIT**（`LICENSE:1-3`）。
> **取证提示**：`main.js` 只是装配器，**全部窗口几何在 `src/main/window-manager.js`**。
> 第三方 VOICEVOX/VVM/Open JTalk **不随仓库分发**（运行时下载，`THIRD-PARTY-NOTICES:3-5`），但产物侧有署名义务「VOICEVOX:[character name]」（`:12`）→ 代码复用**无 copyleft 障碍**。

#### 7.5.1 布局：三窗完全独立，气泡是唯一的文本面

| 窗口 | 尺寸/装饰 | 证据 |
| --- | --- | --- |
| 设置窗 | `width:480, height:600, frame:true, resizable:false` | `src/main/window-manager.js:25-29` |
| 桌宠窗 | `width:300, height:300, frame:false, transparent:true, alwaysOnTop:true, resizable:true, minimizable:false, maximizable:false, fullscreenable:false, skipTaskbar:true` | `window-manager.js:58-62` |
| 气泡窗 | `width:250, height:80, frame:false, transparent:true, alwaysOnTop:true, focusable:false, show:false` | `window-manager.js:178-185` |

- 三窗各自 `loadFile`：`index.html` / `desktop-pet.html` / `pet-chat-bubble.html`，**无 iframe、无多视图，1 窗 = 1 HTML = 1 职责**（`window-manager.js:36,71,194`）。
- 置顶级别与全空间可见：`setAlwaysOnTop(true,'screen-saver')` + `setVisibleOnAllWorkspaces(true,{visibleOnFullScreen:true})`（气泡窗同）（`window-manager.js:69-70,192-193`）。
- 桌宠落点：`ctx.petWindow.setPosition(width - 220, height - 220)`（主屏 workArea）——300px 窗右/下各溢出 80px，**出厂即有一角在屏外**（`window-manager.js:75-77`）。
- 气泡锚定在**宠物窗 bounds** 而不是屏幕坐标：`x: petBounds.x + (petBounds.width - 250)/2`、`y: petBounds.y - 80 + petBounds.height*0.25`（`window-manager.js:180-181`）→ 【推断】气泡水平居中于桌宠、底边钉在宠物窗高度 25% 处。
- 气泡尺寸反被内容驱动：`Math.min(Math.max(160, Math.ceil(rect.width)+50), 300)` / `Math.ceil(rect.height)+60`（`src/renderer/pet-chat-bubble.js:59-60`）。
- 渲染层：`body` 透明、禁滚禁选；Live2D 锚点 `anchor.set(0.5,0.5)`、`x=w/2`、`y=h*(canvasYRatio || 0.60)`（`desktop-pet.html:10-17`、`src/renderer/model-adapter.js:80,112-113`）。
- 窗内控件：右下悬浮条，hover 才显现 `opacity: 0 → #pet-container:hover .controls { opacity:1 }`（`desktop-pet.html:39,42,45`）。
- 窗内**无侧栏、无栅格、无折叠**：设置窗是单列卡片堆，仅两处内部滚动（`max-height:400px; overflow-y:auto`）（`index.html:250,381`）。

#### 7.5.2 视觉语言：全仓库没有任何 token（最强的反例）

- **零 `.css` 文件**（非 `node_modules` 下 `find -name "*.css"` 为空），样式 100% 内嵌；三个 HTML 中 `:root` 命中 **0**、`var(--` 命中 **0**。
- 浅色固定主题：`background:#f5f5f5; color:#333`；**无 `prefers-color-scheme`、无 `backdrop-filter`**（命中 0）（`index.html:10`）。
- 主强调色紫 `#7c4dff`（focus 边框、主按钮、激活 tab），hover `#651fff`，危险 `#ff5252`（`index.html:27,32,33,36,57`）。
- 语义状态三对：success `#e8f5e9/#2e7d32`、error `#ffebee/#c62828`、info `#e3f2fd/#1565c0`（`index.html:64-66`）。
- **第二个蓝**：页脚链接行内 `style="color:#58a6ff"`（`index.html:196`）。
- **全仓库仅 1 档阴影**：`box-shadow: 0 1px 3px rgba(0,0,0,0.1)`（`index.html:18`）。
- 圆角四档：卡片 12px、输入/按钮/tab 8px、状态/图标 4px、宠物圆钮 50%（`index.html:16,23,29,42,54-55`、`desktop-pet.html:48`）。
- 间距无体系：body 20 / 卡片 16 / 卡距 12 / gap 8 / 微 4、3（px）（`index.html:11,17,18,39,70`）。
- 字体两套栈：设置窗 `'Microsoft YaHei','Segoe UI',sans-serif` vs 气泡 `'Microsoft YaHei',Arial,sans-serif`（`index.html:9`、`pet-chat-bubble.html:18`）。
- 字号 px 标度 18/14/13/12/11，**无 rem/em**；`index.html` 内联 `<style>` 用 **20 个不同硬编码色**，另有 **33 处** `style="…"` 行内样式。

#### 7.5.3 信息架构：顶部 5 标签 + 单列卡片

- 顶部标签（**等宽 `flex:1`**，圆角只给首尾拼成胶囊段）逐字：**`Settings / Model / Emotion / TTS / Prompt`**（`index.html:48-59,126-131`；中文 i18n 为 `设置 / 模型 / 表情 / TTS / Prompt`，`src/i18n/locales.js:254`）。
- 各标签内小节名（h2 逐字）：
  - **Settings** = `API Configuration` / `Translation API (for TTS)` / `Response Length` / `Detection Settings` + 3 张无标题卡（`index.html:137,152,164,175`）
  - **Model** = `Model Mode` / `Live2D Model` / `Parameter Mapping` / `Canvas Y Anchor` / `Image Folder` / `Bubble Frame` / `App Icon`（`:203,212,223,230,237,254,262`）
  - **Emotion** = `Expression Frequency` / `Expression List` / `Motion List`（`:280,290,299`）
  - **TTS** = `VOICEVOX TTS` / `Audio Mode` / `Test` / `Default Audio` / `Voice Model (VVM)`（`:320,335,361,367,379`）
  - **Prompt** = `Character Card` / `Character Name` / `Character Identity` / `Address Term` / `Description` / `Personality` / `Scenario` / `Hard Rules` / `Response Language` / `Interaction Descriptions`（`:390,408,412,417,422,426,430,434,438,442`）
- 辅助导航：托盘菜单、桌宠右键 `Size`（200/300/400/500）（`src/main/utility-ipc.js:43-49`、`src/i18n/locales.js:250`）。
- **孤立 i18n 键**：`'tab.enhance'` 在英/中/日三语都定义（`locales.js:8,254,500`），但只有 5 个 tab 按钮（`grep -c 'class="tab-btn'` = 5）→ **翻译先行、界面漏做**。

#### 7.5.4 关键交互

- **模型切换 = 单下拉三选一**（`No Model (Guide) / Live2D / Image Folder`）→ 保存后主进程推热重载重建适配器（`index.html:203-207`；`utility-ipc.js:31` `send('model-config-update', …)`）。
- **表情/动作：只有列表 + 时长/频率，无手动试播按钮**（对比 N.E.K.O 的 ▶，§3.2）；触发走 `set-talking-state` → `currentAdapter.playMotion(group, index)`（`settings-ui.js:593,645`、`emotion-ipc.js:69`、`desktop-pet.html:225`）。
- **音量/静音**：无独立 mute 开关，等于音量滑杆 `0.0–2.0` + `Audio Mode` 三选一（含 Silent）（`index.html:337-339,355`）。
- **文本面只有一种 = 独立气泡窗**：无字幕、无浮动 caption、无窗内文字层；气泡存活时长常量 `AUDIO_BUBBLE_BUFFER_MS = 800` / `DEFAULT_BUBBLE_MS = 8000` / `MIN_BUBBLE_MS = 3000`（`src/core/message-session.js:18,20,22`）。
- **三态呈现**：连接＝一行文字（`settings-ui.js:150`）；**思考＝完全无 UI**（只剥 `<thinking>` 标签，`src/core/ai-chat.js:111`）；说话＝切换图包，代码注释 `// Priority: emotion > talking > idle`（`model-adapter.js:340-343`）。

#### 7.5.5 动效

- CSS `transition` 只有 **5 条**（`border-color .2s`、`all .2s`、`opacity .3s` 等），**缓动全部缺省 `ease`**，无自定义 cubic-bezier（`index.html:25,30,44,52`、`desktop-pet.html:42`）。
- `@keyframes` 只有气泡窗 3 个：`textFadeIn` / `fadeIn` / `fadeOut`，应用为 `fadeIn .3s ease-out`、`textFadeIn .4s ease-out .15s forwards`（`pet-chat-bubble.html:41-46,59-75`）。
- **无 JS 动画库**（`package.json:14-18` 仅 `active-win`/`electron-log`/`koffi`）；渲染动效由 PIXI ticker 驱动，**表情是逐帧直写参数而非补间**（`model-adapter.js:96-99`）。
- **无窗口级动画**：无 `setOpacity`/淡入淡出，靠 `transparent` + `showInactive()`（`window-manager.js:229`）。
- 鼠标跟随是 **50ms 离散采样**而非弹簧：`(cursor.x - cx)/300`，`}, 50)`（`desktop-pet.html:137-143`）。

#### 7.5.6 明确的设计缺陷

1. **“进行中”状态永久不可见（最硬的一条）**：`showStatus` 拼 `el.className = 'status ' + type`，而两处传空 type（`showStatus('tts-test-status', t('tts.synthesizing'), '')`、`showStatus('default-audio-status', t('tts.generating'), '')`）得到 `class="status "`；基类 `.status { display:none }` 只在 `.success/.error/.info` 才 `display:block` → **合成中/生成中的提示永远渲染不出来**（`settings-ui.js:119-123,1062,1133` + `index.html:60-66`）。
2. **状态定时器不清理**：`if (type !== 'info') setTimeout(() => { el.className='status' }, 5000)`，连续操作会互相清屏（`settings-ui.js:123`）。
3. **气泡框被拉伸变形**：内置 `assets/dialog-frame.png` 实测 **1920×1080（16:9）**，却被 `object-fit: fill` 塞进 250×80（≈3:1）窗口（`pet-chat-bubble.html:37` + `window-manager.js:179`）——**九宫格缺失的典型后果**。
4. **padding 双计**：气泡文本 `padding: 30px 25px`（`pet-chat-bubble.html:49`），而窗口高度由 JS `rect.height + 60` 反算（`pet-chat-bubble.js:60`）→ 【推断】留白被算两遍。
5. **文字初始不可见依赖动画跑成**：`.message-text { opacity: 0 }` + 靠 `.text-in` 动画 `forwards` 显示 → 【推断】`prefers-reduced-motion` 或动画失败即永久空白（`pet-chat-bubble.html:38,41-43`）。
6. **两个竞争强调色 + 两套成功绿**：`#7c4dff` vs `#58a6ff`（`index.html:32,196`）；`#2e7d32` vs 行内 `#4a4`（`settings-ui.js:1196`）。
7. **emoji/符号当图标**：宠物悬浮按钮 `⚙`、列表编辑/删除/还原 `✎ ✕ ⟲`（`desktop-pet.html:65`、`index.html:396-399`）。
8. **无加载态/骨架屏**：全仓库无 spinner/loader/skeleton；只有列表文字空态（`settings-ui.js:581-583`）。
9. **反馈与控件脱节**：Model 页 8 张卡的操作结果全部写进第一张卡里的状态元素（`index.html:209`）。
10. **内联样式泛滥**：`index.html` 33 处 `style="`，另有 17 处由 JS 写入。
11. **i18n 覆盖不全**：`<title>Desktop Pet Settings</title>`、`<h1>Desktop Pet</h1>` 硬编码英文（`index.html:5,124`）。

#### 7.5.7 可迁移的设计决策

1. **因为** 每个窗口独立 `loadFile` 一个职责单一的 HTML（`window-manager.js:36,71,194`），**所以我们应该**：坚持“1 OS 窗 = 1 职责”，让透明桌宠窗与浅色设置窗的样式互不污染。
2. **因为** 气泡位置由宠物窗 bounds 反算（`window-manager.js:180-181`），**所以我们应该**：把“跟随锚点”收敛成一个纯函数并做单测（角色缩放/贴边时气泡才不漂出屏外）。
3. **因为** 气泡窗 `focusable:false` + `showInactive()`（`window-manager.js:184,229`），**所以我们应该**：把“消息气泡永不抢焦点”写成桌宠默认契约。
4. **因为** 它 0 个 CSS 变量、单块 `<style>` 里 20 个硬编码色、并存 `#7c4dff` 与 `#58a6ff`，**所以我们应该**：第一天就立 `:root` 令牌（1 主色 + 3 语义 + 3 圆角 + 1~3 阴影）——这是本调查里最省事的“不要重演”。
5. **因为** 进行中状态因空 type 撞上 `display:none` 而永久隐身（`settings-ui.js:1062,1133` + `index.html:60-66`），**所以我们应该**：状态组件只接受枚举（`success|error|info|progress`）、**默认可见**，且每个控件绑定自己的局部状态位。
6. **因为** 16:9 气泡框被 `object-fit: fill` 压进 3:1 窗口，**所以我们应该**：气泡框用九宫格（Flutter 用 `DecorationImage(centerSlice:)`）或让框比例随内容，并由布局只计算内容区尺寸。
7. **因为** 动效只有 5 条 transition + 3 个 keyframes 且无动画库，**所以我们应该**：继续“CSS/隐式动画做 UI 反馈 + 渲染库 ticker 做角色动效”的轻量路线，但**补一条窗口级淡入淡出**（它当前气泡是硬切）。
8. **因为** `tab.enhance` 三语都翻译了却没有对应 tab（`locales.js:8,254,500` vs 5 个 tab 按钮），**所以我们应该**：把 i18n 键的完整性当作 IA 契约来测（有翻译无控件即报错）。

---

### 7.6 ChatVRM（pixiv/ChatVRM）与 Amica（semperai/amica）—— 两个“没有设计系统”的 VRM 样本

> ChatVRM ⭐846、`pushed_at` 2025-05-27、Next.js 13 + Tailwind 3.3；Amica ⭐1,593、`pushed_at` 2025-07-23、Next.js 14 + Tailwind 3.4。**两者均 MIT**（`LICENSE:1-3`）；Amica 是 ChatVRM 的硬 fork（tailwind 颜色块逐字节相同）。`awesome-ai-companion` 把 Amica 标注为 **Unmaintained**。

#### 7.6.1 布局：舞台铺满 + 一堆绝对定位浮层（没有真正的分栏）

- ChatVRM：舞台 `absolute top-0 left-0 w-screen h-[100svh] -z-10`（`vrmViewer.tsx:45`），其余全部是绝对定位浮层；设置是**全屏模态** `absolute z-40 w-full h-full bg-white/80 backdrop-blur`（`settings.tsx:47`）。**面板从不引起渲染面重排。**
- Amica：同样固定全视口舞台（`z-1 fixed left-0 top-0 h-full w-full`，`vrmViewer.tsx:103`），但“chat mode”额外加 `left-[65%] top-[50%]`（`:104`）**并且**把渲染器分辨率减半（`width = width/2; height = height/2`，`viewer.ts:967-970`）而 canvas 仍是 `h-full w-full` → **不是分栏，是半分辨率 + 右移出屏**；且 `left-0` 与 `left-[65%]` 同时存在，谁生效取决于 CSS 顺序。

#### 7.6.2 视觉语言：Amica 静默丢掉了 ChatVRM 依赖的 Tailwind preset

- ChatVRM `tailwind.config.js:9-16` 有 `presets:[createTailwindConfig({version:"v3",theme:{":root":light}})]` —— `typography-N`、`rounded-4/8/16`、`w-col-span-N`、`bg-surfaceN` 全部由这个 `@charcoal-ui/tailwind-config` preset 定义。
- Amica 的 `tailwind.config.js` **没有 `presets` 键**（只有 `plugins:[@tailwindcss/forms]`，`:28-30`），`:1` 的 `import {light, dark}` 是死代码。
- **后果（约 14 处类名渲染为零样式）**：`typography-16` ×7（`assistantText.tsx:40`、`chatModeText.tsx:96,104`、`subconciousText.tsx:62,69`、`chatLog.tsx:204`、`userText.tsx:30`）、`typography-20`（`introduction.tsx:38`）、`w-col-span-6`（`chatLog.tsx:127`、`subconciousText.tsx:17`）、`rounded-4`（`BackgroundImgPage.tsx:37,46`、`CharacterModelPage.tsx:62,71`）、`text-text1`（`settings.tsx:818`）。
- **同族 bug**：`bg-rose/90` 是**全部四个消息卡**的标题栏（`assistantText.tsx:25`、`userText.tsx:23`、`chatModeText.tsx:70`、`subconciousText.tsx:52`），但裸 `rose` 不在 Tailwind 默认色板里 → **透明标题栏 + 白字**；而 `common.tsx:265` 用的是正确的 `rose-600`。另有 `z-index-50`（`settings.tsx:762`）、`z-1`（`vrmViewer.tsx:103`）、`active:bg-primary-press-press`（`textButton.tsx:8`）、`active:bg-secondary-active`（`CharacterModelPage.tsx:77`）均无效。
- **`@charcoal-ui` preset 的实际数值：未核到**（checkout 里没有 `node_modules`，preset 未 vendored）→ 因此这两个项目的字号/圆角/间距**具体数值无法断言**。
- **零阴影、零动效（ChatVRM）**：`grep "shadow" src/` = 0 命中；`grep transition|animate-|duration-|ease-|framer` = 0 命中，`package.json` 无 framer-motion；唯一动效是 `scrollIntoView smooth`（`chatLog.tsx:17-20`）。Amica 加了 `@headlessui Transition`（`settings.tsx:860-869`、`alert.tsx:36-45`）与开关过渡（`switchBox.tsx:38,48`），但**设置/日志面板仍然无过渡**。
- **Amica 的强调色至少 6 族**：token 紫 `#856292` / 粉 `#FF617F`（`messageInput.tsx:269`）+ `pink-600/80` 名牌（`assistantText.tsx:26`）+ `cyan-600/80`（`userText.tsx:24`）+ `indigo-600` 开关（`switchBox.tsx:37`）+ `blue-500/90` 思考头（`thoughtText.tsx:32`）+ `slate-600` 日志按钮（`chatLog.tsx:106`）+ `teal` focus ring `rgba(35,167,195,0.3)` 与 `#b6c3c6` 边框（`flexTextarea.module.css:34,39`）。
- **圆角混乱（ChatVRM）**：`rounded-16 / rounded-8 / rounded-4 / rounded-oval / border-radius:4px` 同产品并存。
- **字体加载**：ChatVRM 从 Google Fonts CDN 拉字体（`_document.tsx:15`）；Amica 有**三套并行机制**（Google Fonts `<link>`、`next/font` 变量、tailwind 裸字体名）→ 对本项目“离线优先”的红线是直接的反面教材。
- **emoji 当图标：两个项目都不存在**（Unicode emoji 范围 grep 在两边 `src/` 均 0 命中）；Amica 用 heroicons + tabler + charcoal 图标集，ChatVRM 用 charcoal 的 `pixiv-icon`。

#### 7.6.3 信息架构

- ChatVRM 设置 = **一个扁平滚动页**（`settings.tsx:57-223`，5 个小节，无 tab、无面包屑）。
- Amica 设置 = **11 项主菜单 + 嵌套 MenuPage + 面包屑 + 返回按钮**（`settings.tsx:356-387,769-844`），命名**以 provider 名为先**（标签见 `common.tsx:195-247`）。

#### 7.6.4 关键交互

- **模型切换**：Amica 的进度回调是**显式 no-op TODO**（`CharacterModelPage.tsx:44-46` `// TODO handle loading progress`），而 `vrmViewer` 另有自己的加载浮层（`vrmViewer.tsx:107-114`）；ChatVRM **完全没有** VRM 加载/错误状态（`vrmViewer.tsx:12`）。
- **表情**：ChatVRM 解析 `[tag]`（`index.tsx:130-134`）；Amica 更进一步——表情名**在运行时从已加载的 VRM 里发现**（`expressionController.ts:38-55`），并在 LLM 漏标签时**主动注入 `[neutral]`**（`chat.ts:396`）。
- **音量/静音**：ChatVRM **完全没有**（`grep volume|mute` 只命中 lipSync 内部）；Amica 只有 mute、没有音量（`index.tsx:199-202,410-424`）。
- **`chatSpeaking` 状态被创建并传进 context 但从未渲染**（`index.tsx:124,309,325`）→ **没有“说话中”指示**。
- **Amica 有 4 个近乎重复的消息卡组件**（`assistantText` / `userText` / `chatModeText` / `subconciousText`），其中两个的展开逻辑**方向相反**：`assistantText.tsx:38` `unlimited ? max-h-[75vh] : max-h-32` vs `chatModeText.tsx:94` `unlimited ? max-h-32 : max-h-[75vh]`。
- **连接状态指示器：两个项目都没有**（Amica 源码中不存在对应组件或文案）。

#### 7.6.5 可迁移的设计决策

1. **因为** Amica 丢掉了 `charcoal-ui` preset，导致约 14 处类名**静默变成零样式**，而构建不报错（`tailwind.config.js` 无 `presets`），**所以我们应该**：设计令牌必须由**我们自己的构建产物**保证存在，并加“未解析令牌即失败”的构建期/测试期守卫——**静默失败是令牌系统最危险的失败模式**。
2. **因为** Amica 的“chat mode”是靠**把渲染分辨率减半 + 右移 65%** 实现（`vrmViewer.tsx:104` + `viewer.ts:967-970`），**所以我们应该**：舞台与面板的分栏必须由**真实布局容器（Flex/Grid）+ 实测尺寸**驱动，渲染分辨率永远跟随实际画布尺寸。
3. **因为** ChatVRM/Amica 的设计令牌依赖**外部 npm preset**并且没有 vendored，**所以我们应该**：令牌源必须在仓库内、可被 diff、可被单测（Flutter 侧就是一份 Dart 常量文件 + 一个 golden 测试）。
4. **因为** ChatVRM 从 Google Fonts CDN 拉字体、Amica 有三套并行字体机制，**所以我们应该**：字体一律自托管（这与本项目 AGENTS.md 里 Flutter 必须 `--no-web-resources-cdn` 的要求同源）。
5. **因为** ChatVRM 的加载/错误状态完全缺失、Amica 的进度回调是 TODO（`CharacterModelPage.tsx:44-46`），**所以我们应该**：**模型导入/切换**是本产品最高风险的操作，必须有“进度 + 失败原因 + 重试”三件套。
6. **因为** Amica 从已加载模型**运行时发现表情名**并在模型漏标签时注入 `[neutral]`（`expressionController.ts:38-55`、`chat.ts:396`），**所以我们应该**：动作/表情清单来自模型自身能力（而不是硬编码枚举），并在状态机侧保证“无指令时回到中性”。

---

### 7.7 super-agent-party（heshengtao/super-agent-party）—— **AGPL-3.0 的同类**

> ⭐ 2,642，`pushed_at` 2026-08-23，JavaScript/Python，**AGPL-3.0**（GitHub API）——与本项目**同许可**，复用摩擦最小。

#### 7.7.1 许可（同许可但**不等于可以随便搬**）

- **代码主体 AGPL-3.0**：`LICENSE:1-4` + `package.json:31`。→ 与本项目**同许可**，代码可吸收。
- ⚠️ 但 `LICENSE-third-party/` 下有 **6 个独立约束文件**：**Font Awesome 要求署名**（CC BY 4.0 / OFL / MIT 混合）、**Open Runde 字体 OFL 有保留字体名条款**、脏词表 CC BY 4.0、**Silero VAD 是 MIT**。→ 搬 `static/fontawesome/` 或 `fonts/` **必须同时搬这些清单**。

#### 7.7.2 布局：**主窗做对话，形象在另一个独立透明窗**

- **分栏比例明确**：`.chat-area { width: 50% }` / `.side-panel { width: 50% }`（`static/css/styles.css:4068` / `:4523`）→ **左右各半**。
- **一级侧栏可折叠**：200px → 64px（`static/js/renderer.js:1752-1758`）；二级磁贴列 220px（`styles.css:2919`）。
- ⭐ **头像（VRM）不在主窗内**：它是一个**独立的 always-on-top 透明窗**，默认 **540×960**（`main.js:2017-2022`）。→ 这是“**控制台与形象分离**”的最彻底实现（比 Warudo 的双窗更彻底：这里形象窗本身就是桌面常驻层）。
- `static/` 下存在多个独立浮层入口：`index.html`、`chat.html`、`subtitle_overlay.html`、`danmaku_overlay.html`、`island.html`、`soulx.html`、`tha.html`；样式分文件在 `static/css/`（含 `island.css`）。

#### 7.7.3 视觉语言：token 完备（9 套主题 × 60+ 变量），但与 362 个硬编码 hex **长期并存**

- 主色：亮色 `--el-color-primary: #17827a`（`static/css/styles.css:21`）、暗色 `#c8815a`（`styles.css:79`）；共 **9 套主题**，定义在 `static/js/vue_data.js:562` 的 `themeValues`。
- **`!important` 1,514 处、内联 `style="` 495 处**、去重硬编码 hex **362 个** → 【推断】token 系统是“后来加的”，旧样式没有回迁。
- 自带 `static/fontawesome/` **图标字体**（而不是 emoji）。

#### 7.7.4 信息架构：一级导航 11 项 + 系统设置 4 节

- **一级导航 11 项**（`index.html:500-572`）：`仪表盘 / 对话 / 模型 / 工具 / 角色 / 机器人 / AI浏览器 / 开发者 / 存储空间 / 系统设置 / 关于我们`。
- **系统设置 = 4 节**（`static/js/vue_data.js:1437-1442` + `zh-CN.js:117,118,1797,2035`）：**`通用 / 外观 / 快捷指令 / 账户管理`**。

#### 7.7.5 关键交互

- ⚠️ **「静音」是 `audio.volume = 0.0000001` 的 hack**（`static/js/vue_methods.js:10877,11091,14316,15769`）；并且**主 UI 根本没有音量滑杆**（8 处 `el-slider` 全与音量无关，`vue_data.js` 无 `volume` 字段）。→ **音量/静音是本次调研里第 4 个栽在这里的项目**（§8 #6）。
- 对照 **Ghost Vessel 的正确做法**：`toggleVoice()` 关麦即 `killAudio()` 并持久化（`player/index.html:709-711,1441`）→ **直接采纳 GV，并把「渲染窗 = 唯一音频源」定为架构规则**。

#### 7.7.6 明确的设计缺陷（**4 处未定义/拼错的 CSS 变量导致样式静默失效**）

1. `var(--primary-color)` **用了 9 次，全仓无定义** → `styles.css:1272-1273` 的菜单 hover/active 主色**实际无效**。
2. `var(--el-primary-color)` **拼错**（正确为 `--el-color-primary`），用 5 次 → `styles.css:3844` 的 `.tts-status` 不生效。
3. `--card-bg-color` / `--code-bg-color` / `--preview-bg-color` / `--el-color-primary-light-8` 均**无定义**。
4. `!important` **1,514 处** + 内联 `style="` **495 处**。
5. 音量/静音 hack（§7.7.5）。
   → **这与 Amica 的 preset 丢失、Meuxe 的 `clay-600`、Live2DPet 的空 type 是同一族缺陷（§8 #4）**：**静默失效的令牌引用**。

#### 7.7.7 可迁移的设计决策

1. **因为** SAP 把**形象放进独立的 always-on-top 透明窗（默认 540×960）而主窗只做对话**（`main.js:2017-2022`），**所以我们应该**：把「形象窗」与「控制台窗」彻底分离成两个 OS 窗——这与本项目 Rust 桌面壳 + Web 前端的天然分层完全一致，也免掉了“面板遮挡角色”的一整类问题。
2. **因为** 它的主窗是**严格的 50/50 分栏**（`.chat-area 50%` / `.side-panel 50%`）且一级侧栏可折到 64px（`styles.css:4068,4523`、`renderer.js:1752-1758`），**所以我们应该**：分栏模式需要时给一个**可记忆的 50/50 基线 + 侧栏 64px 图标轨道**的折叠档，而不是自由拖拽。
3. **因为** SAP 的菜单 hover/active 主色因 `var(--primary-color)` 未定义而**实际无效**、`.tts-status` 因变量拼错而失效（§7.7.6），**所以我们应该**：所有 token 引用必须在**构建/测试期校验存在性**——这条已经被本次调研的 4 个项目反复证明（Amica、Meuxe、Live2DPet、SAP）。
4. **因为** SAP 的「静音」只是把音量设成 `0.0000001` 而**没有音量 UI**（`vue_methods.js:10877` 等），而 GV 用 `killAudio()` 真正停止并持久化状态，**所以我们应该**：**采用 GV 的做法**——静音必须是可持久化的显式状态，且**渲染窗是唯一音频源**（与我方 AGENTS.md「静音置 GainNode 0、音频图保持活着」的裁决方向一致，但要落在**单一音频源**上）。
5. **因为** SAP 的第三方资产有 6 份独立约束（Font Awesome 署名 / Open Runde OFL 保留字体名 / 脏词表 CC BY / Silero MIT），**所以我们应该**：引入任何图标字体/字体文件时**同时引入其许可清单**，并记录在本项目的 `SOURCES.md`（本仓已有该文件）。
6. **因为** SAP 把**字幕浮层（`subtitle_overlay.html`）、弹幕浮层（`danmaku_overlay.html`）、桌面小岛（`island.html`）拆成独立页面**，**所以我们应该**：把“贴桌面/贴屏幕的每一类浮层”当成独立渲染入口（各自可单独开关、单独定位），而不是在一个 HTML 里用 `display` 开关。

> ⚠️ **本节未核到项**：灵动岛 9 个面板的中文 UI 名（`island.html` 只有英文 class）；`listening/typing/speaking/idle` 四态文案的实际渲染位置（模板与主 JS 零引用，疑似死文案）；`vrm.js` 内 `#control-panel` 的完整尺寸/字号表（样式由 JS 拼字符串生成）；`tha.html`/`soulx.html`/`shotOverlay.html`/`chat.html` 的视觉样式；打包后的 CSS 产物内容（见 §11 第 13 条）。

---

### 7.8 Nexus（FanyinLiu/Nexus）—— 两窗分离已成，但样式体系已失控

> ⭐ 18，`pushed_at` 2026-08-27，Electron + React 19 + TS，**MIT**（`LICENSE:1-3`；`package.json` 无 license 字段）。
> 规模：`.ts/.tsx` 686 个文件；**41 个 CSS 共 26,831 行**（最大 `src/app/App.css` 5,985 行、`settings-themes.css` 5,622 行）。Tailwind v4 虽在依赖里，但组件 className **全是自写 BEM**（如 `CompanionPanelV2.tsx:206 className="nexus-panel-v2__stage"`），**grep 不到 Tailwind 工具类用法** → Tailwind 只当 token 容器。

#### 7.8.1 布局：**两个独立无边框透明 OS 窗**；舞台整窗满铺 + 全屏覆盖式聊天

- **Pet 窗常量**（`electron/windowCreation.js:62-67`）：`PET_WINDOW_SCREEN_MARGIN_PX = darwin ? 80 : 24` / `DEFAULT_WIDTH = 320` / `DEFAULT_HEIGHT = 460` / `MIN_WIDTH = 260` / `MIN_HEIGHT = 340` / `PET_ALWAYS_ON_TOP_LEVEL = 'floating'`。
- **Pet 窗构造**（`windowCreation.js:155-179`）：`frame:false, transparent:true, hasShadow:false, alwaysOnTop:true, skipTaskbar:true, resizable:true, maxWidth:1400, maxHeight:1400, maximizable:false, fullscreenable:false, backgroundColor:'#00000000'`；置顶级别 `setAlwaysOnTop(true, 'floating')`（`:201`）；默认落**右下角留边**（`:192-194`）。
- **Panel 窗常量**（`electron/panelWindowController.js:6-11`）：`DEFAULT_WIDTH 460` / `DEFAULT_HEIGHT 660` / `MIN_WIDTH 400` / `MIN_HEIGHT 540` / `COLLAPSED_WIDTH 380` / `COLLAPSED_HEIGHT 92`；构造同 pet 但 `alwaysOnTop:false, skipTaskbar:false`。
- **尺寸持久化**：单文件 JSON，键 `'pet'`/`'panel'`（`electron/services/windowBoundsStore.js:7` `const FILE_NAME = 'window-bounds.json'`）；校验 `if (width < 200 || height < 200) return null` + 校验中心点是否落在显示器 workArea（`:25`）；**折叠时跳过落盘**（`panelWindowController.js:29-31`）。
- **React 布局（V2 = 默认路径）**：面板容器 `width:100vw; height:100vh; min-width:400px; min-height:540px;`（`src/features/uiV2/panel-v2.css:26-30`，与 Electron 最小值**重复定义**）；Live2D 是**绝对定位浮层**：`position:absolute; top:18px; bottom:116px; left:50%; width:min(480px, calc(100% - 32px)); transform:translateX(-50%)`（`panel-v2.css:142-150`）；聊天是**全屏覆盖层** `.chat-sheet-v2 { position:absolute; z-index:12; inset:0; width:100%; height:100% }`（`chat-sheet-v2.css:16-24`）→ **没有 split/grid 比例**。
- Pet 窗 V2 用**容器查询**：`inline-size:100vw; block-size:100vh; min-inline-size:260px; min-block-size:340px; container-type:size`（`companion-v2.css:22-38`）。
- **可折叠 = 真窗口缩放，不是 CSS**：`setResizable(false)` + `setMinimumSize(380, 92)` + `setBounds({... width:380, height:92}, true)`（第 2 参 = macOS 原生动画）（`panelWindowController.js:126-139`）；React 侧只切 `visibility: hidden`（`panel-v2.css:48-51`）。

#### 7.8.2 视觉语言：**两个「中心源」互相打架 + 3 套局部体系**，token 基本没人用

- 中心源 A：`src/index.css:5-82` 的 `@theme`。`--radius-sm: 8px; --radius-md: 8px; --radius-lg: 8px; --radius-xl: 8px; --radius-pill: 999px;`（`:63-67`，**四档同值**）；仅 3 个 `@utility`：`glass`(16px) / `glass-heavy`(24px) / `shadow-glow`（`:184-198`）。
- 中心源 B（**运行时真正生效**）：`src/features/themes/tokens.ts` + `presets/`，经 `cssVariables.ts` 的 `style.setProperty()` 内联注入 `documentElement`（调用点 `src/app/providers/ThemeProvider.tsx:30`）；共 **8 个主题**（`features/themes/registry.ts:14-23`）。accent 逐主题不同：`tokens.ts:15` 的 `accent: '#A88BFF'`；presets 里 `soft #5a8acc` / `high-contrast #000000` / `editorial #b8956a` / `warm-day #c76645`。
- **冲突 1（声明与实际不符）**：`index.css:54` 的 `--color-accent: #333333`（灰）vs `tokens.ts:15` 的 `accent: '#A88BFF'`（紫）；内联 style 优先级更高 → `#333333` **运行时永不生效**。
- **冲突 2（同一语义 token 两窗 fallback 不同，且 fallback 恒生效）**：`--color-accent-secondary` / `--color-danger` / `--color-on-accent` **在 41 个 CSS 中 0 处定义**，于是 `panel-v2.css:5-17` 的 `--nx-v2-accent: var(--color-accent, #8274c9)` / `--nx-v2-danger: var(--color-danger, #a95f5b)` **vs** `companion-v2.css:1-13` 的 `--nx-v2-accent: var(--color-accent, #7d67d9)` / `--nx-v2-danger: var(--color-danger, #a5443d)` → **同一产品两个窗的 accent 与 danger 都不同色**。
- 另外 3 套局部体系：`--settings-*`（`src/app/styles/settings.css:1543` `--settings-accent: #8fa8ff`）、`--nx-settings-*`、`--image4-*`（`panel-companion-rhythm.css:4-5` 青 `rgba(125,229,246,.09)` / 琥珀 `rgba(255,213,145,.17)`）。
- 明暗：`index.css:91` 有 `color-scheme: light dark`，但 `:47-50` 默认全是**浅色半透明面**；暗色只在 8 个主题内。
- 玻璃：`backdrop-filter` **92 处**，其中 **40 处为 `none`**；实际 blur 6/12/14/16/18/20/24px 混用；设置面板**明确反玻璃**（`settings-product-shell.css:8-10` `backdrop-filter: none;`）。
- **圆角**：硬编码 `border-radius: 8px` **223 处**，并存 10px(21)/12px(15)/9px(14)/6px(14)/7px(13)/2px(13)/14px(8)/16px(3)/13px(2)/5px(2)。**`var(--radius-*)` 全仓仅 1 次使用**。
- **阴影**：**151 个互不相同的 `box-shadow` 字面值**；`var(--shadow-sm|md|lg)` 使用 **0 次**。
- **字体**：中心 `--font-sans: 'Segoe UI', 'Microsoft YaHei UI', system-ui, sans-serif;`（`index.css:7`），但 V2 两个核心文件**另写一套且丢了中文字体**（`panel-v2.css:33`、`chat-sheet-v2.css:22` 均 `-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`）。
- **字阶**：**18 种 px，无模数**（12px×123 / 13px×69 / 11px×35 / 14px×34 / 10px×18 / 16px×16 / 18px×9 / 20px×8 / 15px×8 / 22px×7 / 9px×6 / 17px×5 + 34/32/29/19/13.5px）。
- **间距**：无 token；唯一局部尺度 `--image4-rhythm-unit: 8px`（`panel-companion-rhythm.css:3`）。
- **设置面板尺寸并存两套**：`settings-v2.css:530-531` 的 `min(760px, calc(100vw - 48px))` / `min(680px, calc(100vh - 32px))` vs `settings-product-shell.css:26-28` 的 `min(720px,…)` / `min(640px,…)`。
- **Token 命名空间爆炸**：`--nx-settings-*` 178 条、`--image4-*` 160 条、`--panel-toolbar-*` 34、`--weather-sky-*` 37、`--nx-v2-*` 29…；全仓 **877 条自定义属性声明 / 339 个不同名字 / 仅 1 个 `:root` 块**。

#### 7.8.3 信息架构：模态抽屉 + 左侧边栏 + **组内二级 tab**；同时残留一整套 V1 IA

- 生效判据：`if (new URLSearchParams(window.location.search).get('uiV2') !== '0') { return (<SettingsDrawerV2 ...`（`src/components/SettingsDrawer.tsx:879`）→ **V1 默认不可达**。
- 导航形态：`<aside className="settings-v2__sidebar">` + `settings-v2__nav`（`src/features/uiV2/SettingsShellV2.tsx:103-140`）；栅格 `grid-template-columns: minmax(152px, 184px) minmax(0, 1fr)`（`settings-v2.css:19`）；**二级 tab** `<nav className="settings-v2__section-tabs">`（仅当该 destination 含 >1 section 时渲染）（`SettingsDrawerV2.tsx:167`）。
- **一级 destination 精确字符串（5 项）**：`Home` / `Companion` / `Voice` / `Memory & privacy` / `Connections & advanced`（`SettingsDrawerV2.tsx:92-119` + `src/i18n/locales/en/core.ts:9,11,13`）。
- **destination → section 映射**：`companion: ['chat','letters','window','autonomy']`、`voice: ['voice']`、`privacy: ['memory','lorebooks']`、`advanced: ['model','integrations','tools','history','console']`（`SettingsDrawerV2.tsx:15-20`）。
- **12 个 section 的 EXACT id 与英文标签**（id 见 `src/components/settingsDrawerSupport.ts:302-313`；文案见 `en/settings-shell.ts:41-48,79,82` + `settings-chat.ts:113`）：
  `model`→**Model**、`chat`→**Profile**、`window`→**Desktop**、`console`→**Diagnostics**、`history`→**History**、`letters`→**Letters**、`voice`→**Voice**、`memory`→**Memory**、`lorebooks`→**Background & Phrases**、`integrations`→**Connections**、`autonomy`→**Presence**、`tools`→**Tools**。
- **IA 内部 key 命名 3 套并存**（同一数组内）：10 项用 `settings.section.<id>`，而 `lorebooks` 用 `'settings.lorebooks.title'`、`integrations` 用 `'settings.section_eyebrow.integrations'`（`settingsDrawerSupport.ts:311,312`）。
- **并存的第二套 IA（仅 V1 可达）**：`src/components/settingsHomeArchitecture.ts:32-73` 定义 5 个分组 `appearanceExperience / companionBehavior / memoryContext / modelConnections / maintenance` → **每加一个设置项要改两处**。

#### 7.8.4 关键交互

- **模型切换：原生 `<select>`**，位于 设置→Profile，**无缩略图、无预览**（`src/features/settingsV3/ChatSectionV3.tsx:138-148`）。
- **动作/表情触发**：mood→expression slot 映射 + motionGroup 回退 —— `case 'speaking': return modelDefinition.motionGroups.speakingStart ?? modelDefinition.motionGroups.interaction`（`src/features/pet/components/live2d/expressions.ts:66-79`）；细粒度 mood 被**折叠**到已有槽位（`'excited'|'proud'|'playful' → 'happy'`，`:24-40`）；**未知手势静默忽略**。
- **音量/静音：只有 TTS 输出音量滑块，全仓无 mute 开关**（`src/features/settingsV3/SpeechOutputProviderV3.tsx:408-411`）；其文案明确 `'This only adjusts TTS output loudness — system volume is unchanged.'`（`en/settings-voice.ts:99`）；`grep -i mute` 在 `src/**/*.{ts,tsx}` 只命中类型名与注释。
- **三种文本呈现面互不复用**：① **浮动 caption**（面板）`<button className="nexus-panel-v2__caption">`，2 行截断（`CompanionPanelV2.tsx:257-272`、`panel-v2.css:200 -webkit-line-clamp: 2`）；宠物窗 caption `inset-block-start: 98px; max-inline-size: min(264px, calc(100% - 24px))`（`companion-v2.css:212-240`）。② **聊天气泡** `data-chat-surface-state`（`MessageBubble.tsx:242`）。③ **宠物对话/思考气泡**：`PetDialogBubble.tsx:75`（自带 `aria-live="polite"`）、`PetThoughtBubble.tsx:9-16` 按 urgency 60/30 分 `--strong/--medium/--soft`。
- **状态展示（7 值状态机 + 按状态换节奏）**：`'idle'|'listening'|'thinking'|'speaking'|'done'|'error'|'offline'`（`src/features/uiV2/state.ts:5-11`），`done` 停留 **550ms**（`:14`）；视觉映射 `[data-companion-motion="breathe"] 5.8s / "think" 1.7s / "listen" 2.4s / "speak" 0.72s / "wait" 1.8s`，**离线仅 `opacity: 0.72`**（`src/app/App.css:2840-2862`）；无障碍播报 `role="status" aria-live="polite"`（`CompanionPanelV2.tsx:361-373`）。
- **字幕消失时长按内容估时（可直接搬运的函数）**：`Math.min(12_000, Math.max(3_000, cjkCount * 350 + latinWordCount * 220))`（`src/features/uiV2/caption.ts:6-9`）。
- **点击穿透**：`win.setIgnoreMouseEvents(Boolean(state.clickThrough) && !Boolean(state.petHotspotActive), { forward: true })`（`electron/petWindowInstances.js:88-91`）——**带 hotspot 豁免**，比 OLV-Web 的实现更完整。

#### 7.8.5 动效：**无任何 JS 动画库**，全部 CSS + Electron 原生

- **55 个 `@keyframes`**（`App.css` 29 / `panel-scene.css` 12 / `settings.css` 6 / `index.css` 3）。
- 消息出现：`animation: message-enter 0.25s cubic-bezier(0.16, 1, 0.3, 1) both;`（`App.css:767`）+ keyframes `opacity 0→1`、`translateY(10px)→0`（`:752-760`）——**与 AIRI 的出场曲线同一条 `cubic-bezier(0.16,1,0.3,1)`**。
- 通用入场：`@utility bounce-in { animation: bounceIn 0.4s var(--ease-bounce); }`（`index.css:200-207`）。
- transition 时长分散：140ms×7 / 150ms×6 / 160ms×5 / 180ms×4 / 120ms×1。
- **面板折叠动画全在 Electron**（`setBounds(..., true)`），React 只切 `visibility`；**聊天 sheet 开合零过渡**；**设置 V2 无入场动画**，只有 hover 过渡（`settings-v2.css:466-473`，在 `@media (prefers-reduced-motion: no-preference)` 内 150ms）。
- reduced-motion 有全局兜底（`index.css:219-228`）+ 分面覆盖。

#### 7.8.6 明确的设计缺陷（量化）

1. **色值完全未收敛**：`src` 下 hex 字面值 **564 次出现 / 229 个不同值**；`rgba?()` **2,183 处**（高频 `#fff`×37 / `#ffffff`×34 / `#a88bff`×23 / `#4e7cff`×17 / `#1a1f29`×15）。
2. **强调色跨视图冲突 6 套**：`#8274c9`（`panel-v2.css:9`）/ `#7d67d9`（`companion-v2.css:4`）/ `#333333`（`index.css:54`）/ `#A88BFF`（`tokens.ts:15`）/ `#8fa8ff`（`settings.css:1543`）/ 青+琥珀（`panel-companion-rhythm.css:4-5`）。
3. **3 个 `--color-*` 被引用但从未定义**：`--color-accent-secondary` / `--color-danger` / `--color-on-accent`（定义数 = 0）→ **fallback 恒生效且两窗不同**。
4. **圆角 token 形同虚设**：`var(--radius-*)` 仅 1 次使用 vs 硬编码 `8px` **223 次**；且 `--radius-sm/md/lg/xl` **四档同值 8px**，token 本身无信息量。
5. **Token 命名空间爆炸**（877 条声明 / 339 个名字）。
6. **重复定义与冲突规则**：`.message-bubble` 在 `App.css:763`（`max-width:88%; padding:12px 16px`）与 `App.css:3266`（`max-width:90%; padding:14px 18px`）各定义一次；`App.css` 内 **37 组重复顶层选择器**。
7. **`!important` 泛滥**：实测 **305 处**，集中在只服务 `uiV2=0` 的补丁层（`panel-companion-visual-lock.css` 124、`panel-companion-final.css` 100）。
8. **z-index 无尺度**：`10000`×3 / `9999`×1 / `120`×1，其余 0–40。
9. **缺加载态（有实据的那一处）**：`<Suspense fallback={null}>`（`CompanionPanelV2.tsx:242`）→ **模型加载期间舞台全空白，无骨架无 spinner**。反例说明那不是全局缺失：设置分节有骨架（`SettingsDrawerActiveSection.tsx:413-417`，带 `role="status" aria-busy="true"`）、聊天有空态（`ChatSheetV2.tsx:275-277`）。
10. **字体栈被局部覆盖**：`panel-v2.css:33` / `chat-sheet-v2.css:22` 丢掉中心 `--font-sans` 的中文字体；且 **CJK 界面 9–11px 共 59 处**（低于可读门槛）。
11. **半死代码**：`App.css`(5,985) + panel-companion 链(3,505) 服务 `LegacyPanelView.tsx`(953) / `LegacyPetView.tsx`(754)，仅 `?uiV2=0` 或无模型可达。**注意：41/41 个 CSS 文件均被 import，未发现完全无人引用的文件**——不要误判为可删。
12. **emoji 当图标：不成立**（与 SoW 形成对照）：全仓仅 4 个文件含 emoji，且集中在 legacy/文案（`LegacyPanelView.tsx:462` 空态 `✦`、`LegacyPetView.tsx:661` `✋`）；图标本体为内联 SVG。
13. **文档漂移**：`design-qa.md` 是本机开发日志（含 `/Users/klein/...` 绝对路径），其记录的 Image4 五行高度 `[presence] 91px [dial] 172px [greeting] 98px [actions] 232px [composer] 100px` 属**已降级为 legacy 的路径**，不可作为当前 V2 设计基线。

#### 7.8.7 可迁移的设计决策

1. **因为** panel 与 pet 各自重定义同一语义 token 且 **fallback 不同色**（`panel-v2.css:9` vs `companion-v2.css:4`），**所以我们应该**：语义 token **只允许主题提供方注入一次**，视图只消费不重定义，并补齐缺失语义色后**删除 fallback**。
2. **因为** `--radius-*` 四档同值且几乎零引用、硬编码 `8px` 223 次，**所以我们应该**：先删死 token，再统一到 3 档尺度，并**用审计脚本**阻止新增字面值（该仓库已有 `image4-visual-contract-audit` 类先例）。
3. **因为** 计数字幕消失时长是**按字符/词数估时**而非固定 timer（`caption.ts:6-9`：`min(12_000, max(3_000, cjk×350 + latinWords×220))`），**所以我们应该**：**原样搬运这个“按内容估时”函数**，而不是换成固定 `setTimeout`——中文 350ms/字、拉丁 220ms/词、clamp 3–12s，是一份现成的可用常量。
4. **因为** 它的折叠动画**已由 Electron 原生 `setBounds(..., true)` 完成**而 React 只切 `visibility`（`panelWindowController.js:126-139` + `panel-v2.css:48-51`），**所以我们应该**：把「窗口级形变」留在宿主层、UI 层只做内容级过渡，**避免双层动画打架**。
5. **因为** 它存在**两套设置 IA**（V2 destination+section tabs 生效；V1 home 分组仅 `?uiV2=0` 可达）且分组事实有两份，**所以我们应该**：**物理删除**旧 IA 分支与旧分组定义，否则每加一项要改两处并保持同步。
6. **因为** 它的点击穿透**带 hotspot 豁免**（`petWindowInstances.js:88-91`：`clickThrough && !petHotspotActive`），**所以我们应该**：穿透逻辑要区分“全局穿透”与“热点豁免”，后者才是角色可拖、可点的关键。
7. **因为** 它的模型切换只是**原生 `<select>`、无预览**（`ChatSectionV3.tsx:138-148`），而 N.E.K.O 有 ▶ 试播、AIRI 有导入前检查（§3.2、§2.4），**所以我们应该**：把“模型/动作选择器带预览”当作明确的比较优势来设计，而不是最低成本实现。
8. **因为** 它的离线条仅靠 `opacity: 0.72`（`App.css:2840-2862`），**所以我们应该**：**离线/错误态必须有独立的颜色与文案**，不能只靠降低不透明度。

---

### 7.9 ghost-vessel（ghdtjrtka/ghost-vessel）与其它 2025–2026 新项目

#### 7.9.1 Ghost Vessel —— **双窗 + 只用 12 个 CSS 变量**的极简样本

> **MIT**（`LICENSE:1-3`）——可直接吸收（保留声明）。⚠️ 但 `presets/README.md:20-24` 明示分层：**引擎 MIT / preset 是可售卖纯数据** → **可复用的是引擎与 UI 代码，不含角色资产**。

- **双窗几何（`src-tauri/tauri.conf.json:14-49`）**：

| 窗口 | 默认尺寸 | 最小尺寸 | 初始位置 | 装饰 |
| --- | --- | --- | --- | --- |
| **video（形象）** | **360×640** | 220×340 | x60 / y60 | `decorations:false, transparent:true, alwaysOnTop:true, shadow:false` |
| **chat（对话）** | **400×560** | 300×320 | x440 / y60 | 同上 |

- **全 UI 只由 12 个 CSS 变量驱动**（`player/index.html:7`），并有 **12 套主题，只换 `bg + accent` 两个值**（`:837-850`）。
- **GV 没有设置页**：全部塞在一个 **280px 浮层 `.thememenu`** 里，分节为 `언어 / 다크 / 라이트 / 시스템 / 마이크 장치 / 음성 엔진`（`player/index.html:859-952`）。
- **静音做法是正例**：`toggleVoice()` 关麦即 `killAudio()` 并持久化（`player/index.html:709-711,1441`）。
- **路线差异**：它用**预渲染的情绪视频片段代替 Live2D/VRM**（`awesome-ai-companion` 列表描述：“pre-rendered emotion clips instead of Live2D or VRM … Low runtime GPU cost”）。→ **设计含义**【推断】：视频片段的“帧集合 = 表情枚举”，UI 只需“切换片段”，不需要参数面板；**代价是无法做连续口型**（只能整段播放）。这正好反证了本项目“音频驱动口型”的必要性。
- **未核到**：`bridge/` 侧是否影响视觉；是否有非 MIT 第三方资产清单（仓库内无 `LICENSE-third-party`）；`prefers-reduced-motion` 覆盖（**完全没有**）。

**可迁移决策**：**因为** GV 用**两个小窗（形象 360×640 / 对话 400×560）+ 12 个 CSS 变量 + 12 套只换 `bg+accent` 的主题**就撑起完整体验，**所以我们应该**：把“最小可用主题”定义为**只换 2 个变量**（背景 + 强调），而不是一整套色板——这与我方 §10 P0-1 的“单一色相基准”是同一思路的两种落地方式。

#### 7.9.2 其它 2025–2026 新项目

- 通过 GitHub 搜索（`live2d llm companion desktop`、`ai companion desktop pet vrm live2d created:>2025-01-01`）发现的 **2025–2026 新项目**（star 数低）：`LunaMate`（Rust/GPUI）、`Agent-LLM-Live2D`（TS，“giving Live2D bodies to AI Agents”）、`DesktopPetMonitor`、`deskpet`、`Norma`（Go）、`yume`、`aicompanion`（“Desktop AI Companion Studio”）、`super-agent-party`（已单列，§7.7）。→ **这些项目均未逐项核到 UI 证据**，本报告不引用其界面细节。
- **参考索引**：`DasterProkio/awesome-ai-companion`（⭐654，`pushed_at` 2026-09-10）是当前对 Live2D/VRM 伴侣类项目覆盖最广的清单，其中把 `AIRI`、`Open-LLM-VTuber`、`Soul-of-Waifu`、`Amica`、`super-agent-party` 标为 `ready`，把 `ai-live2d-body` 标为 `Guide only`，并注明 `Amica` 已 **Unmaintained**。→ 若后续还要扩样，应从这里出发而不是重新搜索。

---


## 8. 设计缺陷总表（跨项目模式）

> 按“出现频次 × 危害”排序；括号内是出现该缺陷的项目。

| # | 缺陷模式 | 出现项目 | 危害 |
| --- | --- | --- | --- |
| 1 | **多个并列强调色**（2–6 个近似色同时存在） | OLV-Web、Amica（≥6 族）、N.E.K.O（5 个近似蓝）、my-neuro（同屏 4 种）、Live2DPet、AIRI（进度条粉色游离）、**Soul of Waifu（6 套）**、**Nexus（6 套）** | 观感立刻分裂，用户感知为“拼接品” |
| 2 | **完全无设计令牌 / 无主题机制** | OLV-Web、N.E.K.O、Amica、ChatVRM、Live2DPet、my-neuro（桌宠层与控制窗）、**Soul of Waifu（335 hex / 1,196 处内联）** | 无法主题化，改色=全仓替换，深色模式靠覆盖表 |
| 3 | **状态指示只有文字、无形状/颜色区分，或干脆没有** | OLV-Web（6 态 1 视觉）、Amica（`chatSpeaking` 从未渲染）、Live2DPet（思考态无 UI）、my-neuro（思考态无 UI）、**N.E.K.O（连接态被 `display:none !important`）**、**Nexus（离线仅 `opacity:.72`）** | 用户不知道 AI 在干什么 |
| 4 | **令牌/类名静默失效**（未定义/拼错的变量、空 type 撞 `display:none`、外部 preset 丢失） | Amica（≈14 处类名 + `bg-rose/90`）、Live2DPet（`class="status "` 永远不可见）、Meuxe（`clay-600` 未定义）、**SAP（`--primary-color` 用 9 次无定义、`--el-primary-color` 拼错）** | 缺陷不报错、不可见、只在上线后被用户发现 |
| 5 | **动画曲线/时长不一致（3+ 套），或 animation 的 keyframes 根本不存在** | OLV-Web（**4 套**曲线 + `pulse` 未定义）、my-neuro、Live2DPet（5 条 transition、缓动全省略）、N.E.K.O（3 套曲线） | 动效无“产品感”，且有静默失效 |
| 6 | **没有音量/静音 UI**（只存在于配置字段、或是 hack、或完全没有） | OLV-Web（完全无）、Meuxe（无）、my-neuro（仅配置字段）、Live2DPet（仅滑杆+Silent 模式）、**Nexus（只有 TTS 音量滑杆、无 mute）**、**SAP（`volume = 0.0000001` hack、无音量滑杆）** | 桌宠第一高频需求缺失；hack 静音会导致口型/音频图状态漂移 |
| 7 | **CJK 字体未处理**（无中文字体栈 / 靠 OS 回退 / 局部覆盖丢掉） | OLV-Web（未核到任何 CJK 处理）、ChatVRM/Amica（靠 Google Fonts）、**Nexus（V2 两个核心 CSS 丢掉 `Microsoft YaHei UI`，9–11px 共 59 处）** | 中文界面字形随平台漂移，且字号低于可读门槛 |
| 8 | **emoji 当图标 / 两套图标语言混用** | my-neuro（`🎵🐂🔊🎤` vs SVG tab）、Live2DPet（`⚙✎✕⟲`）、**Soul of Waifu（201 处，含托盘菜单）** | 跨平台渲染不一致，廉价感 |
| 9 | **同一数值在多处硬编码 / 权威源分裂** | OLV-Web（440px/24px 写 3 遍）、AIRI（order 1..8 分散 8 文件）、**Soul of Waifu（`_BG` 12 处三种值、同一字段 JSON 与代码默认值矛盾）**、**Nexus（4 档 radius 同名同值、Electron 与 CSS 各定义一次最小值）** | 改一处要记得改三处；不可预测 |
| 10 | **深色/浅色只做一半**（另一个是死代码或覆盖表） | OLV-Web（light 链路全死）、AIRI（浅色对比度过低）、N.E.K.O（2,903 行覆盖表）、**Meuxe（`ChatPanel` dark 分支是死代码）**、**Nexus（`color-scheme: light dark` 但默认全浅）** | 主题切换是伪功能 |
| 11 | **贴图/九宫格缺失导致变形** | Live2DPet（16:9 贴图塞进 3:1、`object-fit:fill`）、N.E.K.O（`background-size:100% 100%` 拉伸 toast 贴图 + PNG 气泡壳） | 视觉硬伤 |
| 12 | **空 Tab / 孤立 i18n 键 / 死按钮 / 死分支** | OLV-Web（TTS Tab 空白、附件按钮无 onClick）、Live2DPet（`tab.enhance` 无 tab）、**AIRI（`categorizedModules`、`/v2/settings` 半成品）**、**Nexus（整套 V1 IA 仅 `?uiV2=0` 可达）**、**SAP（四态文案零引用）** | IA 与实现脱节，每加一项要改多处 |
| 13 | **面板开关是硬切（无过渡）** | Amica（设置/日志无过渡）、OLV-Web（消息上屏无动画）、Live2DPet（气泡硬切）、ChatVRM（零动效）、**Meuxe（右面板瞬时挂卸、无宽度过渡）**、**Nexus（聊天 sheet 零过渡）** | 无“产品感” |
| 14 | **窗口装饰与应用底色不匹配 / 原生标题栏残留在无边框壳里** | OLV-Web（`#111111` vs `gray.900`）、AIRI（设置窗保留 mac 原生标题栏） | 两层壳语言冲突 |
| 15 | **`!important` / 内联样式泛滥**（token 系统“后加”，旧样式未回迁） | **SAP（`!important` 1,514 处 + 内联 495 处）**、**Nexus（`!important` 305 处 + 37 组重复选择器）**、**Soul of Waifu（内联 `setStyleSheet` 1,196 处）**、N.E.K.O（深色靠覆盖表） | 后续所有样式修改都可能被静默覆盖 |
| 16 | **缺少加载/空态，尤其是模型加载期间** | **Nexus（`<Suspense fallback={null}>` → 舞台全空白）**、Amica（进度回调是 TODO）、ChatVRM（无加载/错误态）、my-neuro、Live2DPet（无 spinner/skeleton）、Soul of Waifu（思考态用 `💭 Thinking...` emoji 文案） | 最高风险的“导入模型”操作无反馈 |
| 17 | **离线/错误态只靠降低不透明度或小字** | **Nexus（离线仅 `opacity:.72`）**、OLV-Web（错误是红色小字）、N.E.K.O（连接文本被隐藏） | 断线用户不知情 |

---

## 9. 正面清单：值得直接抄的“配方”

> 这些是可以逐字落到 Flutter 侧的（不涉及代码复用，只涉及设计决策），每条都有一手来源。

| 配方 | 具体值 | 来源 |
| --- | --- | --- |
| **毛玻璃卡片** | 4px 半透明强调色描边 + `rounded-xl` + 50–70% 半透明底 + `backdrop-blur-md` | AIRI `ChatContainer.vue:2-7` |
| **深色底 / 浅色底** | `rgb(18 18 18)` / `rgb(255 255 255)`；切主题时 `html { transition: all .3s ease-in-out }` | AIRI `styles/main.css:12-27` |
| **出场缓动** | `cubic-bezier(0.16, 1, 0.3, 1)`（弹层 150ms、遮罩 300ms、滑入 400ms、淡变 200ms） | AIRI `uno.config.ts:265-283` |
| **弹性缓动** | `cubic-bezier(.2,.9,.3,1.2)`（pop-in 300ms） | Meuxe `index.css:107-108` |
| **扁平分层** | 阴影 = `0 0 0 1px rgba(20,20,25,0.07)`（描边而非投影） | Meuxe `index.css:101-104` |
| **圆角 5 档** | 12 / 12 / 16 / 20 / 24 px（control / field / card / panel / sheet） | Meuxe `index.css:91-95` |
| **半透明浮层（深色场景）** | `blackAlpha 70%` + `backdrop-blur 8px` | OLV-Web `electron-style.tsx:4-27` |
| **连接徽标** | 圆角 20px 胶囊 + `padding 8px 16px` + `fontSize 14px`；绿=已连接/黄=连接中/红=点击重连 | OLV-Web `canvas-styles.tsx:55-72` |
| **字幕条** | `rgba(0,0,0,.7)` + `padding 15px 30px` + `radius 12px` + `minWidth 60% / maxWidth 95%` + `fontSize 1.5rem` + `lineHeight 1.4` + `pre-wrap` | OLV-Web `canvas-styles.tsx:39-54` |
| **侧栏收起留把手** | 面板 440px，收起 `translateX(-100% + 24px)`，整条 24px 竖边可点 | OLV-Web `sidebar-styles.tsx:35-67` |
| **深色字幕风格** | 白字 + 8 向黑描边 + 900 字重 + 手写体 | my-neuro `styles.css:98-111` |
| **加载扫描条** | 容器 `h-1` + 子元素 `w-1/3` 主色 + 2s 线性 `translateX(-100% → 400%)` | AIRI `InteractiveArea.vue:62-68,93-103` |
| **状态 Pill** | 三态三色（listening=peach / speaking=accent / thinking=honey）+ 呼吸点 | Meuxe `App.tsx:471-477,538-547` |
| **图标 rail** | 竖向一列圆形图标，每个一个语义色底 | VTube Studio 官方截图 |
| **“数值滑块 + 就近设置”** | 字幕不透明度/大小/语言就贴在字幕下方 | N.E.K.O 官方截图 `ss3` |
| **字幕模块化配色** | **116 个 `--subtitle-*` 变量 + 7 套 `[data-subtitle-color-scheme]` 配色档**；字号离散档 16/21/26/34/44px（默认 26）；内边距用 `clamp()` 跟自身尺寸联动 | N.E.K.O `subtitle.css:13-42,127-176`、`subtitle.html:55-59` |
| **字幕/弹幕双模** | 独立 `/subtitle` 窗口可切弹幕模式，有独立 lane 层 + 边缘渐隐 | N.E.K.O `subtitle.css:336-346` |
| **字幕逐词揭示参数** | 45ms/字 + `base_gap 100`；句读 +450ms、逗号 +200ms；clamp `MIN 80 / MAX 700`；淡入 85ms / 淡出 380ms；字号自适应 18–40px | Soul of Waifu `sow_system_signals.py:4225-4232,4288-4300` |
| **字幕消失时长（按内容估时）** | `min(12000, max(3000, cjk×350 + latinWords×220))`（中文 350ms/字、拉丁 220ms/词） | Nexus `caption.ts:6-9` |
| **状态色 × 状态动画** | `breathe 5.8s / think 1.7s / listen 2.4s / speak 0.72s / wait 1.8s`（按状态换节奏，不止换色） | Nexus `App.css:2840-2862` |
| **状态机 → 颜色/文案单点映射（四态四色）** | `Ready rgb(70,70,70)` / `Listening rgb(46,204,113)` / `Thinking rgb(241,196,15)` / `Speaking rgb(52,152,219)`，并**同步到渲染面** | Soul of Waifu `sow_system_signals.py:213-218,177,240` |
| **12 变量主题** | 全 UI 只由 12 个 CSS 变量驱动；12 套主题**只换 `bg + accent` 两个值** | Ghost Vessel `player/index.html:7,837-850` |
| **点击穿透带热点豁免** | `setIgnoreMouseEvents(clickThrough && !petHotspotActive, {forward:true})` | Nexus `petWindowInstances.js:88-91` |
| **窗口折叠 = 宿主原生形变** | Electron `setBounds({...380×92}, true)`（macOS 原生动画）+ React 侧只切 `visibility` | Nexus `panelWindowController.js:126-139` |

---

## 10. 落地到本项目的设计决策清单（按优先级）

> 形式：**因为 X（一手证据），所以我们应该 Y（可执行动作）**。前 6 条是“地基”，建议在任何 UI 实现之前先冻结。

### P0 —— 先立规矩（做 UI 之前）

1. **因为** 8/10 个项目都栽在“多个强调色”上（§8 #1），**所以我们应该**：
   在 `shell/flutter/lib/ui/` 下建 `tokens.dart`，**只暴露一个品牌色基准**（建议沿用当前的 `#7C5CFF` 或其色相），其余色值全部派生；并用一个 Dart 常量类 + 一个 `test/ui/tokens_test.dart` 断言“不存在未使用的裸色值”（可用一个自定义 lint/正则测试）。
2. **因为** 令牌静默失效在本次调研中出现了 **4 次**——Amica 丢 preset（≈14 处类名）、Meuxe 的 `clay-600` 未定义、SAP 的 `--primary-color` 未定义（用了 9 次）、Live2DPet 的空 type 撞 `display:none`（§8 #4），**所以我们应该**：
   所有令牌引用必须是编译期常量（Dart `const`），**不允许字符串查表**；并加一个**枚举式校验测试**：遍历所有被引用的 token 名，断言每一个都有定义（这正是 Meuxe 抓到自己 bug 的方法，值得照搬）。
3. **因为** 本次调研中**没有一个项目把 CJK 字体处理对**——OLV-Web 完全无中文字体栈（靠 OS 回退，§4.3）、ChatVRM/Amica 从 Google Fonts CDN 拉字体（§7.6.2）、Nexus 的 V2 CSS 局部覆盖丢掉了 `Microsoft YaHei UI` 且中文界面出现 9–11px 共 59 处（§7.8.6.10）、Soul of Waifu 用 `Inter Tight` 撞 CJK（§7.2.2）——而本项目**离线优先**（`--no-web-resources-cdn`）且 Flutter Web 的 CanvasKit **取不到设备字体**，**所以我们应该**：
   把「中文字体自托管」写成**硬约束**（本项目 AGENTS.md 已裁决：`assets/fonts/` 内置 Noto Sans SC 子集，OFL-1.1，回归在 `test/theme_test.dart`）；在 `tokens.dart` 里规定两条字体栈（UI 正文 / 强调体），并**禁止任何组件层覆盖字体族**（Nexus 的教训）。
4. **因为** Meuxe 证明了“描边分层”在深色 UI 上比投影更清晰（§7.3.2），**所以我们应该**：
   阴影令牌只保留 2 档，**以 1px 描边为主**；透明桌宠形态下不用投影。
5. **因为** OLV-Web 出现 **4 套**曲线、N.E.K.O 出现 3 套（`0.35s/0.32s/0.4s` 各不同曲线）、my-neuro 有 `prefers-reduced-motion`、AIRI 提供“关闭页面转场”开关（§2.2e），**所以我们应该**：
   动效令牌固定为 **2 条曲线 + 3 档时长**（快 120ms / 中 200ms / 慢 320ms），出场统一 `cubic-bezier(0.16,1,0.3,1)`（AIRI 与 Nexus 独立收敛到同一条），弹性另立一条；并提供**全局“减少动效”开关**。
6. **因为** 所有项目都没有把“状态”做成单一契约（§8 #3：6 个项目有缺口，只有 Meuxe 与 Soul of Waifu 做对），**所以我们应该**：
   定义 `UiState` 枚举（**离线 / 连接中 / 空闲 / 聆听 / 思考 / 说话 / 错误**）→ 一张“色 + 形 + 字 + 节奏”映射表（可参考 Nexus 的按状态换动画速度，§9）→ **一个** `StatePill` 组件；禁止任何页面自造状态样式。

### P1 —— 布局与舞台/聊天关系

7. **因为** 10 个开源样本里只有 3 个做真分栏（Meuxe / SAP 50-50 / OLV-Web 440px），其余都是浮层或独立窗口（§1.1），**所以我们应该**：
   默认形态 = **舞台满幅 + 聊天浮层**（参考 N.E.K.O 的左侧毛玻璃浮层 `ss1` + AIRI 的浮层控制岛）；分栏作为“复盘/工作模式”的第二种布局，且切换时**必须由实测容器宽度驱动**（Flutter `LayoutBuilder`），参考 OLV-Web 与 Amica 的双重教训（§4.1、§7.6.1）。
8. **因为** OLV-Web 的聊天面板收起后仍保留 24px 把手（§4.9.1），**所以我们应该**：聊天浮层收起时保留一个明确的“拉出”把手，且命中区 ≥ 24px。
9. **因为** AIRI 的托盘提供“推荐 / 全高 / 半高 / 全屏 + 5 个对齐位”而不是让用户自由拖（§2.7.7），**所以我们应该**：桌宠窗提供**一组尺寸/对齐预设**。可用的一手档位：AIRI 主窗 **450×600**、Meuxe 迷你 **280×420**（S/M/L/XL 260×400…380×620）、Nexus pet **320×460**（min 260×340）、Ghost Vessel 形象 **360×640**、SAP VRM 窗 **540×960**。
10. **因为** 五个项目独立收敛到同一组窗口参数（§结论先行-4，另加 Nexus/GV），**所以我们应该**：把 `transparent / frame:false / hasShadow:false / alwaysOnTop('screen-saver' 或 'floating') / visibleOnAllWorkspaces / skipTaskbar` 写进桌面壳的一处常量；**气泡与字幕窗必须 `focusable:false` + `showInactive()`**（Live2DPet `window-manager.js:184,229`）；**穿透逻辑要带热点豁免**（Nexus `petWindowInstances.js:88-91`）。
11. **因为** super-agent-party 把**形象放进独立 always-on-top 透明窗（540×960）、主窗只做对话**（§7.7.2），**所以我们应该**：认真评估「形象窗 / 控制台窗」双窗方案——它与本项目 Rust 壳 + Web 前端的边界天然对应，代价是要处理两窗之间的状态同步（Meuxe 用 `app:mode-changed` 事件，`window.rs:64-78`）。
12. **因为** Ghost Vessel 用**全 UI 仅 12 个 CSS 变量、12 套主题只换 `bg+accent`** 就跑通完整体验（§7.9.1），**所以我们应该**：把“最小可用主题”定义为**只换 2 个变量**（背景 + 强调），先做到这一点再谈完整色板。

### P1 —— 信息架构

13. **因为** AIRI 的设置首页是“图标 + 标题 + 一句说明”的卡片菜单，且每项都强制有 description（§2.3a），**所以我们应该**：设置首屏用**声明式清单**（`List<SettingsEntry>`）渲染卡片菜单，每个条目必须有 `title + description + icon`；条目顺序由一个显式 `order` 字段决定，并**加测试防止重号**（修正 AIRI 的 order 分散问题，§2.6.2）。
14. **因为** Meuxe 把页面枚举集中在一个类型 + `PAGE_META`（§7.3.3），**所以我们应该**：设置页路由用**枚举 + 元数据表**，导航 id 直接采用用户可见语义（避免 `llm` vs `Agent` 的脱钩）。
15. **因为** Warudo 的 Editor 只有 2 个顶层 Tab 就覆盖全部配置（§6），而 SAP 用了 11 项一级导航（§7.7.4），**所以我们应该**：控制台一级导航 ≤ 3（建议：**形象 / 行为 / 系统**）；设置首页卡片菜单**最多 8 项**（AIRI 的实测上限）；二级列表项上限参考 Soul of Waifu 的 6 项/组（§7.2.3）。
16. **因为** OLV-Web 出现“可见 Tab 点进去空白”（§4.8.2）、Live2DPet 的“有翻译无控件”（§7.5.7.8）、Nexus 的“整套 V1 IA 仅 `?uiV2=0` 可达”（§7.8.3），**所以我们应该**：为 IA 写契约测试——**每个导航项必须有非空内容页**，每个 i18n key 必须有引用者，**不允许存在不可达的第二套 IA**。

### P1 —— 关键交互

17. **因为** 音量/静音在 **6 个项目里都有缺口**（OLV-Web 无、Meuxe 无、my-neuro 仅配置字段、Live2DPet 仅滑杆、Nexus 只有 TTS 滑杆无 mute、SAP 是 `volume=0.0000001` hack），**所以我们应该**：把 **主音量 + 静音**做成一级控件（浮层底部常驻），实现按 AGENTS.md「GainNode 增益、与口型正交、静音置 0 但保持音频图活着」，并采纳 Ghost Vessel 的「静音是可持久化的显式状态」+「渲染窗是唯一音频源」（§7.9.1、§7.7.5）。
18. **因为** N.E.K.O 的动作/表情列表**每项都有 ▶ 试播**而 Live2DPet 没有、Nexus 只是原生 `<select>` 无预览（§3.2、§7.5.4、§7.8.4），**所以我们应该**：模型/动作/表情选择器**必须支持即时试播**；这需要在 core 侧暴露“UI 触发动作”的合法指令路径。
19. **因为** N.E.K.O 把“常驻/待机表情”与“对话触发表情”分成两个入口（§3.2），**所以我们应该**：模型配置里把 **idle 组**与**事件组**显式分为两类。
20. **因为** ChatVRM/Amica 完全缺加载/错误态、Amica 的进度回调是 TODO、Nexus 用 `<Suspense fallback={null}>` 导致舞台全空白（§7.6.4、§7.8.6.9），**所以我们应该**：**模型导入/切换**必须有“进度 + 失败原因 + 重试”三件套，且**默认不是空白**。
21. **因为** AIRI 的“停止朗读”只停语音、不取消已生成文字（§2.4），**所以我们应该**：把“停止朗读”做成独立按钮，且**不打断文本流**。
22. **因为** OLV-Web 的字幕**不随音频结束清空**、也没有队列（§4.6），**所以我们应该**：字幕生命周期绑定到“句子播放单元”（与 AGENTS.md「一句一单元」一致）；计时可直接采纳 Nexus 的按内容估时函数 `min(12000, max(3000, cjk×350 + latinWords×220))`（§7.8.4），逐词节奏参考 Soul of Waifu 的 45ms/字（§7.2.4）。
23. **因为** OLV-Web 的“打断/举手”是**同一个按钮两种语义、外观不变**（§4.6），**所以我们应该**：禁止语义随状态变化的按钮；若必须复用，**外观必须随之变化**。
24. **因为** Meuxe 与 Soul of Waifu 都把「正在朗读的那一句」做成**独立于历史消息**的浮动字幕（§7.3.4、§7.2.4），**所以我们应该**：区分「实时朗读字幕」与「历史消息」两种载体，避免流式字幕塞进消息列表引发重排与滚动抖动。

### P2 —— 观感与打磨

25. **因为** AIRI 的加载态是 2px 扫描条（§9），**所以我们应该**：所有异步容器复用同一扫描条组件。
26. **因为** N.E.K.O 把富消息卡片（音乐播放器）插进对话流（§3.1a），**所以我们应该**：消息模型从第一天就是“卡片列表”（文本/图片/播放器/工具结果），而不是字符串。
27. **因为** N.E.K.O 的字幕设置就贴在字幕上（§3.1b），**所以我们应该**：字幕的不透明度/大小/位置就近可调（配合 my-neuro 的“调整字幕位置/复位字幕位置”，§7.4.4）。
28. **因为** N.E.K.O 的 `subtitle.css` 是全仓唯一彻底变量化的模块（**116 个 `--subtitle-*` + 7 套 `[data-*-scheme]` 配色档**，§3.1c），**所以我们应该**：**每个可换肤浮层都走「语义变量 + 配色档选择器」**，把字幕作为第一个按此规范实现的组件。
29. **因为** VTube Studio 默认**零常驻 UI**、N.E.K.O 用桌宠上的 48px 浮动按钮列（§5、§3.3），**所以我们应该**：陪伴态默认隐藏大部分 UI，只保留可收起的浮动按钮列（48px 命中区）；控制台放在独立入口（Warudo 双窗 / SAP 独立控制台窗，§6、§7.7.2）。
30. **因为** AIRI 支持“从模型提取主题颜色”并把色相渗透到 logo 与窗口标题栏（§2.2），**所以我们应该**：预留“从当前皮套立绘采样 → 派生 UI 色相”的入口（这是把角色与 UI 绑定的高性价比动作）。
31. **因为** Soul of Waifu 的舞台是 **3px 抓边、200–800px 可拖**的右栏（§7.2.1），而 Nexus 的折叠是**真窗口缩放**（§7.8.1），**所以我们应该**：舞台尺寸调整只在**模型配置/复盘模式**下开放（3px 抓边），陪伴态锁死；窗口级形变交给宿主层，UI 层不重复做动画。

---

## 11. 未能核到证据的清单（明确留空）

> 以下内容**我没有拿到一手证据**，因此报告中未作任何断言。列在此处以便后续补齐。

**按维度：**
1. **各项目的真实渲染像素值**（对比度、实际模糊半径、最终圆角）——全部来自静态源码或截图目视，**未做浏览器/桌面渲染测量**。
2. **`@charcoal-ui/tailwind-config` 的具体数值**（ChatVRM/Amica 的 `typography-N` 的 px/rem、`rounded-4/8/16` 的半径、`w-col-span-N`、`bg-surfaceN` 色值）——checkout 内无 `node_modules`，preset 未 vendored。
3. **AIRI 的 `presetChromatic` 源码**：`@proj-airi/unocss-preset-chromatic` **不在本地磁盘**（`plugins/` 下只有 5 个业务插件，`grep baseHue` 在 `plugins/`、`packages/` 均无命中）→ **primary-50…950 的具体色值未能核到**；本报告只引用了它在 `uno.config.ts:158-164` 的调用参数（`baseHue: 220.44`）。
4. **Chakra v3 的 `md` 断点具体 px**——项目未自定义 breakpoints，`node_modules` 不在磁盘；编译产物里的 `@media (max-width:576px/768px)` 经比对属 **chatscope 自带样式**，不是 Chakra。（第 4 章引用的 Chakra token 数值来自编译产物 `assets/main-CKgUHFa9.js` 中抠出的 `defineTokens`，属**框架默认值**而非项目自定义。）
5. **N.E.K.O 的深色模式实际观感**——只读了 `dark-mode.css`（2,903 行）的存在与规模，**未逐条比对**它与 `index.css` 的覆盖差异，也未取得深色截图。
6. **VTube Studio / Warudo / VSeeFace 的视觉数值**（色值、圆角、字号）——闭源，只有官方截图与官方文档，**未做任何数值断言**。
7. **VSeeFace 的官方文档结构**——本轮搜索命中的是第三方教程与 `emilianavt/VSeeFaceManual` 仓库页，**未取得其官方 UI 结构的权威描述**，故本报告未单列 VSeeFace 章节。
8. **N.E.K.O 官网**——`https://project-n-e-k-o.github.io/` 返回 **404**（实测），其官网入口**未核到**；本报告的 N.E.K.O 视觉结论全部来自 **Steam 商店官方截图 + 本仓 CSS**。
9. **各项目的 commit hash / 版本对应关系**——全部为 zip 快照，无 `.git`；AIRI 的截图对应 **0.12.0-beta.5**（`setup-and-use/index.md:19`），OLV-Web 源码对应 `package.json` 的 **1.2.1**。
10. **chatscope 覆写是否在运行时真正生效**——覆写依赖 `--chakra-colors-*` 变量（`sidebar-styles.tsx:483-561`），而编译产物的静态 CSS 里**找不到任何 `--chakra-colors-*` 定义**；需实机验证（已标【推断】）。
11. **AIRI 的字体与图标资产许可**——`Nunito / Comfortaa / DM Sans / Kiwi Maru / ChillRoundM` 等字体与 **Iconify/Solar 图标集**是独立资产，**不随 MIT 一并授权**。
12. **`vaul-vue`（AIRI 移动端抽屉）的内部动画时长/缓动**——源码不在本地。
13. **SAP / Ghost Vessel 的实际渲染截图与对比度测量**——只有源码，未运行。

**按项目 / 按样本细节（UI 证据不完整或仅有部分维度）：**

14. **super-agent-party 的未核到部分**——已核到：主色 `#17827a`/`#c8815a`、9 套主题、50/50 分栏、一级侧栏 200→64px、VRM 独立窗 540×960、一级导航 11 项、系统设置 4 节、12 项缺陷（含 4 处未定义变量）。**未核到**：灵动岛 9 个面板的中文 UI 名（`island.html` 只有英文 class）；`listening/typing/speaking/idle` 四态文案的**实际渲染位置**（模板与主 JS 零引用，**疑似死文案**）；`vrm.js` 内 `#control-panel` 的完整尺寸/字号表（样式由 JS 拼字符串生成）；`tha.html` / `soulx.html` / `shotOverlay.html` / `chat.html` 的视觉样式；打包后的 `element-plus.css` / `fontawesome/*.min.css` 内容。
15. **Ghost Vessel 的未核到部分**——已核到双窗几何、12 变量主题、无设置页（280px 浮层）、静音做法。**未核到**：`bridge/` 侧是否影响视觉；是否有非 MIT 第三方资产清单（仓库内**没有** `LICENSE-third-party`）；`prefers-reduced-motion`（**完全没有**）；`presets/` 内角色资产的具体内容与授权（`presets/README.md:20-24` 只说“可售卖纯数据”）。
16. **Meuxe 的“自曝机制”是否被 CI 强制**——确认了 `--color-*: initial` 机制与它抓到 `clay-600` 的事实，但**未核到**仓库内是否有对应的 CI 检查脚本（可能只靠人工/一次性脚本）。**另**：Meuxe 无深色模式，其 `ChatPanel` 的 `dark` 分支是死代码（§7.3.6.4）。
17. **Soul of Waifu 的 Web 客户端视觉**——只核到 `app/web_client/` 存在第二套 IA 与少量 CSS 事实，**未展开**其完整视觉体系；其 `CompanionSubtitleOverlay` 的**完整样式**（字号/颜色/描边）亦未逐条核到，只核到时序参数。
18. **Nexus 的 Tailwind 工具类实际收益**——依赖在但组件里**零使用**，是计划中还是遗留**未核到**；`tokens.ts` 与 `index.css` 谁是权威**无源码可判定**（两者都被注入，冲突时以 CSSOM 内联优先为准，属【推断】）；`--radius-*` 四档同值是刻意还是复制粘贴错误**未核到**。
19. **ghost-vessel 之外的 LunaMate / Agent-LLM-Live2D / DesktopPetMonitor / deskpet / Norma / yume / aicompanion 等 2025–2026 新项目**——**均未核到 UI 一手证据**，本报告只引用了 GitHub 搜索返回的元数据与 `awesome-ai-companion` 的分类描述。
20. **`Miru` / `VPet-Ultra` / `AI-Vtuber(Ikaros)` / `Neuro-sama` 本身**——本报告未覆盖（前四者已在 `competitive-analysis-report.md` 中做过功能层面比较；Neuro-sama 是闭源直播形态，无 UI 可核）。
21. **所有项目的 Live2D Cubism SDK 与内置皮套资产许可**——`OLV-Web/src/renderer/WebSDK/` 已核到是官方 Cubism SDK 样例（Core 为 **Live2D Proprietary Software License**、仅 3 个文件可再分发；Framework 对**年营收 ≥1000 万日元**企业要求 Cubism SDK Release License）。但 **my-neuro（随仓库分发 `live2dcubismcore.min.js` 且无 notice）、N.E.K.O（有 `assets/yui-*.tar.gz`、`static/mao_pro/` 但无 Cubism 许可文件）、AIRI（内置 `hiyori` 模型）三家的 Cubism 与模型资产条款均未核到**。→ **复用时必须逐项独立确认：代码许可不能替代 SDK 许可。**

---

## 12. 方法与可复现命令

```bash
# 元数据（star / license / 最后推送）
curl -s "https://api.github.com/repos/<owner>/<repo>" | python3 -m json.tool

# 取源码（本机 git clone 不可用：git 全局 http.proxy 指向失效地址）
curl -sL -o X.zip "https://codeload.github.com/<owner>/<repo>/zip/refs/heads/<branch>"
python3 -c "import zipfile; zipfile.ZipFile('X.zip').extractall('x_X')"

# 商店页官方截图（Steam appdetails API 的 screenshots[].path_full）
curl -s "https://store.steampowered.com/api/appdetails?appids=4099310&l=schinese"

# AIRI 文档内 .avif 截图转 PNG 后可直接目视
ffmpeg -y -loglevel error -i manual-settings-window.avif out.png
```

**证书/许可再确认**：第 0.2 节的每一行都来自仓库内 `LICENSE` / `NOTICE` 文件的实际读取，或 GitHub API 的 `license.spdx_id` 字段；OLV-Web 的 `NOASSERTION` 已按 `LICENSE` 正文如实修正为「Apache-2.0 + Additional Conditions」。

---

*报告结束。所有 `未核到` 项集中在第 11 节；如需补齐，优先级建议为：**AIRI 的 `@proj-airi/unocss-preset-chromatic` 源码**（唯一影响“抄色相方案能不能直接落地”的硬缺口）→ **SAP 的四态文案与灵动岛面板**（判断其导航模式是否可抄）→ **VSeeFace 官方文档结构**（补齐商业标杆三角）。*

*本报告全部结论来自 2026-09-10 当日抓取的源码快照与官方截图；star 数、`pushed_at`、license 均标注了查询日期。仓库已归档的 Python 前端（`Live2D-Ai-pc/`）与 `neko-ui-alignment-and-gaps.md` 的对标对象**未被重复调研**，本报告的对标基线是各项目的**当前上游**。*
