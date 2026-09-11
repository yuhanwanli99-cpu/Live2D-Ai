import 'package:flutter/widgets.dart';
import 'package:web/web.dart' as web;

/// web：在 [child] 下面垫一层透明的 `<div>` 平台视图（见同目录 `.dart` 头注）。
///
/// 为什么不是「把 iframe 的 `pointer-events` 置 `none`」：那是**全有或全无**
/// 的开关，会把「设置开着时还能拖模型」一起关掉；而垫层是按盒子生效的，
/// 只让被控件盖住的那一块回到父页。
///
/// [enabled] 为 `false` 时把这一层 `<div>` 的 `pointer-events` 改回 `none`：
/// 元素**留在 DOM 里**（不换 widget 类型 ⇒ 下面的子树不重建），只是不再接指针，
/// 于是指针重新落回 iframe。这是给「折叠而不是卸载」的控件用的（见
/// `CollapsiblePanel` 与 `StagePointerInterceptor` 的头注）。
Widget buildStagePointerInterceptor(Widget child, {required bool enabled}) =>
    _PointerShield(enabled: enabled, child: child);

class _PointerShield extends StatefulWidget {
  const _PointerShield({required this.enabled, required this.child});

  final bool enabled;
  final Widget child;

  @override
  State<_PointerShield> createState() => _PointerShieldState();
}

class _PointerShieldState extends State<_PointerShield> {
  web.HTMLDivElement? _div;

  /// 空 div 默认就吃指针；显式写出来是为了不被别处的全局样式改掉，
  /// 也为了让「它凭什么能接到事件」在代码里看得见。
  void _apply() {
    final web.HTMLDivElement? div = _div;
    if (div == null) return;
    div.style
      ..setProperty('pointer-events', widget.enabled ? 'auto' : 'none')
      ..setProperty('background', 'transparent')
      ..setProperty('border', '0');
  }

  @override
  void didUpdateWidget(_PointerShield oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.enabled != widget.enabled) _apply();
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      // `passthrough`：垫层不改变布局，盒子的尺寸/位置**完全跟着 child 走**。
      fit: StackFit.passthrough,
      children: <Widget>[
        Positioned.fill(
          child: HtmlElementView.fromTagName(
            tagName: 'div',
            onElementCreated: (Object element) {
              _div = element as web.HTMLDivElement;
              _apply();
            },
          ),
        ),
        widget.child,
      ],
    );
  }
}
