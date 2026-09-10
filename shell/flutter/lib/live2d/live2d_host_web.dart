import 'dart:async';
import 'dart:js_interop';

import 'package:flutter/widgets.dart';
import 'package:web/web.dart' as web;

import 'live2d_transport.dart';

/// Web 平台支持把 `/render` 内嵌为 iframe。
const bool live2dHostSupported = true;

/// 构建承载 `/render` 的 iframe，并在 DOM 元素创建后回调传输实现。
///
/// 与 `live2d_host_stub.dart` 暴露同一 API，由调用方条件导入。
Widget buildLive2DHost({
  Key? key,
  required void Function(Live2DTransport transport) onTransport,
  required void Function(String message) onError,
}) {
  return HtmlElementView.fromTagName(
    key: key,
    tagName: 'iframe',
    onElementCreated: (Object element) {
      try {
        final iframe = element as web.HTMLIFrameElement;
        iframe.setAttribute('allow', 'autoplay; fullscreen');
        iframe.setAttribute(
          'style',
          'border:0;width:100%;height:100%;display:block;background:transparent',
        );
        // 同源相对路径：只有在 /app/ 与 /render 由同一个 loopback 服务托管时成立。
        iframe.src = '/render';
        onTransport(WebIframeTransport(iframe, onError: onError));
      } catch (error) {
        onError('渲染面 iframe 初始化失败：$error');
      }
    },
  );
}

/// iframe 传输：`postMessage(jsonString, location.origin)` 发送，
/// 监听 window message 并校验 `source == iframe.contentWindow` 且
/// `origin == location.origin`。
class WebIframeTransport implements Live2DTransport {
  WebIframeTransport(this._iframe, {required this.onError}) {
    _listener = ((web.MessageEvent event) {
      try {
        if (event.origin != web.window.location.origin) return;
        final source = event.source;
        if (source == null || source != _iframe.contentWindow) return;
        final data = event.data;
        if (data == null || !data.isA<JSString>()) return;
        if (!_controller.isClosed) _controller.add((data as JSString).toDart);
      } catch (error) {
        onError('渲染面消息解析失败：$error');
      }
    }).toJS;
    web.window.addEventListener('message', _listener);
  }

  final web.HTMLIFrameElement _iframe;
  final void Function(String) onError;
  final StreamController<String> _controller =
      StreamController<String>.broadcast();
  late final JSFunction _listener;
  bool _disposed = false;

  @override
  Stream<String> get events => _controller.stream;

  @override
  Future<void> send(String json) async {
    if (_disposed) return;
    final target = _iframe.contentWindow;
    if (target == null) {
      throw StateError('iframe.contentWindow 不可用（渲染面未挂载）');
    }
    target.postMessage(json.toJS, web.window.location.origin.toJS);
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    web.window.removeEventListener('message', _listener);
    await _controller.close();
  }
}
