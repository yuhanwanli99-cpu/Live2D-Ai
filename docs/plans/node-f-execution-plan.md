# 节点 F/G 执行计划（核心链路收口 → 5 Mod 开箱即用 → 全平台）

> 接续 `node-e-execution-plan.md`（节点 E 已完成，tag `node-e-final`）。
> 主线（用户裁定）：**核心链路收口 → 5 Mod 开箱即用/UX → 全平台支持**。
> 工作模式同 E 级：原子任务 + Laguna S 2.1 FREE 子代理 + 主代理编排/review/独立复核 + 每任务 commit+tag。

## P0 核心链路收口（进行中）

- [x] **F1** 主页 65% 区接入 /render（tag `node-f-f1`，`87b4ce2a`）
- [x] **F2** 设置抽屉信息架构重排（tag `node-f-f2`，`568f8f81`）
- [x] **F3** renderMods 监听器累积 bug + 去重（tag `node-f-f3`，`ca62b558`）
- [x] **F5** dispatch_event → per-Mod on_event 路由（tag `node-f-f5`，`86f593b0`；SharedRuntime 槽位 + try_lock 竞争跳过 + 失败清槽隔离，7 测试全绿）
- [x] **F6-T1** WS 音频帧后端（tag `node-f-f6t1`，`98c48c53`）
- [x] **F6-T2** PCM 接线（tag `node-f-f6t2`，`37f1b490`；AUDIO_CB thread-local + `LIVE2D_AI_WS_AUDIO=1` 开关）
- [x] **F6-T3** 前端播放器（tag `node-f-f6t3`，`acd557ed`；AudioContext 队列+epoch 打断+口型双通道）
- [ ] **F6 端到端验收**：`LIVE2D_AI_WS_AUDIO=1` 起服务 → 配好 LLM/TTS → 浏览器发消息 → **浏览器出声**（人类实测，需真实 LLM/TTS 端点）
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

## P1 完成状态（已 tag `p1.0.0`）✅

- F8 local-llm（`p1-f8`，含 F9 local-tts）：5 Mod 全注册 AVAILABLE_MOD_FACTORIES，20 组测试绿、rust-ratio PASS。
- F10 external-input token 鉴权（`p1-f10`）：纯函数 check_token + docs。
- F2b 抽屉三占位组填实（`p1-f2b`）：动作面板/外观舞台/模型管理，9 新函数。
- P1 完成 tag `p1.0.0`。

## P2 完成状态（已 tag `p2.0.0`）✅（本会话可验证项全落地）

- **G2** headless/Termux 部署（`p2-g2`）：`--web` 已证纯 headless，0 代码改动，docs/headless-deploy.md。
- **G3** CI 多平台 release-build（`p2-g3`）：ubuntu/windows/macos 三平台二进制 + wasm 可选，无 Tauri/WebView2 依赖。
- **P2 全量门禁**：20 组测试 ok、fmt/clippy 干净、rust-ratio PASS。

### ⚠ F4 / G1 为真机验证任务（本会话不可验证，已列为规格待真机实施）

**F4 Tauri 桌宠壳（PC 透明窗/点击穿透/托盘）**：
- 本会话无法验证（WSL 无桌面会话/D-Bus，透明窗/置顶/托盘需真实桌面；已实测托盘因无 D-Bus 失败）。
- **不引入** Tauri 依赖污染主仓库 95% Rust 边界。
- 实施规格（真机）：Tauri v2 壳工程 → 透明窗+`set_ignore_cursor_events`（穿透）+托盘+WebView 加载 `--web` 主页；`settings_ui.rs`(egui) 迁入 pet-desktop Mod；Win/Mac/Linux 一壳三平台。注：Wayland 穿透受限（合成器），Windows 最佳。
- G3 已产出三平台后端二进制供真机接入壳。

**G1 Android 悬浮窗壳**：
- 需真机 SYSTEM_ALERT_WINDOW 权限 + WebView（Termux 拿不到悬浮窗权限）。
- 实施规格：Kotlin + WebView 指向 `http://<host>:<port>`，模型/对话/音频全走现有 Web 前端。

**平台窗壳总原则**（调研存档）：Web 核心天然全平台；桌宠透传/手机悬浮窗必须薄原生壳（Tauri / Kotlin WebView）——已是最小代价方案，F4/G1 均为独立壳工程，不污染核心。


## P3 真实性收口（dev/integrity 分支，验收=可见效果，非"测试绿"）

