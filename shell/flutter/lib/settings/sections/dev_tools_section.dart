/// 「Mod」「诊断」「模型库」「开发模式」四个分区的实现。
///
/// 合在一个文件里：它们共享同一套「列表 + 动作 + 内联结果」的骨架，
/// 拆成四个文件只会让同一段列表渲染代码出现四遍。每个类都短、
/// 职责单一，找起来靠类名即可。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../api/diagnostics_api.dart';
import '../../api/models_api.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../ui/emphasized_text.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../../ui/inline_notice.dart';
import '../../ui/theme.dart';

/// 列表里的一行（四个分区共用）。
class AdminRow extends StatelessWidget {
  const AdminRow({
    required this.title,
    required this.subtitle,
    this.badges = const <String>[],
    this.trailing,
    super.key,
  });

  final String title;
  final String subtitle;

  /// 文字徽标（**不靠颜色表意**）。
  final List<String> badges;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: Space.s2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(title, style: theme.textTheme.titleSmall),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: colors.contentMuted,
                  ),
                ),
                if (badges.isNotEmpty) ...<Widget>[
                  const SizedBox(height: Space.s1),
                  Wrap(
                    spacing: Space.s1,
                    children: <Widget>[
                      for (final String b in badges)
                        DecoratedBox(
                          decoration: BoxDecoration(
                            // 槽位从 ColorScheme 取（AppColors 里没有这一项）。
                            color: Theme.of(context)
                                .colorScheme
                                .surfaceContainerHighest,
                            borderRadius: BorderRadius.circular(AppRadius.xs),
                          ),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: Space.s2,
                              vertical: 1,
                            ),
                            child: Text(b, style: theme.textTheme.labelSmall),
                          ),
                        ),
                    ],
                  ),
                ],
              ],
            ),
          ),
          ?trailing,
        ],
      ),
    );
  }
}

/// 空态（**空白面板会被读成「坏了」**）。
class AdminEmpty extends StatelessWidget {
  const AdminEmpty({required this.icon, required this.title, required this.hint, super.key});

  final IconData icon;
  final String title;
  final String hint;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: Space.s5),
      child: Column(
        children: <Widget>[
          Icon(icon, size: 28, color: appColorsOf(context).contentFaint),
          const SizedBox(height: Space.s2),
          Text(title, style: theme.textTheme.titleSmall),
          const SizedBox(height: Space.s1),
          Text(
            hint,
            textAlign: TextAlign.center,
            style: theme.textTheme.bodySmall?.copyWith(
              color: appColorsOf(context).contentMuted,
            ),
          ),
        ],
      ),
    );
  }
}

// ────────────────────────────────────────────────────────────── 模型库

class ModelsSection extends StatelessWidget {
  const ModelsSection({
    required this.models,
    required this.loading,
    this.error,
    this.devMode = false,
    this.onActivate,
    this.onReload,
    this.onImport,
    this.busyId,
    this.activateMessage,
    super.key,
  });

  final List<ModelInfo> models;
  final bool loading;
  final String? error;
  final bool devMode;
  final Future<void> Function(String id)? onActivate;
  final Future<void> Function()? onReload;

  /// 导入一个**已在磁盘上**的模型目录（`assets/models/<id>/`）。
  ///
  /// 后端不做文件上传（ZIP 上传是后置项且**曾被我方能力快照谎报为 supported**）：
  /// 用户把模型放进目录，这里登记 + 校验。没有这个入口，「导入 → 激活 → 换皮」
  /// 这条 DoD 主路径在界面上就没有起点。
  final Future<void> Function(String id)? onImport;

