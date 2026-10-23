/// 导演「表演倾向 / 动作频率」草稿的纯逻辑回归（0.4.4-rc.2，2026-10-23）。
///
/// 覆盖：
/// ① 默认两档都是 normal（缺键 / 坏值也回落 normal）；
/// ② 改选只进草稿（dirty），未知档位忽略、同值不发通知；
/// ③ `adoptSaved` 干净时跟随已保存值、**脏草稿不被覆盖**（只更新基线）；
/// ④ `markSaved` 把当前草稿认成已保存；`discard` 回到已保存；
/// ⑤ `payload` **两键一起发**（服务端旁路一次写、一次重载）；
/// ⑥ `GET /api/v1/mods/director/config` 的解析与 404 抛错（宿主据此回缺省）。
///
/// 为什么这一层单独测：它决定「界面显示哪一档 / 底部按钮出不出现 / 保存发什么」，
/// 而宿主接线（part of main.dart）在 VM 里加载不了；把状态机钉在这里，
/// 接线只剩「谁调谁」。
library;

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/director_motion_draft.dart';

http.Response _json(String body, int status) => http.Response.bytes(
  utf8.encode(body),
  status,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

void main() {
  group('DirectorMotionDraft', () {
    test('默认两档都是 normal、不脏、payload 仍发两键', () {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      expect(draft.bias, kDirectorMotionDefaultBias);
      expect(draft.rate, kDirectorMotionDefaultRate);
      expect(draft.bias, 'normal');
      expect(draft.rate, 'normal');
      expect(draft.dirty, isFalse);
      // 干净草稿的 payload 仍是**两键**：保存永远整份写，不按脏键挑。
      expect(draft.payload(), <String, Object?>{
        'motion_bias': 'normal',
        'motion_rate': 'normal',
      });
    });

    test('改选只进草稿：dirty + 通知 + payload 一次带两键', () {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      int notifications = 0;
      draft.addListener(() => notifications++);

      draft.selectBias('eager');
      expect(draft.bias, 'eager');
      expect(draft.rate, 'normal');
      expect(draft.dirty, isTrue);
      expect(notifications, 1);

      draft.selectRate('sparse');
      expect(draft.rate, 'sparse');
      expect(notifications, 2);
      expect(draft.payload(), <String, Object?>{
        'motion_bias': 'eager',
        'motion_rate': 'sparse',
      });
      // 只是草稿：已保存基线没动。
      expect(draft.savedBias, 'normal');
      expect(draft.savedRate, 'normal');
    });

    test('未知档位 / 同值不发通知（协议只认三档）', () {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      int notifications = 0;
      draft.addListener(() => notifications++);

      draft.selectBias('wild');
      draft.selectRate('');
      draft.selectBias('normal');
      draft.selectRate('normal');
      expect(draft.dirty, isFalse);
      expect(notifications, 0);
    });

    test('adoptSaved 干净时跟随已保存值', () {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      draft.adoptSaved(<String, Object?>{
        'motion_bias': 'eager',
        'motion_rate': 'dense',
      });
      expect(draft.bias, 'eager');
      expect(draft.rate, 'dense');
      expect(draft.savedBias, 'eager');
      expect(draft.savedRate, 'dense');
      expect(draft.dirty, isFalse);
    });

    test('adoptSaved 不覆盖脏草稿（只更新已保存基线）', () {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      draft.selectBias('eager');
      expect(draft.dirty, isTrue);

      draft.adoptSaved(<String, Object?>{
        'motion_bias': 'restrained',
        'motion_rate': 'sparse',
      });
      // 用户刚选的档位不被后台重新载入抹掉。
      expect(draft.bias, 'eager');
      expect(draft.rate, 'normal');
      // 基线更新了：草稿与基线仍不等 → 仍是脏的（底部按钮不会消失）。
      expect(draft.savedBias, 'restrained');
      expect(draft.savedRate, 'sparse');
      expect(draft.dirty, isTrue);
    });

    test('adoptSaved 让已保存值追上草稿时，脏自动消失', () {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      draft.selectBias('eager');
      draft.selectRate('dense');
      draft.adoptSaved(const <String, Object?>{});
      expect(draft.dirty, isTrue, reason: '空 config = normal，追上不了 eager/dense');

      draft.adoptSaved(<String, Object?>{
        'motion_bias': 'eager',
        'motion_rate': 'dense',
      });
      expect(draft.bias, 'eager');
      expect(draft.rate, 'dense');
      expect(draft.dirty, isFalse, reason: '两份相等 = 没有未保存改动');
    });

    test('adoptSaved：缺键 / 坏值回落 normal', () {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      draft.adoptSaved(<String, Object?>{
        'motion_bias': 'bogus',
        'motion_rate': 42,
      });
      expect(draft.bias, 'normal');
      expect(draft.rate, 'normal');
      expect(draft.savedBias, 'normal');
      expect(draft.savedRate, 'normal');
    });

    test('markSaved：当前草稿成为已保存值，dirty 清除', () {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      draft.selectBias('eager');
      draft.selectRate('sparse');
      int notifications = 0;
      draft.addListener(() => notifications++);

      draft.markSaved();
      expect(draft.dirty, isFalse);
      expect(draft.savedBias, 'eager');
      expect(draft.savedRate, 'sparse');
      expect(notifications, 1);
      // 干净时再调一次不发通知。
      draft.markSaved();
      expect(notifications, 1);
    });

    test('discard：回到已保存那份，dirty 清除', () {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      draft.adoptSaved(<String, Object?>{
        'motion_bias': 'eager',
        'motion_rate': 'dense',
      });
      draft.selectBias('restrained');
      draft.selectRate('sparse');
      expect(draft.dirty, isTrue);

      draft.discard();
      expect(draft.bias, 'eager');
      expect(draft.rate, 'dense');
      expect(draft.dirty, isFalse);
    });

    test('payload 每次返回新 map（保存前现场快照，不共享可变状态）', () {
      final DirectorMotionDraft draft = DirectorMotionDraft();
      addTearDown(draft.dispose);
      final Map<String, Object?> first = draft.payload();
      first['motion_bias'] = 'eager';
      expect(draft.bias, 'normal', reason: '改返回的 map 不得反向改草稿');
      expect(draft.payload()['motion_bias'], 'normal');
    });
  });

  group('ModsApi.config（GET /api/v1/mods/{id}/config）', () {
    test('打到正确路径并解出 config', () async {
      late http.Request seen;
      final ModsApi api = ModsApi(
        base: 'http://127.0.0.1:18080',
        client: MockClient((http.Request r) async {
          seen = r;
          return _json(
            '{"id":"director","config":{"motion_bias":"eager",'
            '"motion_rate":"dense"}}',
            200,
          );
        }),
      );
      final Map<String, Object?> config = await api.config('director');
      expect(seen.method, 'GET');
      expect(seen.url.path, '/api/v1/mods/director/config');
      expect(config['motion_bias'], 'eager');
      expect(config['motion_rate'], 'dense');
      expect(
        seen.headers.containsKey('content-type'),
        isFalse,
        reason: '只读 GET 不需要 Content-Type',
      );
    });

    test('404（导演不在注册表）→ 抛 ApiException，宿主可吞掉回缺省', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient(
          (_) async => _json(
            '{"error":{"code":"not_found","message":"Mod director 不在注册表"}}',
            404,
          ),
        ),
      );
      expect(
        () => api.config('director'),
        throwsA(
          isA<ApiException>()
              .having((ApiException e) => e.code, 'code', 'not_found')
              .having((ApiException e) => e.status, 'status', 404),
        ),
      );
    });

    test('响应缺 config → 空 map（调用方按缺键回 normal）', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient((_) async => _json('{"id":"director"}', 200)),
      );
      expect(await api.config('director'), isEmpty);
    });

    test('网络失败 → network_error（不抛出裸异常）', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient((_) async => throw const _Boom()),
      );
      expect(
        () => api.config('director'),
        throwsA(
          isA<ApiException>().having(
            (ApiException e) => e.code,
            'code',
            'network_error',
          ),
        ),
      );
    });
  });
}

class _Boom implements Exception {
  const _Boom();
  @override
  String toString() => '连接被拒绝';
}
