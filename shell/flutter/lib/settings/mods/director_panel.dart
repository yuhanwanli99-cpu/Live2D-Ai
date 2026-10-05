/// `director` 的产品面板（2026-09-15 用户裁决：大幅瘦身）。
///
/// # 这一版只回答一个问题
///
/// **本轮选中了哪条动作预设**（`state_json.latest.preset_id`）——加上
/// 「开启开关」（卡片标题行的 Switch）与「清空决策账本」一个按钮，就是全部。
///
/// # 删掉了什么（都是过时或重复）
///
/// - `kDirectorL1ScopeNotice`（「本轮不做 L1 / 未接任何下行通道」）：早在
///   动作预设落地那一刻就不成立了，留着只会让人以为功能没接；
/// - 逐字段的运行态表（情绪 / 意图 / 建议语速 / 建议音高 / 结项 / 轮次）：
///   与「最近决策（旧到新）」列表重复，且用户要的是**结论**不是中间量；
/// - 最近 N 条列表：`latest` 一条就是本轮结论，历史留在 Rust 侧日志里。
///
/// 通用「运行态（只读）」兜底块不需要再手工关：2026-10-06 起它**只服务没有
/// 专用面板的 Mod**，本面板在册即由自己承担运行态（含 `stateError` 面）；
/// 当初关它的理由照旧成立——十几个键（含 `presets: 8 项`）会把这一屏淹没。
///
/// # 与表演层的关系（2026-09-23 改口径，必读）
///
/// **表演层是主路由，本卡片的 `staging_*` 是回退**（见
/// `docs/architecture/performance-layer-v0.md` §7）：主链 `[performance]` 开着时
/// 本轮 cue 以 performance 为准，host **不**把 `SentenceReady` 转给本 Mod；
/// 只有表演层关或失败时，才由 `staging_*`（规则层）兜底给 cue。
/// 两块都默认关、职责重叠——**不存在两个大脑抢 cue**。
/// 面板顶部用 [kDirectorLegacyNotice] 把这句话原样说出。
///
/// 本文件由 director 轨道独占，其他轨道不要改。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../live2d/live2d_stage.dart' show isExpressionChannel;
import '../../ui/inline_notice.dart';
import '../../ui/theme.dart';
import '../preset_labels.dart';
import 'mod_panel.dart';

/// 面板顶部**必须原样说清**的一句话（一句话说清「启用了会发生什么」）。
const String kDirectorPresetNotice =
    '启用后按情绪触发表情 / 短动作（表情保持约 2.6 秒，短动作约 0.9 秒）；'
    '不改 TTS 输出、不碰手臂/特效。';

/// 与表演层的并存说明（2026-09-22 起，2026-09-23 改口径）——
/// **必须紧跟在 [kDirectorPresetNotice] 之后**。
///
/// 这是「别再以为有两套大脑抢 cue」的唯一出口：**表演层（主链 `[performance]`）
/// 是主路由，本卡片的 `staging_*` 是回退**——表演层开着时本轮 cue 由它决定，
/// host **不**把 `SentenceReady` 转给本 Mod；只有表演层关或失败时，
/// `staging_*` 才按规则层兜底（未配端点则只走规则层）。
const String kDirectorLegacyNotice =
    '表演层是主路由、staging_* 是回退（遗留字段）：'
    '日常请用「设置 → LLM → 表演层」。'
    '表演层开启时本轮 cue 以 performance 为准（本 Mod 收不到 SentenceReady）；'
    '此处 staging_* 仅兼容旧配置，只有表演层关或失败时才由它兜底规则 cue。';

/// 空态指引里必须出现的短语（未启用 / 还没聊过两种空态共用）。
const String kDirectorEmptyHint = '先启用并聊一轮';

/// 情绪稳定码 → 中文标签（未知码回落原始码，**不隐藏**）。
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

/// 稳定码 + 中文标签（`开心（happy）`）；标签与码相同时只回码。
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
/// 通道判据（W7 B②）优先查标签表 [labels] 的 `channel`（唯一真源 =
/// `assets/actions/preset_labels.json`）；**取不到表 / 该 id 没有 channel 时
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

Map<String, Object?> _stringMap(Object? value) {
  final Map<String, Object?> out = <String, Object?>{};
  if (value is Map) {
    value.forEach((dynamic k, dynamic v) {
      if (k is String) out[k] = v;
    });
  }
  return out;
}

/// 导演 Mod 的一等决策面板（瘦身版）。
class DirectorPanel extends ModPanel {
  const DirectorPanel();

  @override
  String get modId => 'director';

  @override
  Map<String, String> get stateLabels => const <String, String>{
    'latest': '最新决策',
    'decisions': '决策数',
  };

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) =>
      _DirectorPanelBody(ctx: ctx);
}

class _DirectorPanelBody extends StatefulWidget {
  const _DirectorPanelBody({required this.ctx});

  final ModPanelContext ctx;

  @override
  State<_DirectorPanelBody> createState() => _DirectorPanelBodyState();
}

class _DirectorPanelBodyState extends State<_DirectorPanelBody> {
  bool _clearing = false;
  String? _message;
  bool _messageIsError = false;

  /// 预设标签表（**显示用**：通道标签优先查它）。
  ///
  /// 面板自己 best-effort 取一次——[ModPanelContext] 不带标签表（宿主只把表
  /// 给了调试面板）。取不到保持空表：回落稳定 id / kExpressionPresetIds 兜底。
  PresetLabelTable _labels = PresetLabelTable.empty;

  @override
  void initState() {
    super.initState();
    unawaited(_loadLabels());
  }

