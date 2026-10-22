/// 播放忙闲上报（0.3.14）：**只在翻转时发**、**失败静默**。
///
/// # 为什么这两条要单独钉住
///
/// `POST /api/v1/playback` 是服务端「外部注入等播放闲」的**唯一**浏览器侧输入：
///
/// 1. 每句都发 = 服务端的「最后一次上报时刻」被刷得比真实播放还新，120 秒
///    超时判据（执行单 §行为）永远不触发——它的语义会被悄悄改掉；
/// 2. 发一次就抛 = 一次网络抖动就能把播放路径带进异常处理（本单的红线是
///    「失败不影响播放、不产生任何界面文案」）。
///
/// 判据与传输都在 `api/playback_api.dart`（纯逻辑 + 可注入 `http.Client`），
/// 所以这里能在 VM 上跑；接线（`AudioPlayer.pendingState` → 上报）是源码扫描的
/// 事，见 `action_cue_test.dart` 的 0.3.14 组。
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:live2d_ai_shell/api/playback_api.dart';

void main() {
  group('PlaybackReporter：只在翻转时上报，失败静默', () {
    test('忙 → 闲各一次；重复值不再发', () async {
      final List<bool> sent = <bool>[];
      final PlaybackReporter reporter = PlaybackReporter(
        send: (bool busy) async => sent.add(busy),
      );
      reporter.report(true);
      reporter.report(true); // 同一个值：不发
      reporter.report(false);
      reporter.report(false); // 同一个值：不发
      reporter.report(true);
      await Future<void>.delayed(Duration.zero);

      expect(
        sent,
        <bool>[true, false, true],
        reason: '服务端只关心「忙 → 闲」这个边沿；重复值会把最后上报时刻刷假',
      );
    });

    test('发送抛异常：不向调用方抛，后续上报照常', () async {
      final List<bool> sent = <bool>[];
      final PlaybackReporter reporter = PlaybackReporter(
        send: (bool busy) async {
          sent.add(busy);
          throw StateError('network down');
        },
      );
      // 两次都不该抛（report 是同步的 fire-and-forget）。
      reporter.report(true);
      reporter.report(false);
      await Future<void>.delayed(Duration.zero);

      expect(sent, <bool>[true, false], reason: '失败不重试、也不吃掉后续边沿');
    });
  });

  group('PlaybackApi：POST /api/v1/playback', () {
    test('同源 POST JSON，body 只有 busy，Content-Type 是 application/json',
        () async {
      final List<http.Request> requests = <http.Request>[];
      final PlaybackApi api = PlaybackApi(
        client: MockClient((http.Request request) async {
          requests.add(request);
          return http.Response('{}', 200);
        }),
      );

      await api.report(true);
      expect(requests, hasLength(1));
      expect(requests.single.method, 'POST');
      expect(requests.single.url.path, '/api/v1/playback');
      expect(requests.single.headers['Content-Type'], 'application/json');
      expect(requests.single.body, '{"busy":true}');

      await api.report(false);
      expect(requests, hasLength(2));
      expect(requests.last.body, '{"busy":false}');
      api.dispose();
    });

    test('非 2xx 不抛（静默）；连接层异常同样不抛', () async {
      final PlaybackApi failing = PlaybackApi(
        client: MockClient((_) async => http.Response('nope', 503)),
      );
      await failing.report(true); // 不抛
      failing.dispose();

      final PlaybackApi throwing = PlaybackApi(
        client: MockClient((_) async => throw StateError('connect refused')),
      );
      await throwing.report(false); // 不抛
      throwing.dispose();
    });
  });
}
