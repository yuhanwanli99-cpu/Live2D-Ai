/// 舞台上的常驻动作浮标（L3，规格 §5.2）：`≤8` 槽，先放 3 个高频。
///
/// 与 [ActionGrid] 的分工：工具条是**不打开设置就能点**的那几个；
/// 网格是全集。同一套 `ActionDispatch` 驱动忙碌态，所以两处的「正在演」
/// 永远一致（不会出现「网格里在演、工具条里是空闲」）。
///
/// 浮在舞台**右下角**、半透明：舞台是主角（原则 P1），工具条是客人。
library;

import 'package:flutter/material.dart';

import '../actions/action_dispatch.dart';
import '../api/commands_api.dart';
import '../design/tokens.dart';
import 'theme.dart';

class ActionToolbar extends StatelessWidget {
  const ActionToolbar({
    required this.commands,
    required this.onInvoke,
    required this.dispatch,
    this.maxSlots = 8,
    super.key,
  });

  /// **已按高频顺序排好**的动作（调用方决定顺序，组件不做业务排序）。
  final List<CommandInfo> commands;
  final void Function(CommandInfo command) onInvoke;
  final ActionDispatch dispatch;
  final int maxSlots;

  @override
  Widget build(BuildContext context) {
    final List<CommandInfo> shown = commands.take(maxSlots).toList();
    if (shown.isEmpty) return const SizedBox.shrink();
    // 同 `ActionGrid`：自己订阅忙碌态，不指望调用方包 ListenableBuilder。
    return ListenableBuilder(
      listenable: dispatch,
      builder: (BuildContext context, Widget? _) => _buildBar(context, shown),
    );
  }

  Widget _buildBar(BuildContext context, List<CommandInfo> shown) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);

    return Padding(
      padding: const EdgeInsets.all(Space.s3),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: colors.glassScrim,
          borderRadius: BorderRadius.circular(AppRadius.pill),
          border: Border.all(color: colors.hairline),
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: Space.s2,
            vertical: Space.s1,
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              for (int i = 0; i < shown.length; i++) ...<Widget>[
                if (i > 0) const SizedBox(width: Space.s1),
                _ToolbarButton(
                  command: shown[i],
                  busy: shown[i].targetAction != null &&
                      dispatch.isBusy(shown[i].targetAction!),
                  onPressed: () => onInvoke(shown[i]),
                  theme: theme,
                  colors: colors,
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _ToolbarButton extends StatelessWidget {
  const _ToolbarButton({
    required this.command,
    required this.busy,
    required this.onPressed,
    required this.theme,
    required this.colors,
  });

  final CommandInfo command;
  final bool busy;
  final VoidCallback onPressed;
  final ThemeData theme;
  final AppColors colors;

  @override
  Widget build(BuildContext context) {
    final String label = command.name.isEmpty
        ? (command.targetAction ?? command.id)
        : command.name;
    return Tooltip(
      message: command.description.isEmpty ? label : command.description,
      child: TextButton(
        onPressed: onPressed,
        style: TextButton.styleFrom(
          visualDensity: VisualDensity.compact,
          backgroundColor: busy ? theme.colorScheme.primaryContainer : null,
          padding: const EdgeInsets.symmetric(horizontal: Space.s2),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            if (busy)
              SizedBox(
                width: 12,
                height: 12,
                child: CircularProgressIndicator(
                  strokeWidth: 2,
                  color: theme.colorScheme.onPrimaryContainer,
                ),
              )
            else
              Icon(Icons.play_arrow_outlined, size: 14, color: colors.contentMuted),
            const SizedBox(width: Space.s1),
            Text(label, style: theme.textTheme.labelLarge),
          ],
        ),
      ),
    );
  }
}

/// 舞台角标三键：缩小 / 放大 / 复位（规格 §5.2 的「舞台角标」）。
///
/// # 为什么放大缩小是「方向」而不是「数值」
///
/// 渲染面**自己算**缩放：`scale * 1.1` 或 `/ 1.1`，clamp 到 `0.5..2.0`，
/// `reset` 还会把偏移归零（`l2d-wasm-demo/src/main.rs:346-357`）。
/// 所以父页发的是 `dir: in|out|reset`，**不是**目标缩放值。
///
/// **应用后的真值要以 `stage-ack` 为准**（`scale`/`offset_x`/`offset_y`）——
/// 用本地值猜会和渲染面漂移（比如已经到 2.0 上限时，本地以为又放大了 10%）。
class StageCornerControls extends StatelessWidget {
  const StageCornerControls({
    required this.onZoom,
    this.scaleFromAck,
    super.key,
  });

  /// `in` / `out` / `reset`。
  final void Function(String dir) onZoom;

  /// 渲染面回报的实际缩放（收到 ack 之前为 `null` → **不显示百分比**，
  /// 而不是显示一个猜的值）。
  final double? scaleFromAck;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    return Padding(
      padding: const EdgeInsets.all(Space.s3),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: colors.glassScrim,
          borderRadius: BorderRadius.circular(AppRadius.pill),
          border: Border.all(color: colors.hairline),
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: Space.s1),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              IconButton(
                onPressed: () => onZoom('out'),
                tooltip: '缩小',
                icon: const Icon(Icons.remove, size: 18),
                visualDensity: VisualDensity.compact,
              ),
              SizedBox(
                width: 44,
                child: Text(
                  // ack 之前不显示数值（**不猜**）。
                  scaleFromAck == null
                      ? '—'
                      : '${(scaleFromAck! * 100).round()}%',
                  textAlign: TextAlign.center,
                  style: Theme.of(context).textTheme.labelSmall?.copyWith(
                    color: colors.contentMuted,
                  ),
                ),
              ),
              IconButton(
                onPressed: () => onZoom('in'),
                tooltip: '放大',
                icon: const Icon(Icons.add, size: 18),
                visualDensity: VisualDensity.compact,
              ),
              IconButton(
                onPressed: () => onZoom('reset'),
                tooltip: '复位缩放与位置',
                icon: const Icon(Icons.center_focus_strong_outlined, size: 18),
                visualDensity: VisualDensity.compact,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
