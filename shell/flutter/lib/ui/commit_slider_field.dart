/// 「拖动只动草稿、停手才提交」的滑杆（2026-10-09 从 appearance_section 库抽出）。
///
/// # 为什么需要这一层
///
/// SliderField.onChanged 是**每帧**回调，而外观区每一条滑杆的落点都是
/// 「改偏好 → 整份 jsonEncode + setItem + 下发渲染面」。用户设了舞台壁纸时
/// （偏好里那张 base64 有协议预算 1.5 M 字符），拖一次滑杆 = 每帧一次 MB 级
/// 同步序列化。音量滑杆 2026-09-28 已经改成 onChangeEnd（见 ui/audio_bar.dart），
/// 这里把剩下的滑杆统一到同一约定：
///
/// | 事件 | 做什么 |
/// | --- | --- |
/// | onChanged（拖动中，每帧） | 只更新**草稿**：读数跟着手指走 |
/// | 停手 AppDurations.base（防抖） | 把草稿**提交**给宿主，落盘一次 |
///
/// # 为什么是防抖而不是 Slider.onChangeEnd
///
/// SliderField 住 ui/field_row.dart，被十几个分区共用；补 onChangeEnd 会动它
/// 的公共面。**代价（如实记录）**：拖动中途停顿超过防抖窗口会多提交一次
/// （「一次连续拖动至多落一次盘」仍成立）。
///
/// # 为什么 2026-10-09 搬出 appearance 库
///
/// 「主题」与「Live2D 动作」拆成两个页面后，两边都要用它：
/// 动作页（motion_section.dart）不能引用另一个分区库里的私有类。
/// 它**不是**新的滑杆实现——逐字搬迁，语义一字不改。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'field_row.dart';

/// 语义：拖动中界面读数跟手；停手后落盘值 == 滑杆终值 == 界面读数
/// （宿主回灌的值优先，不拿草稿硬撑）。
class CommitSliderField extends StatefulWidget {
  const CommitSliderField({
    required this.label,
    required this.icon,
    required this.value,
    required this.min,
    required this.max,
    required this.onCommit,
    this.percentage = true,
    this.suffix = "",
    this.description,
    this.minLabel,
    this.maxLabel,
    super.key,
  });

  final String label;
  final IconData icon;
  final double value;
  final double min;
  final double max;

  /// 停手那一刻的提交（**一次连续拖动至多一次**）。
  final ValueChanged<double> onCommit;
  final bool percentage;
  final String suffix;
  final String? description;
  final String? minLabel;
  final String? maxLabel;

  /// 防抖窗口 = AppDurations.base（200 ms）。
  ///
  /// 为什么不自己写一个毫秒数：UI 时长受**令牌门禁**管
  /// （design_tokens_lint_test：「UI 动效时长只能取 AppDurations 的 4 档」），
  /// 而「停手多久算停手」与「内容切换用多久」在观感上是同一量级。
  static const Duration debounce = AppDurations.base;

  @override
  State<CommitSliderField> createState() => _CommitSliderFieldState();
}

class _CommitSliderFieldState extends State<CommitSliderField> {
  /// 拖动中的草稿（null = 跟随 widget.value）。
  ///
  /// 除了「停手才提交」之外还有一条理由：拖动过程中宿主会因为**别的原因**重建
  /// （每个流式增量都在重建整棵壳），草稿留在 State 里，滑杆才不会被旧值弹回去。
  double? _draft;
  Timer? _pending;

  @override
  void didUpdateWidget(CommitSliderField oldWidget) {
    super.didUpdateWidget(oldWidget);
    // 宿主把值回灌了（提交成功 / 别处改了这项）⇒ 草稿使命结束。
    // 判据是「值变了」而不是「值等于草稿」：宿主若把值夹持成别的数，
    // 界面必须显示宿主那份。
    if (widget.value != oldWidget.value) {
      _pending?.cancel();
      _pending = null;
      _draft = null;
    }
  }

  @override
  void dispose() {
    _pending?.cancel();
    super.dispose();
  }

  void _onDrag(double next) {
    setState(() => _draft = next);
    _pending?.cancel();
    _pending = Timer(CommitSliderField.debounce, _commit);
  }

  void _commit() {
    _pending?.cancel();
    _pending = null;
    final double? draft = _draft;
    if (draft == null) return;
    // 先把草稿交还给宿主：宿主回填多少就显示多少（它可能夹持）。
    setState(() => _draft = null);
    widget.onCommit(draft);
  }

  @override
  Widget build(BuildContext context) => SliderField(
    label: widget.label,
    icon: widget.icon,
    value: _draft ?? widget.value,
    min: widget.min,
    max: widget.max,
    percentage: widget.percentage,
    suffix: widget.suffix,
    description: widget.description,
    onChanged: _onDrag,
    minLabel: widget.minLabel,
    maxLabel: widget.maxLabel,
  );
}
