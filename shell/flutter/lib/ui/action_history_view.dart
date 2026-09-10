/// 动作日志视图（L3）：`[时间] [动作中文名] [强度 Ⅰ/Ⅱ/Ⅲ] [来源徽标] [epoch]`。
///
/// 规格 §8.3「三处呈现」的第 1 处。
///
/// # 两条实现约束
///
/// 1. **必须虚拟化**：日志是长期运行会累积的东西。用 `ListView.builder` +
///    `itemExtent`，构建成本是 O(可见行) 而不是 O(条目数)。
/// 2. **空态要说话**：空白面板会被误读成「坏了」。空态明确写「还没有动作」，
///    并说明动作从哪来（你点的 / AI 触发的）。
library;

import 'package:flutter/material.dart';

import '../actions/action_history.dart';
import '../design/tokens.dart';
import 'action_source_badge.dart';

/// 一行的高度。写死是为了 `itemExtent` —— 虚拟化的前提。
const double kActionHistoryRowExtent = 44;

/// 动作日志。
class ActionHistoryView extends StatelessWidget {
  const ActionHistoryView({
    required this.entries,
    this.current,
    this.onClear,
    this.droppedCount = 0,
    super.key,
  });

  final List<ActionHistoryEntry> entries;

  /// 正在表演的那一条（用于高亮）。
  final ActionHistoryEntry? current;

  /// 清空回调；`null` 表示不显示清空按钮。
  final VoidCallback? onClear;

  /// 因超出上限被丢弃的条目数（>0 时显示提示，**不静默**）。
  final int droppedCount;

  @override
  Widget build(BuildContext context) {
    final TextTheme text = Theme.of(context).textTheme;

    if (entries.isEmpty) {
      return const _EmptyState();
    }

    // 最新的在最上面（日志的常规约定）。倒序只反转索引，不复制列表。
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.fromLTRB(Space.s3, Space.s2, Space.s1, Space.s2),
          child: Row(
            children: <Widget>[
              Expanded(
                child: Text(
                  droppedCount > 0
                      ? '动作日志（${entries.length} 条，已丢弃 $droppedCount 条旧记录）'
                      : '动作日志（${entries.length} 条）',
                  style: text.labelLarge,
                ),
              ),
              if (onClear != null)
                TextButton(
                  onPressed: onClear,
                  child: const Text('清空'),
                ),
            ],
          ),
        ),
        Expanded(
          child: ListView.builder(
            itemExtent: kActionHistoryRowExtent,
            itemCount: entries.length,
            // 倒序索引：最后一条（最新）在最上面。
            itemBuilder: (BuildContext context, int index) {
              final ActionHistoryEntry entry = entries[entries.length - 1 - index];
              return _HistoryRow(
                entry: entry,
                highlighted: identical(entry, current),
              );
            },
          ),
        ),
      ],
    );
  }
}

class _HistoryRow extends StatelessWidget {
  const _HistoryRow({required this.entry, required this.highlighted});

  final ActionHistoryEntry entry;
  final bool highlighted;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final TextTheme text = theme.textTheme;
    final ColorScheme scheme = theme.colorScheme;

    return Semantics(
      label: entry.semanticLabel,
      excludeSemantics: true,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: Space.s3, vertical: Space.s1),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: highlighted ? scheme.surfaceContainerHighest : null,
            borderRadius: BorderRadius.circular(AppRadius.sm),
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: Space.s2),
            child: Row(
              children: <Widget>[
                // 时间（本地收到时刻）。服务端 ts 与浏览器时钟不可比，故用本地。
                SizedBox(
                  width: 62,
                  child: Text(
                    _hhmmss(entry.at),
                    style: text.bodySmall?.copyWith(
                      color: scheme.onSurfaceVariant,
                      fontFeatures: const <FontFeature>[
                        FontFeature.tabularFigures(),
                      ],
                    ),
                  ),
                ),
                Expanded(
                  child: Text(
                    entry.label,
                    overflow: TextOverflow.ellipsis,
                    style: text.bodyMedium,
                  ),
                ),
                if (entry.strengthText != null)
                  Padding(
                    padding: const EdgeInsets.only(right: Space.s2),
                    child: Text(
                      entry.strengthText!,
                      style: text.labelLarge?.copyWith(
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                  ),
                ActionSourceBadge(source: entry.source, dense: true),
                const SizedBox(width: Space.s2),
                Text(
                  'e${entry.epoch}',
                  style: text.labelSmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(Space.s5),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(
              Icons.history_toggle_off,
              size: 28,
              color: theme.colorScheme.onSurfaceVariant,
            ),
            const SizedBox(height: Space.s2),
            Text('还没有动作', style: theme.textTheme.titleSmall),
            const SizedBox(height: Space.s1),
            Text(
              '你点动作、或 AI 自己触发动作后，会在这里留下记录——'
              '包括「是谁触发的」。',
              textAlign: TextAlign.center,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// `HH:mm:ss`；缺失时刻显示占位（**不显示 "null"**）。
String _hhmmss(DateTime? at) {
  if (at == null) return '--:--:--';
  String two(int v) => v.toString().padLeft(2, '0');
  return '${two(at.hour)}:${two(at.minute)}:${two(at.second)}';
}
