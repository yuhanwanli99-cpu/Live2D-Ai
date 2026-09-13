/// 界面状态跟踪器：把 WS 信号 + 渲染面状态收敛成 [UiSignals]。
///
/// # 为什么需要它（而不是在 `main.dart` 里拼）
///
/// `ui_phase.dart` 是纯函数，但「**信号从哪来、什么时候清**」同样是逻辑，
/// 而且是更容易错的一半：`voice_started` 之后谁负责把它清掉？`new_epoch`
/// 的打断窗口由谁计时？留在 `main.dart` 里就永远测不到——而 `main.dart`
/// 必须 `import 'package:web'`（localStorage / Live2D iframe）。
///
/// 本文件**不碰 `package:web`**：它只消费已经解析好的 [WsEvent] 与 [WsStatus]，
/// 于是可以喂真实抓包的帧、断言派生出的 [UiPhase]。
///
/// # 错误文案的**唯一**格式（2026-09-11）
///
/// `error` 帧有两条消费路径：本类（顶部横幅，在 `main.dart` 里优先级更高）
/// 与 `ChatController`（聊天面板）。两边都调 `formatWsError`，保证
/// 「码 + 说明 + 提示」在任何一处上屏时都不丢——过去本类只放 `message`，
/// 恰好把 `code` 丢在了用户真正看得见的那条路径上。
///
/// # 定时器可注入
///
/// 「已打断」是一个**有保持期的瞬时状态**（`kInterruptedHold`）。为了让测试
/// 不依赖真实时钟，定时器通过 [TimerFactory] 注入；生产代码用默认实现。
library;

import 'dart:async';

import 'package:flutter/foundation.dart';

import '../api/ws_frame.dart';
import '../api/ws_status.dart';
import 'ui_phase.dart';

/// 定时器工厂（可注入以便测试确定性地推进「打断保持期」）。
typedef TimerFactory = Timer Function(Duration delay, void Function() callback);

Timer _defaultTimerFactory(Duration delay, void Function() callback) =>
    Timer(delay, callback);

class UiStateTracker extends ChangeNotifier {
  UiStateTracker({
    Duration hold = kInterruptedHold,
    TimerFactory? timerFactory,
    // 具名参数不写成 `this._hold`：那会把私有字段名暴露进公开签名
    // （调用方要写 `UiStateTracker(_hold: …)`）。这里保持 `hold:` → `_hold`。
    // ignore: prefer_initializing_formals
  }) : _hold = hold,
       _newTimer = timerFactory ?? _defaultTimerFactory;

  final Duration _hold;
  final TimerFactory _newTimer;

  WsStatus _wsStatus = WsStatus.idle;
  bool _turnActive = false;
  bool _voiceActive = false;
  bool _interrupted = false;
  bool _errorActive = false;
  String? _errorMessage;
  String? _errorCode;
  Timer? _interruptTimer;

  /// 已释放。
  ///
  /// 定时器回调里必须查它：`Timer.cancel()` 能阻止**尚未触发**的回调，但
  /// 回调**已经排进事件队列**时取消是无效的。在已 dispose 的 `ChangeNotifier`
  /// 上调用 `notifyListeners()` 会直接断言失败——生产环境里这会变成
  /// 「停止对话后偶发崩溃」（时序相关，最难查的一类）。
  bool _disposed = false;

  /// 当前信号快照。
  UiSignals get signals => UiSignals(
    wsConnected: _wsStatus.isUsable,
    turnActive: _turnActive,
    voiceActive: _voiceActive,
    interrupted: _interrupted,
    errorActive: _errorActive,
  );

  /// 派生出的界面相位（**唯一**真源，UI 各处都读它）。
  UiPhase get phase => deriveUiPhase(signals);

  WsStatus get wsStatus => _wsStatus;
  bool get voiceActive => _voiceActive;
  bool get turnActive => _turnActive;
  bool get interrupted => _interrupted;
  bool get errorActive => _errorActive;

  /// 最近一条错误文案（`error` 帧 / `turn_state: failed`）。
  ///
  /// **含错误码**：由 `formatWsError` 拼成 `code：message（hint）`。
  /// 2026-09-11 修：这里原先只放 `message`，而本字段在 `main.dart` 里
  /// **优先于** `ChatController.error` 上屏——于是即便后端把码发过来了，
  /// 用户看到的仍是「上游非成功状态 401」这种没有码的散文（用户原话：
  /// 「后端出错无具体错误代码」「前端无法知道错误信息」）。
  String? get errorMessage => _errorMessage;

  /// 最近一条错误的机器码（与 [errorMessage] 成对；供「下一步」按钮分流）。
  String? get errorCode => _errorCode;

  // ── 输入 ──────────────────────────────────────────────────────────

