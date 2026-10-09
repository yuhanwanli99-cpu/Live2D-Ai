/// 应用外壳（L3）：按 `SizeClass` 选导航形态，**舞台恒常驻**。
///
/// # 保活是这一层唯一的硬约束
///
/// 舞台是一个 `<iframe>` 平台视图。它一旦离开 Widget 树，iframe 就被销毁，
/// 模型重载、口型时间轴清零。所以：
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
/// # 压在舞台上的控件必须垫指针垫层（2026-09-11 修）
///
/// 舞台是 `<iframe>` 平台视图：它在 DOM 里排在 Flutter 画布**之上**，而画布是
/// `pointer-events: none`。指针落在舞台区域时会进 **iframe 自己的文档**，父页
/// （Flutter）一个都收不到——于是所有压在舞台上的 Flutter 控件都是
/// **「看得见、点不着、也滑不动」**：设置面板整块（medium 下几乎全在舞台上方）、
/// 断线横幅的「点此重试」、舞台右下角的缩放四键、错误覆盖层的「重试」。
///
/// 修法：给它们套一层 [StagePointerInterceptor]（原理见其头注）。
/// **以后新增任何压在舞台上的可交互控件，都要照做。**
///
/// # 左侧不再有分区 rail（2026-09-11 用户裁决）
///
/// 用户：「把左边的这些设置一级选项去掉留给舞台」。于是 `SectionRail` 整个删除，
/// 那一列 168 px 全部还给舞台。导航不再有第二个入口：
///
/// - 入口只有**一处**：非 compact 是 AppBar 的「设置」按钮，compact 是聊天面板头；
/// - 分区切换在**面板内部**（`SettingsScaffold` 的 chip 行）——它本来就是导航的真身，
///   rail 只是它的副本（同一个 `sectionNotifier` 驱动的第二处渲染）。
///
/// 顺带删掉的行为：以前「再点一次 rail 上同一个分区」会收起侧板。现在收起只有
/// **✕ / Esc**（`showModalBottomSheet` 另有自带的关闭手势）。
///
/// # 诚实记录的偏离：内联侧板不实现「点空白关闭」
///
/// 规格 §4.2 写的是「可 Esc / 点空白关闭」。垫层只罩得住**它包起来的那个盒子**，
/// 而「点空白关闭」要求整块舞台都接得住点击——那得在设置打开期间把整个舞台铺一层
/// 遮挡，代价是「设置开着时还能拖模型」（渲染面 `sync.clickEnabled` 的真实交互）
/// 一起没了。两害相权，先不铺。因此收起路径是**✕ / Esc**；
/// `showModalBottomSheet` 自带的模态障碍层同理，在舞台区域不生效。
///
/// # 行数拆分（2026-10-06，R4-T2）
///
/// 原本单文件 **998 行**（>500，豁免上限 1000；拆分登记在 Stage C3）。
/// 现在按**顶层类边界**拆成库（367 行）+ `app_shell_state.dart`（635 行，
/// `AppShellState` 与 `NeverNotifies`）——`part` 继承库的 import，零可见性
/// 改动。**类不能跨 part**，所以切口只能落在 `class AppShellState` 那一行。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';


import '../api/ws_status.dart';
import '../chat/chat_message.dart';
import '../design/background_item.dart';
import '../design/breakpoints.dart';
import '../design/tokens.dart';
import '../settings/sections/appearance_section.dart';
import '../settings/settings_sections.dart';
import '../live2d/live2d_bridge.dart';
import '../live2d/stage_pointer_interceptor.dart';
import '../state/ui_phase.dart';
import '../ui/chat_panel.dart';
import '../ui/connection_badge.dart';
import '../chat/chat_session.dart';
import '../ui/error_banner.dart';
import '../ui/inline_notice.dart';
import '../ui/session_sheet.dart';
import '../ui/settings_scaffold.dart';
import '../settings/display_prefs.dart';
import '../ui/background_logic.dart';
import '../ui/shell_backdrop.dart';
import '../ui/soft_motion.dart';
import '../ui/stage_host.dart';
import '../ui/state_pill.dart';
import '../ui/theme.dart';
import 'app_shortcuts.dart';
import 'collapsible_panel.dart';
import 'nav_host.dart';
import 'page_cross_fade.dart';

