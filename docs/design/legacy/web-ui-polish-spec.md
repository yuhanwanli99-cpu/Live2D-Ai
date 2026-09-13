# Live2D-Ai Web UI 精修设计规格（v1）

> 主 Agent 亲自产出，子代理严格照此实现，**不得自由发挥设计**。
> 目标：修正"很粗糙"的观感 + 补独立复位按键 + 整体视觉/排版精修。
> 只动前端 3 文件：index.html / style.css / app.js（+ l2d-wasm-demo 的 reset 消息接收，主 Agent 处理）。

---

## A. 复位独立按键（用户明确要求，参照 py 交互控件）

### A1. 主页 65% 渲染区放一个悬浮控制条（不是笨重大按钮）
位置：`#live2d-display` 右上角，半透明悬浮（不遮挡模型），进入区域淡入。

**HTML**（index.html，`#live2d-display` 内、iframe 之后）：
```html
<div class="stage-toolbar" id="stageToolbar">
  <button class="stage-btn" id="btnZoomIn"  title="放大">＋</button>
  <button class="stage-btn" id="btnZoomOut" title="缩小">－</button>
  <button class="stage-btn" id="btnResetView" title="复位视角">⟲</button>
</div>
```

**CSS**（style.css）：
```css
.stage-toolbar {
  position: absolute; top: 12px; right: 12px; z-index: 5;
  display: flex; gap: 6px; padding: 4px;
  background: rgba(16, 20, 24, 0.35); backdrop-filter: blur(6px);
  border: 1px solid rgba(255,255,255,0.12); border-radius: 10px;
  opacity: 0; transition: opacity .2s ease; pointer-events: none;
}
#live2d-display:hover .stage-toolbar { opacity: 1; pointer-events: auto; }
.stage-btn {
  width: 32px; height: 32px; border-radius: 8px; border: 0;
  background: rgba(255,255,255,0.10); color: var(--fg); font-size: 16px;
  cursor: pointer; transition: background .15s ease;
}
.stage-btn:hover { background: rgba(255,255,255,0.20); }
```

**app.js**（复用现有 `postToStage` 发消息）：
```js
els.btnZoomIn.addEventListener("click", () => postToStage({ type: "stage-config", scale: clamp(scaleOf() * 1.1, .5, 2) }));
els.btnZoomOut.addEventListener("click", () => postToStage({ type: "stage-config", scale: clamp(scaleOf() / 1.1, .5, 2) }));
els.btnResetView.addEventListener("click", () => postToStage({ type: "stage-config", scale: 1.0, off: 0 }));
```
- 需要一个 `scaleOf()`：从 iframe 侧读当前 scale？跨 iframe 读不到。**简化**：`stage-config` 复用现有字段，加一个 `reset: true` FLAG。见 A2。

### A2. **wasm 侧加 reset 专用消息**（主 Agent 处理，这里只写接口）
现有 `stage-config` 已支持 scale/offset。为保证"复位"同时归零 scale + offset，wasm 的 listener 对 `stage-config` 若收到 `reset: true` → `scale=1.0; offset_x=0; offset_y=0`（同现有 dblclick 逻辑）。**接口约定**：`{type:"stage-config", reset:true}`。
- 前端 btnResetView 发 `{type:"stage-config", reset:true}`；
- btnZoomIn/Out 发 `{type:"stage-config", scale:<现值±10%>}` —— 现值前端不知道，**改为 wasm 每次 scale 变化时回传一个 `stage-state` 消息**（`{type:"stage-state", scale}`），前端缓存。**接口约定**：wasm 在 apply_bridge_effects 里若 scale 变化 → `postToStage`? 不行，wasm 在 iframe 内不能 postToStage 到父……用 `window.parent.postMessage`。**补接口**：wasm 每帧（或 scale 变化时）`window.parent.postMessage({type:"stage-state", scale}, "*")`。前端监听 `message` 收 `stage-state` 更新缓存 scale，供 ± 按钮计算。

