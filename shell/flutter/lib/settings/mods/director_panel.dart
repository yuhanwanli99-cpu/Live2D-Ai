/// `director` 的产品面板（产品级加强波次）。
///
/// 职责：把 `state_json` 里的 `latest`（最近一条决策）与 `recent_decisions`
/// （最近 N 条）做成一等面板——带标签的行 + 旧到新的列表，不再是裸 JSON；
/// 空态给「先启用并聊一轮」的指引，而不是空白。
///
/// **零投递语义一行不变**：面板只展示，`delivered` 恒 `false`、`channel` 恒
/// `"none"`，顶部用文字说明「仅建议、不驱动动作、不影响 TTS 输出」。
/// 本文件由 director 轨道独占，其他轨道不要改。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../ui/inline_notice.dart';
import '../../ui/theme.dart';
import 'mod_panel.dart';

/// 面板顶部**必须原样说清**的一句话：只建议，没有任何下行通道。
///
/// 与 Rust 侧状态面同源：`delivered` 恒 `false`、`channel` 恒 `"none"`
/// （见 `crates/live2d-ai-mod-director/src/lib.rs` 头注）。
const String kDirectorZeroDeliveryNotice =
    '仅建议：不驱动动作、不影响 TTS 输出（delivered=false、channel=none）。';

/// 空态指引里必须出现的短语（未启用 / 还没聊过两种空态共用）。
const String kDirectorEmptyHint = '先启用并聊一轮';

/// **L1 范围声明**（2026-09-15）：director 本轮**不做 L1**。
///
/// 为什么要在面板里明说而不是只写在文档里：这个面板看起来「什么都有」
/// （情绪 / 意图 / 建议语速 / 建议音高 / 结项），最容易被当成一个已完成的产品
/// 能力去验收。用户裁决是「director 下轮」，所以面板顶部必须自己认领这件事——
/// 把建议面板宣传成产品级完成，正是本轮明令禁止的夸大。
const String kDirectorL1ScopeNotice =
    '本轮不做 L1（用户裁决：director 下轮）。本面板只展示建议与登记状态，'
    '未接任何下行通道；请不要把它当作本轮 L1 的完成证据。';

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

/// 数值字段（语速 / 音高）的展示文案：整数不拖小数点，其余保留两位。
String directorNumberText(Object? value) {
  if (value is num) {
    final double d = value.toDouble();
    if (d == d.roundToDouble() && d.abs() < 1e15) return d.toInt().toString();
    return d.toStringAsFixed(2);
  }
  return value == null ? '—' : '$value';
}

/// 结项状态的展示文案（文字表意，不靠颜色）。
String directorClosedText(Object? closed) =>
    closed == true ? '已结项' : '未结项（进行中）';

/// 轮次 turn id 的展示文案：未结项时 turn id 还没写。
String directorTurnText(Object? turn) {
  final String value = turn is String ? turn : '';
  return value.isEmpty ? '未结项' : value;
}

/// 最近决策的**一行**摘要（旧到新列表里的每一项）。纯函数，可单测。
String directorRecentLine(Object? entry) {
  final Map<String, Object?> e = _stringMap(entry);
  final Map<String, Object?> tts = _stringMap(e['suggested_tts']);
  final Object? seq = e['seq'];
  return '#${seq ?? '?'} · 轮 ${directorTurnText(e['turn'])}'
      ' · ${directorEmotionText(e['emotion'])}'
      ' · ${directorIntentText(e['intent'])}'
      ' · 语速 ${directorNumberText(tts['speed'])}'
      ' · 音高 ${directorNumberText(tts['pitch'])}'
      ' · ${e['closed'] == true ? '已结项' : '未结项'}';
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

/// 导演 Mod 的一等决策面板。
class DirectorPanel extends ModPanel {
  const DirectorPanel();

  @override
  String get modId => 'director';

  @override
  Map<String, String> get stateLabels => const <String, String>{
    // `latest` 是产品级加强波次新增的「最近一条决策」（前端直接消费）。
    'latest': '最新决策',
    'decisions': '决策数',
    'turns_seen': '见过轮数',
    'turns_ended': '结项轮数',
    'silent': '静默轮数',
    'errors': '错误数',
    'log_capacity': '日志容量',
    'emotion_lexicon': '情绪词表',
    'recent_decisions': '最近决策',
    'delivered': '已投递',
    'channel': '投递通道',
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
        Text('导演决策（一等面板）', style: theme.textTheme.titleSmall),
        const SizedBox(height: Space.s1),
        // L1 范围声明放在**最前**：先认领「本轮不做」，再说它有什么。
        const InlineNotice(
          message: kDirectorL1ScopeNotice,
          severity: NoticeSeverity.warning,
        ),
        // 零投递语义：**文字**说清，不靠颜色，也不藏在 tooltip 里。
        const InlineNotice(
          message: kDirectorZeroDeliveryNotice,
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
        _hint(muted, 'director 未启用：$kDirectorEmptyHint，然后回来查看每一轮的决策。'),
      ];
    }
    final int decisions = state['decisions'] is num
        ? (state['decisions']! as num).toInt()
        : 0;
    if (decisions == 0) {
      return <Widget>[
        _hint(muted, '还没有决策：$kDirectorEmptyHint（每完成一轮对话，这里就会多一条）。'),
      ];
    }
    final List<Widget> out = <Widget>[];
    final Map<String, Object?> row = _stringMap(latest);
    if (row.isEmpty) {
      out.add(_hint(muted, '账本计数为 $decisions，但最新决策暂时为空——点「刷新运行态」再看一次。'));
    } else {
      out.addAll(_latestBlock(theme, muted, row));
    }
    out.addAll(_recentBlock(theme, muted, state['recent_decisions']));
    return out;
  }

  List<Widget> _latestBlock(
    ThemeData theme,
    TextStyle? muted,
    Map<String, Object?> row,
  ) {
    final Map<String, Object?> tts = _stringMap(row['suggested_tts']);
    return <Widget>[
      const SizedBox(height: Space.s2),
      Text('本轮决策', style: theme.textTheme.labelLarge),
      _kv(theme, muted, '本轮情绪', directorEmotionText(row['emotion'])),
      _kv(theme, muted, '意图', directorIntentText(row['intent'])),
      _kv(theme, muted, '建议语速', directorNumberText(tts['speed'])),
      _kv(theme, muted, '建议音高', directorNumberText(tts['pitch'])),
      _kv(theme, muted, '是否已结项', directorClosedText(row['closed'])),
      _kv(theme, muted, '轮次', directorTurnText(row['turn'])),
    ];
  }

  List<Widget> _recentBlock(ThemeData theme, TextStyle? muted, Object? recent) {
    final List<Object?> entries = recent is List ? recent : const <Object?>[];
    return <Widget>[
      const SizedBox(height: Space.s2),
      Text('最近决策（旧到新）', style: theme.textTheme.labelLarge),
      if (entries.isEmpty)
        Text('（暂无）', style: muted)
      else
        for (final Object? entry in entries)
          Padding(
            padding: const EdgeInsets.only(top: OpticalNudge.thin),
            child: Text(directorRecentLine(entry), style: theme.textTheme.bodySmall),
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

  Widget _kv(ThemeData theme, TextStyle? muted, String label, String value) =>
      Padding(
        padding: const EdgeInsets.only(top: OpticalNudge.thin),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            SizedBox(width: 72, child: Text('$label：', style: muted)),
            Expanded(
              child: Text(value, style: theme.textTheme.bodySmall),
            ),
          ],
        ),
      );
}
