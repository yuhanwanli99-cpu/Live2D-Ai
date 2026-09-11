# 节点 E — Mod 系统边界裁决 ADR（E0）

> 状态：**裁决生效（2026-08-31）**
> 关联：`docs/plans/node-e-mod-system-plan.md`（已退回修改，本文档为其修正版「边界裁决」）
> 作用：在实现前冻结 Mod v1 的架构边界，消除计划中的三处技术错误，
> 防止 E4/E5/E6 产生错误产品预期。

---

## 0. 核心裁决：Mod 是静态编译模块

**不采用 `.so` ABI 动态加载**（明确排除）。

| 能力 | v1 是否支持 | 说明 |
|---|---|---|
| compiled-in Mod | ✅ | 编译进当前 binary（`available_mod_factories` 清单） |
| enable / disable | ✅ | 运行时开关（manifest `enabled = true/false`），启动时读取 |
| restart Mod 实例 | ✅ | 销毁 + 重新创建 `ModRuntime` |
| reload config | ✅ | 重新读取 Mod 配置（`~/.config/live2d-ai/mods/<id>.json`） |
| install Mod（运行期加新 crate） | ❌ | 需重新编译发行包 |
| hot reload code | ❌ | 不支持运行期热加载 Rust 源码 |

> **新增 Mod 必须重新构建应用**。manifest 只决定「已编译 Mod 的启用/禁用/配置」，
> 不决定「从磁盘加载哪个 crate」。

---

## 1. Mod 接口（修正后的 v1）

### 1.1 工厂与运行时
```rust
pub type ModId = &'static str;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDescriptor {
    pub id: ModId,
    pub name: &'static str,
    pub version: &'static str,
    pub api_version: u32,   // live2d-ai-mod-system API 版本
}

pub trait ModFactory: Send + Sync {
    fn descriptor(&self) -> &'static ModDescriptor;

    /// 由已编译 host 调用，构造一个 Mod 实例。
    /// `config` = 已解析的该 Mod JSON 配置（namespaced）。
    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError>;
}

pub trait ModRuntime: Send {
    fn start(
        &mut self,
        registrar: &mut ModRegistrar<'_>,
    ) -> Result<(), ModError>;

    fn shutdown(&mut self) -> Result<(), ModError> {
        Ok(())
    }
}
```

**关键**：
- `apply()` 改为 **`start(&mut self, registrar)` 返回 `Result<_, ModError>`** ——可报告失败。
- 用 `ModFactory` + `ModRuntime`（工厂/实例分离），对齐生命周期。
- `Send` 必须（事件线程隔离要求）。

### 1.2 不用 `apply(ctx)` 的原因
`apply` 的语境是「执行业务」，但 v1 Mod 真实职责是**注册能力**（settings/事件订阅），
故用 `start(&mut self, registrar)`。若未来需对齐 pi/DSH 心智，可在 `start` 内调用
`self.apply(&mut ctx)` 作为内部实现，但对外接口保持 `start -> Result`。

---

## 2. Host Services（注入，非任意 Box<dyn Fn> 字段堆叠）

```rust
#[derive(Clone)]
pub struct ModServices {
    action_tx: ModActionSender,   // 主动作请求通道（host 映射为固定优先级）
    say_tx: SaySender,            // 外部文本 → 进 LLM/TTS 主链路
    event_tx: ModEventSender,     // host → mod 的事件投递（有界）
    logger: ModLogger,            // 带 mod_id 前缀的日志
}
```

**原则**：不直接公开多个任意 `Box<dyn Fn>` 字段，而是打包成有语义的 services。

### 2.1 动作来源（修正计划技术错误）
**移除** `register_action_source(ActionSource)`——Rust 封闭 enum 无法由外部 crate
运行时扩展，且允许 Mod 自定优先级会破坏 core 仲裁边界。

