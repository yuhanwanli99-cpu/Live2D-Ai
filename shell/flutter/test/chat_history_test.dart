/// 本地会话历史 → 请求体 `history` 的 Dart VM 回归（纯逻辑，不碰 web 依赖）。
///
/// 守三件事：① [buildChatHistory] 的取舍边界（系统 / 失败占位、当前这句、
/// 8 对 / 2000 字上限）；② `ApiClient.sendChat` 只在有历史时才序列化
/// `history`（absent = 老协议逐字不变）；③ wire 形状 `{role, text}` 与服务端
/// `sanitize_history` 的解析口径一致。
library;

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/chat/chat_history.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';

ChatMessage _user(String text) => ChatMessage(role: ChatRole.user, text: text);

ChatMessage _assistant(String text, {bool failed = false}) =>
    ChatMessage(role: ChatRole.assistant, text: text, failed: failed);

ChatMessage _system(String text) =>
    ChatMessage(role: ChatRole.system, text: text, failed: true);

void main() {
  group('buildChatHistory：取舍边界', () {
    test('只带 user / assistant：系统占位、失败占位、空正文都被排除', () {
      final List<ChatHistoryEntry> history = buildChatHistory(<ChatMessage>[
        _user('第一问'),
        _assistant('第一答'),
        _system('本轮没有文字'),
        _user('第二问'),
        _assistant('（失败占位）', failed: true),
        _assistant('   '),
        _user('当前这句'),
      ]);
      expect(
        history.map((ChatHistoryEntry e) => e.toJson()).toList(),
        <Map<String, Object?>>[
          <String, Object?>{'role': 'user', 'text': '第一问'},
          <String, Object?>{'role': 'assistant', 'text': '第一答'},
          <String, Object?>{'role': 'user', 'text': '第二问'},
        ],
      );
    });

    test('排除尾部当前用户消息（调用点在 append 之后取历史）', () {
      final List<ChatHistoryEntry> history = buildChatHistory(<ChatMessage>[
        _user('上一问'),
        _assistant('上一答'),
        _user('正在发的这句'),
      ]);
      expect(history.length, 2, reason: '当前这句不得进 history（模型会读成两句）');
      expect(history.last.text, '上一答');
    });

    test('只有当前一句 / 空列表 → 空历史（调用方据此不发送 history 字段）', () {
      expect(buildChatHistory(const <ChatMessage>[]), isEmpty);
      expect(buildChatHistory(<ChatMessage>[_user('第一句')]), isEmpty);
      expect(
        buildChatHistory(<ChatMessage>[_system('本轮没有文字'), _user('第一句')]),
        isEmpty,
      );
    });

    test('上限 8 对（16 条）：丢最旧的，保留最近的', () {
      final List<ChatMessage> messages = <ChatMessage>[];
      for (int i = 0; i < 12; i++) {
        messages.add(_user('问$i'));
        messages.add(_assistant('答$i'));
      }
      messages.add(_user('当前这句'));
      final List<ChatHistoryEntry> history = buildChatHistory(messages);
      expect(history.length, kChatHistoryMaxMessages);
      expect(history.length, 16);
      expect(history.first.toJson(), <String, Object?>{
        'role': 'user',
        'text': '问4',
      });
      expect(history.last.text, '答11');
    });

    test('单条按字符截到 2000（不切坏中文 / emoji）', () {
      final String long = List<String>.filled(
        kChatHistoryMaxTextChars + 500,
        '字',
      ).join();
      final List<ChatHistoryEntry> history = buildChatHistory(<ChatMessage>[
        _user(long),
        _assistant('答'),
        _user('当前这句'),
      ]);
      expect(history.first.text.runes.length, kChatHistoryMaxTextChars);
      // rune 边界没被切坏：整串都是同一个汉字。
      expect(history.first.text.split('').toSet().length, 1);
    });

    test('裁完若落在半轮：丢掉开头的 assistant，历史必须从 user 开始', () {
      final List<ChatMessage> messages = <ChatMessage>[
        _assistant('上一轮的残句'), // 旧存档里可能出现的孤立 assistant
      ];
      for (int i = 0; i < 12; i++) {
        messages.add(_user('问$i'));
        messages.add(_assistant('答$i'));
      }
      messages.add(_user('当前这句'));
      final List<ChatHistoryEntry> history = buildChatHistory(messages);
      expect(history, isNotEmpty);
      expect(
        history.first.role,
        'user',
        reason: '部分上游要求 system 之后先出现 user；半轮上下文没有价值',
      );
    });
  });

  group('ApiClient.sendChat：history 只在有内容时上线', () {
    test('history 序列化为 [{role,text}]；空表 / 缺省时字段缺席', () async {
      final List<Map<String, Object?>> bodies = <Map<String, Object?>>[];
      final ApiClient api = ApiClient(
        base: 'http://127.0.0.1:18080',
        client: MockClient((http.Request request) async {
          bodies.add(jsonDecode(request.body) as Map<String, Object?>);
          return http.Response(
            '{"accepted":true,"epoch":1,"pending_cleared":false}',
            200,
            headers: const <String, String>{'content-type': 'application/json'},
          );
        }),
      );

      await api.sendChat(
        '当前这句',
        sessionId: 's-1',
        history: const <ChatHistoryEntry>[
          ChatHistoryEntry(role: 'user', text: '上一问'),
          ChatHistoryEntry(role: 'assistant', text: '上一答'),
        ],
      );
      expect(bodies.single['text'], '当前这句');
      expect(bodies.single['session_id'], 's-1');
      expect(bodies.single['history'], <Map<String, Object?>>[
        <String, Object?>{'role': 'user', 'text': '上一问'},
        <String, Object?>{'role': 'assistant', 'text': '上一答'},
      ]);

      // 空表（第一轮 / 没有可带的历史）→ 不发字段：老协议逐字不变。
      await api.sendChat(
        '再来一句',
        sessionId: 's-1',
        history: const <ChatHistoryEntry>[],
      );
      expect(bodies.last.containsKey('history'), isFalse);
      // 缺省参数同样不发。
      await api.sendChat('老客户端', sessionId: 's-1');
      expect(bodies.last.containsKey('history'), isFalse);
    });
  });
}
