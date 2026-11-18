/// `memory` 的产品面板（产品级加强波次 + L1 主动管理面；2026-11-18 去会话绑定）。
///
/// 职责：概览一行 / **记忆列表（查看 · 编辑 · 删除）** / **导入一条** / 清空整库。
/// 本文件由 memory 轨道独占，其他轨道不要改。
///
/// # 2026-11-18：会话分桶 / 注入进提示词两条旧承诺**已停用**
///
/// 服务端说话路径不再读 memory Mod 写的会话注入槽（`session_scope.rs` 的
/// `speak_prompt_for` 剔除 persona / memory）；2026-11-16 起模型这一轮的记忆
/// 来自主链热槽（系统句 + 钉住的事 + 最近对话 + 热卡），与「当前会话」无关。
/// 树内 memory Mod 也默认不注册（`AVAILABLE_MOD_FACTORIES` 为空）。所以面板：
/// - 命令一律走**全局**路径（不再带 `session_id`），切会话不改变列表；
/// - 用一句诚实话说明「这里的内容不会进模型提示词」，不再按会话分桶。
///
/// # 边界（与其它轨道 / 既有契约的关系）
///
/// - 面板**自己不做网络**：所有动作走 [ModPanelContext.onCommand]；
/// - 数值来自 `GET /api/v1/mods/memory/state`；列表来自
///   `command("list")`（最新在前），编辑/删除按 `id` 定位；
/// - 「注入开关」= 既有 `settings_spec` 的 `enabled_injection` 字段（在卡片
///   配置区），本面板**不**复制一个开关，只解释它 != Mod 启停；
/// - 「清空记忆库」走 `command("clear")`。成功/失败都写**带错误码**的文案；
/// - **同轮生效**：面板只提示，不改时序（Timing 在 Rust 侧，见 §2）。
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

/// 列表一次拉多少条（与 Rust `DEFAULT_LIST_LIMIT` 同值；Rust 另有 200 上限）。
const int kMemoryListLimit = 50;

/// 计数的展示文本：缺失 → `—`（不假装是 0）。纯函数，可单测。
String memoryCountText(Object? value) {
  if (value is num) return value.toInt().toString();
  return '—';
}

/// 运行态摘要一行：**只留条数**（2026-10-08）。纯函数，可单测。
///
/// 从前这一行把 hits / injects / evicted / last_hits 全摊开——那些是检索内部
/// 计数，用户既看不懂也不处置得了。用户要知道的只有「记住了多少」。
/// 缺 `records`（还没读到）时照旧显示 `—`，不假装是 0。
String memorySummaryLine(Map<String, Object?>? state) {
  final Map<String, Object?> s = state ?? const <String, Object?>{};
  return '记住了 ${memoryCountText(s['records'])} 条';
}

/// **记忆作用域说明（2026-11-18）**：诚实说清「这里的记忆不进模型提示词」。
///
/// 旧面板按「当前会话」说「只看这次对话记住的内容」，但服务端说话路径已不读
/// memory 的会话注入槽；模型这一轮用的记忆来自主链热槽。再说「只看这次对话」
/// 就是谎报能力——用户会以为换会话就换了记忆。产品面上不出现「桶」这个词。
const String kMemoryScopeNotice =
    '这里的记忆只进 Mod 自己的记忆库，不会写进模型提示词；'
    '模型这一轮的上下文与当前会话无关（旧版按会话记忆已停用）。';

/// 一条记录的展示文本：压平空白 + 截断（面板不把整段糊上来）。纯函数，可单测。
String memoryRecordText(Object? raw, {int maxChars = 80}) {
  final String flat = (raw is String ? raw : '')
      .split(RegExp(r'\s+'))
      .where((String s) => s.isNotEmpty)
      .join(' ');
  if (flat.isEmpty) return '（空记录）';
  if (flat.length <= maxChars) return flat;
  return '${flat.substring(0, maxChars)}…';
}

/// 从 `list` 返回体解析记录（未知形状 → 空列表，不崩）。纯函数，可单测。
List<Map<String, Object?>> memoryParseRecords(Object? raw) {
  if (raw is! List) return const <Map<String, Object?>>[];
  return raw.whereType<Map<Object?, Object?>>().map((Map<Object?, Object?> m) {
    final Map<String, Object?> out = <String, Object?>{};
    m.forEach((Object? k, Object? v) {
      if (k is String) out[k] = v;
    });
    return out;
  }).toList();
}

