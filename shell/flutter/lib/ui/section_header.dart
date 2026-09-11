/// 分区标题 + 分区说明（L3）。
///
/// **唯一**允许在这里写字号/字重的地方（其余组件一律走 `TextTheme` 槽位）。
/// 理由：标题层的字号阶梯如果各处各写，就会长出「一个界面里三种 16px」这种
/// 只有肉眼能发现的失配。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'emphasized_text.dart';
import 'theme.dart';

class SectionHeader extends StatelessWidget {
  const SectionHeader({required this.title, this.description, super.key});

  final String title;

  /// 一句话说明；`null` 时不渲染（不占位、不留空白）。
  final String? description;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(title, style: theme.textTheme.titleMedium),
        if (description != null) ...<Widget>[
          const SizedBox(height: Space.s1),
          // 说明文案里有 `**强调**`（Markdown 习惯）——必须**渲染**它，
          // 否则用户看到的是两个字面的星号。
          EmphasizedText(
            description!,
            style: theme.textTheme.bodySmall?.copyWith(
              color: appColorsOf(context).contentMuted,
            ),
          ),
        ],
      ],
    );
  }
}
