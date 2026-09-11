import 'dart:async';
import 'dart:js_interop';

import 'package:flutter/foundation.dart';
import 'package:web/web.dart' as web;

import 'gain.dart';
import 'sentence_assembler.dart';
import 'wav.dart';

/// [PcmFrame] 的家搬到了纯逻辑模块 `sentence_assembler.dart`（它同时是装配器的
/// 输入，两份同形 DTO 没有意义）。这里 re-export，**旧调用方
/// （`import 'audio/audio_player.dart'` 拿 `PcmFrame`）一行都不用改**。
export 'sentence_assembler.dart' show PcmFrame;

/// 一次正在播放（或已排队）的句子：媒体元素 + blob URL + 它的包络。
class _Playback {
  _Playback({required this.sentence, this.element, this.url});

  final AssembledSentence sentence;

  /// 承载这一句的 `<audio>`。`null` = **降级**：本浏览器建不出媒体元素
  /// （[AudioPlayer._mediaSupported] 已为 false），这一句只能靠本地时钟模拟
  /// 时间轴来驱动口型（见 [AudioPlayer._play]），发不出声音。
  final web.HTMLAudioElement? element;

  /// `URL.createObjectURL` 的返回值。**必须在句尾 `revoke`**——不回收就是
  /// 把整句音频（几十 KB ~ 几百 KB）连同 blob 一起永久留在页面内存里。
  /// 降级句没有 URL（`null`）。
  final String? url;

  /// 降级句的播放进度时钟（媒体元素没有时用它当时间轴）。
  final Stopwatch clock = Stopwatch();

  /// DOM 事件是否已经收过尾（`ended` 与 ticker 兜底可能同时到）。
  bool released = false;
}

/// 播放器：**攒一句 → 封 WAV → `<audio>` 媒体元素播**（2026-09-11 重写）。
///
/// # 为什么从 Web Audio 换成媒体元素（用户裁决，结论有实测支撑）
///
/// 用户报「我这浏览器禁用声音但是依旧有声音传出」。查证两条事实：
///
/// 1. **后端根本不发声**：`GET /api/v1/app/status` 的 `audio.backend = "none"`、
///    `available: false`（硬编码于 `app_routes.rs`）——声音只可能来自浏览器；
/// 2. **Web Audio 与媒体元素走两套权限**：Web Audio 归 Chromium 的
///    **autoplay 策略**管（有用户手势 `resume()` 就能响），而站点的
///    「声音 = 阻止」是作用在**媒体元素**上的。用 `AudioContext` +
///    每 20 ms 一片 `AudioBufferSourceNode` 播放，站点级静音拦不住它——
///    这正是「禁了还有声」。
///
/// 所以播放路径改为：整句 PCM 包成 **WAV**（44 字节头 + 裸 PCM，零依赖，
/// 见 [wavFromPcm16]）→ `Blob` → `URL.createObjectURL` → `HTMLAudioElement.play()`。
/// 静音/音量直接作用在 `element.muted` / `element.volume` 上——这是本次改动的
/// **全部意义**：让浏览器站点的「声音 = 阻止」真的能管住它。
///
/// # 为什么是 WAV 不是 MP3
///
/// 用户最初要求推 mp3，但本机 TTS **实测出不了 mp3**：`response_format=mp3`
/// → **HTTP 400**、`wav` → **400**、只有 `pcm` → **200**（24 kHz s16le 单声道）。
/// 要么后端自造编码器（与「运行时依赖 = 0」冲突），要么用 WAV。
///
/// # 附带的收益：一整类「电流声」缺陷随之消失
///
/// Web Audio 逐片播放时，context 采样率不是 PCM 率的整数倍就会**每片**重采样
/// 一次（滤波器启动瞬态 → 50 Hz 咔哒串；实测边界跳变 16.36× 片内均值，
/// 原生率只有 0.91×）。媒体元素吃的是**一整个 WAV 文件**，解码器只做一次
/// 连续重采样，这类缺陷从结构上不再存在（旧的 `sample_rate.dart` 整条判定
/// 随之删除）。
///
/// # 不变量
///
/// 1. **一句一单元**：攒到 `end` 才封 WAV；封好的句子进 [SerialQueue] **串行**
///    播放，上一句没 `ended` 之前绝不开下一句（重叠/断句比延迟糟得多）；
/// 2. **口型按播放进度**：媒体元素没有逐片播放时刻，30 ms 轮询
///    `element.currentTime` 后用 [levelAtSec] 在句内包络上取值；没在播就发 0，
///    [interrupt] 立即发 0；
/// 3. **blob URL 生命周期与句子绑定**：句尾 / 出错 / interrupt / dispose 都要
///    `revokeObjectURL`（漏一次就漏一整句音频的内存）；
/// 4. **降级不破链**：建元素 / 建 blob / `play()` 任何一步失败都只影响这一句——
///    不抛、不卡住队列。没有媒体后端时这一句以**降级句**入队（本地时钟当播放
///    时间轴，只有口型没有声音），队列顺序与口型照常；autoplay 被拦时这一句停在
///    队首等用户手势（[unlock] 会重试）。两种情况都通过 [unlocked] /
///    [unlockState] 显式暴露给界面（`AudioBar` 的「点击任意位置或按任意键以
///    启用声音」）。
class AudioPlayer {
  final StreamController<double> _levels = StreamController<double>.broadcast();
  final SentenceAssembler _assembler = SentenceAssembler();
  final SerialQueue<_Playback> _queue = SerialQueue<_Playback>();

