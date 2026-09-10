# Live2D-Ai 核心契约与架构边界（Rust 重构版）

> 状态：2026-09-10 增补「双主导分层」与前端层豁免（原版 2026-08-31 只描述 Rust 单主线；
> 更早版本描述 Android/Python 双端，已归档，见 `ANDROID_ARCHIVE_POINTER.md` 与 tag `py-legacy`）。
> 目标：把核心闭环的稳定接口固定下来，让后续扩展、可观测性、Web 集成都有可依赖的边界。

## 1. 最小核心闭环

```
文本
  → LLM（OpenAI 兼容 /chat/completions，流式 SSE）
  → 分句（**只按真实句读边界**，不做句中硬切）
  → TTS（OpenAI 兼容 /audio/speech，**一句一单元完整合成**）
  → Live2D（口型 / 表情 / 动作 / 待机；wgpu 渲染）
```

核心由**单一 Rust workspace（`crates/*`）** 实现；用户界面由 **Flutter Web（`shell/flutter/`）**
承担，经版本化消息桥驱动 Rust 渲染面（`/render`）。契约保证：

- 纯 reducer 状态机（`live2d-ai-core`）无 IO、无 async，可穷举测试；
- LLM/TTS provider 走统一 OpenAI 兼容协议，不绑定厂商 SDK；
- 动作/口型/渲染经 supervisor 事件循环串行化，主线程只做窗口与渲染副作用；
- **前端不复制核心逻辑**：状态机、动作仲裁、LLM/TTS 协议只在 Rust 侧，
  Flutter 只做展示与调度（详见 `AGENTS.md` §分层与技术栈）。

### 1.1 语音输出契约（2026-09-10 用户裁决）

**一句一单元：一个句子必须完整合成后连续播放。延迟可接受，断句不可接受。**

- 分句只按真实句读边界（`。！？…` 等）；**禁止**按字符位置硬切
  （历史上 `SentenceAssembler` 的 48 字位置切分会把一句话从中间劈开，
  TTS 听感即为「断句」）；
- 播放侧为整句完整性可引入预缓冲：宁可晚开口，不可中途卡顿；
- 违反该契约的改动视同回归。

## 2. 分层与模块边界（crates/*）

| crate | 职责 | 关键不变量 |
| --- | --- | --- |
| `live2d-ai-core` | 纯 reducer 状态机（epoch/turn/sentence/audio-chunk + 动作仲裁） | 零第三方依赖；epoch 仅根 State 持有；stop 推进 epoch |
| `live2d-ai-runtime` | LLM/TTS 网络层、settings（三态 patch/原子写回）、conversation | SSE decoder 不依赖 chunk 边界；`ApiSecret` 不实现 serde |
| `l2d` | Live2D 模型资产（model3/moc3/physics）与 renderer 封装 | Ayagami pin 固定 rev；路径拒绝绝对/反斜杠/`..` |
| `live2d-ai-desktop` | 窗口（winit+wgpu）、音频（cpal）、supervisor、Web API、egui 设置面板 | supervisor 独占 reducer/engine/audio；PCM 不经 AppEvent |
| `l2d-wasm-demo` | 浏览器渲染验证（wasm32） | 与桌面共享 `l2d` renderer core |
| `xtask` | Rust 占比统计等工程工具 | — |

## 3. 核心状态机契约（live2d-ai-core）

```
IDLE
  ├─ UserSubmitted        → GENERATING（turn 内）: 开闩锁
  ├─ Action               → 动作仲裁（capability gate / 优先级 / 幂等）
  └─ StopRequested        → 推进 epoch，拦截所有迟到事件

GENERATING（turn）
  ├─ GenerationFinished   → 恰一次；双闩锁（generation + playback）收口
  ├─ ActionPlaybackFinished→ 携带动作身份，避免旧动作完成误伤新动作
  └─ StopRequested        → do_stop：root epoch → cancel → clear pending → VoiceEnded
```

核心不变量：

- I1：`StopRequested` 是唯一推进 epoch 的事件；epoch 单调且只增。
- I2：迟到事件（旧 epoch 的生成/完成）统一拦截，不污染新代次。
- I3：动作子状态不复制 epoch；自然完成携带动作身份。
- I4：`PerformancePlayer` 无内部时钟，调用者注入 dt（可测试）。

## 4. Supervisor 契约（live2d-ai-desktop）

- supervisor = 独立 OS 线程 + `current_thread` tokio runtime。
- `run_one_turn` 五阶段：生成循环 → pending pump → GenerationFinished 恰一次 → 收尾。
- 控制命令：`Stop` / `Quit` / `Reload`。turn 收尾**不** drain 通道（Quit/Stop 由
  空闲态 select 自然消费；Reload 走 `reload_pending` 原子标志）。
- 正常 turn 完成**不**发 `NewEpoch`（epoch 未变）；`NewEpoch` 只由 stop 路径发。

## 5. Web API 契约（web_api）

- loopback-only（`127.0.0.1`）；默认拒绝跨域（不回 `Access-Control-Allow-Origin`）。
- mutating 端点（settings PATCH / chat / commands invoke / models 写操作）统一走
  dispatch 的 Origin + Content-Type 校验（`is_mutating_route(route, method)`）。
- 命令 invoke 必须经 `SupervisorHandle::trigger_action` → core 仲裁，禁止绕过。
- WS：单向事件流（`turn_state` / `text_delta` / `runtime_status` / `subscribe_ack`），
  客户端命令走 HTTP；服务端每 10s heartbeat，写失败即退并 unsubscribe。

## 6. 共享资产（shared/）现状

| 文件 | 状态 |
| --- | --- |
| `persona.yaml` | 历史模板资产；Rust 侧用 `[persona] system_prompt`（见裁决 §8） |
| `model_registry.json` / `model_registry.schema.json` | 历史清单；Rust 用 XDG data_dir 独立 registry |
| `emotion-protocol.md` | 历史协议文档（Rust 动作集已收敛为 6 基础动作） |
| `model-adapter/` | 历史适配器；Rust 用 `BaiParamAdapter` |
| `mcp_tools.json` / `model_dict.json` | 历史 LLM/工具定义；Rust 走 OpenAI 兼容 + tool 描述 |

## 7. 验收口径

任何改动必须保证：

1. `cargo test --workspace --all-targets` 全绿（当前 674 + 3 doctest）。
2. `cargo fmt --all --check` 干净。
3. `cargo clippy --workspace --all-targets -- -D warnings` 干净。
4. root `pytest tests/` 全绿（当前 22 passed, 1 skipped——Android 归档后合理跳过）。
5. 无 API Key、无网络时：配置解析 / 状态机 / TTS 降级行为确定（不崩溃、常驻引导）。
6. 安全红线：密钥不进 GET/日志/WS/导出；loopback-only；mutating 需
   `application/json`；命令 invoke 必经 core 仲裁。

## 8. 历史决策对 Rust 的影响（摘要）

- 已迁移：6 基础动作、release 控制语义、动作强度 1–3、persona system prompt 概念、
  OpenAI 兼容 LLM/TTS、model3 资产结构、settings 分区思想。
- 明确废弃：SLOP 规则（Py TTS 链路）、soullink 表演引擎、8 情绪协议完整消费路径、
  背景导入 / 模型 ZIP 上传（D3.2 后置）、动作时间线（v1 无编排）。
- 待裁决：`shared/persona.yaml` 的权威地位（见 `docs/plans/branch-review-boundary-2026-08-31.md`）。
