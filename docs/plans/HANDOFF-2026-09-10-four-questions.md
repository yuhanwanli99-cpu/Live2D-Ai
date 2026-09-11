# 交接审计（2026-09-10）：渲染亮度 / UI 增强 / TTS 速率 / 口型驱动

> 本文回答四个议题，**每条结论都有可复现的实测证据**（命令与数值在文内）。
> 环境：仓库根 `/home/skystar/Live2D-Ai`，`main` @ `1ca1c0fb`（工作区脏）,
> 后端 PID 于 `:18080` 运行中，TTS `:8080` 运行中。测量日期 2026-09-10。

---

## 0. 一句话结论

| 议题 | 结论 |
| --- | --- |
| ① 模型「有点高亮」 | **不是渲染器过曝**。实测本渲染器输出与另两个参考实现（bakeoff ayagami、竞品 mocari）色彩统计几乎相同（平均饱和度 0.075 / 0.077，平均亮度 225.3 / 223.0），也与源纹理一致；皮套本身就是**极浅的白色系**。真正缺的是「曝光/色调」这个**可调参数**——目前一个都没有。 |
| ② UI 增强 | 前端**有两套并存**：`/`（原生 JS，2069 行，7 个设置页签）与 `/app/`（Flutter，2396 行 Dart，只有舞台 + 聊天）。Flutter 版是**功能回退**。且 Flutter 第二运行时**不在 `rust-ratio` 门禁统计范围内**，与 `core-contracts.md:17`「无第二语言运行时」冲突，计划文档要求的显式豁免**未落地**。 |
| ③ TTS 速率 | 实测 RTF ≈ **1.3–1.7×（慢于实时）**，抖动时到 5–6×；且 **`stream=true` 完全没有流式效果**（首字节 ≈ 总耗时）。两层原因：服务端不支持 + **本项目请求体里根本没有 `stream` 字段**。项目自定验收线 TTFA ≤ 1 s，实测 4.3–13.7 s（差一个数量级）。瓶颈主要在 TTS 服务端；项目侧可优化的有 6 处（见 §4.5）。 |
| ④ 口型驱动 | **做了，而且接线正确、时间对齐正确**；但电平是**未放大的线性 RMS**。实测真实语音 RMS 中位数 0.030 / p90 0.118，直接写进 `ParamMouthOpenY`（0..1）→ 嘴巴只张开 10% 左右，肉眼近乎不动。**需要约 5× 增益**（见 §4.6 附图）。 |

---

## 1. 版本与仓库治理（可立刻执行）

### 1.1 当前状态

```
分支     main，领先 origin/main 105 个提交（未推送）
工作区   39 项未提交 = 26 已修改 + 13 未跟踪
工作区版本 0.3.0（Cargo.toml:24，全 crate 继承）
最近 tag  v0.3.0（无本轮「核心链路闭环」的 tag）
Flutter   shell/flutter/pubspec.yaml = 1.0.0+1（与 workspace 0.3.0 不一致）
```

### 1.2 提交前必须先处理的三件事

> **2026-09-10 处置状态**：#1 与 #3 已修（`.gitignore` 加 `/mods.json`；
> `live2d-ai.toml.example` 与 `docs/architecture/cosyvoice3-tts-integration.md`
> 的 `wav` → `pcm` 并补上实测更正）。#2（提交 `shell/`）随本轮本地提交一并落地。

| # | 问题 | 处置 |
| --- | --- | --- |
| 1 | **`mods.json` 是运行时产物，但没被 ignore**（`git check-ignore mods.json` 无输出）→ 会被误提交，且内含本机路径/开关状态 | ✅ 已加 `/mods.json` 到 `.gitignore` |
| 2 | **`shell/` 整个 Flutter 前端未跟踪**（27 个文件 / 200 KB；`build/`、`.dart_tool/` 已被嵌套 `.gitignore` 排除，不会入库 122 MB） | ✅ 本轮一并提交 |
| 3 | **`live2d-ai.toml.example` 推荐的 `response_format = "wav"` 会被真实服务端拒绝**（见 §4.3，实测 400） | ✅ 已改回 `"pcm"`，接入文档同步更正 |

### 1.3 文档卫生

