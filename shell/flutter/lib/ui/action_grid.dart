/// 动作试演网格（L3）：6 个动作 × 强度 SEG（规格 §4.1.5 的 B 组）。
///
/// # 三条来自调研的约束
///
/// 1. **动作按钮由 `capabilities.actions` 驱动显隐/置灰**，不硬编码 6 个
///    （后端加动作时前端不该静默少显示）。
/// 2. **强度用分段按钮（SEG），不是滑杆**——1/2/3 是三个离散档位，
///    滑杆会暗示「2.5 也有意义」。
/// 3. **屏幕槽位按 8 设计**（VRChat「up to 8 controls per menu」与 VTS
///    `onScreenButtonID` 1–8 双源收敛），先放 6 个，留 2 个给未来的 Mod 动作。
///
/// 「正在演」的忙碌态由 WS 的 `action_state` 驱动（`ActionDispatch`），
/// **不是**由 HTTP 响应驱动——响应看不出 `Dropped(AlreadyActive)`。
library;

import 'package:flutter/material.dart';

import '../actions/action_dispatch.dart';
import '../api/commands_api.dart';
import '../design/tokens.dart';
import 'theme.dart';

/// 屏幕槽位上限（**按 8 设计**，见头注）。
const int kActionSlotLimit = 8;

/// 高频动作（常驻工具条先放这三个）。
///
/// 取值依据：这三个是「对话里最常被触发的社交动作」——点头/摇头是应答，
/// 惊讶是情绪反馈。其余三个（歪头/环视/倾听）放面板网格。
const List<String> kHighFrequencyActions = <String>['nod', 'shake_no', 'surprise'];

class ActionGrid extends StatelessWidget {
  const ActionGrid({
    required this.commands,
    required this.strength,
    required this.onStrengthChanged,
    required this.onInvoke,
    required this.dispatch,
    this.columns = 3,
    super.key,
  });

  /// 可触发的动作（已按 `allowed_sources` 过滤）。
  final List<CommandInfo> commands;

  /// 当前强度档位（1..3），**全局一份**：用户选一次，点哪个动作都用它。
  final int strength;
  final ValueChanged<int> onStrengthChanged;

  /// 触发（`id` + 强度）。
  final void Function(CommandInfo command) onInvoke;

  /// 忙碌态与重复点击解释。
  final ActionDispatch dispatch;

  final int columns;

  @override
  Widget build(BuildContext context) {
    if (commands.isEmpty) {
      return const _NoActions();
    }
    // **组件自己订阅** `dispatch`：忙碌态变了要重绘，这是组件的职责。
    // 让每个调用方都记得包一层 `ListenableBuilder` 是不可靠的——
    // 漏一个地方的表现就是「点了动作按钮不变忙碌态」，而且只在真机上才看得出来。
    return ListenableBuilder(
      listenable: dispatch,
      builder: (BuildContext context, Widget? _) => _buildBody(context),
    );
  }

  Widget _buildBody(BuildContext context) {
    // 槽位上限：多出来的动作**不静默丢弃**，如实说明。
    final List<CommandInfo> shown = commands.take(kActionSlotLimit).toList();
    final int hidden = commands.length - shown.length;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _StrengthPicker(
          strength: strength,
          onChanged: onStrengthChanged,
          duplicateHint: dispatch.duplicateHint,
          showDuplicateHint: dispatch.lastError == null && dispatch.lastInvoke != null,
        ),
        if (dispatch.lastError != null)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s2),
            child: Text(
              '⚠ ${dispatch.lastError}',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: AppPalette.danger,
              ),
            ),
          ),
        GridView.count(
          crossAxisCount: columns,
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          mainAxisSpacing: Space.s2,
          crossAxisSpacing: Space.s2,
          childAspectRatio: 1.6,
          children: <Widget>[
            for (final CommandInfo c in shown)
              _ActionCard(
                command: c,
                busy: c.targetAction != null && dispatch.isBusy(c.targetAction!),
                onPressed: () => onInvoke(c),
              ),
          ],
        ),
        if (hidden > 0)
          Padding(
            padding: const EdgeInsets.only(top: Space.s2),
            child: Text(
              '另有 $hidden 个动作未显示（屏幕槽位上限 $kActionSlotLimit）',
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                color: appColorsOf(context).contentMuted,
              ),
            ),
          ),
      ],
    );
  }
}

