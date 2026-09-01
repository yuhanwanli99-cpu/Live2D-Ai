# 节点 F/G 执行计划（核心链路收口 → 5 Mod 开箱即用 → 全平台）

> 接续 `node-e-execution-plan.md`（节点 E 已完成，tag `node-e-final`）。
> 主线（用户裁定）：**核心链路收口 → 5 Mod 开箱即用/UX → 全平台支持**。
> 工作模式同 E 级：原子任务 + Laguna S 2.1 FREE 子代理 + 主代理编排/review/独立复核 + 每任务 commit+tag。

## P0 核心链路收口（进行中）

- [x] **F1** 主页 65% 区接入 /render（tag `node-f-f1`，`87b4ce2a`）
- [x] **F2** 设置抽屉信息架构重排（tag `node-f-f2`，`568f8f81`）
- [x] **F3** renderMods 监听器累积 bug + 去重（tag `node-f-f3`，`ca62b558`）
- [ ] **F5** dispatch_event → per-Mod on_event 路由（子代理 `f6e2b065` 进行中；共享 runtime 槽位方案；director TurnStarted 序列生效前提）
- [ ] **F6-T1** WS 音频帧后端（子代理 `11720e96` —— `build_audio_frames`/`broadcast_audio_frames` 已落盘 55 测试绿，待 review 提交）
- [ ] **F6-T2** PCM 流经处接线（supervisor/audio 侧调 broadcast_audio_frames；含 cpal 双声消解开关）
- [ ] **F6-T3** 前端 audio 帧播放器（chat.js：AudioContext 队列 + volume 口型联动 + epoch 打断，参照 py 版 ws-bridge.ts 语义）
- [ ] **F7** README CLI 段落张力修正（小）
- P0 验收：浏览器端到端——发消息→流式回复→**浏览器出声**（Windows 声卡）→口型动。这是"开箱即用"的定义线。

## P1 五 Mod 开箱即用 / UX

- [ ] **F8** `live2d-ai-mod-local-llm`：本地 LLM Mod（ollama/llama-server 进程 spawn+健康检查+就绪探测+自动指 base_url；settings: 二进制/端口/模型/自启）
- [ ] **F9** `live2d-ai-mod-local-tts`：本地 TTS Mod（kokoroi-rs 静态二进制生命周期，OpenAI 兼容 `/v1/audio/speech`，零客户端改动；settings: 路径/端口/声线/自启）
- [ ] **F2b** 抽屉三占位组填实：动作与互动（动作面板/强度）、外观与舞台（背景/缩放/口型开关）、模型管理（模型列表/切换，对接 `/api/v1/models`）
- [ ] **F10** external-input 实用化：token 鉴权（settings 已有 secret 字段）+ curl/webhook 示例 + 文档
- P1 验收：全新机器装好 5 Mod → 抽屉里一键起本地 LLM+TTS → 对话/动作/口型/音频全链路可用，无需手动配端点。

## P2 全平台支持

- [ ] **F4** Tauri v2 桌宠壳：透明窗+点击穿透（`set_ignore_cursor_events`）+托盘+WebView 加载主页；settings_ui(egui) 迁入 pet-desktop Mod；Win/Mac/Linux 一壳三平台。注：Wayland 穿透受合成器限制（WSLg 即 Wayland），Windows 体验最佳——平台事实。
- [ ] **G1** Android 悬浮窗壳：Kotlin 薄 APK（SYSTEM_ALERT_WINDOW + WebView 指向后端），模型/对话/音频全走现有 Web 前端
- [ ] **G2** Termux/headless 支持：core 链路无窗口依赖已成立，补构建目标+文档（winit/wgpu/egui/托盘均在 Mod 边界内不阻塞）
- [ ] **G3** CI 多平台产物：windows-msvc / aarch64-apple-darwin / linux-musl / android aar
- P2 验收：PC 桌宠模式（可穿透置顶）+ 手机悬浮窗 + 服务器裸跑三种形态同一套后端+前端。

## 技术备忘（调研结论存档）

- **py 版出声机制**（py-legacy 分支 ws-bridge.ts）：TTS 音频 20ms 切片 base64 → WS JSON `{"type":"audio"}` → 前端 `new Audio(blob)` 播放——音频从来在浏览器（Windows 声卡），WSL 音频不参与。F6 复刻此已验证路径。
- **F6-T1 帧协议**：`{"type":"audio","data":{"epoch","audio"(b64 s16le mono),"sample_rate","slice_ms":20,"volume"(RMS 0..1),"start","end"}}`。
- **kokoroi-rs**（doiito）：Rust+ONNX 中文 TTS，OpenAI 兼容端点，静态构建（musl/MSVC）——F9 首选。pocket-tts（Python 进程）不契合；pguso/kokoro（纯库缺服务层）次选。
- **Qwen3.5-4B**：Q4 ~2.5GB / Q5 ~2.9GB，RTX 5060 8GB 充裕；经 ollama/llama-server 的 OpenAI 兼容端点接入（不 FFI 嵌入推理，守住 Rust 边界）——F8 形态。
- **平台窗壳结论**：Web 核心（HTTP+WS+浏览器）天然全平台；桌宠透传/手机悬浮窗必须薄原生壳（Tauri / Kotlin WebView），已是最小代价方案。
- **双声问题**：F6-T2 落地后服务端 cpal 播放与前端播放并存会双声——需 settings 开关（默认前端播放=平台解锁），服务端播放保留给 headless 场景。
