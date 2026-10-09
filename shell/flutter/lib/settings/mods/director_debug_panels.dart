/// 开发者模式的两块调试 + 临时幅度覆盖（2026-10-09 从核心「开发模式」页搬到
/// 「扩展 → 导演」卡片的下级块）。
///
/// # 为什么搬
///
/// 表情调试 / 动作调试 / 临时幅度覆盖产出的是**动作与表情**——它们与导演 Mod
/// 是同一件事的两端（导演决定「演什么」，这两块让人**手动**试一下）。住在导演
/// 卡片上，用户不必先想起「开发模式页」才能试一条表情。
///
/// **仍只在开发者模式里显示**（`ModPanelContext.devMode`）：产品面不放调试按钮。
/// 能力一个都没删，也不接 `action_tx`——下发仍走渲染面 `preset` 帧（与导演同一条）。
///
/// # 它是什么（逐条沿用原头注）
///
/// - **表情调试**：只列 `channel=expression` 的包（标签表派生），点按只发 Face 槽；
///   独立滑条「基础表情强度」（缺省 1.0）。
/// - **动作调试**：只列 `channel=motion` 的包，滑条「动作强度」（缺省 1.2）；
///   另有「叠加基础表情」开关（**缺省关**）。
/// - **临时幅度覆盖**：三项倍率只发渲染面、**不落盘**（产品值才是真源）。
library;

import 'dart:async';

import 'package:flutter/foundation.dart' show ValueListenable;
import 'package:flutter/material.dart';

import '../../api/settings_models.dart';
import '../../design/tokens.dart';
import '../../live2d/live2d_stage.dart'
    show PresetStatus, kDefaultExpressionIntensity, kDefaultPresetIntensity;
import '../../ui/emphasized_text.dart';
import '../../ui/field_row.dart';
import '../../ui/inline_notice.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';
import '../preset_labels.dart';
/// 「动作调试」下发一条预设：id + 强度（none = 归零，强度被忽略）。
typedef PresetApply = void Function(String id, double intensity);

/// 临时覆盖三项幅度倍率（head / body / expression）。
typedef PresetScaleApply = void Function(double head, double body, double expression);

/// 可注入时钟（表情到点计时用；不要在 widget 里写死 `DateTime.now()`）。
typedef DebugClock = DateTime Function();

// 表情的**到点时刻不在这里写死**：权威值在渲染面 preset 的 `duration_ms`
// （表情 2600ms；`crates/l2d-wasm-demo/src/preset/mod.rs` 的 `EXPRESSION_MS`），
// 经 `live2d_stage.dart` 的显示用 ttl 投影到 [PresetStatus.ttl]。面板只消费
// 舞台那个 `PresetStatus` 通知器——**不在 Dart 里再抄一份时长**（RESEARCH §4 E7；
// 也避开 design_tokens_lint 的「协议时长只能走豁免路径」红线）。

/// 「全部扫一遍」的步进间隔（每条演一小会儿，肉眼能分清）。
///
/// 取 UI 动效 4 档里的 [AppDurations.reveal]（600ms）：**不新增裸时长**——
/// 门禁 design_tokens_lint_test 只允许 UI 时长来自那 4 档。
const Duration kDebugSweepInterval = AppDurations.reveal;

/// 开发模式的**两块调试**（2026-09-23 从「万能动作调试」拆开）。
///
/// - **表情调试**：只列 `channel=expression` 的包（**标签表派生**），点按只发
///   Face 槽；独立滑条「基础表情强度」（缺省 [kDefaultExpressionIntensity] = 1.0）。
/// - **动作调试**：只列 `channel=motion` 的包，滑条「动作强度」
///   （缺省 [kDefaultPresetIntensity] = 1.2）；另有「叠加基础表情」开关
///   （**缺省关**，2026-09-21 W3）——开着且基础表情**仍在有效期内**时，播放手势
///   会先把这份表情写进 Face 槽，再写手势（Face → Gesture 与渲染面同帧写入顺序
///   一致，共享通道由手势赢）。
///
/// 两块共享同一份「基础表情」状态，所以「动作扫一遍」自动带上当前基础表情。
/// 基础表情按渲染面同口径（[PresetStatus.ttl]，表情 2600ms）**到点即视为 none**：
/// UI 显示「无（已到点）」，叠加与重发都不再认它。真正的执行者仍是渲染面的双槽
/// 状态机（表情 2600ms / 手势 900–1000ms，各按 ttl 撤销）。
class DebugPanels extends StatefulWidget {
  const DebugPanels({
    this.onApplyPreset,
    this.status,
    this.productScales,
    this.pinnedScales,
    this.onApplyScales,
    this.onClearScales,
    this.labels = PresetLabelTable.empty,
    this.clock,
    super.key,
  });

