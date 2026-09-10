/// 动作日志的**接线层**：WS 帧 → [ActionHistory]，并维护「有未读的 AI 动作」。
///
/// # 为什么要有这一层
///
/// `chat_controller.dart` 必须 `import 'package:web'`（WebSocket），所以
/// **它整体在 `flutter test` 里加载不了**。P2 的全部价值是「core 早已下发的
/// `source` 终于看得见」——如果接线只有 `ChatController` 一条路，那么
/// 「帧真的会变成历史里那一行」这件事就永远测不到（这正是 §0.3 那个缺陷
/// 能长期存活的原因）。
///
/// 本文件**不碰 `package:web`**：它只消费已经解析好的 [WsEvent]，
/// 于是 `测试里可以喂真实抓包的帧原文` → 断言历史里的中文名与来源徽标。
///
/// # 未读计数（为什么值得单独做）
///
/// 「AI 自己触发了动作」是一个**转瞬即逝**的信号：动作演完就没了。如果日志
/// 只是静静躺在弹层里，用户根本不知道刚才 AI 动过。所以给入口按钮加一个
/// 未读计数——这是 P2 这条差异化点的最小可用形态（规格 §8.3 的「三处呈现」
/// 第 3 处要求舞台小卡提示，那属于 P5 的舞台角标）。
library;

import 'package:flutter/foundation.dart';

import '../api/commands_api.dart';
import '../api/ws_frame.dart';
import 'action_event.dart';
import 'action_history.dart';

class ActionLogController extends ChangeNotifier {
  ActionLogController({ActionHistory? history, DateTime Function()? now})
    : history = history ?? ActionHistory(),
      _now = now ?? DateTime.now {
    // 显式 `this.history`：参数与字段同名时，构造体内裸写 `history` 解析到
    // **参数**（`ActionHistory?`），会得到「可能在 null 上调用」的告警。
    this.history.addListener(_onHistoryChanged);
  }

  /// 底层列表模型（可直接交给 `ActionHistoryView`）。
  final ActionHistory history;

  final DateTime Function() _now;

  int _unseenAiActions = 0;

  /// 未读的 **AI/规则触发** 动作条数（用户自己点的不算——不用再告诉他一遍）。
  int get unseenAiActions => _unseenAiActions;

  /// 有新未读 AI 动作（入口按钮显示角标用）。
  bool get hasUnseenAiActions => _unseenAiActions > 0;

  List<ActionHistoryEntry> get entries => history.entries;
  ActionHistoryEntry? get current => history.current;
  int get droppedCount => history.droppedCount;

  /// 消费一帧。**只认 `action_state`**，其余返回 `false`（不通知、不改状态）。
  bool consume(WsEvent event) {
    if (event is! ActionStateEvent) return false;
    return history.consume(ActionEvent.fromFrame(event, receivedAt: _now()));
  }

  /// 直接用一帧**原文**（测试与调试用；内部就是 `parseWsFrame` + [consume]）。
  ///
  /// 坏帧 → `parseWsFrame` 返回 `null` → 直接 `false`，不抛。
  bool consumeRaw(String raw) {
    final WsEvent? event = parseWsFrame(raw);
    if (event == null) return false;
    return consume(event);
  }

  /// 灌入动作目录（`GET /api/v1/commands`），把「协议名 → 中文名」接上。
  void useCatalog(CommandCatalog catalog) {
    final Map<String, String> labels = catalog.labelByAction;
    if (labels.isEmpty) return;
    history.useLabels((String action) => labels[action] ?? action);
  }

  /// 标记未读已读（打开日志弹层时调用）。
  void markSeen() {
    if (_unseenAiActions == 0) return;
    _unseenAiActions = 0;
    notifyListeners();
  }

  /// 清空日志（同时清未读）。
  void clear() {
    history.clear();
    if (_unseenAiActions != 0) {
      _unseenAiActions = 0;
      notifyListeners();
    }
  }

  void _onHistoryChanged() {
    final ActionHistoryEntry? current = history.current;
    // 只对**刚开始表演的 AI/规则动作**计数：core 的 source 字段就是为这个存在的。
    if (current != null &&
        current.isPerforming &&
        current.source.isAiInitiated &&
        !identical(current, _lastCounted)) {
      _lastCounted = current;
      _unseenAiActions++;
    }
    notifyListeners();
  }

  /// 上一次计入未读的那一条（防止同一动作被重复计数）。
  ActionHistoryEntry? _lastCounted;

  @override
  void dispose() {
    history.removeListener(_onHistoryChanged);
    super.dispose();
  }
}
