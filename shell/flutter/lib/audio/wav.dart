/// WAV 封装 + 播放进度取电平：**纯逻辑，零 Flutter / 零 `package:web` 依赖**。
///
/// # 为什么需要它（2026-09-11 用户裁决）
///
/// 用户报「浏览器禁用声音，但依旧有声音传出」，并裁定音频路径为：
/// **后端出 WAV 文件、前端 web 播放**（`<audio>` 媒体元素），而不是后端播放、
/// 也不是继续用 Web Audio 逐片调度。
///
/// 两条技术依据（都已实测，不是推断）：
///
/// 1. **后端不会发声**：`GET /api/v1/app/status` 的 `audio.backend = "none"`、
///    `available: false`（硬编码于 `app_routes.rs`）——后端没有声卡输出，
///    声音只可能来自浏览器。
/// 2. **Web Audio 与媒体元素走的是两套权限**：Web Audio 归 Chromium 的
///    **autoplay 策略**管（有用户手势 `resume()` 即可运行）；而「站点声音 =
///    阻止」这类设置是作用在**媒体元素**上的。用 Web Audio 逐片播放时，
///    站点级静音很可能拦不住——这正是「禁了还有声」的表现。
///
/// # 为什么不是 MP3
///
/// 用户最初说「推 mp3」，但本机 TTS **实测出不了 mp3**：
/// `response_format=mp3` → HTTP 400、`wav` → 400、只有 `pcm` → 200
/// （24 kHz s16le 单声道）。所以要么后端自己引入编码器（与「运行时依赖 = 0」
/// 冲突），要么用 **WAV**——WAV 就是「44 字节头 + 裸 PCM」，零依赖可自造。
/// 这就是本文件存在的理由。
///
/// # 口型
///
/// 媒体元素下没有「逐样本调度时刻」可用了，所以口型改为**按播放进度取电平**：
/// 服务端每片本来就算好了 `data.volume`（RMS），把它按**片在句内的偏移**记成
/// 一条包络，播放时用 `audio.currentTime` 去查（[levelAtSec]）。
library;

import 'dart:typed_data';

/// 每样本字节数（WAV 只支持整字节，s16le = 2）。
const int _bytesPerSample = 2;

/// 把一个句子的**裸 s16le PCM** 包成 WAV 文件字节。
///
/// 入参就是 WS `audio` 帧里 `audio` 字段解出来的原始字节（s16le 小端），
/// **不做格式转换**——WAV 的载荷与 PCM 逐字节相同，转换只会引入损失和 bug。
///
/// 这是[媒体元素]能直接吃的容器：`Blob([bytes], {type:'audio/wav'})` 后
/// `URL.createObjectURL` 给 `<audio src>`。
///
/// [样本字节序]固定小端（RIFF 规定），而 TTS 侧（CosyVoice `pcm`）也是小端，
/// 因此无需字节交换。
///
/// [媒体元素]: https://developer.mozilla.org/docs/Web/HTML/Element/audio
Uint8List wavFromPcm16(
  Uint8List pcm, {
  required int sampleRate,
  int channels = 1,
}) {
  assert(channels > 0, 'channels 必须为正');
  final int byteRate = sampleRate * channels * _bytesPerSample;
  final int dataLen = pcm.length;
  final Uint8List out = Uint8List(44 + dataLen);
  final ByteData view = ByteData.view(out.buffer);

  void ascii(int offset, String s) {
    for (int i = 0; i < s.length; i++) {
      out[offset + i] = s.codeUnitAt(i);
    }
  }

  // RIFF chunk
  ascii(0, 'RIFF');
  // 整个文件长度 - 8（"RIFF" 与自身长度字段）
  view.setUint32(4, 36 + dataLen, Endian.little);
  ascii(8, 'WAVE');
  // fmt sub-chunk
  ascii(12, 'fmt ');
  view.setUint32(16, 16, Endian.little); // PCM fmt chunk 长度
  view.setUint16(20, 1, Endian.little); // audioFormat=1 (PCM)
  view.setUint16(22, channels, Endian.little);
  view.setUint32(24, sampleRate, Endian.little);
  view.setUint32(28, byteRate, Endian.little);
  view.setUint16(32, channels * _bytesPerSample, Endian.little); // blockAlign
  view.setUint16(34, 8 * _bytesPerSample, Endian.little); // bitsPerSample
  // data sub-chunk
  ascii(36, 'data');
  view.setUint32(40, dataLen, Endian.little);
  out.setRange(44, 44 + dataLen, pcm);
  return out;
}

/// 句内某时刻的电平（`offsetSec` 从**这一句开头**算起）。
typedef LevelPoint = ({double offsetSec, double level});

/// 按播放进度取口型电平。
///
/// # 为什么不是「到达即推」
///
/// TTS 分片是**毫秒级突发**到达的（一句 2 s 的音频在眨眼间到齐）。若在到达时
/// 直接把电平推给渲染面，嘴唇会比声音早最多数秒。媒体元素没有逐片调度时刻，
/// 所以用**播放进度**（`currentTime`）作为时间轴——语义更直观，也更好测。
///
/// 判据是**保持**（step / hold）而不是线性插值：包络点本身就是 20 ms 粒度的
/// RMS 测量值，插值不会更准，反而会凭空造出中间值。点之间保持前一点的取值，
/// 与「这一片的声音还在响」这一物理事实一致。
///
/// [progressSec] 超出最后一个点 → 返回最后一点的电平（而不是突然归零：
/// 归零那一帧嘴唇会"啪"地闭上，听感/观感都比多张 20 ms 更差）；
/// 为负（`currentTime` 尚未推进）→ 返回第一个点。
double levelAtSec(List<LevelPoint> points, double progressSec) {
  if (points.isEmpty) return 0;
  if (progressSec <= points.first.offsetSec) return points.first.level;
  double current = points.first.level;
  for (final LevelPoint p in points) {
    if (p.offsetSec > progressSec) break;
    current = p.level;
  }
  return current;
}
