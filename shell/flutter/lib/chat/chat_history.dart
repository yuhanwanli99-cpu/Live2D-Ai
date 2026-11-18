/// 本地会话历史 → `POST /api/v1/chat` 的 `history` 字段：**纯逻辑，零 web 依赖**。
///
/// # 为什么需要它（2026-11-18）
///
/// 协议从前只收 `{text, session_id}`，模型每轮都是「失忆」的：本地会话切来
/// 切去，气泡各自独立，而模型把每一句都当成第一句。协议加上可选 `history` 后，
/// 由前端把**当前会话**最近几轮喂回去，每个本地会话才真的是模型眼里的一个
/// 独立上下文（服务端只把它拼进本轮请求，不写自己的历史窗口）。
///
/// # 三条边界（与服务端 `chat_routes.rs` 的兜底口径一致）
///
/// - 只发 `user` / `assistant`：`ChatRole.system`（「本轮没有文字」之类系统占位）
///   与 `failed` 占位都不是模型说过的话，发过去只会污染上下文；
/// - **不带当前这句**：调用点是在把用户输入 append 进会话**之后**取历史的，
///   列表最后一条用户消息就是本次请求的正文，再放进 `history` 模型会读成两句
///   （`echoUser: false` 的重发路径同样成立——那条用户消息本来就是本次正文）；
/// - 硬上限 8 对（16 条）/ 每条 2000 字符：越界时丢**最旧**的（越近越相关），
///   单条按字符截断而不是整条丢弃。
library;

import 'chat_message.dart';

/// 协议 `history` 的硬上限：8 对（user + assistant）= 16 条。
const int kChatHistoryMaxPairs = 8;

/// 条数上限（8 对）。按**条**裁而不是按对：失败 / 空正文被过滤后，
/// 消息列表本来就可能不是严格交替的，按对裁会把边界算错。
const int kChatHistoryMaxMessages = kChatHistoryMaxPairs * 2;

/// 单条正文的字符上限（与 Rust `MAX_CHAT_HISTORY_TEXT_CHARS` 同值）。
const int kChatHistoryMaxTextChars = 2000;

/// 一条要发给服务端的本地历史。`role` 是 wire 值：只可能是 `user` / `assistant`。
class ChatHistoryEntry {
  const ChatHistoryEntry({required this.role, required this.text});

  /// wire 角色：`user` / `assistant`（只此两种，服务端会忽略其余）。
  final String role;

  /// 正文（已按 [kChatHistoryMaxTextChars] 截断）。
  final String text;

  /// 协议形状：`{"role": "...", "text": "..."}`。
  Map<String, Object?> toJson() => <String, Object?>{
    'role': role,
    'text': text,
  };
}

/// 从**当前会话的消息列表**构建本次请求的 `history`。
///
/// [messages] 就是 `ChatSession.messages`（调用点已 append 本次用户输入）：
/// 尾部那条用户消息会被排除——它是本次请求的正文，不是历史。
/// 全空 / 只有系统占位时回空表：调用方据此**不发送** `history` 字段，
/// 老协议（absent = 无历史）逐字不变。
List<ChatHistoryEntry> buildChatHistory(List<ChatMessage> messages) {
  final List<ChatHistoryEntry> entries = <ChatHistoryEntry>[];
  for (final ChatMessage message in messages) {
    // 系统 / 失败占位不是对话内容：它们解释的是「这一轮怎么了」，不是模型说过的话。
    if (message.role == ChatRole.system || message.failed) continue;
    final String text = message.text;
    if (text.trim().isEmpty) continue;
    entries.add(
      ChatHistoryEntry(
        role: message.role == ChatRole.user ? 'user' : 'assistant',
        text: _capChatHistoryText(text),
      ),
    );
  }
  // 尾部用户消息 = 本次输入的正文（见文件头注第二条边界）。
  if (entries.isNotEmpty && entries.last.role == 'user') entries.removeLast();
  if (entries.length > kChatHistoryMaxMessages) {
    entries.removeRange(0, entries.length - kChatHistoryMaxMessages);
  }
  // 兜底：裁完起点若落在 assistant，丢掉直到第一条 user——部分上游的 chat
  // 模板要求 system 之后先出现 user；从半轮开始的上下文本来也没有价值。
  while (entries.isNotEmpty && entries.first.role == 'assistant') {
    entries.removeAt(0);
  }
  return entries;
}

/// 按**字符**（rune）截断：中文 / emoji 在 UTF-16 下可能占两个 code unit，
/// 按 `String.length` 切会把字切坏。
String _capChatHistoryText(String text) {
  final List<int> runes = text.runes.toList();
  if (runes.length <= kChatHistoryMaxTextChars) return text;
  return String.fromCharCodes(runes.take(kChatHistoryMaxTextChars));
}