- 根目录 `plan.md` 是 **piagent 任务运行的临时草稿**（引用已结束的 run `20260909-124120-a62o`），却留在根目录且已入库 → 建议移到 `docs/plans/` 或删除。
- 根目录 `PLAN.md` / `PROGRESS.md` 顶部已标「已归档」，但文件名看起来像当前文档 → 建议加 `-archive` 后缀或移入 `docs/archive/`。
- 本轮的 `docs/plans/HANDOFF-2026-09-10.md` 里写「37 个文件未提交」，实际是 39 项（26 M + 13 ??）→ 以 `git status` 为准。

### 1.4 建议的收口顺序

```bash
# 1) 先让门禁全绿（交接文档声称全绿，但工作区已改动，需复验）
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets && cargo test --doc --workspace
cargo run -p xtask -- rust-ratio        # 本机实测 95.4692% PASS

# 2) 修 .gitignore（mods.json）与 toml.example（response_format）
# 3) 分批提交：Rust 核心链路 / Flutter 前端 / 文档 三笔，便于回滚
# 4) 打 tag（建议 v0.3.1 或 v0.4.0-rc1），并记录 Flutter 引入的治理豁免
```

---

## 2. 议题①：模型「有点高亮」

### 2.1 渲染器的色彩路径（这是「核心渲染器相关参数」的答案）

| 参数 | 位置 | 当前值 | 含义 |
| --- | --- | --- | --- |
| `colorspace` | `crates/l2d/src/renderer/model_core.rs:46` | `RenderColorspace::SRgb`（**硬编码，无配置入口**） | 片元着色器手动做 linear→sRGB 编码；混合模式在**编码后的 unorm 空间**做 |
| surface 格式 | `crates/live2d-ai-desktop/src/app/capability.rs:261` | 优先 `Bgra8Unorm` / `Rgba8Unorm`（**非 sRGB**） | 与上一行配套，避免二次编码 |
| surface 格式（WASM） | `crates/l2d-wasm-demo/src/web/surface.rs:100-111` | 优先 `Rgba8Unorm` / `Bgra8Unorm` | 同上 |
| alpha 合成模式 | `surface.rs:112-118` | 优先 `PreMultiplied` | 上游输出即预乘 alpha |
| 中间 part 缓冲 | ayagami `renderer.rs:1314/1350/1428` | `Bgra8Unorm` | 混合运算在 8-bit 编码空间 |
| 模型纹理格式 | ayagami `texture.rs:126` | `Rgba8UnormSrgb` | 采样时硬件解码到线性 |

> `SRgb` 是**有意选择**，不是 bug：`docs/verification/rust-bakeoff-decision.md:18`
> 记录的理由是「SRgb 让混合在 unorm gamma 空间进行，近似 Cubism 行为」。
> 上游自带 demo 走的是相反组合（`Linear` + sRGB surface），本项目选择前者。

### 2.2 实测：渲染器没有过曝

对三方产物做同样的色彩统计（角色像素；mocari 用背景色距离分割，其余用 alpha）：

| 来源 | 平均饱和度 | p95 饱和度 | 平均亮度 |
| --- | --- | --- | --- |
| 本渲染器 `.tmp-audit/bai_frame.png`（1024²） | **0.075** | 0.219 | **225.3** |
| bakeoff 参考 `verification/rust-bakeoff/ayagami-bai.png` | 0.075 | 0.216 | 225.3 |
| 竞品参考 `verification/rust-bakeoff/mocari-bai.png` | 0.077 | 0.225 | 223.0 |
| **源纹理** `bai.16384/texture_00_4096.png`（alpha>250） | 0.080 | 0.237 | — |

三者与源纹理几乎一致（差 ≤1%）。**渲染器忠实还原了纹理，「高亮」来自皮套本身**
（白发 + 白外套 + 极低饱和的浅蓝），不是色彩管线错误。

复现：

```bash
cargo run -p l2d --example render_model -- assets/models/bai/runtime/bai.model3.json out.png 1024 1024
```

### 2.3 想要「调暗」时该动哪里

现在**没有任何**亮度/对比度/色调/曝光参数，两级都没有：

- 渲染器：`ModelRendererCore` 只暴露 transform / viewport / pose，无色调入口；
- API：`/api/v1/models/{id}/display` 只有 `width/height/scale/background_color/background_opacity`（`models_routes/registry.rs:200-224`）。

三条可落地路径，按成本排序：

