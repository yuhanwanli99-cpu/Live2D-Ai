# Web UI 重做 — 阶段1+2 实现规格（主 Agent 定，子代理严格照做）

> 依据：`docs/plans/web-ui-redo-plan.md`（用户已审核，路线 A / 前端豁免 / 品牌蓝确认）。
> 哲学：deepseekharness（触发行+居中模态+左导航 rail）+ NEKO（胶囊化品牌蓝）+ Rust>95%（路线 A vanilla，前端豁免）。
> 子代理**不得自由发挥设计**：结构/类名/id/样式按本规格字面。

## 涉及文件（仅前端 3 文件 + 前端豁免改 xtask）

### A. 前端豁免（xtask，Rust 小改动）
- 文件：`/home/skystar/deepseekharness/Live2D-Ai/xtask/src/main.rs`
- 在 `walk()` 的目录判断处，加：若路径等于/包含 `crates/live2d-ai-desktop/src/web_api` 则跳过（该目录是 include_str 前端文件，豁免）。实现：`if path.ends_with("src/web_api") { continue; }` 或遍历时判断目录名。加注释："前端豁免：web_api 的前端文件（index.html/app.js/chat.js/style.css）经 include_str 嵌入，不计 Rust 占比"。
- 验证：`cargo run -p xtask -- rust-ratio` 仍 >95%，且明细里不再含 web_api 的 .js/.html。

### B. 阶段1：design token 体系
文件：`/home/skystar/deepseekharness/Live2D-Ai/crates/live2d-ai-desktop/src/web_api/style.css`
- 重写 `:root`，加入统一 token（NEKO 命名 + deepseekharness 结构）：
```css
:root {
  /* 色彩（NEKO 品牌蓝为基调） */
  --color-main: #40C5F1;          /* 主品牌蓝：标题/主按钮/激活态 */
  --color-main-deep: #22b3ff;     /* 描边/聚焦光晕 */
  --color-main-light: #e3f4ff;    /* 浅背景蓝 */
  --color-border: #b3e5fc;        /* 辅助边框 */
  --color-bg: #0f1115; --color-panel: #1a1d23; --color-panel-2: #232830;
  --color-border-soft: #2d333c;
  --color-fg: #d8dee9; --color-fg-dim: #8a93a3;
  --color-ok: #2ecc71; --color-warn: #f39c12; --color-err: #ff5252;
  /* 圆角（胶囊化） */
  --radius-capsule: 50px; --radius-pill: 999px; --radius-card: 20px; --radius-sm: 10px;
  /* 间距 */
  --spacing-xs: 4px; --spacing-sm: 8px; --spacing-md: 12px; --spacing-lg: 16px; --spacing-xl: 24px;
  /* 字体（NEKO + 技术字段 Courier） */
  --font-sans: 'Segoe UI','PingFang SC','Hiragino Sans GB','Microsoft YaHei',sans-serif;
  --font-mono: 'SF Mono','JetBrains Mono','Consolas','Courier New',monospace;
  /* 动效（deepseekharness ease 曲线） */
  --ease: cubic-bezier(0.4,0,0.2,1); --dur: 0.2s; --dur-fast: 0.1s;
}
```
- 把现有 style.css 里的**硬编码色值**全部替换为 token（现有 `#101418`/`#1a1d23`/`#5e9ed6` 等换 `--color-*`）。能换尽换，不残留硬编码。
- 保留现有类（.app-layout/.chat-msg/.stage-toolbar 等）语义，只是色值走 token。

### C. 阶段2：设置模态 + 左导航 rail（改平铺）——核心
文件：`/home/skystar/deepseekharness/Live2D-Ai/crates/live2d-ai-desktop/src/web_api/index.html` + `app.js` + `style.css`

**C1. index.html 结构**（替换侧滑 `#settings-drawer` 为模态+rail）：
```html
<!-- 触发行（原 topbar ⚙ 保留，点击开模态） -->
<!-- 模态面板 -->
<div id="settings-overlay" hidden></div>
<div id="settings-modal" hidden role="dialog" aria-modal="true" aria-labelledby="settingsTitle">
  <header class="settings-modal-header">
    <h2 id="settingsTitle">设置</h2>
    <button class="settings-close" id="btnSettingsClose" aria-label="关闭">✕</button>
  </header>
  <div class="settings-modal-body">
    <nav class="settings-rail" id="settingsRail">
      <!-- 由 app.js 按 section 注册生成；每项 data-section -->
    </nav>
    <div class="settings-section-area" id="settingsSectionArea">
      <!-- 一次只渲染一个 section -->
      <section data-panel="appearance" class="settings-section">…外观…</section>
      <section data-panel="ai" class="settings-section">…AI与语音…</section>
      <section data-panel="character" class="settings-section">…角色…</section>
      <section data-panel="actions" class="settings-section">…动作…</section>
      <section data-panel="models" class="settings-section">…模型…</section>
      <section data-panel="status" class="settings-section">…系统状态…</section>
      <section data-panel="dev" class="settings-section">…开发者…</section>
      <section data-panel="mods" class="settings-section">…Mod…</section>
    </div>
  </div>
  <footer class="settings-modal-footer" id="drawer-savebar">
    <span class="status" id="saveStatus"></span>
    <div class="row buttons"><button id="btnSave">保存</button>…</div>
  </footer>
</div>
```
- **保留全部现有 id**（llmBaseUrl/ttsBaseUrl/systemPrompt/devMode/btnSave/btns/appScale/appBgDark/appLipSyncToggle/actionsList/modelsList/btns/logsCard/logs 等）——它们从原分组**原样搬进**对应 `data-panel` section，id 不变（app.js/chat.js 引用不破）。
- **取消平铺**：8 个 section 全在 DOM，但**每次只显示一个**（CSS `.settings-section { display:none } .settings-section.active { display:block }`）。

