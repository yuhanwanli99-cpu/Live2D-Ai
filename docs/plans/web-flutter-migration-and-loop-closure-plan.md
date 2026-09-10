# 核心链路闭环补齐 + 本地 Mod 接入 + Web→Flutter 迁移计划

> 状态：**待用户拍板**（本文只做勘察与排期，不含实现）。
> 日期：2026-09-10。基线：`main`（工作区 HEAD），本机实测 `cargo test -p live2d-ai-desktop` = **490 passed**，
> `cargo run -p xtask -- rust-ratio` = **95.38% PASS**（rs 53931 / js 1497 / py 1114 行）。
> 口径：本计划区分「可门禁交付」与「外部验收（真机/浏览器/真实端点）」，外部项不 fake 为完成。

---

## 0. TL;DR（一句话结论）

| 议题 | 结论 |
|---|---|
| ① 核心链路闭环 | **代码级闭环成立且测试覆盖充分**（文本→LLM→TTS→PCM→声卡→RMS→口型→动作/渲染）；**产品级未闭环**：语音输入（STT）完全缺失、`--web` 无原生窗口（只能浏览器 wasm）、`--chat/--pet-mode` 为骨架、F6 真端点端到端未验收。 |
| ② 本地 TTS/LLM Mod | **框架接入已完成**（HostChannels→apply_settings→写盘→reload 真回路），但**默认全禁用**（无 `mods.json`）、**仅 `--web` 模式装配**、Mod 为「进程管理 + 探活」，不内置推理；有 3 处一致性/体验缺陷。 |
| ③ Web→Flutter | 桌面那个 `daemon` 是 **Flutter 笔记应用（flutter_acrylic + media_kit）**，不渲染 Live2D，只能借鉴视觉不借鉴实现。Flutter 桌面**没有成熟 Live2D 插件**（`flutter_live2d` 仅 Android/iOS）。建议：**不整体换栈**，走「Flutter 壳 + 复用 /render」或「Web 视觉升级」二选一（见 §3.3 决策）。 |

---

## 1. 勘察结论（证据）

### 1.1 核心链路：代码级闭环 ✅，产品级未闭环 ⚠️

**已闭环的链路（每段都有实现 + 测试）**

```text
文本输入（--chat REPL / POST /api/v1/chat）
  → ConversationEngine（runtime/src/conversation，单 owner）
  → LLM SSE（runtime/src/llm.rs，OpenAI /chat/completions）
  → 分句 TTS（runtime/src/tts.rs，OpenAI /audio/speech，pcm s16le）
  → AudioChunk（supervisor/turn.rs；经句柄 prepared→有界环，绝不经 AppEvent）
  → cpal 声卡（audio/output.rs + audio/ring.rs）
  → RmsMeter 对「实际写给声卡的 f32」算 RMS → mouth_bits 原子快照（ring.rs:382-383）
  → MouthSnapshot → app/frame.rs:280-289 → adapter.apply_mouth_level
  → ParamMouthOpenY（adapter/mod.rs:156，final_override 最高优先级）
动作支线：LLM 工具 live2d_perform_action → core 仲裁（capability/优先级/幂等）
  → PerformancePlayer → 渲染 override；Web 命令走 POST /api/v1/commands（不绕过 core）
渲染：原生 wgpu（--chat/--model-smoke/benchmark）或浏览器 wasm（--web 的 /render iframe）
状态机：core 纯 reducer（epoch 闸门 + 双闩锁），stop 事务、迟到事件拦截均有专测
```

证据：`docs/architecture/core-contracts.md` §1/§3/§4；`supervisor.rs:495` `run_forever`、`supervisor/turn.rs`；`supervisor/tests_loop.rs|tests_pcm.rs|tests_fault.rs|tests_stall.rs|tests_stream.rs|tests_timing.rs`；本机 490 tests 全绿。

**未闭环的部分（本计划的 P0）**

1. **语音输入不存在**。全仓库 grep 无 `getUserMedia`/`MediaRecorder`/`whisper`/`sherpa`/任何 STT；
   README「语音对话」是**目标描述**（`README.md:24,48`），契约写的「文本/语音」中「语音」无实现。
   Web UI 的 `i-mic` 只是「AI 与语音」栏目的图标，无录音按钮。
2. **`--web` 没有原生桌宠窗口**。`run_web_mode` 只起 tiny_http 控制平面（`cli_entry.rs:47`），
   Live2D 只在浏览器 `/render`（wasm）里渲染；原生窗口只在 `--chat/--model-smoke` 路径。
3. **播放链路在 `--web` 下默认「服务端出声 + 浏览器不出声」**。WS 音频广播由
   `LIVE2D_AI_WS_AUDIO=1` 显式开启（`cli_entry.rs:367`），且开启后 cpal 与浏览器**双声**，
   代码自己注明是 v1 已知缺陷（`cli_entry.rs:362-366`）。
