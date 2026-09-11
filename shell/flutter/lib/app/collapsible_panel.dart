/// 可折叠面板：**折起来的时候，子树不重建**。
///
/// 2026-09-11（前端加强计划 P1-1）新增，形状来自 Morrow 的
/// `collapsible_panel.dart`（Apache-2.0，见 `CREDITS.md` §6.5），
/// 按本项目约束改造了两处：时长走 `AppDurations` 令牌、减少动画走
/// [appMotion] 统一出口。
///
/// # 为什么不能用 `if (expanded) panel`
///
/// 条件插入 = 关掉再打开时**子树是新的**。对设置面板来说，代价是：
/// 滚动位置回到顶部、输入框焦点丢失、分区草稿被重置、内联的
/// 「测试连接」结果消失。用户看到的是「我刚滚到下面，关一下再开就跳回去了」。
///
/// 本项目对**舞台**已经有这条纪律（`test/stage_keepalive_test.dart` 用
/// `initState` 计数钉死 iframe 不重建），但**设置面板自己**没有——
/// `app_shell.dart` 里写的是 `if (settingsOpen && inlineSettings) Positioned.fill(...)`。
/// 本组件把同一条纪律补给面板。
///
/// # 三个实现要点（照抄 Morrow 的解法，逐条都有理由）
///
/// 1. **尺寸靠 `Align(widthFactor/heightFactor)`，不靠条件插入**：
///    `widthFactor: 0` 让子树参与构建但**不占位**，于是 Element 一直活着。
/// 2. **`child` 传进 `TweenAnimationBuilder` 的 `child` 参数**（不是 builder 里现造）：
///    这是「同一个 Element 实例」的关键——builder 每次重建都拿到同一个 child。
/// 3. **折起来时同时关掉四条通路**：`IgnorePointer`（不吃指针）、
///    `ExcludeFocus`（不进 Tab 序）、`ExcludeSemantics`（不被读屏念到）、
///    `TickerMode`（子树里的动画不再跑）。少关任何一条，折叠面板都会
///    「明明收起来了却还能被点到 / 念到」——这四条是**一起**才有意义。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import '../ui/soft_motion.dart';

/// 沿 [axis] 折叠/展开的单子节点容器。
class CollapsiblePanel extends StatelessWidget {
  const CollapsiblePanel({
    required this.expanded,
    required this.child,
    this.axis = Axis.horizontal,
    this.extent,
    this.duration,
    super.key,
  });

  /// 展开中（`false` = 收起，但子树仍在树里）。
  final bool expanded;

  /// 被折叠的内容。**必须是稳定实例**——包一层 `Builder` 或在调用处现造
  /// 一个新 widget 都不影响 Element 复用（同类型同位置即复用），
  /// 但别给它加会变的 `key`，那会强制重建。
  final Widget child;

  /// `Axis.horizontal` = 折叠宽度；`Axis.vertical` = 折叠高度。
  final Axis axis;

  /// 展开时的固定尺寸（`horizontal` 时是宽度）。`null` = 用子树的固有尺寸。
  final double? extent;

  /// 缺省 `AppDurations.slow`（320 ms）——折叠是一次「位置变化」，
  /// 比 hover 那种 120 ms 的即时反馈慢一档，又不至于像入场动画那样拖沓。
  final Duration? duration;

  @override
  Widget build(BuildContext context) {
    return TweenAnimationBuilder<double>(
      tween: Tween<double>(end: expanded ? 1 : 0),
      duration: appMotion(context, duration ?? AppDurations.slow),
      curve: Motion.state,
      // child 提到 builder 外面 —— 折叠/展开的每一帧都复用它。
      child: child,
      builder: (BuildContext context, double value, Widget? child) {
        final Widget panel = IgnorePointer(
          ignoring: !expanded,
          child: ExcludeFocus(
            excluding: !expanded,
            child: ExcludeSemantics(
              excluding: !expanded,
              child: TickerMode(
                enabled: value > 0,
                child: Opacity(
                  opacity: value.clamp(0.0, 1.0),
                  child: axis == Axis.horizontal
                      ? SizedBox(width: extent, child: child)
                      : child,
                ),
              ),
            ),
          ),
        );
        return ClipRect(
          child: Align(
            alignment: Alignment.topLeft,
            widthFactor: axis == Axis.horizontal ? value : null,
            heightFactor: axis == Axis.vertical ? value : null,
            child: panel,
          ),
        );
      },
    );
  }
}
