# 音频路径改造：后端出 WAV，前端 `<audio>` 播放

> 状态：**已定方案，待实施**（2026-09-11 用户裁决）。
> 前置于本改造：动作/工具系统拆除（前端 + Rust 两个改动）——它们与本改造
> **改同一批文件**（后端 `supervisor/` 音频发射、前端 `main.dart` 接线），
> 必须先在同一个工作树上落地，再动音频，否则会互相踩。

## 一、为什么改（用户报的现象）

用户：「我这浏览器禁用声音但是依旧有声音传出。」

## 二、两条实测结论（不是推断）

| 检查 | 结果 | 出处 |
| --- | --- | --- |
| 后端能否发声 | `audio.backend = "none"`、`available: false` | `GET /api/v1/app/status`；硬编码 `web_api/app_routes.rs:214` |
| 本地 TTS 出不出 mp3 | `mp3` → **HTTP 400**；`wav` → **400**；`pcm` → **200**（24 kHz s16le） | 直接 POST `127.0.0.1:8080/v1/audio/speech` 实测 |

⇒ ①**声音只可能来自浏览器**，不是后端播放；②**「推 mp3」当前做不到**，
要么后端加编码器（与「运行时依赖 = 0 / 离线优先」红线冲突），要么用 WAV
（= 44 字节头 + 裸 PCM，零依赖可自造）。

## 三、根因判断

Web Audio 与媒体元素**走两套权限**：

- Web Audio 归 Chromium 的 **autoplay 策略**管——有用户手势后 `resume()` 即可
  运行（[Chromium autoplay 文档](https://chromium.googlesource.com/experimental/website/+show/bd5b5718bc173fb8282743f5003e4f13f5a4df4a/site/audio-video/autoplay/index.md)）；
- 「站点声音 = 阻止」这类设置作用在**媒体元素**上。

现在用的是 `AudioContext` 逐片调度，所以站点级静音很可能拦不住。

## 四、现状播放链路（逐环已验，含出处）

```
① LLM 纯对话（流式）→ 按句读切句
② 每句合成完毕才上屏：EngineEvent::SentenceVoiced → WS text_delta
   （supervisor/handlers.rs:54-70 = 「一句一单元」）
③ tts.rs:44-52  POST <tts.base_url>/audio/speech  { response_format: "pcm" }
④ 后端切 960 字节（20 ms）→ base64 → WS audio 帧
   data: { audio, epoch, sample_rate, start, end, volume, muted }
⑤ ws_frame.dart:314-331 → AudioEvent
⑥ chat_controller._onAudio → audio.feed(PcmFrame)
⑦ **前端 Web Audio**：AudioContext(24 kHz) → 每片 BufferSource
   → GainNode（静音=增益 0）→ destination
⑧ 口型：同一份 PCM 的 RMS → MouthEnvelope → LevelTimeline
   → main.dart:369 stage.setMouth → bridge「mouth」→ iframe
   → surface.rs:1159 set_parameter("ParamMouthOpenY", mouth)
```

## 五、目标链路

```
① 同前
② 后端把**整句** PCM 包成 WAV（44 字节头 + 裸 s16le，**不转格式**）
③ 经 WS 推给前端（一句一帧），同时带上该句的**电平包络**
④ 前端 Blob([wav], {type:'audio/wav'}) → URL.createObjectURL → <audio> 播放
⑤ 口型：按**播放进度** audio.currentTime 查包络 → levelAtSec → setMouth
```

### 为什么用 WS 推、不另开端点

用户要的是「后端推文件」。WS 推送：

- 无需新端点、无需服务端存储 WAV、无需 TTL 清理与 Range 处理；
- **总流量不变**——今天就已在传同样多的 PCM 字节（只是切成 20 ms 帧），
  改成一句一帧只是重新分帧；
- 复用已跑通的 start/end 分句边界。

### 已接受的代价

- **首声延迟变大**：必须等整句合成完才播。这正是「一句一单元」契约明文允许的
  ——AGENTS.md：「宁可晚开口，不可中途卡顿或重复」。

## 六、改动清单

**后端（Rust）**
1. 音频发射：把「按 20 ms 切片」改为「按句攒齐 → 包 WAV → 一句一帧」。
   帧内新增 `wav`（base64 整句 WAV）与 `levels`（`[{offsetSec, level}]` 包络），
   保留 `epoch` / `start` / `end` / `muted` 供前端做轮次闸门。
2. 保留 `LIVE2D_AI_MUTE_AUDIO=1` 语义：**零 PCM + 真实 volume**（音量与口型正交）。
3. WAV 头生成抽成**纯函数**并单测（44 字节逐字段；写错一个字节浏览器**静默不播**）。
4. 注意：`audio.backend = "none"` 下本地 ring/声卡本就不参与，
   改发射路径**不应**触碰双闩锁（`TurnCompleted` / Drained）语义——
   动到判定就必须补回归。

**前端（Dart）**
5. `audio_player.dart`：把 Web Audio 调度换成 `<audio src=blob:>`；
   一句播完 `revokeObjectURL`。
6. 口型：用 `audio.currentTime` 查包络（`levelAtSec`），**不再**用 Web Audio 时间轴。
7. 保留既有对外契约：`levels` 流、`muted` / `volume`、`unlock()`（autoplay 手势）。

## 七、已完成的准备（可与拆除并行、不冲突）

- `shell/flutter/lib/audio/wav.dart`：`wavFromPcm16()`（纯 Dart，零依赖）
  + `levelAtSec()`（按进度取电平，保持而非插值）。
- `shell/flutter/test/wav_test.dart`：**16 条**回归（头部逐字段、载荷逐字节、
  空句合法、48 kHz 立体声、包络边界与「超出末尾不突然归零」）。
- `scripts/verify_core_chain.py`：核心链路**端到端验收**（14 跳，只走公开
  HTTP/WS），已用它当场抓到一次真实的「空回合」（零文字零音频但 completed）。
  本改造落地后，需在该脚本里把「音频帧带 sample_rate」一类的判据
  更新为「帧带整句 WAV + 包络」。

## 八、验收

1. `cd shell/flutter && flutter analyze && flutter test` 全绿；
2. `cargo test --workspace --all-targets` + `fmt` + `clippy -D warnings` + `rust-ratio ≥95%`；
3. `python3 scripts/verify_core_chain.py` 14 跳全过；
4. **人工**：浏览器「站点声音 = 阻止」后应真的无声（这是本次改造的**唯一目的**）；
   应用内静音按钮仍应只关声音、不关口型。
