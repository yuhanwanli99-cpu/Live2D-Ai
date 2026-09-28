# BATCH-0009 — Phase 1 · lib/audio/ 全量 + lib/voice/

> 账本：`AUDIT-B/`。行号基于 HEAD `5ef879f4`。

## 本批读过
| 文件 | 行 | 一句话 |
|---|---|---|
| lib/audio/sentence_assembler.dart | 436 | 全读：攒句六条规则写在头注里且逐条可验；`end` 是唯一闸门（`start`/seq 变化只作防御）；空句不产空 WAV；`SerialQueue` 把「不许并发」做成纯逻辑 ✓；`_RmsEnvelope` 与后端 `RmsMeter` 同参数 ✓ |
| lib/audio/audio_player.dart | 535 | 全读：blob URL **建完即有回收路径**（`_create` catch 补 revoke、`_release` 顺序在播放之后）；元素 `display:none` 插 DOM；`_tick` 以 `ended` 事件为主、时长为兜底；静音/音量落 `element.muted`/`volume`（不折增益）✓ |
| lib/audio/epoch_gate.dart · stage_clock.dart · gain.dart | 155 | 全读：三个纯判据模块（打断/epoch 变化/30 ms 不补帧节拍/音量曲线），每个都有对应测试文件 |
| lib/voice/voice_listen_controller.dart | 526 | 读 1-270 + 385-460：`_disposed` 守卫遍布每个 await 之后；两个 Timer 在 dispose 归零；`fatal` 错误会复位 `_ptt`（唯一的兜底，见 F-0009-1） |
| lib/voice/speech_recognizer{,_web,_stub}.dart | 201 | 条件导入（Web 有实现 / VM stub），`kVoiceWebSpeechNote` 诚实性文案在位 |
| 红线 O 验证 | — | 30 Hz 通路实测：`AudioPlayer._tick` → `_levels.add`（Stream）→ `main.dart:599 _audio.levels.listen((level) => _stageKey.currentState?.setMouth(level))` ⇒ **不进 ChangeNotifier、不进 Widget 树** ✓；stage-clock 同理经 `sendStageClock` |
| test 抽查 | — | `wav_test`(133) · `sentence_assembler_test`(418) · `audio_epoch_gate_test`(105) · `voice_listen_controller_test`(420) · `audio_gain_test`(74) · `stage_clock_test`(128) |

## 发现
- **F-0009-1（P3）** PTT（按住说话）没有「松手事件丢失」的兜底：`_ListenButtonState.dispose` 只取消计时器、不补发 release ⇒ 极端路径下 `_ptt` 永久为 true（麦克风持续占用 + 常驻听被挡）。

## 本批核对过、不成发现的（正面记录）
- **红线 O 成立**且是本仓做得最扎实的一条：30 Hz 口型与 stage-clock 都走 Stream → GlobalKey → postMessage，连 `RepaintBoundary` 都另加在 stage_host 上；`test/stage_clock_test.dart:107-127` 的接线扫描虽然粗糙，但**每个断言字符串在对应文件里只出现一次**（判别力可接受），且上面 99 行是真实行为断言。
- `wav` 直通分支（`PcmFrame.wav`，ws_frame 的 `hasWav`）在装配器里是**自洽的**：它先 `_reset()` 丢弃正在攒的半截、用 `_wavDurationSec` 估时长、包络只放一个点——三条都对得上（该字段当前服务端不发，是前向兼容钩子）。
- `AudioPlayer.dispose` 先 `interrupt()` 再置 `_disposed`，让最后一次「停」信号发得出去（:229 注释写明「绝不在下一次音频开始时补播旧帧」）——顺序是有意的且正确。
- 静音与口型正交这条硬约束在实现层是真的：`muted` 只写 `element.muted`，`PcmFrame.muted`（服务端静音观测值）明确「不据此改行为」，播出来是静音但嘴照动。
- `voice_listen_controller` 的 await 纪律：`pressStart` 的三次 `if (_disposed || !_ptt) return;` 分别守在 `_voiceModBlockReason` / `_reloadWakePhrase` / `_begin` 之后——是本仓里数得过来的、把「await 后状态可能已变」逐处显式处理的文件。

## 本批未核实
- `speech_recognizer_web.dart` 的 Web Speech 事件绑定（`onresult`/`onend`/`onerror` 的具体处理）只看了接口面，未逐行；它的 144 行是本批唯一的**未读主体**（留给 B10 与 live2d 一起看 Web API 用法）。
- F-0009-1 的触发概率（丢失 pointerup）未量化；修法本身是 1 行。
