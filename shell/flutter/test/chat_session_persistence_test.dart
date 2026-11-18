/// 会话存档 v2：逐条隔离 / v1 迁移 / 通知通道 / 思考与跟读落盘
/// （2026-11-18，#8 / #9 / #10 / #11）。
///
/// 为什么单独一份：`chat_session_test.dart` 已经 500 行，而这一波的判据
/// （一条坏记录不许吃掉整份历史）需要大量「故意弄坏存档」的样本，混进去会让
/// 那份文件既超上限又难读。
///
/// 本文件只测**纯逻辑**：`browser_io` 的 localStorage I/O 在 VM 上跑不了
/// （`package:web`），所以判据全部下沉到 `chat/chat_session.dart` 的
/// `decodeChatSessionIndex` / `loadChatSessionEntries` 等纯函数——
/// 本文件用「索引字符串 + 条目字符串表」模拟 localStorage，测的是同一套代码。
library;

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/chat_session.dart';

ChatMessage user(String t) => ChatMessage(role: ChatRole.user, text: t);
ChatMessage bot(String t) => ChatMessage(role: ChatRole.assistant, text: t);

DateTime at(int minute) => DateTime(2026, 11, 18, 12, minute);

/// 把 store 编码成「索引 + 每会话一条」的字符串表（= localStorage 的替身）。
Map<String, String?> encodeArchive(ChatSessionStore store) => <String, String?>{
  'index': encodeChatSessionIndex(store),
  for (final ChatSession s in store.sessions) s.id: encodeChatSessionEntry(s),
};

/// 按 `browser_io.loadChatSessions` 的同一流程从字符串表加载。
ChatSessionStore loadArchive(Map<String, String?> archive) {
  final ChatSessionLoadPlan plan = decodeChatSessionIndex(archive['index']);
  final ChatSessionStore? legacy = plan.legacyStore;
  if (legacy != null) return legacy;
  final Map<String, String?> entries = <String, String?>{
    for (final String id in plan.ids) id: archive[id],
  };
  return loadChatSessionEntries(plan, entries);
}

/// 建一个有三条会话、每条一句用户话的 store。
ChatSessionStore threeSessions() {
  final ChatSessionStore store = ChatSessionStore.empty();
  for (int i = 0; i < 3; i++) {
    store.create(now: at(i));
    store.append(user('第 $i 条'), now: at(i));
  }
  return store;
}

