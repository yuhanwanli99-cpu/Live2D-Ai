/// 可关闭的行内错误提示（L3）。规格 §6.6。
///
/// # 两条来自调研的硬要求
///
/// 1. **禁止把失败渲染成「只有降低不透明度」或「红色小字」**——
///    这是同类项目最一致的缺陷之一（Nexus / OLV-Web）【调研 survey §8 缺陷 17】。
///    所以这里用 `dangerSurface` 底 + `dangerBorder` 描边 + 图标 + 可点动作。
/// 2. **失败要有「下一步」**：`429 busy` 给「打断并重发」、`503 no_supervisor`
///    给「去 LLM 设置」、`network_error` 给「重试」。只报告不给出路的错误提示
///    会让用户停在原地。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'theme.dart';

/// 错误横幅上的一个动作。
class ErrorAction {
  const ErrorAction({required this.label, required this.onPressed});

  final String label;
  final VoidCallback onPressed;
}

class ErrorBanner extends StatelessWidget {
  const ErrorBanner({
    required this.message,
    this.onDismiss,
    this.actions = const <ErrorAction>[],
    super.key,
  });

  final String message;

  /// 关闭回调；`null` 时不显示关闭按钮。
  final VoidCallback? onDismiss;

  /// 建议动作（0–2 个）。超过 2 个说明这条错误该拆。
  final List<ErrorAction> actions;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    final TextTheme text = Theme.of(context).textTheme;

    return Semantics(
      liveRegion: true,
      container: true,
      child: Container(
        margin: const EdgeInsets.all(Space.s2),
        decoration: BoxDecoration(
          color: AppPalette.dangerSurface,
          borderRadius: BorderRadius.circular(AppRadius.sm),
          border: Border.all(color: AppPalette.dangerBorder),
        ),
        padding: const EdgeInsets.fromLTRB(Space.s3, Space.s2, Space.s1, Space.s2),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Padding(
                  padding: const EdgeInsets.only(top: 1),
                  child: Icon(
                    Icons.error_outline,
                    size: 16,
                    color: AppPalette.danger,
                  ),
                ),
                const SizedBox(width: Space.s2),
                Expanded(
                  child: Text(
                    message,
                    style: text.bodySmall?.copyWith(color: colors.contentMuted),
                  ),
                ),
                if (onDismiss != null)
                  IconButton(
                    onPressed: onDismiss,
                    icon: const Icon(Icons.close, size: 16),
                    tooltip: '关闭提示',
                    visualDensity: VisualDensity.compact,
                    color: colors.contentMuted,
                  ),
              ],
            ),
            if (actions.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(left: 24, top: Space.s1),
                child: Wrap(
                  spacing: Space.s2,
                  children: <Widget>[
                    for (final ErrorAction action in actions)
                      TextButton(
                        onPressed: action.onPressed,
                        child: Text(action.label),
                      ),
                  ],
                ),
              ),
          ],
        ),
      ),
    );
  }
}
