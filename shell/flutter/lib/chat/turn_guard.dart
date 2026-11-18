/// 一轮的**本地身份**（token + 服务端 epoch）与「作废后吞帧」状态机：
/// **纯逻辑，零 Flutter / 零 package:web 依赖**。
///
/// # 为什么需要它（2026-11-18，审计 #16 / #17 / #18）
///
/// 缺陷现场：一轮还在流式时切走会话，旧实现只把本地气泡标成结束
/// （`ChatController._abandonTurn`）、既不叫停服务端，也没有任何「这一帧属于哪
/// 一轮」的判据。于是服务端继续生成，而 `_assistant == null` 让迟到的
/// `text_delta` 走进 `_appendDelta` 的「服务端主动开口」分支——它会
/// `sessions.append` 到**当时的活动会话**：用户在 B 会话里突然看到 A 会话那一轮
/// 的后半段（第二个助手气泡，落在错误的对话里）。
///
/// 两条独立的纪律在这里合成一个显式状态机：
///
/// 1. **轮次身份**：本地开始一轮时发一个自增 token；服务端代次（epoch）从受理
///    响应 / 帧上补记。帧只有「代次对得上（或任一侧未知）」才收。
/// 2. **作废吞帧**：本地切会话 / 删会话时把当前轮移进 `abandoned`，同时叫停
///    服务端（fire-and-forget）。在服务端确认停下之前，任何证不出「属于新一代」
///    的帧都静默丢弃；确认之后那一代的代次进 `retired`——**那之后迟到的旧帧
///    仍然丢**，因为 stop 会推进 epoch，旧代次永不复用。
///
/// 为什么不把状态放在 `chat_controller.dart`：那个文件经 `ws_client.dart`
/// 依赖 `package:web`，在 `flutter test` 里加载不了；而「切会话后迟到的帧该不该
/// 收」正是那种**错了不会报错**、只会静默串会话的判据，最该有回归。
library;

/// 一帧流式载荷的去留。
enum FrameVerdict {
  /// 收：属于当前这一轮，或空闲态下由它**开启**新的一轮。
  accept,

  /// 丢：代次与当前轮不符（上一轮的迟到帧）。
  dropStale,

  /// 丢：本地已作废这一轮，服务端可能还在生成。**静默**——用户已经切走，
  /// 不该在别的会话里补字，也不该弹错。
  dropAbandoned;

  /// 这一帧要不要写进会话。
  bool get accepted => this == FrameVerdict.accept;
}

/// 一轮的生命周期身份 + 作废吞帧判据。
///
/// 用法（都在 `ChatController` 的同一代行动里）：
/// - 开始一轮 → [start] 拿 token；受理响应回来 → [noteEpoch]；
/// - 每帧载荷 → [verdictFor]；收口帧 → [completesCurrentTurn]；
/// - 切 / 删会话 → [abandon]，然后 `POST /chat/stop`，回执到了 → [onStopAcknowledged]；
/// - 服务端 `new_epoch` → [onNewEpoch]；WS 断连 → [reset]。
class TurnGuard {
  /// 自增 token 的下一个值（只在本地有意义：区分「同一轮的续帧」与「新一轮」）。
  int _nextToken = 0;

  int? _activeToken;
  int? _activeEpoch;
  String? _activeSession;

  int? _abandonedToken;
  int? _abandonedEpoch;
  String? _abandonedSession;

  /// stop 回执到了没有。**本地已确定收口**，但服务端可能还在跑——
  /// 只有「回执 + 代次都已知」才解除 pendingStop（见 [_finishAbandonIfReady]）。
  bool _stopAcked = false;

  /// 已作废 / 已被 new_epoch 作废的代次。旧代次永不复用（stop 推进 epoch），
  /// 所以这个集合可以一直留着：它挡的是「收口之后才到的旧轮帧」。
  final Set<int> _retiredEpochs = <int>{};

  /// 本地是否有一轮在飞。
  bool get inFlight => _activeToken != null;

  /// 本地作废了一轮、但服务端还没确认停下（#17：这段时间不许静默开新轮）。
  bool get pendingStop => _abandonedToken != null;

  /// 有轮在飞**或**有轮等着服务端确认停下。
  bool get busy => inFlight || pendingStop;

  /// 当前轮所属会话 id（null = 还没会话 / 全局桶）。
  String? get activeSession => _activeSession;

  /// 当前轮在服务端的代次（受理响应 / 帧上补记；不知道 = null）。
  int? get activeEpoch => _activeEpoch;

