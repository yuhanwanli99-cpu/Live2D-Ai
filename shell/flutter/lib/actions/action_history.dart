/// 动作历史：**纯列表模型**（无 Flutter widget 依赖，仅用 `ChangeNotifier`）。
///
/// 规格 §8.3「三处呈现」的第 1 处：每一行显示
/// `[时间] [动作中文名] [强度 Ⅰ/Ⅱ/Ⅲ] [来源徽标] [epoch]`。
///
/// # 三条容易做错的语义，都在这里定义并单测
///
/// 1. **`cease` 是收口，不是新条目。** 它不是「又发生了一件事」，而是「上一件
///    事结束了」。把 `cease` 也压进列表会让日志里一半是噪声。
/// 2. **`perform` 撞上未收口的 `perform` = 上一动作被抢占。** core 的仲裁是
///    `user 100 > llm 90 > rule 50`，用户的手动动作会**顶掉** AI 正在演的动作，
///    而且此时**不会**先来一个 `cease`。若不做收口，`current` 会永久停在旧的
///    那一条上，界面显示「正在演 点头」而模型其实在摇头。
/// 3. **条目上限。** 动作日志是长期运行会累积的东西，必须有界。
library;

import 'package:flutter/foundation.dart';

import '../api/action_source.dart';
import 'action_event.dart';

/// 历史里的一行。
@immutable
class ActionHistoryEntry {
  const ActionHistoryEntry({
    required this.action,
    required this.label,
    required this.source,
    required this.epoch,
    this.strength,
    this.at,
    this.seq,
    this.endedAt,
    this.superseded = false,
  });

  /// 协议名（`nod` / `shake_no` / …）。
  final String action;

  /// 给人看的中文名（由注入的目录解析；目录缺失时等于 [action]）。
  final String label;

  /// 强度档位 1..=3；缺失为 `null`。
  final int? strength;

  /// 谁触发的（**这一列就是本规格的差异化点**）。
  final ActionSource source;

  /// 代次。
  final int epoch;

  /// 本地收到 `perform` 的时刻。
  final DateTime? at;

  /// 帧序号。
  final int? seq;

  /// 本次表演**结束**的时刻（收到 `cease` 或被下一次 `perform` 抢占时写入）。
  final DateTime? endedAt;

  /// 是否因「新的 `perform` 到来」而结束（而不是收到 `cease`）。
  final bool superseded;

  /// 是否正在表演。
  bool get isPerforming => endedAt == null;

  /// 显示用的强度标签（越界/缺失为 `null`）。
  String? get strengthText => strengthLabel(strength);

  /// 无障碍读法。
  String get semanticLabel {
    final StringBuffer buffer = StringBuffer('$label，来源${source.label}');
    final String? s = strengthSemanticLabel(strength);
    if (s != null) buffer.write('，$s');
    buffer.write(isPerforming ? '，正在表演' : '');
    return buffer.toString();
  }

  /// 收口：写入结束时刻。**幂等**——已经是结束态时保持原状。
  ActionHistoryEntry end(DateTime? when, {bool superseded = false}) {
    if (endedAt != null) return this;
    return ActionHistoryEntry(
      action: action,
      label: label,
      source: source,
      epoch: epoch,
      strength: strength,
      at: at,
      seq: seq,
      endedAt: when ?? at,
      superseded: superseded,
    );
  }

  @override
  String toString() =>
      'ActionHistoryEntry($action/$label, ${source.wire}, '
      'strength: $strength, epoch: $epoch, performing: $isPerforming)';
}

/// 动作名解析器：协议名 → 给人看的中文名。
///
/// **注入而不是内建表**：动作目录的权威来源是服务端 `GET /api/v1/commands`
/// （`target_action` + `name`）。在 Flutter 侧再抄一份映射表就是复制核心数据
/// （AGENTS.md：「不得在 Flutter 侧复制核心逻辑」），而且服务端加一个动作时
/// 前端会静默显示内部 id。缺省实现是**恒等**：拿不到目录就显示协议名——
/// 看得懂，且不撒谎。
typedef ActionLabelResolver = String Function(String action);

/// 恒等解析器（无目录时的缺省）。
String identityActionLabel(String action) => action;

