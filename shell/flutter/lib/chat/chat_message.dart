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
  assistant;

  /// 界面上给人看的短标签。
  String get label => switch (this) {
    ChatRole.user => '你',
    ChatRole.assistant => '助手',
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
  });

  final ChatRole role;
  String text;
  final int? epoch;
  bool streaming;
  bool failed;

  /// 是否是一条「系统提示」而不是真实回复（失败/无输出的占位文案）。
  ///
  /// 气泡据此换底色（`dangerSurface`），而不是靠「文字里有没有 ⚠」。
  bool get isPlaceholder => failed;
}
