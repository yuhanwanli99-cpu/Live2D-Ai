# live2d-ai-runtime — 最小统一 API 层（Rust 重建）

把「OpenAI-compatible LLM（`/chat/completions`，`stream=true` SSE）」与
「OpenAI-compatible TTS（`/audio/speech`，chunked bytes）」统一成一套最小客户端 API。
本地服务（Ollama/vLLM 等）与云端同构，仅 `base_url` 不同。
在此之上提供**对话流编排纯逻辑**（`dialogue` 模块）：流式切句、工具调用组装、
统一的对话事件流；以及把它们串成流水线的**最小异步对话引擎**
（`conversation` 模块，见下文）。

## 边界（与 RFC D6/D7/D9 一致）

- **不内置任何 provider / TTS 引擎**，不调各家私有 SDK；
- TTS 传输层**原样转发 chunked bytes**；音频纯处理收敛在 `audio` 模块：
  - `response_format = "pcm"` 是本批唯一保证的格式，明确定义为
    **little-endian signed 16-bit interleaved raw PCM**（默认按 24 kHz 单声道解释，
    `TtsConfig::spec` 可覆盖，非零校验）；
  - [`PcmS16LeDecoder`]：增量解码，支持任意奇数字节 chunk（半个样本保留到下批），
    输出归一化 f32 ∈ [-1, 1]；`finish` 对悬挂字节报 `TruncatedPcm`；
  - [`RmsMeter`]：从「实际被声卡消费的样本」滑动窗口算 RMS，带 attack/release
    平滑与 [0,1] clamp；静音精确回 0、永不产出 NaN；
  - [`SampleQueue`]：有界 f32 样本 FIFO；溢出策略显式（接纳前缀并返回接纳数），
    不阻塞音频线程；现为纯 std `VecDeque`，接入 cpal 无锁回调时内部换 `ringbuf`
    而对外 facade 不变；
  - WAV / MP3 / FLAC 等一律 `Error::UnsupportedFormat`——**不假装流式支持**；
- **不做 GUI**；动作能力 gate 与优先级仲裁由 `live2d-ai-core` 处理，
  本 crate 只固定 `live2d_perform_action` 的 wire 类型与请求 schema；
- 纯逻辑的 [`SseDecoder`] 支持任意字节切割、多行 data、`[DONE]`、
  `delta.content` 与 `tool_calls.function.name/arguments` 分块——绝不假设单个 chunk 是完整行。

## 对话流编排（纯逻辑，`dialogue` 模块）

把 LLM 增量事件整理成「完整句 / 完整工具调用 / 结束」，**不发网络、不做 TTS**，
无锁无线程，输入任意切割粒度都安全：

- **`LlmEvent::ToolCallDelta`** 携带 `index`（wire 缺省 0）与 `id`（通常仅首个分片携带），
  同轮多个工具调用靠 `index` 分组组装。
- [`SentenceAssembler`]：`TextDelta` → 完整句（含原标点）。中英文 `.?!。！？…\n`
  为句界；省略号/连续标点折叠为一个句界、不产生空句；`.` 保守规则——小数
  （`3.14`）与后随小写字母的缩写续词不切；缓冲字符数有上界，超限按纯位置强制
  切分；`flush` 收尾。核心保证：**任意 delta 切割 ⇒ 完全一致的输出**，且输出
  拼接 == 输入（逐字不丢）。
- [`ToolCallAssembler`]：按 `index` 组装 name/arguments 分片；`seal_parseable`
  在「新 index 出现 / 正文恢复 / DONE」等时机取走已完整的调用；`finish` 对应
  `[DONE]`/flush，全量结算——要么全部成功（解析为
  [`Live2dPerformActionArgs`]），要么返回类型化 [`IncompleteToolCalls`]：
  `MissingName` / `MissingArguments` / `InvalidArgumentsJson` /
  `UnsupportedTool`（只接受 `live2d_perform_action`），逐调用带 index/id，
  绝不静默丢弃。**不做**能力/优先级判定（归 core）。
- [`DialogueAssembler`]：组合两者输出 [`DialogueEvent`]——`SentenceReady` /
  `ToolReady` / `Done`（携带可选的残缺报告）；文本与工具的相对顺序 =
  网络到达顺序（正文恢复或新 index 出现时先封存挂起结果再处理当前事件）。
  `flush` 是对端未发 `[DONE]` 就关流时的幂等兜底，终态后 `push` 一律忽略。

