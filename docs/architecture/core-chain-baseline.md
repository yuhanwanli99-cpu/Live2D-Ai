# 核心链路基线（2026-09-11）

> 本文件是**上层体验升级的底座**说明：核心链路是什么、边界在哪、哪些东西
> 已经不在链路里但代码仍在、以及怎么一键验证它没坏。
>
> 用户原话（2026-09-11）：「我需要的是一个完整核心链路完善的基线作为上层体验
> 相关升级的底座」+「核心链路只有 tts 驱动口型 llm 纯对话 live2d 皮套渲染
> 前端 ui」+「和相关设置等基础设施」。

## 1. 核心链路（唯一的产品链路）

```
文本输入（前端 UI）
  → LLM 纯对话（无工具、无函数调用）
  → TTS 合成语音
  → 驱动口型（音频电平 → ParamMouthOpenY）
  → Live2D 皮套渲染（/render，wasm 渲染面）
  → 前端 UI 呈现（/app/，Flutter Web）
```

配套**基础设施**（属于基线，不是上层体验）：LLM/TTS/模型/外观四个设置分区、
连接状态与诊断、WS 实时通道、音频调度与口型时间线、离线优先构建约束。

## 2. 链路逐环与出处（都已实线接通）

| # | 环节 | 实现位置 |
| --- | --- | --- |
| 1 | 文本 → LLM 请求 | 前端 `chat_controller.send` → `POST /api/v1/chat` |
| 2 | LLM 流式（**无 tools**） | `crates/live2d-ai-runtime/src/llm.rs`；`tools` 字段已删除并有断言守着 |
| 3 | 按句读切句、整句合成后上屏 | `conversation/engine.rs` → `EngineEvent::SentenceVoiced` → WS `text_delta`（`supervisor/handlers.rs`） |
| 4 | TTS 合成 | `live2d-ai-runtime/src/tts.rs`：`POST <tts.base_url>/audio/speech`，`response_format="pcm"` |
| 5 | 音频下发 | WS `audio` 帧：`{audio(base64), epoch, sample_rate, start, end, volume, muted}` |
| 6 | 前端播放 | `shell/flutter/lib/audio/audio_player.dart`（**正在改为 `<audio>` + WAV**，见下） |
| 7 | 口型电平 | 服务端 `data.volume`（RMS，静音时置零但 `volume` 仍是真值）→ `MouthEnvelope` → `LevelTimeline` → `main.dart` `setMouth` |
| 8 | 口型落地 | bridge `mouth` → iframe → `l2d-wasm-demo/src/web/surface.rs` `set_parameter("ParamMouthOpenY", …)` |
| 9 | 皮套渲染 | `/render`（wasm，WebGPU→WebGL 回退） |
| 10 | 前端 UI | `/app/`（Flutter Web，`/` 302 到它） |

## 3. 已经移出链路的东西（**不要**再当成产品功能）

### 3.1 LLM 工具 + 动作系统（2026-09-11 用户裁决，已拆）

- **裁决**：「llm 不暴露任何工具只做对话」+「从前后端抹去相关字段不做实现」。
- **已拆**：`live2d_perform_action` 工具、tool_calls 解析与组装、`tool_adapter`、
  `EngineEvent::ToolAction`、`RenderCommand`、WS `action_state` 投影、
  `/api/v1/commands`（手动触发动作的端点）与其注册表、
  前端 `lib/actions/*`、动作日志 UI、动作设置分区、`sendActionState` 接线。
- **为什么拆**：空 system prompt + 一个 `perform_action` 工具，会让模型有相当
  概率**只调工具、不说话**——产生「正常完成但一个字都没有」的回合，
  正好挡住链路闭环（`scripts/verify_core_chain.py` 当场复现过一次）。

### 3.2 `live2d-ai-core` 的动作子系统：**保留，但不再被驱动**

`crates/live2d-ai-core/src/action/`（`ActionState` / `ActionCommand` /
`ActionEffect` / `rule_fallback`，约 800 行）与 `src/performance/`（六动作参数采样）
**保持原样**。理由：

