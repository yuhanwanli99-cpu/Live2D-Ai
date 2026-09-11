/// 会话与多会话管理：**纯逻辑，零 Flutter / 零 package:web 依赖**。
///
/// 可在 VM 上单测（与 `display_prefs.dart` / `breakpoints.dart` 同一条纪律：
/// 「能被单测的东西不要放在 web 依赖后面」）。
///
/// # 这一层解决什么，不解决什么（**边界必须说清楚**）
///
/// **解决**：聊天记录**留在本机浏览器**——刷新/重开页面还在；可以开多个会话、
/// 切换、重命名、删除。关掉页面再回来，之前聊过的东西一眼能找到。
///
/// **不解决**：模型**没有记忆**。跨刷新、跨会话，LLM 都不记得上一轮说过什么。
/// 原因是协议本身：`POST /api/v1/chat` 只接受 `{text}`，**没有历史字段**，
/// 前端无从把历史喂回去；服务端 `persona.max_history_pairs` 出厂为 `0`
/// （= 服务端自己也不带历史）。
///
/// 所以这一层是**「本地记录」**，不是**「模型记忆」**。真正的会话记忆需要
/// 协议 + 端点 + 服务端持久化，已登记为**待实现**（见
/// `docs/plans/future-roadmap-2026-09.md` 与 `docs/design/web-ui-spec-v3.md`）。
///
/// 这个区分必须写在这里，而不是只在文档里：**否则下一个人会把「历史还在」
/// 误当成「模型记得」**，然后花时间 debug 一个不存在的 bug。
library;

import 'chat_message.dart';

/// 一个会话：标题 + 消息列表 + 时间戳。
///
/// **可变**是刻意的（与 [ChatMessage] 同一条理由）：流式追加是每帧一次的高频
/// 操作，每次 delta 都重建整个会话对象会制造大量垃圾。
class ChatSession {
  ChatSession({
    required this.id,
    this.title = '',
    List<ChatMessage>? messages,
    required this.createdAt,
    required this.updatedAt,
  }) : messages = messages ?? <ChatMessage>[];

  /// 稳定标识。用**创建时刻的微秒**而不是随机数：不需要引入 uuid 依赖
  /// （运行时依赖 = 0 是硬约束），而同一毫秒内创建两个会话在人类操作下
  /// 不可能发生。
  final String id;

  /// 用户起的名字。**空串 = 没起过名**，此时显示 [autoTitle]。
  ///
  /// 「空串」而不是「存一个默认文案」：默认文案会随语言/规则变，
  /// 存下来就成了历史包袱；而「有没有起过名」是一个稳定的事实。
  String title;

  final List<ChatMessage> messages;

  final DateTime createdAt;

  /// 最后一次变动（发消息 / 收回复 / 改标题）。列表按它倒序。
  DateTime updatedAt;

  /// 用户起过名没有。
  bool get isNamed => title.trim().isNotEmpty;

  /// 列表里显示的标题。
  String get displayTitle => isNamed ? title : autoTitle;

  /// 没起名时**从内容推**一个标题：第一条用户消息的前若干字。
  ///
  /// 取用户消息而不是助手消息：用户自己打的字是他找这条会话时最可能记得的
  /// 线索（助手的第一句常是「好的！」这类没有辨识度的开场）。
  String get autoTitle {
    for (final ChatMessage m in messages) {
      if (m.role == ChatRole.user && m.text.trim().isNotEmpty) {
        return _truncate(m.text.trim(), kAutoTitleMaxChars);
      }
    }
    return kUntitledSessionLabel;
  }

  /// 有没有内容（空会话可以不留）。
  bool get isEmpty => messages.isEmpty;