1. **零代码（推荐先试）**：给宿主 canvas 加 CSS 滤镜。
   `/render` 页在 `web_api/wasm_assets.rs::render_html` 内联样式里；Flutter 侧在
   `shell/flutter/lib/live2d/live2d_stage.dart:132` 的 `ColoredBox` 附近。
   `filter: brightness(.92) contrast(1.06) saturate(1.08)` 即可，回滚成本≈0。
2. **低成本（推荐做）**：把「曝光/色调」做成显示配置项并下发到渲染面——
   在 `ModelRendererCore` 增一个 `tint: [f32;4]`，渲染后加一个全屏 pass（或在
   `ArtMeshUniform.multiply_color` 上叠乘）。同时补进 `DisplayStage` DTO 与设置 UI，
   这样议题②的「外观」页签也就有了真内容。
3. **不做**：改 `RenderColorspace` 为 `Linear`。会同时改变全部混合模式语义，
   与 §2.1 的选型理由冲突，且需要重做全部视觉回归基线。

---

## 3. 议题②：UI 增强

### 3.1 现状：两套前端并存

| | `/`（原生 JS） | `/app/`（Flutter Web） |
| --- | --- | --- |
| 实现 | `web_api/index.html` 260 行 + `app.js` 897 + `chat.js` 600 + `style.css` 312 | `shell/flutter/` 2396 行 Dart |
| 页签 | 角色 / 外观 / AI 与语音 / 动作 / 模型 / 开发 / Mods（`index.html:104-214`） | **无**，只有舞台 + 聊天面板 + WS 徽标（`main.dart:96-152`） |
| 设置 | 完整（含 key 绑定、persona 五字段、日志、Mods） | 无 |
| 测试 | `tests/webapp_behavior.rs` 行为基线（Rust 驱动 node 跑真实 `app.js`） | `test/audio_schedule_test.dart` + `live2d_bridge_test.dart` |
| 托管 | `index_html.rs`（`include_str!`） | `flutter_app.rs`（读盘 + SPA fallback） |

Flutter 版目前**只有聊天与舞台**，把上面的设置能力全部丢了。

### 3.2 治理冲突（必须先拍板）

- `docs/architecture/core-contracts.md:17`：「单一 Rust workspace（`crates/*`）实现，**无第二语言运行时**」。
- `AGENTS.md`：「唯一实现主线 = Rust workspace」「主仓库保持 95-99% Rust」。
- `xtask/src/main.rs:26-27`：`rust-ratio` 只统计 `rs/wgsl/py/ts/js/c/cc/cpp/h/hpp`——
  **`.dart` 不在其中**，2396 行 Dart 对门禁完全不可见（实测 95.4692% PASS）。
- `docs/plans/web-flutter-migration-and-loop-closure-plan.md` §3.3 明确：方案 C1（Web 视觉升级、
  不换栈）为推荐；C2（Flutter 壳）**「需你显式豁免并更新 AGENTS/契约」**。
  实际执行了 C2，但**豁免与契约更新都未落地**。

→ 需要二选一：**补豁免**（更新 `core-contracts.md` + `AGENTS.md`，并把 `.dart` 纳入
`rust-ratio` 分母或单独设一道 Dart 体积门禁），或**回到 C1**。

### 3.3 UI 增强的具体清单（按性价比）

| 优先级 | 项 | 落点 |
| --- | --- | --- |
| P0 | Flutter 壳补齐设置面板（至少 AI/TTS 端点 + 音色 + persona） | 复用现有 `/api/v1/settings` GET/PATCH 与 `settings_routes/`，`app.js` 的 `buildPatch`/`renderAll` 语义已有行为基线可对照 |
| P0 | 「外观」页签真内容：§2.3 的曝光/色调 + 背景色/透明度 + 缩放 | `DisplayStage` DTO 已有 `background_*`/`scale`，只需加色调字段并打通到渲染面 |
| P1 | 模型导入/激活入口（`/api/v1/models*` 已实现，Flutter 侧无入口） | `models_routes/` |
| P1 | 命令面板 / 日志查看（`/api/v1/commands`、`/api/v1/logs` 已实现） | `commands_routes.rs` / `log_routes.rs` |
| P2 | 口型灵敏度滑杆（§4 的增益参数） | 新增 settings 字段 + `mouth` 消息透传 |
| P2 | 移除或明确降级旧的 `/` JS 前端，避免两套并存产生行为分歧 | `index_html.rs` / `index.html` |

---

## 4. 议题③：TTS 合成慢——速率因素