- 它是自洽且带单测的内部能力，不是"字段"；
- core 的不变量文档（`lib.rs` 第 4 条「单 active 动作」、第 6 条「表演参数曲线」）
  以它为前提，动它会牵动 reducer 的语义论证；
- 本轮目标是把**链路**做干净，不是把每一处未接线能力都删掉——

但它现在**不在产品链路里，而且这一点是结构性的**（2026-09-12，rc.2）：

| 层级 | 状态 | 位置 |
|---|---|---|
| `live2d-ai-mod-director`（动作序列的唯一驱动方） | **已删除** | 归档在分支 `archive/action-layer-p6`；静态 Mod 工厂数由 `main.rs::mod_count_is_three` 守住 |
| `SupervisorHandle::trigger_action` + supervisor 的 `action_rx` select 分支 | **已删除** | 那是**唯一**会把 `RootEvent::Action` 送进 core reducer 的实现 |
| `HostChannels.trigger_action`（`ActionRequest → core` 的 host 映射） | **已删除** | `mod_registry.rs`；`ModServices.action_tx` 仍在（Mod API 契约），但注入的是**固定休眠 sender**：请求只留一行 debug 日志、返回 `false` |
| core 动作子系统（类型 + reducer + capability gate） | **保留、休眠** | `crates/live2d-ai-core/src/action/`、`/performance/` |

**谁休眠、为什么、谁能唤醒**：core 的动作/表演子系统休眠，因为产品路径上不存在动作
（§3.1 的裁决）；唤醒 = 先重新论证本节的三条理由 + `lib.rs` 的两条不变量，
再**显式**恢复一条 host 通道（`RootEvent::Action` 的注入分支）——而不是顺手接回去。

回归钉子两条：
`mod_registry::tests::action_request_is_dormant_not_delivered`（`ActionRequest` 必须
**不被接受**）与 `main.rs::mod_count_is_three`（工厂数不得回到 4）。

「连 `action/` + `performance/` 一起删干净」这条路依然可行，但必须连 `lib.rs` 的两条
不变量一起重新论证——那是另一次结构改动，不在 rc.2 范围内。

### 3.3 渲染面的动作残件：**已删除**（2026-09-12，rc.2 —— 推翻了原判断）

**删了什么**（`crates/l2d-wasm-demo/src/`）：

| 位置 | 内容 |
|---|---|
| `main.rs` | `"action-state"` 消息处理（强度档位 → 倍率、start/end 建/清 `ActiveAction`） |
| `web/surface.rs` | `BridgeState.action`、`ActiveAction`、`ACTION_PARAM_IDS`、`Keyframe`、`CHOREOGRAPHY` 六动作关键帧表、`str_eq`、`choreography_total_ms`、`blend`、`action_frames`、`semantic_frames`、`final_override` 写入/清除块，以及 8 条编舞回归测试 |
| `param_scale.rs`（整文件） | 语义域 → 模型域换算——它的**唯一**调用点就是 `action_frames`，删编舞后即成孤儿 |

`surface.rs` 1721 → **1169** 行（-32%）；归档在分支 `archive/action-layer-p6`。

**原判断是「不删」，这里为什么推翻**（原文的三条理由逐条回应）：

1. 原第 1 条「没有任何东西会再发 `action-state`，所以它惰性、不可能误触发」——
   **理由成立，但结论要反过来**：一个惰性的、约 350 行的关键帧表 + 150 行测试，
   正是最容易被误读成「这里有动作功能」的残留。rc.2 的 DoD #1 要求动作在产品
   路径上**不存在**（不是「存在但没人调」），可读性口径也要求宁删勿加。
2. 原第 2 条「wasm 门控、原生测试覆盖不到」——**仍然成立**，所以本次删除的验收
   照旧是 **wasm 重建 + 肉眼确认**（皮套仍渲染、口型仍动、待机呼吸/眨眼仍在），
   与 §3.4 的保留项一起验，不靠单元测试兜底。
3. 原第 3 条「爆炸半径是舞台不渲染，为零收益不值」——**收益前提变了**：
   它是 `surface.rs` 里最大的一块，且是 M3「大文件止血」与 M5「可读性收尾」
   点名的对象；同时它删掉后 `final_override` 层再无写入方，少一层优先级的
   心智负担。风险用**结构性**手段压低（下条）。

