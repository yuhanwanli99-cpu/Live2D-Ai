/// 聊天消息模型：**纯逻辑，无 `package:web` 依赖**。
///
/// 从 `chat_controller.dart` 抽出来的理由：那个文件必须 `import 'package:web'`
/// （经 `ws_client.dart`），于是 `ChatMessage` 在 `flutter test` 里加载不了——
/// 而 `MessageBubble`（气泡：角色标签 / 流式光标 / 失败态）是**纯展示**，
/// 它没有任何理由不可测。
///
/// 规格 §10.2 的同一条纪律：**能被单测的东西不要放在 web 依赖后面**。
library;

/// 消息角色。
enum ChatRole {
  user,
  assistant,

  /// **系统提示**：既不是用户说的，也不是模型说的。
  ///
  /// 存在的理由（2026-09-11，用户报「第一次对话之后没输出了」）：模型有时会
  /// **整轮不返回文字**，而那是一个**正常完成**的回合——
  /// `turn_state{completed}`、没有任何 `error` 帧、TTS 一次都没被调用。
  /// 前端曾经把这种空气泡**直接删掉**，于是界面上「什么都没有」，
  /// 与「应用坏了」无法区分。
  /// （当时的原因是「模型去调用了动作工具」；2026-09-11 用户裁定 LLM 无工具后
  /// 该原因已不存在，但「没有文字的正常回合」这个现象本身与原因无关。）
  ///
  /// 现在改成放一条这个角色的消息：事实照说（`kWordlessTurnNotice`），
  /// 但**不伪装成模型说的话**——它有自己的面色与语义标签（「系统」），
  /// 读屏与视觉都不会把它当成角色的回复。
  system;

  /// 界面上给人看的短标签。
  String get label => switch (this) {
    ChatRole.user => '你',
    ChatRole.assistant => '助手',
    ChatRole.system => '系统',
  };
}

/// 会话里**最后一条用户消息**的正文（没有则 `null`）。
///
/// 用途单一：busy 的「打断并重发」要从这里取回正文（F-0007-2，审计 2026-09-28）。
/// `_send` 在 POST **之前**就 `clear()` 了输入框，所以重发**不能**读输入框
/// ——读它只会拿到空串，然后被 `send()` 的空文本早退静默吞掉，
/// 于是按钮又退化成「只打断」（那正是本缺陷的现场）。
///
/// 从后往前找 `ChatRole.user`，而不是取 `messages.last`：busy 之后最后一条
/// 是 assistant 的失败气泡（`⚠ 发送未被受理（服务端忙碌）`），把它当用户的话
/// 重发出去就是**伪造一条用户消息**。
///
/// 纯函数（本文件零 web 依赖）：判据要在 VM 上测得到——`ChatController`
/// 经 `ws_client.dart` 间接依赖 `package:web`，它里面的分支跑不到。
String? lastUserText(List<ChatMessage> messages) {
  for (int i = messages.length - 1; i >= 0; i--) {
    final ChatMessage message = messages[i];
    if (message.role != ChatRole.user) continue;
    if (message.text.trim().isEmpty) continue;
    return message.text;
  }
  return null;
}

/// 一句已上屏的正文。`seq` 与正在播放的音频句号是同一个数。
///
/// # 为什么要落盘（2026-11-18，#11）
///
/// 它是**跟读高亮**的坐标：`MessageBubble` 用 `spokenHighlight` 在它上面算
/// 当前句子的区间。以前不落盘，刷新页面后每个气泡都只剩拼接好的 [text]，
/// 高亮永远回不来——而「刚才那句在哪里」恰恰是回看时最想找的东西。
///
/// 体积不是问题：每句正文本来就在 [text] 里各出现一次，`spoken` 只是把同一批
/// 文字**再按句号切一遍**（总字符数至多与正文相同）。
class SpokenSpan {
  SpokenSpan({required this.seq, required this.text});

  final int seq;
  final String text;