/// 命令失败码 → **可处置**的一句话（必须带码）。纯函数，可单测。
String memoryCommandErrorMessage(ApiException e, String action) {
  switch (e.code) {
    case 'command_unavailable':
      return '$action失败：Mod 未启用或正忙，可稍后重试（503 command_unavailable）';
    case 'unsupported_command':
      return '$action失败：服务端这个版本不认识这条命令（409 unsupported_command）';
    case 'command_failed':
      return '$action失败：${e.message}（409 command_failed）';
    case 'not_found':
      return '$action失败：这个 Mod 不在服务端注册表（404 not_found）';
    default:
      return '$action失败：$e';
  }
}

/// `command("clear")` 的失败码 → 可处置的一句话（保留旧入口，语义同
/// [memoryCommandErrorMessage]）。纯函数，可单测。
String memoryClearErrorMessage(ApiException e) =>
    memoryCommandErrorMessage(e, '清空');

/// 把任意值当 JSON 对象读（非对象 -> null）。纯函数，可单测。
Map<String, Object?>? memoryParseObject(Object? raw) {
  if (raw is! Map) return null;
  final Map<String, Object?> out = <String, Object?>{};
  raw.forEach((Object? k, Object? v) {
    if (k is String) out[k] = v;
  });
  return out;
}

/// `state_json.summary`（P1-5）：非对象 -> null（面板整块不渲染）。纯函数，可单测。
Map<String, Object?>? memorySummaryState(Map<String, Object?>? state) =>
    memoryParseObject(state == null ? null : state['summary']);

/// 摘要正文（空 / 缺失 -> null：空摘要按「没有」处理）。纯函数，可单测。
String? memorySummaryText(Map<String, Object?> summary) {
  final Object? raw = summary['text'];
  if (raw is! String) return null;
  final String trimmed = raw.trim();
  return trimmed.isEmpty ? null : trimmed;
}

class MemoryPanel extends ModPanel {
  const MemoryPanel();

  @override
  String get modId => 'memory';

  /// `store_path`：产品界面**一个都不画**（也不收进「高级」）——库放哪是部署细节。
  ///
  /// 键仍被解析，`mods.json` 里已有的值保存时原样带走
  ///（见 `dev_tools_mod_config.dart` 的 `_buildConfig`）。
  @override
  Set<String> get hiddenKeys => const <String>{'store_path'};

  /// 其余 10 个键：**开发模式才画**（产品面不画、保存时原样带走）。
  ///
  /// 与 `memory_settings_spec()` 逐键对应，**一个不漏**：检索条数 / 条数上限
  /// + 摘要子系统的 8 项。它们是调参与排障旋钮，不是产品面该摆的开关。
  @override
  Set<String> get devKeys => const <String>{
    'top_k',
    'max_records',
    'summary_enabled',
    'summary_base_url',
    'summary_model',
    'summary_api_key_env',
    'summary_timeout_ms',
    'summary_ratio',
    'summary_cooldown_turns',
    'summary_keep_recent_turns',
  };

  // 2026-10-09：fieldHelp 已退役（三级功能介绍全删）。

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
    'session_scoped': '会话分桶（已停用）',
    'active_session': '当前会话（仅显示）',
    'bucket_path': '记忆库文件',
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
  bool _listLoading = true;
  String? _listError;
  List<Map<String, Object?>> _records = const <Map<String, Object?>>[];
  Object? _total;
  String? _bucket;
  final TextEditingController _importController = TextEditingController();

  /// 列表取数的**代**：迟到的旧响应必须丢掉，否则它会把刚刷新的列表
  /// 重新填回上一次取数的内容。
  int _listEpoch = 0;

  @override
  void initState() {
    super.initState();
    // 进入面板就拉一次列表；不在 initState 里 setState（首帧还没建）。
    unawaited(_refreshList(initial: true));
  }

  @override
  void dispose() {
    _importController.dispose();
    super.dispose();
  }

  Future<void> _refreshList({bool initial = false}) async {
    final int epoch = ++_listEpoch;
    if (!initial && mounted) {
      setState(() {
        _listLoading = true;
        _listError = null;
      });
    }
    try {
      // 全局一条路：不再带 `session_id`（会话分桶已停用，服务端也没有按会话的
      // 说话注入）。命令走 Mod 自己的库，与「当前看哪个会话」无关。
      final ModCommandResult result = await widget.ctx.onCommand(
        'list',
        <String, Object?>{'limit': kMemoryListLimit},
      );
      // 迟到的旧响应：丢掉（否则会把刚刷新的列表填回上一次取数的内容）。
      if (!mounted || epoch != _listEpoch) return;
      setState(() {
        _listLoading = false;
        _listError = null;
        _records = memoryParseRecords(result.result['records']);
        _total = result.result['total'];
        final Object? bucket = result.result['bucket'];
        _bucket = bucket is String ? bucket : null;
      });
    } on ApiException catch (e) {
      if (!mounted || epoch != _listEpoch) return;
      setState(() {
        _listLoading = false;
        _listError = memoryCommandErrorMessage(e, '读取记忆列表');
      });
    } catch (e) {
      if (!mounted || epoch != _listEpoch) return;
      setState(() {
        _listLoading = false;
        _listError = '读取记忆列表失败：$e';
      });
    }
  }