改为 **host 统一映射固定优先级**：
```
UserCommand   100
LlmTool        90
Mod            70
RuleFallback   50
```

Mod 主动动作经 `ModActionSender` 提交：
```rust
pub struct ModActionRequest {
    pub mod_id: ModId,
    pub action: ActionId,
    pub strength: Strength,
}
```
host 端把 `Mod` 类动作映射为固定优先级（不暴露自定优先级）。第一版也允许将 Mod
动作全部映射为现有 `UserCommand`，同时在审计元数据记 `mod_id`。

### 2.2 事件隔离（按审计要求）
Mod 事件**不跑在 supervisor 主循环**。host → Mod 经**有界 channel + 独立 worker 线程**：
```
supervisor/AppEvent
  → ModEventBridge.try_send()   // 有界 channel（容量固定，满则丢 + dropped 计数）
  → Mod worker（每启用 Mod 一个） // 独立线程，事件处理不卡 supervisor
```
**失败隔离**：Mod worker 处理失败 → `last_error` + 停止投递 + `state=Failed`，核心继续。

---

## 3. Mod 状态机

```rust
pub enum ModStatus {
    Disabled,
    Starting,
    Running,
    Failed { message: String },
    Stopping,
}
```

### 失败分类
| 场景 | 处理 | 隔离 |
|---|---|---|
| 初始化失败（`factory.create/start` 返回 Err） | `state=Failed`，清理已注册 subscription/route | 核心继续启动 |
| 事件处理失败（worker 返回 Err） | 记 `last_error`，停止投递，`state=Failed` | 核心继续 |
| panic | `catch_unwind` 转 `Failed`（若 `panic=unwind`） | 部分隔离 |

**诚实声明**：
- `panic=abort` / 死循环 / unsafe 内存破坏 **无法隔离**（编译进同一进程的 Rust Mod 不是安全沙箱）。
- 个人项目可接受，但不描述成进程级隔离。

---

## 4. Settings 数据协议（前端消费）

**Rust Mod 不能向浏览器注入 HTML/JS/DOM**。主仓库 Web 前端只消费数据 schema。

```rust
pub struct ModSettingsSpec {
    pub mod_id: String,
    pub title: String,
    pub version: u32,
    pub fields: Vec<ModSettingField>,
}

pub enum ModSettingField {
    Bool   { key: String, label: String, default: bool },
    String { key: String, label: String, secret: bool },
    Number { key: String, label: String, min: f64, max: f64 },
    Select { key: String, label: String, options: Vec<SelectOption> },
}
```

前端用统一 renderer 生成控件。**禁止 Mod**：注入任意 HTML / JS / 直接操作 DOM / 自带前端 bundle。

**持久化**（namespaced）：
```
~/.config/live2d-ai/mods/<mod-id>.json
```
或主配置文件：
```toml
[mods.external_input]
enabled = true
port = 39222
```

---

## 5. ModRegistrar / Registry

```rust
pub struct SubscriptionId(u64);

pub trait ModRegistrar {
    fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError>;
    fn subscribe(&mut self, topic: ModEventTopic) -> Result<SubscriptionId, ModError>;
    fn unsubscribe(&mut self, id: SubscriptionId) -> Result<(), ModError>;
}

pub struct ModState { ... } // 见 §3

pub enum ModEventTopic {
    TurnStarted,
    TextDelta,
    ActionFinished,
    VoiceStarted,
    VoiceEnded,
    // ...
}
```

### Registry 生命周期（host 持有，Mod 无权重载 host）
```rust
registry.disable(id)
registry.enable(id)
registry.restart(id)      // 销毁 + 重建实例
registry.reload_config(id)
```
**移除** ModContext 上宽泛的 `reload`。Mod 只能返回 `ModAction::RequestRestart`，由 host 决定。

---

## 6. 主仓库 Mod 边界（编译组合）

