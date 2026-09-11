/// 舞台宿主（L3）：保活、`RepaintBoundary`、加载/错误覆盖层、语义标签。
///
/// 规格 §3.4 的「舞台保活」落地写法。两条约束值得写在这里：
///
/// 1. **`RepaintBoundary` 包住舞台**：口型电平是 30 Hz 的高频信号，
///    它走 `GlobalKey → Bridge → postMessage`，**不进 Widget 树**
///    （原则 P6）。`RepaintBoundary` 是这条纪律的兜底——即便某天有人
///    不小心让上层重建了，也不会把整棵聊天列表拖进重绘。
/// 2. **覆盖层不卸载下层**：加载/错误覆盖层用 `Stack` 叠在 iframe 上，
///    **不**用 `if (ready) stage else overlay` 的条件表达式——那会重建
///    iframe，进而丢掉模型与口型时间轴。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import '../live2d/live2d_bridge.dart';
import '../live2d/stage_pointer_interceptor.dart';
import 'theme.dart';

class StageHost extends StatelessWidget {
  const StageHost({
    required this.stage,
    required this.phase,
    this.progress,
    this.errorMessage,
    this.onRetry,
    this.overlayTop,
    this.corner,
    this.modelName,
    super.key,
  });

  /// 舞台本体。**必须是同一个实例/同一个 key**，否则保活失效。
  ///
  /// 类型是 `Widget` 而不是具体的 `Live2DStage`：宿主只需要「把某个东西铺满」，
  /// 不该要求调用方给一个真的 iframe。这正好让覆盖层与语义可以**单独测**
  /// （规格 §11.3 明确要求「用可注入的 stage 占位，**不要**为测试给 stage
  /// 加分支」）。
  final Widget stage;

  /// 渲染面阶段。
  final Live2DBridgePhase phase;

  /// 加载进度 0..1（`null` = 不确定）。
  final double? progress;

  /// 错误文案。
  final String? errorMessage;

  /// 「重试」回调（重建 iframe）。
  final VoidCallback? onRetry;

  /// 顶部覆盖层（离线横幅）。
  final Widget? overlayTop;

  /// 右下/左下的常驻角标（状态胶囊、缩放三键）。
  final Widget? corner;

  /// 当前模型名（用于舞台的语义标签）。
  ///
  /// **平台视图对读屏是黑盒**：不给语义标签的话，读屏用户完全不知道
  /// 页面里有一个 Live2D 舞台，更不知道在显示哪个模型。
  final String? modelName;

  @override
  Widget build(BuildContext context) {
    return RepaintBoundary(
      child: FocusTraversalGroup(
        policy: OrderedTraversalPolicy(),
        child: Stack(
          fit: StackFit.expand,
          children: <Widget>[
            // 舞台永远在树里——覆盖层只是叠上去。
            //
            // 语义只包**舞台本体**，**不能**包整个 Stack：加载/错误覆盖层
            // 各自有语义（「模型加载中」/「模型加载失败」+ 重试按钮），
            // 在 Stack 上写 `excludeSemantics: true` 会把它们一起吃掉，
            // 读屏用户就再也点不到「重试」了。（第一版就是这么写的。）
            Semantics(
              container: true,
              label: modelName == null || modelName!.isEmpty
                  ? 'Live2D 舞台'
                  : 'Live2D 舞台，正在显示 $modelName',
              // iframe 内部对读屏是黑盒 → 屏蔽，避免出现一个空节点。
              child: ExcludeSemantics(child: SizedBox.expand(child: stage)),
            ),
            // 覆盖层压在 iframe 之上 ⇒ **必须**垫一层指针垫层：否则它们会
            // 「看得见、点不着」（错误态的「重试」按钮就是这么失效的）。
            // 见 `StagePointerInterceptor` 的头注。
            if (phase == Live2DBridgePhase.loading)
              StagePointerInterceptor(
                child: _StageLoadingOverlay(progress: progress),
              ),
            if (phase == Live2DBridgePhase.error)
              StagePointerInterceptor(
                child: _StageErrorOverlay(message: errorMessage, onRetry: onRetry),
              ),
            if (overlayTop != null)
              Align(alignment: Alignment.topCenter, child: overlayTop),
            if (corner != null)
              Align(alignment: Alignment.bottomRight, child: corner),
          ],
        ),
      ),
    );
  }
}

class _StageLoadingOverlay extends StatelessWidget {
  const _StageLoadingOverlay({this.progress});

  final double? progress;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    return Semantics(
      label: '模型加载中',
      container: true,
      child: ColoredBox(
        color: colors.glassScrim,
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              SizedBox(
                width: 160,
                child: LinearProgressIndicator(value: progress),
              ),
              const SizedBox(height: Space.s2),
              Text(
                progress == null
                    ? '模型加载中…'
                    : '模型加载中… ${(progress! * 100).round()}%',
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _StageErrorOverlay extends StatelessWidget {
  const _StageErrorOverlay({this.message, this.onRetry});

  final String? message;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);

    return Semantics(
      label: '模型加载失败',
      container: true,
      child: ColoredBox(
        color: colors.glassScrim,
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Icon(Icons.error_outline, size: 28, color: appPaletteOf(context).danger),
              const SizedBox(height: Space.s2),
              Text('模型加载失败', style: theme.textTheme.titleSmall),
              if (message != null) ...<Widget>[
                const SizedBox(height: Space.s1),
                ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 320),
                  child: Text(
                    message!,
                    textAlign: TextAlign.center,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: colors.contentMuted,
                    ),
                  ),
                ),
              ],
              if (onRetry != null) ...<Widget>[
                const SizedBox(height: Space.s3),
                FilledButton.tonal(onPressed: onRetry, child: const Text('重试')),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
