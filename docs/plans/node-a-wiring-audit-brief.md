# 高级节点 A —— 最终应用接线前审计（交接文档）

> **给谁看**：被指派对本文档所列问题做最终裁决的高级 Agent。
> **何时**：任何「ConversationEngine 接桌面状态机/音频/模型」的实现代码开始**之前**。
> **产出要求**：见文末「六、审计输出格式」。实现方将严格按裁决落地，不再自行决断。

---

## 一、背景与范围

Rust 重建当前可运行：透明原生窗口 + Bai 实时渲染 + 自动六动作 + 合成口型参数
（`--model-smoke` / `--pet-mode`，WSLg 实测 present）。所有底层模块已验证，
**尚无一条真实对话链路**。本审计只回答一个问题：

> 把现有模块串成「终端输入 → LLM → TTS → 声卡 → 动作/口型 → 原生窗口」时，
> 并发、取消、所有权与参数仲裁的**唯一规则集**是什么？

不在本审计范围：渲染非阻塞改造（节点 C）、发布删旧（节点 D）。

## 二、现状快照（2026-08-26）

| 模块 | 位置 | 状态 |
|---|---|---|
| LLM SSE / TTS chunked 客户端 | `crates/live2d-ai-runtime/src/{llm,tts,sse}.rs` | 已验证 |
| 对话流水线引擎 | `crates/live2d-ai-runtime/src/conversation.rs` | 已验证，未接 UI |
| 句子切分 / 工具组装 | `crates/live2d-ai-runtime/src/dialogue.rs` + `tool.rs` | 已验证 |
| PCM 解码/重采样/RMS | `crates/live2d-ai-runtime/src/audio/*` | 已验证 |
| 应用配置 `live2d-ai.toml` | `crates/live2d-ai-runtime/src/settings.rs`（本批新增）+ 根目录模板 | 测试 7 绿 |
| 终端 REPL 纯解析 | `crates/live2d-ai-desktop/src/repl.rs`（本批新增，未接线） | 进行中 |
| 声卡输出（cpal+ringbuf+epoch 打断） | `crates/live2d-ai-desktop/src/audio.rs` | 冒烟通过，未接对话流 |
| winit/wgpu 窗口壳 + 桌宠能力 | `crates/live2d-ai-desktop/src/app.rs`（`ShellApp`）、`platform.rs` | 冒烟通过 |
| 六动作确定性曲线 + PerformancePlayer | `crates/live2d-ai-core/src/action*` + `performance.rs` | 已验证 |
| 参数帧 → Bai 参数适配 | `crates/live2d-ai-desktop/src/adapter.rs` | 已验证 |
| 托盘 ksni + 跨线程 UserEvent | `tray.rs` + `user_event.rs` | 代码在，最后一轮验证待回收（节点 C 复核） |

测试基线：runtime lib 107 绿（本批新增 settings 7 个）；desktop 在基线复跑中发现
1 个断言失败（`backend::tests::description_declares_pet_capabilities_and_runtime_gate`
——描述文本与测试漂移），修复进行中。全 workspace 统一复跑在本节点收口后执行。

## 三、关键契约速查（审计引用用）

```rust
// conversation.rs:314 — 引擎入口；&mut self ⇒ 单任务内天然互斥
pub async fn run_turn(&mut self, epoch: u64, user_text: &str,
    event_tx: mpsc::Sender<EngineEvent>, cancel: CancellationToken) -> TurnReport

// EngineEvent（均带 epoch）：TextDelta | ToolAction{index,id,args}
//   | AudioChunk{sentence_seq,samples,spec,final_chunk} | Error{kind}
//   | Done{} | Cancelled{}   ← 收尾契约：Done 保证在「LLM 结束 && TTS worker 排空」后，
//     每句音频恰好一个 final_chunk=true；取消轮只有 Cancelled。

// audio.rs — 声卡侧
PlaybackHandle::enqueue_pcm_f32(&mut self, &[f32]) -> EnqueueOutcome // 需 &mut：SPSC 单生产者
PlaybackHandle::stop_and_clear(&self) -> u64                        // epoch 打断
PlaybackHandle::is_drained(&self) / mouth_level(&self) -> f32       // 原子快照

// performance.rs / adapter.rs — 动作与参数
PerformancePlayer::{play(SemanticAction), interrupt(), update(dt, &mut ParameterFrame) -> SampleStatus}
BaiParamAdapter::{apply_frame(core,&frame)->ApplyOutcome, apply_mouth_level(core,level)->bool}

// user_event.rs — 事件循环通信（当前仅托盘 5 个 Copy 变体）
PetUserEvent::TrayToggleClickThrough | TrayToggleAlwaysOnTop | TrayToggleVisible
  | TraySetVisible(bool) | TrayExit    // 经 EventLoopProxy 发送；Window 调用只在主线程
```

## 四、必须裁决的问题（D1–D14）

### 组 1：进程结构与所有权（最高优先级）

- **D1 tokio Runtime 放哪**：建议方案——`Runtime::new()` 于 main 启动一个
  后台线程跑 `Runtime::block_on(supervisor_task)`；winit 主线程完全不碰 tokio。
  是否同意？替代方案（main 内 `Handle::block_on` 包住事件循环）是否明确否决？
