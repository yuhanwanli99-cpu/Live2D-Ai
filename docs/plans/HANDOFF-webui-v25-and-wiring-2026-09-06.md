# 交接：Web UI v2.5 真实迁移 + 设置接线 v1（2026-09-06）

> 本轮会话的完整交接。回滚锚点链：`p3-webui-v25-migrated` → `p3-settings-wiring-v1`。
> 分支 `dev/integrity`。

## 1. 本轮完成（按 commit）

### `74a4f66e` — v2.5 UI 迁移到真实 App（tag `p3-webui-v25-migrated`）
- `web_api/index.html` / `style.css` / `app.js` 全量替换为 v2.5 设计（源自 `docs/design/ui-preview-v2.html`）：
  6-rail `data-s`、6 分区 `data-p`、SVG sprite（`i-user/i-palette/i-mic/i-motion/i-tools/i-puzzle`…）、
  `footer.statusbar`、`#actPanel` 浮窗、`/render` iframe 舞台。
- `tests_html.rs`：旧占位锚点 `Live2D 展示区` → 真实 `id="live2d-frame"` + `src="/render"`。

### `a1a89458` — 设置打不开第一轮修复
- `#settings-overlay` 补 `class="overlay"`（迁移丢类）。
- `renderGroups()` 的 `grp-logs` 空引用加守卫（v2.5 HTML 已无该 id）。
- `.act-grid .act-btn[data-act]` 全局委托 → `actFire`（此前只有浮窗绑定）。

### `d401a072` — 设置不可见第二轮修复（关键）
- 根因：CSS 用类选择器 `.settings-modal`，HTML 只有 `id="settings-modal"` 没有 class →
  模态无样式（透明、无定位）→「黑幕出现但看不到设置」。
- `#settings-modal` 补 `class="settings-modal"`；补 `.live2d-display/.settings-section-area/`
  `.chat-status/.chat-title/.dot.idle/.row.buttons` 等缺失 CSS。
- 门禁测试锁死两个 class 属性。

### `7e4c0bbe` — rail 文字不显示修复
- `.rail-item .lbl-t { display:none }`（preview 遗留，preview 文字是裸文本节点所以没暴露）→ `flex:1`。

### `f1395232` — 设置接线 v1（tag `p3-settings-wiring-v1`，规格 `docs/design/web-ui-settings-wiring-v1.md`）
- **A1 角色卡**：`PersonaSettings` +`name`/`description`（toml 持久化、PATCH 三态 null=清除）；
  `PersonaView`/`PersonaPatch` 同步；`build_effective_system_prompt()` 把角色卡注入会话 system_prompt
  （5 分支单测）。egui F10 面板不暴露（走 Web UI）。
- **B1 互动开关**：wasm `stage-config` 新增 `idleEnabled`/`clickEnabled`；idle=false 跳过呼吸/眨眼/微表情；
  click=false 禁拖拽/滚轮/双击复位。前端 `f-idle`/`f-click` → `postStageConfig`。
- **C2 背景图（本地）**：`#f-bgimg-btn/input/clear` → FileReader dataURL → `stage-bg` →
  wasm canvas CSS `background-image: cover`；localStorage `dsh.stage.bg` 记忆；不进后端。
- 顺手清理：删既有死函数 `scale_of`（clippy 欠账）+ fmt 规范化 `layout_probe.rs`。

## 2. 当前门禁状态（全绿）
- fmt / clippy `-D warnings` / workspace 全量测试（desktop 479）/ doc / rust-ratio **95.34%**。
- trunk 构建新 wasm：`l2d-wasm-demo-61295c3883bf419e_bg.wasm`（旧 11c4aa6e 已被覆盖）。
- 实起 curl 验证过：PATCH persona name/desc 写→读→清除、新控件 id、wasm hash。
- `node --check` app.js / chat.js 通过。

## 3. 本轮踩过的坑（重要经验）
1. **迁移丢 class 是系统性模式**：preview 用 `class=… id=…`，迁移只搬 id。已加门禁测试锁
   `overlay`/`settings-modal` 两个 class；**其余组件未锁**，后续迁移再出「样式不生效」先查这个。
2. **CARGO_HOME=/tmp/cargo 会 timeout**（ayagami git fetch 不通）；用默认 `~/.cargo`（缓存齐全）。
3. **PATCH 需要 Origin 头**（密钥安全设计）：curl 测试要带
   `-H 'Origin: http://127.0.0.1:PORT'`，否则 403 origin_required。
4. stub 模拟（node + 假 DOM）能证明「无顶层异常」但证明不了视觉问题；
   本轮两次视觉 bug 都是 CSS 选择器失配，最终靠「黑幕出现=JS 正常」这类现象推理定位。

