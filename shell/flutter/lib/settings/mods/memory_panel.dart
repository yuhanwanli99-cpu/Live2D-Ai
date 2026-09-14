/// `memory` 的产品面板（产品级加强波次）。
///
/// 职责：可见条数 / hits / 清空；注入可关；与 persona 的策略在 UI 里说清。
/// 本文件由 memory 轨道独占，其他轨道不要改。
///
/// # 边界（与其它轨道 / 既有契约的关系）
///
/// - 面板**自己不做网络**：所有动作走 [ModPanelContext.onCommand]；
/// - `records` / `hits` / `injects` / `evicted` / `last_hits` 的数值来自
///   `GET /api/v1/mods/memory/state` 的 `state`，面板只按 key 展示；
/// - 「注入开关」= 既有 `settings_spec` 的 `enabled_injection` 字段（在卡片
///   配置区），本面板**不**复制一个开关，只解释它 != Mod 启停；
/// - 「清空记忆库」走 `command("clear")`。成功/失败都写**带错误码**的文案。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../ui/inline_notice.dart';
import '../../ui/theme.dart';
import 'mod_panel.dart';

/// 可复制验收步骤所在文档（面板里的指向）。
const String kMemoryDocPath = 'docs/architecture/memory-mod-v0.md';

/// 计数的展示文本：缺失 → `—`（不假装是 0）。纯函数，可单测。
String memoryCountText(Object? value) {
  if (value is num) return value.toInt().toString();
  return '—';
}

/// 运行态摘要一行：条数 / 命中 / 注入 / 淘汰 / 上轮命中。纯函数，可单测。
String memorySummaryLine(Map<String, Object?>? state) {
  final Map<String, Object?> s = state ?? const <String, Object?>{};
  return '记忆条数 ${memoryCountText(s['records'])} 条'
      ' · 累计命中 ${memoryCountText(s['hits'])} 次'
      ' · 注入 ${memoryCountText(s['injects'])} 轮'
      ' · 已淘汰 ${memoryCountText(s['evicted'])} 条'
      ' · 上轮命中 ${memoryCountText(s['last_hits'])} 条';
}

/// `command("clear")` 的失败码 → **可处置**的一句话（必须带码）。纯函数，可单测。
String memoryClearErrorMessage(ApiException e) {
  switch (e.code) {
    case 'command_unavailable':
      return '清空暂时不可用（503 command_unavailable：未启用或 Mod 正忙），可稍后重试';
    case 'unsupported_command':
      return '这个版本不认识清空命令（409 unsupported_command）';
    case 'command_failed':
      return '清空失败（409 command_failed）：${e.message}';
    case 'not_found':
      return '这个 Mod 不在服务端注册表（404 not_found）';
    default:
      return '清空失败：$e';
  }
}

class MemoryPanel extends ModPanel {
  const MemoryPanel();

  @override
  String get modId => 'memory';

  @override
  Map<String, String> get stateLabels => const <String, String>{
    'records': '当前条数',
    'writes': '写入条数',
    'hits': '命中次数',
    'injects': '注入轮数',
    'errors': '错误数',
    'evicted': '已淘汰',
    'last_hits': '上轮命中',
    'top_k': '每轮注入条数',
    'max_records': '条数上限',
    'enabled_injection': '注入开关',
    'store_path': '记忆库路径',
    'turns_seen': '经历轮数',
  };

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) =>
      _MemoryPanelBody(ctx: ctx);
}

class _MemoryPanelBody extends StatefulWidget {
  const _MemoryPanelBody({required this.ctx});

  final ModPanelContext ctx;

  @override
  State<_MemoryPanelBody> createState() => _MemoryPanelBodyState();
}

class _MemoryPanelBodyState extends State<_MemoryPanelBody> {
  bool _busy = false;
  String? _message;
  bool _messageIsError = false;

  Future<void> _clear() async {
    setState(() {
      _busy = true;
      _message = null;
    });
    try {
      final ModCommandResult result = await widget.ctx.onCommand('clear');
      if (!mounted) return;
      final bool residue = result.result['residue'] == true;
      setState(() {
        _busy = false;
        _messageIsError = false;
        _message =
            '已清空记忆库：清掉 ${memoryCountText(result.result['removed'])} 条，'
            '现存 ${memoryCountText(result.result['records'])} 条'
            '${residue ? '。提示词里仍留着上一轮注入的记忆块，停用 Mod 会按既有语义剥离' : ''}';
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _messageIsError = true;
        _message = memoryClearErrorMessage(e);
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _messageIsError = true;
        _message = '清空失败：$e';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final ModPanelContext ctx = widget.ctx;
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s3),
        Text('记忆概览', style: theme.textTheme.titleSmall),
        const SizedBox(height: Space.s1),
        if (ctx.stateLoading)
          Text('正在读取运行态…', style: muted)
        else if (ctx.stateError != null)
          InlineNotice(message: ctx.stateError!, dense: true)
        else
          Text(memorySummaryLine(ctx.state), style: theme.textTheme.bodySmall),
        const SizedBox(height: Space.s2),
        const InlineNotice(
          message: '配置区的「注入开关」（enabled_injection）只决定要不要把命中内容'
              '拼进下一轮提示词；关掉后记忆照记。它不是 Mod 启停——启停在卡片标题行的开关。',
          severity: NoticeSeverity.info,
          dense: true,
        ),
        const SizedBox(height: Space.s2),
        const InlineNotice(
          message: '记忆与人设（persona）写的是同一份 system_prompt：谁后写谁覆盖，'
              '不合并、不仲裁。想稳定用人设，就别同时开 memory 注入。',
          severity: NoticeSeverity.warning,
          dense: true,
        ),
        const SizedBox(height: Space.s2),
        Text(
          '要让相关内容下一轮被提起：先发一句要记住的话，等这一轮结束，'
          '再问相关的问题（本 Mod 只对下一轮生效）。逐条可复制的验收步骤见 '
          '$kMemoryDocPath 第 12 节。',
          style: muted,
        ),
        const SizedBox(height: Space.s2),
        Align(
          alignment: Alignment.centerLeft,
          child: OutlinedButton.icon(
            onPressed: (!ctx.enabled || _busy) ? null : () => unawaited(_clear()),
            icon: _busy
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.delete_outline, size: 16),
            label: Text(_busy ? '清空中…' : '清空记忆库'),
          ),
        ),
        if (!ctx.enabled)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: Text('Mod 未启用，清空不可用——先在卡片标题行的开关里启用。', style: muted),
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
}
