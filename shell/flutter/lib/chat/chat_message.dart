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
  });

  final ChatRole role;
  String text;
  final int? epoch;
  bool streaming;
  bool failed;

  /// 推理模型的**思考**正文（`reasoning_delta` 累积；2026-09-13）。
  ///
  /// # 为什么不落盘（`toJson` / `fromJson` 都不含它）
  ///
  /// 思考通常比正文长 **5–20 倍**（实测一个普通提问 1200+ 字思考 vs 20 字回复）。
  /// 写进 localStorage 会让会话存档膨胀一个数量级，而它的价值只在
  /// 「刚发生的那一轮」——用户想看的是「它刚才怎么想的」。
  ///
  /// 代价说清楚：**刷新页面后旧气泡不再有思考**。这是刻意的取舍，不是遗漏。
  /// 要改成持久化，需要同时给会话存档加体积上限（否则长会话会顶穿配额）。
  String reasoning;

  /// 是否是一条「系统提示」而不是真实回复（失败/无输出的占位文案）。
  ///
  /// 气泡据此换底色（`dangerSurface`），而不是靠「文字里有没有 ⚠」。
  bool get isPlaceholder => failed;

  /// 序列化（写进 localStorage 的会话存档）。
  ///
  /// **不存 `streaming`**：存下来的消息永远是「已完成」的——把
  /// `streaming: true` 写进存储，重开页面会得到一个永远转圈的空气泡。
  Map<String, Object?> toJson() => <String, Object?>{
    'role': role.name,
    'text': text,
    if (epoch != null) 'epoch': epoch,
    if (failed) 'failed': true,
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
    return ChatMessage(
      role: role,
      text: rawText,
      epoch: rawEpoch is int ? rawEpoch : null,
      failed: raw['failed'] == true,
    );
  }
}
