/// 「导演」一级卡（2026-10-17，rc4：从「扩展 → 导演」Mod 卡片升上来）。
///
/// # 为什么它是一级、而且没有开关
///
/// 导演在 rc4 已并回核心：**唯一**的按句 cue 写者就是核心自己（[performance].enabled
/// 的 cues 会被抑制，避免第二个写者）。它不再是「可启停的 Mod」——所以这里**没有
/// 启用开关**，只有一句常驻状态；「扩展」分区里也不再出现导演卡片。
///
/// # 卡上有什么（按从上到下）
///
/// 1. **常驻状态卡**：标题 = [kDirectorAlwaysOnTitle]，一句说明；
/// 2. **动作幅度**（[DirectorActionScalesBlock]，2026-10-09 从「外观与互动」搬来，
///    本轮只是换了一级卡这个宿主）：头 / 身体 / 表情三键 + 「本模型覆盖」+
///    「恢复跟随全局」。数据仍是 [action] / [action.models.<id>]，滑条走设置草稿 +
///    底部「保存并重载」；本模型覆盖仍走既有 PATCH。**不写进 mods.json、不新增键、
///    不接回 action_tx**；
/// 3. **导演可观测四栏**（本文件 import 的 director_observer_section.dart）：
///    只在 [devMode] 为真时渲染；
/// 4. **表情调试 / 动作调试 / 临时幅度覆盖**（director_debug_panels.dart）：
///    同样只在 [devMode] 为真时渲染；预设帧仍由前端直发渲染面，不接 action_tx。
///
/// # 删掉了什么（rc4）
///
/// 导演 Mod 的配置表单整体退休：8 个隐藏键（词表映射 + `staging_*`）与
/// `motion_bias` 都**不再有任何界面**——mods.json 里的既有值由宿主忽略（本机配置
/// 不改）。本文件也不复制第二套设置面：滑条 / 覆盖 / 观测 / 调试全部复用原有组件。
library;

import 'package:flutter/material.dart';

import '../../api/settings_models.dart';
import '../../design/tokens.dart';
import '../../live2d/live2d_stage.dart' show isExpressionChannel;
import '../../ui/emphasized_text.dart';
import '../../ui/field_row.dart';
import '../../ui/group_card.dart';
import '../../ui/inline_notice.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';
import '../mods/director_debug_panels.dart';
import '../mods/mod_panel.dart';
import '../preset_labels.dart';
import 'director_observer_section.dart';

/// 常驻状态卡的标题——**无开关**这一句必须在产品面上说清楚。
const String kDirectorAlwaysOnTitle = '导演常驻开启（无开关）';

/// 常驻状态卡的**一句**说明（不解释词表 / 接管 / 分段送 TTS）。
const String kDirectorAlwaysOnNotice = '每一句都按原文给表情与轻微的头、颈动作，不需要启用。';

/// 情绪稳定码 → 中文标签（未知码回落原始码，**不隐藏**）。
///
/// 开发者面的标签表（原 director_panel.dart 的展示纯函数一并搬来）；产品卡不渲染它。
const Map<String, String> kDirectorEmotionLabels = <String, String>{
  'neutral': '中性',
  'happy': '开心',
  'sad': '难过',
  'angry': '生气',
  'surprised': '惊讶',
  'anxious': '焦虑',
  'affectionate': '亲昵',
};

/// 意图稳定码 → 中文标签（未知码回落原始码，**不隐藏**）。
const Map<String, String> kDirectorIntentLabels = <String, String>{
  'chat': '闲聊',
  'question': '提问',
  'greeting': '打招呼',
  'farewell': '告别',
  'request': '请求',
  'complaint': '吐槽',
  'silence': '静默',
};

/// 稳定码 + 中文标签（开心（happy））；标签与码相同时只回码。
String _labeled(String code, Map<String, String> labels) {
  if (code.isEmpty) return '—';
  final String? label = labels[code];
  return label == null ? code : '$label（$code）';
}

/// 情绪字段的展示文案。纯函数，可单测。
String directorEmotionText(Object? value) =>
    _labeled(value is String ? value : '', kDirectorEmotionLabels);