  /// 落盘用（见 [ChatMessage.toJson]）。
  Map<String, Object?> toJson() => <String, Object?>{'seq': seq, 'text': text};

  /// 反序列化：坏条目返回 `null`（丢这一句，不丢整条消息）。
  ///
  /// `seq` 必须是 int、`text` 必须是 String：缺一个，高亮就算不出区间——
  /// 留着它只会在 `spokenHighlight` 里制造一个静默的空 span。
  static SpokenSpan? fromJson(Object? raw) {
    if (raw is! Map) return null;
    final Object? seq = raw['seq'];
    final Object? text = raw['text'];
    if (seq is! int || text is! String) return null;
    return SpokenSpan(seq: seq, text: text);
  }
}

/// 思考正文落盘时的**字符上限**（按 rune 计，2026-11-18，#11）。
///
/// 取值理由：思考通常比正文长 5–20 倍（实测一个普通提问 1200+ 字思考 vs 20 字
/// 回复）。落盘是为了刷新后还能展看「它当时怎么想的」，但不设上限的话，一个
/// 长会话光思考就能把 localStorage（常见配额 5 MB）顶穿——届时**整份存档**写入
/// 失败，用户连正文历史都看不到。2000 字 ≈ 4–8 KB/条，最坏情况随
/// `ChatSessionStore.kMaxMessagesPerSession`（500）线性放大，仍在配额内。
///
/// 超限时截断并加 [kPersistedReasoningTruncatedMarker]，而不是丢掉整个字段：
/// 保留开头比「什么都没有」诚实（代价只是尾巴不见）。
const int kPersistedReasoningMaxChars = 2000;

/// 思考正文被截断时补在末尾的标记（见 [kPersistedReasoningMaxChars]）。
const String kPersistedReasoningTruncatedMarker = '…';

/// 把思考正文裁到落盘上限：短的原样返回，长的截断加省略号。
///
/// **幂等**（截断过的再截一次结果不变），所以写盘与读盘两侧都能放心调用。
String capPersistedReasoning(String text) {
  final List<int> runes = text.runes.toList();
  if (runes.length <= kPersistedReasoningMaxChars) return text;
  return '${String.fromCharCodes(runes.take(kPersistedReasoningMaxChars))}$kPersistedReasoningTruncatedMarker';
}

/// 一条聊天消息（assistant 气泡可流式追加）。
///
/// **可变**是刻意的：流式追加是每帧一次的高频操作，每条 delta 都重建一个
/// 不可变对象会制造大量垃圾。可变性的代价用「只在 `ChatController` 里改、
/// 展示层只读」来约束。
class ChatMessage {
  ChatMessage({
    required this.role,
    this.text = '',
    this.epoch,
    this.streaming = false,
    this.failed = false,
    this.reasoning = '',
    this.unfinished = false,
  });

  final ChatRole role;
  String text;

  /// 本轮业务代次（服务端受理时给出）。
  ///
  /// **可变**（2026-09-23）：气泡现在在 POST **之前**就建好，拿到受理响应后
  /// 才回填代次——这样「服务端在 HTTP 应答之前就跑完本轮并广播 error /
  /// turn_state」的快失败不会因为气泡还不存在而丢掉收口（见 ChatController.send）。
  int? epoch;
  bool streaming;
  bool failed;

  /// 推理模型的**思考**正文（`reasoning_delta` 累积；2026-09-13）。
  ///
  /// # 落盘与上限（2026-11-18，#11）
  ///
  /// 以前刻意不落盘：思考通常比正文长 5–20 倍，整段写进 localStorage 会让存档
  /// 膨胀一个数量级。代价是**刷新页面后思考折叠区永远是空的**——用户回看一轮
  /// 时最想找的「它当时怎么想的」恰好没了。
  ///
  /// 现在改成「落盘但封顶」：写到 [kPersistedReasoningMaxChars] 个字符（rune）
  /// 就截断并加省略号（[kPersistedReasoningTruncatedMarker]）。读盘一侧同样封顶
  /// ——存档可能来自别的版本或被人手工塞大，上限只有在写与读两侧都成立才算数。
  String reasoning;

