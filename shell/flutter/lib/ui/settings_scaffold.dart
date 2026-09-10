/// 设置宿主骨架（L3）：分区导航 + 内容槽 + 未保存提示条。
///
/// **同一个 widget 服务三种宿主**（内联侧板 / medium 浮层 / compact 浮层）——
/// 宿主只决定「放在哪」，不决定「长什么样」（规格 §4.2）。
///
/// 草稿生命周期（`SettingsController`：dirty / 拦截 / 保存-放弃）是 **P4** 的
/// 范围；本文件先给出稳定的骨架与导航，P4 只往内容槽里填 8 个 pane。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import '../settings/settings_sections.dart';
import 'section_header.dart';
import 'theme.dart';

class SettingsScaffold extends StatelessWidget {
  const SettingsScaffold({
    required this.sections,
    required this.selected,
    required this.onSelect,
    required this.child,
    this.leading,
    this.trailing,
    this.onClose,
    this.dirty = false,
    this.saving = false,
    this.onSave,
    this.onDiscard,
    this.statusMessage,
    this.statusIsError = false,
    super.key,
  });

  /// 可见分区（已按 `dev_mode` 过滤）。
  final List<SettingsSection> sections;

  final SettingsSection selected;
  final ValueChanged<SettingsSection> onSelect;

  /// 内容。
  final Widget child;

  /// 标题左侧（返回/关闭）。
  final Widget? leading;

  /// 标题右侧（保存/放弃，P4 用）。
  final Widget? trailing;

  /// 关闭浮层。
  final VoidCallback? onClose;

  /// 有没有未保存的改动。
  final bool dirty;

  /// 是否正在保存。
  final bool saving;
  final VoidCallback? onSave;
  final VoidCallback? onDiscard;

  /// 上次保存的结果文案（**按 `apply_status` 分流**，见 §4.4）。
  final String? statusMessage;
  final bool statusIsError;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);

    // 固定 Tab 顺序：不写的话顺序跟随 Widget 树，重构时会静默改变。
    return FocusTraversalGroup(
      policy: OrderedTraversalPolicy(),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          // ── 标题栏：分区标题 + 分区说明（**唯一**写字号字重的地方之一） ──
          Padding(
            padding: const EdgeInsets.fromLTRB(
              Space.s3,
              Space.s3,
              Space.s2,
              Space.s2,
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                if (leading != null) ...<Widget>[
                  leading!,
                  const SizedBox(width: Space.s1),
                ],
                Expanded(
                  child: SectionHeader(
                    title: selected.label,
                    description: selected.description,
                  ),
                ),
                // **未保存必须有可见提示**（规格 §4.3 的硬要求）：
                // 现状完全缺失，用户改完就切走、改动无声消失。
                if (dirty)
                  Padding(
                    padding: const EdgeInsets.only(right: Space.s1),
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        color: AppPalette.warning.withValues(alpha: 0.18),
                        borderRadius: BorderRadius.circular(AppRadius.pill),
                        border: Border.all(
                          color: AppPalette.warning.withValues(alpha: 0.5),
                        ),
                      ),
                      child: Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: Space.s2,
                          vertical: 2,
                        ),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: <Widget>[
                            const Icon(
                              Icons.circle,
                              size: 7,
                              color: AppPalette.warning,
                            ),
                            const SizedBox(width: Space.s1),
                            Text(
                              '未保存',
                              style: theme.textTheme.labelSmall?.copyWith(
                                color: AppPalette.warning,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ?trailing,
                if (onClose != null)
                  IconButton(
                    onPressed: onClose,
                    tooltip: '关闭设置',
                    icon: const Icon(Icons.close),
                  ),
              ],
            ),
          ),
          // ── 分区导航：宽用 SegmentedButton 换行（不挤占内容宽度），
          //    窄也用同一套（**不**再造第二套导航）。 ──
          Padding(
            padding: const EdgeInsets.fromLTRB(Space.s3, 0, Space.s3, Space.s2),
            child: Wrap(
              spacing: Space.s1,
              runSpacing: Space.s1,
              children: <Widget>[
                for (final SettingsSection section in sections)
                  ChoiceChip(
                    avatar: Icon(section.icon, size: 16),
                    label: Text(section.label),
                    selected: section == selected,
                    onSelected: (_) => onSelect(section),
                  ),
              ],
            ),
          ),
          Divider(height: 1, color: colors.hairline),
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(
                Space.s3,
                Space.s3,
                Space.s3,
                Space.s5,
              ),
              child: child,
            ),
          ),
          // ── 保存 / 放弃操作条（**只在有改动或刚有结果时出现**，
          //    常驻一条空操作条是纯噪声） ──
          if (dirty || statusMessage != null)
            DecoratedBox(
              decoration: BoxDecoration(
                border: Border(top: BorderSide(color: colors.hairline)),
              ),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(
                  Space.s3,
                  Space.s2,
                  Space.s3,
                  Space.s2,
                ),
                child: Row(
                  children: <Widget>[
                    if (statusMessage != null)
                      Expanded(
                        child: Text(
                          statusMessage!,
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: statusIsError
                                ? AppPalette.danger
                                : colors.contentMuted,
                          ),
                        ),
                      )
                    else
                      const Spacer(),
                    if (dirty && onDiscard != null)
                      TextButton(
                        onPressed: saving ? null : onDiscard,
                        child: const Text('放弃'),
                      ),
                    if (dirty && onSave != null) ...<Widget>[
                      const SizedBox(width: Space.s2),
                      FilledButton(
                        onPressed: saving ? null : onSave,
                        child: Text(saving ? '保存中…' : '保存'),
                      ),
                    ],
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// 尚未落地的分区占位。
///
/// **如实说明「还没有」比给一个看起来能用但点了没反应的面板好**——
/// 后者会让用户以为是坏了（同类项目里最一致的缺陷就是「静默失效」）。
class SettingsPendingPane extends StatelessWidget {
  const SettingsPendingPane({required this.section, super.key});

  final SettingsSection section;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: Space.s5),
      child: Column(
        children: <Widget>[
          Icon(section.icon, size: 28, color: colors.contentFaint),
          const SizedBox(height: Space.s2),
          Text('「${section.label}」还没接上', style: theme.textTheme.titleSmall),
          const SizedBox(height: Space.s1),
          Text(
            '这一分区的内容仍在施工中。已经可用的是「动作与互动」。',
            textAlign: TextAlign.center,
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.contentMuted,
            ),
          ),
        ],
      ),
    );
  }
}
