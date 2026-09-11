# 接手修复与渲染档位计划（2026-09-07）

> 分支：`dev/integrity`。
> 状态：**S1+S2 已实现并通过全部门禁**（2026-09-07 接手会话复核）；S3/S4 为外部 native 渲染环境验证任务，未执行。

> **执行状态（2026-09-07 接手会话复核）**：
> - **S1 缺陷1 完成**：director whitelist → `shake_no` + 默认序列 + 2 个回归测试全绿。
> - **S2 缺陷2 完成**：`SupervisorConfig.mod_events` 事件桥（包裹 emit + 2 处显式调用）+ cli_entry/supervisor_slot 双路径接线 + 投影单元测试 + 端到端 TurnStarted 测试；`dispatch_event` 去 dead_code，生产侧已打通。
> - **门禁基线实测全绿**：workspace/all-targets + doc + fmt + clippy `-D warnings` + rust-ratio **95.28%**（≥ 95%）。
> - **S3/S4（5060 16384 native 靶标验证 / 渲染档位）未执行**：需外部 native 渲染环境，见 §4.4 / §5。

## 0. 动机

用户问「16384 渲染能否做到」，本会话经源码精读确认了两点：

1. **渲染档位的真实语义**：`4096/8192/16384` 作为「纹理档位」在仓库中**尚不存在**——源码无任何档位常量，渲染尺寸完全运行时派生（canvas 像素 × `DPR_CAP=1.5`，`surface.rs:164-173`）。`bai.16384/` 只是**资产文件夹名**，实际纹理是 `texture_00_4096.png`（4096×4096）。
2. **渲染后端**：native 走 wgpu 29（Vulkan→GL→all 回退，`gpu.rs:76-80`）；wasm 走 `BROWSER_WEBGPU | GL`（`surface.rs:60-62`）。

同时发现有**真实缺陷**需优先修复（见 §2）。

### 5060 实际上限（实测于本机）

- GPU：**NVIDIA GeForce RTX 5060 Laptop，8151 MiB（≈8GB）显存**，驱动 592.01。
- 环境：**WSL2 (Ubuntu 26.04)**，`DISPLAY=:0`，无 `vulkaninfo`。
- **推论（待 §4 验证任务确认）**：
  - 单张 16384×16384 RGBA8 离屏靶标 ≈ **1GB**，8GB 显存可负担 1-2 张；多张/mipmap 吃紧。
  - 当前仓库全部用 `wgpu::Limits::default()`（`gpu.rs:107`、`bootstrap.rs:91`、wasm `..Default::default()`），其 `max_texture_dimension_2d = 8192`。**要跑 16384 需自定义 `required_limits.max_texture_dimension_2d = 16384`。**
  - wasm 端上限取决于浏览器 surface 能力，通常远低于 16384；`/render` 走浏览器，故 16384 档主要面向 **native 路径**。

---

## 1. 目标

1. **修复缺陷**（先行）：director 动作白名单漂移 + supervisor→Mod 事件桥。
2. **渲染档位可行性**：将「纹理档位」落到离屏渲染靶标尺寸档（超采样/降采样），在 5060 8GB 上给出安全档位与自定义 limits 的接法。
3. **5060 实际上限验证**：native 路径（非浏览器）实测 16384 靶标能否建立。

每步都以**可见效果 + 门禁全绿**验收，避免「测试绿但假接线」。

---

## 2. 缺陷清单与修复

### 缺陷1（P0，d1）：director 白名单与 core 动作名漂移

**位置**：`crates/live2d-ai-mod-director/src/lib.rs:17`
```rust
const ACTIONS_WHITELIST: &[&str] = &["nod", "shake", "tilt", "look_around", "listen", "surprise"];
```
**矛盾**：core `ActionId::name()`（`crates/live2d-ai-core/src/action/mod.rs:55`）对 `ShakeNo` 返回 **`"shake_no"`**。host 侧 `parse_action_id`（`crates/live2d-ai-desktop/src/mod_registry.rs:73`）严格匹配 `name()`，故：
- director 默认序列 `["nod","shake"]` 里的 `"shake"` → host `parse_action_id("shake")` 返回 `None` → 静默拒绝，摇头**永远不触发**；
- 用户想配 `["shake_no"]` 又被 whitelist 挡掉。

**修复方案**：whitelist 改为 **`["nod","shake_no","tilt","look_around","listen","surprise"]`**，默认序列改为 `["nod","shake_no"]`，并新增回归测试。**不改 `parse_action_id` 的严格匹配**（那是协议正确行为）。

**验收**：`cargo test -p live2d-ai-mod-director` 新增用例通过；无其它 crate 受影响。

### 缺陷2（P1，d2）：supervisor→Mod 事件桥无生产者

**位置**：`crates/live2d-ai-desktop/src/mod_registry.rs:336` 的 `dispatch_event` 是事件投递入口，但整个 `crates/` 搜索 `dispatch_event(` **仅测试调用，无生产调用**。