  /// 口型轮询节拍：30 ms（≤ 33 ms）。消费端（Live2D 桥）另有 ≤30 Hz 节流，
  /// 所以再密也不会更快上屏。
  static const Duration _levelTick = Duration(milliseconds: 30);

  /// WAV 的 MIME：媒体元素据此选解复用器（`<audio>` 不靠扩展名）。
  static const String _wavMime = 'audio/wav';

  Timer? _ticker;
  double _level = 0;
  bool _disposed = false;
  bool _muted = false;
  double _volume = 1.0;

  Stream<double> get levels => _levels.stream;
  double get level => _level;

  /// 播放后端是否可用。**名字沿用旧契约**（调用方语义不变），在媒体元素路径下
  /// 它表示「能建出 `HTMLAudioElement` 且建 blob 没抛」——探测失败即置 false，
  /// 界面据此不再承诺「开启声音」。
  bool get hasAudioContext => _mediaSupported;
  bool _mediaSupported = true;

  /// 是否静音（**只关声音，不关口型**）。
  ///
  /// 默认 `false`（2026-09-10 用户裁决改为**默认出声**）：产品默认就该能听见。
  /// 需要安静时由用户按静音，或跑测试/无人值守时用服务端
  /// `LIVE2D_AI_MUTE_AUDIO=1` 在源头静音。
  bool get muted => _muted;

  /// 切换静音：直接改媒体元素的 `muted`（**不改音量**，二者正交）。
  set muted(bool value) {
    if (_muted == value) return;
    _muted = value;
    _applyOutput();
  }

  /// 主音量（0..1）。**与 [muted] 正交**：静音不重置它，取消静音后恢复原音量。
  double get volume => _volume;

  /// 设置主音量；非有限值按满音量处理，其余夹到 `[0,1]`。
  ///
  /// 曲线用 [gainForVolume]（`v²` 的温和感知压缩）写到 `element.volume` 上：
  /// 与旧 GainNode 的听感完全一致，手感不变。
  set volume(double value) {
    final double next = value.isFinite ? value.clamp(0.0, 1.0) : 1.0;
    if (_volume == next) return;
    _volume = next;
    _applyOutput();
  }

