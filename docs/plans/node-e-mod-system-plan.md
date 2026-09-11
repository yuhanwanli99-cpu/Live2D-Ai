# Live2D-Ai 项目结构演进与 Mod 系统计划（节点 E）

> 状态：方向已确认（2026-08-31）
> 目标：**主仓库 = 最小最简核心链路**（TTS + LLM + Live2D + Web UI），
> 3 个 Mod 功能分仓隔离（失败不影响主链路）；Mod 用 **trait 注册中心**接入
> （参考 pi 的 factory(pi) / DSH 的 apply(ctx)），主仓库保持 95-99% Rust。
> 本文档落地计划：前端 UI 设计、Mod 系统接入入口、3 个 Mod 子仓库、
> 工程文件拆分治理、入口介绍改版。

---

## 一、方向共识（已确认）

| 项 | 决策 |
|---|---|
| 主仓库 UI | **只留 Web 前端**（展示渲染 + 对话界面），egui F10 面板 → 桌宠 Mod 的衍生补充 Mod |
| 主仓库 Rust 占比 | **95-99%**（现状 95.79% ✅）；Mod 仓宽松 |
| Mod 接入协议 | **trait + 注册中心**（`Mod::apply(&ModContext)`，对齐 pi/DSH 心智） |
| Mod 隔离 | 独立 crate / 子仓库，编译期组合，不引入 .so ABI 复杂度 |
| 前端渲染 | 主仓库 Web 前端接入 `l2d-wasm-demo` 渲染核心（D3 路线 A） |
| 3 个 Mod 仓 | ① 桌宠透传 ② 外部事件接入 ③ 导演系统 |

---

## 二、前端 UI 设计（主仓库 Web 前端）

### 2.1 桌面端布局（16:9 全窗口）
```
┌─────────────────────────────────────┬──────────────┐
│  Live2D 展示区 (65%)                │  ⚙ 对话区(35%)│
│  （wasm 渲染，透明背景）             │  消息流       │
│                                     │  输入框       │
└─────────────────────────────────────┴──────────────┘
```

### 2.2 设置面板（右上角 ⚙ 浮层，不跳页）
**可滑动/折叠**（为 Mod 接入的更多设置项预留）。分层结构：

```
⚙ 设置浮层（右上角，可滚动）
├─ ① 模型展示  缩放/位置/旋转/fit/背景(透明|色|图|透明度)/水平翻转
├─ ② 连接     LLM（base_url/model/api_key_env/温度/max_tokens/top_p/思考开关）
│             TTS（base_url/model/voice/api_key_env/语速/音调/采样率/声道）
├─ ③ 人设     角色卡(名/头像)/system_prompt/历史轮数/记忆开关
├─ ④ 互动     默认动作强度/自动点头摇/待机动作/口型灵敏度/快捷键
├─ ⑤ 模型管理 导入(ZIP)/删除/切换/激活
├─ ⑥ 高级     日志级别/旁白动作快捷/版本号(前端/后端)/端口/重启/重置
├─ ⑦ 日志     （dev_mode 门控，实时日志面板）
└─ ⑧ Mod 管理 Mod 列表/启用禁用/加载状态（为 Mod 接入预留）
```

> 设置面板**支持滑动**（scrollable），1-6 用户设置 + 7 日志 + 8 Mod 管理。

### 2.3 对话区（右侧 35%）
酒馆风格：消息流（user/assistant bubble）+ 状态条 + 输入框 + 动作快捷按钮（dev_mode 门控）。

---

## 三、Mod 系统接入设计（trait 注册中心）

### 3.1 核心接口（主仓库提供，Mod 实现）
```rust
// 主仓库 crates/live2d-ai-mod-system/src/
pub trait Mod {
    fn id(&self) -> &'static str;            // 唯一 id
    fn version(&self) -> &'static str;
    fn apply(&self, ctx: &mut ModContext);    // 统一入口（对齐 pi.extension / DSH.apply）
}

// 注入的上下文（Mod 通过它注册行为，主仓库控制可暴露的能力）
pub struct ModContext {
    // 主链路动作入口（走 core 仲裁，不绕过）
    pub trigger_action: Box<dyn Fn(ActionId, Strength, ActionSource) -> bool>,
    pub say: Box<dyn Fn(String) -> bool>,
    pub stop: Box<dyn Fn()>,
    // 事件订阅（对齐 pi.on）
    pub on_event: Box<dyn Fn(ModEvent, Box<dyn Fn(&ModEventCtx)>)>,
    // 注册自定义 ActionSource（扩展动作来源）
    pub register_action_source: Box<dyn Fn(ActionSource)>,
    // 设置项注册（Mod 向设置面板注入自己的设置子面板）
    pub register_settings: Box<dyn Fn(ModSettingsSpec)>,
    // 热重载/clone 通知
    pub reload: Box<dyn Fn()>,
    pub log: Box<dyn Fn(Level, String)>,
}
```

