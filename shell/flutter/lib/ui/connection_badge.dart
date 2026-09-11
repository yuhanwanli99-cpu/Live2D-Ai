/// WS 连接状态徽标（L3）。
///
/// 规格 §6.4：**图标 + 文字 + 语义标签**，5 态文案固定，
/// **不靠颜色单独表意**——色盲用户无法只凭颜色区分 5 个状态。
///
/// 「连接态常驻可见」是硬规则：同类项目里有把连接态用
/// `display:none !important` 主动隐藏的（N.E.K.O），结果是用户永远不知道
/// 后端掉线了【调研 survey §3.4】。这里永远在 AppBar 上。
library;

import 'package:flutter/material.dart';

import '../api/ws_status.dart';
import '../design/tokens.dart';
import 'theme.dart';

class ConnectionBadge extends StatelessWidget {
  const ConnectionBadge({required this.status, this.onRetry, super.key});

  final WsStatus status;

  /// 可点的重试（规格 §6.5-4：横幅必须是可点的重试，不是纯文本）。
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    final TextTheme text = Theme.of(context).textTheme;

    final (IconData icon, Color tone) = switch (status) {
      WsStatus.connected => (Icons.cloud_done_outlined, appPaletteOf(context).success),
      WsStatus.connecting => (Icons.cloud_sync_outlined, appPaletteOf(context).warning),
      WsStatus.disconnected => (Icons.cloud_off, appPaletteOf(context).danger),
      WsStatus.closed => (Icons.cloud_off, appPaletteOf(context).danger),
      WsStatus.idle => (Icons.cloud_queue, colors.contentMuted),
    };

    final Widget content = Semantics(
      label: '实时通道：${status.description}',
      button: status.isProblem && onRetry != null,
      excludeSemantics: true,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Icon(icon, size: 16, color: tone),
          const SizedBox(width: Space.s1),
          Text(
            status.label,
            style: text.labelLarge?.copyWith(color: tone),
          ),
        ],
      ),
    );

    if (status.isProblem && onRetry != null) {
      return TextButton(
        onPressed: onRetry,
        child: content,
      );
    }
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: Space.s2),
      child: content,
    );
  }
}

/// 舞台顶部的离线横幅（规格 §6.3 的 `offline` 行）。
///
/// 与 [ConnectionBadge] 的分工：徽标是**常驻小字**，横幅是**首次明显提示**。
/// 两者都在，不是重复——横幅会消失，徽标不会。
class OfflineBanner extends StatelessWidget {
  const OfflineBanner({required this.status, this.onRetry, super.key});

  final WsStatus status;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final TextTheme text = Theme.of(context).textTheme;

    // 连上了就**什么都不画**（2026-09-11，P1-2）。
    //
    // 过去这条横幅是条件插入的（`if (ws != connected)`），所以它从不需要回答
    // 「已连接时显示什么」。现在它常驻在树里、靠折叠滑入滑出，
    // 于是「已连接」这一态必须自己说清楚——否则会渲染出
    // **「后端未连接（已连接）」**这种自相矛盾的文案（真的会，只是被折起来了）。
    //
    // 在这里返回 `SizedBox.shrink()` 而不是让调用方判断：这样无论谁把它挂在
    // 哪里，它都不会说谎。
    if (status == WsStatus.connected) return const SizedBox.shrink();

    return Material(
      color: appPaletteOf(context).danger.withValues(alpha: 0.16),
      child: InkWell(
        onTap: onRetry,
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: Space.s4,
            vertical: Space.s2,
          ),
          child: Row(
            children: <Widget>[
              Icon(Icons.cloud_off, size: 16, color: appPaletteOf(context).danger),
              const SizedBox(width: Space.s2),
              Expanded(
                child: Text(
                  // 文案里要有**下一步动作**，不能只报告事实。
                  onRetry == null
                      ? '后端未连接（${status.label}）'
                      : '后端未连接（${status.label}）· 点此重试',
                  style: text.bodySmall?.copyWith(color: appPaletteOf(context).danger),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