  /// 被作废那一轮所属的会话 id（诊断 / 删会话判归属用）。
  String? get abandonedSession => _abandonedSession;

  /// 已退休的代次（只读快照，测试用；生产侧不该依赖它做别的判断）。
  Set<int> get retiredEpochs => Set<int>.unmodifiable(_retiredEpochs);

  /// 本地开始一轮，返回这一轮的 token。
  ///
  /// 会清掉作废态：能合法走到这里说明新的本地轮已经确认过了（send() 的
  /// 「待停」闸门 / 新一轮的 user_line 证出了新代次）。
  int start({String? sessionId, int? epoch}) {
    _activeToken = ++_nextToken;
    _activeEpoch = epoch;
    _activeSession = sessionId;
    _clearAbandoned();
    _stopAcked = false;
    return _activeToken!;
  }

  /// 记下某一轮的受理代次（POST /api/v1/chat 的 ChatAccepted.epoch）。
  ///
  /// token 必须匹配：异步响应可能在本地已经作废这一轮之后才回来——那时它
  /// 补的是**作废轮的代次**（作废帧能不能拦得住全靠它）。
  void noteEpoch(int token, int? epoch) {
    if (epoch == null) return;
    if (_activeToken == token) {
      _activeEpoch ??= epoch;
      return;
    }
    if (_abandonedToken == token && _abandonedEpoch == null) {
      _abandonedEpoch = epoch;
      _finishAbandonIfReady();
    }
  }

  /// 帧上带回来的代次（旧服务端可能没有）：当前轮还不知道代次时补记。
  void noteFrameEpoch(int? epoch) {
    if (epoch == null) return;
    if (_activeToken != null && _activeEpoch == null) _activeEpoch = epoch;
  }

  /// 这一帧载荷（text_delta / text_fallback / user_line / reasoning_delta）
  /// 该不该收。
  ///
  /// - 有当前轮：代次对得上（任一侧未知按「对得上」，与 [belongsToCurrentTurn]
  ///   同一条宽容口径）→ 收；对不上 → 旧轮迟到帧，丢。
  /// - 有作废轮：只有**代次已知且不同**（服务端已经推进到新一代）才收，
  ///   并且顺手解除作废态——新代次出现证明旧轮不可能还活着；其余一律静默丢
  ///   （证不出来就别赌，串会话的代价远大于漏一帧外部注入）。
  /// - 空闲：已退休代次的帧丢（stop 之后迟到的旧帧），其余收（服务端主动开口 /
  ///   外部注入开启新一轮）。
  FrameVerdict verdictFor(int? frameEpoch) {
    if (_activeToken != null) {
      if (!_epochMatches(frameEpoch)) return FrameVerdict.dropStale;
      _activeEpoch ??= frameEpoch;
      return FrameVerdict.accept;
    }
    if (_abandonedToken != null) {
      final int? abandoned = _abandonedEpoch;
      // **只有代次更大**的帧才可能是「服务端已推进到新一代」：epoch 只增不减
      // （stop 时 next()），比作废代更小的帧是更早的迟到帧，同样必须丢。
      if (frameEpoch != null &&
          abandoned != null &&
          frameEpoch > abandoned &&
          !_retiredEpochs.contains(frameEpoch)) {
        _retireAbandoned();
        return FrameVerdict.accept;
      }
      return FrameVerdict.dropAbandoned;
    }
    if (frameEpoch != null && _retiredEpochs.contains(frameEpoch)) {
      return FrameVerdict.dropStale;
    }
    return FrameVerdict.accept;
  }

  /// 这一帧的收口信号（turn_state / text_delta{completed} / 致命 error）
  /// 是否属于**当前**这一轮。
  ///
  /// 空闲 / 作废态一律 false：收口帧不许凭空收掉新一轮，也不许在别的会话里
  /// 补一条收口提示。作废轮自己的收口帧到了则说明旧轮已结束——退休该代次，
  /// 让之后迟到的正文兜底帧继续被丢掉。
  bool belongsToCurrentTurn(int? frameEpoch) {
    if (_activeToken == null) return false;
    if (!_epochMatches(frameEpoch)) return false;
    _activeEpoch ??= frameEpoch;
    return true;
  }

  /// turn_state 到了：当前轮匹配 → true（调用方去收口）。
  ///
  /// 作废轮匹配 → 只做「旧轮已结束」的记账并返回 false（用户已经切走，
  /// 不该在别的会话里写任何东西）。
  bool completesCurrentTurn(int? frameEpoch) {
    if (_activeToken != null) return belongsToCurrentTurn(frameEpoch);
    if (_abandonedToken != null &&
        frameEpoch != null &&
        frameEpoch == _abandonedEpoch) {
      _retireAbandoned();
    }
    return false;
  }