  /// 正在激活的模型 id（禁用该行按钮，防重复提交）。
  final String? busyId;
  final String? activateMessage;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '模型库',
          description: '模型由你合法导入，本仓库不捆绑任何模型二进制。',
        ),
        if (error != null)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s2),
            // 同上：统一走 `InlineNotice`（2026-09-11，P1-5）。
            child: InlineNotice(message: error!),
          ),
        if (activateMessage != null)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s2),
            child: EmphasizedText(
              activateMessage!,
              style: theme.textTheme.bodySmall,
            ),
          ),
        if (loading && models.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: Space.s4),
            child: Center(child: CircularProgressIndicator()),
          )
        else if (models.isEmpty)
          const AdminEmpty(
            icon: Icons.view_in_ar_outlined,
            title: '还没有导入模型',
            hint: '把模型放到 assets/models/<id>/ 下，然后在下面填目录名导入。'
                '列表为空是正常的——项目刻意不捆绑模型。',
          )
        else
          for (final ModelInfo m in models)
            AdminRow(
              title: m.displayName.isEmpty ? m.id : m.displayName,
              subtitle: '${m.id} · v${m.version} · ${m.humanSize} · '
                  '${m.textureCount} 张贴图',
              badges: <String>[
                if (m.active) '当前激活',
                if (m.hasPhysics) '含物理',
                if (m.hasDisplayInfo) '有显示配置',
              ],
              trailing: m.active
                  ? null
                  : FilledButton.tonal(
                      onPressed: busyId != null || onActivate == null
                          ? null
                          : () => onActivate!(m.id),
                      child: Text(busyId == m.id ? '激活中…' : '激活'),
                    ),
            ),
        if (onImport != null) ...<Widget>[
          const SizedBox(height: Space.s3),
          _ImportModelField(onImport: onImport!, busy: busyId != null),
        ],
        if (onReload != null) ...<Widget>[
          const SizedBox(height: Space.s3),
          Align(
            alignment: Alignment.centerLeft,
            child: OutlinedButton.icon(
              onPressed: () => onReload!(),
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('重新载入列表'),
            ),
          ),
        ],
        if (devMode)
          const Padding(
            padding: EdgeInsets.only(top: Space.s3),
            child: Text(
              '开发者提示：导入只接受 assets/models/ 下已存在的目录名；'
              '删除激活中的模型会被服务端拒绝（409 model_active）。',
            ),
          ),
      ],
    );
  }
}

/// 「导入」输入行：填 `assets/models/` 下的目录名 → `POST /models/import`。
///
/// 有状态只是为了拿住 `TextEditingController`；导入本身由宿主执行
/// （它负责刷新列表与提示），成功后清空输入框。
class _ImportModelField extends StatefulWidget {
  const _ImportModelField({required this.onImport, required this.busy});

  final Future<void> Function(String id) onImport;
  final bool busy;

  @override
  State<_ImportModelField> createState() => _ImportModelFieldState();
}

class _ImportModelFieldState extends State<_ImportModelField> {
  final TextEditingController _id = TextEditingController();

  @override
  void dispose() {
    _id.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final String id = _id.text.trim();
    if (id.isEmpty) return;
    await widget.onImport(id);
    if (mounted) _id.clear();
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      children: <Widget>[
        Expanded(
          child: TextField(
            controller: _id,
            enabled: !widget.busy,
            decoration: const InputDecoration(
              isDense: true,
              labelText: '导入模型（目录名）',
              hintText: '例如 bai',
            ),
            onSubmitted: (_) => unawaited(_submit()),
          ),
        ),
        const SizedBox(width: Space.s2),
        FilledButton.tonal(
          onPressed: widget.busy ? null : () => unawaited(_submit()),
          child: const Text('导入'),
        ),
      ],
    );
  }
}

// ────────────────────────────────────────────────────────────── Mod

class ModsSection extends StatelessWidget {
  const ModsSection({
    required this.mods,
    required this.loading,
    this.error,
    this.onToggle,
    this.onReload,
    this.busyId,
    super.key,
  });