/// 意图字段的展示文案。纯函数，可单测。
String directorIntentText(Object? value) =>
    _labeled(value is String ? value : '', kDirectorIntentLabels);

/// 预设 id → 中文（稳定码）+ 通道；未知 id 原样显示。
///
/// 通道判据（W7 B②）优先查标签表 [labels] 的 channel（唯一真源 =
/// assets/actions/preset_labels.json）；**取不到表 / 该 id 没有 channel 时
/// 才回落** [kExpressionPresetIds]——[isExpressionChannel] 是两处共用的口径。
String directorPresetText(Object? value, {PresetLabelTable? labels}) {
  if (value is! String || value.isEmpty || value == 'none') {
    return '不投递（本轮无动作）';
  }
  final String channel = isExpressionChannel(value, labels: labels)
      ? '表情'
      : '短动作';
  return '$channel（$value）';
}

/// 数值字段的展示文案：整数不拖小数点，其余保留两位。
String directorNumberText(Object? value) {
  if (value is num) {
    final double d = value.toDouble();
    if (d == d.roundToDouble() && d.abs() < 1e15) return d.toInt().toString();
    return d.toStringAsFixed(2);
  }
  return value == null ? '—' : '$value';
}

/// 设置 → 一级「导演」。
class DirectorSection extends StatelessWidget {
  const DirectorSection({
    this.devMode = false,
    this.actionScales,
    this.directorDebug,
    this.observer,
    super.key,
  });

  /// 开发者模式：只决定**可观测 / 调试**两块画不画（常驻状态与滑条照画）。
  final bool devMode;

  /// 动作幅度接线；null = 宿主还没接线（纯 widget 测试）时整块不画。
  final ActionScalesWiring? actionScales;

  /// 调试面板接线（表情调试 / 动作调试 / 临时幅度）；null = 不画那块。
  final DirectorDebugWiring? directorDebug;

  /// 可注入的观测面（测试用；缺省 = 真实的四栏 [DirectorObserverSection]）。
  ///
  /// 为什么要留这个口：真实观测面会在 initState 里读日志端点，纯 widget 测试
  /// 不该发真网络；测试注入一个不联网的替身即可。
  final Widget? observer;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // ① 常驻状态卡：没有开关——导演已在核心里常驻。
        GroupCard(
          title: '导演',
          description: kDirectorAlwaysOnNotice,
          child: const InlineNotice(
            message: kDirectorAlwaysOnTitle,
            severity: NoticeSeverity.info,
            dense: true,
          ),
        ),
        // ② 动作幅度：宿主没接线时不画（不假装有滑条）。
        if (actionScales != null) DirectorActionScalesBlock(wiring: actionScales!),
        // ③ 导演可观测四栏：**只在开发模式里**渲染。
        if (devMode) ...<Widget>[
          const Divider(),
          observer ?? const DirectorObserverSection(),
        ],
        // ④ 表情调试 / 动作调试 / 临时幅度：同样只在开发模式里渲染。
        if (devMode && directorDebug != null) ...<Widget>[
          const Divider(),
          const SectionHeader(title: '手动调试'),
          DebugPanels(
            onApplyPreset: directorDebug!.onApplyPreset,
            status: directorDebug!.status,
            productScales: directorDebug!.productScales,
            pinnedScales: directorDebug!.pinnedScales,
            onApplyScales: directorDebug!.onApplyScales,
            onClearScales: directorDebug!.onClearScales,
            labels: directorDebug!.labels,
            clock: directorDebug!.clock,
          ),
        ],
      ],
    );
  }
}

/// 「动作幅度」块（2026-10-09 从「外观与互动 → 动作幅度」搬来；2026-10-17 随导演
/// 一级卡搬到本文件）。
///
/// 三条滑条 + 「本模型覆盖」开关 + 「恢复跟随全局」。数据仍是
/// [action] / [action.models.<id>]（live2d-ai.toml），仍走既有 PATCH 与
/// ActionScalesSyncer，**没有**写进 mods.json、**没有**接回 action_tx。
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
