/// 多会话：模型 + 序列化 + 持久化往返（2026-09-11）。
///
/// # 这一层是什么、不是什么（**别把它们搞混**）
///
/// **是**：聊天记录留在**本机浏览器**——刷新/重开页面还在；可开多个会话、
/// 切换、重命名、删除。
///
/// **不是**：模型**没有记忆**。`POST /api/v1/chat` 只收 `{text}`，没有历史
/// 字段，前端无从把历史喂回去；服务端 `persona.max_history_pairs` 出厂为 0。
/// 真正的「会话记忆」需要协议 + 端点 + 服务端持久化，**已登记为待实现**。
///
/// 这条区分值得一整段注释：**否则「历史还在」很容易被误当成「模型记得」**，
/// 然后有人去 debug 一个不存在的 bug。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/chat_session.dart';
import 'package:live2d_ai_shell/chat/turn_liveness.dart';

ChatMessage user(String t) => ChatMessage(role: ChatRole.user, text: t);
ChatMessage bot(String t) => ChatMessage(role: ChatRole.assistant, text: t);

/// 固定时钟：**不依赖系统时间**（否则「相对时间」与排序断言会随机红）。
DateTime at(int minute) => DateTime(2026, 9, 11, 12, minute);

void main() {
  group('会话模型：新建 / 切换 / 删除', () {
    test('空 store 里没有任何会话（**不在打开页面时就建一个**）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      expect(store.sessions, isEmpty);
      expect(store.active, isNull);
      expect(store.messages, isEmpty);
      // 没有会话时 messages 是 const 空表——不会被误写入。
      expect(store.messages, isA<List<ChatMessage>>());
    });

    test('新建会选中它，且旧会话仍在列表里', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession a = store.create(now: at(0));
      final ChatSession b = store.create(now: at(1));
      expect(store.sessions.length, 2);
      expect(store.active, b, reason: '新建之后应当切到新的那个');
      expect(store.activeId, b.id);
      expect(store.sessions.contains(a), isTrue);
    });

    test('select 切换；切到不存在的 id 返回 false 且不改状态', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession a = store.create(now: at(0));
      store.create(now: at(1));
      expect(store.select(a.id), isTrue);
      expect(store.active, a);
      expect(store.select('不存在'), isFalse);
      expect(store.active, a, reason: '失败的 select 不该把 activeId 弄坏');
    });

    test('删掉当前会话 → 自动选下一个最近用过的；删光 → activeId 为空', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession a = store.create(now: at(0));
      final ChatSession b = store.create(now: at(1));
      final ChatSession c = store.create(now: at(2));

      expect(store.active, c);
      expect(store.delete(c.id), isTrue);
      expect(store.activeId, b.id, reason: '应当落到最近用过的那个（b）');
      store.delete(b.id);
      expect(store.activeId, a.id);
      store.delete(a.id);
      expect(
        store.activeId,
        isNull,
        reason: '删光了就是空的 —— 不该自动新建一个（用户删光就是想要干净）',
      );
      expect(store.sessions, isEmpty);
    });

    test('删掉**非**当前会话不影响选中', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession a = store.create(now: at(0));
      final ChatSession b = store.create(now: at(1));
      expect(store.active, b);
      store.delete(a.id);
      expect(store.active, b);
    });

    test('ensureActive：没有就建一个，有就原样返回', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession first = store.ensureActive(now: at(0));
      final ChatSession again = store.ensureActive(now: at(5));
      expect(identical(first, again), isTrue);
      expect(store.sessions.length, 1);
    });
  });

  group('会话模型：标题', () {
    test('没起过名时用**第一条用户消息**自动起标题', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession s = store.ensureActive(now: at(0));
      expect(s.displayTitle, kUntitledSessionLabel);
      store.append(bot('你好！'), now: at(0));
      store.append(user('帮我看看这个报错'), now: at(1));
      expect(
        s.displayTitle,
        '帮我看看这个报错',
        reason: '取用户消息而不是助手消息：用户打的字是他找会话时的线索',
      );
    });

    test('自动标题超过上限就截断并加省略号（**按字符截，不切坏中文**）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession s = store.ensureActive(now: at(0));
      store.append(user('一' * 40), now: at(1));
      expect(s.displayTitle.runes.length, kAutoTitleMaxChars + 1); // + 省略号
      expect(s.displayTitle.endsWith('…'), isTrue);
    });

    test('起过名就用名字；**改名传空串 = 恢复自动标题**', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession s = store.ensureActive(now: at(0));
      store.append(user('原始内容'), now: at(1));

      store.rename(s.id, '  我的名字  ', now: at(2));
      expect(s.title, '我的名字', reason: '两端空白要去掉');
      expect(s.displayTitle, '我的名字');
      expect(s.isNamed, isTrue);

      store.rename(s.id, '', now: at(3));
      expect(s.isNamed, isFalse);
      expect(s.displayTitle, '原始内容', reason: '清空输入框 = 不要这个名字了');
    });

    test('重命名不存在的会话返回 false', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      expect(store.rename('没有这个', 'x'), isFalse);
    });
  });

  group('会话模型：排序与上限', () {
    test('byRecency 按最后活动倒序', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession a = store.create(now: at(0));
      final ChatSession b = store.create(now: at(1));
      final ChatSession c = store.create(now: at(2));
      expect(store.byRecency.map((ChatSession s) => s.id), <String>[c.id, b.id, a.id]);

      // 给最老的追加一条消息 → 它应当浮到最上面。
      store.select(a.id);
      store.append(user('刚说话'), now: at(9));
      expect(store.byRecency.first.id, a.id);
    });

    test('会话数超过上限时丢**最久没用过**的那个', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      for (int i = 0; i < ChatSessionStore.maxSessions; i++) {
        store.create(now: at(i));
      }
      final ChatSession oldest = store.byRecency.last;
      expect(store.sessions.length, ChatSessionStore.maxSessions);

      // 再建一个触发裁剪。
      store.create(now: at(59));

      expect(store.sessions.length, ChatSessionStore.maxSessions);
      expect(
        store.sessions.contains(oldest),
        isFalse,
        reason: '最久没用过的那个应当被丢掉',
      );
    });

    test('裁剪**绝不删掉当前选中的会话**（存档被塞爆时也要成立）', () {
      // 这条走 `fromJson` 而不是 `create`：`create` 会把 active 切到新会话，
      // 于是「active 是最老的那个」这个条件建不出来。而**从存储读进来**时
      // 完全可能发生——存档里有 70 个会话，activeId 指着最早那个。
      final List<Object?> many = <Object?>[
        for (int i = 0; i < ChatSessionStore.maxSessions + 20; i++)
          <String, Object?>{
            'id': 's$i',
            // i 越大越新；activeId 将指向最老的 s0。
            'createdAt': '2026-09-11T12:00:00.000',
            'updatedAt':
                '2026-09-11T12:${(i % 60).toString().padLeft(2, '0')}:00.000',
            'messages': <Object?>[],
          },
      ];
      final ChatSessionStore store = ChatSessionStore.fromJson(
        <String, Object?>{
          'version': kChatStoreVersion,
          'activeId': 's0',
          'sessions': many,
        },
      );

      expect(store.sessions.length, ChatSessionStore.maxSessions);
      expect(
        store.activeId,
        's0',
        reason: 'activeId 不该在裁剪里被清空',
      );
      expect(
        store.active,
        isNotNull,
        reason: '**当前选中的会话被裁掉了** —— 用户一打开页面就没得看',
      );
    });

    test('单个会话的消息超过上限时丢**最旧**的', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession s = store.ensureActive(now: at(0));
      for (int i = 0; i < ChatSessionStore.kMaxMessagesPerSession + 10; i++) {
        store.append(user('第 $i 条'), now: at(0));
      }
      expect(s.messages.length, ChatSessionStore.kMaxMessagesPerSession);
      expect(
        s.messages.first.text,
        '第 10 条',
        reason: '丢的是最旧的 10 条，留下的是后 500 条',
      );
    });

    test('消息上限与视图层一致（两处不一致会出现「看得见但重开就没了」）', () {
      // `kChatHistoryLimit` 在 `ui/chat_panel.dart`（视图层裁剪）。
      // 这里不 import 它（那会把 Flutter 拖进纯逻辑测试），改为钉住数值：
      // 改动任何一边都会让这条红，逼着人同时看两处。
      expect(ChatSessionStore.kMaxMessagesPerSession, 500);
    });

    test('pruneEmpty 清理空会话，但**保留当前那个**（哪怕它是空的）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession staleEmpty = store.create(now: at(0));
      final ChatSession withMsg = store.create(now: at(1));
      // `append` 落到**当时**的活动会话，也就是 withMsg。
      store.append(user('有内容'), now: at(2));
      final ChatSession currentEmpty = store.create(now: at(3));

      expect(withMsg.messages.length, 1);
      expect(currentEmpty.messages, isEmpty);
      expect(store.activeId, currentEmpty.id);

      final int removed = store.pruneEmpty();

      expect(removed, 1, reason: '只该清掉 staleEmpty 那一个');
      expect(store.sessions.contains(staleEmpty), isFalse);
      expect(store.sessions.contains(withMsg), isTrue, reason: '非空的不该动');
      expect(
        store.sessions.contains(currentEmpty),
        isTrue,
        reason: '当前选中的空会话要留着 —— 用户正看着它',
      );
    });
  });

  group('序列化：往返一致', () {
    test('完整往返（会话、消息、标题、时间、选中）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatSession a = store.create(now: at(0));
      store.append(user('问题'), now: at(1));
      store.append(bot('回答'), now: at(2));
      store.rename(a.id, '我的会话', now: at(3));
      store.create(now: at(4));

      final ChatSessionStore back = ChatSessionStore.fromJson(store.toJson());

      expect(back.sessions.length, store.sessions.length);
      expect(back.activeId, store.activeId);
      final ChatSession a2 = back.sessions.firstWhere(
        (ChatSession s) => s.id == a.id,
      );
      expect(a2.title, '我的会话');
      expect(a2.displayTitle, '我的会话');
      expect(a2.messages.length, 2);
      expect(a2.messages[0].role, ChatRole.user);
      expect(a2.messages[0].text, '问题');
      expect(a2.messages[1].role, ChatRole.assistant);
      expect(a2.messages[1].text, '回答');
      expect(a2.createdAt, at(0));
      expect(a2.updatedAt, at(3));
    });

    test('`streaming` **不落盘**（否则重开页面会得到永远转圈的气泡）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      store.append(
        ChatMessage(role: ChatRole.assistant, text: '半句话', streaming: true),
        now: at(0),
      );
      final ChatSessionStore back = ChatSessionStore.fromJson(store.toJson());
      expect(back.messages.single.streaming, isFalse);
      expect(back.messages.single.text, '半句话', reason: '文字本身要保住');
    });

    test('失败态要落盘（重开页面仍能看到「生成失败」）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      store.append(
        ChatMessage(role: ChatRole.assistant, text: '（生成失败）', failed: true),
        now: at(0),
      );
      final ChatSessionStore back = ChatSessionStore.fromJson(store.toJson());
      expect(back.messages.single.failed, isTrue);
    });

    test('未收尾兜底标记要落盘（重开页面仍要标「未收尾」）', () {
      // rc.3 N0（2026-09-13）：兜底正文是**真实内容**，所以文字必须留下；
      // 但它没有语音收尾，这个限定也必须一起留下——否则刷新后那段文字看起来
      // 与正常回复一模一样，等于把「未收尾」这个事实抹掉了。
      final ChatSessionStore store = ChatSessionStore.empty();
      store.append(
        ChatMessage(
          role: ChatRole.assistant,
          text: '第一句。第二',
          unfinished: true,
        ),
        now: at(0),
      );
      final ChatSessionStore back = ChatSessionStore.fromJson(store.toJson());
      expect(back.messages.single.text, '第一句。第二');
      expect(back.messages.single.unfinished, isTrue);
    });
  });

  group('序列化：坏数据**永不抛**（丢一条，不丢整段历史）', () {
    test('null / 空 / 非 Map → 空 store', () {
      for (final Object? bad in <Object?>[null, '', 42, <Object?>[], true]) {
        final ChatSessionStore store = ChatSessionStore.fromJson(bad);
        expect(store.sessions, isEmpty);
        expect(store.activeId, isNull);
      }
    });

    test('版本不符 → 整体丢弃（半懂不懂地读未来格式更危险）', () {
      final Object json = <String, Object?>{
        'version': kChatStoreVersion + 1,
        'sessions': <Object?>[
          <String, Object?>{'id': 'x', 'messages': <Object?>[]},
        ],
      };
      expect(ChatSessionStore.fromJson(json).sessions, isEmpty);
    });

    test('坏会话条目被丢弃，**好的仍然保留**', () {
      final Object json = <String, Object?>{
        'version': kChatStoreVersion,
        'activeId': 'good',
        'sessions': <Object?>[
          <String, Object?>{'noId': true}, // 缺 id → 丢
          'not a map', // 类型不对 → 丢
          <String, Object?>{'id': ''}, // 空 id → 丢
          <String, Object?>{
            'id': 'good',
            'title': '好的',
            'createdAt': '2026-09-11T12:00:00.000',
            'updatedAt': '2026-09-11T12:01:00.000',
            'messages': <Object?>[
              <String, Object?>{'role': 'user', 'text': '在'},
              <String, Object?>{'role': '不存在的角色', 'text': 'x'}, // 丢
              <String, Object?>{'role': 'assistant'}, // 缺 text → 丢
              'nope', // 类型不对 → 丢
            ],
          },
        ],
      };
      final ChatSessionStore store = ChatSessionStore.fromJson(json);
      expect(store.sessions.length, 1);
      expect(store.sessions.single.id, 'good');
      expect(store.sessions.single.messages.length, 1);
      expect(store.activeId, 'good');
    });

    test('activeId 指向不存在的会话 → 当作没选（而不是崩）', () {
      final Object json = <String, Object?>{
        'version': kChatStoreVersion,
        'activeId': '幽灵',
        'sessions': <Object?>[
          <String, Object?>{'id': 'real', 'messages': <Object?>[]},
        ],
      };
      final ChatSessionStore store = ChatSessionStore.fromJson(json);
      expect(store.activeId, isNull);
      expect(store.active, isNull);
    });

    test('反序列化之后**仍然**受上限约束（存档被外部塞爆也要夹住）', () {
      final List<Object?> many = <Object?>[
        for (int i = 0; i < ChatSessionStore.maxSessions + 20; i++)
          <String, Object?>{
            'id': 's$i',
            'createdAt': '2026-09-11T12:00:00.000',
            'updatedAt': '2026-09-11T12:${(i % 60).toString().padLeft(2, '0')}:00.000',
            'messages': <Object?>[
              for (int j = 0; j < 600; j++)
                <String, Object?>{'role': 'user', 'text': 'x$j'},
            ],
          },
      ];
      final ChatSessionStore store = ChatSessionStore.fromJson(
        <String, Object?>{
          'version': kChatStoreVersion,
          'sessions': many,
        },
      );
      expect(store.sessions.length, ChatSessionStore.maxSessions);
      for (final ChatSession s in store.sessions) {
        expect(s.messages.length, ChatSessionStore.kMaxMessagesPerSession);
      }
    });
  });

  group('时间格式化（纯函数，显式传 now）', () {
    final DateTime now = DateTime(2026, 9, 11, 12, 0);

    test('分档正确', () {
      expect(relativeTime(now, now: now), '刚刚');
      expect(relativeTime(now.subtract(const Duration(seconds: 30)), now: now), '刚刚');
      expect(relativeTime(now.subtract(const Duration(minutes: 5)), now: now), '5 分钟前');
      expect(relativeTime(now.subtract(const Duration(hours: 3)), now: now), '3 小时前');
      expect(relativeTime(now.subtract(const Duration(days: 1)), now: now), '昨天');
      expect(relativeTime(now.subtract(const Duration(days: 3)), now: now), '3 天前');
      expect(relativeTime(DateTime(2026, 9, 1, 8), now: now), '2026-09-01');
    });

    test('未来时间不显示负数（时钟回拨也要说人话）', () {
      expect(relativeTime(now.add(const Duration(hours: 2)), now: now), '刚刚');
    });
  });

  group('会话 id：同一毫秒内建两个也不撞', () {
    test('连建 50 个（同一个 now）id 两两不同', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final Set<String> ids = <String>{};
      for (int i = 0; i < 50; i++) {
        ids.add(store.create(now: at(0)).id);
      }
      expect(ids.length, 50, reason: '纯时间戳在快速连点「新建」时会撞 —— 加了序号');
    });
  });

  group('replace：就地替换（空气泡 → 系统提示，2026-09-11）', () {
    test('替换后**位置不变**（提示必须贴在它解释的那一轮上）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      store.append(user('第一句'));
      final ChatMessage bubble = ChatMessage(
        role: ChatRole.assistant,
        streaming: true,
      );
      store.append(bubble);
      store.append(user('第二句'));

      expect(
        store.replace(
          bubble,
          ChatMessage(role: ChatRole.system, text: kWordlessTurnNotice),
        ),
        isTrue,
      );
      expect(store.messages.length, 3);
      expect(store.messages[1].role, ChatRole.system);
      expect(store.messages[1].text, kWordlessTurnNotice);
      expect(
        store.messages[2].text,
        '第二句',
        reason: '替换不许把后面的消息挤走或打乱顺序',
      );
    });

    test('目标不在活动会话里 → false（不抛、不改动任何东西）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      store.append(user('甲'));
      final ChatMessage stray = ChatMessage(
        role: ChatRole.assistant,
        streaming: true,
      );

      expect(
        store.replace(
          stray,
          ChatMessage(role: ChatRole.system, text: kWordlessTurnNotice),
        ),
        isFalse,
      );
      expect(store.messages.length, 1);
    });

    test('推进 updatedAt（会话要因此在列表里浮上来）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      final ChatMessage bubble = ChatMessage(
        role: ChatRole.assistant,
        streaming: true,
      );
      store.append(bubble, now: at(0));
      store.replace(
        bubble,
        ChatMessage(role: ChatRole.system, text: kWordlessTurnNotice),
        now: at(5),
      );
      expect(store.active!.updatedAt, at(5));
    });

    test('没有活动会话 → false（不自动建一个）', () {
      final ChatSessionStore store = ChatSessionStore.empty();
      expect(
        store.replace(
          bot('x'),
          ChatMessage(role: ChatRole.system, text: kWordlessTurnNotice),
        ),
        isFalse,
      );
      expect(store.sessions, isEmpty);
    });
  });
}