  /// 直发一条预设（none = 归零）。为空时按钮禁用（不假装能点）。
  final PresetApply? onApplyPreset;

  /// 舞台的本地状态（Live2DStageState.presetStatus）。
  final ValueListenable<PresetStatus?>? status;

  /// 产品设置里的动作幅度（显示 + 「恢复」目标）；null = 未加载。
  final ActionSettingsView? productScales;

  /// 当前临时覆盖（ActionScalesSyncer.pinned）；null = 没有。
  ///
  /// 滑条初值 = `pinnedScales ?? productScales`，所以临时覆盖期间离开分区
  /// 再回来，面板显示的仍是渲染面正在用的那组值（T4 实测症状的根因）。
  final Map<String, double>? pinnedScales;

  /// 临时覆盖（不落盘）；null 时滑条禁用。
  final PresetScaleApply? onApplyScales;

  /// 清掉临时覆盖并写回产品值（null 时「恢复产品设置」禁用）。
  final VoidCallback? onClearScales;

  /// 预设 id → 展示名（读共享标签表；空表回落到 id）。
  final PresetLabelTable labels;

  /// 可注入时钟（表情到点计时）；null = [DateTime.now]。
  final DebugClock? clock;

  @override
  State<DebugPanels> createState() => _DebugPanelsState();
}

class _DebugPanelsState extends State<DebugPanels> {
  /// 当前「基础表情」包（面板**最近一次真正下发**的那条）；none = 没有。
  ///
  /// 它只记「最近点了哪条」；**是否还在演**由 [_expressionLive] 用注入时钟 +
  /// 渲染面回执的 [PresetStatus.ttl] 判定——到点后这里仍留着 id 供文案说
  /// 「已到点」，但行为上一律按 none 对待（不叠加、不重发）。
  String _expressionId = 'none';

  /// 基础表情的**到点时刻**；null = 还没拿到渲染面回执（或不生效）。
  DateTime? _expressionExpiresAt;

  /// 已经消费过的 [PresetStatus.startedAt]，避免舞台 200ms 一次的等价刷新
  /// 把到点时刻反复往后推。
  DateTime? _statusStartConsumed;

  /// 基础表情到点时触发一次重建（UI 从「某条」切到「无（已到点）」）。
  Timer? _faceExpiryTimer;

  /// 基础表情强度：表情调试用它，动作调试「叠加基础表情」也用它。
  double _faceIntensity = kDefaultExpressionIntensity;

  /// 手势强度。
  double _actionIntensity = kDefaultPresetIntensity;

  /// 播放手势时是否先把基础表情写进 Face 槽。
  ///
  /// **缺省 false**（2026-09-21 W3，症状①）：调试面板不再默认替用户接管表情槽，
  /// 否则「点手势总带着上一张老脸」。开关仍在、语义不变，只是要用户显式打开。
  bool _overlayFace = false;

  Timer? _faceSweepTimer;
  Timer? _gestureSweepTimer;
  int _faceSweepIndex = 0;
  int _gestureSweepIndex = 0;

  /// 时钟（**可注入**；测试用假时钟钉住到点行为）。
  DateTime _now() => (widget.clock ?? DateTime.now)();

  /// 表情 / 手势按钮清单的**唯一来源**：标签表的 `channel` 字段（空表 → 空列表）。
  List<String> get _expressionIds => widget.labels.idsForChannel('expression');
  List<String> get _gestureIds => widget.labels.idsForChannel('motion');