  /// 本轮已上屏的句子（`text_delta` 的 `sentence_seq` + 该句正文）。
  ///
  /// 落盘（2026-11-18，#11）：刷新后跟读高亮的坐标还在，不再是「只剩拼接好的
  /// [text]」。序号来自服务端，**不要**用标点把 [text] 再切一遍——切法不同会
  /// 让高亮落在错的字上。
  final List<SpokenSpan> spoken = <SpokenSpan>[];

  /// 这一轮的正文是**失败兜底**（`text_fallback` 帧）来的：真实生成、
  /// 但没有语音收尾（rc.3 N0，2026-09-13）。
  ///
  /// 只在气泡上加一条说明行（[kUnfinishedTurnCaption]），**不改**文字本身——
  /// 文字是真实收到的内容，丢了就是篡改记录。它**必须落盘**（#11 之后
  /// [reasoning] 也落盘了）：刷新后那段文字还在，就必须还带着「未收尾」这个限定。
  bool unfinished;

  /// 是否是一条「系统提示」而不是真实回复（失败/无输出的占位文案）。
  ///
  /// 气泡据此换底色（`dangerSurface`），而不是靠「文字里有没有 ⚠」。
  bool get isPlaceholder => failed;

  /// 序列化（写进 localStorage 的会话存档）。
  ///
  /// **不存 `streaming`**：存下来的消息永远是「已完成」的——把
  /// `streaming: true` 写进存储，重开页面会得到一个永远转圈的空气泡。
  ///
  /// **存 [reasoning] 与 [spoken]**（2026-11-18，#11）：刷新后思考折叠区与
  /// 跟读高亮都靠它们，否则回看旧对话时只剩一段没有「为什么」的正文。
  /// 两者都只在非空时写键（旧存档 / 空思考不占地方）。
  Map<String, Object?> toJson() => <String, Object?>{
    'role': role.name,
    'text': text,
    if (epoch != null) 'epoch': epoch,
    if (failed) 'failed': true,
    if (unfinished) 'unfinished': true,
    if (reasoning.isNotEmpty) 'reasoning': capPersistedReasoning(reasoning),
    if (spoken.isNotEmpty)
      'spoken': <Object?>[for (final SpokenSpan span in spoken) span.toJson()],
  };

  /// 反序列化：**坏数据返回 `null`（丢弃这一条），绝不抛**。
  ///
  /// 与 `DisplayPrefs.fromJson` 同一条纪律：一份被改坏的存储不该让整个
  /// 聊天区打不开——丢一条消息的代价远小于丢整段历史。
  static ChatMessage? fromJson(Object? raw) {
    if (raw is! Map) return null;
    final Object? rawRole = raw['role'];
    if (rawRole is! String) return null;
    ChatRole? role;
    for (final ChatRole candidate in ChatRole.values) {
      if (candidate.name == rawRole) role = candidate;
    }
    if (role == null) return null;
    final Object? rawText = raw['text'];
    if (rawText is! String) return null;
    final Object? rawEpoch = raw['epoch'];
    // 旧存档没有这两个字段（#11 之前）：缺省空，不要因此丢掉整条消息。
    final Object? rawReasoning = raw['reasoning'];
    final String reasoning = rawReasoning is String
        ? capPersistedReasoning(rawReasoning)
        : '';
    final List<SpokenSpan> spoken = <SpokenSpan>[];
    final Object? rawSpoken = raw['spoken'];
    if (rawSpoken is List) {
      for (final Object? item in rawSpoken) {
        final SpokenSpan? span = SpokenSpan.fromJson(item);
        if (span != null) spoken.add(span);
      }
    }
    return ChatMessage(
      role: role,
      text: rawText,
      epoch: rawEpoch is int ? rawEpoch : null,
      failed: raw['failed'] == true,
      unfinished: raw['unfinished'] == true,
      reasoning: reasoning,
    )..spoken.addAll(spoken);
  }
}
