/// external-input（外部事件接入）的产品面板（L1 产品级波次）。
///
/// 面板给四件事：计数摘要 + 重置计数；**测试注入**（command test_inject，等价
/// POST /api/v1/external/chat：同一条渲染口径、同一个 say_tx，但不需要 token）；
/// token_set 两态人话（不回显明文）；模板 / 前缀的本地渲染示例。
/// 副标题只给中性信息（Mod id：external-input），不做品牌化改名。
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

/// 停用态说明：L1「停用 403 可读」在 UI 上的**静态**落点（不点按钮也能看到）。
const String kExternalInputDisabledNotice =
    'Mod 已停用：HTTP 端点会回 403 mod_disabled，'
    '命令通道回 503 command_unavailable；请先启用。';
/// 与按钮等价的真 HTTP 调用（<随机文本> 是占位符，用户自己替换）。
const String kExternalInputCurlEquivalent =
    'curl -X POST http://127.0.0.1:18080/api/v1/external/chat '
    '-H "Content-Type: application/json" '
    "-d '{\"text\":\"<随机文本>\"}'";
/// 示例用的「外部文本」（模板渲染示例里被替换进占位符的那一段）。
const String kExternalInputSampleText = '主播好';
/// 计数四项的稳定 key（与 Rust counters::COUNTER_KEYS 逐字一致）。
const List<String> kExternalInputCounterKeys = <String>[
  'accepts',
  'rejects',
  'busy',
  'v2_ignored',
];
/// 与 Rust render_injected_text **同语义**的本地渲染（纯函数，不发请求）。
///
/// - template 空 / 纯空白 → 等价只留占位符；
/// - 含 {text} → 替换全部占位符；
/// - 非空但不含占位符 → 视为字面前缀，外部文本追加其后；
/// - prefix 永远拼在最前。
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
/// 当前配置下模板渲染出的**示例**字符串；prefix/template 都空 → null
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
/// 令牌状态的人话（两态；state == null = 运行态还没读到）。纯函数，可单测。
String tokenStatusText(Map<String, Object?>? state) {
  if (state == null) {
    return '运行态还没读到：令牌是否已设置暂时未知（可点上面的「刷新运行态」）';
  }
  return state['token_set'] == true
      ? '已设置令牌：外部请求必须带 token（env 或 Mod 配置；界面不回显明文）'
      : '未设令牌：仅本机可用（端点只监听 127.0.0.1，不鉴权）';
}
/// 注入回执一句：文本 + 当前两项计数（计数由 onRefreshState 刷新后回填）。
/// 纯函数，可单测。
String injectReceiptText({
  required String injectedText,
  required bool accepted,
  Map<String, Object?>? state,
}) {
  final String counts =
      '已接受 ${_count(state, 'accepts')} / 忙碌丢弃 ${_count(state, 'busy')}';
  return accepted
      ? '已注入：$injectedText（$counts）'
      : '主链忙碌，本条被丢弃：$injectedText（$counts）；可稍后重试。';
}
/// command test_inject 的失败码 → **可处置**的一句话（必须带码）。纯函数，可单测。
///
/// command_unavailable 必须点明「Mod 未启用或正忙（503）」并给出**下一步动作**
/// （先打开卡片标题行的开关）——L1 的「停用后再注入 → 可读失败」就落在这里。
String externalInputInjectErrorMessage(ApiException e) {
  switch (e.code) {
    case 'command_unavailable':
      return '${e.code}：${e.message}\n'
          'Mod 未启用或正忙（503）：请先打开卡片标题行的开关；'
          '若是刚启停 / 保存过，Mod worker 可能仍在忙——稍后可重试。';
    case 'unsupported_command':
      return '${e.code}：${e.message}\n'
          '服务端这只 Mod 不认识 test_inject（409）：多为版本不匹配，请重启服务后重试。';
    case 'command_failed':
      return '${e.code}：${e.message}\n'
          '命令执行失败（409）：常见原因是渲染后文本超过 2000 字符，改短后重试。';
    default:
      return '${e.code}：${e.message}';
  }
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
    'inject_via_command': '命令注入可用',
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

  late final TextEditingController _injectController;
  bool _injecting = false;
  String? _injectError;
  String? _lastInjected;
  bool _injectAccepted = false;

  @override
  void initState() {
    super.initState();
    // 初值 = 按当前配置渲染好的示例（看到什么就注入什么；配置后续改了不追改）。
    _injectController = TextEditingController(
      text: templateExample(widget.ctx.mod.config) ?? kExternalInputSampleText,
    );
  }

  @override
  void dispose() {
    _injectController.dispose();
    super.dispose();
  }

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
        const SizedBox(height: Space.s1),
        Text(
          'Mod id：external-input（端点契约不变）',
          key: const Key('external-input-mod-id'),
          style: muted,
        ),
        // ---- 停用态静态说明（不点按钮也必须能看到）----
        if (!ctx.enabled) ...<Widget>[
          const SizedBox(height: Space.s2),
          const InlineNotice(
            key: Key('external-input-disabled-notice'),
            message: kExternalInputDisabledNotice,
            severity: NoticeSeverity.warning,
            dense: true,
          ),
        ],
        // ---- 测试注入（等价 HTTP 端点，但走命令通道、不需要 token）----
        const SizedBox(height: Space.s3),
        Text('测试注入', style: theme.textTheme.labelLarge),
        const SizedBox(height: Space.s1),
        Text(
          '框里的初值就是按当前前缀 / 模板渲染好的样子；改它 = 改要注入的内容。'
          '命令把这段文本逐字送进同一支 say_tx 主链（与 HTTP 端点同一条链、'
          '同一套计数），区别是本命令走 Mod 命令通道、不需要 token。',
          style: muted,
        ),
        const SizedBox(height: Space.s1),
        TextField(
          key: const Key('external-input-inject-text'),
          controller: _injectController,
          minLines: 1,
          maxLines: 3,
          decoration: const InputDecoration(
            labelText: '测试文本（逐字注入）',
            border: OutlineInputBorder(),
          ),
        ),
        const SizedBox(height: Space.s1),
        // 停用时**不禁用**：点下去拿到的 503 command_unavailable 正是
        // 「停用后注入 → 可读失败」的现场证据（上面的静态说明同时给处置办法）。
        Align(
          alignment: Alignment.centerLeft,
          child: FilledButton.icon(
            key: const Key('external-input-inject'),
            onPressed: _injecting ? null : _inject,
            icon: _injecting
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.send, size: 16),
            label: Text(_injecting ? '注入中…' : '测试注入'),
          ),
        ),
        if (_injectError != null) ...<Widget>[
          const SizedBox(height: Space.s1),
          InlineNotice(
            key: const Key('external-input-inject-error'),
            message: _injectError!,
            dense: true,
          ),
        ],
        if (_lastInjected != null) ...<Widget>[
          const SizedBox(height: Space.s1),
          InlineNotice(
            key: const Key('external-input-inject-receipt'),
            message: injectReceiptText(
              injectedText: _lastInjected!,
              accepted: _injectAccepted,
              state: state,
            ),
            severity: _injectAccepted
                ? NoticeSeverity.info
                : NoticeSeverity.warning,
            dense: true,
          ),
        ],
        const SizedBox(height: Space.s1),
        Text(
          '真 HTTP 等价（同一条渲染 + 主链路径；命令通道不需要 token，'
          '下面的 curl 在服务端配了 token 时需要带 token）：',
          style: muted,
        ),
        const SizedBox(height: Space.s1),
        Text(
          kExternalInputCurlEquivalent,
          key: const Key('external-input-curl-equivalent'),
          style: theme.textTheme.bodySmall,
        ),
        // ---- 计数摘要 + 重置 ----
        const SizedBox(height: Space.s3),
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
  /// 「测试注入」：command test_inject（prefix:false = 文本框里已是最终文本）。
  Future<void> _inject() async {
    final ModPanelContext ctx = widget.ctx;
    final String text = _injectController.text.trim();
    if (text.isEmpty) {
      setState(() {
        _lastInjected = null;
        _injectError = '「测试文本」不能为空。';
      });
      return;
    }
    setState(() {
      _injecting = true;
      _injectError = null;
      _lastInjected = null;
    });
    try {
      final ModCommandResult result = await ctx.onCommand(
        'test_inject',
        <String, Object?>{'text': text, 'prefix': false},
      );
      if (!mounted) return;
      if (!result.ok) {
        setState(() {
          _injecting = false;
          _injectError = 'test_inject 失败：服务端返回 ok=false';
        });
        return;
      }
      final Object? injected = result.result['injected_text'];
      setState(() {
        _injecting = false;
        _lastInjected = injected is String ? injected : text;
        _injectAccepted = result.result['accepted'] == true;
      });
      // 计数要立刻反映这次注入（宿主拿到新快照后本面板会重建）。
      // 不调 notifyChanged：注入是瞬时运行行为、不改任何配置，
      // 说「需重新点火 / 重启后生效」是夸大。
      await ctx.onRefreshState();
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _injecting = false;
        _injectError = externalInputInjectErrorMessage(e);
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _injecting = false;
        _injectError = 'test_inject 失败：$e';
      });
    }
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
  /// 命令结果里的 before 快照 → 「清零前：…」一句（形状不对时不硬编）。
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
