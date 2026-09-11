/// 统一字段行（L3）：**图标 + 标签 + 控件 + 帮助文本 + 语义值 + 错误槽**。
///
/// 规格 §5.2 / §6.6。**8 个分区里所有字段都走这里**——同类项目里
/// 「每个面板自己写一套行布局」的代价是标签宽度、帮助文本位置、错误显示方式
/// 各不一样，改一次要对齐 N 个文件。
///
/// # 为什么拆成一组小 widget 而不是一个泛型大 widget
///
/// 第一版写成一个 `FieldRow<T>` 带 6 个具名构造（`.slider`/`.toggle`/…）。
/// 那是一团**类型体操**：`T` 在每个构造里具体化，常量列表不能用类型参数，
/// 未用到的字段要在每个构造里显式初始化，回调类型还得 `as dynamic` 强转。
/// 编译器报了 19 个错，而且读起来比分开写更费劲。
///
/// 现在每个形态一个 widget，共享同一个 [_FieldShell] 外壳。类型是真类型，
/// 测试也能按 widget 类型定位。
///
/// # 两条硬要求
///
/// 1. **每个控件都有语义值**：读屏念「模型缩放 120%」，而不是一个裸数字。
/// 2. **错误内联在字段下方**，不用 toast（规格 §6.6：toast 一闪而过，
///    用户来不及看是哪个字段错了）。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'emphasized_text.dart';
import 'inline_notice.dart';
import 'soft_motion.dart';
import 'theme.dart';

/// 字段行的形态（**声明出来是为了可枚举与可测试**，而不是靠 widget 类型猜）。
enum FieldRowKind { slider, toggle, text, number, choice, readonly }

/// 所有字段行共用的外壳：图标 + 标签 + 说明 + 控件 + 错误槽。
class _FieldShell extends StatelessWidget {
  const _FieldShell({
    required this.icon,
    required this.label,
    required this.child,
    this.description,
    this.error,
  });

  final IconData icon;
  final String label;
  final Widget child;
  final String? description;
  final String? error;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final bool hasError = error != null && error!.isNotEmpty;

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: Space.s2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Padding(
                padding: const EdgeInsets.only(top: Space.s3),
                child: Icon(
                  icon,
                  size: 16,
                  color: hasError ? appPaletteOf(context).danger : colors.contentMuted,
                ),
              ),
              const SizedBox(width: Space.s2),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(label, style: theme.textTheme.labelLarge),
                    if (description != null)
                      EmphasizedText(
                        description!,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: colors.contentMuted,
                        ),
                      ),
                    const SizedBox(height: Space.s1),
                    child,
                  ],
                ),
              ),
            ],
          ),
          // 结果行**渐入渐出**（2026-09-11，P2-2）。
          //
          // 过去它是条件插入：测试一返回就「啪」地多出一行，把下面的字段
          // 整体推下去——用户正在看的那个字段会跳。`SoftSwap` 让新旧内容
          // 交叉淡入的同时**旧内容不再吃指针与语义**，高度也随动画长出来。
          SoftSwap(
            child: hasError
                ? Padding(
                    padding: const EdgeInsets.only(left: Space.s6, top: OpticalNudge.thin),
                    // 用统一的 `InlineNotice`（紧凑档）而不是红色小字 +
                    // 一个 `⚠` 字符：字符冒充图标在不同平台字形/基线都不一样，
                    // 也躲过了「图标要能对齐」这件事（2026-09-11，P1-5）。
                    child: InlineNotice(message: error!, dense: true),
                  )
                : const SizedBox.shrink(),
          ),
        ],
      ),
    );
  }
}

/// 滑杆字段（值域由调用方给，**不在组件里写死**）。
class SliderField extends StatelessWidget {
  const SliderField({
    required this.label,
    required this.icon,
    required this.value,
    required this.min,
    required this.max,
    required this.onChanged,
    this.percentage = true,
    this.suffix = '',
    this.divisions,
    this.description,
    this.error,
    this.enabled = true,
    super.key,
  });

  final String label;
  final IconData icon;
  final double value;
  final double min;
  final double max;
  final ValueChanged<double> onChanged;

  /// 显示成百分比（缩放/音量/灵敏度）还是原始值（旋转角度）。
  final bool percentage;
  final String suffix;
  final int? divisions;
  final String? description;
  final String? error;
  final bool enabled;

  String _format(double v) => percentage
      ? '${(v * 100).round()}%'
      : '${v.toStringAsFixed(1)}$suffix';