  /// 最后一条消息的时间（列表副标题用）。
  DateTime? get lastMessageAt =>
      messages.isEmpty ? null : updatedAt;

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'title': title,
    'createdAt': createdAt.toIso8601String(),
    'updatedAt': updatedAt.toIso8601String(),
    'messages': <Object?>[
      for (final ChatMessage m in messages) m.toJson(),
    ],
  };

  /// 反序列化。**坏数据返回 `null`（丢弃这一条），绝不抛**。
  ///
  /// 与 `DisplayPrefs.fromJson` 同一条纪律：一份被外部改坏 / 版本不匹配的
  /// 存储不该让整个聊天区打不开。丢一条会话的代价远小于丢整个列表。
  static ChatSession? fromJson(Object? raw) {
    if (raw is! Map) return null;
    final Map<String, Object?> json = raw.cast<String, Object?>();
    final String? id = _readString(json['id']);
    if (id == null || id.isEmpty) return null;

    final DateTime createdAt =
        _readTime(json['createdAt']) ?? DateTime.fromMillisecondsSinceEpoch(0);
    final DateTime updatedAt = _readTime(json['updatedAt']) ?? createdAt;

    final List<ChatMessage> messages = <ChatMessage>[];
    final Object? rawMessages = json['messages'];
    if (rawMessages is List) {
      for (final Object? item in rawMessages) {
        final ChatMessage? m = ChatMessage.fromJson(item);
        if (m != null) messages.add(m);
      }
    }

    return ChatSession(
      id: id,
      title: _readString(json['title']) ?? '',
      messages: messages,
      createdAt: createdAt,
      updatedAt: updatedAt,
    );
  }
}

/// 会话集合 + 当前选中的会话。**所有操作都在这个类里**（UI 不直接改列表）。
class ChatSessionStore {
  ChatSessionStore({List<ChatSession>? sessions, this.activeId})
    : sessions = sessions ?? <ChatSession>[];

  /// 空store：**不含任何会话**——第一次真正需要时才建（见 [ensureActive]）。
  ///
  /// 刻意不在这里就建一个空会话：那会让「打开页面」也产生一次写入，
  /// 而用户可能只是看了一眼就从没聊过。
  factory ChatSessionStore.empty() => ChatSessionStore();

  /// 全部会话。**按 [ChatSession.updatedAt] 倒序**（最近用过的在最上面）。
  ///
  /// 排序在**读取时**做而不是维护一个有序列表：会话数量很小（上限 50），
  /// 每次排序的代价可以忽略，而维护有序插入要多一堆「什么时候该移动」
  /// 的分支——那种分支正是 bug 的温床。
  final List<ChatSession> sessions;

  /// 当前选中的会话 id；`null` = 还没选（或一个都没有）。
  String? activeId;

  /// 会话数上限。超了丢**最久没用过**的那个。
  ///
  /// 取值理由：localStorage 常见配额 5 MB，纯文本消息很小，50 个会话
  /// 即使每个都到 [kMaxMessagesPerSession] 也远在配额内。
  static const int maxSessions = 50;

  /// 每个会话保留的消息条数上限，超了丢**最旧**的。
  ///
  /// 与视图层的 `kChatHistoryLimit`（500）**取同一个值**：两处不一致会出现
  /// 「界面上能看到但重开就没了」这种最难查的偏差。
  static const int kMaxMessagesPerSession = 500;

  ChatSession? get active {
    final String? id = activeId;
    if (id == null) return null;
    for (final ChatSession s in sessions) {
      if (s.id == id) return s;
    }
    return null;
  }

  /// 绑定给 UI 的消息列表（**就是活动会话的那个 List 实例**）。
  List<ChatMessage> get messages => active?.messages ?? const <ChatMessage>[];

  /// 按最近使用倒序的会话视图。
  List<ChatSession> get byRecency {
    final List<ChatSession> out = List<ChatSession>.of(sessions)
      ..sort((ChatSession a, ChatSession b) => b.updatedAt.compareTo(a.updatedAt));
    return out;
  }

  /// 保证有一个活动会话（没有就建），返回它。
  ///
  /// 首次发消息时调用——而不是构造函数里。见 [ChatSessionStore.empty]。
  ChatSession ensureActive({DateTime? now}) {
    final ChatSession? current = active;
    if (current != null) return current;
    return create(now: now);
  }

  /// 新建会话并选中它。
  ChatSession create({DateTime? now, String title = ''}) {
    final DateTime at = now ?? DateTime.now();
    final ChatSession session = ChatSession(
      id: _nextId(at),
      title: title,
      createdAt: at,
      updatedAt: at,
    );
    sessions.add(session);
    activeId = session.id;
    _trimSessions();
    return session;
  }

  /// 选中一个会话。返回是否真的切过去了（id 不存在 → `false`）。
  bool select(String id) {
    if (active?.id == id) return false;
    for (final ChatSession s in sessions) {
      if (s.id == id) {
        activeId = id;
        return true;
      }
    }
    return false;
  }