**风险是怎么压的**：待机生命体征 `IdleState` / `idle_enabled` / `apply_idle_life`
**一行未动**（见 §3.4），口型仍写 input 层 `ParamMouthOpenY`；未知消息类型本来就由
`main.rs` 的 `_ => {}` 吞掉，所以即便有旧发送方也不会报错。

**没有本地方案的验收**：本节改动的正确性**只能**由 wasm 重建 + 人在浏览器里看
（`./scripts/ignite.sh` → Windows 打开 `/app/`）。这是 rc.2 点火记录的一部分。

### 3.4 待机生命体征：**必须保留**（不是动作系统）

`surface.rs` 的 `IdleState` / `idle_enabled`（呼吸 / 眨眼 / 微表情，
RM6 待机层）是**另一套机制**，写 input 层，与已删除的动作 `final_override` 层
从来不是同一个东西。前端「外观与互动 → 待机小动作」开关就是它。
**删动作残件时绝不要连带删掉它**（2026-09-12 删编舞时已核对：`apply_idle_life`
的调用点与 `idle_enabled` 判定原样保留）。

### 3.5 字体子集外的字符会破坏「断网可用」（2026-09-11 修 + 加门禁）

**症状**：真机点火时网络里出现 `GET https://fonts.gstatic.com/s/notosanssc/v37/…woff2 [200]`。

**根因**：界面里用了**不在自托管子集**的字符。CanvasKit 取不到设备字体，缺字时引擎
会去 Google 下载回退字体 —— **有网时完全看不出来，断网就是豆腐块**。

**实测违规字符只有两个**（用 fontTools 读子集真实 cmap，22 036 码点，逐字符扫全仓
Dart 字符串字面量）：

| 字符 | 位置 | 何时上屏 |
| --- | --- | --- |
| `▍` U+258D | `ui/message_bubble.dart` 流式光标 | **每次流式回复** |
| `⌘` U+2318 | `app/app_shortcuts.dart` macOS 前缀 | 打开快捷键帮助 |

**修法**：两处都改成**不依赖字形**——流式光标画成竖条
（`SizedBox(width: OpticalNudge.thin, height: Space.s4)` + `ColoredBox`），
macOS 前缀用 ASCII `Cmd`。

**门禁（防复发）**：

```bash
# 换字体后必须重新生成覆盖表（否则门禁会因哈希不符而红）
pip install fonttools brotli          # 仅脚本需要，不是项目依赖
python3 scripts/font_subset_ranges.py  # 生成 assets/fonts/*.ranges.txt
cd shell/flutter && flutter test test/font_subset_test.dart
```

`test/font_subset_test.dart` 做三件事：① 覆盖表与字体文件同源（字节数 + FNV-1a 哈希，
**换了字体没重新生成 → 红**）；② 扫 `lib/**/*.dart` 字符串字面量，出现子集外字符 → 红；
③ 自带小词法器（跳过注释）并有自测。已实测：把 `▍` 放回去即变红。

## 4. 音频路径改造（已完成）

用户报「浏览器禁用声音但依旧有声音传出」，并裁定：
**后端出 WAV 文件、前端 web 播放**（`<audio>` 媒体元素），**不是后端播放**。

两条实测依据：

| 检查 | 结果 |
| --- | --- |
| 后端能否发声 | `audio.backend = "none"`、`available: false`（硬编码 `web_api/app_routes.rs`）——后端无发声能力 |
| 本机 TTS 出不出 mp3 | `mp3` → **HTTP 400**、`wav` → **400**、`pcm` → **200**（24 kHz s16le） |

⇒ mp3 做不到（除非引入编码器，与「运行时依赖 = 0」冲突）；改用 **WAV**
（= 44 字节头 + 裸 PCM，零依赖）。根因是 Web Audio 与媒体元素**走两套权限**：
站点级「声音=阻止」作用在**媒体元素**上，而 Web Audio 归 autoplay 策略管。