  /// 音频是否**真的**解锁了（用户手势已发生、媒体元素可用）。
  ///
  /// 媒体元素没有 `AudioContext.resume()`，解锁的语义变成「用户手势已发生，
  /// 之后 `element.play()` 会被 autoplay 策略放行」。这个标志让界面能把失败
  /// **显式化**——否则纯键盘用户会遇到「消息发得出去、嘴在动、但没有声音，
  /// 且界面不解释」。
  bool get unlocked => _unlocked;
  bool _unlocked = false;

  /// 解锁状态变化（界面据此隐藏提示）。
  final ValueNotifier<bool> unlockState = ValueNotifier<bool>(false);

  /// 首次用户手势调用：探测媒体元素后端，并**重试**被 autoplay 拦下的句子。
  void unlock() {
    if (_disposed) return;
    try {
      // 能力探针：建得出就说明本浏览器有媒体元素后端。探针不插进 DOM、不发声。
      web.HTMLAudioElement();
      _mediaSupported = true;
      _setUnlocked(true);
    } catch (_) {
      // 没有媒体元素后端：播放降级，但口型照常（降级不破链）。
      _mediaSupported = false;
      _setUnlocked(false);
      return;
    }
    // 因为 autoplay 被拒而停在队首的那一句，现在立刻重试。
    final _Playback? current = _queue.current;
    if (current?.element?.paused ?? false) _play(current!);
  }

  void _setUnlocked(bool value) {
    if (_unlocked == value) return;
    _unlocked = value;
    unlockState.value = value;
  }

  /// 播放一帧：喂给攒句器；一句齐了就入队。
  void feed(PcmFrame frame) {
    if (_disposed) return;
    final AssembledSentence? sentence;
    try {
      sentence = _assembler.push(frame);
    } catch (_) {
      return; // 坏帧只丢这一帧，不影响后续与口型。
    }
    if (sentence != null) _enqueue(sentence);
  }

  /// **收尾**：把还在攒的半截封成一句（服务端漏发 `end` 时的兜底）。
  ///
  /// 只在「一轮已收口、不会再有本句分片」时调用——这是防「最后一句话永远卡在
  /// 累积里播不出来」的安全网，不是常规路径（常规路径认 `end`）。
  void flush() {
    if (_disposed) return;
    final AssembledSentence? sentence = _assembler.flush();
    if (sentence != null) _enqueue(sentence);
  }

  /// 打断当前播放（epoch 切换 / 抢占 / stop）并清零口型。
  void interrupt() {
    _assembler.clear();
    for (final _Playback item in _queue.all) {
      _release(item);
    }
    _queue.clear();
    _stopTicker();
    _emitLevel(0);
  }

  void dispose() {
    if (_disposed) return;
    interrupt();
    _disposed = true;
    _levels.close();
    unlockState.dispose();
  }

  // ───────────────────────────── 播放队列（串行） ─────────────────────────────

  void _enqueue(AssembledSentence sentence) {
    // 媒体不可用时不丢句子，而是入一个**降级句**：顺序契约（一句一单元）照旧，
    // 只是它没有声音、只有口型（见 [_play]）。丢掉才会「破链」——后面的句子
    // 会一股脑涌上来。
    final _Playback item = _create(sentence) ?? _Playback(sentence: sentence);
    _queue.add(item);
    _startNextIfIdle();
  }

