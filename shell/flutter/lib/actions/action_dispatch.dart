/// 动作触发的**本地状态**：幂等提示 + 「正在演」游标。
///
/// # 为什么需要它（服务端回答不了这两个问题）
///
/// 1. **重复点击**：core 对「动作 + 强度 + 来源完全相同」返回
///    `Dropped(AlreadyActive)`（幂等），但 **HTTP 响应里看不出来**——实测两次
///    invoke 的响应一字不差（都是 `accepted: true`）。用户看到的是
///    「点了没反应且没有任何解释」。只能在本地比对最近一次 invoke。
/// 2. **「正在演」**：可靠信号不是 invoke 的响应，而是 WS 上的
///    `action_state{kind:"perform", source:"user_command"}`——它经过 core 仲裁，
///    是**真的发生了**。所以按钮的忙碌态由 WS 驱动，而不是由 HTTP 响应驱动。
///
/// 纯逻辑、无 web 依赖、时钟可注入 → 可确定性地单测。
library;

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'action_event.dart';

/// 渲染面关键帧表的**总时长**（毫秒），动作名 → 时长。
///
/// 取值来源：`l2d-wasm-demo/src/web/surface.rs` 的 `choreography_total_ms`
/// 及其自带断言（`assert!((choreography_total_ms("nod") - 1320.0).abs() < 1e-3)`
/// 等，surface.rs:1590-1597）。**未知动作回落 1500 ms**（渲染面的兜底值）。
///
/// # 为什么前端要抄一份时长
///
/// **服务端目前永远不发 `cease`**：`ActionCommand::Release` 在全仓库
/// **没有任何调用点**（桌面包与 director Mod 里只有 `Play`），
/// 而 `cease` 帧由 `ActionEffect::End` 驱动 → 手动/规则触发的动作**收不了口**。
/// 实测：invoke 之后监听 30 秒，只收到一条 `perform`，**没有 `cease`**。
///
/// 所以「正在演」如果只等 `cease`，会**永久卡住**——按钮一直是「正在演」，
/// 动作日志的当前项永远不结束。这是必须在前端兜住的事，
/// 而兜底的时长**不该是猜的**：渲染面自己就有一张表，抄它、并注明出处。
///
/// **它不是权威**：真收到 `cease` 时立刻以 `cease` 为准（见 [ActionDispatch.consume]）。
const Map<String, int> kActionDurationMs = <String, int>{
  'nod': 1320,
  'shake_no': 1000,
  'tilt': 1500,
  'look_around': 1920,
  'listen': 2180,
  'surprise': 1300,
};

/// 未知动作的兜底时长（与渲染面 `choreography_total_ms` 的兜底一致）。
const int kUnknownActionDurationMs = 1500;

/// 本地兜底要额外留的余量。
///
/// 渲染面的时长是「末帧 + 尾 fade」，实际停下来的时刻会略晚于它；
/// 再加一点网络/调度抖动。取得小一点（不要为了保险留 1 秒，
/// 否则「正在演」会比动作本身长很多，用户会觉得按钮卡住了）。
const Duration kActionExpirySlack = Duration(milliseconds: 250);

/// 某个动作的兜底总时长。
Duration actionDurationFor(String action) => Duration(
  milliseconds:
      (kActionDurationMs[action] ?? kUnknownActionDurationMs) +
      kActionExpirySlack.inMilliseconds,
);

/// 定时器工厂（可注入，便于确定性测试）。
typedef DispatchTimerFactory = Timer Function(Duration, void Function());

/// 一次本地 invoke 的记录。
@immutable
class InvokeRecord {
  const InvokeRecord({
    required this.action,
    required this.strength,
    required this.at,
  });

  final String action;
  final int? strength;
  final DateTime at;

  /// 与另一次 invoke 是否「同一动作 + 同一强度」。
  bool sameAs(String action, int? strength) =>
      this.action == action && this.strength == strength;
}

/// 动作触发的本地状态机。
class ActionDispatch extends ChangeNotifier {
  ActionDispatch({
    DateTime Function()? now,
    DispatchTimerFactory? timerFactory,
    this.duplicateWindow = kDuplicateWindow,
    this.useLocalExpiry = true,
  }) : _now = now ?? DateTime.now,
       _newTimer = timerFactory ?? _defaultTimer;

  static Timer _defaultTimer(Duration d, void Function() cb) => Timer(d, cb);

  final DispatchTimerFactory _newTimer;

  /// 是否启用「本地兜底过期」（见 [kActionDurationMs] 的说明）。
  ///
  /// 生产必须为 `true`——服务端不发 `cease`，关掉它「正在演」就会永久卡住。
  /// 可关是为了在测试里单独验证「只靠 WS 驱动」的那条路径。
  final bool useLocalExpiry;

