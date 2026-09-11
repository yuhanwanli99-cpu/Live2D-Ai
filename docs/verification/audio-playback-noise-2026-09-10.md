# 浏览器音频「很吵的杂音」根因与修复（2026-09-10）

> 症状（用户原话）：先「音频播放是杂音很吵 无任何音频输出」，修完 LLM 无返回后仍「还是吵」。
> 范围：Flutter Web 前端 + WS 音频帧协议 + TTS 分片调度。本文是完整证据链与修复记录。

## 一句话结论

Flutter 侧把 WS 帧里的 `audio.start`（服务端**每个 200 ms TTS 分片的首帧**）当成了
「播放时间轴重置指令」：每片都把时间轴拽回 `now`。而 TTS 分片是 **1 ms 级突发到达**，
拖回时上一片通常**仍在播放**——于是同一份音频被多路同时播放。WebAudio 对同时发声的音源
**逐样本相加**，结果是削波失真（听感即「很吵」），而不是「更响」。

真实客户端实测：重叠 **529 对 → 0**，最大同时发声 **17 → 1**，累计重叠 **9.79 s → 0.00 s**。

## 证据链

### 1. 先排除「源不干净」：TTS 服务与 WS 字节都是干净 PCM

- CosyVoice 3 直连返回：`content-type: audio/pcm`、`x-sample-rate: 24000`、3.60 s、
  峰值 0.3882、RMS 0.05834、过零率 0.1368 —— 正常语音。
- WS 帧拼接后的原始 PCM：峰值 0.673、RMS 0.0735、**零削波样本** —— 也没有坏。
- `_toFloat32`（s16le → [-1,1]）与 `MouthEnvelope` 逐行核对无误。
  **结论：噪声不是「数据坏」，而是「播放调度坏」**——单看字节流永远看不出来，必须看调度。

### 2. 抓包：分片到达节奏是关键事实

用 Node 客户端订阅 `/ws/state` 并记录每帧到达时间戳（一次三句话的对话）：

| 指标 | 实测 |
| --- | --- |
| 帧数 / 帧长 | 432 帧 × 20 ms |
| `start=true` 次数 | **44**（= TTS 分片数） |
| 音频总量 | 8.64 s |
| 句内分片到达间隔 | **0~2 ms（突发）** |
| 句间空档 | **4.286 s / 3.743 s** |

`DEFAULT_AUDIO_CHUNK_SAMPLES = 4800` @24 kHz = 每分片 200 ms；
`broadcast_audio_frames` 把每个分片再切成 10 个 20 ms 帧，**每分片首帧都带 `start=true`**。
所以「一轮语音」会产出几十个 `start`。

### 3. 数值混音：把两种调度规则的实际输出算出来

把抓到的 432 帧按两种规则排到时间轴上并逐样本相加：

| 规则 | 重叠对 | 累计重叠 | 最大同时发声 | 混音峰值 | 时间轴跨度 |
| --- | --- | --- | --- | --- | --- |
| A 现行（`start` 重置） | 402 | 44.03 s | **17** | 1.109（削波） | 8.26 s（音频本身 8.64 s！） |
| B 修复（不重置） | 0 | 0 s | 1 | 0.673（= 源峰值） | 10.69 s |

规则 A 把 8.64 s 音频塞进 8.26 s 时间轴——**物理上不可能**，只能是多路叠加。

### 4. 真实客户端打桩（决定性证据）

不靠听、不靠推测：用 CDP 在页面加载前 patch `AudioBufferSourceNode.prototype.start`，
记录**产品代码**实际发出的每一个 `when`，再对 `(when, buffer.duration)` 做区间重叠分析。

**修复前（基线）**

| 指标 | 值 |
| --- | --- |
| `start()` 调用 | 534 |
| 音频总量 | 10.68 s |
| 时间轴跨度 | 13.99 s（压缩比 **0.76×**） |
| **重叠对** | **529** |
| **累计重叠** | **9.79 s** |
| **最大同时发声** | **17** |
| 重置到 `now` 的次数 | 56，其中 **52 次上一片仍在播** |

**修复后（同一套打桩、同一条对话流程）**