part 'app_shell_state.dart';

class AppShell extends StatefulWidget {
  const AppShell({
    required this.stage,
    required this.phase,
    required this.wsStatus,
    required this.messages,
    this.playingSentenceSeq,
    required this.input,
    required this.onSend,
    required this.onStop,
    required this.onRetryConnection,
    required this.volume,
    required this.muted,
    required this.prefs,
    this.backgroundIndex = 0,
    this.backgroundHydrating = false,
    required this.onVolumeChanged,
    required this.onMutedChanged,
    required this.sections,
    required this.sectionBuilder,
    this.onEnsureSectionLoaded,
    this.settingsChanges = const NeverNotifies(),
    this.settingsRevision = 0,
    this.section = SettingsSection.theme,
    this.onSectionChanged,
    this.modRestartNotice,
    this.onDismissModRestart,
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
    this.sessions = const <ChatSession>[],
    this.activeSessionId,
    this.sessionsListenable,
    this.readSessions,
    this.readActiveSessionId,
    this.onNewSession,
    this.onSelectSession,
    this.onRenameSession,
    this.onDeleteSession,
    this.shortcuts = const AppShortcutCallbacks(isMacOS: false),
    this.announcement,
    this.modelName,
    this.stagePhase = Live2DBridgePhase.loading,
    this.stageProgress,
    this.stageError,
    this.onRetryStage,
    this.listenSupported = false,
    this.listening = false,
    this.listenStatus,
    this.listenError,
    this.onToggleListen,
    this.listenNote,
    super.key,
  });

  /// 舞台本体。外壳**不**构造它、**不**替换它——只负责放在哪。
  final Widget stage;

  final UiPhase phase;
  final WsStatus wsStatus;

  final List<ChatMessage> messages;

  /// **当前正在播放的句号**（`sentence_seq`）；`null` = 没有朗读高亮。
  ///
  /// 外壳只做转手：真源是 `ChatController.playingSentenceSeq`（它把
  /// `AudioPlayer` 的两条流原样转进 `SpokenHighlightController`）。
  final int? playingSentenceSeq;

  final TextEditingController input;
  final VoidCallback onSend;
  final VoidCallback onStop;
  final VoidCallback onRetryConnection;

  final double volume;
  final bool muted;

  /// 壳背景库（`DisplayPrefs.backgrounds`）与它的渲染参数。
  ///
  /// 2026-09-27：由「单张图 + 固定 0.15」升级成「有序背景库 + 轮播 + 铺法 +
  /// 位置 + 模糊 + 遮罩」。这一层只**铺**，不持有任何状态：索引与定时器住在
  /// 宿主（`main.dart` 的 `_slideshow` / `_backgroundIndex`）。
  final DisplayPrefs prefs;

  /// 轮播到了第几项（`backgrounds` 的下标；库里为空时恒为 0）。
  ///
  /// **运行时状态，不落盘**：外壳把它连同「此刻画的那一项」一起注入
  /// BackgroundRuntimeScope，设置面板据此标「当前」并按它决定铺法 / 位置区
  /// 的显隐（DEC-6）。
  ///
  /// 2026-09-28（F-0001-5）：这里曾经挂着两个**全仓零调用**的背景索引回调
  /// 参数，以及一条指向并不存在的宿主类的头注——它们让下一位改背景域的人
  /// 以为存在第四条索引通路。两个参数与那条注释都已删除。
  final int backgroundIndex;

  /// 背景库字节是否还在水合（交接项 9b）。
  ///
  /// 宿主在 `initState` 之后读一次 IndexedDB；读完（或 3 s 超时）置 `false`。
  /// 设置面板据此显示「正在读回背景库…」并禁用背景库的写操作——**不做**
  /// 一个按了没反应、或与读回打架的按钮。
  final bool backgroundHydrating;

  final ValueChanged<double> onVolumeChanged;
  final ValueChanged<bool> onMutedChanged;
  final bool serverMuted;
  final bool audioUnlocked;
  final VoidCallback? onEnableSound;

  /// 任意用户手势（指针按下 / 任意按键）——用于解锁 WebAudio。
  final VoidCallback? onUserGesture;

