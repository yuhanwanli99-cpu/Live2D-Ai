/// director 的产品面板（2026-10-09：接住动作幅度 + 导演可观测两块）。
///
/// # 卡片上有什么（2026-10-08 再瘦身之后的形态，本轮只加两块）
///
/// 1. 卡片标题行的启用开关（ModsSection 自己画）+ 一句人话
///    [kDirectorTakeoverNotice]；
/// 2. **动作幅度**（头 / 身体 / 表情三键 + 「本模型覆盖」+「恢复跟随全局」）
///    —— 2026-10-09 从「外观与互动」搬来：手势 / 表情 / 头部动作不是单 LLM
///    主链的资产，强度旋钮跟**产出这些动作的导演 Mod** 放在一起。
///    数据仍是 [action] / [action.models.<id>]，仍走既有 PATCH 与
///    ActionScalesSyncer；**不写进 director 的 mods.json、不接回 action_tx**。
/// 3. **导演可观测四栏**（原「开发模式 → 导演可观测」，2026-10-09 搬到这里
///    作为下级块）：devMode 为假时整块不渲染（组件自己也短路一次）。
///
/// # 拿掉了什么（2026-10-08，本轮不变）
///
/// 词表映射、二路 LLM 开关与地址、接管状态、清空决策账本——8 个配置键经
/// [hiddenKeys] 从**产品界面**整体消失（也不收进「高级」），但仍被解析、
/// 仍可写在 mods.json 里；保存表单时原样不写回。
///
/// 本文件由 director 轨道独占，其他轨道不要改。
library;

import 'package:flutter/material.dart';

import '../../api/settings_models.dart';
import '../../design/tokens.dart';
import '../../live2d/live2d_stage.dart' show isExpressionChannel;
import '../../ui/emphasized_text.dart';
import '../../ui/field_row.dart';
import '../../ui/inline_notice.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';
import '../preset_labels.dart';
import '../sections/director_observer_section.dart';
import 'mod_panel.dart';

part 'director_action_scales.dart';

/// 面板上**唯一**的一句话（紧跟在启用开关后面）。
///
/// 用户只需要知道「打开后会发生什么」，不需要知道词表、接管、分段送 TTS。
const String kDirectorTakeoverNotice = '打开后，说话时会带上表情和轻微的头、颈动作。';

/// 情绪稳定码 → 中文标签（未知码回落原始码，**不隐藏**）。
///
/// 开发者面（「导演可观测」A 栏 / 词表记录）用它，产品面板不用。
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

/// 导演 Mod 的产品面板。
class DirectorPanel extends ModPanel {
  const DirectorPanel();

  @override
  String get modId => 'director';

  /// 8 个配置键：产品界面不渲染（也不收进「高级」）。
  ///
  /// 与 crates/live2d-ai-mod-director/src/lib.rs 的 director_settings_spec()
  /// **逐键对应**——那张 spec 仍是解析真源，这里只负责「不画」。
  @override
  Set<String> get hiddenKeys => const <String>{
    'preset_happy',
    'preset_sad',
    'preset_greeting',
    'staging_enabled',
    'staging_base_url',
    'staging_model',
    'staging_api_key_env',
    'staging_timeout_ms',
  };

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) {
    final ActionScalesWiring? scales = ctx.actionScales;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const Padding(
          padding: EdgeInsets.only(top: Space.s3),
          child: InlineNotice(
            message: kDirectorTakeoverNotice,
            severity: NoticeSeverity.info,
            dense: true,
          ),
        ),
        // 动作幅度：宿主没接线（widget 测试 / 还没拿到设置）时整块不画。
        if (scales != null) DirectorActionScalesBlock(wiring: scales),
        // 导演可观测四栏：**只在开发模式里**渲染（组件自己也短路一次）。
        if (ctx.devMode) const DirectorObserverSection(),
      ],
    );
  }
}