  /// 「当前基础表情」是否仍在演（到点时刻 = 渲染面回执的 [PresetStatus.ttl]）。
  ///
  /// 拿不到回执（`_expressionExpiresAt == null`）时**不算在演**：没有渲染面
  /// 权威 ttl 就不假装知道它演多久——这也保证「到点后不重发」是真的。
  bool get _expressionLive {
    final DateTime? until = _expressionExpiresAt;
    return _expressionId != 'none' && until != null && _now().isBefore(until);
  }

  // 2026-10-09：「是否已到点」的**文案投影**（原来那行「无（已到点）」）随
  // 控件说明一起删了；判据本体仍是 [_expressionLive]——到点即等于 none，
  // 叠加与重发都不再认它（回归见 developer_section_test）。

  /// 叠加 / 展示用的**有效**表情 id：到点后就是 none（不叠加、不重发）。
  String get _liveExpressionId => _expressionLive ? _expressionId : 'none';

  late double _head;
  late double _body;
  late double _expression;

  bool get _enabled => widget.onApplyPreset != null;
  bool get _scalesEnabled => widget.onApplyScales != null;
  bool get _clearEnabled => widget.onClearScales != null;

  /// 是否有临时覆盖：真源 = 宿主只读传进来的 [DebugPanels.pinnedScales]。
  bool get _hasPin => widget.pinnedScales != null;

  @override
  void initState() {
    super.initState();
    _syncScalesFromEffective();
    widget.status?.addListener(_onStatusChanged);
  }

  @override
  void didUpdateWidget(DebugPanels oldWidget) {
    super.didUpdateWidget(oldWidget);
    // pinnedScales 变化也要重新播种：宿主清了临时覆盖（或换了一组临时值）时，
    // 滑条必须跟着回到渲染面的有效值，而不是停在上一组。
    if (widget.productScales != oldWidget.productScales ||
        widget.pinnedScales != oldWidget.pinnedScales) {
      _syncScalesFromEffective();
    }
    // 舞台重建会换一个 presetStatus 通知器：跟着换监听，别听死那个旧的。
    if (widget.status != oldWidget.status) {
      oldWidget.status?.removeListener(_onStatusChanged);
      widget.status?.addListener(_onStatusChanged);
    }
  }

  /// 滑条初值 = **渲染面当前有效值**：临时覆盖优先，否则产品值。
  ///
  /// 不能只看产品值：临时覆盖生效期间离开分区再回来，滑条会掉回产品值，
  /// 与舞台 / HUD 上真正生效的临时值分叉（T4 实测症状）。
  void _syncScalesFromEffective() => _seedScales(widget.pinnedScales);

  /// 播下三项滑条值：`pinned` 非空就用它，否则用产品值 / 出厂默认。
  void _seedScales(Map<String, double>? pinned) {
    final ActionSettingsView? p = widget.productScales;
    _head =
        pinned?['head'] ?? p?.headScale ?? ActionSettingsView.defaultHeadScale;
    _body =
        pinned?['body'] ?? p?.bodyScale ?? ActionSettingsView.defaultBodyScale;
    _expression = pinned?['expression'] ??
        p?.expressionScale ??
        ActionSettingsView.defaultExpressionScale;
  }

  void _applyScales() => widget.onApplyScales?.call(_head, _body, _expression);

  /// 「恢复产品设置」：滑条回到产品值，并让宿主**清掉临时覆盖**
  /// （不能走 [_applyScales]——那会把当前临时值再钉一次）。
  ///
  /// 这里**只**按产品值播种（`_seedScales(null)`）：宿主清 pin 与随后的重建
  /// 排在这一帧之后，浮层宿主（medium 底部浮层）甚至不会因这次点击重建——
  /// 若仍读旧 pin，滑条会停在刚被清掉的那组临时值上。
  void _resetScales() {
    setState(() => _seedScales(null));
    widget.onClearScales?.call();
  }

  @override
  void dispose() {
    widget.status?.removeListener(_onStatusChanged);
    _faceSweepTimer?.cancel();
    _gestureSweepTimer?.cancel();
    _faceExpiryTimer?.cancel();
    super.dispose();
  }

