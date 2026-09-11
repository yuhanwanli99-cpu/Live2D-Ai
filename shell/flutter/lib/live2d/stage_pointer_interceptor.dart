/// 舞台指针垫层：让**压在 `/render` 之上的 Flutter 控件**重新收得到指针。
///
/// # 症状（2026-09-11 用户报告「设置能唤醒但点不动、也不能上下滑」）
///
/// 舞台是 `<iframe>` 平台视图，而**设置浮层、断线横幅、缩放角标、错误重试**
/// 全都画在它上面。它们看得见、却完全点不着——连滚轮都不响应。
///
/// # 根因（真机 DOM，不是猜的）
///
/// Flutter Web 的场景结构（`flt-glass-pane` 的 shadow root）是：
///
/// ```text
/// FLT-SCENE-HOST
///   FLT-CANVAS-CONTAINER  → CANVAS        pointer-events: none   ← 舞台下面的画面
///   FLT-PLATFORM-VIEW-SLOT → <slot>       pointer-events: auto   ← 平台视图插槽
///   FLT-CANVAS-CONTAINER  → CANVAS        pointer-events: none   ← **画在舞台上层的画面**
/// FLT-PLATFORM-VIEW#flt-pv-0 → IFRAME      pointer-events: auto
/// ```
///
/// 两张画布都是 `pointer-events: none`；真正接指针的是 **iframe 这个 DOM
/// 元素**。于是指针落在舞台区域时，事件被派发进 **iframe 自己的文档**——
/// 父页（Flutter）**一个都收不到**（iframe 文档里的事件不冒泡到父文档）。
/// 结果：Flutter 把控件**画**在了 iframe 之上（上层画布），却**接不到**那一块
/// 的指针。`elementsFromPoint(500, 300)` 的答案是 `IFRAME`，不是控件。
///
/// # 做法
///
/// 在控件**下面**垫一层**透明的 `<div>` 平台视图**：
///
/// - 它在场景里排在 iframe **之后**（因为控件本来就画在舞台之后），
///   所以在 DOM 里也排在 iframe 之上；
/// - 它默认 `pointer-events: auto`，于是那一块的指针落到**父页**，
///   Flutter 的命中测试再把事件交给真正画在上面的控件。
///
/// 与 `flutter/packages` 的 [`pointer_interceptor`] 同一手法（官方就是为这个
/// 问题发的包，见 flutter/flutter#190574）。本项目运行时依赖只有
/// `flutter + http + web`（`shell/README.md` §依赖约束），所以自带一版最小实现。
///
/// [`pointer_interceptor`]: https://pub.dev/packages/pointer_interceptor
///
/// # 规矩（**新增压在舞台上的控件时必须照做**）
///
/// 只要一个控件**可能盖住舞台 iframe**，且它需要点击/滚动，就必须包一层
/// [StagePointerInterceptor]。漏了不会报错——只会「看得见、点不着」，
/// 所以 `test/stage_pointer_interceptor_test.dart` 把已知的四处钉住了。
///
/// # 诚实记录的边界（垫层管不到的地方）
///
/// 垫层只能罩住**它包起来的那个盒子**，所以下面两处仍然点不着（都在舞台上方）：
///
/// - `showModalBottomSheet` 自带的**模态障碍层**（「点空白关闭」）；
/// - `showModalBottomSheet(showDragHandle: true)` 的那根**拖拽把手**——
///   它由 `BottomSheet` 画在 `builder` 的子树**之外**，我们的盒子罩不到它。
///
/// 关闭设置因此仍是 **Esc / 面板右上角 ✕ / rail 上再点一次同一分区** 三条路
/// （compact 还有抽屉自身的关闭）。要连障碍层一起修，只能在设置面板打开期间
/// 把整块舞台铺一层遮挡——那会把「设置开着时还能拖模型」一起吃掉，
/// 而模型拖动是渲染面的真实交互（`sync.clickEnabled`）。两害相权，先不铺。
library;

import 'package:flutter/widgets.dart';

import 'stage_pointer_interceptor_stub.dart'
    if (dart.library.js_interop) 'stage_pointer_interceptor_web.dart' as impl;

/// 包在**压在舞台之上、且需要接收指针的控件**外面。
///
/// 非 web 平台是透明的（直接返回 [child]），所以 widget 测试里它只是一个
/// 可被 `find.byType` 认出来的标记。
///
/// # [enabled]：垫层自己也要能被关掉（2026-09-11，P1-1 加的）
///
/// 垫层的代价是「这块区域的指针回到父页」——**包括它没收起来的时候**。
/// 一旦某个控件改成「折叠而不是卸载」（见 `CollapsiblePanel`：
/// 折起来时子树仍在树里），它外面那层垫层也会一直留着：收起后宽度为 0
/// 看着没事，但只要它的盒子还有面积，舞台那一块就**再也拖不动模型**了。
///
/// 所以垫层必须能跟着收起：`enabled: false` 时把 DOM 元素的
/// `pointer-events` 设成 `none`，指针重新落到 iframe（模型照常可拖）。
/// 刻意用「改样式」而不是「不挂载」：不挂载会换掉 widget 类型 →
/// 下面的子树重建 → 折叠面板的保活白做了。
class StagePointerInterceptor extends StatelessWidget {
  const StagePointerInterceptor({
    required this.child,
    this.enabled = true,
    super.key,
  });

  /// 真正要接收指针的内容。
  final Widget child;

  /// `false` = 暂时把这一块的指针还给舞台。
  final bool enabled;

  @override
  Widget build(BuildContext context) =>
      impl.buildStagePointerInterceptor(child, enabled: enabled);
}
