/// external-input（外部事件接入）的产品面板（2026-10-08 二次口径）。
///
/// # 这一版有什么
///
/// 1. 卡片标题行的启用开关（由 `ModsSection` 画）+ 一句话：
///    「打开后，直播间的弹幕和礼物会说给角色听。」；
/// 2. **四项配置回到产品表单**（端口提示 / 令牌 / 弹幕怎么说给角色 / 每条前面
///    加的字）：标签在 Rust spec 里，说明由 [fieldHelp] 给；
/// 3. `devMode == true` 时多一块**自检**：发一条测试 + 四项计数 + 计数清零。
///
/// # 与上一版的差别（不要再回退）
///
/// 上一版把四个键整体藏起来、把自检与计数整块删掉；这一版按维护者口径把
/// **配置放回产品面**、把**排障件放回开发模式**——两件事都不该混在同一个开关里。
/// 仍未回来的：curl 等价命令、端口长文、403 / 503 闲置说明、真 HTTP 等价入口。
///
/// # 留下来的纯函数
///
/// [renderInjectedText] / [templateExample] / [counterSummary] / [tokenStatusText]
/// 与 [kExternalInputCounterKeys] 都**保留**：它们是 Mod 侧语义的本地镜像
/// （模板渲染、计数口径、令牌两态），由回归守着，且被 `docs/external-input.md`
/// 引用。
///
/// 本文件由 external-input 轨道独占。
library;

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../ui/inline_notice.dart';
import '../../ui/theme.dart';
import 'mod_panel.dart';

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

int _count(Map<String, Object?>? state, String key) {
  final Object? value = state?[key];
  return value is num ? value.toInt() : 0;
}

String _string(Object? value) => value is String ? value : '';

class ExternalInputPanel extends ModPanel {
  const ExternalInputPanel();

  @override
  String get modId => 'external-input';

  // 2026-10-09：fieldHelp 已退役（三级功能介绍全删），本面板不再声明它。

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

/// 面板本体：一句话常驻 + `devMode` 才画的自检块。
class _ExternalInputBody extends StatefulWidget {
  const _ExternalInputBody({required this.ctx});

  final ModPanelContext ctx;

  @override
  State<_ExternalInputBody> createState() => _ExternalInputBodyState();
}

class _ExternalInputBodyState extends State<_ExternalInputBody> {
  final TextEditingController _text = TextEditingController();
  bool _busy = false;
  String? _message;
  bool _messageIsError = false;
  bool _clearing = false;

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  /// 发一条测试：命令名与入参与 HTTP 端点同源（见 Rust `inject.rs`）。
  Future<void> _send() async {
    final String text = _text.text.trim();
    if (text.isEmpty) {
      setState(() {
        _messageIsError = true;
        _message = '先写一条要发的话';
      });
      return;
    }
    setState(() {
      _busy = true;
      _message = null;
    });
    try {
      final ModCommandResult result = await widget.ctx.onCommand(
        'test_inject',
        <String, Object?>{'text': text},
      );
      if (!mounted) return;
      final bool accepted = result.result['accepted'] == true;
      final String rendered = _string(result.result['injected_text']);
      setState(() {
        _busy = false;
        _messageIsError = !accepted;
        _message = accepted
            ? '主链接住了这条：${rendered.isEmpty ? text : rendered}'
            : '主链正忙，这条没接住：稍后再点一次';
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _messageIsError = true;
        _message = '发送失败：${e.message}（${e.code}）';
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _messageIsError = true;
        _message = '发送失败：$e';
      });
    }
  }

  Future<void> _resetCounters() async {
    setState(() {
      _clearing = true;
      _message = null;
    });
    try {
      await widget.ctx.onCommand('reset_counters', const <String, Object?>{});
      if (!mounted) return;
      setState(() {
        _clearing = false;
        _messageIsError = false;
        _message = '计数已清零';
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _clearing = false;
        _messageIsError = true;
        _message = '计数清零失败：${e.message}（${e.code}）';
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _clearing = false;
        _messageIsError = true;
        _message = '计数清零失败：$e';
      });
    }
  }

  Widget _devBlock(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final ModPanelContext ctx = widget.ctx;
    final bool canSend = ctx.enabled && !_busy;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s3),
        Text('直播接入自检', style: theme.textTheme.titleSmall),
        const SizedBox(height: Space.s1),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Expanded(
              child: TextField(
                key: const Key('external-input-test-field'),
                controller: _text,
                enabled: canSend,
                decoration: const InputDecoration(
                  isDense: true,
                  border: OutlineInputBorder(),
                  hintText: '像观众发来的一条弹幕',
                ),
              ),
            ),
            const SizedBox(width: Space.s2),
            TextButton.icon(
              key: const Key('external-input-test-send'),
              onPressed: canSend ? () => _send() : null,
              icon: const Icon(Icons.send_outlined, size: 16),
              label: const Text('发送这条测试'),
            ),
          ],
        ),
        const SizedBox(height: Space.s2),
        Text(counterSummary(ctx.state ?? const <String, Object?>{}), style: theme.textTheme.bodySmall),
        const SizedBox(height: Space.s1),
        Align(
          alignment: Alignment.centerLeft,
          child: OutlinedButton.icon(
            key: const Key('external-input-reset-counters'),
            onPressed: ctx.enabled && !_clearing ? () => _resetCounters() : null,
            icon: const Icon(Icons.restart_alt, size: 16),
            label: Text(_clearing ? '清零中…' : '计数清零'),
          ),
        ),
        if (!ctx.enabled)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: Text(
              'Mod 未启用：发送与清零都不可用——先在卡片标题行的开关里启用。',
              style: theme.textTheme.bodySmall?.copyWith(
                color: colors.contentMuted,
              ),
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
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const Padding(
          padding: EdgeInsets.only(top: Space.s3),
          child: InlineNotice(
            message: '打开后，直播间的弹幕和礼物会说给角色听。',
            severity: NoticeSeverity.info,
            dense: true,
          ),
        ),
        if (widget.ctx.devMode) _devBlock(context),
      ],
    );
  }
}
