/// 主页语音的**控制器**：产品路径（听写）与旧的常驻 / PTT 共用一套识别器
/// （纯 Dart、可注入）。
///
/// # 产品路径 = 点一下、再点一下（2026-10-08）
///
/// - **点「听」** → [startDictation]：开始录音；
/// - **再点「停」** → [stopDictation]：把定稿**交给宿主**（[onDictation]），
///   由宿主放进输入框。**不打** `/api/v1/voice/transcript`，也**不自动发送**。
///
/// 「交给宿主」的成功与失败走**同一个**回调（[onDictation]，空串 = 没听清）：
/// 面板/聊天栏据此决定「写进输入框」还是「提示再点一次」。
///
/// # 旧路径仍在（但不再接到聊天栏）
///
/// - [toggle] / [start]：常驻唤醒开/关；
/// - [pressStart] / [pressRelease]：PTT，松手提交，**不要求唤醒词**；
/// - **播报期间** → [suspendForPlayback] / [resumeAfterPlayback]：暂停听
///   （AI 自己的声音 / 环境人声会误触发，回声消除不够）。
///
/// # 一份真相在服务端（旧路径）
///
/// 前端本地匹配只决定「要不要发」；真正的门控是 Rust 的
/// `live2d_ai_mod_voice_input::gate::evaluate_mode`：
/// - 常驻：定稿**以唤醒词开头**才当「在叫角色」（句首锚定，P0-4）；
/// - PTT：提交时带 `ptt: true`，服务端对**该请求**跳过唤醒匹配。
library;

import 'dart:async';

import 'package:flutter/foundation.dart';

import '../api/voice_api.dart';
import 'speech_recognizer.dart';

// 私有字段 + 命名参数无法用 this._x 初始化（Dart 不允许下划线开头的命名参数），
// ignore_for_file: prefer_initializing_formals

/// 新装 / 配置缺 `wake_phrase` 时的产品缺省唤醒词。
///
/// **必须与 Rust 侧 `live2d_ai_mod_voice_input::gate::DEFAULT_WAKE_PHRASE`
/// 逐字一致**——两边默认值漂移会让「按钮说小可爱、服务端拒小可爱」。
/// 回归在 `test/voice_wake_default_consistency_test.dart`。
const String kDefaultWakePhrase = '小可爱';

/// 点按 / 按住的判定阈值（UI 层用；见 `ui/chat_panel.dart`）。
const Duration kVoiceTapThreshold = Duration(milliseconds: 150);

/// Web Speech 模式的**诚实说明**（P0-4：UI 必须明说）。
///
/// 这不是本地识别：语音会离开本机（送浏览器厂商云端），且**必须联网**。
/// 离线 / 本地 ASR 是 P1/P2，不在本轮——文案不能让人以为本地可用。
const String kVoiceWebSpeechNote =
    '此模式需联网：语音会送浏览器厂商云端识别（音频会出本机），结果再回本机后端。';

/// 送一段转写（返回服务端结论）。[ptt] = 按住说话（服务端跳过唤醒匹配）。
typedef VoiceTranscriptSend =
    Future<VoiceTranscriptResult> Function(String text, {bool ptt});

/// 读当前生效的唤醒词（来自 voice-input Mod 的 config；失败给缺省）。
typedef WakePhraseLoader = Future<String> Function();

/// 读 voice-input Mod 是否已启用（`null` loader = 不做这道预检）。
typedef VoiceModEnabledLoader = Future<bool> Function();

/// **Mod 未启用**时的可执行提示（L1，2026-09-16）。
///
/// 主界面「听」按钮旁的常驻红字与控制器里的预检共用这一份文案：以前
/// Mod 没启用时界面只显示识别到的字，用户以为在听、其实端点一直 403
/// mod_disabled——必须**先**说清「去启用它」。
const String kVoiceModDisabledMessage =
    '语音输入未启用：请先在「设置 → Mod 管理」启用 voice-input（启停后可能需重新点火）';

/// 识别结果落回宿主的回调（busy / 听写定稿都把正文交回输入框）。
///
/// 听写路径**成功与失败都调它**：空串 = 没听清（宿主不动输入框）。
typedef VoiceResultSink = void Function(String text);

