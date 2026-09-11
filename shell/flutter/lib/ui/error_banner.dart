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
///
/// # 2026-09-11（P1-5）：本组件只剩「接口」，外形交给 `InlineNotice`
///
/// 同一件事过去有三套写法（这里、`field_row` 的 `'⚠ $error'`、
/// `dev_tools_section` 的同一句字面文本）。现在**外形只有 `InlineNotice`
/// 一处**，本组件只负责把 `ErrorAction` 翻译过去、并固定成 danger 档。
/// 这样「错误长什么样」以后只改一个文件。
library;

import 'package:flutter/material.dart';

import 'inline_notice.dart';

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
  Widget build(BuildContext context) => InlineNotice(
    message: message,
    onDismiss: onDismiss,
    actions: <NoticeAction>[
      for (int i = 0; i < actions.length; i++)
        NoticeAction(
          label: actions[i].label,
          onPressed: actions[i].onPressed,
          // **只有一个动作时它才是主动作**。两个都做成实心，用户就得先读
          // 完两个再决定按哪个——那正是「提示给了路却还是停在原地」。
          primary: actions.length == 1,
        ),
    ],
  );
}
