/// WS 连接状态：**纯逻辑，无 `package:web` 依赖**。
///
/// 从 `ws_client.dart` 抽出来的理由与 `ws_frame.dart` 完全一样：`ws_client.dart`
/// 必须 `import 'package:web'`，于是**它的任何类型在 `flutter test` 里都加载不了**。
/// 而 `WsStatus` 是 UI 层的输入（`ConnectionBadge`、`derivUiPhase`），把它留在
/// 那个文件里会让**整个状态呈现层无法单测**——包括规格 §6.3 那张「色 + 形 + 字 +
/// 节奏」映射表，那正是「6 个项目状态指示缺失」的教训所在。
library;

/// `/ws/state` 连接状态（5 态，与 `ws_client.dart` 的 `_setStatus` 取值一一对应）。
enum WsStatus {
  /// 尚未发起连接。
  idle,

  /// 正在建立连接。
  connecting,

  /// 已连接（收到首帧 `subscribe_ack` 后会进入此态）。
  connected,

  /// 连接断开，正在退避重连。
  disconnected,

  /// 已关闭（dispose 之后）。
  closed;

  /// 界面上给人看的短标签。
  ///
  /// **每个状态都要有文字**：规格 §6.4 明确「不靠颜色单独表意」——
  /// 色盲用户无法只凭颜色区分 5 个状态。
  String get label => switch (this) {
    WsStatus.idle => '未连接',
    WsStatus.connecting => '连接中',
    WsStatus.connected => '已连接',
    WsStatus.disconnected => '重连中',
    WsStatus.closed => '已关闭',
  };

  /// 读屏/诊断用的一句话。
  String get description => switch (this) {
    WsStatus.idle => '实时通道尚未建立',
    WsStatus.connecting => '正在建立实时通道',
    WsStatus.connected => '实时通道正常',
    WsStatus.disconnected => '实时通道断开，正在自动重连',
    WsStatus.closed => '实时通道已关闭（页面即将释放）',
  };

  /// 是否「能用」：只有连上才算。
  ///
  /// `connecting` **不算**可用——把「正在连」当可用会让 UI 显示「已连接」
  /// 而消息其实还发不出去（用户看到的是「发出去没回应」）。
  bool get isUsable => this == WsStatus.connected;

  /// 是否处于「用户应该看到问题」的状态（供 `ConnectionBadge` 上 danger 色）。
  bool get isProblem => this == WsStatus.disconnected || this == WsStatus.closed;
}
