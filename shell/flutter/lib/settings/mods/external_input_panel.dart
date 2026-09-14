/// `external-input` 的产品面板（产品级加强波次）。
///
/// 职责（对着本轨的产品级诉求逐条）：
/// 1. 把 `state_json` 的 accepts/rejects/busy/v2_ignored 摊成**一行人话摘要**，
///    并给一个可点动作「重置计数」（`command reset_counters`，二次确认）；
/// 2. 把 `token_set` 说成人话（「已设置令牌」/「未设令牌：仅本机可用」），
///    **不回显任何明文**；
/// 3. 说明模板 / 前缀怎么写，并用**本地**配置渲染一条示例（纯前端，不发请求）。
///
/// 本文件由 external-input 轨道独占，其他轨道不要改。
library;

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../ui/inline_notice.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';
import 'mod_panel.dart';

/// 示例用的「外部文本」（模板渲染示例里被替换进占位符的那一段）。
const String kExternalInputSampleText = '主播好';

/// 计数四项的稳定 key（与 Rust `counters::COUNTER_KEYS` 逐字一致）。
const List<String> kExternalInputCounterKeys = <String>[
  'accepts',
  'rejects',
  'busy',
  'v2_ignored',
];

/// 与 Rust `render_injected_text` **同语义**的本地渲染（纯函数，不发请求）。
///
/// - `template` 空 / 纯空白 → 等价只留占位符；
/// - 含 `{text}` → 替换全部占位符；
/// - 非空但不含占位符 → 视为字面前缀，外部文本追加其后；
/// - `prefix` 永远拼在最前。
String renderInjectedText({
  required String prefix,
  required String template,
  required String text,
}) {
  final String tpl = template.trim().isEmpty ? '{text}' : template;
  final String body = tpl.contains('{text}')
      ? tpl.replaceAll('{text}', text)
      : '$tpl$text';
  return '$prefix$body';
}

/// 当前配置下模板渲染出的**示例**字符串；prefix/template 都空 → `null`
/// （= 原样透传，没有模板效果可展示）。纯函数，不发请求。
String? templateExample(
  Map<String, Object?> config, {
  String sample = kExternalInputSampleText,
}) {
  final String prefix = _string(config['prefix']);
  final String template = _string(config['text_template']);
  if (prefix.trim().isEmpty && template.trim().isEmpty) return null;
  return renderInjectedText(prefix: prefix, template: template, text: sample);
}

/// 一行人话摘要（四项计数；**不靠颜色**）。纯函数，可单测。
String counterSummary(Map<String, Object?> state) =>
    '已接受 ${_count(state, 'accepts')} 条弹幕/礼物'
    ' · 拒绝 ${_count(state, 'rejects')}'
    ' · 忙碌丢弃 ${_count(state, 'busy')}'
    ' · 礼物 v2 兜底 ${_count(state, 'v2_ignored')}';

/// 令牌状态的人话（两态；`state == null` = 运行态还没读到）。纯函数，可单测。
String tokenStatusText(Map<String, Object?>? state) {
  if (state == null) {
    return '运行态还没读到：令牌是否已设置暂时未知（可点上面的「刷新运行态」）';
  }
  return state['token_set'] == true
      ? '已设置令牌：外部请求必须带 token（env 或 Mod 配置；界面不回显明文）'
      : '未设令牌：仅本机可用（端点只监听 127.0.0.1，不鉴权）';
}

int _count(Map<String, Object?>? state, String key) {
  final Object? value = state?[key];
  return value is num ? value.toInt() : 0;
}

String _string(Object? value) => value is String ? value : '';

class ExternalInputPanel extends ModPanel {
  const ExternalInputPanel();

  @override
  String get modId => 'external-input';

  @override
  Map<String, String> get stateLabels => const <String, String>{
    'accepts': '已接受',
    'rejects': '已拒绝',
    'busy': '忙碌拒绝',
    'v2_ignored': '礼物 v2 兜底',
    'ready': '已就绪',
    'token_set': '令牌已设置',
  };

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) =>
      _ExternalInputBody(ctx: ctx);
}

class _ExternalInputBody extends StatefulWidget {
  const _ExternalInputBody({required this.ctx});

  final ModPanelContext ctx;

  @override
  State<_ExternalInputBody> createState() => _ExternalInputBodyState();
}

class _ExternalInputBodyState extends State<_ExternalInputBody> {
  bool _resetting = false;
  String? _message;
  bool _messageIsError = false;