  /// 只发 Face 槽；none = 两槽同清（渲染面协议）。
  void _applyExpression(String id) {
    widget.onApplyPreset?.call(id, id == 'none' ? 0 : _faceIntensity);
  }

  /// 记录「面板刚把哪条基础表情发下去」（none = 清掉计时）。
  ///
  /// **不在这里写死 ttl**：到点时刻等渲染面回执 [PresetStatus.ttl]（权威）来定，
  /// 见 [_onStatusChanged]。**到点只是重建 UI**：[_expressionId] 保留最后一条
  /// 供文案说「已到点」，真正的有效判据始终是 [_expressionLive]（读注入时钟），
  /// 所以即使定时器晚到、甚至测试里根本不推进定时器，只要时钟过了到点时刻就
  /// 不会再重发。
  void _setBaseExpression(String id) {
    _faceExpiryTimer?.cancel();
    _faceExpiryTimer = null;
    _statusStartConsumed = null;
    setState(() {
      _expressionId = id;
      _expressionExpiresAt = null;
    });
  }

  /// 舞台回执驱动到点时刻：只认**与当前基础表情同 id** 的那条 [PresetStatus]，
  /// 用它的 [PresetStatus.ttl]（渲染面 `duration_ms` 的投影）起算。
  ///
  /// 用 [PresetStatus.startedAt] 去重：舞台每 200ms 重发一个等价对象，不能让它
  /// 把到点时刻一路往后推——那正是「老表情续期」的翻版。
  void _onStatusChanged() {
    final PresetStatus? s = widget.status?.value;
    if (s == null || _expressionId == 'none' || s.id != _expressionId) return;
    if (_statusStartConsumed == s.startedAt) return;
    _statusStartConsumed = s.startedAt;
    _armExpressionExpiry(_now().add(s.ttl));
  }

  /// 按 [until] 定下到点时刻并排一个一次性重建定时器（到点后不重发）。
  void _armExpressionExpiry(DateTime until) {
    _faceExpiryTimer?.cancel();
    final Duration left = until.difference(_now());
    _faceExpiryTimer = Timer(left.isNegative ? Duration.zero : left, () {
      _faceExpiryTimer = null;
      if (mounted) setState(() {});
    });
    if (mounted) setState(() => _expressionExpiresAt = until);
  }

  /// 发手势：**先** Face（可选叠加）再 Gesture。
  ///
  /// 叠加基础表情（W3 症状①修复）：叠加的输入只认「**仍然有效**」的那条表情
  /// [_liveExpressionId]——它按渲染面 preset 的 duration_ms（表情 2600ms）到点，
  /// 到点后就是 none。于是：
  ///
  /// - 点手势**不会**把一张演完的老脸重新点亮 / 续期——过期即不发；
  /// - 表情要再看一次，只能由用户重新点那条表情按钮（那才是「发一条新表情」，
  ///   会重新起算它自己的 ttl），手势绝不承担「把旧表情重发一遍来保活」的角色。
  ///
  /// 手势本身始终只是一条 Gesture 槽指令，不带任何表情状态。
  void _applyGesture(String id) {
    final String face = _liveExpressionId;
    if (_overlayFace && face != 'none') {
      widget.onApplyPreset?.call(face, _faceIntensity);
    }
    widget.onApplyPreset?.call(id, _actionIntensity);
  }

  /// 表情扫一遍：只扫表情包；**不**在结尾送 none（免得误清正在演的手势槽），
  /// 最后一条按自己的表情 ttl 到点撤销。
  void _sweepExpressions() {
    final List<String> ids = _expressionIds;
    _faceSweepTimer?.cancel();
    _faceSweepIndex = 0;
    _faceSweepTimer = Timer.periodic(kDebugSweepInterval, (Timer t) {
      if (_faceSweepIndex >= ids.length) {
        t.cancel();
        _faceSweepTimer = null;
        if (mounted) setState(() {});
        return;
      }
      _applyExpression(ids[_faceSweepIndex]);
      if (mounted) setState(() => _faceSweepIndex++);
    });
  }