  /// 所有主动动作走同一条路：命令 → 成功/失败文案 → 刷新运行态 + 刷新列表 +
  /// 通知宿主「行为变了，可能要重启/重新点火」。
  Future<void> _send(
    String action,
    String command,
    Map<String, Object?> args,
    String Function(ModCommandResult result) successText,
  ) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _message = null;
    });
    try {
      final ModCommandResult result = await widget.ctx.onCommand(command, args);
      if (!mounted) return;
      final String text = successText(result);
      setState(() {
        _busy = false;
        _messageIsError = false;
        _message = text;
      });
      widget.ctx.notifyChanged(text);
      await widget.ctx.onRefreshState();
      await _refreshList();
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _messageIsError = true;
        _message = memoryCommandErrorMessage(e, action);
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _messageIsError = true;
        _message = '$action失败：$e';
      });
    }
  }

  Future<void> _import() async {
    final String text = _importController.text.trim();
    if (text.isEmpty) {
      setState(() {
        _messageIsError = true;
        _message = '导入失败：请先输入要记住的内容';
      });
      return;
    }
    await _send('导入', 'import', <String, Object?>{
      'text': text,
    }, (ModCommandResult _) => '已导入一条记忆（下一条命中它的用户话，本轮就会带上）');
    if (mounted) _importController.clear();
  }

  Future<void> _edit(Map<String, Object?> record) async {
    final String id = (record['id'] as String?)?.trim() ?? '';
    // 用 `initialValue + onChanged` 而不是 controller：对话框退出动画期间
    // widget 仍在树上，此时 dispose controller 会让重建命中已释放的焦点节点
    // （真机上表现为一次诡异的 FocusScope 断言）。
    String draft = record['text'] is String ? record['text']! as String : '';
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: const Text('编辑记忆'),
        content: TextFormField(
          key: const Key('memory-edit-field'),
          initialValue: draft,
          maxLines: 4,
          autofocus: true,
          decoration: const InputDecoration(labelText: '记忆正文'),
          onChanged: (String value) => draft = value,
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(dialogContext, false),
            child: const Text('取消'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(dialogContext, true),
            child: const Text('保存'),
          ),
        ],
      ),
    );
    final String text = draft.trim();
    if (confirmed != true) return;
    if (text.isEmpty) {
      setState(() {
        _messageIsError = true;
        _message = '更新失败：正文不能为空';
      });
      return;
    }
    await _send('更新', 'update', <String, Object?>{
      'id': id,
      'text': text,
    }, (ModCommandResult _) => '已更新这条记忆（id 不变）');
  }

  Future<void> _delete(Map<String, Object?> record) async {
    final String id = (record['id'] as String?)?.trim() ?? '';
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: const Text('删除这条记忆？'),
        content: Text('删除后不可恢复：${memoryRecordText(record['text'])}'),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(dialogContext, false),
            child: const Text('取消'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(dialogContext, true),
            child: const Text('确认删除'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    await _send('删除', 'delete', <String, Object?>{
      'id': id,
    }, (ModCommandResult _) => '已删除这条记忆');
  }

  Future<void> _clear() async {
    await _send('清空', 'clear', const <String, Object?>{}, (
      ModCommandResult result,
    ) {
      final bool residue = result.result['residue'] == true;
      return '已清空记忆库：清掉 ${memoryCountText(result.result['removed'])} 条，'
          '现存 ${memoryCountText(result.result['records'])} 条'
          '${residue ? '。提示词里仍留着上一轮注入的记忆块，停用 Mod 会按既有语义剥离' : ''}';
    });
  }

  /// 「压缩过的记忆」块（2026-10-08 瘦身）：**有正文才画**，只画标题 + 正文。
  ///
  /// 拿掉了版本号、覆盖位点、桶占比、冷却状态与「回滚上一版摘要」——那一整块
  /// 是摘要子系统的内部状态，用户唯一会用的是「看看它压成了什么」，回滚则属于
  /// 排障（保留在 Mod 命令通道里，面板不摆按钮）。没有摘要就不画这块。
  Widget _summarySection(BuildContext context, ModPanelContext ctx) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final Map<String, Object?>? summary = memorySummaryState(ctx.state);
    if (summary == null) return const SizedBox.shrink();
    final String? body = memorySummaryText(summary);
    if (body == null) return const SizedBox.shrink();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text('压缩过的记忆', style: theme.textTheme.titleSmall),
        const SizedBox(height: Space.s1),
        Container(
          width: double.infinity,
          padding: const EdgeInsets.all(Space.s1),
          decoration: BoxDecoration(
            border: Border.all(color: colors.hairline),
            borderRadius: BorderRadius.circular(AppRadius.sm),
          ),
          child: Text(
            body,
            style: theme.textTheme.bodySmall,
            maxLines: 4,
            overflow: TextOverflow.ellipsis,
          ),
        ),
        const SizedBox(height: Space.s3),
      ],
    );
  }

  Widget _recordRow(
    BuildContext context,
    ModPanelContext ctx,
    Map<String, Object?> record,
  ) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final bool locked = !ctx.enabled || _busy;
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s1),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Expanded(
            child: Text(
              memoryRecordText(record['text']),
              style: theme.textTheme.bodySmall,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
          ),
          IconButton(
            key: Key('memory-edit-${record['id']}'),
            tooltip: '编辑',
            iconSize: 18,
            visualDensity: VisualDensity.compact,
            onPressed: locked ? null : () => unawaited(_edit(record)),
            icon: Icon(Icons.edit_outlined, color: colors.contentMuted),
          ),
          IconButton(
            key: Key('memory-delete-${record['id']}'),
            tooltip: '删除',
            iconSize: 18,
            visualDensity: VisualDensity.compact,
            onPressed: locked ? null : () => unawaited(_delete(record)),
            icon: Icon(Icons.delete_outline, color: colors.contentMuted),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final ModPanelContext ctx = widget.ctx;
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    // 「刷新」的职责是把列表拉回来：`list` 失败后也必须仍可点。
    // 导入 / 编辑 / 删除 / 清空则要求列表已到位（`_listLoading` 期间不动作）。
    final bool canRefresh = ctx.enabled && !_busy;
    final bool canAct = ctx.enabled && !_busy && !_listLoading;
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
          message: kMemoryScopeNotice,
          severity: NoticeSeverity.info,
          dense: true,
        ),
        const SizedBox(height: Space.s2),
        const SizedBox(height: Space.s3),

        // ---- 主动管理：导入一条 ----
        Text('导入一条', style: theme.textTheme.titleSmall),
        const SizedBox(height: Space.s1),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Expanded(
              child: TextField(
                key: const Key('memory-import-field'),
                controller: _importController,
                maxLines: 2,
                enabled: canAct,
                decoration: const InputDecoration(
                  hintText: '要记住的一句话',
                  isDense: true,
                  border: OutlineInputBorder(),
                ),
              ),
            ),
            const SizedBox(width: Space.s2),
            TextButton.icon(
              key: const Key('memory-import-button'),
              onPressed: canAct ? () => unawaited(_import()) : null,
              icon: const Icon(Icons.add, size: 16),
              label: const Text('导入一条'),
            ),
          ],
        ),
        const SizedBox(height: Space.s3),

        // ---- 主动管理：记忆列表 ----
        Row(
          children: <Widget>[
            Text('记忆列表', style: theme.textTheme.titleSmall),
            const SizedBox(width: Space.s2),
            if (_total != null)
              Text('共 ${memoryCountText(_total)} 条', style: muted),
            const Spacer(),
            TextButton.icon(
              key: const Key('memory-refresh-button'),
              onPressed: canRefresh ? () => unawaited(_refreshList()) : null,
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('刷新'),
            ),
          ],
        ),
        if (_bucket != null)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s1),
            child: Text('记忆库文件：$_bucket', style: muted),
          ),
        if (_listLoading)
          Text('正在读取记忆列表…', style: muted)
        else if (_listError != null)
          InlineNotice(message: _listError!, dense: true)
        else if (_records.isEmpty)
          Text('还没有记住任何内容——上面导入一条，或先聊一句。', style: muted)
        else
          ..._records.map(
            (Map<String, Object?> record) => _recordRow(context, ctx, record),
          ),
        const SizedBox(height: Space.s2),

        // ---- 真摘要（P1-5）：状态 + 回滚上一版 ----
        _summarySection(context, ctx),

        // ---- 清空 ----
        Align(
          alignment: Alignment.centerLeft,
          child: OutlinedButton.icon(
            onPressed: canAct ? () => unawaited(_clear()) : null,
            icon: _busy
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.delete_sweep_outlined, size: 16),
            label: Text(_busy ? '处理中…' : '清空记忆库'),
          ),
        ),
        if (!ctx.enabled)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: Text(
              'Mod 未启用：列表 / 导入 / 编辑 / 删除 / 清空都不可用——'
              '先在卡片标题行的开关里启用。',
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
      ],
    );
  }
}