**矛盾**：
- supervisor 线程 emit 的是 `AppEvent`（`crates/live2d-ai-desktop/src/app_event.rs`），与 `ModEventTopic`（`crates/live2d-ai-mod-system/src/topics.rs`）是两个独立枚举，**无映射桥接**。
- `cli_entry` 装配时把 supervisor 注入 `HostChannels`（Mod→host，`cli_entry.rs:131-215`），但 **没把 registry 交给 supervisor**（host→Mod）。
- 结果：director 订阅了 `TurnStarted`/`ActionFinished`，但 supervisor 从不发这些事件 → `auto_sequence_on_turn` 形同虚设。

**修复方案**（跨 crate 接口，见 §3 设计冻结）：
1. 在 `SupervisorConfig` 加可选回调字段 `mod_events: Option<Arc<dyn Fn(ModEventTopic, &str) + Send + Sync>>`。
2. supervisor 在 turn 生命周期关键点调用它：
   - `TurnStarted`：`run_forever` 收到 `say_rx` 文本、进入 `run_one_turn` 前；
   - `ActionFinished`：`finish_rx` 收到 `ActionFinishedFact` 时（携带动作名作 payload）；
   - `VoiceStarted`/`VoiceEnded`：`handlers` 中 emit 对应 `AppEvent::Conversation` 处；
   - `TextDelta`：`handle_engine_event` 收到 `EngineEvent::TextDelta` 处。
3. `cli_entry` 生产装配：构造该闭包，内部调用 `ctx.mod_registry.lock().dispatch_event(topic, payload)`。
4. 补一张 **`AppEvent` ↔ `ModEventTopic` 投影映射表**（放 doc 头注，供后端/前端共同依赖）。

**验收**：`cargo test -p live2d-ai-desktop` 新增 supervisor→registry 事件路由测试（用可注入的 registry）；`cargo test --workspace --all-targets` 全绿。

### 观察项（非缺陷，仅记录）
- **d3（观察）**：texture 档位概念不存在，属「投机性重构」而非缺陷——见 §4。
- **d4（观察）**：`conversation history`（`VecDeque<(String,String)>`）纯内存、reload 即丢（源码注释确认为公开语义）——chat history API 价值偏低，本期不做。

---

## 3. 缺陷2 设计冻结（接口）

> 本段在实现前由主 Agent 冻结，subagent 照做，不改此接口。

### 3.1 `SupervisorConfig` 新增字段

```rust
pub struct SupervisorConfig {
    // ...既有字段...
    /// host→Mod 事件桥（None = 不投递；测试/无 Mod 场景）。
    /// 由 supervisor 在 turn 生命周期点调用（非阻塞；should not block）。
    pub mod_events: Option<Arc<dyn Fn(ModEventTopic, &str) + Send + Sync>>,
}
```

> 用回调而非把整个 `ModRegistry` 交给 supervisor：**解耦**（supervisor 不依赖 registry 内部实现）、**可测**（测试注入收集闭包）、**与 `emit: Fn(AppEvent)` 风格一致**（supervisor 只发纯回调）。

> **2026-09-07 冻结追加**：实现方式 = **包裹 `emit` 闭包 + 2 处显式调用**。
> - `AppEvent::Conversation` 三变体（`TextDelta`/`VoiceStarted`/`VoiceEnded`）在
>   `handlers::handle_engine_event` 与 `emit_voice_ended_if_needed` 内已经 emit，
>   故在 `spawn_supervisor_impl` 里把传入的 `emit` **再包一层**：转发给 AppEvent 发射
>   的**同时**用 `mod_events` 桥到对应 `ModEventTopic`（投影见 §3.2）。这不需要改
>   `run_one_turn`/`handlers` 的签名——只需在构造闭包时捕获 `mod_events` 的 clone。
> - `TurnStarted`/`ActionFinished` **不是 AppEvent**，需**2 处显式调用**：
>   - `TurnStarted`：`run_forever` 收到 `say_rx` 文本、`run_one_turn` 调用前；
>   - `ActionFinished`：`run_forever` 收到 `finish_rx` `ActionFinishedFact` 时。
> - `mod_events` 注入到 `run_forever`（新增参数）供上述 2 处显式调用；包裹的 emit
>   闭包在 `spawn_supervisor_impl` 内捕获 `mod_events` clone（`Option<Arc<…>>`）。

### 3.2 事件投影映射表（`AppEvent` ↔ `ModEventTopic`）

| 生命周期点 | AppEvent 侧 | ModEventTopic | payload |
|---|---|---|---|
| 新 turn 提交 | `run_forever` 收到 `say_rx` 文本 | `TurnStarted` | `"turn_id"`（当前 turn id） |
| LLM 文本增量 | `AppEvent::Conversation(TextDelta)`(emit 包裹) | `TextDelta` | `text`（增量） |
| 动作完成 | `run_forever` 收到 `ActionFinishedFact` | `ActionFinished` | `action.name()` |
| 语音开始 | `AppEvent::Conversation(VoiceStarted)`(emit 包裹) | `VoiceStarted` | `epoch` 字符串 |
| 语音结束 | `AppEvent::Conversation(VoiceEnded)`(emit 包裹) | `VoiceEnded` | `epoch` 字符串 |
| 模型激活 | `ModelActivated`（由 models_routes 侧发） | `ModelActivated` | `model_id` |

