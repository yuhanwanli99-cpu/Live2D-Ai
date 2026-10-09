/// `memory` 的产品面板（产品级加强波次 + L1 主动管理面）。
///
/// 职责：概览一行 / **记忆列表（查看 · 编辑 · 删除）** / **导入一条** /
/// 清空整库 / 会话分桶的降级说明；注入可关与 persona 的策略在 UI 里说清。
/// 本文件由 memory 轨道独占，其他轨道不要改。
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

/// 记忆放在哪的说明（2026-10-08：说人话，不出现「桶」）。纯函数，可单测。
///
/// `activeSessionId == null` 时必须**说清降级**，不能假装绑好了：这时记的东西
/// 先汇总在一起，等有会话了再按会话分开。
String memoryBucketNotice(String? activeSessionId) {
  final String id = activeSessionId?.trim() ?? '';
  if (id.isEmpty) {
    return '还没有对话。记住的内容先放到一起，开始聊天后再按这次对话分开。';
  }
  return '只看这次对话记住的内容。';
}

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
  return raw
      .whereType<Map<Object?, Object?>>()
      .map((Map<Object?, Object?> m) {
        final Map<String, Object?> out = <String, Object?>{};
        m.forEach((Object? k, Object? v) {
          if (k is String) out[k] = v;
        });
        return out;
      })
      .toList();
}