  /// 手势扫一遍：每一步都带当前基础表情（若开关开着且仍在有效期内）。
  void _sweepGestures() {
    final List<String> ids = _gestureIds;
    _gestureSweepTimer?.cancel();
    _gestureSweepIndex = 0;
    _gestureSweepTimer = Timer.periodic(kDebugSweepInterval, (Timer t) {
      if (_gestureSweepIndex >= ids.length) {
        t.cancel();
        _gestureSweepTimer = null;
        if (mounted) setState(() {});
        return;
      }
      _applyGesture(ids[_gestureSweepIndex]);
      if (mounted) setState(() => _gestureSweepIndex++);
    });
  }

  /// 取不到标签表（或表里没有该 channel）时**如实说明**，而不是静默渲染一个
  /// 空按钮组——「空按钮组」会被读成「这个功能不存在」，与事实相反。
  Widget _missingPresetNotice(String channel, String kind) {
    final String why = widget.labels.length == 0
        ? '预设标签表未加载（GET /actions/preset_labels.json 取不到）'
        : '预设标签表里没有 channel=$channel 的条目';
    return InlineNotice(
      message:
          '$why，因此不列出$kind按钮——这不是「没有$kind」，'
          '请检查静态路由后刷新页面。',
      severity: NoticeSeverity.warning,
      dense: true,
    );
  }

