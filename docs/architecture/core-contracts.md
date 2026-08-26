# Live2D-Ai 核心契约与架构边界

> 状态：地基文档 v0.1
> 目标：把“最小核心闭环”的稳定接口固定下来，让后续重构、插件化、双端一致性都有可依赖的边界。

## 1. 项目最小核心闭环

```
文本/语音
  → LLM（流式回复 + 情绪）
  → TTS（语音合成 + 嘴型数据）
  → Live2D（表情 / 口型 / 动作 / 待机）
```

当前 Android 与 PC 都围绕这条闭环实现，但实现细节不同。核心契约要保证：

- 两端对同一份 `shared/` 配置有相同理解；
- 核心环路的“状态 / 事件 / 副作用指令”可被纯逻辑测试；
- TTS / LLM / 表情 / 动作都通过接口接缝解耦，不直接依赖具体厂商。

## 2. 核心模块边界

### 2.1 Android 核心层（`app/src/main/java/com/live2d/ai/android/core/`）

| 模块 | 职责 | 稳定接口 |
| --- | --- | --- |
| `LoopStateMachine` | 三态状态机：IDLE / SENDING / SPEAKING | `transition(state, event)`、`transition(event)` |
| `LoopCoordinator` | 状态机副作用执行与编排 | `sendMessage(text)`、`onEvent(event)`、`attachView(host)` |
| `LlmLink` | LLM 冷流接口 | `send(text): Flow<LlmEvent>`、`reset()` |
| `HttpLlmLink` | DeepSeek/OpenAI 兼容 SSE 实现 | 实现 `LlmLink` |
| `ExpressionSink` | 表情落点 | `apply(emotion)`、`release()` |
| `SpeechSink` | 语音播报落点 | `speak(text)`、`stop()` |
| `LoopViewHost` | Live2D 视图宿主接缝 | `zeroMouth()`、`onAttach()`、`onDetach()` |

### 2.2 Android TTS 层

| 模块 | 职责 | 稳定接口 |
| --- | --- | --- |
| `TtsProvider` | TTS 引擎统一接口 | `speak(text)`、`stop()`、`isSpeaking`、`audioFrameFlow` |
| `TtsProviderRegistry` | 引擎注册与降级链 | `register()`、`cascade()`、`get()`、`all()` |
| `VoiceIoController` | 播报统一入口 + LipSync 驱动 | `speak(text)`、`stopSpeaking()`、`isSpeaking` |

### 2.3 PC 核心层（`Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/`）

| 模块 | 职责 | 稳定接口 |
| --- | --- | --- |
| `conversations/tts_manager.TTSTaskManager` | TTS 并发合成 + 顺序发送 | `speak_async()`、`flush()`、`interrupt()` |
| `mood_engine.MoodEngine` | 持续心情值 → 表情/TTS/prompt 微调 | `on_emotion_event()`、`get_mood_tier()`、`get_tts_modifier()` |
| `tts/tts_interface.TTSInterface` | TTS 引擎统一接口 | `synthesize()` 等 |
| `agent/stateless_llm/stateless_llm_interface` | LLM 统一接口 | `chat()` / `stream_chat()` |

## 3. 共享契约（`shared/`）

| 文件 | 内容 | 使用方 |
| --- | --- | --- |
| `persona.yaml` | 人设、system prompt、模型参数 | Android assets + PC conf |
| `model_registry.json` | Live2D 模型注册表 | Android assets + PC fallback |
| `model-adapter/bai.adapter.json` | 表情索引 / 参数帧 / motion 别名 | Android + PC 一致性校验 |
| `emotion-protocol.md` | 8 表情协议、JSON/`[keyword]` 降级规则 | Android + PC 解析 |
| `mcp_tools.json` | MCP 工具定义 | PC MCP 服务 + Android 预留 |
| `model_dict.json` | **LLM 模型路由表**（注意：不是 PC Live2D model_dict） | PC / 未来 Android LLM 配置 |

> ⚠️ 易混淆点：
> - `shared/model_dict.json` = LLM 路由表（deepseek / glm 等）
> - `Live2D-Ai-pc/open-llm-vtuber/model_dict.json` = PC Live2D 模型字典（name / emotionMap / url）
> - 两者名称相同但用途不同，文档中必须区分。

## 4. 状态机契约（Android）

```
IDLE
  ├─ SubmitText        → SENDING: [LlmSend, UiAddUserBubble]
  └─ Reset             → IDLE:   [SpeechStop, MouthZero, ExprRelease, LlmReset]

SENDING
  ├─ LlmDelta          → SENDING: [UiAppendStreamBubble]
  ├─ LlmCompleted      → SPEAKING: [ExprApply, SpeechSpeak]
  ├─ LlmFailed         → IDLE:   [UiShowError]
  └─ Reset             → IDLE:   [reset commands]

SPEAKING
  ├─ SubmitText        → SENDING: [SpeechStop, MouthZero, ExprRelease, LlmSend]  // 打断
  ├─ SpeakFinished     → IDLE:   [MouthZero, ExprRelease]
  ├─ SpeakFailed       → IDLE:   [MouthZero, ExprRelease]
  └─ Reset             → IDLE:   [reset commands]
```

核心不变量：

- I1：SENDING 中拒绝新提交，至多一个 LLM 请求在途。
- I2：TTS 播报单走廊，双播报免疫。
- I3：LLM 冷流单消费者。
- I6：视图接线替换语义，不累积双 hook。
- I7：任意副作用异常 fail-safe 回 IDLE。

## 5. 扩展边界（未来插件）

所有“非最小闭环”能力都应走扩展接口，而不是进入核心：

- `ChatHook`：beforeSend / afterReceive
- `ToolDefinition`：LLM 工具调用
- `TtsProvider` / `LlmLink`：Provider 接缝
- 视觉、记忆、主动陪伴、Agent 工具：插件实现

核心只依赖接口，不依赖具体插件。

## 6. 验收口径

任何改动只要影响以下任一项，必须保证不破坏：

1. Android JVM 单测全绿（当前 663）
2. PC pytest 全绿（当前 150）
3. `shared/check_cross_platform_consistency.py` 全绿
4. Android 核心状态机穷举测试全绿
5. 无 API Key、无网络时，配置解析 / 状态机 / TTS 降级链行为确定


## 7. Renderer 参数空间约定

PC 渲染器（`renderer/src/`）同时处理两类参数写入源：

- **model 空间**：绝对 Cubism 参数值（例如 `ParamAngleX` 的 -30..30）。
- **normalized 空间**：0..1 的归一化值，需要按每个参数的 min/default/max 映射到 model 空间。

### 规则

1. 写入 `ParamArbiter` 时，通过 `valueSpace: 'model' | 'normalized'` 显式声明值空间；未声明时默认 `model`。
2. `HalfBodyController`、表情、口型等来源如果产生 normalized 值，必须标记 `valueSpace: 'normalized'`。
3. `ParamArbiter.update()` 负责把 normalized 源映射到 model 空间后再写入 coreModel，保证多源混合时单位一致。
4. 每个参数在 fade 完成后若值未变化，通过 `dirty` 标记跳过重复写入，降低 GPU/CPU 开销。

### 为何需要区分

历史 bug：`HalfBodyController` 直接写入 normalized 值，而 `EmotionConsumer` 写入 model 值，导致半身动作后表情参数被错误缩放。统一 valueSpace 后，所有源在仲裁前已对齐到同一空间。