  @override
  Widget build(BuildContext context) {
    return _FieldShell(
      icon: icon,
      label: label,
      description: description,
      error: error,
      child: Row(
        children: <Widget>[
          Expanded(
            child: Slider(
              value: value.clamp(min, max),
              min: min,
              max: max,
              divisions: divisions,
              label: label,
              onChanged: enabled ? onChanged : null,
              // 读屏念「标签 + 值」，不是裸数字。
              semanticFormatterCallback: (double next) => '$label ${_format(next)}',
            ),
          ),
          SizedBox(
            width: 56,
            child: Text(
              _format(value),
              textAlign: TextAlign.end,
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
        ],
      ),
    );
  }
}

/// 开关字段。
class ToggleField extends StatelessWidget {
  const ToggleField({
    required this.label,
    required this.icon,
    required this.value,
    required this.onChanged,
    this.description,
    this.error,
    this.enabled = true,
    super.key,
  });

  final String label;
  final IconData icon;
  final bool value;
  final ValueChanged<bool> onChanged;
  final String? description;
  final String? error;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final Widget toggle = Switch(value: value, onChanged: enabled ? onChanged : null);
    return _FieldShell(
      icon: icon,
      label: label,
      description: description,
      error: error,
      // 语义交给开关自己（它已经是可调节控件），但标签必须带上——
      // 否则读屏只会念「开/关」，不知道开的是什么。
      child: Semantics(label: label, child: toggle),
    );
  }
}

/// 文本字段。
class TextFieldRow extends StatefulWidget {
  const TextFieldRow({
    required this.label,
    required this.icon,
    required this.value,
    required this.onChanged,
    this.hint,
    this.description,
    this.error,
    this.obscure = false,
    this.multiline = false,
    this.maxLines,
    this.enabled = true,
    super.key,
  });

  final String label;
  final IconData icon;
  final String value;
  final ValueChanged<String> onChanged;
  final String? hint;
  final String? description;
  final String? error;
  final bool obscure;
  final bool multiline;
  final int? maxLines;
  final bool enabled;

  @override
  State<TextFieldRow> createState() => _TextFieldRowState();
}

class _TextFieldRowState extends State<TextFieldRow> {
  late final TextEditingController _controller = TextEditingController(
    text: widget.value,
  );

  @override
  void didUpdateWidget(TextFieldRow oldWidget) {
    super.didUpdateWidget(oldWidget);
    // 外部值变了（保存后回填 / 放弃改动）→ 同步进来。
    //
    // **必须判「用户是否正在编辑」**：不判的话每次父级重建都会把光标推回开头，
    // 用户打字时会被打断（典型表现是「输入框自己在跳」）。
    if (widget.value != oldWidget.value &&
        widget.value != _controller.text &&
        !_hasFocus) {
      _controller.text = widget.value;
    }
  }

  bool _hasFocus = false;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return _FieldShell(
      icon: widget.icon,
      label: widget.label,
      description: widget.description,
      error: widget.error,
      child: Focus(
        onFocusChange: (bool focused) => _hasFocus = focused,
        child: TextField(
          controller: _controller,
          enabled: widget.enabled,
          obscureText: widget.obscure,
          minLines: widget.multiline ? (widget.maxLines ?? 3) : 1,
          maxLines: widget.multiline ? (widget.maxLines ?? 8) : 1,
          onChanged: widget.onChanged,
          decoration: InputDecoration(
            hintText: widget.hint,
            isDense: true,
            border: const OutlineInputBorder(),
          ),
        ),
      ),
    );
  }
}

/// 数字字段（越界输入**不回调**——不能让一个手滑的 0 把配置写坏）。
class NumberField extends StatefulWidget {
  const NumberField({
    required this.label,
    required this.icon,
    required this.value,
    required this.onChanged,
    this.min,
    this.max,
    this.description,
    this.error,
    this.enabled = true,
    super.key,
  });

  final String label;
  final IconData icon;
  final int value;
  final ValueChanged<int> onChanged;
  final int? min;
  final int? max;
  final String? description;
  final String? error;
  final bool enabled;

  @override
  State<NumberField> createState() => _NumberFieldState();
}

class _NumberFieldState extends State<NumberField> {
  late final TextEditingController _controller = TextEditingController(
    text: '${widget.value}',
  );

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return _FieldShell(
      icon: widget.icon,
      label: widget.label,
      description: widget.description,
      error: widget.error,
      child: SizedBox(
        width: 150,
        child: TextField(
          controller: _controller,
          enabled: widget.enabled,
          keyboardType: TextInputType.number,
          onChanged: (String raw) {
            final int? parsed = int.tryParse(raw.trim());
            if (parsed == null) return;
            if (widget.min != null && parsed < widget.min!) return;
            if (widget.max != null && parsed > widget.max!) return;
            widget.onChanged(parsed);
          },
          decoration: InputDecoration(
            isDense: true,
            border: const OutlineInputBorder(),
            helperText: widget.min == null && widget.max == null
                ? null
                : '${widget.min ?? '−∞'} – ${widget.max ?? '∞'}',
          ),
        ),
      ),
    );
  }
}

