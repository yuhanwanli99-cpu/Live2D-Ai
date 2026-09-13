/// 快捷键帮助对话框（`Ctrl/Cmd + /`）。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` **原样搬出**：内容完全来自
/// `shortcutHelp()`（唯一文案来源），与组合根的装配职责无关。
library;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'app_shortcuts.dart';

/// 展示快捷键帮助。
///
/// 内容来自 [shortcutHelp]——**唯一文案来源**，改快捷键定义时帮助自动跟上。
/// macOS 前缀（`Cmd`）按运行平台选，不靠硬编码。
Future<void> showShortcutHelpDialog(BuildContext context) async {
  final bool isMac = defaultTargetPlatform == TargetPlatform.macOS;
  await showDialog<void>(
    context: context,
    builder: (BuildContext dialogContext) => AlertDialog(
      title: const Text('快捷键'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          for (final ShortcutHelpEntry e in shortcutHelp(isMacOS: isMac))
            Padding(
              padding: const EdgeInsets.symmetric(vertical: Space.s1),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  SizedBox(
                    width: 110,
                    child: Text(
                      e.keys,
                      style: Theme.of(dialogContext).textTheme.labelLarge,
                    ),
                  ),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(e.action),
                        if (e.note != null)
                          Text(
                            e.note!,
                            style: Theme.of(dialogContext).textTheme.bodySmall,
                          ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(dialogContext).pop(),
          child: const Text('知道了'),
        ),
      ],
    ),
  );
}