  // ── 常态语音检测（唤醒词缺省「小可爱」） ──
  //
  // 外壳只**呈现**：能力布尔 + 一行状态/错误 + 一个切换回调。
  // 识别与注入的真身在 `VoiceListenController`（宿主持有）。
  final bool listenSupported;
  final bool listening;
  final String? listenStatus;
  final String? listenError;

  /// 点「听」：开始 / 结束一次听写（定稿进输入框；2026-10-08）。
  final VoidCallback? onToggleListen;

  /// 一行诚实说明（Web Speech 需联网、音频出本机）。
  final String? listenNote;

  /// L1 基座：Mod 变更后的统一「需重新点火 / 重启后生效」提示。
  ///
  /// 挂在聊天区顶部（常驻、可关）——用户改完 Mod 回聊天时，第一眼就该看到
  /// 「这件事还没完全生效」，而不是盯着一个没变化的舞台猜。
  final String? modRestartNotice;
  final VoidCallback? onDismissModRestart;

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

  /// 设置数据变化时通知外壳重建设置面板内容。
  ///
  /// # 为什么必须要（2026-09-11 浏览器里抓到的第二个 bug）
  ///
  /// `showModalBottomSheet` 的 `builder` **只在路由入栈时跑一次**——medium /
  /// compact 的浮层内容因此被**冻结在打开那一刻**。设置数据是异步加载的：
  /// 打开时还在 `loading`，浮层画出一个转圈；数据回来之后外层 `setState`
  /// 重建的是外壳、**不是浮层**，于是那个转圈**永远转下去**。
  ///
  /// expanded 的内联侧板没有这个问题（它就是外壳自己的一棵树，`setState`
  /// 直接重建它）——所以这个 bug 只在 medium / compact 上出现，
  /// 而 widget 测试全都注入桩内容、从不经过加载，于是谁也没发现。
  ///
  /// 传进来的应该是 `SettingsController`（它本来就是 `ChangeNotifier`）。
  final Listenable settingsChanges;

  /// 设置面板内容所依赖的**宿主状态代际**（F-0005-2，审计 45 条 · rc.7 A 组）。
  ///
  /// 宿主每变一次「外壳看不见、只有 [sectionBuilder] 闭包读得到」的状态
  /// （模型库 / Mod / 诊断 / 自检结果 / 本模型覆盖 …）就 +1。
  ///
  /// 为什么需要：`AppShell.build()` 会被聊天增量（`text_delta` 毫秒级）高频
  /// 带动重建，而设置面板是常驻树里的子树（折叠**不卸载**）——过去每个 delta
  /// 都会把当前分区整棵重建 + 布局一次，**即使面板关着**。现在分区内容按代际
  /// 放行（[AppShellState._pane]），所以这个值**绝不能**跟着聊天增量前进。
  final int settingsRevision;

  /// **打开设置前**让宿主把当前分区的内容加载出来。
  ///
  /// 外壳不持有设置数据（那是 `SettingsController` 的事），所以它只能上报
  /// 「用户要看设置了」这个意图。少了这一步，直接点「设置」会看到一个
  /// 「读不到服务端设置」的空壳——见 [AppShellState.openSettings] 的说明。
  final VoidCallback? onEnsureSectionLoaded;

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

  /// 会话列表（已按最近使用排序）。空 = 还没建过会话。
  ///
  /// ⚠️ 这是**打开会话浮层那一刻**的快照。浮层刷新走
  /// [sessionsListenable] + [readSessions]（见 `ui/session_sheet.dart`）——
  /// 删一行之后浮层里的那份副本不会自己更新。
  final List<ChatSession> sessions;

  /// 当前会话 id（`null` = 没有）。同上：打开那一刻的快照。
  final String? activeSessionId;

  /// 会话存储的变更通知（`ChatController`）。
  ///
  /// 浮层在**自己的 builder 里**订阅它：`showModalBottomSheet` 的 builder 只跑
  /// 一次，不订阅就会出现「删掉一行、行还在」（2026-10-08 修）。
  final Listenable? sessionsListenable;

  /// 读**此刻**的会话列表 / 当前 id（浮层每次 build 都重新读）。
  final List<ChatSession> Function()? readSessions;
  final String? Function()? readActiveSessionId;

  final VoidCallback? onNewSession;
  final ValueChanged<String>? onSelectSession;
  final void Function(String id, String title)? onRenameSession;
  final ValueChanged<String>? onDeleteSession;

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