实现（已完成）：前端 `lib/audio/sentence_assembler.dart`（纯逻辑：攒句 /
包络 / 串行队列）+ `lib/audio/wav.dart`（WAV 封装 + 按进度取电平），
`lib/audio/audio_player.dart` 只剩 `<audio>`/Blob/URL 胶水与 30 ms 口型轮询。
产物核验：`main.dart.js` 里 `createObjectURL` / `revokeObjectURL` / `audio/wav`
各存在，而 **`AudioContext` / `createBufferSource` 均为 0** —— 旧的 Web Audio
逐片调度路径已彻底移除。

## 5. 一轮链路里三个**已修**的真缺陷（都是跑验收脚本抓到的）

### 5.1 音频帧的句子边界是错的（2026-09-11 修）

**症状**：一句正文（例如「一、二、三。」，实测只有 1 句）在 WS 上产出
**0 个 `start=true`、13 个 `end=true`**。

**根因**：`start` 由 `Broadcaster::audio_epoch_seen` 按 epoch 记账去推
（`previous != token`），而**一句音频本来就会被引擎切成十几块**
（`EngineEvent::AudioChunk` 带 `final_chunk`）；`end` 则是「每次广播调用的末片」。
两者层级不同（轮级 vs 块级），永远配不成句。

**修法**：把句界变成**引擎的契约**——`EngineEvent::AudioChunk` 新增
`first_chunk`，WS 帧据此给出 `start`（首块首片）/`end`（末块末片）/
`sentence_seq`，并删掉那份 epoch 记账状态。

**为什么重要**：前端要把「一句」的 PCM 封成一个 WAV 交给 `<audio>`；句界错了就会
把一句切成十几段播 —— **正是用户讨厌的断续**。

**回归**：`web_api/ws.rs` 的 `sentence_boundaries_are_exactly_one_start_and_one_end`
（断言一整句只有一个 start、一个 end，且可落在不同块里）+ 验收脚本的
「句子边界配对」一跳（服务端句界不对就 FAIL）。

### 5.2 纯空白句会让**整轮**判失败（2026-09-11 修）

**症状**（真机日志原文）：

```text
ERROR TTS 阶段错误: 上游非成功状态 400 Bad Request: {"detail":"input 为空"}
code=tts_upstream_400 stage=tts fatal=true
```

**根因**：分句器把**换行**也算句读（`dialogue::sentence::is_terminator` 含 `'\n'`），
所以模型输出 `"你好！  \n再见！"` 时会切出一个**只含空白**的「句子」。它
`is_empty() == false`（躲过了分句器的非空守卫），却会被 TTS 上游 trim 成空串 →
上游 400 → 而 TTS 错误是 **fatal** → **整轮失败**，用户看到的是「说了半句就没了」。

**修法**：`synthesize_sentence` 对 `text.trim().is_empty()` 的句子**不发 HTTP**，
与「TTS 未配置」走同一条静音句路径（空 `final_chunk` + `SentenceVoiced`）——
分句器「逐字不丢」的硬不变量不动，链路不断。

**回归**：`conversation_engine_tts_flow::whitespace_only_sentence_never_reaches_tts_and_turn_completes`。
该用例的 TTS mock **复刻真实上游**（空 input 回 400），因此去掉修复它会红
（已实测：撤掉修复 → `TurnStatus::Failed`），是真正的钉子而不是装饰。

### 5.3 空末块丢掉 `end` 闸门，导致整句播不出来（2026-09-11 修）

**症状**：某些轮次的句子音频**听不到**（前端等不到句尾闸门）。

**根因**：引擎允许某句的**末块样本数为 0** —— 只要该句样本数正好是
`audio_chunk_samples` 的整数倍就会出现（默认 4 800 样本 = 200 ms @24 kHz，
所以 200/400/600 ms 的句子全都命中）。

- `build_audio_frames` 对空 PCM 直接返回空表（`chunks()` 在空切片上不产出片）；
- `handlers.rs` 又对 `samples.is_empty()` 提前 `return`。

