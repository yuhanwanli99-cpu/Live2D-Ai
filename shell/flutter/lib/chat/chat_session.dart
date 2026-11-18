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
/// **会话上下文 = 本机历史 + 每轮随请求附带**（2026-11-18 审计 #14）：发送时
/// 前端把本会话**最近 8 轮**（16 条）连同这一句放进 `POST /api/v1/chat` 的可选
/// `history`，服务端只把它拼进**本轮** prompt，**不**持久化会话。于是：
/// - 切换会话 = 换一份历史 ⇒ 模型上下文真的跟着换；
/// - 刷新 / 重开页面：历史从本机存档读回，下一轮照旧带上；
/// - 换设备 / 清站点数据 = 历史没了（服务端没有备份）。
///
/// 所以这一层是**「本机会话记录」**：服务端没有跨轮记忆，`max_history_pairs`
/// 只服务别的入口；真正决定模型看到什么的，是这一段会话在本机存了多少（上限
/// 见 [ChatSessionStore.kMaxMessagesPerSession]）。
///
/// 这个区分必须写在这里：**否则下一个人会把「历史还在」误当成「服务端记得」**，
/// 然后花时间 debug 一个不存在的 bug。
library;

import 'dart:convert';

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

  /// 有没有内容。**空会话不再被自动清理**（#12）：只有用户显式删除才会消失，
  /// 所以这个判据只用于展示与「清空」语义，不再驱动任何 prune。
  bool get isEmpty => messages.isEmpty;

  /// 最后一条消息的时间（列表副标题用）。
  DateTime? get lastMessageAt => messages.isEmpty ? null : updatedAt;

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'title': title,
    'createdAt': createdAt.toIso8601String(),
    'updatedAt': updatedAt.toIso8601String(),
    'messages': <Object?>[for (final ChatMessage m in messages) m.toJson()],
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
  /// 即使每个都到 [kMaxMessagesPerSession] 也远在配额内。思考也会落盘但被封顶
  /// （见 `chat_message.dart` 的 kPersistedReasoningMaxChars）；真顶穿配额时
  /// 保存会失败并提示，而不是静默丢历史（#9）。
  static const int maxSessions = 50;

  /// 每个会话保留的消息条数上限，超了丢**最旧**的。
  ///
  /// 与视图层的 `kChatHistoryLimit`（500）**取同一个值**：两处不一致会出现
  /// 「界面上能看到但重开就没了」这种最难查的偏差。
  static const int kMaxMessagesPerSession = 500;

  // ──────────────────────────────────────────────────────── 通知通道（#8/#9/#10）
  //
  // 这里存的是三类**本来会被静默吞掉**的事实：#8 裁剪、#9 落盘失败、#10 读取
  // 损坏。store 不持有 UI，也不自己弹任何东西——只把事实如实放在这里，由组合根
  // / 会话浮层决定怎么显示。理由：静默丢数据是本项目最贵的 bug 类型（用户以为
  // 记录还在，其实早没了），而「有地方能查到」是防它的最低成本。

  int _droppedSessionsByTrim = 0;
  int _droppedMessagesByTrim = 0;

  /// **裁剪通知**：这次运行里真的丢过东西时才非空（#8）。
  ///
  /// 说清「丢了多少、丢的是什么、上限是多少」——只说「已优化存储」等于没说。
  /// 用 getter 现算而不是存字符串：同一次运行可能先裁会话再裁消息，两个数字都
  /// 要在，存最后一句话会把前一个覆盖掉。
  String? get trimNotice {
    if (_droppedSessionsByTrim == 0 && _droppedMessagesByTrim == 0) {
      return null;
    }
    final List<String> parts = <String>[
      if (_droppedSessionsByTrim > 0)
        '会话超过上限（最多 $maxSessions 个），已自动删除最久未用的 $_droppedSessionsByTrim 个',
      if (_droppedMessagesByTrim > 0)
        '单个会话超过上限（最多 $kMaxMessagesPerSession 条），已自动丢弃最早的 $_droppedMessagesByTrim 条',
    ];
    return '${parts.join('；')}。';
  }

  /// 裁剪提示被用户看过之后调用（下次**真正**丢东西时才会再出现）。
  void clearTrimNotice() {
    _droppedSessionsByTrim = 0;
    _droppedMessagesByTrim = 0;
  }

  int _droppedLoadEntries = 0;
  bool _archiveUnreadable = false;

  /// **读取通知**：存档有坏条目 / 整个索引读不出来时非空（#10）。
  ///
  /// 以前一条坏记录就让整份存档返回空 store——用户看到的是「历史没了」，与
  /// 「被清空」无法区分。现在逐条隔离：坏条目只丢自己，但**必须说出来**。
  String? get loadWarning {
    if (_archiveUnreadable) {
      return '会话存档读不出来（格式损坏或版本不兼容），这次从空列表开始';
    }
    if (_droppedLoadEntries > 0) {
      return '有 $_droppedLoadEntries 条会话存档读不出来，已跳过（其余会话正常）';
    }
    return null;
  }

  /// 读盘时跳过的坏条目数（#10；[loadWarning] 的原始数字，便于测试与上报）。
  int get droppedLoadEntries => _droppedLoadEntries;

  /// 存档索引本身读不出来（坏 JSON / 未知版本）：留一条提示，别把「空」当成「没有」。
  void markArchiveUnreadable() {
    _archiveUnreadable = true;
  }

  void clearLoadWarning() {
    _droppedLoadEntries = 0;
    _archiveUnreadable = false;
  }

  bool _saveFailed = false;

  /// **落盘失败**：无痕模式 / 配额满 / 站点存储被禁时为 `true`（#9）。
  ///
  /// UI 据此说 [kSessionsNotPersistedNote] 那句话——不说的话，用户会以为记录
  /// 还在，刷新后才发现全没了（与 DisplayPrefs 保存失败同一条教训）。
  bool get saveFailed => _saveFailed;

  /// 保存路径写失败时调用（见 `app/browser_io.dart` 的 saveChatSessions）；**绝不抛**。
  void markSaveFailed() {
    _saveFailed = true;
  }

  /// 保存路径写成功时调用。
  void clearSaveFailure() {
    _saveFailed = false;
  }

  /// 这个 store 是从 v1 单键全量存档**迁移**出来的（#10）。
  ///
  /// 读取时在内存里换成新模型，**下一次落盘**自动写成 v2（索引 + 逐条）。
  /// 该标记只用于观测 / 测试；保存路径不依赖它决定格式（**永远**写 v2）。
  bool migratedFromLegacy = false;

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
      ..sort(
        (ChatSession a, ChatSession b) => b.updatedAt.compareTo(a.updatedAt),
      );
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

  // 这里曾经有 `pruneEmpty()`：`ChatController.newSession()` 每次新建会话都
  // 顺手删掉其他空会话。**已删除**（2026-11-18，#12）：用户点「新建」建了几个
  // 空会话、还没来得及打字，下一次新建就把它们静默清了——那是用户自己建的
  // 会话，不是垃圾。空会话现在**只由用户显式删除**（[delete]）。

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

  /// 序列化**整份快照**（v2 版本号 + 内联 sessions）。
  ///
  /// **不是真实落盘格式**：真实落盘是「索引 + 每个会话一条键」
  /// （[toIndexJson] + [ChatSession.toJson]，见 `app/browser_io.dart`）。
  /// 保留这个函数是为了单测 / 导出 / 备份能一条函数往返，也让
  /// [ChatSessionStore.fromJson] 有一个对称的写入口。
  Map<String, Object?> toJson() => <String, Object?>{
    'version': kChatStoreVersion,
    'activeId': activeId,
    'sessions': <Object?>[for (final ChatSession s in sessions) s.toJson()],
  };

  /// 存档**索引**（v2 的真实落盘格式之一）：只放版本、当前会话、id 列表。
  ///
  /// 索引刻意**不内联会话正文**：v1 的毛病就是「一个键装全部」——任何一条
  /// 记录坏掉，`jsonDecode` 整份失败，历史全没了（#10 的现场）。拆开之后，
  /// 一条坏记录只影响它自己。
  Map<String, Object?> toIndexJson() => <String, Object?>{
    'version': kChatStoreVersion,
    'activeId': activeId,
    'ids': <String>[for (final ChatSession s in sessions) s.id],
  };

  /// 反序列化**整份快照**：**永不抛**。坏条目丢弃，未知版本整体丢弃。
  ///
  /// 两种输入都认（见 [kChatStoreVersion] / [kChatStoreLegacyVersion]）：
  /// - v1：旧版**单键全量**存档（`sessions` 数组）。读到就在内存里迁移，并标
  ///   [migratedFromLegacy]——下一次落盘自动写成 v2；
  /// - v2：带 `sessions` 数组的整份快照（测试 / 导出 / 备份）。真实加载**不走**
  ///   这里，而是 [decodeChatSessionIndex] + [loadChatSessionEntries] 的逐条
  ///   隔离路径。
  factory ChatSessionStore.fromJson(Object? raw) {
    if (raw is! Map) return ChatSessionStore.empty();
    final Map<String, Object?> json = raw.cast<String, Object?>();
    final Object? version = json['version'];
    // 版本门禁：不认识的版本**整体丢弃**而不是尽力解析——半懂不懂地读
    // 一份未来格式，比重来一遍更危险（可能把新字段写没了）。
    // v1 例外：旧存档必须能读（读法仍是「全量 sessions 数组」）。
    if (version != kChatStoreVersion && version != kChatStoreLegacyVersion) {
      return ChatSessionStore.empty();
    }

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
    store.migratedFromLegacy = version == kChatStoreLegacyVersion;
    store._trimSessions();
    for (final ChatSession s in store.sessions) {
      store._trimMessages(s);
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
      // 删了就要说（#8）：静默丢一个会话，用户会以为是自己删的 / 存档坏了。
      _droppedSessionsByTrim++;
    }
  }

  void _trimMessages(ChatSession session) {
    final int excess = session.messages.length - kMaxMessagesPerSession;
    if (excess <= 0) return;
    session.messages.removeRange(0, excess);
    // 同理：丢的是**最早的**记录，不提示就永远不知道上下文为什么断了。
    _droppedMessagesByTrim += excess;
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

/// 存档**索引**的格式版本（#10，2026-11-18：v1 → v2）。
///
/// v2 = 索引（本键）+ 每个会话一条独立键（`<索引键>.<id>`）。
/// 详见 [ChatSessionLoadPlan] 与 `app/browser_io.dart` 的保存 / 读取路径。
const int kChatStoreVersion = 2;

/// 旧版**单键全量**格式版本：`{version:1, activeId, sessions:[...]}`。
///
/// 仍然必须能读——用户升级前保存的存档只有这一种。读取时在内存里迁移成
/// store（[ChatSessionStore.migratedFromLegacy]），下一次落盘自动写成 v2。
const int kChatStoreLegacyVersion = 1;

/// 落盘失败时给用户的一句话（#9）：无痕模式 / 配额满 / 站点存储被禁。
///
/// 放在纯逻辑层而不是 UI：它是**事实陈述**，不是排版；UI 只决定显示在哪。
/// 不说这句话，用户会以为记录还在，刷新后才发现全没了。
const String kSessionsNotPersistedNote = '会话只保留在本次页面里（浏览器存储不可用或已满）。';

/// 会话列表里给 id 短提示加的标签（#7：必须读起来像「编号」，不是标题）。
const String kSessionIdHintLabel = '编号';

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

/// 存档格式（v2，2026-11-18，#10）：**索引 + 每个会话一条独立键**。
///
/// ```
/// live2d-ai.chat-sessions            → {"version":2,"activeId":"…","ids":["…"]}
/// live2d-ai.chat-sessions.<会话 id>  → 一条 ChatSession 的 JSON
/// ```
///
/// 为什么要拆：v1 是「一个键装全量 sessions 数组」，任何一条记录坏掉都会让
/// `jsonDecode` 整份失败，于是用户看到「历史全没了」——**一条坏数据吃掉所有
/// 会话**。拆开之后每条独立解码，坏一条只丢一条，并被计入
/// [ChatSessionStore.loadWarning]。
///
/// 键名定义在 `app/browser_io.dart`（那里才允许 `package:web`）；本文件只管
/// **纯 JSON 结构**，于是整条加载 / 保存逻辑都能在 VM 上单测。
/// v2 索引的解码结果：告诉调用方「该去读哪些逐条键」。
///
/// 纯逻辑（本文件零 web 依赖）：`browser_io` 只负责 `getItem`，「索引长什么样、
/// 什么算坏」全部在这里判，才能在 VM 上测到（#10 的可测部分）。
class ChatSessionLoadPlan {
  ChatSessionLoadPlan._({
    this.ids = const <String>[],
    this.activeId,
    this.legacyStore,
    this.indexCorrupt = false,
  });

  /// v2：需要逐条读取的会话 id（顺序 = 保存时顺序；展示仍按 updatedAt 重排）。
  final List<String> ids;

  /// v2：存档里的当前会话 id（由 [loadChatSessionEntries] 校验存在性）。
  final String? activeId;

  /// v1：已经在内存里迁移好的 store。**非空 = 不需要逐条读取**。
  final ChatSessionStore? legacyStore;

  /// 索引本身读不出来（坏 JSON / 未知版本 / 类型不对）。
  ///
  /// 与「没有存档」区分开：前者要给一句「历史没读出来」，后者应该安静。
  final bool indexCorrupt;

  /// 是不是旧版（v1）单键全量存档：是的话下一次落盘会自动写成 v2。
  bool get isLegacy => legacyStore != null;
}

/// 解码索引原文（`localStorage['live2d-ai.chat-sessions']` 的 value）。
///
/// **永不抛**：坏 JSON / 类型不对 → [ChatSessionLoadPlan.indexCorrupt]；
/// 空串 / null → 「没有存档」（安静返回空 plan，不算损坏）。
///
/// v1 存档在这里**直接迁移**（[ChatSessionStore.fromJson] 读全量 sessions），
/// 后续保存路径总是写 v2，所以迁移只需发生在内存里。
ChatSessionLoadPlan decodeChatSessionIndex(String? raw) {
  if (raw == null || raw.isEmpty) return ChatSessionLoadPlan._();
  Object? decoded;
  try {
    decoded = jsonDecode(raw);
  } catch (_) {
    return ChatSessionLoadPlan._(indexCorrupt: true);
  }
  if (decoded is! Map) return ChatSessionLoadPlan._(indexCorrupt: true);
  final Map<String, Object?> json = decoded.cast<String, Object?>();
  final Object? version = json['version'];
  if (version == kChatStoreLegacyVersion) {
    final ChatSessionStore store = ChatSessionStore.fromJson(json);
    // fromJson 只在版本是 v1 时才认（这里已经判过）；迁移标记再显式写一次是
    // 防御——就算标记丢了下一次保存也不会写错格式（保存**恒**写 v2）。
    store.migratedFromLegacy = true;
    return ChatSessionLoadPlan._(legacyStore: store);
  }
  if (version != kChatStoreVersion) {
    // 未知版本 = 未来格式：不半懂不懂地读（可能把新字段写没了）。
    return ChatSessionLoadPlan._(indexCorrupt: true);
  }
  final List<String> ids = <String>[];
  final Object? rawIds = json['ids'];
  if (rawIds is List) {
    for (final Object? item in rawIds) {
      if (item is String && item.isNotEmpty && !ids.contains(item)) {
        ids.add(item);
      }
    }
  }
  final Object? rawActiveId = json['activeId'];
  return ChatSessionLoadPlan._(
    ids: ids,
    activeId: rawActiveId is String ? rawActiveId : null,
  );
}

/// v2 逐条加载：索引 + 每个 id 的**条目 JSON 原文** → store。
///
/// [entryJsonById] 的 value 为 `null` = 该键不存在（缺失也算坏条目）。
/// **逐条隔离**：任何一条解不出 / 不是合法会话，只跳过它自己并计数，其余照常
/// 加载（#10 的修复本体）。
///
/// 返回的 store 上：
/// - [ChatSessionStore.droppedLoadEntries] / [ChatSessionStore.loadWarning]
///   记录跳过了几条；
/// - activeId 指向被跳过 / 不存在的会话时当作没选（不凭空选一个）；
/// - 加载完仍受 [ChatSessionStore.maxSessions] /
///   [ChatSessionStore.kMaxMessagesPerSession] 夹持（存档被外部塞爆也要夹住）。
ChatSessionStore loadChatSessionEntries(
  ChatSessionLoadPlan plan,
  Map<String, String?> entryJsonById,
) {
  final List<ChatSession> sessions = <ChatSession>[];
  int dropped = 0;
  for (final String id in plan.ids) {
    final ChatSession? session = parseChatSessionEntry(entryJsonById[id]);
    if (session == null) {
      dropped++;
      continue;
    }
    sessions.add(session);
  }
  final String? activeId = plan.activeId;
  final ChatSessionStore store = ChatSessionStore(
    sessions: sessions,
    activeId: sessions.any((ChatSession s) => s.id == activeId)
        ? activeId
        : null,
  );
  store._droppedLoadEntries = dropped;
  store._trimSessions();
  for (final ChatSession s in store.sessions) {
    store._trimMessages(s);
  }
  return store;
}

/// 单条会话的 JSON 原文 → [ChatSession]；坏数据返回 `null`（**绝不抛**）。
ChatSession? parseChatSessionEntry(String? raw) {
  if (raw == null || raw.isEmpty) return null;
  try {
    return ChatSession.fromJson(jsonDecode(raw));
  } catch (_) {
    return null;
  }
}

/// 索引的 JSON 字符串（写 `localStorage['live2d-ai.chat-sessions']` 用）。
String encodeChatSessionIndex(ChatSessionStore store) =>
    jsonEncode(store.toIndexJson());

/// 单条会话的 JSON 字符串（写 `live2d-ai.chat-sessions.<id>` 用）。
String encodeChatSessionEntry(ChatSession session) =>
    jsonEncode(session.toJson());

/// 会话 id 的**短提示**（列表副标题里显示，帮用户在几个同名会话之间对上号，#7）。
///
/// id 形如 `<微秒时间戳>-<同刻序号>`，末尾 6 个字符同时含时间戳低位与序号，
/// 足够区分同一屏里的会话；显示全长（十六七位数字）会把副标题挤爆。
/// **只是显示用**：不参与持久化、不改变 [ChatSession.id] 的语义。
String sessionIdHint(String id, {int maxChars = 6}) {
  final List<int> runes = id.runes.toList();
  if (runes.length <= maxChars) return id;
  return String.fromCharCodes(runes.skip(runes.length - maxChars));
}
