/// 动作触发来源与动作明细：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 单独成文件的理由与 `settings/display_prefs.dart`、`audio/gain.dart` 相同，
/// 但这里还多一层：`api/ws_client.dart` 要 `import 'package:web'`（WebSocket），
/// 于是**它的解析逻辑在 VM 上根本跑不到**——这正是下面这个缺陷能长期存活的原因。
library;

/// 动作触发来源（协议 v1 `data.action.source`）。
///
/// 与 Rust 侧 `live2d_ai_core::ActionSource` 一一对应；服务端在
/// `crates/live2d-ai-desktop/src/web_api/ws/events.rs` 的
/// `RenderCommand::Perform` 分支里手工序列化成 snake_case
/// （core 侧该枚举没有 serde 派生）。
enum ActionSource {
  /// 用户显式触发（界面上点的动作 / 命令行）。
  userCommand('user_command'),

  /// LLM 通过工具调用触发。
  llmTool('llm_tool'),

  /// 规则兜底（关键词命中之类）。
  ruleFallback('rule_fallback'),

  /// 未识别：字段缺失、拼写变动、或协议新增了来源而本前端还是旧的。
  ///
  /// **绝不抛异常**——协议加一个来源不该让旧前端崩。
  unknown('unknown');

  const ActionSource(this.wire);

  /// 协议里的字符串值。
  final String wire;

  /// 宽松解析：认不出就 [unknown]。
  static ActionSource fromWire(String? raw) {
    for (final ActionSource value in values) {
      if (value != unknown && value.wire == raw) return value;
    }
    return unknown;
  }

  /// 是否由 AI 自己发起（而非用户手动）。界面据此标注来源。
  bool get isAiInitiated =>
      this == ActionSource.llmTool || this == ActionSource.ruleFallback;

  /// 界面上给人看的短标签。
  String get label => switch (this) {
    ActionSource.userCommand => '你',
    ActionSource.llmTool => 'AI',
    ActionSource.ruleFallback => '规则',
    ActionSource.unknown => '未知',
  };
}

/// `action_state` 帧里 `data.action` 这个**对象**的内容。
///
/// # 这里曾经有一个真实缺陷（2026-09-10 修复）
///
/// 服务端下发的是**对象**：
/// `{"action": {"action": "nod", "strength": 2, "source": "llm_tool"}}`
/// （见 `ws/events.rs`）。而前端曾写成 `_str(data['action'])` —— `_str` 对
/// `Map` 返回 `null`，于是：
///
/// - 动作名**永远是 null**；
/// - `strength` 与 `source` **从未被读取**。
///
/// 结果是「这个动作是用户点的、还是 AI 自己做的」在界面上根本无法表达。
/// core 侧的 `user 100 > llm 90 > rule 50` 仲裁与 `source` 字段**早就下发了**——
/// 所以这是**接线缺口，不是设计缺口**。修解析即可。
class ActionDetail {
  const ActionDetail({
    this.action,
    this.strength,
    this.source = ActionSource.unknown,
  });

  /// 动作名（`nod` / `shake_no` / `tilt` / `look_around` / `listen` / `surprise`）。
  final String? action;

  /// 强度档位（协议 `strength_levels` = `[1, 2, 3]`）；缺失为 null。
  final int? strength;

  /// 谁触发的。
  final ActionSource source;

  /// 是否解析到了有效内容（用于区分「真的有动作」与「空对象」）。
  bool get isEmpty => action == null;

  /// 从 `data.action` 原始 JSON 值宽容解析。
  ///
  /// 不是对象、键名不符、类型不对——一律降级为「空明细」，**绝不抛异常**。
  /// 协议层对前端来说是外部输入，坏一帧不该让整条事件流断掉。
  factory ActionDetail.fromData(Object? raw) {
    if (raw is! Map) return const ActionDetail();
    String? action;
    int? strength;
    String? source;
    raw.forEach((Object? key, Object? value) {
      switch (key) {
        case 'action':
          if (value is String) action = value;
        case 'strength':
          if (value is int) {
            strength = value;
          } else if (value is num) {
            strength = value.toInt();
          }
        case 'source':
          if (value is String) source = value;
      }
    });
    return ActionDetail(
      action: action,
      strength: strength,
      source: ActionSource.fromWire(source),
    );
  }

  @override
  String toString() =>
      'ActionDetail($action, strength: $strength, source: ${source.wire})';
}