  final List<ModInfo> mods;
  final bool loading;
  final String? error;
  final Future<void> Function(String id, bool enabled)? onToggle;
  final Future<void> Function()? onReload;
  final String? busyId;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: 'Mod',
          description: '扩展能力走 Mod 边界隔离，默认全部停用。'
              '核心只提供接口，不把功能堆进来。',
        ),
        if (error != null)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s2),
            // 同上：统一走 `InlineNotice`（2026-09-11，P1-5）。
            child: InlineNotice(message: error!),
          ),
        if (loading && mods.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: Space.s4),
            child: Center(child: CircularProgressIndicator()),
          )
        else if (mods.isEmpty)
          const AdminEmpty(
            icon: Icons.extension_outlined,
            title: '没有注册任何 Mod',
            hint: 'Mod 通过 live2d-ai-mod-system 的 trait 注册中心接入。',
          )
        else
          for (final ModInfo m in mods)
            AdminRow(
              title: m.name.isEmpty ? m.id : m.name,
              subtitle: '${m.id} · v${m.version} · api v${m.apiVersion}',
              // **状态用文字**（「运行中」/「已停用」），不靠颜色。
              badges: <String>[m.statusLabel],
              trailing: Switch(
                value: m.enabled,
                onChanged: busyId != null || onToggle == null
                    ? null
                    : (bool v) => onToggle!(m.id, v),
              ),
            ),
        if (onReload != null) ...<Widget>[
          const SizedBox(height: Space.s3),
          Align(
            alignment: Alignment.centerLeft,
            child: OutlinedButton.icon(
              onPressed: () => onReload!(),
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('重新载入'),
            ),
          ),
        ],
      ],
    );
  }
}

// ────────────────────────────────────────────────────────────── 诊断

/// 诊断快照（一键复制用）。
class DiagnosticsSnapshot {
  const DiagnosticsSnapshot({required this.lines});

  final List<String> lines;

  String get text => lines.join('\n');
}

class DiagnosticsSection extends StatelessWidget {
  const DiagnosticsSection({
    required this.status,
    required this.capabilities,
    required this.wsStatusLabel,
    this.logs = const <LogLine>[],
    this.logsError,
    this.devMode = false,
    this.onReload,
    this.onCopy,
    this.copied = false,
    super.key,
  });