  /// 把一句封成「blob URL + `<audio>`」，失败返回 null（降级）。
  ///
  /// 顺序有讲究：**先建元素、再建 blob、最后才 `createObjectURL`**。这样任何一步
  /// 在中途抛（媒体后端不可用）时都还没有 URL 要回收；万一 URL 建好之后才抛，
  /// catch 里补一次 `revoke`——否则这一句的音频就永远留在内存里（blob URL 不回收
  /// = 整句音频不回收）。
  _Playback? _create(AssembledSentence sentence) {
    String? url;
    try {
      final web.HTMLAudioElement element = web.HTMLAudioElement();
      final web.Blob blob = web.Blob(
        <JSAny>[sentence.bytes.toJS].toJS,
        web.BlobPropertyBag(type: _wavMime),
      );
      url = web.URL.createObjectURL(blob);
      element.src = url;
      element.preload = 'auto';
      final _Playback item =
          _Playback(sentence: sentence, element: element, url: url);
      element.addEventListener('ended', ((web.Event _) => _onEnded(item)).toJS);
      element.addEventListener('error', ((web.Event _) => _onError(item)).toJS);
      _applyOutputTo(item);
      // 部分浏览器要求媒体元素在文档里才肯解码。`display:none` 不占布局。
      try {
        element.style.display = 'none';
        web.document.body?.append(element);
      } catch (_) {
        // 插不进 DOM 也照播（Chrome/Firefox 对游离的 audio 元素一样放行）。
      }
      return item;
    } catch (_) {
      _mediaSupported = false;
      if (url != null) {
        try {
          web.URL.revokeObjectURL(url);
        } catch (_) {
          // 回收失败也只能算了：不能因为清理失败把这一帧变成异常。
        }
      }
      return null;
    }
  }

  /// 空闲就放行队首；**还有一句在播时什么都不做**（一句一单元）。
  void _startNextIfIdle() {
    if (_disposed) return;
    final _Playback? item = _queue.takeNext();
    if (item == null) return;
    _play(item);
    _ticker ??= Timer.periodic(_levelTick, (_) => _tick());
  }

  void _play(_Playback item) {
    final web.HTMLAudioElement? element = item.element;
    if (element == null) {
      // 降级：没有媒体后端 → 用本地时钟当播放时间轴，口型照常按包络走。
      // 这是「降级不破链」的字面含义：声音没了，链路（口型 + 队列顺序）还在。
      item.clock
        ..reset()
        ..start();
      return;
    }
    _applyOutputTo(item);
    try {
      element.play().toDart.then((_) {
        if (_disposed || item.released) return;
        _setUnlocked(true);
      }).catchError((Object _) {
        // autoplay 被拒 / 元素不可播：**不崩、不吞队列**——界面重新显示
        // 「点击任意位置或按任意键以启用声音」，用户下一次手势时 [unlock] 会
        // 重试这一句（它还在队首，blob URL 也还在，没有泄漏）。
        if (_disposed || item.released) return;
        _setUnlocked(false);
      });
    } catch (_) {
      // 同步抛（媒体后端整个不可用）：跳过这一句，别卡住后面的。
      _mediaSupported = false;
      _finish(item);
    }
  }

  /// DOM `ended`：这一句自然播完。
  void _onEnded(_Playback item) => _finish(item);

  /// DOM `error`：解码/加载失败（坏 WAV、blob 被回收）。跳过这一句，链条不断。
  void _onError(_Playback item) {
    if (!_disposed) _setUnlocked(false);
    _finish(item);
  }

  /// 收尾一句：出队、释放 blob URL、让位给下一句。
  void _finish(_Playback item) {
    if (identical(_queue.current, item)) {
      _queue.finishCurrent();
      _release(item);
      _emitLevel(0);
      _startNextIfIdle();
    } else {
      // 迟到的 ended/error（例如 interrupt 已经把它摘掉）：只保证释放。
      _release(item);
    }
    if (_queue.isEmpty) _stopTicker();
  }

  /// 释放一句占用的一切：暂停、摘 DOM、**revoke blob URL**。
  void _release(_Playback item) {
    if (item.released) return;
    item.released = true;
    item.clock.stop();
    final web.HTMLAudioElement? element = item.element;
    if (element != null) {
      try {
        element.pause();
      } catch (_) {
        // 从未开始播放
      }
      try {
        element.remove();
      } catch (_) {
        // 不在 DOM 里
      }
    }
    final String? url = item.url;
    if (url != null) {
      try {
        // **顺序在播放结束之后**：blob 已经在媒体元素里解码完，此时 revoke 不会
        // 截断声音，却能立刻把这句音频从内存里放掉。
        web.URL.revokeObjectURL(url);
      } catch (_) {
        // 已回收 / 环境不支持
      }
    }
  }

