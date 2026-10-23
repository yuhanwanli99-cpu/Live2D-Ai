/// 「这条发给」路由 + 本机 Pi 侧列表（0.4.5 rc.2）。
///
/// # 产品锁（0.4.5 执行单「界面」）
///
/// - 三个选项：`skin` 皮套（缺省）/ `pi` 本机 Pi / `claude`
///   Claude Code（这一版不接，**画出来但禁用**）；
/// - 皮套路由行为一行不改：发送仍走 `ChatController.send`（say / TTS / 口型 /
///   导演 cue 全在那一侧）；
/// - Pi 路由只把这一条用户原文 POST 到本机 Pi，回复进**本文件的侧列表**，
///   **不**进 `ChatController.messages`（那份历史会被送给皮套模型）、
///   **不**进 TTS / 口型 / `action_cue`、**不**落盘（刷新可消失）。
///
/// # 为什么 Pi 条目不住在 `ChatMessage` 里
///
/// `ChatController.messages` 是**会送给皮套模型的历史**。Pi 的正文若混进去，
/// 下一轮皮套就会把 Pi 的话当成自己说过 / 用户说过的内容；而刷新后又消失，
/// 前后两份历史对不上。所以侧列表只住在这个控制器里，`ChatMessage` 零改动。
///
/// 纯 VM 可测：HTTP 客户端可注入（`MockClient`），不依赖真实服务。
library;

import 'dart:async';

import 'package:flutter/widgets.dart'
    show ChangeNotifier, TextEditingController;

import '../api/peer_api.dart';
import 'chat_message.dart';

/// 路由控件的标题（执行单锁死为「这条发给」）。
const String kPeerRouteTitle = '这条发给';

/// 路由下面那句说明：**逐字**来自 0.4.5 执行单「界面」。
const String kPeerRouteNote =
    '皮套仍走现在的对话和出声。本机 Pi 只回文字，不出声，也不带动皮套。'
    '别人来调本服务仍是入站，没挂上时没有入口。';

/// 五个错误码的中文文案（执行单「出站」表，逐字）。
const String kPeerMissingText = '本机没有 pi';
const String kPeerBusyText = '上一条还在等 Pi';
const String kPeerTimeoutText = 'Pi 超时';
const String kPeerFailedText = 'Pi 没有返回一句可读的话';
const String kPeerUnavailableText = '这一版不接 Claude Code';

const Map<String, String> kPeerErrorTexts = <String, String>{
  'peer_missing': kPeerMissingText,
  'peer_busy': kPeerBusyText,
  'peer_timeout': kPeerTimeoutText,
  'peer_failed': kPeerFailedText,
  'peer_unavailable': kPeerUnavailableText,
};

/// 错误码 → 用户能看懂的一句话。未知码 / 连接期失败回落 `peer_failed`。
String peerErrorText(String? code) => kPeerErrorTexts[code] ?? kPeerFailedText;

/// 「这条发给」的三个选项（顺序 = 界面顺序）。
enum PeerRoute {
  skin('skin', '皮套'),
  pi('pi', '本机 Pi'),
  claude('claude', 'Claude Code（这一版不接）');

  const PeerRoute(this.value, this.label);

  /// 与执行单表格里的「值」一致（后端路由 / 将来扩展用）。
  final String value;

  /// 界面上逐字显示的标签。
  final String label;

  /// Claude Code 这一版只占位置：**不能选中**。
  bool get selectable => this != PeerRoute.claude;
}

/// 侧列表一条的性质。
enum PeerEntryKind {
  /// 用户发给 Pi 的原文（可见，但**不是** ChatMessage）。
  user,

  /// Pi 的那一句回复。
  pi,

  /// 这一条失败的原因（**不伪装成 Pi 的回复**）。
  error,
}

/// 侧列表的一条（只在内存里；刷新即消失是**刻意的**）。
class PeerEntry {
  const PeerEntry({
    required this.kind,
    required this.text,
    required this.anchor,
  });

  final PeerEntryKind kind;
  final String text;

  /// 创建时 `ChatController.messages` 的条数：列表按它把两边排成一条时间线。
  final int anchor;
}

