/// 动效基元：**全仓库唯一的 reduced-motion 出口** + 两个换内容的小工具。
///
/// 2026-09-11（前端加强计划 P1-0）新增。来历值得写清楚：
///
/// # 为什么需要一个「出口函数」，而不是各写各的
///
/// 本项目的动效令牌（`AppDurations` 4 档 + `Motion` 2 条曲线）比参考项目
/// Morrow 规范得多，但**接线数为 0**——在加这个文件之前，全应用只有
/// `theme_picker.dart` 一处 `AnimatedContainer`。原因是缺一个**统一出口**：
/// 每个要动的地方都得自己写一遍
/// `MediaQuery.disableAnimationsOf(context) ? Duration.zero : ...`，
/// 写两遍之后就会有人漏写，而漏写**不报错**——只在开了「减少动画」的
/// 用户那里表现为「说好的不动，结果它还在动」。
///
/// Morrow 的全部动效都经过它那个四行的 `motionDuration(context, ms)`
/// （见 `docs/plans/PLAN-frontend-strengthening-2026-09-11.md` §1 判定一）。
/// 我们抄这个**形状**，但参数收窄成 `AppDurations` 的令牌：
///
/// ```dart
/// duration: appMotion(context, AppDurations.base)   // ✅
/// duration: Duration(milliseconds: 200)             // ❌ 裸值，门禁会红
/// ```
///
/// # `Duration.zero` 的语义是「**立刻到终态**」，不是「不动」
///
/// 这两者差别很大：`AnimationController` 拿到 `Duration.zero` 会直接跳到
/// `value = 1`（完成态），而不是停在起点。Morrow 的
/// `SettingsPageTransition` 专门为这件事写了一行
/// （`if (widget.duration == Duration.zero) motion.value = 目标值`），
/// 好处是**动画播到一半时系统打开「减少动画」也能立刻收敛**，
/// 而不是卡在半透明状态。
///
/// 所以用 `appMotion` 的地方不要自己再判断 `reduce` 然后返回 `child`——
/// 交给隐式动画组件处理 `Duration.zero` 即可（`AnimatedContainer` /
/// `AnimatedSwitcher` / `TweenAnimationBuilder` 都是这个语义）。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';

/// 动效时长出口：把令牌过一次「减少动画」闸门。
///
/// 返回 `Duration.zero` 时，隐式动画组件会**立刻到终态**（详见文件头注）。
Duration appMotion(BuildContext context, Duration token) =>
    MediaQuery.disableAnimationsOf(context) ? Duration.zero : token;

/// 换内容：旧内容淡出缩放、新内容淡入缩放，**旧内容不再吃指针与语义**。
///
/// 与 `AnimatedSwitcher` 的默认 `layoutBuilder` 区别在这里：默认实现会把
/// 新旧两个 child 直接叠起来，旧 child 在淡出期间**仍然可点、仍被读屏念到**
/// （点两下会触发两次）。Morrow 的 `SoftSwap` 给旧 child 套了
/// `IgnorePointer` + `ExcludeSemantics`，本实现沿用。
///
/// 缩放从 0.9 起（不是 0），是为了避免「从无到有」的弹跳感——
/// 换的是内容，不是弹窗。
class SoftSwap extends StatelessWidget {
  const SoftSwap({required this.child, this.duration, super.key});

  final Widget child;

  /// 缺省 `AppDurations.base`（200 ms）。
  final Duration? duration;

  @override
  Widget build(BuildContext context) {
    return AnimatedSwitcher(
      duration: appMotion(context, duration ?? AppDurations.base),
      switchInCurve: Motion.enter,
      switchOutCurve: Motion.state,
      layoutBuilder: (Widget? current, List<Widget> previous) => Stack(
        alignment: Alignment.center,
        children: <Widget>[
          for (final Widget child in previous)
            IgnorePointer(child: ExcludeSemantics(child: child)),
          ?current,
        ],
      ),
      transitionBuilder: (Widget child, Animation<double> animation) =>
          FadeTransition(
            opacity: animation,
            child: ScaleTransition(
              scale: Tween<double>(begin: 0.9, end: 1).animate(animation),
              child: child,
            ),
          ),
      child: child,
    );
  }
}

/// 尺寸变化：包一层 `AnimatedSize`，减少动画时**直接返回 child**。
///
/// 这里刻意**不**用 `Duration.zero`：`AnimatedSize` 在零时长下仍会重建一次
/// 布局管线，而减少动画的用户要的是「根本没有这一跳」。两者观感相同，
/// 但直通少一次无谓的 layout。
class SoftResize extends StatelessWidget {
  const SoftResize({
    required this.child,
    this.duration,
    this.alignment = Alignment.topCenter,
    super.key,
  });

  final Widget child;
  final Duration? duration;
  final Alignment alignment;

  @override
  Widget build(BuildContext context) {
    if (MediaQuery.disableAnimationsOf(context)) return child;
    return AnimatedSize(
      duration: duration ?? AppDurations.base,
      alignment: alignment,
      child: child,
    );
  }
}

/// **启动揭示**：整个界面在首帧淡入一次，之后再不重播。
///
/// # 为什么需要它（不只是「好看」）
///
/// 舞台是一个 `<iframe>` 平台视图，它自己的第一帧（渲染面 wasm 起来之前）
/// 是一块空白。没有揭示动画时，用户看到的是「先闪一块白，然后内容跳出来」；
/// 有一次淡入就把这段白挡在背后了。
///
/// 时长取 `AppDurations.reveal`（600 ms）——这一档的语义就是
/// 「**仅此一处**长动画」，用的是它该用的地方（规格 §2.7）。
///
/// # 为什么必须「只播一次」
///
/// 外壳会被 WS 事件频繁重建。如果揭示动画挂在每次 build 上，界面会**不停地
/// 淡入**。所以控制器住在 [State] 里、`forward()` 只在 `initState` 调一次。
///
/// 减少动画时**直接返回 child**（不是零时长）：启动时不该有任何等待感。
class StartupReveal extends StatefulWidget {
  const StartupReveal({required this.child, super.key});

  final Widget child;

  @override
  State<StartupReveal> createState() => _StartupRevealState();
}

class _StartupRevealState extends State<StartupReveal>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: AppDurations.reveal,
      value: 1,
    );
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // 只播一次：`value == 1` 表示已经就位或正在播。
    if (_controller.value == 1 && !_revealed) {
      _revealed = true;
      if (MediaQuery.disableAnimationsOf(context)) {
        _controller.value = 1;
      } else {
        _controller.forward(from: 0);
      }
    }
  }

  bool _revealed = false;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => FadeTransition(
    opacity: _controller,
    child: widget.child,
  );
}