class VoiceListenController extends ChangeNotifier {
  VoiceListenController({
    required SpeechRecognizer? recognizer,
    required VoiceTranscriptSend send,
    required WakePhraseLoader loadWakePhrase,
    VoiceModEnabledLoader? loadModEnabled,
    this.onBusyResult,
    this.onDictation,
    this.restartDelay = const Duration(milliseconds: 500),
    this.wakeWindow = const Duration(seconds: 8),
    this.pttFinalizeDelay = const Duration(milliseconds: 300),
  }) : _recognizer = recognizer,
       _send = send,
       _loadWakePhrase = loadWakePhrase,
       _loadModEnabled = loadModEnabled;

  final SpeechRecognizer? _recognizer;
  final VoiceTranscriptSend _send;
  final WakePhraseLoader _loadWakePhrase;

  /// voice-input Mod 启停预检（缺省不检查；服务端 403 仍是兜底真源）。
  final VoiceModEnabledLoader? _loadModEnabled;

  /// 主链忙（`200 ok:false`）时把识别到的正文交回宿主（落输入框）。
  final VoiceResultSink? onBusyResult;

  /// **产品听写路径的定稿**：成功与失败（空串）都走它。
  ///
  /// 宿主把它写进输入框；空串时宿主什么都不做（控制器已经写了
  /// 「没听清，再点一次说」）。
  final VoiceResultSink? onDictation;

  /// 一次识别会话结束后的重启延迟（防抖：避免 `onEnd` 立刻重启打转）。
  final Duration restartDelay;

  /// 「已唤醒、等正文」的窗口；超时清掉，避免无关的话被当成正文。
  final Duration wakeWindow;

  /// 松手后等最后一份定稿的短窗口。
  ///
  /// Web Speech 的 `stop()` 在 Chrome 上不保证立刻交稿，而 `abort()` 会**丢弃**
  /// 结果——所以等一小段再提交；等不到就用已收到的正文（可能不完整）。
  final Duration pttFinalizeDelay;

  bool _continuous = false;
  bool _recognizing = false;
  bool _ptt = false;
  /// 产品听写（点「听」→ 点「停」）是否正在录音。
  bool _dictating = false;
  bool _suspended = false;
  bool _disposed = false;
  bool _sending = false;
  String _wakePhrase = kDefaultWakePhrase;
  /// 已听到唤醒词（等待正文）。
  bool _armed = false;
  /// 「已唤醒」后累积的正文。
  String _body = '';
  /// PTT 期间累积的正文。
  String _pttBody = '';
  /// 听写期间累积的定稿正文。
  String _dictationBody = '';
  String _interim = '';
  String? _error;
  String? _lastSent;
  Timer? _restartTimer;
  Timer? _armTimer;

  /// 这个构建是否有语音识别能力（VM / 不支持的浏览器 → false）。
  bool get supported => _recognizer != null;

  /// 按钮的「听 / 停」态：常驻唤醒**或**产品听写正在进行。
  bool get listening => _continuous || _dictating;

  /// 产品听写是否正在录音。
  bool get dictating => _dictating;

  /// PTT 是否正在按住。
  bool get pttActive => _ptt;

  /// 是否因播报而暂停听（P0-4）。
  bool get suspendedForPlayback => _suspended;

  /// 当前生效的唤醒词（缺省「小可爱」）。
  String get wakePhrase => _wakePhrase;

  /// 最近一次可读错误（权限被拒 / 没有麦克风 / 服务不可达…）。
  String? get error => _error;

  /// 按钮旁那一行状态（没有活动会话时为 null，不占位）。
  String? get statusLine {
    if (_dictating) return '正在听，说完再点一次';
    if (_ptt) {
      return _interim.isEmpty ? '按住说话：说完松手发送…' : '听到：「$_interim」';
    }
    if (!_continuous) return null;
    if (_suspended) return '角色在说话：先暂停听，说完自动继续';
    final String? sent = _lastSent;
    if (sent != null) return '已发送：「$sent」';
    if (_armed) return '听到了，接着说…';
    if (_interim.isNotEmpty) return '听到：「$_interim」';
    return '在听：说「$_wakePhrase ……」';
  }

