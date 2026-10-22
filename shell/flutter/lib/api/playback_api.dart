/// 播放忙闲的**静默上报**（`POST /api/v1/playback`，0.3.14）。
///
/// # 契约（执行单 §行为）
///
/// 服务端用它判断「浏览器把这一轮播完了没有」——下一次**外部**注入要等本轮
/// 生成结束（`turn_in_flight == false`）**且**播放不忙才开下一轮。缺省是
/// **闲**：前端从没上报过时，服务端只看生成是否结束（无头调用不卡住）。
///
/// 三条纪律：
///
/// 1. body 只有 `{"busy":true|false}`，同源、`Content-Type: application/json`；
/// 2. **只在翻转时发**（忙 / 闲各一次），重复值不发——见 [PlaybackReporter]；
/// 3. **失败一律静默**：不抛、不写错误、不上屏、不影响播放。播放忙只是服务端
///    排队的辅助信号，它坏了不该让用户听见 / 看见任何东西。
///
/// 真正的调用点是 `app/shell_playback_wiring.dart`（`AudioPlayer.pendingState`
/// 翻转处）；本文件只有传输与去重，**没有任何界面文案**（字体子集门禁不涉及
/// 新字形；这里也不该有说话的地方）。
library;

import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;

/// `POST /api/v1/playback` 的最小客户端（可注入 `http.Client`，VM 可测）。
class PlaybackApi {
  PlaybackApi({String? base, http.Client? client})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String _base;
  final http.Client _client;

  /// 上报一次忙闲。**永不抛**：连接失败 / 非 2xx / 超时都只等于少一次观测。
  Future<void> report(bool busy) async {
    try {
      await _client.post(
        Uri.parse('$_base/api/v1/playback'),
        headers: const <String, String>{'Content-Type': 'application/json'},
        body: jsonEncode(<String, bool>{'busy': busy}),
      );
    } catch (_) {
      // 刻意吞掉：见文件头注第 3 条。
    }
  }

  void dispose() => _client.close();
}

/// 忙闲**翻转**才发一次的上报器（0.3.14）。
///
/// 为什么在客户端去重：服务端只关心「忙 → 闲」这个**边沿**；每句都发一遍既
/// 浪费一次请求，也会把服务端的「最后一次上报时刻」刷得比真实播放还新。
///
/// 去重状态刻意在**发送前**就落账：失败不重试（静默链路，重试风暴比少一次
/// 观测更糟）。
class PlaybackReporter {
  PlaybackReporter({required this.send});

  /// 真正的发送（生产里是 [PlaybackApi.report]；测试注入假发送器）。
  final Future<void> Function(bool busy) send;

  /// 上一次真的发出去的值（`null` = 还没发过）。
  bool? _last;

  /// 上报 [busy]：与上一次相同则什么都不做；发送异常一律吞掉。
  void report(bool busy) {
    if (_last == busy) return;
    _last = busy;
    unawaited(_send(busy));
  }

  Future<void> _send(bool busy) async {
    try {
      await send(busy);
    } catch (_) {
      // 静默：上报失败不影响播放，也不产生任何界面文案。
    }
  }
}

/// 同源根：`API_BASE` 覆盖优先，否则用页面 origin。
///
/// `Uri.base.origin` 对**非 http(s)** 方案会抛（`file://` 在 `flutter test` 里
/// 常见，于是「无参构造」会直接崩）——回落一个 loopback 地址，构造永不抛，
/// 与 `api/api_client.dart` 的同一条口径。
String _normalize(String raw) {
  var value = raw.trim();
  while (value.endsWith('/')) {
    value = value.substring(0, value.length - 1);
  }
  if (value.isNotEmpty) return value;
  final Uri base = Uri.base;
  return base.scheme == 'http' || base.scheme == 'https'
      ? base.origin
      : 'http://127.0.0.1';
}