## 4. 未做完 / 后续工作

### 4.1 UI/接线遗留（优先级从高到低）

> **2026-09-06 交接更新（本接手会话已完成下划线项）**：
> - **已修复**：`modelsList`/`modelsStatus`/`btnRefreshModels` 缺 DOM → 新增「模型管理」rail 第 7 分区 +
>   section，接入既有 `loadModels/renderModels/activateModel`（commit `4c7dcf8c`）。
> - **已修复**：`appScale`/`appBgDark` 缺 DOM + `bindAppearance` 提前 return → 外观区补缩放滑杆/暗色开关/
>   口型联动；`bindAppearance` 改每控件独立绑定（缺一个不影响其余），并初始同步一次 stage-config；
>   删除无效 `segColor`（发 `bg` 死字段）；动作浮窗强度 seg 接入 `segSel`。修复前口型开关点击不生效。
> - **已清理**：`grp-logs`/`badge`/`log-line`/`log-stream`/`mod-state` 死 CSS 规则；删 `layout_probe.rs`
>   探针（头注"验证后删除"，commit `16ee801f`）。
> - **已加提示**：f-bgimg 大 dataURL 超 localStorage 限额 → 提示"图片过大，刷新后不会保留"（不再静默）。
> - **已验证（curl）**：服务返回 7 分区 + 新控件 DOM；`/api/v1/models` 可用；`/render` 页引用
>   `61295c3883bf419e` wasm 且 5.3MB 正常 serve。门禁全绿。
> - **日志分级 UI**：开发者→实时日志区新增「级别≥」下拉（`id=logLevel`），由
>   `populateLogLevels` 从 `/api/v1/logs/levels` 填充；`refreshLogs` 按级别过滤显示
>   （`LOG_LEVEL_RANK` / `filterLogLines`，选中 info 显示 info+warn+error，隐藏行数末尾提示）。
>   commit `19ba4268`。门禁测试 `index_html_template_has_log_level_filter_ui` 锁前端消费。
> - **后端/功能性 staging 已动工**：`/api/v1/logs/levels` 端点已存在并测试，前端消费已接（见上）；
>   其余 4.2 项仍未动。

待办（需外部环境浏览器 / 真 LLM 端点，或下轮继续）：
- [ ] **背景图在 `/render` iframe 内的实测**：C2 用 wasm canvas CSS 实现，已 curl 确认消息链路，
      但人类视觉验收未做（jpg 是否铺满、clear 是否恢复底色）。**需浏览器环境做视觉验收。**
- [ ] **角色卡 LLM 注入的实测**：填 name 后新对话，LLM 请求里的 system 是否带 `[角色卡]`——
      代码路径已测（`resolve()` → `build_effective_system_prompt`，单元测试 5 分支覆盖），
      端到端（真 LLM 对话）未跑。**需真 LLM 端点做对话验证。**
- [ ] 待重启徽章（apply_status=restart_required 的 UI 提示）只有文案映射，无 badge 组件。

### 4.2 后端/功能性 staging
- [x] ~~RUST_LOG 分级进 logs/levels UI~~ → 已完成（commit `19ba4268`）
- [ ] 背景 jpg 导入走后端（当前 C2 是纯本地方案）
- [ ] texture 尺寸档位（4096/8192/16384）
- [ ] director 事件 producer（supervisor→dispatch_event）
- [ ] chat history API
- [ ] home-places `name`/model config
- [ ] RUST_LOG 分级进 logs/levels UI

### 4.3 已知技术债
- `crates/l2d/tests/layout_probe.rs` 头注写着「验证后删除」——探针已验证过左乘语义，可删。
- 浏览器缓存反复坑：`run-web.sh` 后必须 Ctrl+Shift+R；用户报「旧实现」时先查 dist hash 与二进制 mtime
  （`scripts/diag-stale.sh` 五步诊断）。

## 5. 关键文件索引
| 域 | 路径 |
|---|---|
| 前端三件套 | `crates/live2d-ai-desktop/src/web_api/{index.html,style.css,app.js}`（chat.js 勿动） |
| 嵌入+门禁测试 | `crates/live2d-ai-desktop/src/web_api/index_html.rs`、`tests_html.rs` |
| persona 后端 | `crates/live2d-ai-runtime/src/settings{.rs,/view.rs,/patch.rs,/settings_tests.rs}` |
| wasm bridge | `crates/l2d-wasm-demo/src/main.rs`（消息分发）、`src/web/surface.rs`（BridgeState/tick） |
| 本轮规格 | `docs/design/web-ui-settings-wiring-v1.md` |
| 一键脚本 | `scripts/run-web.sh`（构建+启动）、`scripts/diag-stale.sh`（旧实现诊断） |