| 指标 | 值 |
| --- | --- |
| `start()` 调用 | 518 |
| 音频总量 | 10.36 s |
| 时间轴跨度 | 12.91 s |
| **重叠对** | **0** |
| **累计重叠** | **0.00 s** |
| **最大同时发声** | **1** |
| 调度提前量 | 3.03 s（→ 由缺陷 2 的修复吸收） |

## 两个缺陷（第二个是修复第一个后才会暴露的连带缺陷）

### 缺陷 1：播放时间轴被分片起始标记重置 → 重叠削波

`audio_player.dart::_schedule` 旧代码：

```dart
if (frame.start || _nextStart < now) _nextStart = now;  // ← frame.start 是元凶
```

`start` 是「分片首帧」不是「轮次起点」。真正的轮次边界早已由 epoch 变化驱动
`AudioPlayer.interrupt()`，这个重置分支纯属多余且有害。

### 缺陷 2：口型在「到达时」释放 → 超前最多 3.36 s

`feed()` 在到达时就把 RMS 电平推给渲染面，但音频要等到 `when` 才出声。
去掉重置后调度提前量变成均值 **1.45 s** / 峰值 **3.36 s**（分片 1 ms 内到齐、句间空档数秒），
口型会「没出声嘴唇先动」。**只修缺陷 1 会换一个 bug**，必须同时修。

## 修复内容

| 文件 | 改动 |
| --- | --- |
| `shell/flutter/lib/audio/schedule.dart` | **新增**（纯逻辑，无 web 依赖）：`scheduleSlice` 保证时间轴只前进；`LevelTimeline` 按播放时刻释放口型电平 |
| `shell/flutter/lib/audio/audio_player.dart` | 调度改走 `scheduleSlice`（不再读 `frame.start`）；口型经 `LevelTimeline` + 20 ms 节拍器延迟到出声时刻 |
| `crates/live2d-ai-desktop/src/web_api/ws.rs` | `Broadcaster` 按 epoch 跟踪，`start=true` **只在每轮语音首片**出现（修复前每 200 ms 分片都发） |
| `shell/flutter/test/audio_schedule_test.dart` | **新增** 7 个单测 |
| `crates/live2d-ai-desktop/src/web_api/ws.rs` 测试 | 新增 `segment_start_false_marks_no_frame_as_start`、`broadcaster_marks_only_first_chunk_of_epoch_as_start` |

### 不变量（修复后）

1. **时间轴只前进**：`when >= 入口 nextStart`，任意两片不重叠；只有「排空」时才锚定 `now`。
2. **口型按出声时刻释放**：到达时只计算包络，入队 `(播放时刻, 电平)`，由 20 ms 节拍器到期释放。
3. **降级不破链**：无 WebAudio 或单帧调度失败只影响该帧，口型照常。

### 服务端语义修正的端到端验证

另起 18081 实例跑同一对话：

```
帧数: 656  音频: 13.12s
start=true 次数: 1        ← 修复前应为 68（= end=true 次数）
end=true 次数: 68
PASS: 一轮语音只发一次 start=true
```

## 复现 / 回归命令

```bash
# 1) 纯逻辑单测（不启浏览器）
cd shell/flutter && ~/flutter/bin/flutter test

# 2) 服务端 start 语义
cargo test -p live2d-ai-desktop web_api::ws

# 3) 真实客户端调度回归（需 Windows 侧 headless Chrome + CDP 9222）
node /tmp/wsprobe/sched_probe3.mjs   # 写 /tmp/wsprobe/sched_after.json
node /tmp/wsprobe/compare.mjs        # 期望：重叠对 0、最大同时发声 1
```

判据：**重叠对必须为 0、最大同时发声必须为 1**。若不为 0，说明又有人把时间轴回退了。

## 教训 / 防回归

- **「分片起始」不等于「语音起点」**。协议字段名必须写清作用域，否则调用方必然误用。
- **只看字节流无法发现播放调度缺陷**。这类缺陷必须看「调度决策」（本次是 `when` 序列），
  这也是为什么当时「音频数据检查都正常」却依然很吵。
- **回归测试要有牙齿**：`audio_schedule_test.dart` 里有一条同输入下「旧规则必然重叠」的钉子，
  保证未来若有人改回重置逻辑，测试会立刻变红。
- 离线数值混音（把 `when` + buffer 逐样本相加看峰值/削波）比反复试听更快也更可复现。
