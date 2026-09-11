import 'package:flutter/widgets.dart';

/// 非 web：没有平台视图，也就没有「iframe 把指针吃进自己的文档」这回事。
///
/// 透明返回 [child]：垫层在 VM（`flutter test`）上不该改变任何布局。
/// `enabled` 在这一侧没有对应物——不放 `<div>` 就没有可关的东西。
Widget buildStagePointerInterceptor(Widget child, {required bool enabled}) =>
    child;
