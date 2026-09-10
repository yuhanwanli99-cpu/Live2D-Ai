/// 应用外壳（L3）：按 `SizeClass` 选导航形态，**舞台恒常驻**。
///
/// # 保活是这一层唯一的硬约束
///
/// 舞台是一个 `<iframe>` 平台视图。它一旦离开 Widget 树，iframe 就被销毁，
/// 模型重载、口型时间轴清零、当前动作中断。所以：
///
/// - **三种断点共用同一个 `Flex`**（只换 `direction` 与子项宽度），
///   **不写** `if (compact) Column(...) else Row(...)`。后者在两套结构之间切换时
///   会让子树被反激活重建 —— 拖动窗口跨过 900 px 就会重载模型。
/// - 设置是**叠加物或浮层**，永远不替换舞台（`showModalBottomSheet` 的 overlay
///   不卸载下层）。
/// - 舞台挂稳定 `GlobalKey`：即便将来有人改动了树形状，reparent 也保留状态。
///
/// `stage_keepalive_test.dart` 用「`initState` 调用次数」把这条约束钉死。
///
/// # 键盘解锁音频（规格 §7.6，钉子 9）
///
/// 「默认出声」与浏览器 autoplay 限制冲突：`AudioContext` 需要**先有一次用户手势**。
/// 只挂 `onPointerDown` 会让**纯键盘用户永远没有声音**（消息发得出去、嘴在动、
/// 但没有声音），这与「默认出声，否则用户会以为 TTS 坏了」的意图直接冲突。
/// 所以同时挂两条路径：指针 + `HardwareKeyboard` 全局处理器。
///
/// # 诚实记录的偏离：内联侧板不实现「点空白关闭」
///
/// 规格 §4.2 写的是「可 Esc / 点空白关闭」。内联侧板浮在**舞台列**之上，
/// 而舞台是 iframe 平台视图——**平台视图会先吃掉指针事件**，铺在下层的
/// 点击捕获区拿不到舞台区域的点击。能拿到的只有少量 Flutter 绘制的空隙，
/// 那不足以称为「点空白关闭」。因此实际提供三条关闭路径：
/// **Esc / 侧板右上角 ✕ / 再点一次 rail 上的同一个分区（切换语义）**。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../api/ws_status.dart';
import '../chat/chat_message.dart';
import '../design/breakpoints.dart';
import '../design/tokens.dart';
import '../settings/settings_sections.dart';
import '../live2d/live2d_bridge.dart';
import '../state/ui_phase.dart';
import '../ui/chat_panel.dart';
import '../ui/connection_badge.dart';
import '../ui/error_banner.dart';
import '../ui/settings_scaffold.dart';
import '../ui/stage_host.dart';
import '../ui/state_pill.dart';
import '../ui/theme.dart';
import 'app_shortcuts.dart';
import 'nav_host.dart';

class AppShell extends StatefulWidget {
  const AppShell({
    required this.stage,
    required this.phase,
    required this.wsStatus,
    required this.messages,
    required this.input,
    required this.onSend,
    required this.onStop,
    required this.onRetryConnection,
    required this.volume,
    required this.muted,
    required this.onVolumeChanged,
    required this.onMutedChanged,
    required this.sections,
    required this.sectionBuilder,
    this.section = SettingsSection.actions,
    this.onSectionChanged,
    this.error,
    this.errorActions = const <ErrorAction>[],
    this.onDismissError,
    this.serverMuted = false,
    this.audioUnlocked = true,
    this.onEnableSound,
    this.onUserGesture,
    this.onRetryLast,
    this.devMode = false,
    this.settingsDirty = false,
    this.settingsSaving = false,
    this.settingsStatus,
    this.settingsStatusIsError = false,
    this.onSaveSettings,
    this.onDiscardSettings,
    this.confirmDiscard,
    this.stageCorner,
    this.shortcuts = const AppShortcutCallbacks(isMacOS: false),
    this.announcement,
    this.modelName,
    this.stagePhase = Live2DBridgePhase.loading,
    this.stageProgress,
    this.stageError,
    this.onRetryStage,
    super.key,
  });

  /// 舞台本体。外壳**不**构造它、**不**替换它——只负责放在哪。
  final Widget stage;

  final UiPhase phase;
  final WsStatus wsStatus;

  final List<ChatMessage> messages;
  final TextEditingController input;
  final VoidCallback onSend;
  final VoidCallback onStop;
  final VoidCallback onRetryConnection;

  final double volume;
  final bool muted;
  final ValueChanged<double> onVolumeChanged;
  final ValueChanged<bool> onMutedChanged;
  final bool serverMuted;
  final bool audioUnlocked;
  final VoidCallback? onEnableSound;

  /// 任意用户手势（指针按下 / 任意按键）——用于解锁 WebAudio。
  final VoidCallback? onUserGesture;