```
主仓库 crates/
  live2d-ai-mod-system/     Mod trait/factory/registry/services/status/topics（新）
  与 3 个 Mod crate（external-input/pet-desktop/director）是 workspace 成员，
  但 by default 不启动（manifest enabled=false）——验证"失败不影响主链路"。

主程序持静态 factory 清单：
static AVAILABLE_MOD_FACTORIES: &[&dyn ModFactory] = &[
    &ExternalInputFactory,
    &PetDesktopFactory,
    &DirectorFactory,
];
```

> 最终发行 binary 需要显式依赖并链接对应 Mod crate（或由独立发行 workspace 组合）。
> 3 个 Mod 是本仓库 workspace 成员（无远程子仓库），默认 disable。

---

## 7. 前端 capability-driven UI

主仓库 Web 前端**只渲染后端已声明的能力**（避免显示不可保存的控件）：
```json
{
  "settings_sections": {
    "llm": true, "tts": true, "stage": true,
    "memory": false, "mod_management": false
  }
}
```
`/api/v1/app/capabilities` 决定显示/隐藏/禁用控件。日志仅 `dev_mode=true` 时挂载。
未实现字段（temperature/max_tokens/top_p/语速/音调/记忆/快捷键等）标状态：
`Implemented` / `Read-only` / `Planned` / `Mod-provided`，不渲染可保存但后端不识别的表单。

---

## 8. 三个 Mod 的实现顺序与定位

| 顺序 | Mod | 定位 | 说明 |
|---|---|---|---|
| 1（E5） | **external-input** | 第一个真实 Mod | 外部请求 → `say()` → LLM/TTS 主链路；最易验证 |
| 2（E7） | **pet-desktop** | adapter 演示 Mod | **不迁移现有 native desktop**（winit/wgpu/audio 迁移风险高）；只演示用 host services 驱动桌宠行为；等接口稳定后再议真迁 |
| 3（E7） | **director** | 最后 | 需先有 `schedule_performance(PerformanceSequence)` host 能力（否则重复"第一步被第二步抢占"）；导演自己维护 worker/timer 或等动作完成事件发下一步 |

> **注意**：E7 前必须先实现 host 的**动作序列编排能力**（action-completion-driven），
> 否则 director 会重新制造 nodes D 刚删掉的 "composed 命令抢占" 问题。

---

## 9. 修正后的实施顺序（替代原 E1-E7）

```
E0  边界裁决（本文档，先冻结）
E1  入口文档 Rust 化 + 定向拆热点文件
E2  WASM 渲染纵切验证（最小闭环）
E3  Web 产品 UI（围绕可用 renderer 实现）
E4  Mod SDK/Host v1（静态 registry + 事件 worker + 内置 demo Mod）
E5  External Input demo Mod（第一个真实 Mod）
E6  Mod 管理 UI（Registry API 稳定后）
E7  Pet/Director Mod（先补 performance sequence 能力；pet 做 adapter 演示）
```

---

## 10. 已纠正的计划原文错误（记录）

| 原文位置 | 错误 | 修正 |
|---|---|---|
| §3.1 `apply()` 返回 `()` | 无法报告失败 | `start() -> Result<_, ModError>` |
| §3.2 `ModRegistry::load(manifest)` | 语义与编译期组合矛盾 | `initialize(AVAILABLE_MOD_FACTORIES, manifest)` |
| §4 `register_action_source` | 封闭 enum 不可外部扩展 | 改为 host 固定优先级映射 |
| §4 `on_event` 同步跑主循环 | 慢 Mod 卡主链路 | 有界 channel + 独立 worker |
| §4 `register_settings` 注入 UI | Rust 无法注入浏览器 DOM | 改为纯数据 schema |
| §5 `reload` 暴露给 Mod | 语义过宽 | 移除，Registry 提供 enable/disable/restart/reload_config |
| §9「Mod 可注册新枚举」 | 技术错误 | 删除（Rust enum 不可运行期扩展） |