  /// WS 连接状态变化。
  void onWsStatus(WsStatus status) {
    if (_wsStatus == status) return;
    _wsStatus = status;
    if (!status.isUsable) {
      // 断开就把「进行中」的信号一并清掉：否则重连后界面会停在
      // 「说话中/思考中」而实际什么都没发生。
      _turnActive = false;
      _voiceActive = false;
      _clearInterrupt();
    }
    notifyListeners();
  }


  /// `POST /api/v1/chat` 已受理 → 本轮开始。
  void markTurnAccepted() {
    if (_turnActive) return;
    _turnActive = true;
    _errorActive = false; // 新一轮开始先收掉上一轮的错
    _errorMessage = null;
    _errorCode = null;
    notifyListeners();
  }

  /// 消费一帧（`ChatController` 之外的第二条订阅；只读，无副作用外溢）。
  bool consume(WsEvent event) {
    switch (event) {
      case RuntimeStatusEvent(:final event):
        switch (event) {
          case 'voice_started':
            _voiceActive = true;
            _turnActive = true; // 出声必然在本轮内
            notifyListeners();
            return true;
          case 'voice_ended':
            _voiceActive = false;
            notifyListeners();
            return true;
          case 'new_epoch':
            _enterInterrupted();
            return true;
          default:
            return false;
        }
      case TextDeltaEvent(:final completed):
        if (completed != null) {
          _turnActive = false;
          _voiceActive = false;
          notifyListeners();
          return true;
        }
        return false;
      // 思考**不改任何相位**：它是旁注，不代表「本轮已开始出声」或「已收到正文」。
      // 若把「正在思考」当成 turnActive/voiceActive，状态胶囊会在正文到来前
      // 就亮起「正在回复」，而那时一个字都还没有。
      case ReasoningDeltaEvent():
        return false;
      case TurnStateEvent(:final status):
        _turnActive = false;
        _voiceActive = false;
        if (status == 'failed') {
          _errorActive = true;
          // 已经收到过带码的 `error` 帧就不要覆盖它——那帧信息更全。
          // 没有的话老实说「没有详情」，并给出去处（诊断日志）。
          _errorMessage ??= '本轮生成失败：服务端未给出错误详情'
              '（可看「设置 → 开发模式 → 诊断日志」）';
          _errorCode ??= 'turn_failed_no_detail';
        }
        notifyListeners();
        return true;
      case WsErrorEvent(:final message, :final code, :final hint):
        _errorActive = true;
        // 与 `ChatController` 用**同一个**格式化函数：两条订阅路径上屏文案
        // 一致，不会出现「聊天面板有码、顶部横幅没码」这种分裂。
        _errorMessage = formatWsError(code: code, message: message, hint: hint);
        _errorCode = code;
        notifyListeners();
        return true;
      case SubscribeAckEvent():
      case HeartbeatEvent():
      case AudioEvent():
      case UnknownWsEvent():
        return false;
    }
  }

  /// 用户点了「停止」：本地立刻收口（不等服务端 `new_epoch`）。
  ///
  /// 与 `new_epoch` 走同一条语义（旧轮作废 + 显示「已打断」），但**不**合成
  /// 一个假的协议帧——那会让「状态从哪来」变得说不清（测试里也看不出区别）。
  void markStopped() => _enterInterrupted();

  /// 用户手动关掉错误提示。
  void clearError() {
    if (!_errorActive && _errorMessage == null) return;
    _errorActive = false;
    _errorMessage = null;
    _errorCode = null;
    notifyListeners();
  }

  /// 进入「已打断」：旧轮作废，保持 [UiStateTracker._hold] 后自动回落。
  void _enterInterrupted() {
    _voiceActive = false;
    _turnActive = false;
    _interrupted = true;
    _restartInterruptTimer();
    notifyListeners();
  }

  /// 重置打断保持期。
  ///
  /// **注意**：这里只取消定时器，**不**动 `_interrupted` 标志。
  /// 第一版把「取消定时器」和「清掉打断态」合成一个 `_cancelInterrupt()`，
  /// 于是 `new_epoch` 分支里刚设上的 `_interrupted = true` 立刻被自己抹掉——
  /// 相位永远是 `idle`，「已打断」这个状态**一次都显示不出来**。
  /// 测试直接抓到了（`Expected: interrupted, Actual: idle`）。
  void _restartInterruptTimer() {
    _interruptTimer?.cancel();
    _interruptTimer = _newTimer(_hold, () {
      _interruptTimer = null;
      if (_disposed || !_interrupted) return;
      _interrupted = false;
      notifyListeners();
    });
  }

  /// 清掉打断态（断开连接时用）。
  void _clearInterrupt() {
    _interruptTimer?.cancel();
    _interruptTimer = null;
    _interrupted = false;
  }

  @override
  void dispose() {
    _disposed = true;
    _interruptTimer?.cancel();
    _interruptTimer = null;
    super.dispose();
  }
}
