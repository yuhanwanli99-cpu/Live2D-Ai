/// 动作来源徽标（L3，允许 import material）。
///
/// 规格 §8.3 的差异化点：core 早已下发 `source`（`user 100 > llm 90 > rule 50`
/// 的仲裁依据），但界面上曾经完全看不见。这里把它变成一等公民信号。
///
/// # 三通道（满足原则 P3「色 + 形 + 字 + 节奏里至少占两个」）
///
/// | 来源 | 色 | 形 | 字 |
/// |---|---|---|---|
/// | 用户 | `primaryContainer` | `touch_app_outlined` | 你 |
/// | AI（LLM 工具） | `tertiaryContainer` | `auto_awesome_outlined` | AI |
/// | 规则兜底 | `surfaceContainerHighest` | `rule_outlined` | 规则 |
/// | 未知 | `surfaceContainerHighest` | `help_outline` | 未知 |
///
/// 只靠颜色是不够的：3 个开源项目都栽在「状态只有颜色」。这里每个来源都同时
/// 有图标与文字，色盲/灰度截图下仍然可读。
///
/// **`tertiary` 是全规格唯一允许使用它的地方**（裁决 G）：来源区分需要「不同
/// 色相 + 小图标」；而 `tertiary` 是 M3 `fromSeed` 派生出来的槽位，不是新造色，
/// 所以不违反原则 P2 的「一个品牌色」。
library;

import 'package:flutter/material.dart';

import '../api/action_source.dart';
import '../design/tokens.dart';

/// 一个来源的完整视觉表达（映射**只在**这里定义一次）。
@immutable
class ActionSourceView {
  const ActionSourceView({
    required this.label,
    required this.icon,
    required this.tone,
    required this.onTone,
  });

  /// 文字通道。
  final String label;

  /// 形状通道。
  final IconData icon;

  /// 底色。
  final Color tone;

  /// 压在上面的前景色。
  final Color onTone;
}

/// `ActionSource` → 视觉表达。**纯函数**（可在 VM / widget 测试里直接断言）。
ActionSourceView actionSourceView(ActionSource source, ColorScheme scheme) =>
    switch (source) {
      ActionSource.userCommand => ActionSourceView(
        label: '你',
        icon: Icons.touch_app_outlined,
        tone: scheme.primaryContainer,
        onTone: scheme.onPrimaryContainer,
      ),
      // tertiary 的唯一合法使用点（裁决 G）。
      ActionSource.llmTool => ActionSourceView(
        label: 'AI',
        icon: Icons.auto_awesome_outlined,
        tone: scheme.tertiaryContainer,
        onTone: scheme.onTertiaryContainer,
      ),
      ActionSource.ruleFallback => ActionSourceView(
        label: '规则',
        icon: Icons.rule_outlined,
        tone: scheme.surfaceContainerHighest,
        onTone: scheme.onSurfaceVariant,
      ),
      // **未知绝不归到 AI 一侧**：协议一变就会把用户的动作标成 AI 做的
      // （`action_source_test.dart` 已把这条钉死）。
      ActionSource.unknown => ActionSourceView(
        label: '未知',
        icon: Icons.help_outline,
        tone: scheme.surfaceContainerHighest,
        onTone: scheme.onSurfaceVariant,
      ),
    };

/// 来源徽标。
class ActionSourceBadge extends StatelessWidget {
  const ActionSourceBadge({required this.source, this.dense = false, super.key});

  final ActionSource source;

  /// 紧凑模式（历史列表里用；动作卡片上用非紧凑）。
  final bool dense;

  @override
  Widget build(BuildContext context) {
    final ColorScheme scheme = Theme.of(context).colorScheme;
    final ActionSourceView view = actionSourceView(source, scheme);
    final double iconSize = dense ? 12 : 14;

    return Semantics(
      // 读屏不要念成「你 图标」这种碎片——给一句完整的。
      label: '来源：${view.label}',
      excludeSemantics: true,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: view.tone,
          borderRadius: BorderRadius.circular(AppRadius.pill),
        ),
        child: Padding(
          padding: EdgeInsets.symmetric(
            horizontal: dense ? Space.s2 : Space.s3,
            vertical: dense ? 1 : Space.s1,
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Icon(view.icon, size: iconSize, color: view.onTone),
              SizedBox(width: dense ? 2 : Space.s1),
              Text(
                view.label,
                style: (dense
                        ? Theme.of(context).textTheme.labelSmall
                        : Theme.of(context).textTheme.labelLarge)
                    ?.copyWith(color: view.onTone),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