  /// 正常 / 失败 / 停止收口：当前轮结束。**不退休代次**——正常完成的下一轮
  /// 沿用同一个 epoch（只有 stop 才推进），退休会把它自己的帧全挡掉。
  void settle() {
    _activeToken = null;
    _activeEpoch = null;
    _activeSession = null;
  }

  /// 本地作废当前轮（切会话 / 删拥有它的会话）。返回是否真有一轮被作废。
  ///
  /// 之后调用方必须叫停服务端（POST /api/v1/chat/stop），回执到了再
  /// [onStopAcknowledged]。
  bool abandon() {
    if (_activeToken == null) return false;
    _abandonedToken = _activeToken;
    _abandonedEpoch = _activeEpoch;
    _abandonedSession = _activeSession;
    _activeToken = null;
    _activeEpoch = null;
    _activeSession = null;
    _stopAcked = false;
    _finishAbandonIfReady();
    return true;
  }

  /// 服务端确认停止了（POST /chat/stop 的 HTTP 回执，成功或失败都算「试过了」）。
  ///
  /// 失败也解除「待停」是刻意的：后端不可达时死锁比漏一帧更糟；那一轮真正
  /// 迟到的帧仍可能被 [verdictFor] 挡住（代次已知时）。
  void onStopAcknowledged() {
    _stopAcked = true;
    _finishAbandonIfReady();
  }

  /// 这一轮的请求**本地就失败了**（网络 / 非 2xx / 200 未受理）：服务端那边
  /// 根本没开始，作废态没有存在的理由，立刻解除。
  void dropAbandonedTurn() {
    _clearAbandoned();
    _stopAcked = false;
  }

  /// 主动退休**当前代**的代次（`new_epoch` 已经证明它作废）：之后它的迟到帧
  /// 一律丢。
  ///
  /// 与 [onNewEpoch] 分开：那里还要清作废态，这里只记账——调用方通常先收口
  /// 当前轮（收口会 [settle]，代次就丢了），所以必须在收口**之前**调用。
  void retireActiveEpoch() {
    final int? epoch = _activeEpoch;
    if (epoch != null) _retiredEpochs.add(epoch);
  }

  /// 服务端推进了代次（runtime_status{event:"new_epoch"}）= 上一代整代作废。
  ///
  /// 当前轮 / 作废轮的代次都退休：new_epoch 之后它们的迟到帧一律丢。
  void onNewEpoch() {
    if (_activeEpoch != null) _retiredEpochs.add(_activeEpoch!);
    if (_abandonedEpoch != null) _retiredEpochs.add(_abandonedEpoch!);
    settle();
    _clearAbandoned();
    _stopAcked = false;
  }

  /// 实时通道断了 / 换服务端：所有身份失效（帧本来也不会重放），
  /// 免得「待停」永远把输入框挡住。
  void reset() {
    settle();
    _clearAbandoned();
    _stopAcked = false;
    _retiredEpochs.clear();
  }

  bool _epochMatches(int? frameEpoch) =>
      frameEpoch == null || _activeEpoch == null || frameEpoch == _activeEpoch;

  /// stop 回执与代次都已知 → 退休那一代并解除作废态。
  ///
  /// 只差一件事也要等：回执先到、受理响应后到（[noteEpoch]）时由那边再调一次。
  void _finishAbandonIfReady() {
    if (_abandonedToken == null) return;
    if (!_stopAcked) return;
    if (_abandonedEpoch == null) return;
    _retireAbandoned();
  }

  void _retireAbandoned() {
    final int? epoch = _abandonedEpoch;
    if (epoch != null) _retiredEpochs.add(epoch);
    _clearAbandoned();
  }

  void _clearAbandoned() {
    _abandonedToken = null;
    _abandonedEpoch = null;
    _abandonedSession = null;
  }
}

/// 「上一段会话还在回答」时的**一行原因**（#17）。
///
/// 场景：用户切走时那一轮被本地作废并叫停服务端，但服务端还没回执；此期间
/// 输入框看着可用，send() 若照旧静默 return，用户会以为「发送键坏了」。
/// 服务端一次只能跑一轮，所以这里明说为什么不能发、下一步做什么。
const String kTurnElsewhereMessage = '上一段会话还在回答，先停止它';

/// 上面对应的机器码（错误横幅的「下一步」按钮按码分流；码是可搜索的契约锚点）。
const String kTurnElsewhereCode = 'turn_elsewhere_running';
