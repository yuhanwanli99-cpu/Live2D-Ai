import 'package:flutter/material.dart';

import 'live2d_transport.dart';

/// 非 web 平台没有 `HtmlElementView`，不支持内嵌 `/render`。
const bool live2dHostSupported = false;

/// 与 `live2d_host_web.dart` 同签名的占位实现（条件导入的另一分支）。
Widget buildLive2DHost({
  Key? key,
  required void Function(Live2DTransport transport) onTransport,
  required void Function(String message) onError,
}) {
  return const Center(
    child: Padding(
      padding: EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Icon(Icons.desktop_access_disabled, size: 48),
          SizedBox(height: 12),
          Text(
            'Live2D 舞台仅在 Web 平台可用',
            textAlign: TextAlign.center,
            style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
          ),
          SizedBox(height: 8),
          Text(
            '请执行 flutter build web 后，由 Rust 服务以 /app/ 托管并打开。',
            textAlign: TextAlign.center,
          ),
        ],
      ),
    ),
  );
}