  /// 「同一动作正在播放」的判定窗口。
  ///
  /// 取值理由：core 的 AlreadyActive 覆盖的是**当前正在演的那一个**动作。
  /// 2 s 比多数动作的编排时长短，所以这个窗口是**保守**的（宁可漏提示，
  /// 不要误报「正在播放」而其实已经演完）。真正的权威仍是 WS 的 `cease`。
  static const Duration kDuplicateWindow = Duration(seconds: 2);

  final DateTime Function() _now;
  final Duration duplicateWindow;

  InvokeRecord? _last;
  Timer? _expiry;
  bool _expiredLocally = false;
  String? _performing;
  int? _performingStrength;
  bool _performingIsUser = false;
  String? _lastError;

  /// 最近一次本地 invoke。
  InvokeRecord? get lastInvoke => _last;

  /// 当前正在演的动作协议名（`null` = 没有）。
  String? get performing => _performing;
  int? get performingStrength => _performingStrength;

  /// 正在演的是不是**用户点的**（决定按钮是否进入忙碌态）。
  bool get performingIsUser => _performingIsUser;

  /// 最近一次触发失败的原因（内联显示）。
  String? get lastError => _lastError;

  /// 某个动作此刻是否「正在演」（按钮据此禁用 + 显示进度环）。
  bool isBusy(String action) => _performing == action;

  /// 最近一次「正在演」的结束是不是**本地兜底**结束的（而不是收到 `cease`）。
  ///
  /// 诊断价值：如果它一直是 `true`，说明服务端确实没发 `cease`
  /// （这正是当下的实情）。UI 不需要区分，但排查时需要知道。
  bool get expiredLocally => _expiredLocally;

  /// 用户点了某个动作。返回 `true` 表示这**很可能**是一次无效的重复点击。
  ///
  /// 注意措辞：这是**本地推测**，不是服务端判决。调用方仍应发出 invoke
  /// （服务端才是仲裁者；而且重复点击的代价只是白跑一趟）。
  bool recordInvoke(String action, int? strength) {
    final DateTime now = _now();
    final InvokeRecord? last = _last;
    final bool duplicate =
        last != null &&
        last.sameAs(action, strength) &&
        now.difference(last.at) < duplicateWindow;
    _last = InvokeRecord(action: action, strength: strength, at: now);
    _lastError = null;
    notifyListeners();
    return duplicate;
  }

  /// 触发失败（HTTP 400 / 网络）。
  void recordError(String message) {
    _lastError = message;
    notifyListeners();
  }

  /// 消费一个动作事件（来自 WS）——**这才是忙碌态的权威来源**。
  bool consume(ActionEvent event) {
    switch (event.kind) {
      case ActionEventKind.perform:
        final String? action = event.action;
        if (action == null) return false;
        _performing = action;
        _performingStrength = event.strength;
        _performingIsUser = event.source.wire == 'user_command';
        _expiredLocally = false;
        _armExpiry(action);
        notifyListeners();
        return true;
      case ActionEventKind.cease:
        if (_performing == null) return false;
        _clearPerforming(locally: false);
        notifyListeners();
        return true;
      case ActionEventKind.unknown:
        return false;
    }
  }

  /// 给「正在演」装一个**本地兜底**到期（服务端不发 `cease`，见常量说明）。
  void _armExpiry(String action) {
    _expiry?.cancel();
    _expiry = null;
    if (!useLocalExpiry) return;
    _expiry = _newTimer(actionDurationFor(action), () {
      _expiry = null;
      // **必须比对动作名**：`Timer.cancel()` 挡不住**已经排进事件队列**的回调。
      // 不比对的话，一个被抢占的旧动作的到期回调会把**新动作**的「正在演」
      // 清掉——表现是「刚点的动作只闪一下就不忙了」。测试（抢占那条）抓到了。
      if (_performing != action) return;
      _clearPerforming(locally: true);
      notifyListeners();
    });
  }

  void _clearPerforming({required bool locally}) {
    _expiry?.cancel();
    _expiry = null;
    _performing = null;
    _performingStrength = null;
    _performingIsUser = false;
    _expiredLocally = locally;
  }

  @override
  void dispose() {
    _expiry?.cancel();
    super.dispose();
  }

  /// 清掉错误提示。
  void clearError() {
    if (_lastError == null) return;
    _lastError = null;
    notifyListeners();
  }

  /// 重复点击时给用户看的解释（**必须解释，否则就是「点了没反应」**）。
  String get duplicateHint =>
      '同一动作正在播放，重复点击不会重播（服务端幂等去重）';
}
