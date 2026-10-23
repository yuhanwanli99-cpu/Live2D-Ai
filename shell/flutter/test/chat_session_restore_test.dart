import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_session.dart';

import 'support/dart_library.dart';

/// 启动恢复出上次对话后，ChatController 必须把活动会话同步给服务端一次
/// （rc.3，2026-10-23）。
///
/// # 为什么分两层（行为 + 源码扫描）
///
/// chat_controller.dart 经 ws_client.dart / audio_player.dart 传递依赖
/// package:web，**VM 的 flutter test 里加载不了**（与 turn_liveness_test.dart /
/// action_scales_wiring_test.dart 同一条既有结论），所以没法在这里直接构造
/// ChatController、拿注入的 http.Client 去数 POST /api/v1/chat/session。
/// 能钉住的拆成两层：
///
/// 1. **行为**（VM 可跑）：ChatSessionStore.fromJson 把存档里的 activeId
///    恢复出来——这是「恢复出上次对话」这个前提本身；
/// 2. **接线**（源码扫描）：构造器在监听注册之后、且只在 activeId 非空时调用
///    **一次** _syncActiveSession()；newSession / selectSession 既有的调用保留
///    （切会话仍要同步）。
///
/// 删掉构造器里那一行 → 第 2 组立刻变红（不是靠肉眼核对）。
void main() {
  group('前提：恢复出上次对话（ChatSessionStore.fromJson）', () {
    test('恢复 activeId；没有 activeId 或指向不存在的会话 → null', () {
      final Map<String, Object?> raw = <String, Object?>{
        'version': kChatStoreVersion,
        'activeId': 's-2',
        'sessions': <Object?>[
          _sessionJson('s-1', '更早那段', '2026-10-20T10:00:00.000'),
          _sessionJson('s-2', '上次那段', '2026-10-21T10:00:00.000'),
        ],
      };

      final ChatSessionStore restored = ChatSessionStore.fromJson(raw);
      expect(restored.activeId, 's-2', reason: '重开页面应回到上次那段对话');
      expect(restored.active?.id, 's-2');

      final ChatSessionStore noActive = ChatSessionStore.fromJson(
        <String, Object?>{...raw, 'activeId': null},
      );
      expect(noActive.activeId, isNull, reason: '存档没选会话就别凭空选一个');

      final ChatSessionStore broken = ChatSessionStore.fromJson(
        <String, Object?>{...raw, 'activeId': 'gone'},
      );
      expect(
        broken.activeId,
        isNull,
        reason: '指向不存在的会话等于没选（同步一个不存在的 id 只会写坏服务端状态）',
      );
    });
  });

  group('接线（源码扫描）：构造完成处只补一次同步', () {
    late String src;
    late String syncPart;
    setUpAll(() {
      // 库 + 它的 part 一起读：chat_controller 声明了 part，只读库文件会漏掉
      // 方法本体（dart_library_guard_test.dart 明确禁止直读 part 库）。
      src = readLibrarySource('lib/chat/chat_controller.dart');
      syncPart = readLibrarySource('lib/chat/session_sync.dart');
    });

    test('监听注册之后、activeId 非空时调用一次 _syncActiveSession()', () {
      final int ctorAt = src.indexOf('ChatController({');
      expect(ctorAt, greaterThanOrEqualTo(0), reason: '找不到 ChatController 构造器');
      final int fieldsAt = src.indexOf('final ApiClient api;', ctorAt);
      expect(fieldsAt, greaterThan(ctorAt), reason: '构造器后面应有字段声明作为边界');
      final String ctor = src.substring(ctorAt, fieldsAt);

      final int listenerAt = ctor.indexOf(
        '_highlight.addListener(notifyListeners);',
      );
      expect(
        listenerAt,
        greaterThanOrEqualTo(0),
        reason: '监听注册那一行是「构造完成」的锚点',
      );
      final int syncAt = ctor.indexOf('_syncActiveSession();');
      expect(
        syncAt,
        greaterThan(listenerAt),
        reason: '恢复出的活动会话必须在监听注册之后同步给服务端一次',
      );
      final String guard = ctor.substring(listenerAt, syncAt);
      expect(
        guard.contains('sessions.activeId'),
        isTrue,
        reason: '调用必须有 activeId 判据（没有活动会话时不发明一个 id）',
      );
      expect(
        _count(ctor, '_syncActiveSession();'),
        1,
        reason: '构造器里只能补一次（恢复只发生一次，重复调用就是重复 POST）',
      );
    });

    test('newSession / selectSession 的既有调用保留（切会话仍同步）', () {
      expect(
        _count(src, '_syncActiveSession();'),
        3,
        reason: '构造器 1 处 + newSession / selectSession 各 1 处；少一处就是切会话没同步',
      );

      final int newAt = src.indexOf('void newSession()');
      final int selectAt = src.indexOf('void selectSession(String id)');
      expect(newAt, greaterThanOrEqualTo(0), reason: '找不到 newSession');
      expect(selectAt, greaterThan(newAt), reason: 'newSession 之后应有 selectSession');
      expect(
        src.substring(newAt, selectAt).contains('_syncActiveSession();'),
        isTrue,
        reason: 'newSession 必须保留既有同步调用',
      );

      // 方法本体住在同库 part 文件（chat_controller 到 800 行棘轮，rc.3 收口）。
      final int renameAt = src.indexOf('/// 重命名', selectAt);
      expect(
        renameAt,
        greaterThan(selectAt),
        reason: 'selectSession 之后应有下一个方法作为边界',
      );
      expect(
        src.substring(selectAt, renameAt).contains('_syncActiveSession();'),
        isTrue,
        reason: 'selectSession 必须保留既有同步调用',
      );
      expect(
        syncPart.contains("part of 'chat_controller.dart';"),
        isTrue,
        reason: '方法本体在 part 文件里，必须挂回同一个库',
      );
      expect(
        syncPart.contains('extension _ChatSessionSync on ChatController'),
        isTrue,
        reason: '扩展必须挂在 ChatController 上（调用点才能解析）',
      );
      expect(
        syncPart.contains('void _syncActiveSession()'),
        isTrue,
        reason: '搬走的是方法本体，不是把它删掉',
      );
    });
  });
}

int _count(String src, String needle) => src.split(needle).length - 1;

Map<String, Object?> _sessionJson(String id, String title, String at) =>
    <String, Object?>{
      'id': id,
      'title': title,
      'createdAt': at,
      'updatedAt': at,
      'messages': <Object?>[],
    };
