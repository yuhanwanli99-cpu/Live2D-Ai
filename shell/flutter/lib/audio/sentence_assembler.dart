/// 句装配 + 串行播放队列：**纯逻辑，零 Flutter / 零 `package:web`**。
///
/// # 为什么单独成文件（2026-09-11 音频路径重写）
///
/// 播放路径从「Web Audio 每 20 ms 排一片 `AudioBufferSourceNode`」改成
/// 「攒一整句 → 封 WAV → `<audio>` 媒体元素播放」（原因见
/// `audio_player.dart` 的头注：站点级「声音 = 阻止」作用在媒体元素上，
/// 管不住 Web Audio）。换掉之后真正需要回归的算术变成两件事：
///
/// 1. **攒句**：服务端按 20 ms 切片下发，一句必须**完整**封成一个 WAV 才播
///    ——「一句一单元」是硬契约（不句中切分、不重叠、不并发）；
/// 2. **包络**：媒体元素没有逐片播放时刻可用，口型改为「按 `currentTime`
///    查表」（[levelAtSec]），所以每片的 `volume` 必须按它在**句内的累计
///    偏移**记下来。
///
/// 这两件事都是纯算术，必须能在 `flutter test` 的 VM 上跑：`audio_player.dart`
/// 因为 `import 'package:web'` 在 VM 上**根本加载不了**（守卫见
/// `test/no_backdrop_filter_test.dart` 的 `package:web` 白名单）。所以算术在
/// 这里，`audio_player.dart` 只留「建 element / 建 blob / play / revoke」胶水。
///
/// # 句界：`end` 是唯一闸门（2026-09-11 协议裁定，实测过的）
///
/// 服务端 WS audio 帧曾有的两个句界信号**都不可用**，直接按它们攒句必然断续：
///
/// | 字段 | 旧实现 | 实测（正文只有「一、二、三。」**一句**） |
/// | --- | --- | --- |
/// | `start` | `segment_start && i == 0`，而 `segment_start` 来自 `previous != token`（token = epoch+1）记账 | **0 个** `start=true`（整轮甚至整个 epoch 只可能出现一次，且 epoch 复用时会丢） |
/// | `end` | `(i + 1 == total)`，即**每次广播调用的末片** | **13 个** `end=true`（一句音频被拆成十几个 `AudioChunk`，每个 chunk 各自末片都发 end） |
///
/// 根因：引擎侧「一句音频恰好一个 `final_chunk = true`」（见
/// `live2d-ai-runtime` 的 `conversation/mod.rs`），而桌面的
/// `build_audio_frames` 是**按一次广播调用**切片的——两者不是同一个粒度。
///
/// 新契约（后端同步修改，并由 `scripts/verify_core_chain.py` 的
/// **「句子边界配对」**判据守着：句界不对称会直接 FAIL，不会静默变成断续播放）：
///
/// - `start=true`：某一句的**第一片**，每句恰好一次；
/// - `end=true`：该句的**最后一片**，每句恰好一次（即引擎的 `final_chunk`）；
/// - `sentence_seq`（可选，从 1 严格递增）：同一句的分片归组，句变则 seq 变；
/// - `epoch` / `sample_rate` / `volume` / `muted` / `slice_ms` 不变。
///
/// 本装配器的攒句规则（**不依赖 `start` 一定出现**）：
///
/// 1. 收到任意一片就开始累积（不等 `start`）；
/// 2. [`SentenceAssembler.push`] 返回非 null = 「这一句齐了，可以封 WAV 去播」，
///    而它**只在 `end` 上触发**——这条在新旧两版服务端上都成立；
/// 3. `start` 只当「上一句已结束、这是新一句开头」的**防御性**信号：累积中途
///    遇到 `start` 说明上一句缺了 `end`，**丢弃半截重来**（宁可丢一句，也绝不
///    把两句拼成一句——拼接错误是听感上最糟的一种，且不会被用户察觉为「丢东西」）；
/// 4. `sentence_seq` 存在时用它归组（seq 变化 = 新一句，同样丢弃半截）；
///    缺失时退化为「上次封口之后继续累积」，不报错；
/// 5. **不为「旧服务端每个 chunk 都发 `end`」加启发式**（例如「太短就继续攒」）：
///    那是猜测，会让正确的句界也走错分支。旧服务端由后端替换，配对错误由上面
///    那条验收判据大声暴露；
/// 6. 缺 `end` 的尾巴**不猜、不自动封口**（自动封口 = 断句，契约上不可接受），
///    由调用方在「确认不会再有本句分片」时显式调 [`SentenceAssembler.flush`]。
library;