- **D2 ConversationEngine 归属**：`&mut self` ⇒ 建议 supervisor 任务独占持有；
  REPL 输入经 `mpsc<Command>` 进 supervisor。确认或给出别的所有权模型。
- **D3 EngineEvent 的消费位置**：AudioChunk 数据量大，建议 supervisor 内直接
  `enqueue_pcm_f32`（声卡 facade 移入 supervisor），窗口线程只读 mouth 原子快照 +
  收轻量通知事件。若否决，音频块过 EventLoopProxy 的代价与背压如何处理？
- **D4 PetUserEvent 非扩展性问题**：现枚举是 `Copy` 且只有托盘变体。建议新增独立
  `AppEvent`（内含 `Tray(PetUserEvent)` 与 `Conversation(...)` 轻量变体），winit
  `EventLoopProxy<AppEvent>`。是否接受该类型重构？
- **D5 mouth 快照读取路径**：`mouth_bits` 在 `SharedState`(Arc) 内，但当前没有
  独立 reader 句柄类型。建议 audio.rs 增加 `MouthSnapshot` 克隆句柄（Arc clone），
  ShellApp 持有之。确认接口形状。

### 组 2：turn 生命周期与取消

- **D6 root epoch 语义**：core 侧 epoch 计数器放 supervisor 还是 ShellApp？
  `/stop` 时 epoch 是否必须自增（使迟到 TextDelta/AudioChunk 全部过期）？建议：是。
- **D7 stop 的完整扇出**：`/stop` ⇒ cancel_token.cancel() + handle.stop_and_clear()
  + PerformancePlayer.interrupt()。三者顺序与原子性要求？cancel 与 clear 之间
  已 enqueue 未播样本由 epoch 过滤兜底是否足够？
- **D8 turn 结束条件**：建议 `Done || Cancelled` **且** `handle.is_drained()` 才允许
  下一次 Say 进入引擎（否则排队）；Error 不清空已入队句子（继续播完）。
  「LLM 出错后已入队句子播不播」请明确裁决。
- **D9 迟到事件闸门位置**：EngineEvent 自带 epoch；建议 supervisor 消费时统一丢弃
  `event.epoch != current_epoch` 的事件，主线程不做二次过滤（信任 supervisor）。
  是否同意？

### 组 3：动作 / 口型 / 物理仲裁

- **D10 写入源优先级终表**：建议每帧合成顺序：
  模型默认值 → idle 呼吸 → 动作 ParameterFrame（PerformancePlayer.update）
  → 口型通道（仅 ParamMouthOpenY，来自 RMS 快照）→ physics（Bai 自带）→ 无 final override。
  特别裁决：a) 动作曲线是否允许覆盖 MouthOpenY（建议：不允许，口型最后写且独占通道）；
  b) blink/eye_open_scale 由谁管（v0 建议：模型自带眨眼，应用不写）。
- **D11 ToolAction 与规则 fallback**：tool `live2d_perform_action` 成功解析 ⇒
  PerformancePlayer.play(action)，同轮后续规则 fallback 静默；无 tool 或解析失败 ⇒
  规则 fallback（关键词→动作）。fallback 表放 core 还是 desktop？
- **D12 动作期间新动作到达**：interrupt() 抢占 or 忽略？建议：同强度忽略、
  更高强度 interrupt（沿用 core 既有语义则直接引用其规则，需指明出处行号）。

### 组 4：REPL 与退出

- **D13 `/stop` `/release` `/quit` 终义**：release = 仅退点击穿透恢复交互？
  还是同时置顶复位？（建议：只动 click_through。）`/quit` 是否需要先 stop 当前轮
  再退出（建议：是，且给 TTS/cpal drop 一个上限等待，如 500ms，超时强退）。
- **D14 stdin 线程模型**：建议专用阻塞线程读行 → `proxy.send_event(AppEvent::ReplLine)`
  或直接 `command_tx.try_send`。EOF（Ctrl-D）按 /quit 处理。二选一请裁决。

## 五、建议接线架构草案（供推翻）

```text
main()
 ├─ Runtime::new() + 后台线程 block_on(supervisor)
 │    supervisor: ConversationEngine + AudioOutputFacade + Command mpsc(rx)
 │      loop { select! { cmd = cmd_rx.recv() => …,
 │                        ev = engine_event_rx.recv() => filter(epoch) → enqueue/play/proxy } }
 ├─ REPL stdin 线程 → Command mpsc(tx)（Say/Stop/Quit）
 └─ winit 主线程: ShellApp( MouthSnapshot, proxy )
      window_event: 每帧 update(dt)+apply_frame+apply_mouth(level=快照)
      user_event:   AppEvent::{Tray, Conversation(轻量状态), ExitRequested}
```

背压边界：LLM→TTS 有界队列（engine 内部，容量 4）；supervisor→声卡为
非阻塞 enqueue（拒绝计数已有）；Command mpsc 容量建议 8，满时 REPL 提示稍候而非阻塞。

## 六、审计输出格式

对 D1–D14 逐条给出：`裁决`（一句话）+ `理由`（≤3 句）+ `实现注意点`（可选）。
另设一节 `额外风险`：列出草案中你发现的、上述清单没有覆盖的问题。
实现方将以你的裁决为唯一规则源开始接线（预计涉及 app.rs / 新 supervisor 模块 /
audio.rs 小改 / cli.rs 新增 chat 子命令）。