void main() {
  group('v2 格式：索引 + 逐条条目', () {
    test('索引只放 version/activeId/ids，**不内联正文**（#10 的关键）', () {
      final ChatSessionStore store = threeSessions();
      final Map<String, Object?> index =
          jsonDecode(encodeChatSessionIndex(store)) as Map<String, Object?>;
      expect(index['version'], kChatStoreVersion);
      expect(index['version'], 2);
      expect(index['activeId'], store.activeId);
      expect((index['ids']! as List<Object?>).length, 3);
      expect(
        index.containsKey('sessions'),
        isFalse,
        reason: '索引里再内联正文就退回 v1 的「一个键装全部」',
      );
    });

    test('完整往返：会话、消息、标题、选中都回来', () {
      final ChatSessionStore store = threeSessions();
      store.rename(store.active!.id, '我的会话', now: at(9));
      final ChatSessionStore back = loadArchive(encodeArchive(store));
      expect(back.sessions.length, 3);
      expect(back.activeId, store.activeId);
      final ChatSession restored = back.sessions.firstWhere(
        (ChatSession s) => s.id == store.activeId,
      );
      expect(restored.title, '我的会话');
      expect(restored.messages.single.text, '第 2 条');
    });

    test('v1 单键全量存档：能读、在内存里迁移、下次写 v2', () {
      final String legacy = jsonEncode(<String, Object?>{
        'version': kChatStoreLegacyVersion,
        'activeId': 's-2',
        'sessions': <Object?>[
          <String, Object?>{
            'id': 's-1',
            'title': '更早那段',
            'createdAt': '2026-11-17T10:00:00.000',
            'updatedAt': '2026-11-17T10:00:00.000',
            'messages': <Object?>[
              <String, Object?>{'role': 'user', 'text': '旧问题'},
            ],
          },
          <String, Object?>{
            'id': 's-2',
            'title': '上次那段',
            'createdAt': '2026-11-18T10:00:00.000',
            'updatedAt': '2026-11-18T10:00:00.000',
            'messages': <Object?>[],
          },
        ],
      });

      final ChatSessionLoadPlan plan = decodeChatSessionIndex(legacy);
      expect(plan.isLegacy, isTrue, reason: 'v1 必须认出来并迁移，而不是「版本不符 → 空」');
      expect(plan.indexCorrupt, isFalse);

      final ChatSessionStore migrated = plan.legacyStore!;
      expect(migrated.sessions.length, 2);
      expect(migrated.activeId, 's-2', reason: '重开页面要回到上次那段');
      expect(migrated.migratedFromLegacy, isTrue);

      // 下一次落盘就是 v2（本函数不依赖迁移标记，永远写 v2）。
      final Map<String, Object?> nextIndex =
          jsonDecode(encodeChatSessionIndex(migrated)) as Map<String, Object?>;
      expect(nextIndex['version'], 2);
      expect((nextIndex['ids']! as List<Object?>), <String>['s-1', 's-2']);
    });

    test('未知版本 → indexCorrupt（不半懂不懂地读未来格式）', () {
      final String raw = jsonEncode(<String, Object?>{
        'version': kChatStoreVersion + 1,
        'ids': <String>['x'],
      });
      final ChatSessionLoadPlan plan = decodeChatSessionIndex(raw);
      expect(plan.indexCorrupt, isTrue);
      expect(plan.ids, isEmpty);
    });

    test('坏 JSON / 非 Map → indexCorrupt；空串 / null → 安静的空 plan', () {
      expect(decodeChatSessionIndex('{不是 JSON').indexCorrupt, isTrue);
      expect(decodeChatSessionIndex('42').indexCorrupt, isTrue);
      // 「没有存档」不是「损坏」：不该给用户一条错误提示。
      for (final String? none in <String?>[null, '']) {
        final ChatSessionLoadPlan plan = decodeChatSessionIndex(none);
        expect(plan.indexCorrupt, isFalse);
        expect(plan.ids, isEmpty);
        expect(plan.isLegacy, isFalse);
      }
    });
  });

  group('逐条隔离（#10）：一条坏记录**不吃掉**其余会话', () {
    test('坏 JSON / 缺键 / 坏条目只丢自己，其余照常加载并计数', () {
      final ChatSessionStore store = threeSessions();
      final List<String> ids = store.sessions
          .map((ChatSession s) => s.id)
          .toList();
      final Map<String, String?> archive = encodeArchive(store);
      // ① 记录内容是坏 JSON；② 另一条键直接缺失（模拟写到一半断电）。
      archive[ids[1]] = '{ 这不是 JSON';
      archive.remove(ids[2]);

      final ChatSessionStore loaded = loadArchive(archive);

      expect(loaded.sessions.length, 1, reason: '坏条目只丢自己');
      expect(loaded.sessions.single.id, ids[0]);
      expect(loaded.droppedLoadEntries, 2);
      expect(loaded.loadWarning, isNotNull);
      expect(loaded.loadWarning, contains('2'));
      expect(loaded.loadWarning, contains('跳过'));
    });

    test('条目是合法 JSON 但不是会话（缺 id）→ 跳过，不抛', () {
      final Map<String, String?> archive = <String, String?>{
        'index': jsonEncode(<String, Object?>{
          'version': kChatStoreVersion,
          'activeId': 'a',
          'ids': <String>['a', 'b'],
        }),
        'a': jsonEncode(<String, Object?>{
          'id': 'a',
          'messages': <Object?>[
            <String, Object?>{'role': 'user', 'text': '在'},
          ],
        }),
        'b': jsonEncode(<String, Object?>{'title': '没有 id'}),
      };
      final ChatSessionStore loaded = loadArchive(archive);
      expect(loaded.sessions.single.id, 'a');
      expect(loaded.droppedLoadEntries, 1);
    });

    test('activeId 指向被跳过的会话 → 当作没选（不凭空选一个）', () {
      final ChatSessionStore store = threeSessions();
      final String active = store.activeId!;
      final Map<String, String?> archive = encodeArchive(store);
      archive[active] = '坏掉了';

      final ChatSessionStore loaded = loadArchive(archive);
      expect(loaded.sessions.length, 2);
      expect(loaded.activeId, isNull);
      expect(loaded.active, isNull);
    });

    test('加载后**仍然**受上限夹持（存档被外部塞爆也要夹住）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      for (int i = 0; i < ChatSessionStore.maxSessions + 5; i++) {
        store.create(now: at(0));
      }
      // create 自己已经裁过了：这里直接手写一份超量索引，模拟外部塞爆。
      final List<String> ids = <String>[
        for (int i = 0; i < ChatSessionStore.maxSessions + 5; i++) 's$i',
      ];
      final Map<String, String?> archive = <String, String?>{
        'index': jsonEncode(<String, Object?>{
          'version': kChatStoreVersion,
          'ids': ids,
        }),
        for (int i = 0; i < ChatSessionStore.maxSessions + 5; i++)
          's$i': jsonEncode(<String, Object?>{
            'id': 's$i',
            'createdAt': at(0).toIso8601String(),
            'updatedAt': at(i % 60).toIso8601String(),
            'messages': <Object?>[],
          }),
      };
      final ChatSessionStore loaded = loadArchive(archive);
      expect(loaded.sessions.length, ChatSessionStore.maxSessions);
      expect(loaded.trimNotice, isNotNull, reason: '加载时裁掉也要提示');
    });
  });

  group('裁剪通知（#8）：静默丢数据不行', () {
    test('会话超过上限：说清丢了多少、上限多少；clearTrimNotice 清掉', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      for (int i = 0; i < ChatSessionStore.maxSessions; i++) {
        store.create(now: at(i));
      }
      expect(store.trimNotice, isNull, reason: '还没丢东西，不该有提示');

      store.create(now: at(59));

      final String? notice = store.trimNotice;
      expect(notice, isNotNull);
      expect(notice, contains('最多 50 个'));
      expect(notice, contains('已自动删除最久未用的 1 个'));

      store.clearTrimNotice();
      expect(store.trimNotice, isNull);
    });

    test('单会话消息超过上限：说清丢弃条数与每个会话上限', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      store.ensureActive(now: at(0));
      for (int i = 0; i < ChatSessionStore.kMaxMessagesPerSession + 10; i++) {
        store.append(user('第 $i 条'), now: at(0));
      }
      final String? notice = store.trimNotice;
      expect(notice, isNotNull);
      expect(notice, contains('最多 500 条'));
      expect(notice, contains('已自动丢弃最早的 10 条'));
    });
  });

  group('落盘失败 / 读取失败通知（#9 / #10）', () {
    test('markSaveFailed / clearSaveFailure 可切回；不改数据', () {
      final ChatSessionStore store = threeSessions();
      expect(store.saveFailed, isFalse);
      store.markSaveFailed();
      expect(store.saveFailed, isTrue);
      expect(store.sessions.length, 3, reason: '标记失败不该动数据');
      store.clearSaveFailure();
      expect(store.saveFailed, isFalse);
    });

    test('markArchiveUnreadable 说明「存档读不出来」；clearLoadWarning 清掉', () {
      final ChatSessionStore store = ChatSessionStore.empty()
        ..markArchiveUnreadable();
      expect(store.loadWarning, isNotNull);
      expect(store.loadWarning, contains('存档读不出来'));
      store.clearLoadWarning();
      expect(store.loadWarning, isNull);
    });

    test('落盘失败文案就是 UI 要显示的那一句（#9）', () {
      expect(kSessionsNotPersistedNote, contains('只保留在本次页面里'));
    });
  });

  group('思考与跟读落盘（#11）', () {
    test('往返保留 reasoning 与 spoken（刷新后折叠区 / 高亮坐标还在）', () {
      final ChatMessage message = ChatMessage(
        role: ChatRole.assistant,
        text: '第一句。第二句。',
        epoch: 7,
        reasoning: '先想一下再回答',
      );
      message.spoken.addAll(<SpokenSpan>[
        SpokenSpan(seq: 1, text: '第一句。'),
        SpokenSpan(seq: 2, text: '第二句。'),
      ]);

      final ChatMessage back = ChatMessage.fromJson(message.toJson())!;
      expect(back.reasoning, '先想一下再回答');
      expect(back.spoken.length, 2);
      expect(back.spoken[0].seq, 1);
      expect(back.spoken[0].text, '第一句。');
      expect(back.spoken[1].seq, 2);
    });

    test('思考超过上限：截断到 2000 rune + 省略号（写读两侧都封顶）', () {
      final String huge = '思' * (kPersistedReasoningMaxChars + 500);
      final ChatMessage message = ChatMessage(
        role: ChatRole.assistant,
        text: '正文',
        reasoning: huge,
      );
      final Object? stored = message.toJson()['reasoning'];
      expect(stored, isA<String>());
      final String persisted = stored! as String;
      expect(persisted.runes.length, kPersistedReasoningMaxChars + 1);
      expect(persisted.endsWith(kPersistedReasoningTruncatedMarker), isTrue);
      expect(persisted.startsWith('思'), isTrue, reason: '保留开头，不能整段丢掉');

      // 读侧同样封顶（存档可能被人手工改大 / 来自旧版本）。
      final ChatMessage back = ChatMessage.fromJson(message.toJson())!;
      expect(back.reasoning.runes.length, kPersistedReasoningMaxChars + 1);
    });

    test('旧条目没有 reasoning / spoken 字段也能加载（向后兼容）', () {
      final ChatMessage old = ChatMessage.fromJson(<String, Object?>{
        'role': 'assistant',
        'text': '旧版本的回复',
      })!;
      expect(old.text, '旧版本的回复');
      expect(old.reasoning, '');
      expect(old.spoken, isEmpty);
    });

    test('spoken 坏条目只丢那一句，不丢整条消息', () {
      final ChatMessage back = ChatMessage.fromJson(<String, Object?>{
        'role': 'assistant',
        'text': '甲。乙。',
        'spoken': <Object?>[
          <String, Object?>{'seq': '不是 int', 'text': '坏'},
          <String, Object?>{'seq': 3, 'text': '乙。'},
          '不是 map',
        ],
      })!;
      expect(back.spoken.length, 1);
      expect(back.spoken.single.seq, 3);
    });

    test('整份存档往返：思考与句子跟着会话进进出出', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession session = store.ensureActive(now: at(0));
      final ChatMessage message = ChatMessage(
        role: ChatRole.assistant,
        text: '你好。',
        reasoning: '打个招呼',
      )..spoken.add(SpokenSpan(seq: 1, text: '你好。'));
      session.messages.add(message);
      session.updatedAt = at(1);

      final ChatSessionStore back = loadArchive(encodeArchive(store));
      final ChatMessage restored = back.messages.single;
      expect(restored.reasoning, '打个招呼');
      expect(restored.spoken.single.text, '你好。');
    });
  });
}