### 4.1 实测基线（真实端点 `127.0.0.1:8080`，voice=`skystar`）

非流式 `pcm`：

| 文本 | 字数 | 音频时长 | 总耗时 | RTF |
| --- | --- | --- | --- | --- |
| `你好呀。` | 4 | 2.04 s | 9.82 s | **4.82×** |
| `你好，今天天气真不错，我们出去走走吧。` | 19 | 4.72 s | 5.71 s | **1.21×** |
| 44 字长句 | 44 | 10.24 s | 13.66 s | **1.33×** |

同一句（12 字）连测三次：4.73 / 5.51 / 5.58 s，音频 3.36 s → RTF **1.41 / 1.64 / 1.66×**。
更短句（`你好`，1.28 s 音频）的独立复测抖动更大：RTF **1.33 / 4.32 / 4.26×**
（wall 1.71 / 5.55 / 5.47 s）。**run-to-run 差异可达 4×**，归因未能确定（容器内
`nvidia-smi` 被拒，无法区分 GPU 争用与模型冷启动）。

**关键：RTF 普遍 > 1，即合成比播放还慢。** 客户端 `schedule.dart` 的注释早已记录同一现象：
「一句 2 s 音频在约 1 ms 内突发到齐 → 3.7~4.3 s 空档」「TTS 供数慢于实时」。

### 4.2 `stream=true` 是假的（最大的单项浪费）

| 文本 | 首音频字节 | 总耗时 |
| --- | --- | --- |
| 19 字，`stream=true` | 4.334 s | 4.340 s |
| 44 字，`stream=true` | 9.809 s | 13.308 s |
| 19 字，无 `stream` | 5.707 s | 5.713 s |

首字节 ≈ 总耗时 → **服务端把整段音频合成完才发第一个字节**，`stream` 参数无效果。
后果：本项目「流式分片」只是在**已完成的整段**上切片，无法降低首字延迟。

两层原因，都要修：

1. **服务端不支持**：直连实测加了 `stream:true` 也是单次突发返回（原始 socket 观测：
   头部 3 ms 到达，全部 body 在 2.686 s / 4.238 s 一次性到达）。
2. **本项目压根没请求流式**：请求体只有 `input/model/voice/response_format`
   （`crates/live2d-ai-runtime/src/tts.rs:21-29`），**没有 `stream` 字段**。
   （LLM 侧是有的：`runtime/src/llm.rs:112-134`。）

> ⚠️ 测量口径：`curl -w '%{time_starttransfer}'` 报的是**响应头**到达时间（实测 3 ms），
> 不是首个音频字节。本文 §4.1 用的是原始 socket 首字节时间，不要用 curl 那个数字评估 TTFA。

### 4.3 服务端能力实测（与项目文档不一致）

```
GET  /openapi.json  ->  /health, /v1/models, /v1/audio/voices, /v1/audio/speech, /
GET  /v1/audio/voices -> {"voices":[{"id":"skystar","mode":"zero_shot"},
                                    {"id":"skystar_plain","mode":"zero_shot"}]}
response_format: pcm -> 200 ;  wav/mp3/opus/flac -> 400
```

- 服务端**只支持 `response_format=pcm`**。而 `docs/architecture/cosyvoice3-tts-integration.md`
  §1/§2.3 与本轮修改过的 `live2d-ai.toml.example` 都推荐/写明 `"wav"` → **照做会 400 失败**。
- 文档里的音色注册端点是 `POST /v1/voices/register`，实际可用的是 `GET /v1/audio/voices`
  （本次未验证注册路径），**接入文档需按真实服务重写**。
- 音色模式是 `zero_shot`（零样本克隆）——每次合成都要跑完整参考音频编码，这是慢的结构性原因。

### 4.4 速率因素清单（按影响排序）

> 项目自定的验收线（`docs/verification/acceptance-metrics-report.md:222-223`）：
> **TTFT ≤ 800 ms(P50)、TTFA(首个音频字节) ≤ 1 s、端到端 ≤ 2 s(P50)**。
> 实测 TTFA 是 **4.3–13.7 s**，离验收线差一个数量级。