4. **浏览器口型是音量包络近似**，非真实 RMS：`chat.js:288-298` 用播放音量 postMessage
   `audio-volume` 给 wasm，`surface.rs:1035` 直写 `ParamMouthOpenY`。
5. **桌面形态未产品化**：README 自述 `--pet-mode` 为演示占位、`--chat` 为历史产物（`README.md:117-120`）；
   置顶能力仅 X11 生效（`main.rs:8-11`）；托盘/穿透接线后置。
6. **外部验收未做**（不是缺陷但要诚实标注）：`future-roadmap-2026-09.md` R2 —— S3 16384 离屏真 GPU、
   S4 档位浏览器视觉、**F6 真实 LLM/TTS→浏览器出声→口型动**、真实角色卡对话闭环、第二模型导入，**全部未勾选**。

### 1.2 本地 TTS/LLM Mod：接通了，但默认关着、只在 web 模式生效

**已完成的接入（真回路，非占位）**

- 静态注册：`main.rs:55-61` 5 个 factory（external-input / director / pet-desktop / local-llm / local-tts）。
- Host→Mod 通道：`HostChannels{trigger_action, say, apply_settings, config_path}`（`mod_registry.rs:47-69`）；
  动作固定映射 `ActionSource::LlmTool` 后走 `supervisor.trigger_action`，不绕过 core。
- Mod→Host 闭环：就绪探测成功 → `event_tx.try_emit({"__apply_settings":true,"patch":{...}})`
  → host 拦截 marker → `apply_patch → plan_atomic_write → rename → supervisor.reload()`
  （`mod_registry.rs:233-260` + `cli_entry.rs:177-236`）。这是真正的「spawn→探活→写配置→热重载」闭环。
- 两个 Mod 的默认值与探测：local-llm `ollama serve` / 11434 / `/api/tags|/v1/models`
  （`live2d-ai-mod-local-llm/src/lib.rs:46-51,280-287`）；local-tts `kokoroi-rs serve --port 8000` /
  `af_heart` / `/v1/models`（`live2d-ai-mod-local-tts/src/lib.rs:47-52,248`）。
- 前端已有「Mod 管理」面板：`app.js:522-556` 启用/禁用/重启 + config，走 `/api/v1/mods*`（`mods_routes.rs`）。

**缺口 / 缺陷（P1）**

| # | 问题 | 证据 | 影响 |
|---|---|---|---|
| M1 | **默认全禁用**：启用状态只来自 `<config_dir>/mods.json`，仓库与示例都没有该文件 → `parse_mod_config` 一律 `enabled=false` | `mod_registry.rs:387-394`、`cli_entry.rs:442-461` | 用户装好 ollama/kokoroi 也「不生效」，没有开箱默认 |
| M2 | **只有 `--web` 装配 ModRegistry** | `cli_entry.rs:147-253`；`backend/chat.rs:180` 传 `mod_events: None` 且无 ModRegistry | `--chat` 原生链路拿不到本地 LLM/TTS |
| M3 | local-llm 默认模型 `qwen3.5:4b` 疑似笔误（ollama 无此 tag） | `local-llm/lib.rs:49` | 探测通过但真对话 404 |
| M4 | spawn 失败语义不一致：local-tts 返回 `Err`→Mod `Failed`；local-llm 只记 `last_error` 继续 | `local-tts/lib.rs:204-215` vs `local-llm/lib.rs:222-231` | 同构 Mod 行为不齐，前端状态含义漂移 |
| M5 | 订阅未参与路由：`subscribe()` 只登记 topic（v1 注释明说），worker 把**所有** topic 广播给**所有** Running Mod | `mod_registry.rs:414-419, 367-383` | 订阅语义名不副实，Mod 收到无关事件 |
| M6 | 探测线程用裸 `std::thread` + 手写 HTTP/1.1，连接/响应解析极简 | 两 Mod `http_get` | 边界情况（分块/非 200 行）脆弱，但仅影响探活 |

> 说明：两个 Mod 都是**进程生命周期管理**（spawn/探活/回收/写 settings），**不内置推理**，设计上如此，不算缺陷。

### 1.3 前端现状 & 桌面 `daemon` 对照

**当前 Web 前端**：非框架 vanilla JS/CSS，经 `include_str!` 嵌入二进制
（`index_html.rs:42-46`：`/static/{style.css,app.js,chat.js}`）。
规模 `app.js` ~905 行 + `chat.js` ~700 行 + `style.css` 21KB + `index.html` 19KB；
布局 65%（`/render` iframe）/ 35% 聊天；设置是「模态 + 左 rail + 8 section」（已做过一轮重做）。

