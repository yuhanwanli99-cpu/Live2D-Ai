import 'dart:async';

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import '../ui/theme.dart';
import 'live2d_bridge.dart';
import 'live2d_host_stub.dart'
    if (dart.library.js_interop) 'live2d_host_web.dart' as host;
import 'live2d_transport.dart';

/// Live2D 舞台：持有 [Live2DBridge]，对外暴露口型与外观同步入口。
class Live2DStage extends StatefulWidget {
  const Live2DStage({
    super.key,
    this.model,
    this.scale = 1.0,
    this.dark = true,
    this.tier,
    this.onError,
    this.onReady,
  });

  /// 可选：渲染面模型路径（缺省由渲染端决定）。
  final String? model;
  final double scale;
  final bool dark;
  final int? tier;

  /// 错误回调（宿主可用于日志或全局提示）。
  final ValueChanged<String>? onError;

  /// 渲染面就绪回调（**每次**进入 ready 都触发，含错误后 [Live2DStageState.retry]
  /// 重建 iframe）。
  ///
  /// 宿主用它把显示偏好（缩放/口型灵敏度）在就绪后补发一次——首次挂载时桥还在
  /// loading，过早 `sync` 只能入队；重建 iframe 后队列已随旧桥销毁。
  final VoidCallback? onReady;

  @override
  State<Live2DStage> createState() => Live2DStageState();
}

/// 公开 State：宿主用 `GlobalKey<Live2DStageState>` 调 [setMouth] / [sync]。
class Live2DStageState extends State<Live2DStage> {
  Live2DBridge? _bridge;
  String? _hostError;

  /// 最近一条渲染面回执。**收到它才说明渲染面真的应用了**——
  /// 收条之前不得提示「已生效」（R2 的规矩）。
  StageAckEvent? _lastAck;
  int _generation = 0;
  bool _readyNotified = false;

  /// 当前消息桥（未挂载时为 `null`）。
  Live2DBridge? get bridge => _bridge;

  Live2DBridgePhase get phase =>
      _bridge?.phase ?? Live2DBridgePhase.loading;

  String? get errorMessage => _hostError ?? _bridge?.errorMessage;

  @override
  void dispose() {
    final bridge = _bridge;
    if (bridge != null) {
      bridge.removeListener(_onBridgeChanged);
      bridge.dispose();
    }
    super.dispose();
  }

  void _onBridgeChanged() {
    if (!mounted) return;
    final error = _bridge?.errorMessage;
    if (error != null && _bridge?.phase == Live2DBridgePhase.error) {
      widget.onError?.call(error);
    }
    // 进入 ready 时通知一次（离开 ready 后允许再次通知，覆盖 retry/重建）。
    final ready = _bridge?.isReady ?? false;
    if (ready && !_readyNotified) {
      _readyNotified = true;
      widget.onReady?.call();
    } else if (!ready) {
      _readyNotified = false;
    }
    setState(() {});
  }

  void _attach(Live2DTransport transport) {
    if (!mounted) {
      unawaited(transport.dispose());
      return;
    }
    final previous = _bridge;
    if (previous != null) {
      previous.removeListener(_onBridgeChanged);
      previous.dispose();
    }
    final bridge = Live2DBridge(transport)..addListener(_onBridgeChanged);
    _bridge = bridge;
    setState(() {});
    bridge.start();
    // ready 前会入队，ready 后按序 flush。
    bridge.sendSync(
      model: widget.model,
      scale: widget.scale,
      dark: widget.dark,
      tier: widget.tier,
    );
  }

  void _handleHostError(String message) {
    if (!mounted) return;
    setState(() => _hostError = message);
    widget.onError?.call(message);
  }

  /// 最近一条 `stage-ack`。
  StageAckEvent? get lastAck => _lastAck;

  /// 心跳：把桥上的回执抄一份到 State 上，便于宿主同步读取。
  void _captureAck() {
    final ack = _bridge?.lastAck;
    if (ack != null && !identical(ack, _lastAck)) _lastAck = ack;
  }

  /// 实时口型（0..1），由音频 RMS 包络驱动。
  void setMouth(double level) => _bridge?.sendMouth(level);

  /// 协议 v1 `action-state`：让模型真的演这个动作。
  ///
  /// **根级 `action`** 是渲染面的要求（它从信封根读 `action`、从 payload 读
  /// `state`/`strength`）——详见 `live2d_bridge.dart` 的头注。
  Future<void> sendActionState({
    required String action,
    String state = 'start',
    int? strength,
  }) async {
    final bridge = _bridge;
    // 未就绪时命令会入队（桥负责），所以这里不丢弃。
    await bridge?.sendActionState(
      action: action,
      state: state,
      strength: strength,
    );
    _captureAck();
  }