> 审计裁决：dev/node-p1-p2 定性「功能骨架 / integration incomplete」。原 p1/p2 tag 保留为回滚锚点。
> 审计核实的假接线：debug WASM 23MB / LowPower adapter / WASM 无 message listener（外观·口型·动作·换模型全 no-op 消费）/ ModServices no-op sender / dispatch_event 无生产者 / local-llm·tts 不写 runtime settings。

- [x] **P0-1a** release WASM：dist 5.27MB（原 23.7MB debug）
- [ ] **P0-1b** HighPerformance + DPR cap + FPS/adapter 状态栏
- [ ] **P0-2** iframe bridge（WASM message listener: stage-config/audio-volume/action-state/load-model）
- [ ] **P0-3** Mod HostServices 真接线（action→supervisor 仲裁 / event 生产侧 dispatch / say_source）
- [ ] **P0-4** local-llm/tts 写 runtime settings + reload + 真实 API 探测 + externally_managed
- [ ] **P0-5** 未接通 UI 诚实标注/隐藏
- 验收：缩放滑块画面即变 / 切背景即变 / 出声口型动 / 点 nod 模型点头 / 激活模型 iframe 换载 / Mod 启用后 settings 被真实更新。全部通过才打 `p3-integrity`。

## P3 真实性收口（dev/integrity 分支）✅ 已 tag `p3-integrity`

- P0-1 release WASM+HighPerformance+DPR+HUD（`p3-p01`）
- P0-2a-1/2/3 bridge 数据层/listener/可见效果（`p3-p02a-1/2/3`）+ P0-2b 发送侧（`p3-p02b`）
- P0-3 HostChannels 真接线（`p3-p03`；动作→core 仲裁 LlmTool 固定档）
- P0-4 local 推理真实闭环（`p3-p04`；真 HTTP 探测→写 settings→reload；externally_managed）
- P0-5 诚实标注（`p3-p05`）
- 门禁：20 组/746 passed/0 失败；fmt/clippy/rust-ratio PASS；冒烟 home/render/wasm 200 + 5 Mod
- 待人类真机验收（5 条可见标准）：缩放滑块画面变 / 切背景变 / 出声口型动 / 点 nod 模型点头 / 激活模型换载；需真实 LLM+TTS 端点

## P3.1 用户实测修复（2026-09-03，用户 5 条实测反馈驱动）✅

用户实测结果：①缩放滑块无效 ②背景切换不可见 ③无 TTS 可测 ④动作全失败 ⑤验收目的不清。修复：

- [x] **F-a** 鼠标拖动平移+滚轮缩放（tag `p3-fa`）：py 版交互对齐（pointer/wheel/dblclick，CSS 盒模型方案——l2d transform 需 glam 被正确排除）
- [x] **F-c** 动作三重根因（tag `p3-fc`）：1500ms + stale override 清除（核心 bug）+ end 帧配合；PoseStack 合成序已核实（override 最高优先级）
- [x] **F-d** 本地推理实操指南（`6aaa9861`）：docs/local-inference-setup.md
- [x] **F-e** Mod 配置管道 4 断点（tag `p3-fe`）：mods.json manifest / enable 透传 config / POST /mods/{id}/config——配置层闭合
- [x] **F-b 部分替代**：背景问题根因 = canvas 被 wgpu 清屏覆盖，CSS 背景不可见。jpg 导入待做（需 wgpu 底图层渲染）——列入下轮
- [x] N.E.K.O 调研归档（`e4639ff9`）：3 借鉴（口型 motion 曲线清洗 / director emotion_mapping schema / proactive vision 需 native host）
- 门禁：20 组 750 passed / 0 失败；fmt/clippy/rust-ratio PASS

## 待用户重测清单（明确操作步骤）

1. **拖动/缩放**：浏览器 `/render` 或主页 65% 区 → 左键拖动模型平移、滚轮缩放（0.5-2.0×）、双击复位
2. **动作**：⚙→动作与互动→点「点头」→模型点头约 1.5 秒（6 个动作逐一试）
3. **TTS→口型**：按 docs/local-inference-setup.md 装 kokoroi-rs（或 Windows ollama 侧）→ enable local-tts → 发消息 → 浏览器出声 + 口型开合
4. **背景图导入**：未实现（需 wgpu 底图层）——下轮排期
5. **换模型**：需第二个模型导入（模型管理组）——依赖资产

## 下轮排期建议（优先级序）
1. 背景图导入（wgpu 底图层）+ 纹理档位选择（4096/8192/16384）
2. director 事件生产侧投影（supervisor→dispatch_event，TurnStarted 序列生效）
3. l2d motion loader 清口型曲线（N.E.K.O 借鉴 1）
4. director emotion_mapping schema（N.E.K.O 借鉴 2）