| # | 因素 | 类型 | 位置 / 旋钮 | 当前值 |
| --- | --- | --- | --- | --- |
| 1 | **服务端不流式**：整句合成完才返回 | 硬约束（服务端） | 换支持真流式的服务，或换更小模型 | — |
| 2 | **模型推理本身 RTF ≈ 1.3–1.7×**（抖动时到 5–6×） | 硬约束（算力/模型） | GPU、模型规格（`Fun-CosyVoice3-0.5B`） | 中位 ≈1.4× |
| 2b | **`zero_shot` 克隆模式**每句重跑参考音频编码 | 可调（服务端） | 换预置音色 / 缓存 speaker embedding | `mode: zero_shot` |
| 3 | **本项目不请求流式**：请求体无 `stream` 字段 | **可调（本项目）** | `runtime/src/tts.rs:21-29` | 无该字段 |
| 4 | **LLM TTFT + 首句生成**（含每次新建 TLS） | 硬约束（云），部分可调 | 模型/provider、system_prompt、`max_history_pairs` | 到 DeepSeek 首响应头 156 ms（其中 TLS 77 ms） |
| 5 | **HTTP 连接复用被禁用** `pool_max_idle_per_host(0)` | **可调（本项目）** | `runtime/src/lib.rs:131` | 0（每轮握手 ≈77 ms 到 DeepSeek） |
| 6 | **分句滞后**：终止符后还要再来一个字符才成句；>48 字按位置硬切 | **可调（本项目）** | `runtime/src/dialogue/sentence.rs:152-155`、`:46` | 上限 48 字 |
| 7 | **TTS 句子队列串行**：`sentence_seq` 到达序逐句串行合成 | **可调（本项目）** | `conversation/worker.rs:62-90`，并发度=1 | 并发 1，队列 4 |
| 8 | **`audio_chunk_samples = 4800`（200 ms）**：首片要攒满才发 | 可调（本项目） | `conversation/mod.rs:124`；**未在 toml 暴露** | 4800 @24 k |
| 9 | **WAV 路径必须缓冲整段**才能解析 RIFF | 可调 | `conversation/worker.rs:134-142,216-240` | 当前走 pcm，不触发 |
| 10 | **整轮串行**：新消息要等本轮 LLM+TTS 排空 | 可调（本项目） | `supervisor.rs:404`（容量 1 通道） | 容量 1 |
| 11 | **WS 每 200 ms 片再切成 10×20 ms JSON 帧**并逐连接重序列化 | 可调（CPU/抖动） | `web_api/ws.rs:443-478`、`ws/connection.rs:199-216` | 10 帧/片/连接 |
| 12 | **WS 每连接 256 槽队列，满则踢订阅者** | 可调（可靠性） | `web_api/ws.rs:156-174,307` | 256（≈5.12 s） |
| 13 | **WS socket 未设 TCP_NODELAY**（Nagle 开） | 可调（局域网才明显） | `web_api/ws.rs:296-304`，全仓库无 `set_nodelay` | 开（loopback ≈0） |
| 14 | **旧 JS 前端 `/` 音频调度有缺陷**（见 §4.6） | 可调（bug） | `web_api/chat.js:268-285,352` | — |
| 15 | 浏览器 WebAudio 输出延迟 | 硬约束 | `audio_player.dart:193-211` | 本次未测 |
| 16 | 客户端排片提前量 | 可调（本项目） | `audio/schedule.dart:48-55` | **0 ms（不额外加延迟）** |
| 17 | HTTP 无超时（stall 会挂住整轮） | 可调 | `runtime/src/lib.rs:125-132` | 无 |
| 18 | PCM 解码 + f32→i16 + base64 | 硬约束，可忽略 | `audio/decoder.rs:48-64`、`ws.rs:509-529` | — |
| 19 | 冷启动 / 抖动：run-to-run 差异可达 4× | 混合 | 保持模型常驻 | 1.7 s → 17.8 s |

### 4.5 建议的优化顺序

1. **先修文档/配置**（§1.2 #3）：`toml.example` 的 `wav` → `pcm`，否则新用户根本起不来。
2. **让 TTS 真的流式，或换更快的引擎**（数量级收益）。这是唯一能把 TTFA 从数秒降到
   亚秒的手段——在此之前，客户端所有调度优化都只是「把整段音频排好队慢慢播」。
   验证必须用**原始 socket 首字节时间**，不能用 `curl time_starttransfer`。
