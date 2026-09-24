/// 未保存改动的确认框：**换分区 / 关设置 / 关浮层三处共用**。
///
/// # 为什么单独一个文件（而不是 showDialog 内联在组合根里）
///
/// 「保存并离开」要**看 [SaveOutcome]**：失败必须留在弹窗里并给出可见错误，
/// 保存中要禁用三个按钮防连点。这些是**有状态的交互**，塞进
/// showDialog(builder:) 的闭包里只能靠 StatefulBuilder 硬撑，
/// 而且没法在 VM 上单测（组合根带 package:web）。抽成 widget 后
/// flutter test 可以直接 pump 它。
///
/// # 为什么弹窗自己也要垫一层 [StagePointerInterceptor]（P0，2026-09-20）
///
/// 症状：**关「开发者模式」后弹出「有未保存的改动」，三个按钮完全没反应**。
///
/// 根因（真机 DOM，不是猜的）：舞台是 iframe 平台视图，它在 DOM 里排在
/// Flutter 画布之上，而两张画布都是 pointer-events: none——指针落在舞台区域
/// 会进 **iframe 自己的文档**，父页（Flutter）一个都收不到。这正是
/// [StagePointerInterceptor] 头注写的失败模式；项目规矩是「**新增压在舞台上的
/// 控件必须照做**」。
///
/// 舞台的加载/错误覆盖层各自垫了一层，**唯独这个 AlertDialog 漏了**：
/// 它由 showDialog 挂在根 overlay 上，画在舞台之上，却没有自己的平台视图。
/// 于是舞台**一切正常**（没有 loading/error 覆盖层）时，弹窗区域在 DOM 上
/// 唯一能接指针的就是那个 iframe ⇒ **看得见、点不着**。舞台恰好处于加载/错误
/// 时反而「能用」，所以早先的临时覆盖层把这条缺陷盖住了。
///
/// 垫层是空 div：它让指针回到父页，Flutter 的命中测试再把事件交给画在
/// 它上面的弹窗按钮。与设置面板、会话浮层同一手法。
library;

import 'package:flutter/material.dart';

import '../live2d/stage_pointer_interceptor.dart';
import '../settings/settings_controller.dart';

/// 弹出确认框；返回 true = 可以离开（用户选择放弃或保存成功）。
///
/// - onSave：「保存并离开」真的执行保存，返回 [SaveOutcome]；
/// - errorOf：失败时取服务端给的**具体错误**（SettingsController.error）。
///
/// 点遮罩 / 按 Esc 关闭 → 返回 false（= 留下，最保守的默认）。
Future<bool> showConfirmDiscardDialog(
  BuildContext context, {
  required Future<SaveOutcome> Function() onSave,
  required String? Function() errorOf,
  String title = '有未保存的改动',
  String message = '离开会丢掉这些改动。要先保存吗？',
}) async {
  final bool? leave = await showDialog<bool>(
    context: context,
    // 显式写出来：设置浮层也挂在导航栈上，弹窗必须在**根**导航器的 overlay 里，
    // 否则会被嵌套 Navigator 的层叠关系吃掉（默认就是 true，这里只做声明）。
    useRootNavigator: true,
    builder: (BuildContext dialogContext) => StagePointerInterceptor(
      child: _ConfirmDiscardDialog(
        title: title,
        message: message,
        onSave: onSave,
        errorOf: errorOf,
      ),
    ),
  );
  return leave ?? false;
}

class _ConfirmDiscardDialog extends StatefulWidget {
  const _ConfirmDiscardDialog({
    required this.title,
    required this.message,
    required this.onSave,
    required this.errorOf,
  });

  final String title;
  final String message;
  final Future<SaveOutcome> Function() onSave;
  final String? Function() errorOf;

  @override
  State<_ConfirmDiscardDialog> createState() => _ConfirmDiscardDialogState();
}

class _ConfirmDiscardDialogState extends State<_ConfirmDiscardDialog> {
  /// **saving 态**：保存期间三个按钮一起禁用，防连点重复提交。
  bool _saving = false;

  /// 上次保存失败的可见错误（带服务端 code）。
  String? _error;

  Future<void> _saveAndLeave() async {
    if (_saving) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    final SaveOutcome outcome = await widget.onSave();
    if (!mounted) return;
    // **只有真失败才留下**：noChange 也意味着磁盘与草稿已一致，可以走。
    if (outcome == SaveOutcome.failed) {
      setState(() {
        _saving = false;
        _error = outcome.messageWith(error: widget.errorOf());
      });
      return;
    }
    Navigator.of(context).pop(true);
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return AlertDialog(
      title: Text(widget.title),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(widget.message),
          if (_error != null) ...<Widget>[
            const SizedBox(height: 12),
            Text(
              _error!,
              key: const Key('confirm-discard-error'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.error,
              ),
            ),
          ],
        ],
      ),
      actions: <Widget>[
        TextButton(
          onPressed: _saving ? null : () => Navigator.of(context).pop(false),
          child: const Text('留下'),
        ),
        TextButton(
          onPressed: _saving ? null : _saveAndLeave,
          child: Text(_saving ? '保存中…' : '保存并离开'),
        ),
        FilledButton(
          onPressed: _saving ? null : () => Navigator.of(context).pop(true),
          child: const Text('放弃改动'),
        ),
      ],
    );
  }
}