  /// 删除一个会话。
  ///
  /// 删的是**当前会话**时，自动选中「下一个最近用过的」；一个都不剩就把
  /// `activeId` 置空（界面显示空态，而不是自动新建一个空会话——用户删光了
  /// 就是想要干净）。
  bool delete(String id) {
    final int index = sessions.indexWhere((ChatSession s) => s.id == id);
    if (index < 0) return false;
    sessions.removeAt(index);
    if (activeId == id) {
      activeId = byRecency.isEmpty ? null : byRecency.first.id;
    }
    return true;
  }

  /// 重命名。`title` 传空串 = **恢复自动标题**（这是有意的：用户清空输入框
  /// 表达的就是「不要这个名字了」，而不是「给我一个空标题」）。
  bool rename(String id, String title, {DateTime? now}) {
    for (final ChatSession s in sessions) {
      if (s.id == id) {
        s.title = title.trim();
        s.updatedAt = now ?? s.updatedAt;
        return true;
      }
    }
    return false;
  }

  /// 清空当前会话的消息（保留会话本身）。
  void clearActive({DateTime? now}) {
    final ChatSession? current = active;
    if (current == null) return;
    current.messages.clear();
    current.updatedAt = now ?? DateTime.now();
  }

  /// 删除所有**空会话**（除了当前这个，免得用户正看着它被删掉）。
  int pruneEmpty() {
    final int before = sessions.length;
    sessions.removeWhere(
      (ChatSession s) => s.isEmpty && s.id != activeId,
    );
    return before - sessions.length;
  }

  /// 追加一条消息到活动会话，并推进 `updatedAt`。
  void append(ChatMessage message, {DateTime? now}) {
    final ChatSession session = ensureActive(now: now);
    session.messages.add(message);
    session.updatedAt = now ?? DateTime.now();
    _trimMessages(session);
  }

  /// **就地替换**一条消息（保持它在列表里的位置）。
  ///
  /// 用途只有一个但很关键：一轮结束时把「还是空气泡的 assistant 消息」
  /// 换成一条**系统提示**（`turn_liveness.dart` 的 `settleTurn`）。
  ///
  /// 为什么不能用「删掉再追加」：那样提示会跑到列表**末尾**，和它解释的
  /// 那一轮脱节——用户看到的是「我的消息 → 别人的消息 → 一句孤立的提示」。
  /// 位置本身就是语义。
  bool replace(ChatMessage old, ChatMessage replacement, {DateTime? now}) {
    final ChatSession? session = active;
    if (session == null) return false;
    final int index = session.messages.indexOf(old);
    if (index < 0) return false;
    session.messages[index] = replacement;
    session.updatedAt = now ?? DateTime.now();
    return true;
  }

  /// 从活动会话移除一条消息（空气泡清理用）。
  void remove(ChatMessage message, {DateTime? now}) {
    final ChatSession? session = active;
    if (session == null) return;
    if (session.messages.remove(message)) {
      session.updatedAt = now ?? DateTime.now();
    }
  }

  /// 把活动会话标记为「刚用过」（收到回复时调用，让它在列表里浮上来）。
  void touchActive({DateTime? now}) {
    final ChatSession? session = active;
    if (session == null) return;
    session.updatedAt = now ?? DateTime.now();
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'version': kChatStoreVersion,
    'activeId': activeId,
    'sessions': <Object?>[
      for (final ChatSession s in sessions) s.toJson(),
    ],
  };

  /// 反序列化：**永不抛**。坏条目丢弃，版本不符整体丢弃。
  factory ChatSessionStore.fromJson(Object? raw) {
    if (raw is! Map) return ChatSessionStore.empty();
    final Map<String, Object?> json = raw.cast<String, Object?>();
    // 版本门禁：不认识的版本**整体丢弃**而不是尽力解析——半懂不懂地读
    // 一份未来格式，比重来一遍更危险（可能把新字段写没了）。
    if (json['version'] != kChatStoreVersion) return ChatSessionStore.empty();

    final List<ChatSession> sessions = <ChatSession>[];
    final Object? rawSessions = json['sessions'];
    if (rawSessions is List) {
      for (final Object? item in rawSessions) {
        final ChatSession? s = ChatSession.fromJson(item);
        if (s != null) sessions.add(s);
      }
    }

    final String? activeId = _readString(json['activeId']);
    final ChatSessionStore store = ChatSessionStore(
      sessions: sessions,
      // activeId 指向一个不存在的会话（存储被外部改坏）→ 当作没选。
      activeId: sessions.any((ChatSession s) => s.id == activeId)
          ? activeId
          : null,
    );
    store._trimSessions();
    for (final ChatSession s in store.sessions) {
      ChatSessionStore._trimMessages(s);
    }
    return store;
  }