/// 一个可选项。
class FieldOption<T> {
  const FieldOption({required this.value, required this.label, this.description});

  final T value;
  final String label;
  final String? description;
}

/// 分段按钮字段（≤5 项）。**超过 5 项请用 [DropdownField]**——
/// 分段按钮挤到换行就失去了「一眼看全」的意义。
class SegmentedField<T> extends StatelessWidget {
  const SegmentedField({
    required this.label,
    required this.icon,
    required this.value,
    required this.options,
    required this.onChanged,
    this.description,
    this.error,
    this.enabled = true,
    super.key,
  }) : assert(options.length <= 5, '分段按钮最多 5 项，多了请用 DropdownField');

  final String label;
  final IconData icon;
  final T value;
  final List<FieldOption<T>> options;
  final ValueChanged<T> onChanged;
  final String? description;
  final String? error;
  final bool enabled;

  @override
  Widget build(BuildContext context) => _FieldShell(
    icon: icon,
    label: label,
    description: description,
    error: error,
    child: Semantics(
      label: label,
      child: SegmentedButton<T>(
        segments: <ButtonSegment<T>>[
          for (final FieldOption<T> o in options)
            ButtonSegment<T>(
              value: o.value,
              label: Text(o.label),
              tooltip: o.description,
            ),
        ],
        selected: <T>{value},
        showSelectedIcon: false,
        onSelectionChanged: enabled
            ? (Set<T> next) => onChanged(next.first)
            : null,
      ),
    ),
  );
}

/// 下拉字段（项数多时用）。
class DropdownField<T> extends StatelessWidget {
  const DropdownField({
    required this.label,
    required this.icon,
    required this.value,
    required this.options,
    required this.onChanged,
    this.description,
    this.error,
    this.enabled = true,
    super.key,
  });

  final String label;
  final IconData icon;
  final T value;
  final List<FieldOption<T>> options;
  final ValueChanged<T> onChanged;
  final String? description;
  final String? error;
  final bool enabled;

  @override
  Widget build(BuildContext context) => _FieldShell(
    icon: icon,
    label: label,
    description: description,
    error: error,
    child: DropdownButton<T>(
      value: value,
      onChanged: enabled
          ? (T? next) {
              if (next != null) onChanged(next);
            }
          : null,
      items: <DropdownMenuItem<T>>[
        for (final FieldOption<T> o in options)
          DropdownMenuItem<T>(value: o.value, child: Text(o.label)),
      ],
    ),
  );
}

/// 只读字段（如 `tts.sample_rate`）——**不是**禁用输入框：
/// 禁用输入框会让用户以为「本来能改但现在不能」，只读文本才诚实地表达
/// 「这不是一个可配置项」。
class ReadonlyField extends StatelessWidget {
  const ReadonlyField({
    required this.label,
    required this.icon,
    required this.text,
    this.description,
    super.key,
  });

  final String label;
  final IconData icon;
  final String text;
  final String? description;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    return _FieldShell(
      icon: icon,
      label: label,
      description: description,
      child: Text(
        text,
        style: Theme.of(context).textTheme.bodyMedium?.copyWith(
          color: colors.contentMuted,
        ),
      ),
    );
  }
}

/// 一段动作按钮（保存 / 重试 / 测试连通性）。
class FieldActionRow extends StatelessWidget {
  const FieldActionRow({
    required this.label,
    required this.icon,
    required this.actionLabel,
    required this.onPressed,
    this.description,
    this.busy = false,
    this.result,
    this.resultIsError = false,
    super.key,
  });

  final String label;
  final IconData icon;
  final String actionLabel;
  final VoidCallback? onPressed;
  final String? description;
  final bool busy;

  /// 内联结果行（连通性自检的 `ok`/`latency_ms`/`error`）。
  final String? result;
  final bool resultIsError;

  @override
  Widget build(BuildContext context) => _FieldShell(
      icon: icon,
      label: label,
      description: description,
      error: resultIsError ? result : null,
      child: Align(
        alignment: Alignment.centerLeft,
        child: OutlinedButton.icon(
          onPressed: busy ? null : onPressed,
          icon: busy
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Icon(icon, size: 16),
          label: Text(busy ? '测试中…' : actionLabel),
        ),
      ),
    );
}