**C2. style.css 布局（模态+rail）**：
```css
#settings-modal { position:fixed; inset:0; margin:auto; width:min(1080px,95vw); height:min(700px,90vh);
  background:var(--color-panel); border-radius:var(--radius-card); box-shadow:...; z-index:200;
  display:flex; flex-direction:column; overflow:hidden; }
#settings-overlay { position:fixed; inset:0; background:rgba(0,0,0,.5); z-index:199; }
.settings-modal-header { display:flex; justify-content:space-between; align-items:center; padding:16px 24px; border-bottom:1px solid var(--color-border-soft); }
.settings-modal-body { flex:1; display:flex; overflow:hidden; }
.settings-rail { width:200px; flex-shrink:0; padding:12px 8px; border-right:1px solid var(--color-border-soft); overflow-y:auto; }
.settings-rail-item { display:flex; align-items:center; gap:10px; padding:10px 14px; border-radius:var(--radius-sm); cursor:pointer; color:var(--color-fg-dim); }
.settings-rail-item:hover { background:var(--color-panel-2); }
.settings-rail-item.active { background:var(--color-main-light); color:var(--color-main); font-weight:600; }
.settings-section-area { flex:1; overflow-y:auto; padding:24px; }
.settings-section { display:none; }
.settings-section.active { display:block; }
.settings-modal-footer { padding:12px 24px; border-top:1px solid var(--color-border-soft); background:var(--color-panel-2); }
/* 胶囊化按钮/输入框 */
button, .field-row input, .field-row select, textarea { border-radius:var(--radius-capsule); }
```
- `#settings-drawer` 旧样式清掉（换 `#settings-modal`）。

**C3. app.js（rail 切换 + 触发行开模态 + capability 驱动）**：
```js
// section 注册（按 deepseekharness slots 思想）
const SETTINGS_SECTIONS = [
  { id:"appearance", label:"外观" }, { id:"ai", label:"AI 与语音" },
  { id:"character", label:"角色" }, { id:"actions", label:"动作" },
  { id:"models", label:"模型" }, { id:"status", label:"系统状态" },
  { id:"dev", label:"开发者" }, { id:"mods", label:"Mod" },
];
let activeSection = "ai";
function setActiveSection(id) {
  activeSection = id;
  document.querySelectorAll(".settings-rail-item").forEach(r => r.classList.toggle("active", r.dataset.section===id));
  document.querySelectorAll(".settings-section").forEach(s => s.classList.toggle("active", s.dataset.panel===id));
}
function openSettings() { settingsModal.hidden=false; settingsOverlay.hidden=false; setActiveSection(activeSection); }
function closeSettings() { settingsModal.hidden=true; settingsOverlay.hidden=true; }
// 触发行（topbar ⚙ btnSettings）→ openSettings
// overlay 点击/Esc → closeSettings
// 渲染 rail：SETTINGS_SECTIONS.map(生成 .settings-rail-item[data-section]) + capability 驱动(复用 renderGroups 逻辑：capabilities 决定显隐)
```
- 之前 `setDrawer/toggleDrawer/closeDrawer/lazyLoadDrawer` 换成 `openSettings/closeSettings/setActiveSection`。
- `btnSettings` 点击 → openSettings；`#settings-overlay`/`Esc`/`#btnSettingsClose` → closeSettings。
- `renderGroups` 的 capability 驱动逻辑迁移到 `setActiveSection` 时按 section 校验。
- **行数**：index ≤240、style ≤380、app ≤700。

## 验收
1. `node --check app.js` 过。
2. 设置不再是平铺——左 rail（8 section）+ 一次一个 section。
3. 品牌蓝 `#40C5F1` 主色 + 胶囊化圆角 + `--color-*`/`--radius-*` token 无硬编码残留。
4. 现有 id 全保留（llmBaseUrl 等 55 个 + chat-* 全在）。
5. `cargo run -p xtask -- rust-ratio` >95%（web_api 前端豁免后）。
6. 模态可关（✕/遮罩/Esc）、rail 切换平滑、存储条在底部。

## 子代理执行纪律
- 严格照本规格字面实现（类名/id/样式名不变），不"改良"、不自由发挥。
- 先 C1 index.html → C2 style.css → C3 app.js → A（xtask 豁免）。
- 每步保存；最后 `node --check` + xtask rust-ratio 验证。