### 3.2 生命周期
```
主仓库启动
  └─ ModRegistry::load(manifest)   // 读 Mod 清单
       └─ 逐个 Mod::apply(&mut ctx)  // Mod 注册工具/事件/设置/动作源
            └─ 主仓库降级保护：Mod 失败只 disable 该 mod，不崩主链路
Mod 运行
  ├─ trigger_action → supervisor → core 仲裁（ActionSource 自定义）
  ├─ on_event → 订阅 turn_state/text/action 等事件
  └─ register_settings → 注入设置子面板（滚动显示在 Mod 管理区）
Mod 卸载/重载
  └─ ModRegistry::reload()  // 热重载，保留主链路
```

### 3.3 设计原则（对齐 pi/DSH）
- **统一入口 `apply(ctx)`**（不做框架 bootstrap）
- **事件订阅 + 注册 Tool/动作源/设置**（对 pi.on/registerTool/registerCommand）
- **Mod 失败 = disable 不崩主链路**（对比 DSH「loud fail」——我们因「失败不影响主链路」要求，改为 disable + 日志）
- **组合清单声明**（类 pi extensions / DSH cordis.yml）

---

## 四、3 个 Mod 子仓库

| Mod 仓 | 职责 | 接入点 | 失败影响 |
|---|---|---|---|
| **① pet-desktop（桌宠透传）** | 悬浮窗桌宠：置顶/穿透/托盘/penetration / egui F10 设置面板（衍生补充） | `trigger_action` + `on_event`（渲染/状态） | 无（主仓库仅 Web，桌宠可选） |
| **② external-input（外部事件接入）** | 非 LLM 对话的输入源（微信/消息/游戏事件）注入聊天 | `say(text)` + `register_action_source` | 无（仅补充输入源） |
| **③ director（导演系统）** | 情绪导演/表演编排（Py schoolink 思路） | `trigger_action`（编排动作序列）+ `on_event`（turn 状态） | 无（仅增强动作） |

> 每 Mod 独立 git 仓库（或主仓库分支），依赖主仓库 `live2d-ai-mod-system` trait 接口。

---

## 五、工程文件拆分治理

### 5.1 已识别的 >500 行文件（需拆分）
```
crates/live2d-ai-desktop/src/web_api/models_routes/tests_models_routes.rs  1121
crates/live2d-ai-desktop/src/web_api/tests_ws.rs                            941
crates/live2d-ai-desktop/src/benchmark/runners_surface.rs                   878
crates/live2d-ai-desktop/src/app/settings_ui.rs                            650
crates/live2d-ai-desktop/src/supervisor/turn.rs                            643
crates/l2d/src/asset/mod.rs                                                571
crates/live2d-ai-runtime/tests/unified_client.rs                           554
crates/live2d-ai-desktop/src/app/frame.rs                                  540
crates/live2d-ai-desktop/src/platform/capabilities.rs                     539
crates/live2d-ai-desktop/src/supervisor/tests_stall.rs                     538
crates/live2d-ai-desktop/src/app/tests.rs                                  538
crates/live2d-ai-desktop/src/supervisor.rs                                 532
crates/live2d-ai-desktop/src/benchmark/mod.rs                              517
```

### 5.2 拆分策略（保持 ≤500 行）
- **模块化拆分**：`settings_ui` 拆成 `settings_ui/{mod.rs, form.rs, model.rs, save.rs}`；`turn.rs` 拆成 `turn/{mod.rs, gen_loop.rs, drain.rs, stages.rs}`；`supervisor.rs` 拆 `supervisor/{mod.rs, handle.rs, run_loop.rs}`
- **测试拆到独立文件**：`tests_ws`/`tests_models_routes`/`tests_stall` 拆成 `mod.rs + <场景>_tests`（或 util helper 抽公共）
- **渲染/平台**：`runners_surface` / `capabilities` / `frame` 抽公共 `contract` + `impl`