  @override
  Widget build(BuildContext context) {
    final ModPanelContext ctx = widget.ctx;
    final Map<String, Object?>? state = ctx.state;
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    final String? example = templateExample(ctx.mod.config);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        const SizedBox(height: Space.s3),
        const SectionHeader(
          title: '外部事件接入',
          description: '直播弹幕 / 本机脚本经 POST /api/v1/external/chat 注入主链路；'
              'B 站协议抓取住在 Win sidecar，不在本进程。',
        ),
        const SizedBox(height: Space.s2),

        // ---- 计数摘要 + 重置 ----
        Text('外部事件计数（本次进程运行以来）', style: theme.textTheme.labelLarge),
        const SizedBox(height: Space.s1),
        if (state == null)
          Text(
            ctx.stateLoading ? '计数读取中…' : '运行态还没读到：点上面的「刷新运行态」再看计数。',
            style: muted,
          )
        else
          Text(
            counterSummary(state),
            key: const Key('external-input-counter-summary'),
            style: theme.textTheme.bodyMedium,
          ),
        const SizedBox(height: Space.s1),
        Align(
          alignment: Alignment.centerLeft,
          child: OutlinedButton.icon(
            key: const Key('external-input-reset'),
            onPressed: !ctx.enabled || _resetting ? null : _confirmReset,
            icon: _resetting
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.restart_alt, size: 16),
            label: Text(_resetting ? '清零中…' : '重置计数（四项全部清零）'),
          ),
        ),
        const SizedBox(height: Space.s1),
        Text(
          ctx.enabled
              ? '会把「已接受 / 拒绝 / 忙碌丢弃 / 礼物 v2 兜底」全部归零，清零后无法恢复。'
              : 'Mod 未启用：启用后才能清零计数。',
          style: muted,
        ),
        if (ctx.stateError != null) ...<Widget>[
          const SizedBox(height: Space.s1),
          InlineNotice(message: '计数暂时读不到：${ctx.stateError}', dense: true),
        ],
        if (_message != null) ...<Widget>[
          const SizedBox(height: Space.s2),
          InlineNotice(
            message: _message!,
            severity: _messageIsError
                ? NoticeSeverity.danger
                : NoticeSeverity.info,
            dense: true,
          ),
        ],

        // ---- 令牌 ----
        const SizedBox(height: Space.s3),
        Text('访问令牌', style: theme.textTheme.labelLarge),
        const SizedBox(height: Space.s1),
        Text(
          tokenStatusText(state),
          key: const Key('external-input-token-status'),
          style: theme.textTheme.bodySmall,
        ),

        // ---- 模板 / 前缀 ----
        const SizedBox(height: Space.s3),
        const SectionHeader(
          title: '模板 / 前缀怎么写',
          description: '占位符 {text} 会被替换成清洗后的外部文本；prefix 拼在最前。'
              '两个都留空 = 外部文本原样送进链路。',
        ),
        const SizedBox(height: Space.s1),
        Text(
          example == null
              ? '当前没有配置前缀 / 模板：外部文本会原样送进链路（示例：$kExternalInputSampleText）。'
              : '当前配置的渲染示例：$example',
          key: const Key('external-input-template-example'),
          style: theme.textTheme.bodyMedium,
        ),
      ],
    );
  }

  Future<void> _confirmReset() async {
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: const Text('清零外部事件计数？'),
        content: const Text(
          '会把「已接受 / 拒绝 / 忙碌丢弃 / 礼物 v2 兜底」四个计数全部归零，清零后无法恢复。',
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('取消'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('确认清零'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    await _reset();
  }

  Future<void> _reset() async {
    setState(() {
      _resetting = true;
      _message = null;
    });
    try {
      final ModCommandResult result = await widget.ctx.onCommand('reset_counters');
      if (!mounted) return;
      setState(() {
        _resetting = false;
        _messageIsError = !result.ok;
        _message = result.ok
            ? '已清零。${_beforeText(result.result)}'
            : '清零失败：服务端返回 ok=false';
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _resetting = false;
        _messageIsError = true;
        // 带错误码：用户要拿界面上的码去日志里搜（项目错误契约）。
        _message = '清零失败：$e';
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _resetting = false;
        _messageIsError = true;
        _message = '清零失败：$e';
      });
    }
  }

  /// 命令结果里的 `before` 快照 → 「清零前：…」一句（形状不对时不硬编）。
  String _beforeText(Map<String, Object?> result) {
    final Object? before = result['before'];
    if (before is! Map) return '计数已归零。';
    final Map<String, Object?> typed = <String, Object?>{};
    before.forEach((Object? key, Object? value) {
      if (key is String) typed[key] = value;
    });
    return '清零前：${counterSummary(typed)}。';
  }
}