  String _statusLine(PresetStatus? s) {
    if (s == null) return '当前没有预设（已归零 / 已到点）';
    final Duration left = s.remaining(_now());
    return '最近触发：${s.id} · 来源 ${s.source} · 约剩 ${left.inMilliseconds} ms';
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: appColorsOf(context).contentMuted,
    );
    final bool sweepingFace = _faceSweepTimer != null;
    final bool sweepingGesture = _gestureSweepTimer != null;
    final List<String> expressionIds = _expressionIds;
    final List<String> gestureIds = _gestureIds;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // ── 表情调试（只发 Face 槽） ──
        const SectionHeader(title: '表情调试'),
        SliderField(
          label: '基础表情强度',
          icon: Icons.mood,
          value: _faceIntensity,
          min: 0.5,
          max: 3.0,
          divisions: 10,
          percentage: false,
          suffix: ' 倍',
          description: '0.5-3.0',
          enabled: _enabled,
          onChanged: (double v) => setState(() => _faceIntensity = v),
        ),
        if (expressionIds.isEmpty)
          _missingPresetNotice('expression', '表情')
        else
          Wrap(
            spacing: Space.s1,
            runSpacing: Space.s1,
            children: <Widget>[
              for (final String id in expressionIds)
                OutlinedButton(
                  onPressed: _enabled
                      ? () {
                          _setBaseExpression(id);
                          _applyExpression(id);
                        }
                      : null,
                  child: Text(widget.labels.display(id)),
                ),
            ],
          ),
        const SizedBox(height: Space.s1),
        Row(
          children: <Widget>[
            TextButton(
              onPressed: _enabled && !sweepingFace && expressionIds.isNotEmpty
                  ? _sweepExpressions
                  : null,
              child: Text(
                sweepingFace
                    ? '表情扫一遍中…（第 $_faceSweepIndex 条）'
                    : '表情扫一遍',
              ),
            ),
            const SizedBox(width: Space.s1),
            TextButton(
              // 语义与 W4 无关：这里只是「两槽同清」的调试入口，原样保留。
              onPressed: _enabled
                  ? () {
                      _setBaseExpression('none');
                      _applyExpression('none');
                    }
                  : null,
              child: const Text('归零（none，两槽同清）'),
            ),
          ],
        ),
        const Divider(),
        // ── 动作调试（只发 Gesture 槽，可叠加基础表情） ──
        const SectionHeader(title: '动作调试'),
        SliderField(
          label: '动作强度',
          icon: Icons.face_retouching_natural,
          value: _actionIntensity,
          min: 0.5,
          max: 3.0,
          divisions: 10,
          percentage: false,
          suffix: ' 倍',
          description: '0.5-3.0',
          enabled: _enabled,
          onChanged: (double v) => setState(() => _actionIntensity = v),
        ),
        ToggleField(
          label: '叠加基础表情',
          icon: Icons.layers_outlined,
          value: _overlayFace,
          enabled: _enabled,
          onChanged: (bool v) => setState(() => _overlayFace = v),
        ),
        if (gestureIds.isEmpty)
          _missingPresetNotice('motion', '手势')
        else
          Wrap(
            spacing: Space.s1,
            runSpacing: Space.s1,
            children: <Widget>[
              for (final String id in gestureIds)
                OutlinedButton(
                  onPressed: _enabled ? () => _applyGesture(id) : null,
                  child: Text(widget.labels.display(id)),
                ),
            ],
          ),
        const SizedBox(height: Space.s1),
        Row(
          children: <Widget>[
            TextButton(
              onPressed: _enabled && !sweepingGesture && gestureIds.isNotEmpty
                  ? _sweepGestures
                  : null,
              child: Text(
                sweepingGesture
                    ? '动作扫一遍中…（第 $_gestureSweepIndex 条）'
                    : '动作扫一遍',
              ),
            ),
            const SizedBox(width: Space.s1),
            TextButton(
              onPressed: _enabled ? () => _applyExpression('none') : null,
              child: const Text('归零（none，两槽同清）'),
            ),
          ],
        ),
        const SizedBox(height: Space.s1),
        // 当前状态：本地镜像（最近触发）＋指向渲染面 HUD 的权威行。
        if (widget.status == null)
          Text('当前状态：舞台未挂载，本地状态不可用（看渲染面 HUD 的 preset: 行）', style: muted)
        else
          ValueListenableBuilder<PresetStatus?>(
            valueListenable: widget.status!,
            builder: (BuildContext context, PresetStatus? s, Widget? _) =>
                Text('当前状态：${_statusLine(s)}', style: muted),
          ),
        const SizedBox(height: Space.s1),
        Text(
          'HUD 权威行形如 preset: face=<id> gesture=<id> face_intensity=<倍率> src=… ms；'
          '点了没反应 = 本皮套缺那条参数，渲染面静默 no-op。',
          style: muted,
        ),
        if (widget.productScales != null) ...[
          const Divider(),
          Text('临时幅度覆盖', style: theme.textTheme.labelLarge),
          const SizedBox(height: Space.s1),
          // 两态明示：有临时覆盖时把「面板滑条 = 渲染面有效值」说清，
          // 免得用户看到滑条与导演卡片里的产品值不同却不知道为什么。
          if (_hasPin) ...[
            EmphasizedText(
              '**临时覆盖生效中**（不落盘，点「恢复产品设置」清除）：'
              '下面三个滑条显示的就是渲染面当前生效的值，不是产品设置值。',
              style: muted,
            ),
            const SizedBox(height: Space.s1),
          ],
          EmphasizedText(
            '优先级（W7，2026-09-23 起）：**临时覆盖 > 草稿 > 磁盘值**。'
            '「应用到渲染面（临时）」后这份临时值一直生效（**不落盘**，'
            '刷新页面即失效）；拖动「扩展 → 导演卡片 → 动作幅度」的草稿、切换主题、'
            '调音量都**不会**再冲掉它——只有点「恢复产品设置」才清掉临时覆盖'
            '并把产品值写回舞台。',
            style: muted,
          ),
          SliderField(
            label: '临时 head',
            icon: Icons.face_retouching_natural,
            value: _head,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: _scalesEnabled,
            onChanged: (double v) => setState(() => _head = v),
          ),
          SliderField(
            label: '临时 body',
            icon: Icons.accessibility_new,
            value: _body,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: _scalesEnabled,
            onChanged: (double v) => setState(() => _body = v),
          ),
          SliderField(
            label: '临时 expression',
            icon: Icons.mood,
            value: _expression,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: _scalesEnabled,
            onChanged: (double v) => setState(() => _expression = v),
          ),
          Row(
            children: <Widget>[
              TextButton(
                onPressed: _scalesEnabled ? _applyScales : null,
                child: const Text('应用到渲染面（临时）'),
              ),
              const SizedBox(width: Space.s1),
              TextButton(
                onPressed: _clearEnabled ? _resetScales : null,
                child: const Text('恢复产品设置'),
              ),
            ],
          ),
        ],
      ],
    );
  }
}
