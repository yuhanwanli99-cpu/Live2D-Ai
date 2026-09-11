# Live2D-Ai Web UI 设计规格 v2（主 Agent 亲自设计）

> 目标：产品级视觉与交互，对齐"AI 桌宠伴侣"的产品气质。
> 参照：deepseekharness（工程骨架/模态/rail/token）+ N.E.K.O（胶囊/品牌蓝/圆润）+ 桌宠类产品惯例（Live2D 悬浮舞台感）。
> 技术约束：路线 A（vanilla JS + include_str 嵌入），Rust>95%，零前端框架。
> **子代理只照本规格实现，所有视觉数值/结构/交互以此为准。**

---

## 一、设计定位（先定调）

**关键词**：深色沉浸舞台 + 品牌蓝点缀 + 轻胶囊 + 内容优先。

- Live2D 模型是**绝对主角**：周边 UI 一律低饱和、低透明度、退后；只有 hover/聚焦才浮出。
- 情绪色 = 品牌蓝 `#40C5F1`，仅用于：主 CTA、激活态、聚焦光晕、助手消息强调线。**不做大面积蓝**（避免廉价感）。
- 三个信息层级：**舞台（65%）/ 对话（35%）/ 设置（模态）**——每层视觉权重严格递减。

## 二、Design Token v2（在现有基础上升级）

```css
:root {
  /* 品牌色（保留） */
  --brand: #40C5F1; --brand-deep: #22b3ff;
  --brand-glow: rgba(64, 197, 241, .25);        /* 聚焦光晕 */
  /* 表面层级（新增 3 级表面 + 透明变体） */
  --bg-0: #0b0d11;                               /* 最底（body） */
  --bg-1: #12151a;                               /* 面板 */
  --bg-2: #1a1e25;                               /* 卡片/抽屉 */
  --bg-3: #222731;                               /* 悬浮/激活面 */
  --glass: rgba(18, 21, 26, .55);                /* 舞台悬浮件（毛玻璃底） */
  /* 文字 */
  --fg-hi: #eef2f7; --fg: #c9d1dc; --fg-mid: #8a93a3; --fg-low: #5b6472;
  /* 语义 */
  --ok: #34d399; --warn: #fbbf24; --err: #f87171;
  /* 线 */
  --line: #262c36; --line-soft: rgba(255,255,255,.06);
  /* 圆角（胶囊保留用于 pill/按钮；面板用中圆角） */
  --r-pill: 999px; --r-lg: 14px; --r-md: 10px; --r-sm: 6px;
  /* 间距（4 的倍数） */
  --s-1: 4px; --s-2: 8px; --s-3: 12px; --s-4: 16px; --s-5: 24px; --s-6: 32px;
  /* 字体 */
  --font: 'Segoe UI','PingFang SC','Hiragino Sans GB','Microsoft YaHei',sans-serif;
  --mono: 'JetBrains Mono','SF Mono',Consolas,'Courier New',monospace;
  /* 动效 */
  --ease: cubic-bezier(.4,0,.2,1); --ease-out: cubic-bezier(.16,1,.3,1);
  --t-fast: 120ms; --t-norm: 200ms; --t-slow: 320ms;
  /* 阴影（3 级，含发光） */
  --sh-1: 0 1px 3px rgba(0,0,0,.3);
  --sh-2: 0 4px 16px rgba(0,0,0,.4);
  --sh-3: 0 8px 40px rgba(0,0,0,.5);
  --sh-glow: 0 0 0 1px var(--brand-glow), 0 0 12px var(--brand-glow);
}
```
旧 token（--color-*/--radius-*/--spacing-*/--dur）**全部迁移**到新命名，style.css 内无残留。

## 三、整体布局

