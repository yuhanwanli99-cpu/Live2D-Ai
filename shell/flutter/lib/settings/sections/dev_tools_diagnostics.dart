part of 'dev_tools_section.dart';

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
        ReadonlyField(label: '实时通道', icon: Icons.cable, text: wsStatusLabel),
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
          text:
              '${capabilities.app} ${capabilities.version}'
              '（schema v${capabilities.schemaVersion}，WS 协议 v${capabilities.wsProtocolVersion}）',
        ),
        ReadonlyField(
          label: '服务端音频后端',
          icon: Icons.speaker_outlined,
          text:
              '${(status['audio'] as Map<String, Object?>?)?['backend'] ?? '未知'}'
              '（sample_rate=${(status['audio'] as Map<String, Object?>?)?['sample_rate'] ?? '?'}）',
          description: 'Web 模式下音频单源是**浏览器**，服务端没有声卡是预期的',
        ),
        ReadonlyField(
          label: 'LLM / TTS 配置',
          icon: Icons.hub_outlined,
          text:
              'LLM: ${(llm as Map<String, Object?>?)?['model'] ?? '未配置'}'
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
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.contentMuted,
            ),
          )
        else if (logs.isEmpty)
          Text(
            '（暂无日志）',
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.contentMuted,
            ),
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

