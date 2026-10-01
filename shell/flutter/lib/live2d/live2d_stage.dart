import 'dart:async';

import 'package:flutter/foundation.dart' show mapEquals, visibleForTesting;
import 'package:flutter/material.dart';

import '../api/ws_frame.dart';
import '../design/tokens.dart';
import '../settings/preset_labels.dart';
import '../ui/soft_motion.dart';
import '../ui/theme.dart';
import 'live2d_bridge.dart';
import 'live2d_host_stub.dart'
    if (dart.library.js_interop) 'live2d_host_web.dart' as host;
import 'live2d_transport.dart';
import 'preset_status.dart';
import 'render_events.dart';

/// [PresetStatus] 的**唯一真源**已搬到 `preset_status.dart`（阶段4d）：
/// 它现在是「渲染面 ack 的显示快照」，不是本地计时器。这里 re-export，
/// 既有调用方（`settings/sections/dev_tools_section.dart` 等）一行都不用改。
export 'preset_status.dart' show PresetStatus;

/// 动作预设的**出厂强度**（L1 产品级，2026-09-16）。
///
/// 1.2 让头 / 身的摆幅肉眼明显（1.0 偏「微抖」，用户实测反馈看不出来），
/// 仍在渲染面 `MAX_INTENSITY = 3.0` 的钳位内。调试面板可在 0.5~3.0 之间调。
const double kDefaultPresetIntensity = 1.2;

/// **基础表情强度**的出厂值（2026-09-23，表情调试 / 动作调试共用）。
///
/// 与手势的 [kDefaultPresetIntensity]（1.2）刻意不同：表情调试的目的是
/// 「确认基础表情强度」，1.0 就是表值本身——看得清、不放大。手势需要 1.2 才
/// 肉眼明显（用户实测 1.0 像微抖），所以两个滑条各有各的缺省，不合并。
const double kDefaultExpressionIntensity = 1.0;

/// 导演按句 cue 的**持有与应用策略**（纯逻辑，VM 可测；阶段3 起取代 W4 那条
/// 基于只读状态面的预设判据）。
///
/// # 为什么抽在这里（而不是留在 `main.dart`）
///
/// 与 PresetStatus（已搬到 `preset_status.dart`）同因：`main.dart` 经 `app/browser_io.dart` 依赖
/// `package:web`，在 `flutter test`（Dart VM）里**加载不了**；而「一份 plan
/// 整体替换、按句**取用一次即移除**、空 id 不动、`none` = 撤销」这条顺序
/// 恰恰是最容易写错、也最该被回归钉住的行为——阶段3 §6.2 明确：「同 seq
/// 重复帧只应用一次」这条保护由整表替换 + 取用即移除承担。
///
/// # 契约（阶段3 D10 / D13）
///
/// - [`replace`]：新 `action_cue` 到达即**整体替换**（一份 plan 覆盖上一份；
///   同 seq 多条只留最后一条 = map 语义）。
/// - [`take`]：该句音频**开始播放**时按 `sentence_seq` 取用一次，**取用即移除**
///   ⇒ 同一句（含同 seq 重复帧）只会被应用一次。
/// - [`applyForSeq`]：取到 cue 才下发；`preset_id` 为空串 = 「本轮不动」（不
///   调用下发）；`preset_id == 'none'` = 显式撤销，**照常下发一次**（渲染面把
///   `none` 翻成两槽 `Revoke`，与旧状态面通道的撤销同义；那条通道已退役）。
class DirectorCuePlan {
  final Map<int, ActionCue> _bySeq = <int, ActionCue>{};

  /// 当前计划里还剩多少条 cue（排障 / 测试用）。
  int get length => _bySeq.length;

  /// 新 plan 到达：**整体替换**。同 seq 的重复帧在这里被 map 语义吃掉。
  void replace(List<ActionCue> cues) {
    _bySeq
      ..clear()
      ..addEntries(
        cues.map((ActionCue c) => MapEntry<int, ActionCue>(c.sentenceSeq, c)),
      );
  }

