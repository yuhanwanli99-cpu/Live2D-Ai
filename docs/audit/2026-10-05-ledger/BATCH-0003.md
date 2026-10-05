# BATCH-0003 · WS 事件投影与连接生命周期

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`

## 读过的文件（全部来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/ws.rs` — 263
2. `crates/live2d-ai-desktop/src/web_api/ws/events.rs` — 389
3. `crates/live2d-ai-desktop/src/web_api/ws/connection.rs` — 308
4. `crates/live2d-ai-desktop/src/web_api/ws/broadcaster.rs` — 215（**追加**：connection.rs 的
   `WsWriter` 发送端语义、`try_send` 失败后的回收路径只在这个文件里，不读它无法判定
   连接生命周期是否泄漏）

合计 4 文件 / 1175 行。

## 为验证契约而读的清单外文件（只读片段，不计入本批审计结论）
- `shell/flutter/lib/api/ws_frame.dart:56-120`（`TextDeltaEvent` 的 `text`/`completed` 双可空）
- `shell/flutter/lib/state/ui_state_tracker.dart:126-150`（`voice_ended` 消费分支）
- `crates/live2d-ai-desktop/src/web_api/ws/audio.rs:1-8, 62-130`（音频帧 20ms 切片 + base64）
- `grep -rn "build_audio_frames"`（确认音频帧经 broadcaster → actor 同一路径）

## 跑过的命令（全部只读）
```
grep -rn "text_delta" shell/flutter/lib/ | head -20
grep -rn "voice_ended" shell/flutter/lib/ | head -15
sed -n '60,120p' shell/flutter/lib/api/ws_frame.dart
sed -n '120,150p' shell/flutter/lib/state/ui_state_tracker.dart
grep -n "base64|fn build_audio_frames" -A 4 crates/live2d-ai-desktop/src/web_api/ws/audio.rs
grep -rn "build_audio_frames" crates/ --include=*.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 E（前后端契约）：✔ 逐帧对照 Dart 侧 `ws_frame.dart`。`text_delta` 的两种载荷
  （`{epoch,ts_ms,text}` 与 `{epoch,completed}`）在 Rust 侧共用一个 type（events.rs:92/100），
  初判为契约风险，**读 Dart 后证伪**：`TextDeltaEvent.text` / `.completed` 双 `String?/bool?`
  可空（ws_frame.dart:68-81），两侧是自洽的。**不记发现。**
- 维度 B（生命周期/清理）：✔ 三条退出路径（写失败 break / Disconnected break / 首帧写失败
  early-return）**都**调 `unsubscriber(conn_id)`（connection.rs:137/166）；
  `broadcaster.unsubscribe` 按 id `retain` 移除并同步减计数（broadcaster.rs:107-119），无泄漏。
- 维度 C（错误三态）：✔ WS 升级三态齐（426 缺 key / 403 Origin / 503 超限），
  每个都有稳定 code；✘ 但两处 403/丢弃只落 `debug!` → F-0003-01
- 红线 Q（契约只增不改）：✔ `action_cue` 复用既有帧型（events.rs:147），既有键
  `sentence_seq/preset_id/intensity/ttl_ms/priority` 逐字保留，v1 新键以可选键摊平（events.rs:138-144）；
  `reasoning_delta` / `text_fallback` 是**独立**帧型且旧客户端 `default: break` 忽略 → 向后兼容 ✔
- 红线 G（秘密）：error 帧投影 `code/stage/message/hint/epoch/fatal`（events.rs:189-199），
  未投影任何密钥字段。**注意**：日志侧有 `SanitizingWriter` 兜底，WS 帧侧**没有**等价脱敏；
  若上游错误体回显 `Authorization`，`err.message` 会原样进浏览器。可达性未证实 → 候选池 C-5。
- 维度 D（热路径）：`wrap_preserialized_with_seq`（connection.rs:199-216）对每帧 parse→插入→重序列化。
  初判为「base64 音频大帧 ⇒ 热路径重解析」，**读 audio.rs 后证伪**：PCM 按 ~20ms 切片
  （audio.rs:3），24kHz 单声道下每帧约 1.3KB，parse+serialize 在 μs 级，
  头注 connection.rs:195 的「<10μs（短帧）」对该尺寸成立。**不记发现。**

## 已核实并主动放弃的假设（记录以免下批重走）
1. 「每帧重解析是热路径瓶颈」——证伪（切片粒度决定帧很小）。
2. 「慢订阅者 try_send 失败会泄漏 actor 线程 / 突破 WS_CONN_LIMIT」——证伪：
   `swap_remove` 丢弃 `ConnectionHandle` 即丢弃该通道**唯一**的 `SyncSender` 克隆
   （`WsWriter = mpsc::SyncSender<String>` 非 Arc，handle_ws_request 只造一个 tx），
   mpsc 立刻置 `Disconnected`，actor 在 `recv_timeout` 上立即返回并走 close+unsubscribe
   （connection.rs:157-166），`subscribed_count` 同步 -1。**残留观察记为 P3（F-0003-04）**。
3. 「`voice_ended` 有三个投影来源（PlaybackDrained / PlaybackCleared / VoiceEnded）会重复触发」——
   证伪为无害：唯一消费者 `ui_state_tracker.dart:136` 是幂等赋值 `_voiceActive = false`。

## 未核实项
1. 前端 WS 是否有自动重连（决定 F-0003-04 的用户可见性）——未读 `ws_client.dart`。
2. C-5（error 帧无脱敏）可达性未证实，需门禁/真实上游验证。

## 本批新增
P0 0 · P1 0 · P2 1 · P3 3