```
┌─────────────────────────────────────────────────────────────┐
│ topbar（h:44, 玻璃感: --glass + blur, 底线 1px --line）        │
│ ● Live2D-Ai  v…  │pill·连接│pill·WS│pill·LLM│pill·TTS│  ⟳ ⚙ │
├───────────────────────────────────┬─────────────────────────┤
│                                   │ chat-panel（--bg-1）      │
│   stage（65%, --bg-0 纯黑渐晕）      │  ┌───────────────────┐  │
│   模型居中                          │  │ header: 对话 ●状态 │  │
│   ┌────────────┐ 右上: stage-      │  ├───────────────────┤  │
│   │ stage-     │ toolbar(玻璃+毛边) │  │ messages(滚动)     │  │
│   │ toolbar    │ ＋ － ⟲          │  │  气泡×N           │  │
│   └────────────┘                   │  ├───────────────────┤  │
│   左下: stage-hint(极淡 FPS/adapter)│  │ composer          │  │
│                                   │  │ [textarea] [↑][■]  │  │
├───────────────────────────────────┴─────────────────────────┤
│ statusbar（h:24, --bg-1, 极小字: 端点/commit/快捷键提示）        │
└─────────────────────────────────────────────────────────────┘
```

### 3.1 topbar（h 44px）
- 背景：`--glass` + `backdrop-blur(8px)`（悬浮玻璃感，与舞台融为一体而非硬分割）。
- 左：logo-dot（品牌蓝 6px 圆点，呼吸微动画）+ 标题 + ver。
- 中：4 个状态 pill（连接/WS/LLM/TTS），pill = 透明底 + 1px --line + 状态点；**异常态**才出现彩色底（红/黄 8% 透明度），正常态完全中性。
- 右：⟳ 刷新 + ⚙ 设置（icon 按钮 32×28，hover 浮起）。

### 3.2 stage（65% 舞台）
- 背景：`radial-gradient(ellipse at 50% 60%, #131820 0%, #0b0d11 70%)`——**聚光感**（中间稍亮，四角压暗），模型站在"光池"里。
- **stage-toolbar**：右上角，垂直排列改**水平**，`--glass` 底 + 1px --line-soft + blur；按钮 36×36 圆角 --r-md；hover 时 `--bg-3` + 微缩放(1.05)；默认 opacity .0，stage hover 时 .95；三键：＋ ／ － ／ ⟲（复位）。
- **stage-hint**：左下角极淡（--fg-low, 11px mono）一行 HUD（GPU/FPS/sim/idle 诊断）——hover 才显现。
- 模型切换时舞台**淡入淡出**（iframe opacity 过渡，不黑屏）。

### 3.3 chat-panel（35%）
- 背景 `--bg-1`，左边 1px --line 分割。
- **header**（40px）：「对话」+ 右侧状态点（空闲/输入中/播放中 三态色）。
- **messages**：`--bg-1` 上直接铺气泡（无卡片嵌套）；间距 s-3；**打字机流式**光标（▍闪烁已有）；空状态居中极淡文案 + 一个「开始对话」引导箭头。
- **气泡设计**：
  - user：右对齐，`--brand` 10% 透明底 + 1px 品牌蓝 25% 边，圆角 `--r-lg`（右下 4px 小角收）。
  - assistant：左对齐，`--bg-2` 底 + 左侧 2px 品牌蓝竖线，圆角 `--r-lg`（左下 4px）。
  - meta（时间/epoch）：11px --fg-low mono。
  - streaming 时 assistant 气泡尾部光标 `▍` 品牌蓝闪烁。
- **composer**：上边 1px --line；textarea 3 行高自适应（max 6 行）；发送按钮 = **圆形 36px 品牌蓝底、深色箭头图标**（主 CTA，全 UI 唯一大蓝块）；停止按钮仅播放中出现（红色描边幽灵按钮）；Enter 发送 / Shift+Enter 换行提示 11px 放 composer 右下。

### 3.4 statusbar（新增，h 24px）
- 极小 11px --fg-low mono：`127.0.0.1:18080 · AGPL-3.0 · Mod 5/5 · WASM a1b2c3 · FPS 60`
- 目的：把 footer 的技术信息收进来，**去掉页面底部大 footer**。

## 四、设置模态（重设计）