```rust
use live2d_ai_runtime::{DialogueAssembler, DialogueEvent, LlmEvent};

let mut d = DialogueAssembler::new(64); // 句子缓冲上限 48~128 都合理
let events = d.push(&LlmEvent::TextDelta("你好呀！今".into()));
// …继续 push TextDelta/ToolCallDelta/Done；对端关流未发 DONE 时调 d.flush()
```

## 最小异步对话引擎（`conversation` 模块）

[`ConversationEngine::run_turn`] 把「LLM 流 → 切句 → TTS → PCM」串成一条
**边生成边合成**的流水线，事件经调用方提供的 `tokio::sync::mpsc` 通道送出：

- **重叠**：第一句切出即进入内部有界队列并立刻发起 `/audio/speech`，
  不等 LLM 结束；集成测试用全局顺序日志证明了这一重叠；
- **严格串行 TTS**：单个 worker 任务按 `sentence_seq`（从 1 递增）逐句合成，
  每句恰好一个 `final_chunk = true` 的 [`EngineEvent::AudioChunk`]；
  增量解码复用 `PcmS16LeDecoder`（奇数字节 chunk 安全），样本按上限切块；
- **事件**：全部携带 `epoch`（core 二次闸门）——`TextDelta` / `ToolAction` /
  `AudioChunk{samples, spec}` / `Error{kind}` / `Done` / `Cancelled`；
- **终态语义（每轮恰一次）**：正常 → `Done`（保证在 LLM 结束 **且** TTS 完全
  排空之后、位于最后）；可恢复错误（残缺工具调用）→ 先 `Error` 再 `Done`，
  **不阻断已生成文本/TTS**；致命错误（`Llm` / `Tts` / `Decode` /
  `Backpressure`）→ 停止生成、丢弃未合成句子后 `Done`；取消 → 只发
  `Cancelled`；
- **取消**：`tokio_util::sync::CancellationToken` 与每个 await 点（网络读取、
  事件发送、队列推送）`select!`；取消后不再产生新事件并尽力 drop 响应。
  引擎内部使用 child token，致命错误时只中止自己的 worker，不触碰调用方令牌；
- **背压与内存界**：句子队列有界（默认容量 4），推满阻塞即把背压回传到 LLM
  消费速率；可选 `queue_push_timeout` 把「下游长期不消费」升级为
  `ErrorKind::Backpressure`；音频按 `audio_chunk_samples`（默认约 200ms@24kHz）
  切块，绝不无限缓冲；
- **并发防护边界**：单 turn 并发由调用方/root core 保证（`&mut self` 在单任务内
  天然互斥）；引擎自身每轮恰好 spawn 一个 TTS worker 并在返回前 join——无泄漏。

```rust,ignore
let (event_tx, mut event_rx) = mpsc::channel(64); // 建议 ≥ tts_queue_capacity + 8
let report = engine.run_turn(epoch, user_text, event_tx, cancel).await;
while let Some(event) = event_rx.recv().await { /* TextDelta/AudioChunk/… */ }
```

历史记忆默认关闭（`max_history_pairs: 0`）；设为正数后，成功轮会把
`(user, assistant)` 提交进环形历史并注入后续请求。

## 安全约定

- API key 用 `ApiSecret` 包装：`Debug`/`Display` 恒定脱敏，不实现 serde；
- key **只**进入 `Authorization: Bearer …` 请求头，不进 URL/请求体/错误文本；
- TLS 固定 rustls（reqwest `default-features = false` + `rustls-tls`），不用 native-tls；
- HTTP 错误、状态码、JSON/SSE 解析失败都有可观测的错误变体（见 `error::Error`）。

## 依赖

库本体仅：`reqwest`(rustls/json/stream)、`serde`/`serde_json`、`futures-util`、
`thiserror`、`url`；异步设施 `tokio`(sync/rt/time/macros) 与
`tokio-util`(CancellationToken) 只服务于 `conversation` 模块的最小对话引擎。
测试用本地 tokio TCP mock（dev-dependencies 额外启用 net/io-util）。
`audio` 模块为纯 std 实现，零新增依赖。

```sh
cargo test -p live2d-ai-runtime
```

## 许可

**AGPL-3.0-only**。以仓库根 `LICENSE` 为准（完整文本见仓库根 `LICENSE`）；
本 crate 不提供独立 LICENSE 文本。商业使用条款见根 `LICENSE` 与 `COMMERCIAL-LICENSE.md`。