  // ───────────────────────────────────────────────── 产品听写（点按两态）

  /// 产品「听」按钮：没在录就开录，正在录就结束并把定稿交给宿主。
  Future<void> toggleDictation() =>
      _dictating ? stopDictation() : startDictation();

  /// 开始一次听写。
  ///
  /// - **角色正在播报时不允许开始**：就地写「角色在说话，说完再听」，不开麦；
  /// - 这条路径**不打语音端点**，所以不做「Mod 未启用」预检
  ///   （那道的意义是别让用户白说一遍 403，而这里根本不发请求）。
  Future<void> startDictation() async {
    if (_disposed || _dictating) return;
    final SpeechRecognizer? recognizer = _recognizer;
    if (recognizer == null) {
      _error = '这个构建没有语音识别（需要桌面版 Chrome / Edge）';
      _safeNotify();
      return;
    }
    if (_suspended) {
      _error = '角色在说话，说完再听';
      _safeNotify();
      return;
    }
    _error = null;
    _lastSent = null;
    _disarm();
    _interim = '';
    _dictationBody = '';
    _dictating = true;
    _safeNotify();
    await _begin(recognizer);
  }

  /// 结束听写：等一小段最后定稿，然后把定稿交给宿主（成功与失败都走同一个回调）。
  Future<void> stopDictation() async {
    if (!_dictating) return;
    _recognizing = false;
    await _recognizer?.stop();
    // 与 PTT 同一条理由：Web Speech 的 `stop()` 不保证立刻交稿，
    // 而 `abort()` 会丢结果——等一小段再用已收到的正文。
    if (pttFinalizeDelay > Duration.zero) {
      await Future<void>.delayed(pttFinalizeDelay);
    }
    _finishDictation();
  }

  /// 收尾：算定稿、写文案、交给宿主。可重入安全（`_dictating` 已经是 false 就返回）。
  void _finishDictation() {
    if (!_dictating) return;
    _dictating = false;
    _interim = '';
    final String body = _dictationBody.trim();
    _dictationBody = '';
    if (body.isEmpty) {
      _error = '没听清，再点一次说';
    } else {
      _error = null;
      _lastSent = body;
    }
    // **成功和失败都走它**：宿主拿空串就什么都不做（输入框不动）。
    onDictation?.call(body);
    _safeNotify();
  }

  // ─────────────────────────────────────────────────────── 常驻（点按）

  /// 开始 / 停止「听」（常驻唤醒）。**产品聊天栏已不再接它**（2026-10-08）。
  Future<void> toggle() => _continuous ? stop() : start();

  /// 开始常驻听。
  Future<void> start() async {
    if (_disposed || _continuous) return;
    final SpeechRecognizer? recognizer = _recognizer;
    if (recognizer == null) {
      _error = '这个构建没有语音识别（需要桌面版 Chrome / Edge；设置里的「验证闸门」仍可手动注入）';
      _safeNotify();
      return;
    }
    _error = null;
    // Mod 未启用：**先**给可执行的红字，不要等到说了话才由 403 回来
    // （那会让用户以为「识别了」）。
    final String? blocked = await _voiceModBlockReason();
    if (_disposed) return;
    if (blocked != null) {
      _error = blocked;
      _safeNotify();
      return;
    }
    _lastSent = null;
    _disarm();
    _interim = '';
    await _reloadWakePhrase();
    if (_disposed) return;
    _continuous = true;
    _safeNotify();
    if (!_suspended && !_ptt) await _begin(recognizer);
  }

  /// 主动停止（用户按「停止听」）。
  Future<void> stop() async {
    if (!_continuous && !_recognizing && !_ptt && !_dictating) return;
    _continuous = false;
    _ptt = false;
    _pttBody = '';
    _dictating = false;
    _dictationBody = '';
    _restartTimer?.cancel();
    _restartTimer = null;
    _disarm();
    _interim = '';
    _recognizing = false;
    _safeNotify();
    await _recognizer?.stop();
  }

  // ─────────────────────────────────────────────────────── PTT（按住）