  final String? error;
  final List<ErrorAction> errorActions;
  final VoidCallback? onDismissError;
  final VoidCallback? onRetryLast;

  /// 可见分区（已过滤 dev）。
  final List<SettingsSection> sections;

  /// 当前分区（受控；外壳不持有它——导航状态属于宿主）。
  final SettingsSection section;
  final ValueChanged<SettingsSection>? onSectionChanged;

  /// 分区内容构建器（**必需**：外壳不自己造设置面板内容）。
  final Widget Function(BuildContext context, SettingsSection section)
  sectionBuilder;

  final bool devMode;

  // ── 设置草稿的生命周期（外壳只负责**呈现与拦截**，不持有状态） ──
  //
  // 状态在 `SettingsController` 里；外壳拿到的只是「dirty 吗 / 在保存吗 /
  // 上次结果是什么」。这样外壳保持可测（不需要造一个真的 ApiClient）。

  /// 有没有未保存的改动。
  final bool settingsDirty;
  final bool settingsSaving;

  /// 上次保存的结果文案（已按 `apply_status` 分流）。
  final String? settingsStatus;
  final bool settingsStatusIsError;

  final VoidCallback? onSaveSettings;
  final VoidCallback? onDiscardSettings;

  /// **三处拦截**的统一问询：返回 `true` = 可以离开（用户选择放弃）。
  final Future<bool> Function()? confirmDiscard;

  // ── 舞台上的常驻浮标 ──
  //
  // 由宿主构造并注入：外壳不知道浮标是什么，只知道「舞台角落有一个」。
  // 这样外壳的布局测试不需要造一整套舞台控件。

  /// 右下角（舞台角标三键）。
  final Widget? stageCorner;

  /// 全局快捷键（规格 §9.3）。外壳只负责**挂上去**，动作由宿主给。
  final AppShortcutCallbacks shortcuts;

  /// 节流后的播报文本（转给流式气泡的 `liveRegion`）。
  final String? announcement;

  /// 当前模型名（舞台的语义标签用）。
  final String? modelName;

  // ── 渲染面状态（转给 `StageHost` 的覆盖层）──
  //
  // 这些**必须真的传进 `StageHost`**：第一版里 `StageHost` 写好了却没人用，
  // 于是「模型加载中 / 加载失败 + 重试」的覆盖层与**舞台语义标签**都不在成品里。
  // 是靠「构建产物里搜不到『Live2D 舞台』这句话」才发现的（见 v0.4.13）。

  /// 渲染面阶段。
  final Live2DBridgePhase stagePhase;

  /// 加载进度 0..1。
  final double? stageProgress;

  /// 渲染面错误文案。
  final String? stageError;

  /// 「重试」（重建 iframe）。
  final VoidCallback? onRetryStage;

  @override
  State<AppShell> createState() => AppShellState();
}

class AppShellState extends State<AppShell> {
  /// 稳定的舞台 key（reparent 时保留状态）。
  final GlobalKey stageKey = GlobalKey(debugLabel: 'app-stage');

  /// 内联侧板是否展开（expanded 专用；浮层宿主由路由自己管生命周期）。
  bool settingsOpen = false;

  /// **内部**分区状态。
  ///
  /// 为什么需要它，而不是直接读 `widget.section`：
  /// `showModalBottomSheet` 的 `builder` **只在路由入栈时构建一次**，
  /// 之后外壳 `setState` 不会重建浮层内容。于是「在浮层里点另一个分区」
  /// 或「刚点 rail 就打开浮层」都会显示**旧的**分区——用户看到点了没反应。
  /// 用一个 `ValueNotifier` 作为导航的单点真相，宿主与浮层都订阅它。
  ///
  /// 对外的受控接口（`widget.section` + `onSectionChanged`）保持不变：
  /// 外部改了 `widget.section` 时由 `didUpdateWidget` 同步进来。
  late final ValueNotifier<SettingsSection> sectionNotifier =
      ValueNotifier<SettingsSection>(widget.section);

  @override
  void initState() {
    super.initState();
    // 键盘解锁：全局处理器**不吞键**（返回 false），所以不会干扰 TextField
    // 或任何快捷键。用全局处理器而不是 `Focus.onKeyEvent` 是因为后者依赖焦点
    // 冒泡——纯键盘用户在焦点还没落到我们节点上时按键不会到达。
    HardwareKeyboard.instance.addHandler(_onKey);
  }