### 3.3 投递纪律

- **非阻塞**：`dispatch_event` 内部 `try_send`（满则丢弃计数），回调不 await。
- **只发 Running Mod 感兴趣的主题**：由 `ModRegistry` 的 worker 侧按订阅路由（`event_worker_loop`）。
- **失败隔离**：`on_event` Err → 清槽位（既有 E0 语义，`mod_registry.rs:378-381`）。

### 3.4 安全
- 事件 payload 是 turn/action/voice 等**非敏感**字符串；**不经网络**（host 内部有界 channel）。
- 不携带 API Key / 密钥 / 完整聊天正文——`TextDelta` payload 仅增量文本，Mod 系统无回写密钥通道。

---

## 4. 渲染档位可行性

### 4.1 现状事实
- 仓库**全部** `wgpu::Limits::default()`；无自定义 `max_texture_dimension_2d`。
- 渲染尺寸 = canvas CSS × `min(DPR, DPR_CAP=1.5)`（`surface.rs:164-173`）。
- 模型资产实际纹理 `texture_00_4096.png` 为 **4096×4096**（`rust-bakeoff-ayagami.md:58`）。
- native 离屏靶标大小为运行时传入（`offscreen.rs:22-25` `width/height` 字段）。

### 4.2 结论：能做，但「16384 档」主要面向 native 靶标而非纹理

| 档位 | 离屏靶标（RGBA8） | 8GB 显存结论 | 建议 |
|---|---|---|---|
| 4096 | 64 MB | 很轻松 | 默认档 |
| 8192 | 256 MB | 轻松 | 高/性能档 |
| 16384 | 1 GB | 单张可，多张/mipmap 吃紧 | 需自定义 limits + 显存预留；仅 native 路径 |

### 4.3 实施要点（如纳入范围）
1. **自定义 limits**：`required_limits.max_texture_dimension_2d = 16384`（native `gpu.rs` / `bootstrap.rs`）；wasm 保持默认（浏览器上限）。
2. **暴露档位**：前端「外观与舞台」区新增「渲染分辨率档」下拉（4096/8192/16384），经 `postStageConfig` → wasm `stage-config` → surface 按档位设离屏靶标尺寸。
3. **DPR cap 联动**：档位作为超采样上限，替代固定 `DPR_CAP=1.5`。
4. **失效兜底**：实际创建失败（内存不足/wgpu 上限）→ 回退上一档 + toast，不 crash。

### 4.4 5060 实际上限验证（本会话任务）
- **native 路径**（`cargo run --release -- --model-smoke` 或 benchmark）实测 16384 靶标能否建立。
- **WSL2 注意**：native 走 Vulkan，WSL2 有对应 GPU 支持；但 bakeoff 文档显示容器/无显卡环境常落到 llvmpipe 软渲染（性能不代表真机）。本机非容器，应能拿到硬件 adapter。

---

## 5. 分阶段执行排期

| 阶段 | 内容 | 类型 | 验收 |
|---|---|---|---|
| **S1** ✅ | 缺陷1：director whitelist + 默认序列 + 回归测试 | T1 | `cargo test -p live2d-ai-mod-director`（6 passed，含 2 新增）；workspace 全绿 |
| **S2** ✅ | 缺陷2：接口冻结 + supervisor 事件桥 + cli_entry 接线 + 测试 | T2 跨 crate | `cargo test -p live2d-ai-desktop`（124 passed，含 projections + TurnStarted 桥）；workspace 全绿 |
| **S3** | 5060 实际上限验证（native 16384 靶标） | 验证任务 | benchmark/冒烟日志展示实际 max texture / 能否建 |
| **S4** | 渲染档位（如 S3 可行且用户确认） | T2 渲染 | 可见档位切换 + 门禁全绿 |

> S3/S4 为外部 native 渲染环境验证任务；S1/S2 纯工程已完成并全绿。

## 6. 门禁与验收（每阶段都必须）

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio   # 门槛 95%
```

> S1/S2 为纯工程，无需外部环境；S3/S4 需要 native 渲染环境。

## 7. 明确的非目标（本期不做）
- chat history API（价值低，语义未定）。
- 背景图走后端（C2 纯本地方案已知；如需另立阶段）。
- director emotion_mapping schema / prodvision（N.E.K.O 借鉴项，下下轮）。
- 双声问题消解（F6-T3 前端播放器就绪后翻转默认）。

## 8. 变更历史
- 2026-09-07：初稿（本接手会话制定，分支 `dev/integrity`）。
- 2026-09-07：接手复核——S1/S2 已实现并通过全部门禁（workspace/all-targets + doc + fmt + clippy + rust-ratio 95.28%）；S3/S4 仍为未执行的真机 native 渲染验证任务。