**`C:\\Users\\33784\\Desktop\\daemon` 的真相**（已实际解包勘察）：
- 它是 **Flutter Windows 应用**（`flutter_windows.dll` + `data/app.so`），用 **flutter_acrylic**（窗口磨砂/超透）、
  **media_kit/libmpv**（GIF/视频纹理背景）、file_selector（见 `THIRD_PARTY_NOTICES.txt` 与 `app.so` 符号）。
- 但功能是**笔记/灵感管理**（`使用说明.txt`：灵感、收藏、分类、清单），**完全不渲染 Live2D**。
- 结论：它「好看」来自 **Flutter Material 3 + 原生丙烯酸窗口效果**，**不能证明 Flutter 能承载 Live2D**；
  视觉可借鉴，实现不可照搬。

**Flutter 桌面渲染 Live2D 的现状（外部调研）**：
- `flutter_live2d` 1.0.2 仅 **Android/iOS（OpenGL ES 2）**，无 Windows/Linux 桌面实现
  （[pub.dev](https://pub.dev/packages/flutter_live2d)）。
- 项目刻意**不用 Live2D 专有 Cubism 闭源 SDK**（用 Ayagami MIT + wgpu，`README.md:54-55`），
  若走 Cubism Native SDK FFI 会与许可/路线冲突。
- Flutter Windows 外部纹理 + Impeller 有已知崩溃 issue（[flutter#190774](https://github.com/flutter/flutter/issues/190774)），
  Rust wgpu ↔ Flutter 纹理互操作属 R&D，不是接线工作。

---

## 2. 缺口清单（按优先级）

- **P0-A 语音输入（STT）**：唯一让「文本/语音」名实相符的缺口。
- **P0-B `--web` 单一声源 + 真实口型**：现网默认浏览器无声、开启即双声；F6 卡在这。
- **P0-C F6 真端点端到端验收**：真实 LLM/TTS 下「浏览器发消息→出声→口型动→动作」。
- **P1-D 本地 Mod 开箱可用**：M1（默认启用/一键启用）+ M3 + M4 + M2。
- **P2-E UI 形态**：Flutter 迁移 or Web 视觉升级（决策见 §3.3）。

---

## 3. 执行计划（波次）

> 纪律：每波独立可交付、可回滚；门禁 = `cargo test --workspace --all-targets` + `--doc`
> + `fmt` + `clippy -D warnings` + `rust-ratio ≥95`；外部项标注不 fake。

### 3.1 波 A：核心链路收口（不改架构）

| 任务 | 内容 | 交付/验收 | 规模 |
|---|---|---|---|
| A1 | **STT 入口**：Web 端录音 → `POST /api/v1/audio/transcribe`（loopback + JSON/多部分校验）→ 注入 `supervisor.say`；后端 provider 走 OpenAI 兼容 `/audio/transcriptions`（与 LLM/TTS 同协议，`base_url` 可指本地 whisper.cpp/faster-whisper） | 新路由 + settings `[stt]` 段 + 前端录音按钮 + 契约测试；无 STT 配置时按钮禁用不报错 | 中 |
| A2 | **`--web` 音源唯一化**：把 cpal 播放与 WS 广播改为二选一（`web.audio_output = "browser" \| "server"`），默认 browser；删除双声 | `cli_entry.rs` 装配分支 + `chat.js` epoch 打断一致性测试 | 中 |
| A3 | **真实口型**：浏览器用 **WebAudio AnalyserNode** 对 WS 音频取 RMS（替代音量包络近似），与后端 `RmsMeter` 同参数（50ms 窗/attack 0.03/release 0.10，见 `runtime audio`） | `chat.js` 替换 `audio-volume` 源；wasm 侧接口不变（仍走 `ParamMouthOpenY`） | 小 |
| A4 | **F6 端到端验收**：真 ollama + 真 kokoro + 浏览器；记录「出声/口型/动作/打断」可见证据 | 验收报告落 `docs/verification/`（外部项，不代偿） | 外部 |

### 3.2 波 B：本地 Mod 产品化

| 任务 | 内容 | 交付/验收 | 规模 |
|---|---|---|---|
| B1 | **开箱可用**：`live2d-ai.toml.example` 增补 `[mods.*]` 说明 + 生成默认 `mods.json`（首次启动写 disabled 骨架）；Mod 面板加「一键启用本地 LLM/TTS」 | M1 消除；文档/示例/实态一致性测试 | 小 |
| B2 | **修默认值**：local-llm `model` 改可用缺省（如 `qwen2.5:7b`，或探测到 `/api/tags` 后回填真实模型名） | 单测：探测到模型列表时取首个可用模型 | 小 |
| B3 | **统一 spawn 失败语义**：两 Mod 一致（建议均为「记录 last_error + 继续探活」，不 Failed） | 两 Mod 对照单测 | 小 |
| B4 | **订阅真路由**：`subscriptions` 参与 worker 分发（未订阅不投递） | `mod_registry` 单测 | 小 |
| B5 | **`--chat` 也装配 ModRegistry**（复用同一 HostChannels 装配函数） | 原生链路可用本地 LLM/TTS | 中 |

### 3.3 波 C：UI 形态（**需你先拍板**）

三个方案，成本/风险递增：

**方案 C1（推荐，最低风险）：Web 视觉升级，不换栈。**
把 `daemon` 的视觉语言（磨砂层次、胶囊圆角、主题可切换）用 design token 移植到现有 `style.css`；
成本 ~1–2 天，零新运行时，rust-ratio 不变，不动任何后端契约。
适合：你真正想要的是「好看」，而不是「Flutter」。

**方案 C2：Flutter 壳 + 复用 `/render`（wasm）。**
Rust 后端/协议完全不动（继续 `--web`）；新增独立 `shell/flutter/` 工程，用 **WebView2/WebKit** 只嵌
`http://127.0.0.1:18080/render`，外壳 UI 用 Flutter（acrylic/磨砂），通过 JS bridge 发
`action-state`/`audio-volume`/`stage-config`（协议与现 `app.js` 完全一致）。
成本 ~1–2 周。风险：Windows 需 WebView2 Runtime；本质仍是「Flutter 壳 + 浏览器渲染」。
**注意**：`.dart` 不在 rust-ratio 统计扩展名内（`xtask/src/main.rs`：rs/wgsl/py/ts/js/c/cc/cpp/h/hpp），
门禁不会拦住它，但违反 `core-contracts.md` §1「无第二语言运行时」与 AGENTS「唯一 Rust 主线」，
需你显式豁免并更新 AGENTS/契约。

**方案 C3：Flutter + Rust `l2d` 外部纹理（真原生渲染）。**
Flutter 通过 external texture / platform view 消费 wgpu 渲染结果。**高风险 R&D**：
Windows 需 D3D/共享纹理与 Impeller 互操作（有已知崩溃），Linux 需 GTK/EGL；工作量以「周」计且结果不确定。
**不建议作为第一步**；若坚持，先做 1 周 spike（仅「Flutter 窗口显示一帧 wgpu 纹理」）再决策。

**明确不建议**：C4 用 Live2D Cubism 官方 Native SDK（与项目不用闭源 Cubism SDK 的路线冲突）。

> 若选 C1/C2，现有 `--web` 与 `/render` 全部保留，回滚成本≈0；若选 C3，需先冻结核心契约再动刀。

---

## 4. 验收口径

| 域 | 验收 |
|---|---|
| 工程 | `cargo test --workspace --all-targets` 全绿 + `--doc` + `fmt --check` + `clippy -D warnings` + `rust-ratio ≥95%` |
| 核心链路 | A1–A3 后：文本与语音两条入口都能跑出「LLM 文本 → 浏览器出声 → 口型动 → 动作」；stop 打断 epoch 语义不回归 |
| 本地 Mod | 未装 ollama/kokoroi 时不崩、状态可见；装好后一键启用 → 自动写 settings → 热重载 → 对话生效 |
| 外部项 | F6/S3/S4/真角色卡/第二模型：真机/浏览器/真实端点证据，可见效果为凭 |
| 文档一致性 | README/AGENTS/core-contracts 与实态对齐（Mod 计数、入口、语言栈声明） |

---

## 5. 风险

| 风险 | 应对 |
|---|---|
| STT 引入新依赖/模型体积 | 走 OpenAI 兼容端点，不内置推理；无配置时功能降级为「仅文本」 |
| 双声/口型改动破坏 epoch 打断 | 先补行为测试再改（沿用 `webapp_behavior.rs` 模式） |
| Flutter 方案导致「第二运行时」与 95% Rust 话语冲突 | 决策前先改 AGENTS/契约并加一致性门禁；`.dart` 不计入 ratio 要显式记录 |
| C3 纹理互操作不可行 | 先 spike（1 周，可失败）再决定是否投入 |
| 默认启用 Mod 造成启动副作用（spawn 子进程） | 默认仍 disabled；「一键启用」为显式用户动作 |

---

## 6. 待你拍板的决策点

1. **UI 形态**：C1（Web 视觉升级）/ C2（Flutter 壳 + wasm）/ C3（Flutter 原生纹理 spike）/ 暂不做？
2. **语音输入**：是否本轮就做 STT（A1）？走本地 whisper.cpp 还是 OpenAI 兼容端点？
3. **执行顺序**：先 A（链路收口）→ B（Mod）→ C（UI），还是先 C（你更在意观感）？
4. 若选 C2/C3：是否接受「第二语言运行时」并同步修改 AGENTS.md / core-contracts.md 的表述？