```
┌───────────────────────────────────────────────┐
│ 设置                                    ✕     │  header h:52
├──────────┬────────────────────────────────────┤
│ ▎外观舞台  │  ┌──────────────────────────────┐  │
│  AI语音   │  │  section 标题（16px 字重600）    │  │
│  角色     │  │  ─────────────────────────    │  │
│  动作互动  │  │  [字段行 ×N]                   │  │
│  模型     │  │  （一列字段, 标签左/控件右对齐行）  │  │
│  系统状态  │  └──────────────────────────────┘  │
│  开发者   │                                    │
│  Mod     │                                    │
├──────────┴────────────────────────────────────┤
│ savebar: 状态文案……        [取消隐含] [保存💾]   │
└───────────────────────────────────────────────┘
```
- 模态：**1020×680**，圆角 `--r-lg`+`--sh-3`，入场动画：scale .96→1 + fade（--ease-out 320ms）。
- **左 rail**（180px）：项 = 12px 图标（字符图标：🎨🎤👤🎭🧩📊🛠🧩）+ 文字 13px；激活项 = 品牌蓝左竖条 3px + 文字 --fg-hi + `--bg-3` 底；hover = --bg-2。
- **字段行**（统一 row 规格）：label 左（13px --fg-mid）、控件右（宽 ≤320px）；输入框 = `--bg-2` 底 + 1px --line + 聚焦品牌蓝描边+glow；select 同；checkbox = 品牌蓝选中。
- **保存栏**：右对齐，保存按钮品牌蓝胶囊；未保存变更时「保存」按钮出现**呼吸微光**（--sh-glow 动画）提示。
- 分组间留白 s-5，字段行间 s-3，**禁止卡片套卡片**（去掉 .card 嵌套，直接平铺字段行于 section 内，用分组标题分隔）。

## 五、动效体系（"看着像样"的关键）

| 场景 | 动效 |
|---|---|
| 设置模态开 | scale .96→1 + fade，--ease-out 320ms |
| rail 切换 | section 内容 fade+translateX(8px→0) 200ms |
| 消息上屏 | translateY(6px→0)+fade 240ms，流式光标闪烁 |
| 悬浮件显隐 | opacity 160ms |
| 按钮 hover | background 120ms + transform 1.02 |
| 保存按钮 | 未保存时 glow 呼吸（2s 循环） |
| 模型切换 | stage fade 240ms（iframe opacity） |

**统一规则**：所有过渡只动 `opacity/transform`（合成层，不触发 layout/reflow——这就是抖动治理的延续）。

## 六、可访问性
- 全部交互件 `:focus-visible` 品牌蓝 2px outline。
- 模态 Esc/遮罩关闭（已有）+ 焦点圈存（打开时聚焦到 rail 第一项）。
- 对比度：--fg-hi/--fg 对 --bg-1 ≥ 7:1；--fg-mid ≥ 4.5:1。

## 七、文件与行数预算
- index.html：≤260（结构 + statusbar 新增 + svg 图标内联）
- style.css：≤520（token v2 + 全组件重写——允许大改）
- app.js：≤760（新增 statusbar 渲染/pill/未保存检测；现有逻辑保留）
- chat.js：不动（除非 class 名联动）

## 八、实现任务拆分（子代理照规格实现，两批）

**批1（骨架+舞台+对话）**：token v2 替换 + 布局三区 + topbar pill/icon + stage 渐晕/toolbar/hint + statusbar + 气泡/composer 全套 + 动效。
**批2（设置模态重排）**：rail 图标化 + 字段行统一（去卡片嵌套）+ 保存呼吸光 + 入场动画 + 未保存检测。

## 九、验收（你逐条过）
1. 第一眼：深色沉浸舞台，模型是绝对主角，UI 元素全部退后。
2. 品牌蓝只出现在：logo 点、发送按钮、激活 rail、助手左线、聚焦描边——无大面积蓝。
3. 设置：rail+一次一 section、字段行对齐、无卡片套卡片、保存呼吸光。
4. 动效：模态/rail/消息/悬浮件全有过渡，无硬切。
5. FPS 不降（无每帧 DOM 写回归——沿用脏检查）。
6. 现有全部 id 兼容。
