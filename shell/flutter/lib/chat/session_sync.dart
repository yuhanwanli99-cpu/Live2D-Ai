part of 'chat_controller.dart';

/// ChatController 的会话同步（L1 会话绑定）。
///
/// 单列 part 文件：chat_controller.dart 已到 Dart 800 行棘轮（rc.3 收口）。
/// part 与主库共享 import 与私有成员，这里只是把方法面挪出去，行为一字不变。
extension _ChatSessionSync on ChatController {
  /// 把当前活动会话告诉服务端（L1 会话绑定）。
  ///
  /// 只影响**不带会话 id 的注入路径**（external-input 弹幕 / voice-input 转写）：
  /// 它们接在当前这段对话上，所以必须知道用户正在看哪个会话。聊天消息本身
  /// 每次都显式带 `session_id`（见 `send`），不依赖这里。
  ///
  /// 失败不抛（见 `ApiClient.setActiveSession` 的说明）。
  void _syncActiveSession() {
    unawaited(api.setActiveSession(sessions.activeId));
  }
}