  /// 按住开始：PTT 优先于常驻（常驻会话先停，松手后若仍开着会自动恢复）。
  Future<void> pressStart() async {
    if (_disposed || _ptt) return;
    final SpeechRecognizer? recognizer = _recognizer;
    if (recognizer == null) {
      _error = '这个构建没有语音识别（需要桌面版 Chrome / Edge）';
      _safeNotify();
      return;
    }
    _error = null;
    // 同一道预检：PTT 也走 voice-input 端点（手动闸 / 总闸不变）。
    final String? blocked = await _voiceModBlockReason();
    if (_disposed) return;
    if (blocked != null) {
      _error = blocked;
      _safeNotify();
      return;
    }
    _lastSent = null;
    _disarm();
    _interim = '';
    _pttBody = '';
    _ptt = true;
    _safeNotify();
    if (_recognizing) {
      _recognizing = false;
      await recognizer.stop();
    }
    if (_disposed || !_ptt) return;
    await _reloadWakePhrase();
    if (_disposed || !_ptt) return;
    await _begin(recognizer);
  }

  /// 松手提交：等一小段最后定稿，然后以 `ptt:true` 送正文（不要求唤醒词）。
  Future<void> pressRelease() async {
    if (!_ptt) return;
    _recognizing = false;
    await _recognizer?.stop();
    // 等最后定稿时 `_ptt` 仍为 true —— 迟到的定稿照收（`_onResult` 的 PTT 分支）。
    if (pttFinalizeDelay > Duration.zero) {
      await Future<void>.delayed(pttFinalizeDelay);
    }
    _ptt = false;
    final String body = _pttBody.trim();
    _pttBody = '';
    if (body.isEmpty) {
      _error = '没听清（按住说话：按住不放，说完再松手）';
    } else {
      _sendNow(body, ptt: true);
    }
    _safeNotify();
    _maybeResumeContinuous();
  }

  // ────────────────────────────────────── 播报期间暂停听（P0-4）

  /// 角色开始播报 → 暂停听（不取消常驻意图；播报结束自动恢复）。
  Future<void> suspendForPlayback() async {
    if (_disposed || _suspended) return;
    _suspended = true;
    _restartTimer?.cancel();
    _restartTimer = null;
    _disarm();
    _interim = '';
    if (_dictating) {
      // 播报开始时**停掉听写**，但已经听到的字仍要落到输入框（2026-10-08）。
      _recognizing = false;
      await _recognizer?.stop();
      _finishDictation();
      return;
    }
    if (_recognizing && !_ptt) {
      _recognizing = false;
      await _recognizer?.stop();
    }
    _safeNotify();
  }

  /// 播报结束 → 恢复常驻听（PTT 进行中不抢）。
  Future<void> resumeAfterPlayback() async {
    if (_disposed || !_suspended) return;
    _suspended = false;
    _safeNotify();
    if (!_ptt) _maybeResumeContinuous();
  }

  void _maybeResumeContinuous() {
    if (_disposed || _suspended || _ptt || !_continuous || _recognizing) return;
    final SpeechRecognizer? recognizer = _recognizer;
    if (recognizer != null) unawaited(_begin(recognizer));
  }

  // ─────────────────────────────────────────────────────── 内部

  /// voice-input Mod 是否启用；未启用 → 可执行提示，否则 `null`。
  ///
  /// 读失败（服务/网络）**不在这里拦**：真发出去时服务端仍会如实回 403，
  /// 那种错误由 [error] 显示。缺 loader = 不检查（旧接线 / 单测）。
  Future<String?> _voiceModBlockReason() async {
    final VoiceModEnabledLoader? load = _loadModEnabled;
    if (load == null) return null;
    try {
      if (!await load()) return kVoiceModDisabledMessage;
    } catch (_) {
      // 见上：不误报，交给端点兜底。
    }
    return null;
  }

  Future<void> _reloadWakePhrase() async {
    try {
      final String phrase = (await _loadWakePhrase()).trim();
      _wakePhrase = phrase.isNotEmpty ? phrase : kDefaultWakePhrase;
    } catch (_) {
      _wakePhrase = kDefaultWakePhrase;
    }
  }