class _StrengthPicker extends StatelessWidget {
  const _StrengthPicker({
    required this.strength,
    required this.onChanged,
    required this.duplicateHint,
    required this.showDuplicateHint,
  });

  final int strength;
  final ValueChanged<int> onChanged;
  final String duplicateHint;
  final bool showDuplicateHint;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s2),
      child: Row(
        children: <Widget>[
          Text('强度', style: Theme.of(context).textTheme.labelLarge),
          const SizedBox(width: Space.s2),
          Semantics(
            label: '动作强度',
            child: SegmentedButton<int>(
              segments: const <ButtonSegment<int>>[
                ButtonSegment<int>(value: 1, label: Text('轻')),
                ButtonSegment<int>(value: 2, label: Text('中')),
                ButtonSegment<int>(value: 3, label: Text('强')),
              ],
              selected: <int>{strength},
              showSelectedIcon: false,
              onSelectionChanged: (Set<int> next) => onChanged(next.first),
            ),
          ),
          const Spacer(),
          if (showDuplicateHint)
            Tooltip(
              message: duplicateHint,
              child: Icon(
                Icons.info_outline,
                size: 16,
                color: appColorsOf(context).contentMuted,
              ),
            ),
        ],
      ),
    );
  }
}

class _ActionCard extends StatelessWidget {
  const _ActionCard({
    required this.command,
    required this.busy,
    required this.onPressed,
  });

  final CommandInfo command;
  final bool busy;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final String label = command.name.isEmpty
        ? (command.targetAction ?? command.id)
        : command.name;

    return Semantics(
      // 读屏要念出「正在演」，而不只是按钮名。
      label: busy ? '$label，正在演' : label,
      button: true,
      excludeSemantics: true,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: busy
              ? theme.colorScheme.primaryContainer
              : theme.colorScheme.surfaceContainerHighest,
          borderRadius: BorderRadius.circular(AppRadius.md),
          border: Border.all(
            color: busy ? theme.colorScheme.primary : colors.hairline,
          ),
        ),
        child: InkWell(
          onTap: onPressed,
          borderRadius: BorderRadius.circular(AppRadius.md),
          child: Padding(
            padding: const EdgeInsets.all(Space.s2),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: <Widget>[
                if (busy)
                  SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: theme.colorScheme.onPrimaryContainer,
                    ),
                  )
                else
                  Icon(
                    Icons.play_arrow_outlined,
                    size: 16,
                    color: colors.contentMuted,
                  ),
                const SizedBox(height: Space.s1),
                Text(
                  label,
                  style: theme.textTheme.labelLarge,
                  textAlign: TextAlign.center,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                if (busy)
                  Text(
                    '正在演',
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: theme.colorScheme.onPrimaryContainer,
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _NoActions extends StatelessWidget {
  const _NoActions();

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(vertical: Space.s4),
    child: Column(
      children: <Widget>[
        Icon(
          Icons.touch_app_outlined,
          size: 24,
          color: appColorsOf(context).contentFaint,
        ),
        const SizedBox(height: Space.s2),
        Text(
          '当前没有可手动触发的动作',
          style: Theme.of(context).textTheme.bodySmall,
        ),
        const SizedBox(height: Space.s1),
        Text(
          '动作清单由服务端 capabilities 提供；'
          '若模型没有动作能力，这里会一直是空的。',
          textAlign: TextAlign.center,
          style: Theme.of(context).textTheme.labelSmall?.copyWith(
            color: appColorsOf(context).contentMuted,
          ),
        ),
      ],
    ),
  );
}