  Future<void> _loadLabels() async {
    final PresetLabelTable table = await fetchPresetLabels();
    if (!mounted || table.length == 0) return;
    setState(() => _labels = table);
  }

  Future<void> _clear() async {
    if (_clearing) return;
    setState(() {
      _clearing = true;
      _message = null;
    });
    try {
      final ModCommandResult result = await widget.ctx.onCommand('clear');
      if (!mounted) return;
      setState(() {
        _clearing = false;
        _messageIsError = !result.ok;
        _message = result.ok
            ? _clearedMessage(result)
            : '清空失败：服务端返回 ok=false';
      });
      if (result.ok) await widget.ctx.onRefreshState();
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _clearing = false;
        _messageIsError = true;
        // 带上错误码：用户要拿界面上的码去日志里搜（项目错误契约）。
        _message = '清空失败：$e';
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _clearing = false;
        _messageIsError = true;
        _message = '清空失败：$e';
      });
    }
  }

  /// 清空回执：**清空前的 counts**（Rust 侧 `command("clear")` 的契约）。
  String _clearedMessage(ModCommandResult result) {
    final Object? cleared = result.result['cleared'];
    final Map<String, Object?> counts = _stringMap(result.result['counts']);
    final Object? before = counts['decisions'];
    return '已清空 ${cleared ?? '?'} 条决策（清空前 decisions=${before ?? '?'}）';
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    final ModPanelContext ctx = widget.ctx;
    final Map<String, Object?>? state = ctx.state;
    final Object? latest = state?['latest'];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s3),
        // 一句话说清「启用后会发生什么」——不再是范围声明。
        const InlineNotice(
          message: kDirectorPresetNotice,
          severity: NoticeSeverity.info,
          dense: true,
        ),
        const SizedBox(height: Space.s2),
        // 紧跟着说清「表演层主路由 / staging 回退」——避免用户以为有两套大脑抢 cue。
        const InlineNotice(
          message: kDirectorLegacyNotice,
          severity: NoticeSeverity.info,
          dense: true,
        ),
        if (ctx.stateError != null)
          Padding(
            padding: const EdgeInsets.only(top: Space.s2),
            child: InlineNotice(message: ctx.stateError!, dense: true),
          ),
        if (ctx.stateLoading)
          Padding(
            padding: const EdgeInsets.only(top: Space.s2),
            child: Text('读取中…', style: muted),
          )
        else if (state == null)
          _hint(muted, '运行态暂时读不到：展开卡片会自动重取，也可以点上面的「刷新运行态」。')
        else
          ..._content(theme, muted, ctx, state, latest),
        ..._clearBlock(theme, ctx, muted),
      ],
    );
  }

  /// 空态与内容的分派：**空白面板会被读成「坏了」**。
  List<Widget> _content(
    ThemeData theme,
    TextStyle? muted,
    ModPanelContext ctx,
    Map<String, Object?> state,
    Object? latest,
  ) {
    if (!ctx.enabled) {
      return <Widget>[
        _hint(muted, '导演未启用：$kDirectorEmptyHint，然后回来查看本轮选中的预设。'),
      ];
    }
    final int decisions = state['decisions'] is num
        ? (state['decisions']! as num).toInt()
        : 0;
    if (decisions == 0 || latest == null) {
      return <Widget>[
        _hint(muted, '还没有决策：$kDirectorEmptyHint（每完成一轮对话，这里就会多一条）。'),
      ];
    }
    final Map<String, Object?> row = _stringMap(latest);
    final String preset = row['preset_id'] is String
        ? row['preset_id']! as String
        : '';
    return <Widget>[
      const SizedBox(height: Space.s3),
      Text('本轮预设', style: theme.textTheme.labelLarge),
      Padding(
        padding: const EdgeInsets.only(top: OpticalNudge.thin),
        child: Text(
          directorPresetText(preset, labels: _labels),
          style: theme.textTheme.titleSmall,
        ),
      ),
      // 情绪 / 意图只作一行注脚（它们解释「为什么选这条」），不做成表格。
      Padding(
        padding: const EdgeInsets.only(top: OpticalNudge.thin),
        child: Text(
          '依据：${directorEmotionText(row['emotion'])} · '
          '${directorIntentText(row['intent'])} · 已决策 $decisions 轮',
          style: muted,
        ),
      ),
    ];
  }

  List<Widget> _clearBlock(
    ThemeData theme,
    ModPanelContext ctx,
    TextStyle? muted,
  ) {
    return <Widget>[
      const SizedBox(height: Space.s3),
      Align(
        alignment: Alignment.centerLeft,
        child: OutlinedButton.icon(
          // 未启用时命令必回 503 command_unavailable——按钮直接禁用，不假装能点。
          onPressed: ctx.enabled && !_clearing
              ? () => unawaited(_clear())
              : null,
          icon: const Icon(Icons.delete_outline, size: 16),
          label: Text(_clearing ? '清空中…' : '清空决策账本'),
        ),
      ),
      if (!ctx.enabled)
        Padding(
          padding: const EdgeInsets.only(top: Space.s1),
          child: Text(
            '未启用时命令不可用（服务端回 503 command_unavailable）。',
            style: muted,
          ),
        ),
      if (_message != null)
        Padding(
          padding: const EdgeInsets.only(top: Space.s2),
          child: InlineNotice(
            message: _message!,
            severity: _messageIsError
                ? NoticeSeverity.danger
                : NoticeSeverity.info,
            dense: true,
          ),
        ),
    ];
  }

  Widget _hint(TextStyle? muted, String text) => Padding(
    padding: const EdgeInsets.only(top: Space.s2),
    child: Text(text, style: muted),
  );
}