### 5.3 治理规则（防混乱）
- 源码 ≤500 行（豁免 ≤1000 需头注理由，已有此约定）
- 每个 crate 一个 `README`（职责/依赖/扩展点）
- `docs/architecture/directory.md` 更新为 Rust 结构
- 主仓库只存核心链路；Mod 逻辑一律分仓，不混入 `crates/`

---

## 六、项目入口介绍改版

### 6.1 README.md（重写为 Rust 版）
- **底层为 Rust 重构现状**（现状 README 仍写 Android/Kotlin/Purism/DeepSeek——已过时）
- 标题改 `Live2D-Ai v0.3.0 (Rust)`；徽章改 Rust/wgpu/AGPL
- 架构图改 core→runtime→l2d→desktop→web
- 新增 **Mod 系统说明** + 3 子仓库链接
- 快速起步改 `cargo run -- --web`

### 6.2 入口文档
- `AGENTS.md` / `docs/README.md` 同步 Rust 结构
- `docs/architecture/directory.md` 重写为 Rust workspace 目录
- `docs/plans/branch-review-boundary` 已存在，需同步最新结构

---

## 七、实施批次（分阶段，每批可验证）

| 批次 | 内容 | 验证 |
|---|---|---|
| **E1** | 拆大文件（settings_ui/turn/supervisor/models/tests）→ ≤500 行 | cargo test 全绿 + 占比不变 |
| **E2** | 前端 UI 实现（16:9 布局 + 左 65/右 35 + 右上角 ⚙ 设置浮层分层 + 滑动） | 浏览器实测设置面板 |
| **E3** | Web 前端接入 wasm 渲染（l2d-wasm-demo → live2d-display div） | 浏览器看到 Live2D 模型 |
| **E4** | 建 `live2d-ai-mod-system` crate（trait Mod + ModContext + ModRegistry + 热重载 + 日志） | cargo test |
| **E5** | 设置面板扩展字段（温度/max_tokens/模型展示全项/语速音调/记忆开关）+ 日志面板 + Mod 管理占位 | 设置项齐全 |
| **E6** | 3 个 Mod 子仓库骨架（pet-desktop/external-input/director）+ README | 各独立编译 + 可 disable |
| **E7** | 入口改版（README/AGENTS/directory） | 文档核对 |

---

## 八、主仓库 / Mod 边界（防污染）

```
主仓库（Live2D-Ai，95-99% Rust）
  crates/
    l2d               Live2D 渲染封装
    live2d-ai-core    纯 reducer 状态机
    live2d-ai-runtime LLM/TTS/settings/conversation
    live2d-ai-desktop supervisor + Web API + 窗口
    live2d-ai-mod-system  Mod trait/context/registry（新）
    l2d-wasm-demo     Web 渲染核心（被 Web 前端复用）
    xtask             工程工具

Mod 子仓库（独立 git，宽松语言）
  pet-desktop/       桌宠透传（+ egui 衍生设置 Mod）
  external-input/    外部事件接入
  director/          导演系统
```
> 主仓库 `crates/` 只放核心链路；任何 Mod 逻辑不混入主仓库。

---

## 九、验收口（节点 E 完成标准）
1. 主仓库 Rust 占比 ≥95%（保持或提升）
2. 所有源码文件 ≤500 行（豁免有头注）
3. Web 前端：16:9 + 左 65/右 35 + 右上角 ⚙ 设置浮层（可滑动，8 个分组）
4. Web 前端显示 Live2D 模型（wasm 渲染接入）
5. `live2d-ai-mod-system` trait 接入点可用（至少 1 个 demo Mod 可加载/disable）
6. 热重载 + 日志系统在设置面板可操作
7. README/入口文档全面 Rust 化
8. 3 个 Mod 子仓库骨架存在且独立编译

---

## 附：未来 Mod 扩展位（预留）
- `ActionSource` 已支持 UserCommand(100)/LlmTool(90)/RuleFallback(50) → Mod 可注册新枚举
- `SupervisorHandle` 的 say/stop/reload/trigger_action → Mod 控制主链路动作
- `on_event` → Mod 订阅 turn_state/text_delta/action 等事件
- `register_settings` → Mod 向设置面板注入自身子面板（滚动）——这正是「面板可滑动为 mod 预留」的落点
