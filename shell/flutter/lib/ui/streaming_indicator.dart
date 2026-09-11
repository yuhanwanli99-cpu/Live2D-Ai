/// 流式中的节流提示（L3）。
///
/// 规格 §5.2 的一条约束：**不用 `CircularProgressIndicator` 做装饰**——
/// 那是一个 Ticker 驱动的持续动画，语义上表示「进度」，用一个转圈表示
/// 「正在生成」既费电又不告诉用户任何进展。
///
/// 这里做的是**三点呼吸**：只在 `thinking` 相位渲染，且尊重 reduced-motion
/// （静止三点）。「说话中」不显示它——口型已经在动了，再加一个指示器是噪声。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import '../state/ui_phase.dart';
import 'theme.dart';

class StreamingIndicator extends StatefulWidget {
  const StreamingIndicator({required this.phase, super.key});

  final UiPhase phase;

  @override
  State<StreamingIndicator> createState() => _StreamingIndicatorState();
}

class _StreamingIndicatorState extends State<StreamingIndicator>
    with SingleTickerProviderStateMixin {
  /// 与 `StatePill` 的呼吸光**共用同一个节拍**（同频 = 看起来是一件事）。
  static const Duration _period = AppRhythms.thinkingBreath;

  AnimationController? _controller;

  @override
  void initState() {
    super.initState();
    _sync();
  }

  @override
  void didUpdateWidget(StreamingIndicator oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.phase != widget.phase) _sync();
  }

  void _sync() {
    if (widget.phase == UiPhase.thinking) {
      _controller ??= AnimationController(vsync: this, duration: _period);
      if (!_controller!.isAnimating) _controller!.repeat();
    } else {
      _controller?.stop();
    }
  }

  @override
  void dispose() {
    _controller?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // 只在「思考中」出现；其它相位**彻底不占位**（不渲染空 SizedBox，
    // 避免在消息列表尾部留一段说不清来历的空白）。
    if (widget.phase != UiPhase.thinking) return const SizedBox.shrink();

    final AppColors colors = appColorsOf(context);
    final bool reduced = MediaQuery.disableAnimationsOf(context);

    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: Space.s2,
        vertical: Space.s1,
      ),
      child: Semantics(
        label: '正在生成回复',
        excludeSemantics: true,
        child: reduced
            ? _Dots(value: 0.5, tone: colors.contentMuted)
            : AnimatedBuilder(
                animation: _controller!,
                builder: (BuildContext context, Widget? _) =>
                    _Dots(value: _controller!.value, tone: colors.contentMuted),
              ),
      ),
    );
  }
}

class _Dots extends StatelessWidget {
  const _Dots({required this.value, required this.tone});

  final double value;
  final Color tone;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        for (int i = 0; i < 3; i++) ...<Widget>[
          if (i > 0) const SizedBox(width: 3),
          Opacity(
            // 三点错峰：相位差 1/3 周期。
            opacity: 0.25 + 0.75 * _wave(value - i / 3),
            child: Container(
              width: 4,
              height: 4,
              decoration: BoxDecoration(shape: BoxShape.circle, color: tone),
            ),
          ),
        ],
      ],
    );
  }

  /// 把相位折到 [0,1] 的三角波（两端 0、中间 1）。
  static double _wave(double phase) {
    final double t = phase - phase.floorToDouble();
    return t < 0.5 ? t * 2 : (1 - t) * 2;
  }
}