3. **本项目可独立做的（低风险、有确定收益）**：
   - `conversation/mod.rs:124`：`DEFAULT_AUDIO_CHUNK_SAMPLES` 4800 → 1200（50 ms）；
   - 新增 `[conversation]` 段把 chunk / 队列容量 / `sentence_max_chars` 暴露到
     `live2d-ai.toml`（现在是 `settings.rs:317-325` 里 `..Default::default()` 的代码常量）；
   - `sentence_max_chars` 48 → 16：后端不流式时，**每请求时长直接决定 TTFA**，小句收益最大；
   - `runtime/src/lib.rs:131`：恢复连接池（用 per-test client 解决 mock 竞态，别全局禁用）。
4. **修旧 JS 前端 `/` 的音频缺陷**（Flutter 侧已正确，照它改即可）。
5. **别动** HTTP 连接复用那处本身以外的 mock 相关假设（有间歇性红的回归钉子，
   见 `lib.rs:118-123` 的注释）。

### 4.6 附带发现：旧 JS 前端 `/` 的两个音频缺陷（Flutter 侧无此问题）

| # | 缺陷 | 位置 | 后果 |
| --- | --- | --- | --- |
| 1 | 下一片只在上一片的 `onended` 里排 —— 每片都吃一个事件循环空档，且时间轴锚 `now` 后追不回来 | `web_api/chat.js:268-285` | 累积性卡顿；应一次排 2–3 片提前量 |
| 2 | `turn_state != running` 就 `interruptAudio()` | `web_api/chat.js:352` | `turn_state{completed}` 是**生成**结束（浏览器模式下无 cpal），不是**播放**结束 → **最后一句被切掉**。应按 epoch / `voice_ended` 打断 |

---

## 5. 议题④：TTS 口型驱动

### 5.1 结论

**做了，且接线与时间对齐都是正确的。** 问题是**驱动幅度太小**，不是「没做」。
`ParamMouthOpenY` 在这个皮套上确实驱动嘴部形变（实测见 §5.6）。

### 5.2 完整链路（含每一跳的速率与参数）

```
TTS PCM (s16le 24k mono)
 └─[WS] 每 20 ms 一片，base64 在 data.audio            ws.rs:410-470（slice_ms=20）
     ├─ 服务端同时算 volume=RMS∈[0,1] 一起下发         ws.rs:461,485-500
     └─ ⚠️ Flutter 客户端**不解析 volume 字段**，自己重算  ws_client.dart:299-305
 └─[客户端] MouthEnvelope：窗 50 ms / attack 0.03s / release 0.10s   audio_player.dart:35-101
 └─[客户端] LevelTimeline：延迟到「真正出声时刻」才释放   schedule.dart:70-110
 └─[客户端] _audio.levels.listen → setMouth(level)        main.dart:65-66
 └─[桥] Live2DBridge.sendMouth，节流 ≤30 Hz（33 ms）        live2d_bridge.dart:39-40,110-124
 └─[postMessage] {version:1,type:"mouth",payload:{level}} live2d_bridge.dart:154
 └─[渲染面] mouth → audio-volume → bridge.volume           surface.rs（消息分发）
 └─[每帧] volume_display = volume.max(prev*exp(-dt/160ms)) surface.rs:1053-1063（TAU_MS=160）
 └─[每帧] if lip_sync && v>0.01 → set_parameter("ParamMouthOpenY", v)  surface.rs:1082-1084
```

`lip_sync` 默认 `true`（`surface.rs:259`），客户端从不覆盖 → 开关不是问题。

### 5.3 根因：未放大的线性 RMS

整条链路**没有任何增益**：

- 服务端 `rms_s16le_mono` 直接 `sqrt(sum/n)` → `[0,1]`（`ws.rs:485-500`）；
- 客户端 `MouthEnvelope` 同样直接 `sqrt(_sumSquares/count)`（`audio_player.dart:85-86`）；
- 渲染面直接写参，只有 `> 0.01` 的门限（`surface.rs:1083`）。

**实测真实 TTS 语音（19 字、4.72 s、24 kHz）的 50 ms 窗 RMS：**

```
p50 = 0.0304    p90 = 0.1177    p99 = 0.2202    max = 0.2824
峰值样本 = 0.5074（TTS 自身只有约 -6 dBFS）
```

→ `ParamMouthOpenY`（0..1 量程）**典型只走到 0.03–0.12**，即量程的 3%–12%。
要达到「明显张嘴」（约 0.6）需要 **≈5× 增益**。

### 5.4 实测形变量（`ParamMouthOpenY` 扫描，1024² 离屏）

