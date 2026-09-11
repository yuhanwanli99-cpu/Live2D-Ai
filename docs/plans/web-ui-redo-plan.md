# Web UI 重做计划表（待审核）

> 依据：deepseekharness（本机，MIT）抽屉/rail 架构哲学 + N.E.K.O（外部）胶囊化视觉 + codex 信息密度。
> 硬约束：**Rust 占比 >95%**（不引 React/Cordis/任何前端框架；保持非框架 vanilla JS + include_str 嵌入，或纯 Rust 渲染）。
> 目标：改掉"设置分类平铺"，做产品级 UI。

## 一、UI 架构哲学（三家共识，照做）

1. **设置 = 模态面板 + 左导航 rail**（deepseekharness）：触发行 → 居中模态（1080x700）→ 左栏 section 导航（一次只看一个 section）→ 取消平铺。
2. **统一 design token**（三家唯一绝对共识）：色板/字体栈/ease 曲线/圆角/间距/阴影，单一来源 `:root`。
3. **Section 按域组合**（deepseekharness slots 思想）：general/models/plugins 等独立注册，capability 驱动显隐。
4. **视觉识别**（NEKO）：胶囊化大圆角 + 品牌蓝，但视觉风格可后续定——工程骨架优先。
5. **CSS 隔离**（deepseekharness CSS Modules 思想）：scoped 命名，不全局污染。

## 二、Rust >95% 的实现路径（关键决策）

当前前端是 `include_str!` 嵌入的 vanilla JS（index.html/app.js/chat.js/style.css）——**Rust 占比已 >95%（前端几十行 JS 不拉低比例）**。重做两条路：

### 路线 A（保守，推荐先做）：非框架 vanilla JS + 新结构
- 保持 include_str 嵌入（Rust 占比不变）。
- 重写 index.html（模态 + rail 结构）+ style.css（design token 体系）+ app.js（rail 切换 + capability 驱动 + 触发行开模态）。
- **零新依赖**，Rust 占比不变（仍 >95%）。
- 风险最低，先跑通再谈 Rust 化。

### 路线 B（激进 Rust 化）：用 Rust 生成 UI / 引入轻量 Rust 前端框架
- 如 `seed`/`yew`（Rust→WASM 前端框架）渲染 UI——但会拉低 Rust 占比（WASM vs native）且重写全部前端逻辑，工程量大。
- 或 Rust 端 googletest 生成 HTML（服务端渲染）——保持 Rust 侧逻辑，前端仅 shell。
- **不推荐立即做**：收益低（前端逻辑简单）、成本高（重写 + WASM 工具链），且可能违背"Rust>95% 是全局占比"（WASM 前端编译产物算 Rust 吗？需澄清）。

**结论**：路线 A（vanilla 非框架 + 新结构）满足 Rust>95% 且最稳；路线 B 作为远期，另行评估。

## 三、落地步骤（计划表，待审核后执行）

### 阶段1：design token 体系（先立骨架）
- [ ] 重写 `style.css` 的 `:root`：色板（参考 NEKO 的 `--color-n-*` 命名 + deepseekharness 的 `--dsw-*` 命名）、字体栈、ease 曲线、圆角(`--radius-*`)、间距(`--spacing-*`)、阴影。
- [ ] 统一改用 token（现 style.css 里硬编码色值全替换为变量）。

### 阶段2：设置模态 + 左导航 rail（改平铺）
- [ ] index.html：`#settings-drawer` 改造成「触发行 → 模态面板 → 左导航 rail + 右侧一次一个 section」结构。
  - 触发行：主页右下角/右上角「⚙ 设置」+ 当前激活 section 提示。
  - 模态：mask + 居中 panel（~1080x700）。
  - 左 rail：section 列表（外观/AI语音/角色/动作/模型/系统状态/开发者/Mod管理——或合并成 deepseekharness 的 general/models/plugins 式），点击切换右侧一次性显示。
  - 右侧：一次一个 section（不是 8 分组全铺）。
- [ ] style.css：模态/rail/panel 布局样式 + 激活态高亮 + 过渡动画。
- [ ] app.js：rail 切换逻辑（active section 状态管理）+ 触发行开模态 + capability 驱动显隐（复用现有 renderGroups 的 capabilities 逻辑）。

### 阶段3：视觉精修（NEKO 胶囊化 + token 应用）
- [ ] 按钮/输入框/标签胶囊化（大圆角 `--radius-capsule`）。
- [ ] 主色调/聚焦光晕（品牌蓝 + 描边）。
- [ ] 聊天气泡/消息区/空状态/加载态/错误反馈 用 token 统一。

### 阶段4：真实链路 + 主要功能接入
- [ ] 仅模型缩放、背景导入、消息链路（这些是核心增强，等同 P0）——确保 UI 重做后仍接得上。

## 四、验收标准

1. 设置不再是平铺——左 rail + 一次一个 section。
2. 统一 design token（无硬编码色值残留）。
3. 彩色/字体/圆角一致，观感"精致"（对比现平铺版）。
4. Rust 占比仍 >95%（路线 A 不变）。
5. 现有 id 兼容（app.js/chat.js 引用不破）或同步迁移。
6. 模态可关（Esc/遮罩）、rail 切换流畅有过渡。

## 五、风险与待澄清

1. **Rust 占比定义**：路线 A（include_str 嵌入的 vanilla JS）算 Rust 吗？—— include_str 嵌入前端文件，ROC 物理行按 Rust 源码计，前端 JS 行不算 Rust 但占比公式是 Rust 行/总行——前端 JS 行会增加分母（拉低占比）。需确认占比计算是否含 include_str 嵌入的前端文件行。若含，路线 A 的少数 JS 行影响极小（几百行 vs 5万行 Rust）。
2. **路线 B 的 Rust/ui框架**：WASM 前端框架（yew/seed）是否算 Rust 行？—— 会拉高 Rust 占比但重写前端+WASM 工具链，本期不做。
3. **视觉风格**：NEKO 品牌蓝 vs deepseekharness 中性色——最终主色调你定（工程骨架先立，色调最后调）。

## 六、对照表（当前 → 目标）

| 项 | 当前（平铺） | 目标（rail+模态） |
|---|---|---|
| 设置结构 | 8 分组平铺在侧滑抽屉 | 模态面板 + 左导航 rail + 一次一 section |
| design token | 硬编码色值 | `:root` 统一 `--color-*/--radius-*/--spacing-*` |
| section 组合 | 全静态平铺 | capability 驱动 + 分域注册 |
| 视觉 | 粗糙 | 胶囊化 + 统一色板/字体/圆角 |
| Rust 占比 | >95% | 不变（路线 A） |

## 七、用户审核确认（2026-09-03）✅

- **路线 A**：非框架 vanilla JS + include_str 嵌入；Rust 占比不变。✅
- **前端豁免**：include_str 嵌入的前端 JS 行**不计入** Rust 占比分母（`cargo run -p xtask -- rust-ratio` 只看 `.rs` 物理行；前端文件豁免）。✅
- **品牌蓝**：主色调用 NEKO `#40C5F1`（`--color-n-main`）。✅

> 已确认方向。工程骨架（design token + rail+模态）由主 Agent 定规格（伪代码/设计），子代理严格照实现，不自由发挥。