import 'dart:collection';
import 'dart:math' as math;
import 'dart:typed_data';

import 'wav.dart';

/// 把包络类型与取值函数随装配器一起暴露：调用方拿到 [AssembledSentence] 就要
/// 用 `envelope` / [levelAtSec] 驱动口型，不该再被迫多 import 一个文件。
export 'wav.dart' show LevelPoint, levelAtSec;

/// 采样率缺省值：帧里没带或带了非法值时用它（与 TTS 默认 24 kHz 一致）。
const int kFallbackSampleRate = 24000;

/// 一次 WS `audio` 帧的领域表示（纯数据，不含 DOM）。
///
/// 这里是它的**家**：它同时是协议层（`api/ws_frame.dart` 的 `AudioEvent`）与
/// 装配器的输入。`audio_player.dart` 只把它**转手**给 [SentenceAssembler]，
/// 所以类型定义放在纯逻辑模块里（`audio_player.dart` 会 re-export 它，
/// 旧调用方 `import 'audio/audio_player.dart'` 拿 `PcmFrame` 的写法不变）。
class PcmFrame {
  const PcmFrame({
    required this.pcm,
    required this.sampleRate,
    this.start = false,
    this.end = false,
    this.volume,
    this.muted = false,
    this.sentenceSeq,
    this.wav,
  });

  /// 本片的裸 s16le 单声道 PCM（已 base64 解码）。
  final Uint8List pcm;

  /// 本片 PCM 的采样率（Hz）。`<= 0` 时回落到 [kFallbackSampleRate]。
  final int sampleRate;

  /// 服务端按**原始**样本算出的 RMS ∈ [0,1]（`data.volume`）。
  ///
  /// **服务端静音时这是口型的唯一来源**：PCM 被置零（谁都发不出声），
  /// 但 volume 仍是真实值。非 null 时优先用它，而不是从（可能已置零的）
  /// PCM 里算 RMS——否则静音会连带把口型关掉。
  final double? volume;

  /// 服务端是否处于静音输出（`data.muted`）——**观测值，装配/播放不据此改行为**。
  ///
  /// 服务端静音时 PCM 本来就是全零，客户端再「跳过播放」只会丢掉口型需要的
  /// 时间轴（口型靠 `element.currentTime` 取包络，不播就没有进度）。所以这一帧
  /// 照常封 WAV、照常播——播出来是静音，嘴照动。字段保留是为了让诊断能把
  /// 「服务端静音」与「本地静音」（`AudioPlayer.muted`）分开看。
  final bool muted;

  /// 服务端 `audio.start`：**某一句的首片**（2026-09-11 新契约）。
  ///
  /// 攒句**不以它为闸门**：旧服务端的 `start` 是**轮级**的（`previous != token`
  /// 记账，整轮只可能出现一次，实测正文只有一句时收到 0 个 `start=true`）。
  /// 现在它只作防御信号：累积中途出现 = 上一句缺了 `end`，丢弃半截重来。
  final bool start;

  /// 服务端 `audio.end`：**该句的末片**，是攒句的**唯一闸门**。
  ///
  /// 旧服务端的 `end` 是「每次广播调用的末片」（一句被拆成十几个 chunk，
  /// 于是单句实测收到 13 个 `end=true`）——那是后端粒度错位，已修；
  /// 本字段本身的语义在新旧两端都成立，故只认它。
  final bool end;

  /// 同一句的分片归组序号（`data.sentence_seq`，可选；从 1 严格递增）。
  ///
  /// 缺省 `null`（旧服务端 / 未来字段改名）→ 装配器退化为「上次封口后继续
  /// 累积」，不报错。
  final int? sentenceSeq;

