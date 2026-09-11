/// 页内切换：两页**互斥**地交叉淡化 + 轻微位移。
///
/// 2026-09-11（前端加强计划 P2-1）新增，形状来自 Morrow 的
/// `settings_page_transition.dart`（Apache-2.0，见 `CREDITS.md` §6.5），
/// 按本项目约束改造：时长走 `AppDurations`、减少动画走 [appMotion]、
/// 曲线走 `Motion`。
///
/// # 为什么需要它
///
/// compact 断点过去的设置入口是**两步跳**：先弹一个 8 项抽屉，选完分区
/// 再弹底部浮层。观感上像「弹了两次」，而且抽屉选完就消失、用户还得
/// 再等一次动画。Morrow 的做法是**页内过渡**：整个工作台淡出、设置页
/// 淡入，一次到位。
///
/// # 三个关键设计（都有具体理由，别改）
///
/// ## ① 时间轴被 `.5` 切成两半 —— 两页**永不同时可见**
///
/// 前半段只淡出 A，后半段只淡入 B；`visible` 是**硬开关**
/// （`motion.value >= 0.5`）。
///
/// 天真的做法是让两页各按 `t` 与 `1-t` 淡入淡出——那在中间时刻**两页都
/// 半透明地叠在一起**，窄屏上会看起来像花屏（文字互相穿插）。
/// 切两半之后任意一帧只有一页在画。
///
/// ## ② `Offstage` 而不是条件插入 —— 工作台**保留状态**
///
/// `Offstage` 不 paint，但 Element/RenderObject 还在：聊天列表的滚动位置、
/// 输入框里的半句话、舞台的 iframe 都不会因为「打开设置」而丢。
/// 这与本项目对舞台的保活纪律是同一条（`test/stage_keepalive_test.dart`）。
///
/// ## ③ `Duration.zero` 的语义是「**立刻到终态**」，不是「不动」
///
/// 减少动画时不能只把时长设成 0 就撒手：那样 `AnimationController` 会停在
/// 起点（旧页），而调用方以为已经切过去了。所以这里显式判断零时长并直接
/// 把 `value` 设到目标位。好处是**动画播到一半时系统打开「减少动画」
/// 也能立刻收敛**，不会卡在半透明。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import '../ui/soft_motion.dart';

/// 两页互斥切换（A ↔ B）。
class PageCrossFade extends StatefulWidget {
  const PageCrossFade({
    required this.showSecond,
    required this.first,
    required this.second,
    this.duration,
    this.slide = 12,
    super.key,
  });

  /// `true` = 显示 [second]。
  final bool showSecond;

  /// 第一页（工作台）。**同一个实例**要一直传进来，否则保活失效。
  final Widget first;

  /// 第二页（设置页）。
  final Widget second;

  /// 缺省 `AppDurations.slow`（320 ms）。
  final Duration? duration;

  /// 水平位移量（px）。两页方向相反，看起来像「一页把另一页推走」。
  final double slide;

  @override
  State<PageCrossFade> createState() => _PageCrossFadeState();
}

class _PageCrossFadeState extends State<PageCrossFade>
    with SingleTickerProviderStateMixin {
  late final AnimationController _motion;

  @override
  void initState() {
    super.initState();
    _motion = AnimationController(
      vsync: this,
      // 初值直接是终态：**首帧不播动画**（页面第一次出现时不该闪一下）。
      value: widget.showSecond ? 1 : 0,
      duration: Duration.zero,
    );
  }

  @override
  void didUpdateWidget(PageCrossFade oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.showSecond == oldWidget.showSecond) return;

    final Duration d = appMotion(context, widget.duration ?? AppDurations.slow);
    if (d == Duration.zero) {
      // 见头注 ③：立刻到终态，不是「不动」。
      _motion.value = widget.showSecond ? 1 : 0;
      return;
    }
    _motion
      ..duration = d
      ..animateTo(widget.showSecond ? 1 : 0, curve: Motion.state);
  }

  @override
  void dispose() {
    _motion.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _motion,
      builder: (BuildContext context, Widget? _) {
        // 硬开关：过半才换树（见头注 ①）。
        final bool secondVisible = _motion.value >= 0.5;
        // 前一半淡出 A、后一半淡入 B —— 两条曲线各占一半时间轴。
        final double firstOpacity =
            1 - Curves.easeInCubic.transform((_motion.value * 2).clamp(0.0, 1.0));
        final double secondOpacity = Curves.easeOutCubic.transform(
          ((_motion.value * 2) - 1).clamp(0.0, 1.0),
        );
        final bool moving = _motion.isAnimating;

        // # 两页**都常驻在树里**（与 Morrow 的一处偏离，理由在这里）
        //
        // Morrow 的版本是 `if (settingsVisible) 第二页`：第二页只在过半之后
        // 才**构建**。对本项目有两个问题：
        //
        // 1. **保活**：compact 关掉设置再打开，设置页的滚动位置与草稿会丢。
        //    本项目对「关掉再打开还在原位」有明确纪律（P1-1 的
        //    `CollapsiblePanel`），compact 不该是例外。
        // 2. **切换那一帧会卡**：过半才构建 = 那一帧要把整棵设置页建出来，
        //    窄设备上正好卡在动画中段。常驻就没有这一下。
        //
        // 隐藏的一页用 `Offstage`：不 paint，但 Element/RenderObject 都在。
        return Stack(
          fit: StackFit.expand,
          children: <Widget>[
            _page(
              key: const ValueKey<String>('page-first-opacity'),
              visible: !secondVisible,
              opacity: firstOpacity,
              slide: -widget.slide * (1 - firstOpacity),
              ignoring: moving,
              child: widget.first,
            ),
            _page(
              key: const ValueKey<String>('page-second-opacity'),
              visible: secondVisible,
              opacity: secondOpacity,
              slide: widget.slide * (1 - secondOpacity),
              ignoring: moving,
              child: widget.second,
            ),
          ],
        );
      },
    );
  }

  /// 一页：隐藏时 `Offstage` 不 paint，但仍保留 Element 与状态。
  ///
  /// `TickerMode(enabled: visible)` 是必须的：隐藏那一页里的动画
  /// （比如设置面板的骨架屏、舞台角标的呼吸）不该继续跑。
  Widget _page({
    required Key key,
    required bool visible,
    required double opacity,
    required double slide,
    required bool ignoring,
    required Widget child,
  }) => Offstage(
    offstage: !visible,
    child: ExcludeFocus(
      excluding: !visible,
      child: TickerMode(
        enabled: visible,
        child: IgnorePointer(
          // 动画中点哪一页都不该生效：正在移动的东西被点中，
          // 用户的意图和实际命中的控件会对不上。
          ignoring: ignoring,
          child: Opacity(
            key: key,
            opacity: opacity,
            child: Transform.translate(offset: Offset(slide, 0), child: child),
          ),
        ),
      ),
    ),
  );
}
