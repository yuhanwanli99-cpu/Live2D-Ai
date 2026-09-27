/// 设置分区里的**组卡片**：标题 + 一行说明 + 内容。
///
/// # 为什么要有这一层（2026-09-27）
///
/// 改之前的设置区是一条**扁平长列表**：「外观 / 舞台与口型 / 互动」三组之间
/// 只靠 `Divider` + `SectionHeader` 区分，字段一多就变成一堵表单墙——
/// 用户看不出「哪些控件是一件事」。
///
/// 换成卡片之后：
///
/// - **组有了实体**：一组控件住在一张面里，边界由面差 + 描边表达，
///   不再需要靠一条线去「暗示」分组；
/// - 层级有了承重：r2 引入的第三级面 [AppPalette.raised] 在这里第一次
///   有了真正的作用——「浮起」= 一张卡，而不是整页都是同一种面；
/// - 圆角 / 描边 / 阴影全部来自令牌，**新组件不新增任何裸值**。
///
/// # 行数豁免（≤1000）
///
/// 本文件是「一个组件 + 一段头注」。拆成 `group_card/` 目录只会把
/// 「组卡片长什么样」这条约定摊到多个文件，而那正是本项目 P4 说的
/// 静默失效的来源。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'emphasized_text.dart';
import 'theme.dart';

/// 一组设置。
class GroupCard extends StatelessWidget {
  const GroupCard({
    required this.title,
    required this.child,
    this.description,
    this.trailing,
    super.key,
  });

  /// 组标题（用 `titleSmall`：比正文重一档，但不到 `titleMedium` 抢戏）。
  final String title;

  /// 一行说明，**只写这一组是干什么的**（不解释每个字段——那是字段自己的
  /// `description` 的活）。
  ///
  /// `null` 时不占任何高度。
  final String? description;

  /// 标题行右侧的可选控件（例：整组的开关）。
  final Widget? trailing;

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppPalette palette = appPaletteOf(context);
    final AppColors colors = appColorsOf(context);
    final BorderRadius radius = BorderRadius.circular(
      colors.radius(AppRadius.lg),
    );
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s3),
      child: DecoratedBox(
        decoration: BoxDecoration(
          // 组卡片也是「面板」，所以跟聊天面板吃同一个 `panelAlpha`。
          color: palette.raised.withValues(alpha: colors.panelAlpha),
          borderRadius: radius,
          border: Border.all(color: colors.hairline),
          // 阴影给的是**卡片浮起来**的那一点，不是「浮在页面上」；
          // 形状与强度都在 `design/tokens.dart` 的 `appRaisedShadow` 里，
          // 本组件不自带任何数值。
          boxShadow: appRaisedShadow(palette),
        ),
        child: ClipRRect(
          borderRadius: radius,
          child: Padding(
            padding: const EdgeInsets.all(Space.s3),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          Text(title, style: theme.textTheme.titleSmall),
                          if (description != null) ...<Widget>[
                            const SizedBox(height: OpticalNudge.thin),
                            // **必须走 `EmphasizedText`**：分组说明里会写
                            // 「**只影响本机显示**」这种加粗强调，用裸 `Text`
                            // 会把星号原样画出来（rc.3 修过 15 处同一个病，
                            // 这里别再添一处）。
                            EmphasizedText(
                              description!,
                              style: theme.textTheme.labelSmall?.copyWith(
                                color: colors.contentMuted,
                              ),
                            ),
                          ],
                        ],
                      ),
                    ),
                    if (trailing != null) ...<Widget>[
                      const SizedBox(width: Space.s2),
                      trailing!,
                    ],
                  ],
                ),
                const SizedBox(height: Space.s2),
                child,
              ],
            ),
          ),
        ),
      ),
    );
  }
}