  Future<void> _begin(SpeechRecognizer recognizer) async {
    if (_disposed || _suspended) return;
    if (!_continuous && !_ptt && !_dictating) return;
    _recognizing = true;
    await recognizer.start(
      lang: 'zh-CN',
      onResult: _onResult,
      onError: _onError,
      onEnd: _onEnd,
    );
  }

  void _onResult(SpeechResult result) {
    if (!_recognizing && !_ptt && !_dictating) return;
    if (!result.isFinal) {
      _interim = result.transcript;
      _safeNotify();
      return;
    }
    _interim = '';
    final String transcript = result.transcript;
    if (_dictating) {
      // 听写：所有定稿都算正文（不看唤醒词——这条路径根本不打端点）。
      _dictationBody = '$_dictationBody$transcript'.trim();
      _safeNotify();
      return;
    }
    if (_ptt) {
      // PTT：所有定稿都算正文（不要求唤醒词）；服务端跳过唤醒匹配。
      _pttBody = '$_pttBody$transcript'.trim();
      _safeNotify();
      return;
    }
    if (_armed) {
      // 已唤醒：后续定稿就是正文（可能分多条）。
      _body = '$_body$transcript'.trim();
    } else if (containsWakePhrase(transcript, _wakePhrase)) {
      final String body = wakeBody(transcript, _wakePhrase);
      if (body.isNotEmpty) {
        // 唤醒词 + 正文在同一条定稿 → 直接送**原文**（服务端剥词）。
        _sendNow(transcript);
        _safeNotify();
        return;
      }
      // 只叫了唤醒词：进入「等正文」窗口。
      _arm();
      _safeNotify();
      return;
    } else {
      // 没有唤醒词的闲聊：不累积（不把无关的话拼起来发）。
      _safeNotify();
      return;
    }
    _maybeSend();
    _safeNotify();
  }

  /// 进入「已唤醒、等正文」状态；超时自动放弃。
  void _arm() {
    _armed = true;
    _body = '';
    _armTimer?.cancel();
    _armTimer = Timer(wakeWindow, _disarm);
  }

  void _disarm() {
    _armed = false;
    _body = '';
    _armTimer?.cancel();
    _armTimer = null;
  }

  /// 已唤醒且累积到正文 → 发 `唤醒词 + 正文`（端点要求含唤醒词）。
  void _maybeSend() {
    if (_sending || !_armed) return;
    if (_body.isEmpty) return;
    _sendNow('$_wakePhrase$_body');
  }

  void _sendNow(String text, {bool ptt = false}) {
    _sending = true;
    _disarm();
    unawaited(_deliver(text, ptt: ptt));
  }

  Future<void> _deliver(String text, {bool ptt = false}) async {
    try {
      final VoiceTranscriptResult result = await _send(text, ptt: ptt);
      if (_disposed) return;
      if (result.ok) {
        _error = null;
        _lastSent = result.text.isEmpty ? text : result.text;
      } else {
        // 200 + ok:false（主链忙）是可退避的正常结果，如实说。
        // **不排队**：把识别到的正文交回宿主（落输入框），让用户改字重发。
        final String code = result.code ?? 'busy';
        final String detail = result.message ?? '稍后再试';
        // busy 是**可退避**的正常结果，文案必须说清「东西没丢、在哪」——
        // 只说「busy」用户会以为白说了（下面把正文落回输入框）。
        _error = code == 'busy'
            ? '主链忙（busy）：角色还在说话，本条已填回输入框，改字后再发'
            : '没送进主链（$code）：$detail';
        final VoiceResultSink? sink = onBusyResult;
        if (sink != null) sink(result.text.isNotEmpty ? result.text : text);
      }
    } catch (e) {
      if (!_disposed) _error = '$e';
    } finally {
      _sending = false;
      if (!_disposed) _safeNotify();
    }
  }

