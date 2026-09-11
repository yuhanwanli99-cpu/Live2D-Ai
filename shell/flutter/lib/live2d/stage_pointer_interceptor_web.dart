import 'package:flutter/widgets.dart';
import 'package:web/web.dart' as web;

/// web：在 [child] 下面垫一层透明的 `<div>` 平台视图（见同目录 `.dart` 头注）。
///
/// 为什么不是「把 iframe 的 `pointer-events` 置 `none`」：那是**全有或全无**
/// 的开关，会把「设置开着时还能拖模型」一起关掉；而垫层是按盒子生效的，
/// 只让被控件盖住的那一块回到父页。
Widget buildStagePointerInterceptor(Widget child) {
  return Stack(
    // `passthrough`：垫层不改变布局，盒子的尺寸/位置**完全跟着 child 走**。
    fit: StackFit.passthrough,
    children: <Widget>[
      Positioned.fill(
        child: HtmlElementView.fromTagName(
          tagName: 'div',
          onElementCreated: (Object element) {
            final web.HTMLDivElement div = element as web.HTMLDivElement;
            // 空 div 默认就吃指针；显式写出来是为了不被别处的全局样式改掉，
            // 也为了让「它凭什么能接到事件」在代码里看得见。
            div.style
              ..setProperty('pointer-events', 'auto')
              ..setProperty('background', 'transparent')
              ..setProperty('border', '0');
          },
        ),
      ),
      child,
    ],
  );
}