| 参数值 | 变化像素数 | 相对满幅 |
| --- | --- | --- |
| 0.12（≈真实语音 p90） | 135 | **27%** |
| 0.30 | 237 | 48% |
| 0.60 | 363 | 74% |
| 1.00 | 492 | 100% |

变化区域 `x∈[471,553] y∈[296,330]`，即嘴部。参数**有效**，只是在 0.12 时几乎看不出来。

### 5.5 次要放大因素

1. `v > 0.01` 门限：低于约 -40 dBFS 直接归零，轻声段落嘴完全闭合。
2. 衰减时间常数 `TAU_MS = 160 ms`（`surface.rs:38`）：偏慢，快速连续音节会被抹平。
3. 桥节流 30 Hz（`live2d_bridge.dart:39`）：够用，但叠加 20 ms 客户端节拍后
   有效更新率约 30 Hz，与 60 Hz 渲染不同步。
4. 服务端已经算好的 `volume` 被客户端丢弃 → **同一条数据算了两遍**，且两处参数不一致
   （服务端按 20 ms 片算，客户端按 50 ms 窗 + attack/release 算）。
   证据：`ws_client.dart` 的 `AudioEvent` 只有 `epoch/pcm/sampleRate/start/end`
   （`ws_client.dart:81-97`），`grep -rn volume shell/flutter/lib/` **无任何命中**。

### 5.6 证据图

`docs/verification/assets/mouth_compare.png`（左 = 0.12，右 = 1.0）：

![口型对比](../verification/assets/mouth_compare.png)

### 5.7 建议

1. **P0：加增益**。最小改动是在渲染面把 `volume_display` 乘一个系数再写参数
   （`surface.rs:1083` 附近），并把该系数做成 settings 字段（默认 4–6）。
   更稳的做法是**对数/自适应归一化**：`level = clamp((rms_dB + 40) / 40, 0, 1)`，
   这样不依赖 TTS 输出电平，换音色也不会失效。
2. **P1：门限与衰减**：门限从 0.01 降到 0.003，`TAU_MS` 从 160 降到 ~90。
3. **P1：单一数据源**：要么客户端用服务端下发的 `volume`，要么服务端停发 `volume`，
   别两处各算一遍。
4. **P2：按 RMS 之外的特征驱动**（如低频能量），口型会更贴元音开合。

---

## 6. 复现命令汇总

```bash
# 环境
pgrep -af live2d-ai-desktop ; ss -ltnp | grep -E '18080|8080'

# ① 渲染一帧并比对（本渲染器 vs 参考图 vs 源纹理，用任意色彩统计脚本）
cargo run -p l2d --example render_model -- assets/models/bai/runtime/bai.model3.json out.png 1024 1024

# ③ TTS 速率（注意：仓库根外无共享 /tmp，写进工作区再删）
printf '%s' '{"input":"你好，今天天气真不错，我们出去走走吧。","voice":"skystar","response_format":"pcm"}' > req.json
curl -s -o out_pcm.bin -w 'ttfb=%{time_starttransfer}s total=%{time_total}s bytes=%{size_download}\n' \
  -X POST http://127.0.0.1:8080/v1/audio/speech -H 'Content-Type: application/json' -d @req.json
# 音频时长 = bytes / 2 / 24000；RTF = total / 时长

# ③ 服务端能力
curl -s http://127.0.0.1:8080/openapi.json
curl -s http://127.0.0.1:8080/v1/audio/voices

# ④ RMS（python3 + numpy）
python3 -c "import numpy as np;b=np.fromfile('out_pcm.bin',dtype='<i2')/32768.0;\
w=1200;print(np.percentile([np.sqrt((b[i:i+w]**2).mean()) for i in range(0,len(b)-w,480)],[50,90,99]))"

# ④ 口型形变扫描：临时 example（跑完即删，勿提交）
#   core().set_parameter("ParamMouthOpenY", v) → render_frame() → save_png()
```

---

## 7. 本轮未做 / 未验证

- 未验证音色注册端点（`docs` 写的 `POST /v1/voices/register` 与实测 `/v1/audio/voices` 不符）。
- 未验证服务端 GPU 占用（容器内 `nvidia-smi` 被拒）——RTF 的硬件归因未直接证据。
- 未做 Flutter 侧端到端重新验收（本轮为只读审计 + 离屏渲染）。
- 未执行任何 git 提交或推送。