> 简化（避免 wasm-frontend 双向复杂化）：**± 按钮直接不依赖现值**，改用 reset+固定档位：只有「复位⟲」和「放大/缩小」三个动作；放大/缩微发 `{type:"stage-config", scale: cur*1.1}` 无法取 cur…… **最简可行**：± 按钮也在 wasm 内实现——发 `{type:"stage-zoom", dir:"in"|"out"|"reset"}`，wasm listener 收到后按方向调 `scale = (scale*1.1|/1.1|1.0).clamp(0.5,2)` + offset 归零（reset）。**最终接口**：
```
{type:"stage-zoom", dir:"in"}   → scale *= 1.1 (clamp 0.5..2)
{type:"stage-zoom", dir:"out"}  → scale /= 1.1
{type:"stage-zoom", dir:"reset"}→ scale=1.0, offset_x=0, offset_y=0
```

**前端**：btnZoomIn→`postToStage({type:"stage-zoom",dir:"in"})`；btnZoomOut→`"out"`；btnResetView→`"reset"`。wasm 处理（主 Agent 加 listener 分支）。

---

## B. 视觉/排版精修（修粗糙感）

### B1. 全局基调
- `background` 从纯色改为**轻微径向渐变**（深色氛围，不干扰模型）：
```css
body { background: radial-gradient(1200px 600px at 30% -10%, #1b2029 0%, #0f1115 55%); }
```
- 圆角统一：`--radius: 10px`；卡片/面板/pill 统一用它。

### B2. topbar 精修
- 去掉"裸按钮"，状态徽标做成**胶囊 pill**（带色点 + 文字），右端"刷新/设置"做成**图标按钮**。
- 徽标：`<span class="pill"><span class="dot ok"></span>已连接</span>` 样。
- 标题左端加一个小 logo 圆点 + Live2D-Ai 字重 600。

### B3. 对话面板精修
- 消息气泡：最大宽度 78%，用户右 / 助手左；给助手气泡加**轻微左侧竖线强调**（主题色）。
- 消息区 padding/字号微调，滚动条样式改用细型。
- 输入区：textarea 无边框深色内嵌，发送按钮主题色圆角，Stop 按钮 danger，一行放置；输入聚焦时发光边。

### B4. 设置抽屉精修
- 抽屉宽 440px，分组标题加**主题色左边线**（2px）+ 小字距。
- 分组之间 1px 分隔线，不堆大 margin。
- 导航锚点行做**当前项高亮**（active class）。
- 保存栏固定 bottom，加顶部阴影。

### B5. 字体/留白
- root 字号 14px；标题 h1 16px；h2 13px 字重 600 带字距。
- 统一 `--fg` 亮度；禁用态统一 opacity .45 + 禁 pointer。

---

## C. 改动范围与验收

**只允许子代理改**：index.html / style.css / app.js（A+B 的 HTML/CSS/前端逻辑）。
**wasm 侧**（A2 的 `stage-zoom` listener 分支 + `reset`）：主 Agent 处理，子代理不要碰 wasm crate。

**验收**（视觉，需你确认）：
1. 主页 65% 区右上角悬浮控制条：hover 出现，`+`、`−`、`⟲` 三个圆角按钮。
2. 点 ⟲ → 模型复位（scale=1, offset=0）；+/− → 缩放。
3. 整体观感：间距/圆角/徽标 pill/气泡左线/抽屉分组左边线 —— 明显比现在"精致"。
4. `node --check app.js` 过；行数 ≤ 合理；不破坏现有 id 与功能。

## D. 实现顺序建议（子代理照做）
1. 先改 index.html 加 A1 的 toolbar 结构 + B 的类结构（新增 id 不破坏现有）。
2. 改 style.css 加 A1 + B 全套样式（保留现有类兼容）。
3. 改 app.js 加 btnZoomIn/Out/ResetView 监听 + `postToStage({type:"stage-zoom",...})` + B2 徽标 pill 逻辑（若改动徽标 render）+ `node --check`。
4. 报告每步。