  final Map<String, Object?> status;
  final AppCapabilities capabilities;
  final String wsStatusLabel;
  final List<LogLine> logs;
  final String? logsError;
  final bool devMode;
  final Future<void> Function()? onReload;
  final Future<void> Function(DiagnosticsSnapshot)? onCopy;
  final bool copied;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final Object? llm = status['llm'];
    final Object? tts = status['tts'];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '诊断',
          description: '当前连接、代次与能力快照。**只读**——这里没有会改状态的按钮。',
        ),
        ReadonlyField(
          label: '实时通道',
          icon: Icons.cable,
          text: wsStatusLabel,
        ),
        ReadonlyField(
          label: '激活模型',
          icon: Icons.view_in_ar_outlined,
          text: '${status['active_model_id'] ?? '（未设置）'}',
        ),
        ReadonlyField(
          label: '当前代次（epoch）',
          icon: Icons.tag,
          text: '${status['current_epoch'] ?? 0}',
          description: '服务端推进 epoch = 上一轮作废（停止 / 抢占）',
        ),
        ReadonlyField(
          label: '服务端版本',
          icon: Icons.info_outline,
          text: '${capabilities.app} ${capabilities.version}'
              '（schema v${capabilities.schemaVersion}，WS 协议 v${capabilities.wsProtocolVersion}）',
        ),
        ReadonlyField(
          label: '服务端音频后端',
          icon: Icons.speaker_outlined,
          text: '${(status['audio'] as Map<String, Object?>?)?['backend'] ?? '未知'}'
              '（sample_rate=${(status['audio'] as Map<String, Object?>?)?['sample_rate'] ?? '?'}）',
          description: 'Web 模式下音频单源是**浏览器**，服务端没有声卡是预期的',
        ),
        ReadonlyField(
          label: 'LLM / TTS 配置',
          icon: Icons.hub_outlined,
          text: 'LLM: ${(llm as Map<String, Object?>?)?['model'] ?? '未配置'}'
              '；TTS: ${(tts as Map<String, Object?>?)?['base_url'] ?? '未配置'}',
        ),
        const SizedBox(height: Space.s3),
        Row(
          children: <Widget>[
            OutlinedButton.icon(
              onPressed: onReload == null ? null : () => onReload!(),
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('刷新'),
            ),
            const SizedBox(width: Space.s2),
            FilledButton.tonalIcon(
              onPressed: onCopy == null
                  ? null
                  : () => onCopy!(DiagnosticsSnapshot(lines: _snapshotLines())),
              icon: Icon(copied ? Icons.check : Icons.copy, size: 16),
              label: Text(copied ? '已复制' : '复制诊断快照'),
            ),
          ],
        ),
        const SizedBox(height: Space.s3),
        const SectionHeader(
          title: '日志',
          description: '需要先打开「开发模式」；只读最后一屏，不做实时跟随。',
        ),
        if (logsError != null)
          Text(
            logsError!,
            style: theme.textTheme.bodySmall?.copyWith(color: colors.contentMuted),
          )
        else if (logs.isEmpty)
          Text(
            '（暂无日志）',
            style: theme.textTheme.bodySmall?.copyWith(color: colors.contentMuted),
          )
        else
          // 必须虚拟化：日志是长期运行会累积的东西。
          SizedBox(
            height: 220,
            child: ListView.builder(
              itemExtent: 20,
              itemCount: logs.length,
              itemBuilder: (BuildContext context, int i) => Text(
                logs[logs.length - 1 - i].asText,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall,
              ),
            ),
          ),
      ],
    );
  }

  /// 快照内容：**把排查需要的东西一次给全**，而不是让用户来回截图。
  ///
  /// **绝不包含密钥**（前端本来也拿不到）；只含可公开的连接与版本信息。
  List<String> _snapshotLines() => <String>[
    '# Live2D Ai 诊断快照',
    'ws=$wsStatusLabel',
    'app=${capabilities.app} version=${capabilities.version} '
        'schema=${capabilities.schemaVersion} ws_proto=${capabilities.wsProtocolVersion}',
    'active_model=${status['active_model_id'] ?? ''}',
    'epoch=${status['current_epoch'] ?? 0}',
    'uptime_s=${status['uptime_s'] ?? '?'}',
    'audio=${status['audio']}',
    'llm=${status['llm']}',
    'tts=${status['tts']}',
    'dev_mode=${status['dev_mode']}',
  ];
}

/// 复制到剪贴板（**复制是唯一的导出方式**，v1 不做上传）。
Future<void> copySnapshot(DiagnosticsSnapshot snapshot) =>
    Clipboard.setData(ClipboardData(text: snapshot.text));

// ────────────────────────────────────────────────────────── 开发模式

class DeveloperSection extends StatelessWidget {
  const DeveloperSection({
    required this.devMode,
    required this.onDevModeChanged,
    required this.forcedByLaunchFlag,
    super.key,
  });

  final bool devMode;
  final ValueChanged<bool> onDevModeChanged;

  /// 是否由启动参数（`--dev-mode`）强制开启。
  ///
  /// **不谎报成功**：强制开启时开关要显示为「已由启动参数开启」且不可关，
  /// 而不是让用户点一下、看起来关了、其实没关（规格 §13.1-11）。
  final bool forcedByLaunchFlag;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '开发模式',
          description: '打开后才显示高级参数、日志与诊断细节（渐进披露的第二层）。',
        ),
        ToggleField(
          label: '开发者模式',
          icon: Icons.terminal_outlined,
          value: devMode,
          enabled: !forcedByLaunchFlag,
          description: forcedByLaunchFlag
              ? '当前由启动参数强制开启，无法在界面里关闭'
              // 文案必须与**实际行为**一致（2026-09-11 修）：分区本身**不再**
              // 随 dev_mode 隐藏——藏起来就没人能再打开它。
              : '关闭后各分区里的「开发者选项」与诊断细节一起隐藏；'
                    '**本分区始终可见**，否则就再也打不开它了',
          onChanged: onDevModeChanged,
        ),
      ],
    );
  }
}
