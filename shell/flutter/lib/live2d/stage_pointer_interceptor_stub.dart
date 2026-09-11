import 'package:flutter/widgets.dart';

/// 非 web：没有平台视图，也就没有「iframe 把指针吃进自己的文档」这回事。
///
/// 透明返回 [child]：垫层在 VM（`flutter test`）上不该改变任何布局。
Widget buildStagePointerInterceptor(Widget child) => child;