  /// **整句 WAV 直通**（前向兼容钩子，可选；当前服务端不发）。
  ///
  /// 将来若帧上直接带封好的整句 WAV（`data.wav` base64），[SentenceAssembler]
  /// 直接拿它当一句，不再攒片。设计成「缺省不报错」是为了将来加字段不必改调用方。
  final Uint8List? wav;
}

/// 一句装配完成的音频单元：**已经是完整可播的 WAV 文件字节**。
class AssembledSentence {
  const AssembledSentence({
    required this.bytes,
    required this.sampleRate,
    required this.envelope,
    required this.durationSec,
  });

  /// 交给 `<audio>` 的完整文件字节（RIFF/WAVE 容器，见 [wavFromPcm16]）。
  final Uint8List bytes;

  /// 这一句的采样率（Hz），来自首片。
  final int sampleRate;

  /// 句内电平包络，`offsetSec` 从**这一句开头**算起（严格升序：按片序累加）。
  final List<LevelPoint> envelope;

  /// 这一句的时长（秒）。用于「播放是否已经走完」的兜底判定（DOM `ended`
  /// 事件是主判据，见 `audio_player.dart`）。
  final double durationSec;

  /// 按句内播放进度取口型电平（保持语义见 [levelAtSec]）。
  double levelAt(double progressSec) => levelAtSec(envelope, progressSec);
}

/// 把 20 ms 音频片攒成「一句」的装配器。**一句一单元**。
///
/// 用法：`push(frame)` 返回非 null 就是一句齐了（`end` 闸门）；调用方把它交给
/// 播放队列即可。[flush] 是「确认不会再有本句分片」时的显式收尾（服务端漏发
/// `end` 的兜底），[clear] 是 epoch 作废 / 抢占时的丢弃。
class SentenceAssembler {
  final BytesBuilder _pcm = BytesBuilder(copy: false);
  final List<LevelPoint> _envelope = <LevelPoint>[];
  final _RmsEnvelope _rms = _RmsEnvelope();

  bool _assembling = false;
  int _rate = kFallbackSampleRate;
  int? _seq;

  /// 本句已累计的样本数（用来算下一片的包络偏移，浮点秒数由它导出）。
  int _samples = 0;

  /// 已累计的裸 PCM 字节数（诊断 / 测试用）。
  int get pendingBytes => _pcm.length;

  /// 已累计的电平点数（诊断 / 测试用）。
  int get envelopePoints => _envelope.length;

  /// 当前是否有一句正在累积（缺 `end` 时会一直为 true，这是**设计如此**）。
  bool get isAssembling => _assembling;

  /// 当前累积句的 `sentence_seq`（服务端没给就是 null）。
  int? get sentenceSeq => _seq;

  /// 投入一片。返回非 null = 这一句已齐（`end` 闸门）。
  ///
  /// 返回 null 有三种情况：还在攒；这一片触发了「丢弃半截重来」（`start` 或
  /// seq 变化）；这一片是空句（`end` 但没有任何 PCM，不产空 WAV）。
  AssembledSentence? push(PcmFrame frame) {
    // 整句 WAV 直通（前向兼容）：它自己就是一句，丢弃正在攒的半截。
    final Uint8List? whole = frame.wav;
    if (whole != null && whole.isNotEmpty) {
      _reset();
      final int rate = _rateOf(frame.sampleRate);
      return AssembledSentence(
        bytes: whole,
        sampleRate: rate,
        envelope: <LevelPoint>[
          if (frame.volume != null && frame.volume!.isFinite)
            (offsetSec: 0.0, level: _clampLevel(frame.volume!)),
        ],
        durationSec: _wavDurationSec(whole.length, rate),
      );
    }

    // 防御 1：累积中途出现 start = 上一句缺 end → 丢弃半截（绝不拼句）。
    if (frame.start && _assembling) _reset();
    // 防御 2：sentence_seq 变了 = 新一句（同样丢弃半截）。缺失时不参与判断。
    final int? seq = frame.sentenceSeq;
    if (_assembling && seq != null && _seq != null && seq != _seq) _reset();

    if (!_assembling) {
      // 规则 1：**收到任意一片就开始累积**，不等 start（它可能整轮都不出现）。
      _assembling = true;
      _seq = seq;
      _rate = _rateOf(frame.sampleRate);
      _rms.reset();
    }

    // 包络点：偏移 = 本片在这一句内的起点。空片（引擎允许 final 块 0 样本）
    // 只要带了 volume 也记一个点——它让嘴巴在句尾准确闭上。
    if (frame.pcm.isNotEmpty || frame.volume != null) {
      _envelope.add((
        offsetSec: _elapsedSec,
        level: _levelFor(frame),
      ));
    }
    if (frame.pcm.isNotEmpty) {
      _pcm.add(frame.pcm);
      _samples += frame.pcm.length ~/ 2;
    }

    // 规则 2：**end 是唯一闸门**（新旧服务端都成立）。
    if (frame.end) return _seal();
    return null;
  }

