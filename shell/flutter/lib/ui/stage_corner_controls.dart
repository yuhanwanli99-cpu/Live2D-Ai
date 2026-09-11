/// 舞台角标三键：缩小 / 放大 / 复位（规格 §5.2 的「舞台角标」）。
///
/// # 文件名的来历（原名 `action_toolbar.dart`，2026-09-11 改名）
///
/// 本文件**只剩缩放三键**，一个动作控件都没有：手动动作触发工具条早已
/// 移出成品（见 `docs/design/web-action-trigger-archive.md`）。用户裁定
/// LLM 无工具、只做对话后，动作子系统整条删除，这个不诚实的旧文件名
/// 一并改成 `stage_corner_controls.dart`——避免下一个人看到
/// `action_toolbar.dart` 就以为它是动作功能而顺手删掉缩放角标。
///
/// # 为什么放大缩小是「方向」而不是「数值」
///
/// 渲染面**自己算**缩放：`scale * 1.1` 或 `/ 1.1`，clamp 到 `0.5..2.0`，
/// `reset` 还会把偏移归零（`l2d-wasm-demo/src/main.rs:346-357`）。
/// 所以父页发的是 `dir: in|out|reset`，**不是**目标缩放值。
///
/// **应用后的真值要以 `stage-ack` 为准**（`scale`/`offset_x`/`offset_y`）——
/// 用本地值猜会和渲染面漂移（比如已经到 2.0 上限时，本地以为又放大了 10%）。
///
/// # 为什么是文字而不是图标（2026-09-11 用户裁决）
///
/// 「尽量少用图片用文字做按钮」。图标要在脑子里翻译一次（`+` / `−` / 准星
/// 各是什么），文字不用。同时**保留百分比读数**：`−` 与 `+` 之间那个数字
/// 才是用户真正在看的东西，它不是装饰。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'glass_rim.dart';
import 'theme.dart';

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
      // 舞台角标是**最小的一块玻璃**：一圈跟随指针的边缘高光就够了
      // （P3-1）。它压在 iframe 上，所以只能画描边——不能模糊也不能采样。
      child: GlassRim(
        borderRadius: BorderRadius.circular(AppRadius.pill),
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
                _ZoomTextButton(
                  label: '缩小',
                  tooltip: '缩小一档',
                  onPressed: () => onZoom('out'),
                ),
                SizedBox(
                  width: 52,
                  child: Text(
                    // ack 之前不显示数值（**不猜**）。
                    scaleFromAck == null
                        ? '—'
                        : '${(scaleFromAck! * 100).round()}%',
                    textAlign: TextAlign.center,
                    style: Theme.of(context).textTheme.labelSmall?.copyWith(
                      color: colors.contentMuted,
                      fontFeatures: const <FontFeature>[
                        FontFeature.tabularFigures(),
                      ],
                    ),
                  ),
                ),
                _ZoomTextButton(
                  label: '放大',
                  tooltip: '放大一档',
                  onPressed: () => onZoom('in'),
                ),
                _ZoomTextButton(
                  label: '复位',
                  tooltip: '复位缩放与位置',
                  onPressed: () => onZoom('reset'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _ZoomTextButton extends StatelessWidget {
  const _ZoomTextButton({
    required this.label,
    required this.tooltip,
    required this.onPressed,
  });

  final String label;
  final String tooltip;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: TextButton(
        onPressed: onPressed,
        style: TextButton.styleFrom(
          visualDensity: VisualDensity.compact,
          minimumSize: const Size(48, 32),
          padding: const EdgeInsets.symmetric(horizontal: Space.s2),
        ),
        child: Text(label),
      ),
    );
  }
}