  @override
  void didUpdateWidget(AppShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    // 外部（宿主）改了分区 → 同步进来。内部已一致时不触发通知（避免空重建）。
    if (widget.section != sectionNotifier.value) {
      sectionNotifier.value = widget.section;
    }
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_onKey);
    sectionNotifier.dispose();
    super.dispose();
  }

  bool _onKey(KeyEvent event) {
    if (event is KeyDownEvent) widget.onUserGesture?.call();
    return false; // 不吞：事件继续正常派发。
  }

  void closeSettings() {
    if (!settingsOpen) return;
    setState(() => settingsOpen = false);
  }

  void _select(SettingsSection section) {
    sectionNotifier.value = section;
    widget.onSectionChanged?.call(section);
  }

  /// rail / 齿轮 / 抽屉 → 打开设置。
  ///
  /// 三种断点的分流就在这里（也是唯一的入口）：
  /// - expanded：内联侧板（纯 `setState`，不开路由）；
  /// - medium：底部浮层；
  /// - compact：抽屉（第 1 层）→ 底部浮层（第 2 层）。
  ///
  /// 再点一次 rail 上**同一个**分区 = 收起内联侧板（切换语义）。
  Future<void> openSettings({SizeClass? sizeClass, bool toggle = false}) async {
    final SizeClass sc =
        sizeClass ?? Breakpoints.sizeClassOf(MediaQuery.sizeOf(context).width);

    if (settingsHostOf(sc) == SettingsHost.inline) {
      setState(() => settingsOpen = toggle ? !settingsOpen : true);
      return;
    }

    if (sc.isCompact) {
      final SettingsSection? picked = await showSectionDrawer(
        context: context,
        devMode: widget.devMode,
        selected: sectionNotifier.value,
      );
      if (!mounted) return;
      if (picked != null) _selectGuarded(picked);
    }
    if (!mounted) return;

    await showSettingsSheet(
      context: context,
      sizeClass: sc,
      // 用**浮层自己的** context 关掉自己：用外壳的 context 去 `maybePop`
      // 依赖「浮层恰好是栈顶」这个巧合，不稳健。
      builder: (BuildContext sheetContext) => _settingsSurface(
        onClose: () => Navigator.of(sheetContext).pop(),
      ),
    );
  }

  /// 设置内容（三种宿主共用**同一个** widget），订阅内部分区状态。
  Widget _settingsSurface({required VoidCallback onClose}) =>
      ValueListenableBuilder<SettingsSection>(
        valueListenable: sectionNotifier,
        builder: (BuildContext context, SettingsSection section, Widget? _) =>
            SettingsScaffold(
              sections: widget.sections,
              selected: section,
              // 换分区也要过拦截（「离开分区」是规格 §4.3 的三处拦截之一）。
              onSelect: _selectGuarded,
              onClose: onClose,
              dirty: widget.settingsDirty,
              saving: widget.settingsSaving,
              onSave: widget.onSaveSettings,
              onDiscard: widget.onDiscardSettings,
              statusMessage: widget.settingsStatus,
              statusIsError: widget.settingsStatusIsError,
              child: widget.sectionBuilder(context, section),
            ),
      );

  /// 换分区前先过草稿拦截。
  void _selectGuarded(SettingsSection next) {
    final Future<bool> Function()? ask = widget.confirmDiscard;
    if (ask == null || next == sectionNotifier.value) {
      _select(next);
      return;
    }
    unawaited(() async {
      if (await ask()) _select(next);
    }());
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      // 指针手势解锁（既有行为，保留）。
      behavior: HitTestBehavior.translucent,
      onPointerDown: (_) => widget.onUserGesture?.call(),
      child: LayoutBuilder(
        builder: (BuildContext context, BoxConstraints constraints) {
          final SizeClass sizeClass = Breakpoints.sizeClassOf(
            constraints.maxWidth,
          );
          final bool compact = sizeClass.isCompact;
          final bool inlineSettings = settingsHostOf(sizeClass) == SettingsHost.inline;

          // **必须走 `StageHost`**：加载/错误覆盖层与舞台语义都在它里面。
          // 直接铺 `RepaintBoundary(child: widget.stage)` 会让那些东西全部失效。
          final Widget stage = StageHost(
            key: stageKey,
            stage: widget.stage,
            phase: widget.stagePhase,
            progress: widget.stageProgress,
            errorMessage: widget.stageError,
            onRetry: widget.onRetryStage,
            modelName: widget.modelName,
          );

          // ── 关键：三种断点共用同一个 Flex，只换 direction 与子项 flex。
          //    子树形状不变 ⇒ 舞台 Element 不被反激活 ⇒ iframe 不重建。 ──
          final Widget body = Flex(
            direction: compact ? Axis.vertical : Axis.horizontal,
            children: <Widget>[
              Flexible(
                flex: compact ? 3 : 1,
                child: Stack(
                  children: <Widget>[
                    Positioned.fill(child: stage),
                    if (widget.wsStatus != WsStatus.connected)
                      Positioned(
                        top: 0,
                        left: 0,
                        right: 0,
                        child: OfflineBanner(
                          status: widget.wsStatus,
                          onRetry: widget.onRetryConnection,
                        ),
                      ),
                    if (settingsOpen && inlineSettings)
                      Positioned.fill(
                        child: InlineSettingsDock(
                          onDismiss: closeSettings,
                          child: _settingsSurface(onClose: closeSettings),
                        ),
                      ),
                    // 舞台浮标：**压在舞台之上**，不占布局、不挤舞台宽度。
                    if (widget.stageCorner != null)
                      Positioned(
                        right: 0,
                        bottom: 0,
                        child: widget.stageCorner!,
                      ),
                  ],
                ),
              ),
              if (compact)
                Divider(height: 1, color: appColorsOf(context).hairline)
              else
                VerticalDivider(width: 1, color: appColorsOf(context).hairline),
              // `flex: 0` = 不参与弹性分配，用自身尺寸（`SizedBox(width: …)`）。
              // compact 下反过来：聊天按 3:2 分高度。
              Flexible(
                flex: compact ? 2 : 0,
                child: SizedBox(
                  width: compact
                      ? null
                      : (sizeClass.isExpanded
                            ? NavMetrics.chatWidthExpanded
                            : NavMetrics.chatWidthMedium),
                  child: _chat(compact: compact),
                ),
              ),
            ],
          );

          final Widget scaffold = Scaffold(
            appBar: AppBar(
              title: const Text('Live2D Ai'),
              actions: <Widget>[
                StatePill(
                  phase: widget.phase,
                  compact: true,
                  onTap: widget.error == null ? null : widget.onRetryLast,
                ),
                const SizedBox(width: Space.s2),
                ConnectionBadge(
                  status: widget.wsStatus,
                  onRetry: widget.onRetryConnection,
                ),
                if (!compact)
                  IconButton(
                    tooltip: '设置',
                    onPressed: () => openSettings(sizeClass: sizeClass),
                    icon: const Icon(Icons.settings_outlined),
                  ),
                const SizedBox(width: Space.s2),
              ],
            ),
            body: compact ? body : _withRail(sizeClass, body),
          );

          // 快捷键挂在**根**（`CallbackShortcuts` 最省样板，且不吞未匹配的键）。
          // 外面再包一个 `FocusTraversalGroup`：整页的 Tab 顺序是确定的，
          // 不会因为某次重构而静默改变。
          return CallbackShortcuts(
            bindings: widget.shortcuts.bindings(),
            child: FocusTraversalGroup(
              policy: OrderedTraversalPolicy(),
              child: scaffold,
            ),
          );
        },
      ),
    );
  }

  /// medium / expanded 的左侧分区 rail。
  Widget _withRail(SizeClass sizeClass, Widget body) {
    final bool extended = sizeClass.isExpanded;
    return Row(
      children: <Widget>[
        ValueListenableBuilder<SettingsSection>(
          valueListenable: sectionNotifier,
          builder: (BuildContext context, SettingsSection current, Widget? _) =>
              NavigationRail(
                extended: extended,
                minWidth: extended
                    ? NavMetrics.railExtendedWidth
                    : NavMetrics.railIconWidth,
                selectedIndex: current.index,
                labelType: extended
                    ? NavigationRailLabelType.none
                    : NavigationRailLabelType.all,
                onDestinationSelected: (int index) {
                  final SettingsSection picked = SettingsSection.byIndex(index);
                  // 已选中同一个分区且侧板开着 → 收起（切换语义）。
                  final bool toggle = picked == current;
                  if (picked != current) _selectGuarded(picked);
                  openSettings(sizeClass: sizeClass, toggle: toggle);
                },
                destinations: <NavigationRailDestination>[
                  for (final SettingsSection section in widget.sections)
                    NavigationRailDestination(
                      icon: Icon(section.icon),
                      label: Text(section.label),
                    ),
                ],
              ),
        ),
        VerticalDivider(width: 1, color: appColorsOf(context).hairline),
        Expanded(child: body),
      ],
    );
  }

  Widget _chat({required bool compact}) => ChatPanel(
    messages: widget.messages,
    phase: widget.phase,
    input: widget.input,
    onSend: widget.onSend,
    onStop: widget.onStop,
    onDismissError: widget.onDismissError ?? () {},
    error: widget.error,
    errorActions: widget.errorActions,
    volume: widget.volume,
    muted: widget.muted,
    onVolumeChanged: widget.onVolumeChanged,
    onMutedChanged: widget.onMutedChanged,
    serverMuted: widget.serverMuted,
    audioUnlocked: widget.audioUnlocked,
    onEnableSound: widget.onEnableSound,
    onOpenSettings: compact ? () => openSettings() : null,
    onRetryLast: widget.onRetryLast,
    announcement: widget.announcement,
  );
}