/// 命令 args：可选的 `session_id` **只在有会话时才带**。
///
/// 与 Rust 侧口径逐字对齐：`args.session_id` 缺省 → 老路径全局桶。面板在
/// `activeSessionId == null` 时不传，正是「全局桶降级」那句文案的行为。
Map<String, Object?> memoryCommandArgs({
  String? sessionId,
  Map<String, Object?> extra = const <String, Object?>{},
}) {
  final Map<String, Object?> args = <String, Object?>{...extra};
  final String id = sessionId?.trim() ?? '';
  if (id.isNotEmpty) args['session_id'] = id;
  return args;
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
    'session_scoped': '会话分桶',
    'active_session': '当前会话',
    'bucket_path': '生效记忆库',
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

  /// **当前列表里的记录属于哪个会话桶**（F-0004-1）。
  ///
  /// 它不是「当前会话」的同义词：切会话时宿主立刻用新 `ctx` 重建本面板
  /// （文案那行现读 `ctx.activeSessionId`，一帧就改口），而列表是异步来的。
  /// 两者一旦分叉，行内编辑/删除就会拿着**桶 A 的 id** 去 `ctx` 说的**桶 B**
  /// 里操作——改错桶的数据。所以：
  ///   · 所有行为的 `session_id` 一律取这份**快照**（记录的家）；
  ///   · 快照与 `ctx.activeSessionId` 不一致期间（切桶的取数窗口），
  ///     列表先清空、按钮先失效（见 [_bucketStale]）——旧桶的行**点不到**。
  late String? _recordsSessionId = widget.ctx.activeSessionId;

  /// 列表取数的**代**：迟到的旧桶响应必须丢掉，否则它会把刚切过去的
  /// 面板重新填回旧桶的内容。
  int _listEpoch = 0;

  /// 展示中的数据与「当前会话」是否已经分叉（切桶取数窗口 / 取数失败）。
  bool get _bucketStale => _recordsSessionId != widget.ctx.activeSessionId;

  @override
  void initState() {
    super.initState();
    // 进入面板就拉一次列表；不在 initState 里 setState（首帧还没建）。
    unawaited(_refreshList(initial: true));
  }

  @override
  void didUpdateWidget(_MemoryPanelBody oldWidget) {
    super.didUpdateWidget(oldWidget);
    // **会话桶换了 → 列表必须跟着换**（F-0004-1）。
    //
    // 从前这里没有 didUpdateWidget：State 被复用、`_records` 停在旧桶，
    // 而 build 里的桶说明行现读 `ctx.activeSessionId` ⇒ 界面自相矛盾
    // （文案说 B、列表是 A），接着「删除」就会拿 A 的行 id 去 B 里删。
    //
    // 现在：先把旧桶的内容清掉（避免分叉窗口里任何 id 还能被点到），
    // 再为新桶取数。取数期间 `_recordsSessionId` 仍是旧桶 ⇒ [_bucketStale]
    // 为真 ⇒ 按钮失效；新桶的响应落地后一切复原。
    if (oldWidget.ctx.activeSessionId != widget.ctx.activeSessionId) {
      // **只改字段**（didUpdateWidget 之后框架本来就会重建一次，这里 setState
      // 是多余的，还会在「父级 build 期间」留下一次额外标记）。
      _records = const <Map<String, Object?>>[];
      _total = null;
      _bucket = null;
      _listError = null;
      _listLoading = true;
      _message = null;
      // 取数走 `initial: true`：它**不在 await 之前** setState，响应落地才更新。
      unawaited(_refreshList(initial: true));
    }
  }

  @override
  void dispose() {
    _importController.dispose();
    super.dispose();
  }

  Future<void> _refreshList({bool initial = false}) async {
    final int epoch = ++_listEpoch;
    // 这一次取数**为哪个桶**取。响应落地时也只有它才算数。
    final String? requested = widget.ctx.activeSessionId;
    if (!initial && mounted) {
      setState(() {
        _listLoading = true;
        _listError = null;
      });
    }
    try {
      final ModCommandResult result = await widget.ctx.onCommand(
        'list',
        memoryCommandArgs(
          sessionId: requested,
          extra: <String, Object?>{'limit': kMemoryListLimit},
        ),
      );
      // 迟到的旧桶响应：丢掉（否则刚切过去的桶会被旧桶内容重新填满）。
      if (!mounted || epoch != _listEpoch) return;
      setState(() {
        _listLoading = false;
        _listError = null;
        _records = memoryParseRecords(result.result['records']);
        _total = result.result['total'];
        final Object? bucket = result.result['bucket'];
        _bucket = bucket is String ? bucket : null;
        // 列表现已属于这个桶 —— 快照在这里才更新。
        _recordsSessionId = requested;
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
    await _send(
      '导入',
      'import',
      memoryCommandArgs(
        sessionId: _recordsSessionId,
        extra: <String, Object?>{'text': text},
      ),
      (ModCommandResult _) => '已导入一条记忆（下一条命中它的用户话，本轮就会带上）',
    );
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
    await _send(
      '更新',
      'update',
      memoryCommandArgs(
        sessionId: _recordsSessionId,
        extra: <String, Object?>{'id': id, 'text': text},
      ),
      (ModCommandResult _) => '已更新这条记忆（id 不变）',
    );
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
    await _send(
      '删除',
      'delete',
      memoryCommandArgs(
        sessionId: _recordsSessionId,
        extra: <String, Object?>{'id': id},
      ),
      (ModCommandResult _) => '已删除这条记忆',
    );
  }

  Future<void> _clear() async {
    await _send(
      '清空',
      'clear',
      memoryCommandArgs(sessionId: _recordsSessionId),
      (ModCommandResult result) {
        final bool residue = result.result['residue'] == true;
        return '已清空记忆库：清掉 ${memoryCountText(result.result['removed'])} 条，'
            '现存 ${memoryCountText(result.result['records'])} 条'
            '${residue ? '。提示词里仍留着上一轮注入的记忆块，停用 Mod 会按既有语义剥离' : ''}';
      },
    );
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
    final bool locked = !ctx.enabled || _busy || _bucketStale;
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
    // 切桶取数窗口里一律不准动（F-0004-1）：下面那几行可能还是旧桶的
    // 记录 id，而 `ctx` 已经改成新会话了 —— 放行就是跨桶写。
    //
    // 「刷新」**不看** `_bucketStale`（2026-10-08）：它的职责正是把当前桶的
    // 列表拉回来，锁上它等于「切桶失败后没有出路」；`list` 失败后也必须仍
    // 可点。导入 / 编辑 / 删除 / 清空才需要「快照 == 当前会话且不在读取中」。
    final bool canRefresh = ctx.enabled && !_busy;
    final bool canAct = ctx.enabled && !_busy && !_bucketStale && !_listLoading;
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
        InlineNotice(
          message: memoryBucketNotice(ctx.activeSessionId),
          severity: ctx.activeSessionId == null
              ? NoticeSeverity.warning
              : NoticeSeverity.info,
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
            if (_total != null) Text('共 ${memoryCountText(_total)} 条', style: muted),
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
            child: Text('生效记忆库：$_bucket', style: muted),
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
            child: Text('Mod 未启用：列表 / 导入 / 编辑 / 删除 / 清空都不可用——'
                '先在卡片标题行的开关里启用。', style: muted),
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
