/// **唯一**的状态呈现：色 + 形 + 字 + 节奏（L3）。规格 §6.3。
///
/// # 为什么必须只有一处
///
/// 6 个开源项目的状态指示缺失/只有文字/连接态被隐藏【调研 survey §8 缺陷 3】，
/// 最典型的是「状态机层面本来就是同一个值，界面上却长出 6 套视觉」。
/// 本文件是那张映射表的**唯一**实现点：其它任何地方都不许再写
/// `if (thinking) … else if (speaking) …` 来决定颜色/图标/文案。
///
/// # 三条硬规则
///
/// 1. **每个状态至少占「色/形/字」里的两个**（本表实际占三个）。
/// 2. **思考态的呼吸光是全应用唯一的持续动画**，且必须尊重
///    `MediaQuery.disableAnimationsOf`。这里用
///    `AnimationController(behavior: AnimationBehavior.normal)`——
///    **默认的 `preserve` 对重复动画刻意免疫 `disableAnimations`**
///    （规格 §9.4），用默认值会让「减少动态效果」对这条动画失效。
/// 3. **思考态不出提示音**：earcon 会增加认知负担，且「如果需要教用户
///    这个音是什么意思，就别用」。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import '../state/ui_phase.dart';
import 'theme.dart';

/// 一个相位的完整视觉表达。
@immutable
class UiPhaseView {
  const UiPhaseView({
    required this.label,
    required this.icon,
    required this.tone,
    required this.filledDot,
  });

  final String label;
  final IconData icon;
  final Color tone;

  /// 形通道的第二维：实心点 = 「正在进行」，空心/轮廓 = 「静止」。
  ///
  /// 只用图标形状的话，`graphic_eq`（说话）与 `psychology`（思考）在 16 px 下
  /// 远看差不多；再加一个实心/空心的差异，灰度截图里也能分开。
  final bool filledDot;
}

/// `UiPhase` → 视觉表达（**纯函数**，可在 VM/widget 测试里直接断言）。
///
/// `offline` 与 `error` 都用 danger 色，但**字与形都不同**——满足「至少两个通道」。
UiPhaseView uiPhaseView(
  UiPhase phase,
  ColorScheme scheme,
  AppColors colors,
  AppPalette palette,
) =>
    switch (phase) {
      UiPhase.offline => UiPhaseView(
        label: '后端未连接',
        icon: Icons.cloud_off,
        tone: palette.danger,
        filledDot: true,
      ),
      UiPhase.idle => UiPhaseView(
        label: '空闲',
        icon: Icons.circle_outlined,
        tone: colors.contentFaint,
        filledDot: false,
      ),
      UiPhase.thinking => UiPhaseView(
        label: '思考中',
        icon: Icons.psychology_outlined,
        tone: palette.warning,
        filledDot: false,
      ),
      UiPhase.speaking => UiPhaseView(
        label: '说话中',
        icon: Icons.graphic_eq,
        tone: scheme.primary,
        filledDot: true,
      ),
      UiPhase.interrupted => UiPhaseView(
        label: '已打断',
        icon: Icons.stop_circle_outlined,
        tone: colors.contentMuted,
        filledDot: false,
      ),
      UiPhase.error => UiPhaseView(
        label: '出错',
        icon: Icons.error_outline,
        tone: palette.danger,
        filledDot: false,
      ),
    };

/// 状态胶囊。
class StatePill extends StatefulWidget {
  const StatePill({
    required this.phase,
    this.compact = false,
    this.onTap,
    super.key,
  });

  final UiPhase phase;

  /// 紧凑模式（舞台角标用；聊天面板头用非紧凑）。
  final bool compact;

  /// 点按（`error` 时打开详情）。
  final VoidCallback? onTap;

  @override
  State<StatefulWidget> createState() => _StatePillState();
}

class _StatePillState extends State<StatePill>
    with SingleTickerProviderStateMixin {
  /// 呼吸周期。取值真源在 [AppRhythms]（不是这里的字面量）。
  static const Duration _breath = AppRhythms.thinkingBreath;

  AnimationController? _controller;

  @override
  void initState() {
    super.initState();
    _syncAnimation();
  }

  @override
  void didUpdateWidget(StatePill oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.phase != widget.phase) _syncAnimation();
  }

  /// 只有 `thinking` 需要动画。**其它相位把 controller 停掉并释放**，
  /// 否则「空闲」时也在按 1.4 s 节拍跑一个没人看的动画（白烧 CPU/电量）。
  void _syncAnimation() {
    if (widget.phase == UiPhase.thinking) {
      _controller ??= AnimationController(vsync: this, duration: _breath);
      if (!(_controller!.isAnimating)) {
        _controller!.repeat(reverse: true);
      }
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
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final UiPhaseView view = uiPhaseView(
      widget.phase,
      theme.colorScheme,
      colors,
      appPaletteOf(context),
    );
    final bool reduced = MediaQuery.disableAnimationsOf(context);

    final Widget body = Padding(
      padding: EdgeInsets.symmetric(
        horizontal: widget.compact ? Space.s2 : Space.s3,
        vertical: widget.compact ? 2 : Space.s1,
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (widget.phase == UiPhase.thinking && !reduced)
            // 脉冲环：静态描边 + 呼吸透明度。**不用** CircularProgressIndicator
            // 做装饰（它是 Ticker 驱动的，且语义上表示「进度」不是「状态」）。
            AnimatedBuilder(
              animation: _controller!,
              builder: (BuildContext context, Widget? child) {
                final double t = 0.35 + 0.65 * _controller!.value;
                return Opacity(opacity: t, child: child);
              },
              child: Icon(view.icon, size: widget.compact ? 13 : 15, color: view.tone),
            )
          else
            Icon(view.icon, size: widget.compact ? 13 : 15, color: view.tone),
          SizedBox(width: widget.compact ? 3 : Space.s1),
          // 形通道第二维：实心/空心点。
          Container(
            width: 5,
            height: 5,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              // `null` = 不填充（只用描边表达「静止」）。
              // **不要**写 `Colors.transparent`：那是裸色字面量，
              // 设计令牌门禁会拦（颜色只能来自 ColorScheme 或 AppPalette/AppColors）。
              color: view.filledDot ? view.tone : null,
              border: Border.all(color: view.tone),
            ),
          ),
          const SizedBox(width: Space.s1),
          Text(
            view.label,
            style: (widget.compact
                    ? theme.textTheme.labelSmall
                    : theme.textTheme.labelLarge)
                ?.copyWith(color: view.tone),
          ),
        ],
      ),
    );

    return Semantics(
      label: '状态：${view.label}',
      button: widget.onTap != null,
      excludeSemantics: true,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: view.tone.withValues(alpha: 0.10),
          borderRadius: BorderRadius.circular(AppRadius.pill),
          border: Border.all(color: view.tone.withValues(alpha: 0.40)),
        ),
        child: widget.onTap == null
            ? body
            : InkWell(
                onTap: widget.onTap,
                borderRadius: BorderRadius.circular(AppRadius.pill),
                child: body,
              ),
      ),
    );
  }
}