  void _onError(String message, bool fatal) {
    if (_disposed) return;
    _error = message;
    if (fatal) {
      _continuous = false;
      _ptt = false;
      _pttBody = '';
      _recognizing = false;
      _restartTimer?.cancel();
      _restartTimer = null;
      _disarm();
      // 听写被识别器判死：听到多少交多少（失败也走宿主的那个回调）。
      if (_dictating) {
        final String heard = _dictationBody.trim();
        _dictating = false;
        _dictationBody = '';
        if (heard.isNotEmpty) onDictation?.call(heard);
      }
    }
    _safeNotify();
  }

  void _onEnd() {
    if (_disposed) return;
    _recognizing = false;
    // 播报暂停 / PTT 进行中 / 用户已停止 → 不自动重启。
    if (_suspended || _ptt || !_continuous) return;
    // 会话被浏览器结束（静音超时）→ 重启，形成常态监听。
    _restartTimer?.cancel();
    _restartTimer = Timer(restartDelay, () {
      if (_disposed || !_continuous || _suspended || _ptt || _recognizing) return;
      _maybeResumeContinuous();
    });
  }

  void _safeNotify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _restartTimer?.cancel();
    _armTimer?.cancel();
    unawaited(_recognizer?.dispose());
    super.dispose();
  }
}

/// 把一次听写定稿接到输入框已有文字的后面（**中间补一个空格**）。纯函数，可单测。
///
/// - [incoming] trim 后为空 → **原样返回** [existing]（输入框不动）；
/// - [existing] trim 后为空 → 就是 trim 过的 [incoming]；
/// - 否则 = `existing + ' ' + incoming`（光标由调用方落到末尾）。
String appendVoiceText(String existing, String incoming) {
  final String body = incoming.trim();
  if (body.isEmpty) return existing;
  return existing.trim().isEmpty ? body : '$existing $body';
}

/// 文本是否**以**唤醒词开头（忽略前导空白、空白差异、大小写）。纯函数，可单测。
///
/// 句首锚定（P0-4）：旧行为是任意位置子串命中，于是「我昨天说小可爱好看」
/// 会被误当成在叫角色，并把中间那个词挖掉。现在只认句首。
bool containsWakePhrase(String text, String phrase) =>
    _wakeSpan(text, phrase) != null;

/// 唤醒词之后的正文（剥词 + 去首尾分隔符 + 折叠空白）；没命中 → 空串。纯函数。
///
/// **只用于「有没有正文」的判断**：真正的剥词 / 归一化在服务端（一份真相）。
String wakeBody(String text, String phrase) {
  final (int, int)? span = _wakeSpan(text, phrase);
  if (span == null) return '';
  final List<int> runes = text.runes.toList();
  final List<int> rest = <int>[
    ...runes.sublist(0, span.$1),
    ...runes.sublist(span.$2),
  ];
  // 唤醒词在句首：把紧随其后的标点 / 空白一起去掉。
  final String body = String.fromCharCodes(
    rest,
  ).replaceFirst(RegExp(r'^[\s\p{P}]+', unicode: true), '');
  // 折叠剥词后留下的多余空白（与 Rust 侧 clean_transcript 同口径）。
  return body.trim().replaceAll(RegExp(r'\s+'), ' ');
}

/// 找唤醒词在文本**句首**的**字符区间**（忽略前导空白、空白差异、大小写）。纯函数。
///
/// 只从第一个非空白字符试一次；出现在中间 → `null`。
(int, int)? _wakeSpan(String text, String phrase) {
  final List<int> hay = text.runes.toList();
  final List<int> needle = phrase.runes
      .where((int c) => !_isWhitespace(c))
      .toList();
  if (needle.isEmpty) return null;
  int start = 0;
  while (start < hay.length && _isWhitespace(hay[start])) {
    start++;
  }
  if (start >= hay.length) return null;
  int pi = 0;
  int i = start;
  while (i < hay.length && pi < needle.length) {
    final int c = hay[i];
    if (_isWhitespace(c)) {
      i++;
      continue;
    }
    if (_lower(c) == _lower(needle[pi])) {
      pi++;
      i++;
    } else {
      break;
    }
  }
  return pi == needle.length ? (start, i) : null;
}

bool _isWhitespace(int rune) => String.fromCharCode(rune).trim().isEmpty;

String _lower(int rune) => String.fromCharCode(rune).toLowerCase();
