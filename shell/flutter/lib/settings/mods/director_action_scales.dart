part of 'director_panel.dart';

/// 「动作幅度」块（2026-10-09 从「外观与互动 → 动作幅度」**逐字搬来**）。
///
/// 三条滑条 + 「本模型覆盖」开关 + 「恢复跟随全局」。数据仍是
/// [action] / [action.models.<id>]（live2d-ai.toml），仍走既有 PATCH 与
/// ActionScalesSyncer，**没有**写进 director 的 mods.json、**没有**接回
/// action_tx（见 ActionScalesWiring 头注）。
///
/// 覆盖开启时三条滑条编辑的是「逐键 override[key] ?? global[key]」的有效值，
/// 回调直接 PATCH；关闭时就是原来的全局值 + 草稿回调（旧语义一字不动）。
class DirectorActionScalesBlock extends StatelessWidget {
  const DirectorActionScalesBlock({required this.wiring, super.key});

  final ActionScalesWiring wiring;

  @override
  Widget build(BuildContext context) {
    final ActionSettingsView actionView = wiring.action;
    final bool modelOverrideOn =
        wiring.modelOverrideEnabled && actionView.activeModelId.isNotEmpty;
    final ActionSettingsView effective = actionView.effectiveForActiveModel;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s3),
        const SectionHeader(title: '动作幅度'),
        // 本模型覆盖块：模型名 + 开关 + 覆盖 PATCH 的结果。
        _ModelOverrideHeader(
          activeModelId: actionView.activeModelId,
          enabled: wiring.modelOverrideEnabled,
          onEnabledChanged: wiring.onModelOverrideEnabledChanged,
          message: wiring.modelOverrideMessage,
          failed: wiring.modelOverrideFailed,
        ),
        // 三条滑条：**不是两套**——关闭态的语义与改动前逐字一致。
        SliderField(
          label: '头部摆幅',
          icon: Icons.face_retouching_natural,
          value: modelOverrideOn ? effective.headScale : actionView.headScale,
          min: ActionSettingsView.minScale,
          max: ActionSettingsView.maxScale,
          divisions: 46,
          enabled: modelOverrideOn
              ? wiring.onModelHeadScaleChanged != null
              : wiring.onHeadScaleChanged != null,
          onChanged: modelOverrideOn
              ? (wiring.onModelHeadScaleChanged ?? (double _) {})
              : (wiring.onHeadScaleChanged ?? (double _) {}),
          // 数字项只留一行范围（数从控件现有 min/max 抄）。
          description: '0.2-2.2',
        ),
        SliderField(
          label: '身体摆幅',
          icon: Icons.accessibility_new,
          value: modelOverrideOn ? effective.bodyScale : actionView.bodyScale,
          min: ActionSettingsView.minScale,
          max: ActionSettingsView.maxScale,
          divisions: 46,
          enabled: modelOverrideOn
              ? wiring.onModelBodyScaleChanged != null
              : wiring.onBodyScaleChanged != null,
          onChanged: modelOverrideOn
              ? (wiring.onModelBodyScaleChanged ?? (double _) {})
              : (wiring.onBodyScaleChanged ?? (double _) {}),
          description: '0.2-2.2',
        ),
        SliderField(
          label: '表情幅度',
          icon: Icons.mood,
          value: modelOverrideOn
              ? effective.expressionScale
              : actionView.expressionScale,
          min: ActionSettingsView.minScale,
          max: ActionSettingsView.maxScale,
          divisions: 46,
          enabled: modelOverrideOn
              ? wiring.onModelExpressionScaleChanged != null
              : wiring.onExpressionScaleChanged != null,
          onChanged: modelOverrideOn
              ? (wiring.onModelExpressionScaleChanged ?? (double _) {})
              : (wiring.onExpressionScaleChanged ?? (double _) {}),
          description: '0.2-2.2',
        ),
        // 「恢复跟随全局」只在覆盖开启时可点（没有覆盖时禁用，不是点了没反应）。
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton(
            onPressed: modelOverrideOn ? wiring.onResetModelOverride : null,
            child: const Text('恢复跟随全局'),
          ),
        ),
      ],
    );
  }
}

/// 「本模型覆盖」子块（阶段5 D40，2026-09-26）：模型名 + 开关 + 结果。
///
/// 三条理由写在这里，避免下一个人再按「第二个全局滑条」改错：
/// 1. 开关的**值**由宿主给（= 本模型是否已有覆盖）；这里不持有状态，
///    所以「开 / 关」不会与磁盘分叉；
/// 2. active_model_id 为空时**如实说「未识别当前模型」并禁用开关**——
///    假装能开会在写回时被服务端拒（模型 id 非法）；
/// 3. 覆盖改走**直接 PATCH**（不需要「保存」），所以这里只上报，不碰草稿。
class _ModelOverrideHeader extends StatelessWidget {
  const _ModelOverrideHeader({
    required this.activeModelId,
    required this.enabled,
    required this.onEnabledChanged,
    required this.message,
    required this.failed,
  });

  final String activeModelId;
  final bool enabled;
  final ValueChanged<bool>? onEnabledChanged;
  final String? message;
  final bool failed;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final AppPalette palette = appPaletteOf(context);
    final bool known = activeModelId.isNotEmpty;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          known ? '当前模型：$activeModelId' : '未识别当前模型',
          style: theme.textTheme.labelLarge,
        ),
        const SizedBox(height: Space.s1),
        ToggleField(
          label: '本模型覆盖',
          icon: Icons.tune,
          value: enabled,
          enabled: known && onEnabledChanged != null,
          onChanged: onEnabledChanged ?? (bool _) {},
        ),
        if (message != null)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: EmphasizedText(
              message!,
              style: theme.textTheme.labelSmall?.copyWith(
                color: failed ? palette.warning : colors.contentMuted,
              ),
            ),
          ),
      ],
    );
  }
}