  /// 显式收尾：把正在累积的半截封成一句（返回 null = 本来就没在攒）。
  ///
  /// **只在调用方确认「不会再有本句分片」时用**（例如一轮已收口而服务端漏发了
  /// `end`）。正常路径不该走这里：中途封口 = 断句，契约上不可接受。
  AssembledSentence? flush() => _assembling ? _seal() : null;

  /// 丢弃正在累积的一切（epoch 变化 / 抢占 / 停止）。
  void clear() => _reset();

  /// 已累计的秒数（= 下一片的包络偏移）。
  double get _elapsedSec => _samples / _rate;

  /// 封口：产出这一句并重置累积状态。
  AssembledSentence? _seal() {
    final Uint8List pcm = _pcm.takeBytes();
    final List<LevelPoint> envelope = List<LevelPoint>.of(_envelope);
    final int rate = _rate;
    _reset();
    // 空句（end 但没有任何 PCM）：不产空 WAV——浏览器对 0 字节 data 块会报
    // 「无法播放」，白白污染一次 error 事件。
    if (pcm.isEmpty) return null;
    return AssembledSentence(
      bytes: wavFromPcm16(pcm, sampleRate: rate),
      sampleRate: rate,
      envelope: envelope,
      durationSec: pcm.length / 2 / rate,
    );
  }

  void _reset() {
    _pcm.clear();
    _envelope.clear();
    _rms.reset();
    _assembling = false;
    _seq = null;
    _samples = 0;
    _rate = kFallbackSampleRate;
  }

  int _rateOf(int raw) =>
      raw > 0 && raw <= 384000 ? raw : kFallbackSampleRate;

  /// 本片的电平：服务端 `volume` 优先，缺字段才在本地算 RMS（见 [PcmFrame.volume]）。
  double _levelFor(PcmFrame frame) {
    final double? server = frame.volume;
    if (server != null && server.isFinite) return _clampLevel(server);
    return _rms.push(frame.pcm, _rate);
  }

  static double _clampLevel(double raw) =>
      raw < 0 ? 0.0 : (raw > 1 ? 1.0 : raw);

  /// 估一个 WAV 的时长：`(文件长度 - 44 字节头) / 2 字节每样本 / 采样率`。
  ///
  /// 只用于「播放是否走完」的兜底判定，所以近似够了——带扩展 chunk 的 WAV
  /// 会被高估几十字节（< 1 ms），不影响判定。
  static double _wavDurationSec(int byteLength, int rate) {
    final int data = byteLength > 44 ? byteLength - 44 : 0;
    return data / 2 / rate;
  }
}

/// **串行**播放队列：一句一单元——同一时刻只放行一个单元。
///
/// 为什么要有它（而不是让 `audio_player.dart` 自己拿个 List）：队列的排序与
/// 「不许重叠」是**契约**（「宁可晚开口，不可中途卡顿或重复」），而
/// `audio_player.dart` 因为 `package:web` 在 VM 上加载不了——放进那里就等于
/// 这条契约没有测试。这里只做顺序决策，不碰任何 DOM。
///
/// 语义：
/// - [add] 入队（顺序 = 调用顺序，**不重排、不合并、不超时丢弃**）；
/// - [takeNext] 只在本单元播完（[finishCurrent]）之后才放行下一个；
///   还有单元在播时返回 null——**这就是「不许并发」的实现**；
/// - [clear] 清空（epoch 作废 / interrupt）。
class SerialQueue<T> {
  final ListQueue<T> _waiting = ListQueue<T>();
  T? _current;