/// 把聊天消息与 Pi 侧列表合并成**一条时间线**（纯函数，VM 可测）。
///
/// [PeerEntry.anchor] = 该条创建时已有多少条聊天消息 ⇒ 插在第 anchor 条
/// 消息**之前**。超过 [limit] 时丢最旧的（与聊天面板的视图上限同一条纪律：
/// 只裁视图，不改数据）。
List<Object> mergePeerTimeline(
  List<ChatMessage> messages,
  List<PeerEntry> peers, {
  required int limit,
}) {
  final List<Object> merged = <Object>[];
  int next = 0;
  for (int i = 0; i < messages.length; i++) {
    while (next < peers.length && peers[next].anchor <= i) {
      merged.add(peers[next++]);
    }
    merged.add(messages[i]);
  }
  while (next < peers.length) {
    merged.add(peers[next++]);
  }
  if (merged.length > limit) {
    return merged.sublist(merged.length - limit);
  }
  return merged;
}

/// 「这条发给」的状态机 + Pi 出站 + 侧列表。
///
/// 宿主（`main.dart`）持有一个实例：路由选择、忙闲与条目全在这里；
/// 聊天面板只**订阅**它，不自己判忙、不自己发请求。
class PeerChatController extends ChangeNotifier {
  PeerChatController({PeerApi? api, this.sessionIdOf, this.chatMessageCountOf})
    : _api = api ?? PeerApi();

  final PeerApi _api;

  /// 当前聊天会话 id（没有活动会话时返回 `null` ⇒ 请求体里的 `session_id`
  /// 是空串，后端按「这一条不记会话」处理）。
  final String? Function()? sessionIdOf;

  /// `ChatController.messages` 此刻的条数——只用来给侧列表排时间线。
  final int Function()? chatMessageCountOf;

  PeerRoute _route = PeerRoute.skin;
  final List<PeerEntry> _entries = <PeerEntry>[];
  bool _busy = false;
  Future<void>? _inFlight;

  /// 当前路由（缺省 [PeerRoute.skin]：**行为与改动前逐字一致**）。
  PeerRoute get route => _route;

  /// 这条路由是不是「本机 Pi」。
  bool get isPi => _route == PeerRoute.pi;

  /// 有没有一条 Pi 请求还在飞（同一时刻只允许一个）。
  bool get busy => _busy;

  /// 侧列表（只读视图；顺序 = 创建顺序）。
  List<PeerEntry> get entries => List<PeerEntry>.unmodifiable(_entries);

  /// 测试与宿主都等的「这一条走完」Future（没有在飞时立即完成）。
  Future<void> get settled => _inFlight ?? Future<void>.value();

  /// 选中一个路由。`claude` 不可选 ⇒ 返回 `false` 且状态不变。
  bool selectRoute(PeerRoute next) {
    if (!next.selectable || next == _route) return false;
    _route = next;
    notifyListeners();
    return true;
  }

  /// 发送键（圆键 / Enter）的**路由分发**。
  ///
  /// 返回 `true` = 这条已经由 Pi 路由处理（调用方**不要**再走皮套发送）；
  /// 返回 `false` = 不是 Pi 路由，调用方走既有发送路径。
  ///
  /// 空输入：标记为「已处理」但不发、也不清空输入框。
  /// 忙（上一条还在等 Pi）：只提示忙——不清输入框、**不打断皮套**、不排队。
  bool submitFromInput(TextEditingController input) {
    if (!isPi) return false;
    if (input.text.trim().isEmpty) return true;
    if (send(input.text)) input.clear();
    return true;
  }

  /// 发一条给 Pi。返回 `true` = 已受理（请求已发出）。
  ///
  /// 空文本 / 非 Pi 路由 / 已有请求在飞都返回 `false`；忙时会在侧列表留一条
  /// 「上一条还在等 Pi」（只提示忙，**不**打断皮套那一轮）。
  bool send(String raw) {
    if (!isPi) return false;
    final String text = raw.trim();
    if (text.isEmpty) return false;
    if (_busy) {
      _add(PeerEntryKind.error, kPeerBusyText);
      return false;
    }
    _busy = true;
    _add(PeerEntryKind.user, text);
    _inFlight = _run(text);
    return true;
  }

  Future<void> _run(String text) async {
    try {
      final String reply = await _api.send(
        text: text,
        sessionId: sessionIdOf?.call() ?? '',
      );
      _add(PeerEntryKind.pi, reply);
    } catch (error) {
      _add(
        PeerEntryKind.error,
        peerErrorText(error is PeerApiException ? error.code : null),
      );
    } finally {
      _busy = false;
      notifyListeners();
    }
  }

  void _add(PeerEntryKind kind, String text) {
    _entries.add(
      PeerEntry(
        kind: kind,
        text: text,
        anchor: chatMessageCountOf?.call() ?? 0,
      ),
    );
    notifyListeners();
  }

  @override
  void dispose() {
    _api.dispose();
    super.dispose();
  }
}
