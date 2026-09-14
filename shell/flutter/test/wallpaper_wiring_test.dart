/// 壁纸接线（Wave 2 B 轨）回归：**Mod 决策 → `DisplayPrefs`** 的纯逻辑、
/// 列表序列化往返与三条预算、以及「停用不轮询」判据。
///
/// 为什么这些必须是纯测试：轮询 / 落盘 / 下发都在 `Timer` 与 `setState` 里，
/// 而**判据**一旦埋进去就没法回归。真实事故面有两条（都在计划里写过）：
/// 停用后还在轮询、列表超预算把**整份偏好**写坏（rc.5 的 localStorage 教训）。
library;

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/wallpaper_api.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';

/// 造一个合法的小 dataURL（内容无所谓，长度有意义）。
String _img(String tag, [int pad = 8]) => 'data:image/png;base64,$tag${'A' * pad}';

void main() {
  group('applyWallpaperPatch：三种 patch', () {
    test('null / 非对象 / 空对象 → 明确跳过（不是假装成功）', () {
      const DisplayPrefs prefs = DisplayPrefs(
        stagePlaylist: <String>['data:image/png;base64,x'],
      );
      for (final Object? patch in <Object?>[null, 'nope', 7, <String, Object?>{}]) {
        final WallpaperPatchResult r = applyWallpaperPatch(prefs, patch);
        expect(r.applied, isFalse);
        expect(r.prefs, same(prefs), reason: '未应用时必须原样返回入参');
        expect(r.reason, 'empty_patch');
      }
    });

    test('sync_shell_stage_bg → copyWith(syncShellStageBg: true)；已是 true 则幂等', () {
      const DisplayPrefs off = DisplayPrefs(syncShellStageBg: false);
      final WallpaperPatchResult on = applyWallpaperPatch(
        off,
        <String, Object?>{'sync_shell_stage_bg': true},
      );
      expect(on.applied, isTrue);
      expect(on.prefs.syncShellStageBg, isTrue);
      // 同步开时壳与舞台共用一张图：effectiveShellImage 立刻等于 stageImage。
      expect(
        applyWallpaperPatch(
          off.copyWith(stageImage: _img('stage')),
          <String, Object?>{'sync_shell_stage_bg': true},
        ).prefs.effectiveShellImage,
        _img('stage'),
      );

      final WallpaperPatchResult again = applyWallpaperPatch(
        on.prefs,
        <String, Object?>{'sync_shell_stage_bg': true},
      );
      expect(again.applied, isFalse, reason: '已经是 true 不该再改一次');
      expect(again.reason, 'already_synced');
    });

    test('stage_index → stageImage = playlist[k % len]（取模回绕）', () {
      final DisplayPrefs prefs = DisplayPrefs(
        stagePlaylist: <String>[_img('a'), _img('b'), _img('c')],
      );
      expect(
        applyWallpaperPatch(prefs, <String, Object?>{'stage_index': 0}).prefs.stageImage,
        _img('a'),
      );
      expect(
        applyWallpaperPatch(prefs, <String, Object?>{'stage_index': 2}).prefs.stageImage,
        _img('c'),
      );
      // 3 % 3 == 0，4 % 3 == 1：Mod 侧游标可能连续增长，落点必须回绕。
      expect(
        applyWallpaperPatch(prefs, <String, Object?>{'stage_index': 3}).prefs.stageImage,
        _img('a'),
      );
      expect(
        applyWallpaperPatch(prefs, <String, Object?>{'stage_index': 4}).prefs.stageImage,
        _img('b'),
      );
      expect(
        applyWallpaperPatch(prefs, <String, Object?>{'stage_index': 4}).applied,
        isTrue,
      );
    });

    test('列表为空 → 跳过（playlist_empty），prefs 原样', () {
      const DisplayPrefs empty = DisplayPrefs();
      final WallpaperPatchResult r = applyWallpaperPatch(
        empty,
        <String, Object?>{'stage_index': 0},
      );
      expect(r.applied, isFalse);
      expect(r.reason, 'playlist_empty');
      expect(r.prefs.stageImage, isNull);
    });

    test('非法 / 未知 patch 不猜', () {
      final DisplayPrefs prefs = DisplayPrefs(
        stagePlaylist: <String>[_img('a')],
      );
      final WallpaperPatchResult negative = applyWallpaperPatch(
        prefs,
        <String, Object?>{'stage_index': -1},
      );
      expect(negative.applied, isFalse);
      expect(negative.reason, 'invalid_index');
      final WallpaperPatchResult unknown = applyWallpaperPatch(
        prefs,
        <String, Object?>{'what_is_this': 1},
      );
      expect(unknown.applied, isFalse);
      expect(unknown.reason, 'unknown_patch');
      // 同步 / 索引都缺 → 也不动作。
      expect(
        applyWallpaperPatch(prefs, <String, Object?>{'sync_shell_stage_bg': false}).reason,
        'unknown_patch',
      );
    });
  });

  group('stagePlaylist 序列化往返（同一条 localStorage 记录）', () {
    test('toJson → jsonEncode → jsonDecode → fromJson 不丢项、不串值', () {
      final DisplayPrefs prefs = const DisplayPrefs()
          .copyWith(stageImage: _img('stage'))
          .copyWith(stagePlaylist: <String>[_img('a'), _img('b'), _img('c')]);
      final String raw = jsonEncode(prefs.toJson());
      final DisplayPrefs back = DisplayPrefs.fromJson(
        (jsonDecode(raw) as Map<Object?, Object?>).cast<String, Object?>(),
      );
      expect(back.stagePlaylist, prefs.stagePlaylist);
      expect(back, prefs, reason: '整份偏好往返相等（含列表）');
      expect(back.stageImage, _img('stage'));
    });

    test('旧存档没有该字段 → 空列表（不是 null / 崩）', () {
      final Map<String, Object?> old = const DisplayPrefs().toJson()
        ..remove('stagePlaylist');
      expect(DisplayPrefs.fromJson(old).stagePlaylist, isEmpty);
      expect(DisplayPrefs.fromJson(null).stagePlaylist, isEmpty);
      expect(DisplayPrefs.fromJson(<String, Object?>{}).stagePlaylist, isEmpty);
    });

    test('坏值：非 List / 混入非字符串 / 空串 → 逐项丢弃，其余保留', () {
      final Map<String, Object?> json = const DisplayPrefs().toJson()
        ..['stagePlaylist'] = <Object?>[
          _img('a'),
          42,
          '',
          null,
          _img('b'),
        ];
      expect(
        DisplayPrefs.fromJson(json).stagePlaylist,
        <String>[_img('a'), _img('b')],
      );
      // 不是 List 一律空列表。
      expect(
        DisplayPrefs.fromJson(<String, Object?>{'stagePlaylist': 'x'}).stagePlaylist,
        isEmpty,
      );
    });

    test('超预算读取：单项超限丢弃；总长超限保留前面的、丢掉放不下的', () {
      // 单项超 kStageImageMaxChars → 丢该项，后面仍读。
      final String huge = 'x' * (kStageImageMaxChars + 1);
      expect(
        DisplayPrefs.readStagePlaylist(<Object?>[huge, _img('ok')]),
        <String>[_img('ok')],
      );
      // 总长超 kStagePlaylistMaxChars → 保留前面的，后面的丢掉。
      // （`huge2` 本身合法，但两份加起来超总预算。）
      final String half = 'y' * (kStagePlaylistMaxChars ~/ 2 + 1);
      expect(
        DisplayPrefs.readStagePlaylist(<Object?>[half, half]),
        <String>[half],
      );
      // 项数上限：超出的丢掉。
      final List<Object?> many = <Object?>[
        for (int i = 0; i < kStagePlaylistMaxItems + 5; i++) _img('n$i'),
      ];
      expect(
        DisplayPrefs.readStagePlaylist(many).length,
        kStagePlaylistMaxItems,
      );
    });

    test('预算常量之间不矛盾（总长 ≥ 单项，项数上限有界）', () {
      expect(kStagePlaylistMaxChars, kStageImageMaxChars);
      expect(kStagePlaylistMaxItems, 16);
      expect(kStagePlaylistMaxItems, greaterThan(0));
    });
  });

  group('appendToStagePlaylist：三条预算逐条把关', () {
    test('正常追加：列表变长、added=true', () {
      final StagePlaylistAppendResult r = appendToStagePlaylist(
        <String>[_img('a')],
        _img('b'),
      );
      expect(r.added, isTrue);
      expect(r.reason, 'added');
      expect(r.playlist, <String>[_img('a'), _img('b')]);
    });

    test('没有当前图 → empty，列表不动', () {
      final StagePlaylistAppendResult r = appendToStagePlaylist(<String>[], null);
      expect(r.added, isFalse);
      expect(r.reason, 'empty');
      expect(r.playlist, isEmpty);
    });

    test('单项超限 → item_too_large，列表不动', () {
      final StagePlaylistAppendResult r = appendToStagePlaylist(
        <String>[],
        'x' * (kStageImageMaxChars + 1),
      );
      expect(r.added, isFalse);
      expect(r.reason, 'item_too_large');
      expect(r.playlist, isEmpty);
    });

    test('项数已达上限 → limit_reached（append 不生效，不是悄悄膨胀）', () {
      final List<String> full = <String>[
        for (int i = 0; i < kStagePlaylistMaxItems; i++) _img('n$i'),
      ];
      final StagePlaylistAppendResult r = appendToStagePlaylist(full, _img('x'));
      expect(r.added, isFalse);
      expect(r.reason, 'limit_reached');
      expect(r.playlist.length, kStagePlaylistMaxItems, reason: '列表长度不变');
      expect(identical(r.playlist, full), isTrue, reason: '失败时原样返回入参');
    });

    test('总长超预算 → budget_exceeded（列表不动）', () {
      final String half = 'z' * (kStagePlaylistMaxChars ~/ 2 + 1);
      final List<String> current = <String>[half];
      final StagePlaylistAppendResult r = appendToStagePlaylist(current, half);
      expect(r.added, isFalse);
      expect(r.reason, 'budget_exceeded');
      expect(r.playlist, current);
      // 恰好填满是可以的（边界不误伤）。
      final int room = kStagePlaylistMaxChars - half.length;
      final StagePlaylistAppendResult ok = appendToStagePlaylist(
        current,
        'w' * room,
      );
      expect(ok.added, isTrue);
      expect(ok.playlist.length, 2);
    });
  });

  group('轮询门禁（停用 / 关模式即停）', () {
    test('未注册 / 未启用 → 不轮询', () {
      expect(
        shouldPollWallpaperState(registered: false, enabled: true, mode: 'interval'),
        isFalse,
      );
      expect(
        shouldPollWallpaperState(registered: true, enabled: false, mode: 'interval'),
        isFalse,
      );
      expect(
        shouldPollWallpaperState(registered: false, enabled: false, mode: 'interval'),
        isFalse,
      );
    });

    test('off / 未知 / 缺失 mode → 不轮询（fail-safe 到不接管）', () {
      for (final Object? mode in <Object?>['off', 'carousel', 7, null]) {
        expect(
          shouldPollWallpaperState(registered: true, enabled: true, mode: mode),
          isFalse,
          reason: 'mode=$mode 不该轮询',
        );
      }
    });

    test('启用 + follow_stage/interval → 轮询', () {
      for (final String mode in <String>['follow_stage', 'interval']) {
        expect(
          shouldPollWallpaperState(registered: true, enabled: true, mode: mode),
          isTrue,
        );
      }
      expect(wallpaperModeIsActive('off'), isFalse);
      expect(wallpaperModeIsActive('interval'), isTrue);
    });

    test('轮询间隔 ≥ 5 s（契约下限）', () {
      expect(kWallpaperPollInterval.inSeconds, greaterThanOrEqualTo(5));
    });
  });

  group('列表长度写回判据', () {
    test('长度不同才写；缺字段时只有非 0 才写', () {
      expect(needsPlaylistLenWriteBack(localLen: 3, remoteLen: 3), isFalse);
      expect(needsPlaylistLenWriteBack(localLen: 3, remoteLen: 0), isTrue);
      expect(needsPlaylistLenWriteBack(localLen: 0, remoteLen: 3), isTrue);
      expect(needsPlaylistLenWriteBack(localLen: 0, remoteLen: null), isFalse);
      expect(needsPlaylistLenWriteBack(localLen: 2, remoteLen: null), isTrue);
      // 服务端回的是整数形态浮点也能比。
      expect(needsPlaylistLenWriteBack(localLen: 2, remoteLen: 2.0), isFalse);
    });

    test('写回是「整份替换 + 只覆盖 playlist_len」——不得抹掉 mode/interval', () {
      final Map<String, Object?> merged = configWithPlaylistLen(
        <String, Object?>{'mode': 'interval', 'interval_secs': 30, 'playlist_len': 1},
        4,
      );
      expect(merged, <String, Object?>{
        'mode': 'interval',
        'interval_secs': 30,
        'playlist_len': 4,
      });
    });
  });

  group('WallpaperApi.state()：503/404 是「暂时读不到」而不是故障', () {
    test('200 → 原样返回 state 对象', () async {
      final WallpaperApi api = WallpaperApi(
        base: 'http://127.0.0.1:1',
        client: MockClient((http.Request request) async {
          expect(request.url.path, '/api/v1/mods/wallpaper/state');
          return http.Response(
            jsonEncode(<String, Object?>{
              'id': 'wallpaper',
              'enabled': true,
              'state': <String, Object?>{
                'mode': 'interval',
                'playlist_len': 3,
                'prefs_patch': <String, Object?>{'stage_index': 1},
              },
            }),
            200,
          );
        }),
      );
      final Map<String, Object?>? state = await api.state();
      expect(state, isNotNull);
      expect(state!['playlist_len'], 3);
      expect(
        applyWallpaperPatch(
          const DisplayPrefs(stagePlaylist: <String>['a', 'b', 'c']),
          state['prefs_patch'],
        ).prefs.stageImage,
        'b',
      );
      api.dispose();
    });

    test('503（未启用 / worker 忙）与 404（没注册）→ null，不抛', () async {
      for (final int code in <int>[503, 404]) {
        final WallpaperApi api = WallpaperApi(
          base: 'http://127.0.0.1:1',
          client: MockClient(
            (http.Request _) async => http.Response('{}', code),
          ),
        );
        expect(await api.state(), isNull, reason: 'HTTP $code 不该抛');
        api.dispose();
      }
    });

    test('其它非 200 → ApiException（带码）', () async {
      final WallpaperApi api = WallpaperApi(
        base: 'http://127.0.0.1:1',
        client: MockClient((http.Request _) async => http.Response('boom', 500)),
      );
      await expectLater(
        api.state(),
        throwsA(
          isA<ApiException>().having((ApiException e) => e.code, 'code', 'http_500'),
        ),
      );
      api.dispose();
    });
  });
}