  /// 正在播放（已放行、未 [finishCurrent]）的单元；没有则 null。
  T? get current => _current;

  /// 还没放行的单元数（不含 [current]）。
  int get waiting => _waiting.length;

  /// 整个队列都空了（没有在播的，也没有排队的）。
  bool get isEmpty => _current == null && _waiting.isEmpty;

  /// 队列里的全部单元，正在播的排在最前（供批量改音量用）。
  List<T> get all => <T>[if (_current != null) _current as T, ..._waiting];

  /// 入队。
  void add(T item) => _waiting.addLast(item);

  /// 放行下一个单元；**还有单元在播时返回 null**（严格串行）。
  T? takeNext() {
    if (_current != null) return null;
    if (_waiting.isEmpty) return null;
    return _current = _waiting.removeFirst();
  }

  /// 标记当前单元结束（播完 / 失败跳过）。返回是否还有下一个可放行。
  bool finishCurrent() {
    _current = null;
    return _waiting.isNotEmpty;
  }

  /// 清空队列（含正在播放的单元——释放由调用方负责）。
  void clear() {
    _waiting.clear();
    _current = null;
  }
}

/// 帧内 RMS 包络（s16le → [0,1]），**只在服务端没给 `volume` 时**用它。
///
/// 参数与后端 `RmsMeter` 对齐：窗口 50 ms、attack 0.03 s、release 0.10 s。
/// 逐片推入（20 ms 一片），输出按 attack/release 做一阶平滑——旧服务端不带
/// `volume` 时，这条回退路径的行为与 Web Audio 时代**完全一致**（同一段算术，
/// 只是从 `audio_player.dart` 搬进了纯逻辑模块）。
class _RmsEnvelope {
  /// 窗口 50 ms：与后端 `RmsMeter` 同尺度（再长会把音节抹平，再短会抖）。
  static const double windowSec = 0.05;

  /// 起音时间常数 0.03 s（张嘴要快）。
  static const double attackSec = 0.03;

  /// 释音时间常数 0.10 s（闭嘴稍慢，避免逐帧抖动）。
  static const double releaseSec = 0.10;

  final ListQueue<double> _squares = ListQueue<double>();
  int _rate = 0;
  int _windowSamples = 0;
  double _sumSquares = 0;
  double _level = 0;

  void reset() {
    _squares.clear();
    _sumSquares = 0;
    _level = 0;
    _rate = 0;
    _windowSamples = 0;
  }

  /// 推入一片 PCM，返回平滑后的电平。
  double push(Uint8List pcm, int sampleRate) {
    final int rate = sampleRate > 0 ? sampleRate : kFallbackSampleRate;
    if (rate != _rate) {
      _rate = rate;
      _windowSamples = math.max(1, (rate * windowSec).round());
      _squares.clear();
      _sumSquares = 0;
    }
    final int samples = pcm.length ~/ 2;
    if (samples == 0) return _level;
    for (int i = 0; i < samples; i++) {
      int value = (pcm[i * 2 + 1] << 8) | pcm[i * 2];
      if (value >= 0x8000) value -= 0x10000;
      final double normalized = value / 32768.0;
      final double square = normalized * normalized;
      _squares.addLast(square);
      _sumSquares += square;
    }
    while (_squares.length > _windowSamples) {
      _sumSquares -= _squares.removeFirst();
    }
    final int count = _squares.length;
    final double rms = count == 0 ? 0.0 : math.sqrt(_sumSquares / count);
    final double target = rms < 0 ? 0.0 : (rms > 1 ? 1.0 : rms);
    final double frameSec = samples / rate;
    final double tau = target > _level ? attackSec : releaseSec;
    double alpha = tau <= 0 ? 1.0 : frameSec / tau;
    if (alpha < 0) {
      alpha = 0;
    } else if (alpha > 1) {
      alpha = 1;
    }
    _level += (target - _level) * alpha;
    if (_level < 0) {
      _level = 0;
    } else if (_level > 1) {
      _level = 1;
    }
    return _level;
  }
}