  /// id 生成：`<微秒时间戳>-<同刻序号>`。
  ///
  /// 加序号是为了防「同一毫秒内建两个」——`DateTime.now()` 的分辨率在
  /// 不同平台上并不保证是微秒，纯时间戳在快速连点「新建」时可能撞。
  static int _seq = 0;
  static String _nextId(DateTime at) {
    _seq++;
    return '${at.microsecondsSinceEpoch}-$_seq';
  }

  void _trimSessions() {
    while (sessions.length > maxSessions) {
      // 丢最久没用过的；**绝不丢当前选中的那个**（否则用户正看着它消失）。
      //
      // 这里刻意**不用** `firstWhere(..., orElse:)`：`orElse` 会在「找不到
      // 非当前会话」时回落到第一个（也就是当前那个），于是兜底反而把它删了。
      // 找不到就**不裁**——宁可暂时超一个上限，也不能删掉用户正在看的会话。
      ChatSession? victim;
      for (final ChatSession candidate in byRecency.reversed) {
        if (candidate.id != activeId) {
          victim = candidate;
          break;
        }
      }
      if (victim == null) return;
      sessions.remove(victim);
    }
  }

  static void _trimMessages(ChatSession session) {
    while (session.messages.length > kMaxMessagesPerSession) {
      session.messages.removeAt(0);
    }
  }
}

/// 相对时间（「刚刚 / 5 分钟前 / 昨天 / 3 天前 / 2026-09-01」）。
///
/// **纯函数、显式传 `now`**：既能单测，又**不依赖系统时钟**
///（依赖 `DateTime.now()` 的断言会随机红）。
///
/// 放在纯逻辑层而不是 `ui/session_sheet.dart`：那个文件 import Flutter，
/// 于是这段算术在 VM 上就测不了了——与 `breakpoints.dart` /
/// `display_prefs.dart` 同一条纪律。
///
/// 分档理由：会话列表里的时间只是「找它」的线索，不需要精确到秒；
/// 越近的越用相对表述（人记得住「刚才那个」），越远的越用绝对日期。
String relativeTime(DateTime at, {required DateTime now}) {
  final Duration d = now.difference(at);
  if (d.isNegative) return '刚刚'; // 时钟回拨 / 未来时间：别显示「-3 分钟前」
  if (d.inMinutes < 1) return '刚刚';
  if (d.inMinutes < 60) return '${d.inMinutes} 分钟前';
  if (d.inHours < 24) return '${d.inHours} 小时前';
  if (d.inDays == 1) return '昨天';
  if (d.inDays < 7) return '${d.inDays} 天前';
  final String m = at.month.toString().padLeft(2, '0');
  final String day = at.day.toString().padLeft(2, '0');
  return '${at.year}-$m-$day';
}

/// 存储格式版本。不匹配就整体丢弃（见 [ChatSessionStore.fromJson]）。
const int kChatStoreVersion = 1;

/// 没起名也没内容时的会话标题。
const String kUntitledSessionLabel = '新对话';

/// 自动标题最多几个字（中文字符）。
///
/// 取值理由：会话列表一行大约放得下 12–14 个汉字（320 px 宽的聊天列减去
/// 内边距）。取 14 是「一行放得下且不截断」的上限，再由 UI 单行省略兜底。
const int kAutoTitleMaxChars = 14;

/// 超过 [max] 个字符就截断并加省略号。
String _truncate(String text, int max) {
  // 按**字符**（rune）截而不是按 UTF-16 code unit：中文与 emoji 都在
  // surrogate pair 之外会被切坏。
  final List<int> runes = text.runes.toList();
  if (runes.length <= max) return text;
  return '${String.fromCharCodes(runes.take(max))}…';
}

String? _readString(Object? raw) => raw is String ? raw : null;

DateTime? _readTime(Object? raw) {
  if (raw is! String) return null;
  return DateTime.tryParse(raw);
}
