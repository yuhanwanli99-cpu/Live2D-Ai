import 'dart:async';

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import '../ui/soft_motion.dart';
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
    this.stageColor,
    this.stageImage,
    this.tier,
    this.onError,
    this.onReady,
    this.onPhaseChanged,
    this.onAck,
  });

  /// 可选：渲染面模型路径（缺省由渲染端决定）。
  final String? model;
  final double scale;
  final bool dark;

  /// 舞台纯色底（`#RRGGBB`）。**这是四套主题的舞台底真正生效的地方**。
  ///
  /// `null` = 不指定，渲染面按 [dark] 用历史默认色（`#101418` / `#e8ecf1`）。
  final String? stageColor;

  /// 自定义舞台背景图（dataURL）。`null` = 只有纯色底。
  ///
  /// **为什么舞台自己持这个值**：`stage-bg` 是独立于 `sync` 的通道，
  /// 而 iframe 是**可重建**的（首帧前入队、错误后 retry、热更新重挂）。
  /// 只在「偏好变化」时发一次的旧实现，会在「iframe 还没就绪」与「重建」
  /// 两种情况下把图丢掉——那正是 Win 侧换背景图失败的现场。
  /// 现在 `_attach` 每次挂桥都重发一次，与 `stageColor` 同一条纪律。
  final String? stageImage;

  final int? tier;

  /// 渲染面阶段变化（宿主据此刷新覆盖层与舞台语义）。
  ///
  /// 没有它的话，宿主拿不到阶段变化：`Live2DStage` 内部 `setState` 只重建自己，
  /// 父级不会跟着重建，于是 `StageHost` 的加载覆盖层会**永远停在 loading**。
  final void Function(Live2DBridgePhase phase)? onPhaseChanged;

  /// 错误回调（宿主可用于日志或全局提示）。
  final ValueChanged<String>? onError;

  /// 每收到一条**新的** `stage-ack` 回调一次。
  ///
  /// 宿主用它把「渲染面实际生效的缩放」显示出来。缺了它，舞台角标的百分比
  /// 只能等用户按一次放大/缩小才有值——首屏会一直显示一个「—」，
  /// 看起来像坏了（2026-09-11 在真浏览器里就是这么显示的）。
  final ValueChanged<StageAckEvent>? onAck;

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
class Live2DStageState extends State<Live2DStage>
    with SingleTickerProviderStateMixin {
  Live2DBridge? _bridge;
  String? _hostError;

  /// 最近一条渲染面回执。**收到它才说明渲染面真的应用了**——
  /// 收条之前不得提示「已生效」（R2 的规矩）。
  StageAckEvent? _lastAck;
  Live2DBridgePhase? _lastReportedPhase;
  int _generation = 0;
  bool _readyNotified = false;

  /// ── 舞台底的**插值**（2026-09-11，P1-3） ──
  ///
  /// 问题：切主题时 `MaterialApp` 会用 200 ms 把整套配色**插值**过去，
  /// 而舞台底是渲染面（iframe）自己画的——它只认一个终色。于是切的那一瞬间
  /// 界面在渐变、舞台已经跳到终色，看起来像「舞台先闪了一下」。
  ///
  /// 做法：舞台自己跑一条**同样 200 ms** 的动画（`AppDurations.base` 正是
  /// `MaterialApp` 的 `kThemeAnimationDuration`），每一帧把插值出来的颜色
  /// 拼成 `#rrggbb` 发给渲染面。观感上两边就是一起变的。
  ///
  /// 代价是这 200 ms 内会多发十几条 `sync`。可以接受：`sync` 是幂等的状态帧，
  /// 桥会把它们按序 flush；渲染面只认最后一个。
  late final AnimationController _stageColorMotion;

  /// 动画起点（当前屏幕上那个颜色）。首帧没有起点，直接就位。
  Color? _stageColorFrom;
  Color? _stageColorTo;

  /// 当前应当显示在舞台上的底色。
  Color? get _displayedStageColor {
    final Color? to = _stageColorTo;
    if (to == null) return null;
    final Color? from = _stageColorFrom;
    if (from == null) return to;
    return Color.lerp(from, to, _stageColorMotion.value);
  }

  /// 当前消息桥（未挂载时为 `null`）。
  Live2DBridge? get bridge => _bridge;

  Live2DBridgePhase get phase =>
      _bridge?.phase ?? Live2DBridgePhase.loading;

  String? get errorMessage => _hostError ?? _bridge?.errorMessage;

  @override
  void initState() {
    super.initState();
    _stageColorTo = parseStageColorCss(widget.stageColor);
    _stageColorMotion = AnimationController(
      vsync: this,
      // 初值是 `Duration.zero`：真正生效的时长在 `didUpdateWidget` 里经
      // `appMotion(context, …)` 设好再 `forward()`（那里才有 context，
      // 也才过得了「减少动画」这道闸门）。构造处的时长从不用来跑动画。
      duration: Duration.zero,
      // 初值 1：首帧不播动画（页面第一次出现时不该有底色渐变）。
      value: 1,
    )..addListener(_pushStageColor);
  }

  @override
  void didUpdateWidget(Live2DStage oldWidget) {
    super.didUpdateWidget(oldWidget);
    // 背景图是独立通道：先处理它，**不能**因为底色没变就被下面的 return 跳过。
    if (widget.stageImage != oldWidget.stageImage) {
      unawaited(
        _bridge?.sendStageBg(widget.stageImage) ?? Future<void>.value(),
      );
    }
    if (widget.stageColor == oldWidget.stageColor) return;
    final Color? next = parseStageColorCss(widget.stageColor);
    // 从**当前显示值**续接，而不是从旧终值——连续切两次主题时不会跳回去。
    _stageColorFrom = _displayedStageColor;
    _stageColorTo = next;
    if (_stageColorFrom == null || next == null) {
      // 拿不到起点或终点（例如 `null` = 交给渲染面按 dark 决定）→ 直接就位。
      _stageColorMotion.value = 1;
      _pushStageColor();
      return;
    }
    _stageColorMotion
      ..duration = appMotion(context, AppDurations.base)
      ..forward(from: 0);
  }

  void _pushStageColor() {
    final Color? color = _displayedStageColor;
    if (color == null) return;
    _bridge?.sendSync(stageColor: stageColorCss(color));
  }

  @override
  void dispose() {
    _stageColorMotion.dispose();
    final bridge = _bridge;
    if (bridge != null) {
      bridge.removeListener(_onBridgeChanged);
      bridge.dispose();
    }
    super.dispose();
  }

  void _onBridgeChanged() {
    if (!mounted) return;
    // 阶段变化先报给宿主（覆盖层/语义都靠它）。
    final phase = _bridge?.phase;
    if (phase != null && phase != _lastReportedPhase) {
      _lastReportedPhase = phase;
      widget.onPhaseChanged?.call(phase);
    }
    final error = _bridge?.errorMessage;
    if (error != null && _bridge?.phase == Live2DBridgePhase.error) {
      widget.onError?.call(error);
    }
    // 新回执 → 上报宿主（缩放百分比要从**渲染面**的真值来，不能猜）。
    final ack = _bridge?.lastAck;
    if (ack != null && !identical(ack, _lastAck)) {
      _lastAck = ack;
      widget.onAck?.call(ack);
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
      stageColor: widget.stageColor,
      tier: widget.tier,
    );
    // 首帧 / 重建 iframe 后**补发背景图**（sync 不带 stage-bg 通道）。
    unawaited(bridge.sendStageBg(widget.stageImage));
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

  /// 协议 v1 `sync` 换模型，并**等到渲染面回执**才返回（R2 规矩）。
  ///
  /// 实现委托给 [`Live2DBridge.swapModel`]（协议层负责「发 sync 并等 loaded」）；
  /// 这里只处理「桥还没挂上」的情况。
  ///
  /// 返回 `true` = 渲染面确认真的换了；`false` = 没换成（超时 / 报错 / 回执报的是
  /// 别的 url）。调用方拿 `false` 时应如实说「已登记，但舞台未确认」。
  Future<bool> swapModel(
    String url, {
    Duration timeout = const Duration(seconds: 15),
  }) async {
    final Live2DBridge? bridge = _bridge;
    if (bridge == null) return false;
    return bridge.swapModel(url, timeout: timeout);
  }

  /// 同步舞台外观（协议 v1 `sync`）。
  void sync({
    String? model,
    double? scale,
    bool? dark,
    String? stageColor,
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
      stageColor: stageColor,
      tier: tier,
      clickEnabled: clickEnabled,
      paused: paused,
      lipSync: lipSync,
      idleEnabled: idleEnabled,
      mouthSensitivity: mouthSensitivity,
    );
  }

  /// 错误后重建 iframe（也适用于渲染端热更新后的重试）。
  /// 「重试」：重建 iframe（并重置错误态）。
  ///
  /// 宿主只需要 `GlobalKey<Live2DStageState>.currentState?.retry()`，
  /// 不必知道舞台内部是怎么重挂的。
  void retry() {
    final previous = _bridge;
    _bridge = null;
    if (previous != null) {
      previous.removeListener(_onBridgeChanged);
      unawaited(previous.destroy());
    }
    _hostError = null;
    // 重建后要**重新上报**阶段（否则宿主停在旧的 error 覆盖层上）。
    _lastReportedPhase = null;
    setState(() => _generation++);
  }

  @override
  Widget build(BuildContext context) {
    final bridge = _bridge;
    return Stack(
      fit: StackFit.expand,
      children: <Widget>[
        // 舞台底有两层真相：渲染面画的那层（iframe 里 canvas 的
        // `background-color`）与**这一层**（iframe 还没加载完、或渲染面
        // 拒绝了这个颜色时的兜底面）。两层必须同时变，否则会看到
        // 「底下的面已经到终色、上面的 iframe 还在旧色」。
        // 所以这里也读同一个插值值（P1-3）。
        AnimatedBuilder(
          animation: _stageColorMotion,
          builder: (BuildContext context, Widget? _) => ColoredBox(
            color: _displayedStageColor ?? appPaletteOf(context).stage,
          ),
        ),
        host.buildLive2DHost(
          key: ValueKey<int>(_generation),
          onTransport: _attach,
          onError: _handleHostError,
        ),
        // ── 加载态与错误态**不在这里画**（2026-09-11 修的双覆盖层） ──
        //
        // 过去这里另有一套「渲染面加载中 N%」徽标 + 「渲染面错误 + 重试」条，
        // 而 `StageHost` 同样画了一套「模型加载中 N%」幕布 + 「模型加载失败 +
        // 重试」。两套叠在舞台同一块区域上，用户会**同时看到两个重试按钮**、
        // 加载时看到两条不同措辞的进度。
        //
        // 现在唯一实现是 `StageHost`（它同时负责语义标签与指针垫层），
        // 本组件只保留**非状态类**的装饰徽标（FPS）。宿主拿阶段的方式是
        // `onPhaseChanged`，拿错误的方式是 `errorMessage`，重建的方式是
        // `retry()` —— 三条通路都在，一套 UI 就够。
        //
        // 回归：`test/stage_overlay_single_test.dart`。
        if (bridge != null && bridge.isReady && bridge.fps != null)
          Positioned(
            right: 12,
            top: 12,
            child: _StageBadge(label: '${bridge.fps} FPS'),
          ),
      ],
    );
  }
}

/// 解析渲染面认的舞台底色串（`#rgb` / `#rrggbb`）。
///
/// 渲染面**只认这两种**，这里也**只认这两种**：认不出来就返回 `null`
/// （= 不指定，让渲染面按 `dark` 自己决定）。宁可不发，也不要发一个格式
/// 可疑的串过去被静默拒掉——那会表现成「主题切了但舞台没变」，
/// 而且**不报错**。
Color? parseStageColorCss(String? css) {
  if (css == null) return null;
  final RegExpMatch? m = RegExp(
    r'^#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})$',
  ).firstMatch(css.trim());
  if (m == null) return null;
  String hex = m.group(1)!;
  if (hex.length == 3) {
    hex = hex.split('').map((String c) => '$c$c').join();
  }
  // 这是从**字符串**还原出来的颜色，不是设计决策——所以拼装用 `fromARGB`
  // 而不是写一个 `Color(0x…)` 字面量（后者会被裸色扫描当成绕过令牌的配色）。
  final int value = int.parse(hex, radix: 16);
  return Color.fromARGB(
    255,
    (value >> 16) & 0xFF,
    (value >> 8) & 0xFF,
    value & 0xFF,
  );
}

/// 舞台角上的装饰徽标（目前只有 FPS）。
///
/// **只是一个药丸本身**，不带 `Align`：定位由调用方的 `Positioned` 决定。
/// 过去它在内部又套了一层 `Align(topLeft)`，而调用方写的是
/// `Positioned(right: 12, top: 12)` —— `Align` 会撑满 Positioned 给的松弛约束，
/// 于是「右上角」实际渲染在左上角。去掉内层 `Align` 后位置才真的由调用方说了算。
class _StageBadge extends StatelessWidget {
  const _StageBadge({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: appColorsOf(context).glassScrim,
        borderRadius: BorderRadius.circular(AppRadius.pill),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: Space.s3,
          vertical: Space.s1,
        ),
        child: Text(label, style: Theme.of(context).textTheme.bodySmall),
      ),
    );
  }
}