/// 动作历史（有界环形列表 + 唯一「正在演」游标）。
class ActionHistory extends ChangeNotifier {
  ActionHistory({this.maxEntries = defaultMaxEntries, ActionLabelResolver? labelOf})
    : _labelOf = labelOf ?? identityActionLabel;

  /// 条目上限。超出后**丢最旧的**。
  ///
  /// 取值理由：动作日志是诊断面，不是主界面；500 行足以覆盖一次长时间调试，
  /// 又不至于让 `ListView` 的构建退化（配 `itemExtent` 后仍是 O(可见行)）。
  static const int defaultMaxEntries = 500;

  final int maxEntries;
  ActionLabelResolver _labelOf;

  final List<ActionHistoryEntry> _entries = <ActionHistoryEntry>[];
  ActionHistoryEntry? _current;
  int _dropped = 0;

  /// 只读快照（**新的不可变列表**，调用方拿到后不会被后续变更影响）。
  List<ActionHistoryEntry> get entries =>
      List<ActionHistoryEntry>.unmodifiable(_entries);

  /// 正在表演的那一条（没有则为 `null`）。
  ActionHistoryEntry? get current => _current;

  /// 因超出上限而被丢弃的条目数（诊断用；不静默）。
  int get droppedCount => _dropped;

  bool get isEmpty => _entries.isEmpty;

  /// 替换动作名解析器（目录异步到达时调用；已在列表里的条目**不回溯改名**）。
  ///
  /// 不回溯的理由：历史是「当时看到了什么」的记录。若目录后到就把旧行改名，
  /// 会让「为什么这一行刚才叫 nod 现在叫点头」变成不可解释的行为。
  void useLabels(ActionLabelResolver resolver) {
    _labelOf = resolver;
  }

  /// 消费一个已投影的动作事件。
  ///
  /// 返回是否改变了状态（便于调用方决定要不要 `notifyListeners`——虽然本类
  /// 自己会通知，返回值是给测试与上层判断用的）。
  bool consume(ActionEvent event) {
    if (!event.isDisplayable) return false;
    if (event.isCease) return _cease(event);
    return _perform(event);
  }

  bool _cease(ActionEvent event) {
    final ActionHistoryEntry? open = _current;
    // 没有未收口的动作时，`cease` 是 no-op：**不建条目**，也不报错。
    // 这很常见——服务端在 `new_epoch` / 停止时会补发一次收口。
    if (open == null) return false;
    // epoch 不同的 `cease` 属于另一轮，不该收口当前这一条。
    if (event.epoch != open.epoch) return false;
    final int index = _entries.indexOf(open);
    if (index >= 0) {
      _entries[index] = open.end(event.receivedAt);
    }
    _current = null;
    notifyListeners();
    return true;
  }

  bool _perform(ActionEvent event) {
    // 抢占：新的 perform 到来即视为上一条结束（core 不会先发 cease）。
    final ActionHistoryEntry? open = _current;
    if (open != null) {
      final int index = _entries.indexOf(open);
      if (index >= 0) {
        _entries[index] = open.end(event.receivedAt, superseded: true);
      }
      _current = null;
    }
    final ActionHistoryEntry entry = ActionHistoryEntry(
      action: event.action!,
      label: _labelOf(event.action!),
      source: event.source,
      epoch: event.epoch,
      strength: event.strength,
      at: event.receivedAt,
      seq: event.seq,
    );
    _entries.add(entry);
    _current = entry;
    _evictOverflow();
    notifyListeners();
    return true;
  }

  /// 超出上限时丢最旧的。被丢的若正好是 `_current`，把游标一并清掉
  /// （否则游标会指向一个已不在列表里的对象）。
  void _evictOverflow() {
    while (_entries.length > maxEntries) {
      final ActionHistoryEntry removed = _entries.removeAt(0);
      _dropped++;
      if (identical(_current, removed)) _current = null;
    }
  }

  /// 清空（「清空日志」按钮 / 换模型）。
  void clear() {
    if (_entries.isEmpty && _current == null) return;
    _entries.clear();
    _current = null;
    _dropped = 0;
    notifyListeners();
  }
}