  /// 协议 v1 `stage-zoom`（`in` / `out` / `reset`）。
  ///
  /// 渲染面自己算缩放（±10%，clamp 0.5..2.0），所以**不发数值**；
  /// 应用后的真值从 [lastAck] 取（`scale`/`offsetX`/`offsetY`）。
  Future<void> sendStageZoom(String dir) async {
    await _bridge?.sendStageZoom(dir);
    _captureAck();
  }

  /// 协议 v1 `stage-bg`（dataURL；`null` = 清除）。
  Future<void> sendStageBg(String? dataUrl) async {
    await _bridge?.sendStageBg(dataUrl);
    _captureAck();
  }

  /// 同步舞台外观（协议 v1 `sync`）。
  void sync({
    String? model,
    double? scale,
    bool? dark,
    int? tier,
    bool? clickEnabled,
    bool? paused,
    bool? lipSync,
    bool? idleEnabled,
    double? mouthSensitivity,
  }) {
    _bridge?.sendSync(
      model: model,
      scale: scale,
      dark: dark,
      tier: tier,
      clickEnabled: clickEnabled,
      paused: paused,
      lipSync: lipSync,
      idleEnabled: idleEnabled,
      mouthSensitivity: mouthSensitivity,
    );
  }

  /// 错误后重建 iframe（也适用于渲染端热更新后的重试）。
  void retry() {
    final previous = _bridge;
    _bridge = null;
    if (previous != null) {
      previous.removeListener(_onBridgeChanged);
      unawaited(previous.destroy());
    }
    _hostError = null;
    setState(() => _generation++);
  }

  @override
  Widget build(BuildContext context) {
    final bridge = _bridge;
    final error = errorMessage;
    return Stack(
      fit: StackFit.expand,
      children: <Widget>[
        const ColoredBox(color: AppPalette.stageBackdrop),
        host.buildLive2DHost(
          key: ValueKey<int>(_generation),
          onTransport: _attach,
          onError: _handleHostError,
        ),
        if (host.live2dHostSupported &&
            error == null &&
            phase == Live2DBridgePhase.loading)
          _StageBadge(label: _loadingLabel(bridge), busy: true),
        if (error != null) _StageErrorOverlay(message: error, onRetry: retry),
        if (bridge != null && bridge.isReady && bridge.fps != null)
          Positioned(
            right: 12,
            top: 12,
            child: _StageBadge(label: '${bridge.fps} FPS'),
          ),
      ],
    );
  }

  /// 加载中文案：带上渲染面上报的资源进度（协议 v1 `progress`）。
  ///
  /// 进度是「已 fetch 资源数 / 总数」的比值；拿不到就退回不带百分比的文案——
  /// 宁可不显示数字，也不显示假的数字。
  static String _loadingLabel(Live2DBridge? bridge) {
    final progress = bridge?.progress;
    if (progress == null || !progress.isFinite) return '渲染面加载中…';
    final percent = (progress.clamp(0.0, 1.0) * 100).round();
    return '渲染面加载中 $percent%';
  }
}

class _StageBadge extends StatelessWidget {
  const _StageBadge({required this.label, this.busy = false});

  final String label;
  final bool busy;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.topLeft,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: appColorsOf(context).glassScrim,
            borderRadius: BorderRadius.circular(AppRadius.pill),
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                if (busy) ...<Widget>[
                  const SizedBox(
                    width: 12,
                    height: 12,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                  const SizedBox(width: 8),
                ],
                Text(label, style: Theme.of(context).textTheme.bodySmall),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _StageErrorOverlay extends StatelessWidget {
  const _StageErrorOverlay({required this.message, required this.onRetry});

  final String message;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.bottomCenter,
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: AppPalette.dangerSurface.withValues(alpha: 0.95),
            borderRadius: BorderRadius.circular(AppRadius.md),
            border: Border.all(color: AppPalette.dangerBorder),
          ),
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  '渲染面错误',
                  style: Theme.of(context).textTheme.titleSmall,
                ),
                const SizedBox(height: 6),
                Text(message, style: Theme.of(context).textTheme.labelLarge),
                const SizedBox(height: 10),
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton.tonalIcon(
                    onPressed: onRetry,
                    icon: const Icon(Icons.refresh, size: 18),
                    label: const Text('重试'),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