  /// 按句子序号取用一次并移除；`seq == null` / 已取过 → `null`。
  ActionCue? take(int? seq) {
    if (seq == null) return null;
    return _bySeq.remove(seq);
  }

  /// 取用 + 下发：空 `presetId` = 本轮不动；其余（含 `'none'`）**恰好一次**。
  Future<void> applyForSeq(
    int? seq,
    Future<void> Function(ActionCue cue) apply,
  ) async {
    final ActionCue? cue = take(seq);
    if (cue == null || cue.presetId.isEmpty) return;
    await apply(cue);
  }
}

/// Live2D 舞台：持有 [Live2DBridge]，对外暴露口型与外观同步入口。
class Live2DStage extends StatefulWidget {
  const Live2DStage({
    super.key,
    this.model,
    this.scale = 1.0,
    this.dark = true,
    this.stageColor,
    this.stageImage,
    this.actionScales,
    this.presetLabels = PresetLabelTable.empty,
    this.tier,
    this.onError,
    this.onReady,
    this.onPhaseChanged,
    this.onProgress,
    this.onAck,
    this.onRenderEvent,
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

  /// 动作幅度倍率（`{head, body, expression}`，2026-09-16）。
  ///
  /// `null` = 不下发，渲染面用自己的出厂默认（0.75 / 0.80 / 1.0）。
  ///
  /// 值来自宿主算出的**当前有效值**（`ActionScalesSyncer.active()`：
  /// 临时覆盖 > 草稿 > 磁盘值）。它是 iframe 重建 / 重挂后的**自愈快照**——
  /// 舞台在 [_attach]（首帧 / retry 重挂）与 [didUpdateWidget]（值变化）时
  /// 用 [Live2DStageState] 的 `sendSync(actionScales: …)` 补发。
  /// 那是**唯一**的直发点；宿主侧的临时覆盖只经 syncer 这一个出口。
  final Map<String, double>? actionScales;

  /// 预设 id → 展示信息（**显示用**：倒计时 ttl 的通道判据）。
  ///
  /// 缺省空表 = 回落 [kExpressionPresetIds]（**不删那份常量**——它是取不到
  /// 标签表时的兜底，见 `settings/preset_labels.dart`）。
  final PresetLabelTable presetLabels;

  final int? tier;

  /// 渲染面阶段变化（宿主据此刷新覆盖层与舞台语义）。
  ///
  /// 没有它的话，宿主拿不到阶段变化：`Live2DStage` 内部 `setState` 只重建自己，
  /// 父级不会跟着重建，于是 `StageHost` 的加载覆盖层会**永远停在 loading**。
  final void Function(Live2DBridgePhase phase)? onPhaseChanged;

  /// 渲染面**加载进度**变化（0..1，`null` = 还不知道）。
  ///
  /// 为什么需要这条通路（2026-10-01，F-0001-4 / F-0010-2）：进度显示在
  /// `StageHost` 的加载幕布里，而那份 `progress` 是**宿主 build 时的快照**
  /// （`main.dart` 现读 `stageKey.currentState?.bridge?.progress`）。
  /// 桥每发一帧 progress 都会 `notifyListeners()`，但宿主不是订阅者——
  /// 它只在**自己**重建时才重新读一次。于是「模型加载中… 40%」会在整段
  /// 加载过程里冻住不动（FPS 徽标也曾同理）。
  ///
  /// 有了这条回调，宿主用**最小重建面**接上即可：值真的变了才 `setState`
  /// 一个 `double?` 字段（见 [onPhaseChanged] 的同款接法），整棵壳不参与。
  ///
  /// **只在值变化时触发**：加载期间进度帧可能很密，同值重复回调会把宿主
  /// 拖进无谓重建。`null → 0.0` 算变化（进度刚开始也是新事实）。
  final ValueChanged<double?>? onProgress;

  /// 错误回调（宿主可用于日志或全局提示）。
  final ValueChanged<String>? onError;

  /// 每收到一条**新的** `stage-ack` 回调一次。
  ///
  /// 宿主用它把「渲染面实际生效的缩放」显示出来。缺了它，舞台角标的百分比
  /// 只能等用户按一次放大/缩小才有值——首屏会一直显示一个「—」，
  /// 看起来像坏了（2026-09-11 在真浏览器里就是这么显示的）。
  final ValueChanged<StageAckEvent>? onAck;

  /// 渲染面**事件级 ack**（协议 §7：四条 ack + `segment-ended`，O13 冻结名）。
  ///
  /// 宿主用它把事件**汇入日志文本喂导演**（§7.3）。前端**不**据此维护镜像
  /// 状态、也不做每帧状态流（V8）——[Live2DStageState.presetStatus] 那份显示
  /// 快照是唯一例外，且它同样只由 ack 驱动。
  final ValueChanged<RenderEvent>? onRenderEvent;

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
  double? _lastReportedProgress;
  int _generation = 0;
  bool _readyNotified = false;

  /// 上一次 build 时读到的「就绪态」。**build 真的读了它**（角标显隐），
  /// 所以它变了才 setState；progress / fps / ack 这些数值类派生值不再
  /// 让整棵舞台重建（2026-10-01，F-0010-2）。
  bool _lastBuiltReady = false;

  /// 「动作调试」的显示快照（阶段4d 起**只由渲染面 ack 驱动**）。
  ///
  /// **没有本地计时器、没有本地推演**：快照在收到 `preset-applied` 时生成、
  /// 收到 `preset-replaced` / `preset-expired` / `preset-dropped` 时清空
  /// （见 [presetStatusUpdateFor]）。权威在渲染面，前端只是把回执显示出来。
  final ValueNotifier<PresetStatus?> presetStatus =
      ValueNotifier<PresetStatus?>(null);

  /// 渲染面事件订阅（每次 [_attach] 重新挂；随旧桥一起取消）。
  StreamSubscription<RenderEvent>? _renderSubscription;

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
    // 动作幅度是独立字段：也要在变化时补发（不能因为底色没变被 return 跳过）。
    if (!mapEquals(widget.actionScales, oldWidget.actionScales)) {
      _bridge?.sendSync(actionScales: widget.actionScales);
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
    unawaited(_renderSubscription?.cancel());
    presetStatus.dispose();
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
    // ── 重建面收窄（2026-10-01，F-0010-2）──
    //
    // 从前这个方法末尾无条件 `setState(() {})`：桥上**任何**一条通知
    // （每秒一帧 fps、加载期每一帧 progress、每一条 ack）都把整棵舞台
    // 重建一遍，连平台视图那一层一起。现在：
    //   · 阶段 / 就绪态变了 → setState（build 读它们决定角标显隐）；
    //   · progress    → 经 [Live2DStage.onProgress] 交给宿主（宿主只
    //                   setState 一个 double? 字段）；
    //   · fps         → 角标自己订阅 `bridge.fpsListenable`（见 `_FpsBadge`）。
    bool needsBuild = false;
    // 阶段变化先报给宿主（覆盖层/语义都靠它）。
    final phase = _bridge?.phase;
    if (phase != null && phase != _lastReportedPhase) {
      _lastReportedPhase = phase;
      widget.onPhaseChanged?.call(phase);
      needsBuild = true;
    }
    final progress = _bridge?.progress;
    if (progress != _lastReportedProgress) {
      _lastReportedProgress = progress;
      // 同值不回调：加载期的进度帧可能很密，重复回调 = 宿主拆房重建。
      widget.onProgress?.call(progress);
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
    if (ready != _lastBuiltReady) {
      _lastBuiltReady = ready;
      needsBuild = true;
    }
    if (needsBuild) setState(() {});
  }

  /// **仅测试**：VM 下 `buildLive2DHost` 解析到 `live2d_host_stub.dart`，它的
  /// 占位实现**从不回调** `onTransport`（见该文件与
  /// `test/asset_guard_preset_dispatch_test.dart` 的取证）⇒ 挂上去的舞台
  /// `_bridge` 恒为 null，「progress/fps 通知 → 重建 / 回调」这条链在
  /// `flutter test` 里一行都驱动不了。
  ///
  /// 这条口子只做一件事：让 widget 测试能挂一个 fake transport。生产路径
  /// （`buildLive2DHost(onTransport: _attach)`）一个字都不用改。
  @visibleForTesting
  void debugAttachTransport(Live2DTransport transport) => _attach(transport);

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
    unawaited(_renderSubscription?.cancel());
    final bridge = Live2DBridge(transport)..addListener(_onBridgeChanged);
    _renderSubscription = bridge.renderEvents.listen(_onRenderEvent);
    _bridge = bridge;
    setState(() {});
    bridge.start();
    // ready 前会入队，ready 后按序 flush。
    bridge.sendSync(
      model: widget.model,
      scale: widget.scale,
      dark: widget.dark,
      stageColor: widget.stageColor,
      actionScales: widget.actionScales,
      tier: widget.tier,
    );
    // 首帧 / 重建 iframe 后**补发背景图**（sync 不带 stage-bg 通道）。
    unawaited(bridge.sendStageBg(widget.stageImage));
  }

  /// 渲染面事件级 ack → ①宿主（导演日志）②调试面板的显示快照。
  ///
  /// **这里没有任何本地计时器 / 本地推演**：快照的生成与清空都由 ack 事件
  /// 触发（阶段4d 删掉了 v0 那条 200ms 猜剩余时长的 Timer.periodic）。
  void _onRenderEvent(RenderEvent event) {
    if (!mounted) return;
    widget.onRenderEvent?.call(event);
    final PresetStatusUpdate update = presetStatusUpdateFor(
      event,
      DateTime.now(),
    );
    switch (update.action) {
      case PresetStatusAction.set:
        presetStatus.value = update.status;
      case PresetStatusAction.clear:
        presetStatus.value = null;
      case PresetStatusAction.ignore:
        break;
    }
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

  /// 协议 v1 `preset`：把表演指令交给渲染面（导演 / 开发工具「动作调试」共用）。
  ///
  /// # 旧路径（缺 [field]）语义**逐字不变**
  ///
  /// - `id` 为空串 = 本轮不动（不发）；
  /// - `id == 'none'` = 立即撤销（渲染面把两槽翻成 `Revoke`）。
  ///
  /// # v1 路径（带 [field]）
  ///
  /// `field` ∈ `body` / `head` / `expression` 时走字段化通道：`x` / `y` /
  /// `z`、`hold`、`at` 与 `seq` 原样透传（渲染面按协议 §4 合成）。此时 `id`
  /// 只对 `expression` 有意义（表情面板 id），`body` / `head` 允许为空。
  ///
  /// `intensity` 缺省 [kDefaultPresetIntensity]（1.2，肉眼明显）；渲染面钳位
  /// `[0, 3]`，非法值等同缺省。调试面板的滑条直接把它透传过去。
  ///
  /// **本地不再记录「还剩多久」**（阶段4d）：没有计时器、没有 ttl 推演——
  /// 显示快照 [presetStatus] 只由渲染面 ack 生成（[presetStatusUpdateFor]），
  /// 这里只把 `ttlMs` 透传给渲染面。
  Future<void> applyPreset(
    String id, {
    String source = 'ui',
    double intensity = kDefaultPresetIntensity,
    double? ttlMs,
    String? field,
    double? x,
    double? y,
    double? z,
    bool? hold,
    String? at,
    int? seq,
    int? epoch,
    int? sentenceSeq,
  }) async {
    final bool hasField = field != null && field.isNotEmpty;
    if (!hasField && id.isEmpty) return; // 旧语义：空串 = 本轮不动
    await _bridge?.sendPreset(
      id,
      source: source,
      intensity: intensity,
      ttlMs: ttlMs,
      field: field,
      x: x,
      y: y,
      z: z,
      hold: hold,
      at: at,
      seq: seq,
      epoch: epoch,
      sentenceSeq: sentenceSeq,
    );
  }

  /// 协议 v1 `stage-clock`：把音频播放时钟下发渲染面（协议 §6.5 / O13）。
  ///
  /// 节拍由 [AudioPlayer] 的 30ms ticker 提供（`audio.stageClock` 流），这里只
  /// 转发；窗口未就绪时桥会入队（与其它下行消息同一条路径）。
  void sendStageClock({int? seg, required int posMs, required bool playing}) {
    unawaited(
      _bridge?.sendStageClock(seg: seg, posMs: posMs, playing: playing),
    );
  }

  /// 协议 v1 stage-zoom（in / out / reset）。
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

  /// 下发动作幅度倍率（`sync.actionScales`，三项都必须有限）。
  ///
  /// 与 [sync] 分开是为了让「当前该用哪组倍率」只由宿主一份逻辑决定
  /// （草稿优先的即时预览，见 `app/shell_prefs.dart`），渲染面只收结果。
  /// 非有限值 / 不是三项 → 静默不发（宁可不发，也不下发一个污染舞台的快照）。
  void applyActionScales(Map<String, double> scales) {
    if (scales.length != 3) return;
    if (!scales.values.every((double v) => v.isFinite)) return;
    _bridge?.sendSync(actionScales: scales);
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
    Map<String, double>? actionScales,
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
      actionScales: actionScales,
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
    unawaited(_renderSubscription?.cancel());
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
        //
        // FPS 角标**自己**订阅 `bridge.fpsListenable`（2026-10-01，F-0010-2）：
        // 渲染面每秒一帧 fps，靠整棵舞台 `setState` 刷新等于每秒重建一次
        // 平台视图那一层；订阅之后重建面只剩这个几十像素的药丸
        // （值还没到 → 角标自己占零尺寸，不需要舞台重建）。
        if (bridge != null && bridge.isReady)
          Positioned(right: 12, top: 12, child: _FpsBadge(bridge: bridge)),
      ],
    );
  }
}

/// 该预设是否走**表情**通道（**显示用**，W7 B②）。
///
/// 唯一真源是标签表的 `channel` 字段（`assets/actions/preset_labels.json`，
/// 与渲染面 `presets.json` 对齐）；**取不到表 / 该 id 没有 channel 时才回落**
/// [kExpressionPresetIds]——那份常量是兜底，不删。
///
/// 与 `settings/mods/director_panel.dart` 的 `directorPresetText` 同口径：
/// 两处都优先查表，保证「表情 / 短动作」标签只有一份事实。
bool isExpressionChannel(String id, {PresetLabelTable? labels}) {
  final String? channel = labels?[id]?.channel;
  if (channel != null) return channel == 'expression';
  return isExpressionPreset(id);
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

/// FPS 角标的**订阅实体**：只监听 `bridge.fpsListenable`。
///
/// 它存在的唯一理由就是「最小重建面」（F-0010-2）：渲染面每秒一帧 `fps`，
/// 而舞台的 build 还背着平台视图那一层——把订阅收在这个小 widget 里，
/// 每秒只有它重建。值还没到（`fps == null`）时占零尺寸，不改变观感。
class _FpsBadge extends StatelessWidget {
  const _FpsBadge({required this.bridge});

  final Live2DBridge bridge;

  @override
  Widget build(BuildContext context) => ValueListenableBuilder<int?>(
    valueListenable: bridge.fpsListenable,
    builder: (BuildContext context, int? fps, Widget? _) => fps == null
        ? const SizedBox.shrink()
        : _StageBadge(label: '$fps FPS'),
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