于是那一句**最后一个非空片带 `final_chunk=false`**（`end=false`），而承载
`end=true` 的空末块被丢弃 → **前端永远等不到 `end`**。

**修法**：空末块仍然产出一帧**只有边界、没有音频体**的帧（`audio: ""` +
`end: true` + `volume: 0.0`）；`start`/`end` 本来就是标记，标记不需要载荷。
`handlers.rs` 只跳过**声卡入环**（空样本没有可播的东西），WS 广播照常。
前端把空音频体解析成 0 字节 PCM，照常在 `end=true` 上封口。

**实测频率**：这条路径**相当常见**——`--text "请只说两个字：好的。"`
连续 3 次全部命中（每轮恰好 1 帧空末块句界帧）。也就是说没有这个修复时，
**这些轮次的音频会整句播不出来**。

**回归**：`web_api/ws.rs` 的
`empty_pcm_with_final_chunk_emits_boundary_only_frame`（+ 非末块空片不发帧、
单块成句同时带 start/end、零采样率优先返回空表三条边界），
以及验收脚本的「空末块句界帧」诊断一跳。

## 6. 一键验证基线没坏

### 6.1 命令行（三条，各自独立）

```bash
# 端到端核心链路（只走公开 HTTP/WS，17 跳）
python3 scripts/verify_core_chain.py

# 前端
cd shell/flutter && flutter analyze && flutter test

# 核心层
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio   # 门槛 95%
```

`verify_core_chain.py` 还额外钉住两条**基线不变量**：
- 全程**不得**出现 `action_state` 帧（工具系统不该复活）；
- `audio` 帧必须带 `sample_rate`，且与 TTS 配置一致（不一致会逐片重采样 → 咔哒声）。

### 6.2 真浏览器点火（2026-09-11 实操过一遍，抓到一个真缺陷）

无头实例有几个**必须先处理的坑**，否则会误判成「前端坏了」：

1. **视口默认 0×0**：先 `resize` 到 1280×860，否则 CanvasKit 不渲染、拿不到任何证据；
2. **语义树默认关闭**：a11y 树只有一个 "Enable accessibility" 占位按钮，必须先把它
   `click()`（DOM 事件即可），之后才拿得到按钮/文本节点来驱动界面；
3. **驱动输入框用真实键入/点击**（`fill` 是直接写 DOM `textarea`，会让 Flutter 的
   editing 状态与 controller 失同步——表现为「发送后输入框看起来没清空」，
   **那是探测手段的假象，不是缺陷**：`main.dart:503` 明确 `_input.clear()`，
   实测按回车也不会重发)。

点火要看的**七项证据**（都是 DOM/网络级，不靠"看起来对"）：

| 看什么 | 怎么取 |
| --- | --- |
| 壳起来了 | `flt-glass-pane` 存在、视口非 0 |
| 舞台在渲染 | iframe 内 `<canvas>` 尺寸非 0；点「放大」后父页读数变化（证明渲染面**能回话**） |
| WS 通 | 状态胶囊文本「实时通道正常」 |
| LLM+TTS+句界 | 自建同源 WS 探针记录 `text_delta` / `audio`（`start`/`end`/`sentence_seq`）/ `turn_state` |
| **音频真在播** | 轮询 `document.querySelectorAll('audio')`：`blob: true`、`paused:false`、**`duration` 是实数**（WAV 头非法则为 NaN/0）、`currentTime` 推进 |
| **口型真在动** | 在 iframe（同源）里 `contentWindow.addEventListener('message')` 抓 `mouth` 消息；应有大量 `level > 0` 且峰值明显 |
| **一句一单元** | 高频采样统计**同时播放的元素数**：必须恒为 `≤1`，且时间线呈 `1→0→1→0…`（句数应与模型回复的句子数一致） |
| 断网红线 | `performance.getEntriesByType('resource')` 里**不得有非本机源**（尤其 `fonts.gstatic.com`） |

> 注：无头实例的 rAF 不节流，渲染面自报 FPS 会远高于真机——**别拿它当性能结论**。
> 另外渲染面把正常进度日志也走 `console.error`，所以「控制台有 error」在它这里
> 不代表出错，要按文案判断。