  // ───────────────────────────── 口型（按播放进度） ─────────────────────────────

  /// 30 ms 轮询：按 `currentTime` 在句内包络上取值。
  ///
  /// 结束判定优先用 DOM 的 `ended` **事件**（[AudioPlayer._onEnded]），这里只做
  /// 兜底：`ended` 万一没到（异步竞态 / 元素被换掉），这一句就会永远占着队首，
  /// 后面所有句子全被卡死——比早收尾几十毫秒糟得多。
  void _tick() {
    if (_disposed) {
      _stopTicker();
      return;
    }
    final _Playback? item = _queue.current;
    if (item == null) {
      _stopTicker();
      _emitLevel(0);
      return;
    }
    final web.HTMLAudioElement? element = item.element;
    double progress;
    bool atEnd;
    bool audible;
    try {
      if (element == null) {
        // 降级句：本地时钟就是播放进度；它「本该有声」，所以口型照走。
        progress = item.clock.elapsedMilliseconds / 1000;
        atEnd = progress >= item.sentence.durationSec;
        audible = true;
      } else {
        progress = element.currentTime;
        atEnd = element.ended || progress >= _totalSec(item, element);
        // 暂停中（缓冲 / 被 autoplay 拦下）＝ 此刻没有声音 → 口型回 0，
        // 别张着嘴发呆。
        audible = !element.paused;
      }
    } catch (_) {
      _finish(item);
      return;
    }
    if (atEnd) {
      _finish(item);
      return;
    }
    _emitLevel(audible ? item.sentence.levelAt(progress) : 0);
  }

  /// 这一句的**总时长**（判定「播完了没有」用）。
  ///
  /// 首选媒体元素自己解码出的 `duration`（权威）。它要等元数据到才有值（此前是
  /// NaN），那时用封 WAV 时算出的 [AssembledSentence.durationSec] 兜底，并留
  /// 200 ms 余量——宁可多等一瞬，也不能把还没放完的一句提前掐掉（那会与下一句
  /// 重叠，正是「一句一单元」要禁止的）。
  static double _totalSec(_Playback item, web.HTMLAudioElement element) {
    final double real = element.duration;
    if (real.isFinite && real > 0) return real;
    return item.sentence.durationSec > 0
        ? item.sentence.durationSec + 0.2
        : double.infinity;
  }

  void _stopTicker() {
    _ticker?.cancel();
    _ticker = null;
  }

  void _emitLevel(double level) {
    _level = level;
    if (!_levels.isClosed) _levels.add(level);
  }

  // ───────────────────────────── 输出设置 ─────────────────────────────

  /// 把 [muted] / [volume] 写到**所有**已知元素（正在播的 + 排队的）。
  ///
  /// 静音用 `element.muted`、音量用 `element.volume`——**不是**断开输出：
  /// 这两者只影响「听不听得到」，不影响播放时间轴，所以口型与音量/静音正交。
  void _applyOutput() {
    for (final _Playback item in _queue.all) {
      _applyOutputTo(item);
    }
  }

  void _applyOutputTo(_Playback item) {
    final web.HTMLAudioElement? element = item.element;
    if (element == null) return; // 降级句：没有元素可设
    try {
      // 静音用浏览器自己的 `muted`、音量用 `volume`：站点级「声音 = 阻止」
      // 作用在媒体元素上，这两条正是本次改动的目的。
      element.muted = _muted;
      element.volume = gainForVolume(_volume);
    } catch (_) {
      // 元素已被释放：忽略。
    }
  }
}
