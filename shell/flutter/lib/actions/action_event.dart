/// 动作事件投影：`ActionStateEvent`（WS 帧）→ 领域事件。
///
/// **纯逻辑、无 Flutter 依赖**，可在 VM 上单测。
///
/// # 为什么要有这一层（而不是直接在 controller 里读帧字段）
///
/// `action_state` 帧有两个**语义陷阱**，都很容易在读字段的地方各错一遍：
///
/// 1. `data.kind` 是 `perform` 或 `cease`——`cease` 时**没有** `data.action`
///    对象，强读字段会得到「空动作」而不是「停止」。两者必须显式区分。
/// 2. `data.action` 是**对象**（`{action, strength, source}`）而不是字符串。
///    这个坑踩过一次：整条来源信息长期丢失（见 `ActionDetail` 头注）。
///
/// 投影层把这两件事收敛成一处，UI 只面对 [ActionEvent]。原来
/// `chat_controller.dart` 里 `case ActionStateEvent(): break;` ——事件被**丢掉**，
/// 所以 core 早已下发的 `source` 在界面上看不见（规格 §0.3、§8.3）。
library;

import '../api/action_source.dart';
import '../api/ws_frame.dart';

/// 动作事件的两种语义。
enum ActionEventKind {
  /// 开始表演。
  perform,

  /// 停止表演（`cease`）。
  cease,

  /// 未识别的 `kind`：协议新增了取值而本前端还是旧的。
  ///
  /// **不抛异常**，也**不当成 perform**——把一个未知语义当成「开始表演」会让
  /// 界面显示一个根本没发生的动作。调用方应忽略它。
  unknown;

  /// 宽松解析：认不出就 [unknown]。
  static ActionEventKind fromWire(String? raw) => switch (raw) {
    'perform' => ActionEventKind.perform,
    'cease' => ActionEventKind.cease,
    _ => ActionEventKind.unknown,
  };
}

/// 一次已投影的动作事件。
class ActionEvent {
  const ActionEvent({
    required this.kind,
    required this.epoch,
    this.action,
    this.strength,
    this.source = ActionSource.unknown,
    this.seq,
    this.ts,
    this.receivedAt,
  });

  /// `perform` / `cease` / `unknown`。
  final ActionEventKind kind;

  /// 代次（与服务端 `epoch` 对齐；用于「这一轮还在演吗」）。
  final int epoch;

  /// 动作的**协议名**（`nod` / `shake_no` / …）；`cease` 时为 `null`。
  final String? action;

  /// 强度档位 1..=3；缺失为 `null`。
  final int? strength;

  /// 谁触发的。
  final ActionSource source;

  /// 帧序号（诊断用）。
  final int? seq;

  /// 服务端时间戳原文（ISO-8601 字符串，**原样带出**，不在这里解析）。
  ///
  /// 不解析的理由：这是**外部输入**，格式不该成为崩溃点；而且历史列表要的是
  /// 「本地收到时刻」，服务端时钟与浏览器时钟不可比。两者都留着。
  final String? ts;

  /// 本地收到该帧的时刻（用于历史列表的时间列）。
  final DateTime? receivedAt;

  /// 是否真的有一个动作可展示（`perform` 且解析出了动作名）。
  bool get hasAction => kind == ActionEventKind.perform && action != null;

  /// 是否是一次停止信号。
  bool get isCease => kind == ActionEventKind.cease;

  /// 是否可进历史列表：`unknown` 语义与「`perform` 但没有动作名」都排除。
  bool get isDisplayable => kind != ActionEventKind.unknown && (isCease || hasAction);

  /// 从 WS 帧投影。`receivedAt` 由调用方注入（便于测试固定时间）。
  factory ActionEvent.fromFrame(ActionStateEvent frame, {DateTime? receivedAt}) =>
      ActionEvent(
        kind: ActionEventKind.fromWire(frame.kind),
        epoch: frame.epoch,
        action: frame.action,
        strength: frame.strength,
        source: frame.source,
        seq: frame.seq,
        ts: frame.ts,
        receivedAt: receivedAt,
      );

  @override
  String toString() =>
      'ActionEvent(${kind.name}, $action, strength: $strength, '
      'source: ${source.wire}, epoch: $epoch)';
}

/// 强度档位的罗马数字标签（`1→Ⅰ` / `2→Ⅱ` / `3→Ⅲ`）。
///
/// 用罗马数字而不是「1/2/3」：这是**档位**不是**数量**，罗马数字在视觉上不会
/// 被误读成「点了 3 次」。规格 §8.3 的呈现要求即此。
///
/// 越界/缺失一律回 `null`（由调用方决定怎么显示「未知强度」），**不做 clamp**：
/// 把 `7` 显示成 `Ⅲ`（最高档）是在编造信息。
String? strengthLabel(int? strength) => switch (strength) {
  1 => 'Ⅰ',
  2 => 'Ⅱ',
  3 => 'Ⅲ',
  _ => null,
};

/// 强度档位的**语义**读法（无障碍用；`Semantics` 里不能只念罗马数字）。
String? strengthSemanticLabel(int? strength) => switch (strength) {
  1 => '强度 轻',
  2 => '强度 中',
  3 => '强度 强',
  _ => null,
};
