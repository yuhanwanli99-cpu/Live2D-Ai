/// 应用入口：**只做装配**（组合根）。
///
/// 本文件只留四件事：**构造/注入**、**dispose**、**顶层入口**，以及必须留在
/// `State` 类体里的字段与少量被源码扫描钉住的成员（`_gotoSection` /
/// `_clearTransientResults` / `_testLlm` / `_testTts`，见
/// `test/transient_results_test.dart`）。
///
/// # 行为接线按边界拆到 `part` 文件（2026-09-13，rc.3 N1）
///
/// | 边界 | 去处 |
/// |---|---|
/// | 管理面（模型库 / Mod / 诊断 / 密钥） | `app/shell_admin.dart` |
/// | 设置草稿与 8 个分区 pane | `app/shell_settings.dart` |
/// | 显示偏好 / 背景图 / 舞台缩放 | `app/shell_prefs.dart` |
/// | 对话回合与读屏播报 | `app/shell_chat.dart` |
///
/// 浏览器 I/O 与两段纯显示逻辑已移出为**独立库**：`app/browser_io.dart`
/// （localStorage + 文件选择，唯一需要 `package:web` 的地方）、
/// `app/shortcut_help_dialog.dart`、`ui/error_actions.dart`。
///
/// # 为什么是 `part` + 扩展方法
///
/// 这些成员读写 `_ShellRootState` 的私有字段。Dart 的私有是**库级**的，搬到
/// 别的库就看不见它们；而 `setState` 又是 `@protected`，扩展方法不能直接调。
/// 因此本轮的拆法是**纯搬移**：`part` 文件里的扩展方法照旧读写同一批字段，
/// 只把 `setState(() { … })` 换成「先改字段、再 `_refresh()`」——语义与原来
/// 逐字等价，调用点一行都不用改（重建时机不变：同为改字段后在当前帧置脏）。
/// 要把它进一步做成 `AdminWiring` 之类的独立协作者（可单测），必须先决定
/// 重建所有权，那属于行为变更，不在「结构性减法」这一轮。
///
/// 其余可测逻辑仍在既有分层里：
///
/// | 逻辑 | 去处 | 测试 |
/// |---|---|---|
/// | 帧解析 | `api/ws_frame.dart` | `ws_frame_test.dart` |
/// | 界面相位派生 | `state/ui_phase.dart` + `state/ui_state_tracker.dart` | 两个 `ui_*_test.dart` |
/// | 主题 / 令牌 / 断点 | `ui/theme.dart`、`design/*` | `theme_test.dart` 等 |
/// | 外壳与布局 | `app/app_shell.dart` | `app_shell_layout_test.dart`、`stage_keepalive_test.dart` |
///
/// （2026-09-11：本表原先还有一行「动作来源 / 历史 → `actions/*`」。用户裁定
/// LLM 无工具、只做对话，动作子系统整条移除，该行的文件与测试一并删除。）

/// # 行数（**超出豁免带**，拆分归 Stage C3）
///
/// 本文件在 rc.7（2026-09-28 · R7-a，F-0005-2 的宿主接线）之后 **1344 行**
/// （`wc -l`），**超过「源码 ≤500 行、豁免 ≤1000 行」那条线**。这不是本轮才超的：
/// rc.6 之后已是 1329 行（`settings/display_prefs.dart` 头注记了同一笔账，那批拆分
/// 登记在 Stage C3），本轮只加不减（+15：设置面板内容的宿主状态代际）。写在这里是
/// **如实**，不是豁免申请。
///
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import 'api/api_client.dart';
import 'api/diagnostics_api.dart';
import 'api/env_api.dart';
import 'api/models_api.dart';
import 'api/mods_api.dart';
import 'api/settings_models.dart';
import 'api/voice_api.dart';
import 'api/ws_client.dart';
import 'app/app_shell.dart';
import 'app/app_shortcuts.dart';
import 'app/browser_io.dart';
import 'app/shortcut_help_dialog.dart';
import 'audio/audio_player.dart';
import 'audio/stage_clock.dart';
import 'chat/chat_controller.dart';
import 'data/background_hydration.dart';
import 'data/background_reorder.dart';
import 'data/background_store.dart';
import 'design/background_item.dart';
import 'design/tokens.dart';
import 'live2d/action_scales_sync.dart';
import 'live2d/live2d_bridge.dart';
import 'live2d/live2d_stage.dart';
import 'live2d/render_events.dart';
import 'live2d/session_baseline.dart';
import 'live2d/stage_cancel.dart';
import 'settings/display_prefs.dart';
import 'settings/mods/mod_panel.dart';
import 'settings/mods/persona_card_picker.dart';
import 'settings/preset_labels.dart';
import 'settings/sections/appearance_section.dart';
import 'settings/sections/dev_tools_section.dart';
import 'settings/sections/director_observer_section.dart';
import 'settings/sections/motion_section.dart';
import 'settings/sections/persona_section.dart';
import 'settings/sections/service_section.dart';
import 'settings/settings_controller.dart';
import 'settings/settings_sections.dart';
import 'state/live_region.dart';
import 'state/ui_state_tracker.dart';
import 'voice/speech_recognizer.dart';
import 'voice/voice_listen_controller.dart';
import 'ui/confirm_discard_dialog.dart';
import 'ui/error_actions.dart';
import 'ui/field_row.dart';
import 'ui/restart_notice.dart';
import 'ui/shell_slideshow.dart';
import 'ui/stage_corner_controls.dart';
import 'ui/theme.dart';

part 'app/shell_admin.dart';
part 'app/shell_chat.dart';
part 'app/shell_prefs.dart';
part 'app/shell_settings.dart';
part 'app/shell_cue_voice_wiring.dart';
part 'app/shell_background_scale_wiring.dart';
part 'app/shell_section_wiring.dart';
part 'app/shell_app_root.dart';
part 'app/shell_lifecycle_wiring.dart';

/// Mod 列表里某个 id 的启停；**不在册**（极简装配 / 自定义注册表）→ `true`。
///
/// 与 web_api 端点的口径一致：端点是核心 say 能力，不该因为一个可选 Mod
/// 缺失而失效（见 `voice_routes.rs` 头注「启停门禁」第三点）。所以这里
/// 「查不到」绝不能当成「未启用」——那会误报红字。
bool _enabledOf(List<ModInfo> mods, String id) {
  for (final ModInfo m in mods) {
    if (m.id == id) return m.enabled;
  }
  return true;
}

void main() {
  runApp(const Live2DShellApp());
}

/// 应用根：Material 3 主题 + **本地显示偏好的持有者**。
///
/// # 为什么偏好住在根、而不是住在 `ShellRoot`
///
/// 主题（黑/白/蓝/灰）必须作用在 `MaterialApp` 上——而 `MaterialApp` 是
/// `ShellRoot` 的**父级**。偏好留在子级就只能靠 `InheritedWidget` 往上捅，
/// 或者让 `MaterialApp` 自己再读一遍 localStorage（两份真相）。
/// 住在这里之后：**一份偏好、一次持久化、主题与组件同源**。

class _ShellRootState extends State<ShellRoot> {
  /// setState 是 @protected —— Dart part 里的 extension 不能直接调它。
  ///
  /// 2026-10-06（E1 拆 main.dart）实测到的唯一硬约束：本库的接线方法大量
  /// 搬进了 part（shell_cue_voice_wiring / shell_background_scale_wiring /
  /// shell_section_wiring），它们都要触发重建。
  ///
  /// part 里一律用这一个桥（_rebuild(...)），不要另开第二条 ——
  /// 同一个动作有两条路，就是下一次漂移的种子。
  void _rebuild(VoidCallback fn) => setState(fn);
  final GlobalKey<Live2DStageState> _stageKey = GlobalKey<Live2DStageState>();
  final GlobalKey<AppShellState> _shellKey = GlobalKey<AppShellState>();
  final TextEditingController _input = TextEditingController();

  late final ApiClient _api;
  late final WsClient _ws;
  late final AudioPlayer _audio;
  late final ChatController _chat;

  /// 界面相位派生（WS 信号 + 渲染面状态 → `UiPhase`）。**纯逻辑，可单测。**
  final UiStateTracker _ui = UiStateTracker();

  StreamSubscription<double>? _levelSubscription;
  StreamSubscription<WsStatus>? _statusSubscription;
  StreamSubscription<WsEvent>? _eventSubscription;
  StreamSubscription<int?>? _sentenceCueSubscription;
  StreamSubscription<StageClockSample>? _stageClockSubscription;

  /// 渲染面事件 → **喂导演的日志文本**（协议 §7.3）。
  ///
  /// 四条 ack + `segment-ended` 由 `Live2DStage.onRenderEvent` 汇进来；前端
  /// **不据此维护镜像状态**（唯一例外是调试面板的显示快照，同样只由 ack 驱动）。
  final DirectorEventLog _directorLog = DirectorEventLog(
    onEvent: DirectorObserverFeed.instance.pushRenderEvent,
  );

  /// 当前 action_cue 的轮次代号（v1 可选键透传给渲染面用，缺省 null）。
  int? _directorEpoch;

  /// 停止 / 新消息的取消纪律（V7 §6.6 / V10 §9.3）——四步顺序的唯一出口。
  late final StageCancellation _stageCancellation;

  /// 导演按句 cue（P1-3，2026-09-16）：**唯一**的舞台驱动通道（阶段3 D10–D13）。
  ///
  /// 新 plan 到达即**整体替换**（一份 plan 覆盖上一份）；音频开始播放时按
  /// 当前句的 sentence_seq 取用一次并移除。产品设置里的幅度倍率由渲染面乘，
  /// 这里只透传 cue 的 intensity。`state_json.latest` 仅供 director 面板
  /// **只读展示**，不再驱动舞台（通道 B 已于阶段3 退役）。
  final DirectorCuePlan _directorCues = DirectorCuePlan();

  /// 当前设置分区（受控；外壳只上报意图）。
  SettingsSection _section = SettingsSection.theme;

  /// 进设置分区时要**定位到的页内组**（2026-10-09）。
  ///
  /// 目前只有「去语音合成设置」用它：目标是「模型服务」页的语音组，
  /// 靠 kServiceVoiceGroup 值 + 该组的 GlobalKey（不靠滚动碰运气）。
  /// null = 只换分区、不定位。
  String? _settingsGroupFocus;

  /// 设置面板内容的**宿主状态代际**（F-0005-2，审计 45 条 · rc.7 A 组）。
  ///
  /// 只在 `_ShellRootState` **自己重建**时前进（见 [build]）——也就是宿主状态
  /// 真的变了的时候。**聊天增量不会让它前进**：`text_delta` 只重建下面那个
  /// `ListenableBuilder` 的子树，不会回头调用 `ShellRoot.build()`。
  ///
  /// 设置面板是常驻树里的子树（折叠不卸载），所以这个代际就是「要不要重建设置
  /// 分区内容」的唯一判据——它不前进，外壳那些高频重建就不会带上那棵子树。
  int _settingsRevision = 0;

  /// 服务端静音观测值（WS `audio.muted`，**只读**）。
  bool _serverMuted = false;

  // ── P4：设置草稿 + 四个管理面 ──
  late final SettingsController _settings;
  late final ModelsApi _modelsApi;
  late final EnvApi _envApi;
  late final ModsApi _modsApi;
  late final DiagnosticsApi _diagApi;

  /// 语音转写客户端 + **常态语音检测**控制器（唤醒词缺省「小可爱」）。
  ///
  /// 见 `voice/voice_listen_controller.dart`：识别平台能力由
  /// `createSpeechRecognizer()` 条件导入（VM/不支持 → null，按钮禁用并说明）。
  late final VoiceApi _voiceApi;
  late final VoiceListenController _voiceListen;

  List<ModelInfo> _models = const <ModelInfo>[];

  /// 密钥真源状态（`GET /api/v1/env`）：键名 + 是否已设置（**没有值**）。
  EnvStatus _envStatus = const EnvStatus();

  /// 当前要渲染面加载的模型 URL（来自 activate 响应的 `model_url`）。
  ///
  /// 为什么要存它：activate 只改后端 registry，**换皮发生在 iframe 里**
  /// （`sync.payload.model`）。不把它存下来，舞台在 retry / 重建 iframe 之后
  /// 会退回默认模型——「激活了但没换皮」的另一种形态。
  String? _activeModelUrl;
  List<ModInfo> _mods = const <ModInfo>[];

  /// voice-input Mod 是否启用（`null` = 还没读到，**不误报**红字）。
  ///
  /// 判据真源 = `GET /api/v1/mods` 的 `enabled`；读失败保持 `null`，
  /// 端点自己的 403 `mod_disabled` 仍是兜底。
  bool? _voiceInputEnabled;
  List<LogLine> _logs = const <LogLine>[];
  String? _logsError;
  Map<String, Object?> _status = const <String, Object?>{};
  AppCapabilities _capabilities = const AppCapabilities();
  bool _adminLoading = false;
  String? _adminError;
  String? _busyId;

  /// 「模型库 / Mod」管理操作的结果（激活、启停）。**与动作子系统无关**——
  /// 2026-09-11 由 `_adminMessage` 改名而来：那是个误导性的旧名字，
  /// 让人以为它属于已被删除的动作链路（实际服务的是 `_activateModel`
  /// 与 `_toggleMod`）。行为不变。
  String? _adminMessage;

  /// **统一的 Mod 变更 → 重新点火/重启提示**（L1 基座，2026-09-15）。
  ///
  /// 由**两类**动作写入：Mod 启停（`_toggleMod`）与各 Mod 产品面板自己的动作
  /// （导入角色卡 / 导入或清空记忆 / 改语音闸）。
  ///
  /// **配置保存（`_saveModConfig`）不在其中**（2026-10-09「两类 TTS」）：那条路径
  /// 的按钮是「保存并应用」，服务端已经 restart 过该 Mod，再挂一条「需重新点火」
  /// 的常驻提示等于让用户去做一件刚刚已经做完的事。
  /// 文案与处置入口的唯一来源是 `ui/restart_notice.dart`——不要在调用点各写一份。
  String? _modRestartNotice;

  /// 连通性自检的结果：**服务端 `ok` + 文案**（`FieldTestResult`）。
  ///
  /// 2026-10-01（审计 F-0012-1）：这里过去只存一行 `String`，成败由
  /// `llm_section` / `tts_section` 从那行文案里猜（`contains('ok'|'ms'|'毫秒')`）。
  /// 现在 `o.ok` 与文案一起落进结构体——**成败只有一个真源**，而且成功那条
  /// 终于有渲染槽（F-0003-2：过去成功时界面毫无反应）。
  FieldTestResult? _llmTest;
  bool _llmTesting = false;
  FieldTestResult? _ttsTest;
  bool _ttsTesting = false;

  /// 背景库选图 / 清图的提示。
  String? _shellImageMessage;
  bool _shellImageFailed = false;

  /// 「本模型覆盖」直接 PATCH 的结果（阶段5 D40）：与全局草稿那套无关，
  /// 因为覆盖不经设置草稿（见 shell_settings.dart 的接线）。
  String? _modelOverrideMessage;
  bool _modelOverrideFailed = false;

  bool _copied = false;
  bool _settingsLoadedOnce = false;

  /// 未保存改动确认框是否**已经开着**（防重入）。
  ///
  /// Esc 连按 / 「换分区」与「关设置」同时进来时，只弹一个；
  /// 第二个入口直接按「不许离开」处理（最保守）。
  bool _confirmingLeave = false;

  /// 预设 id → 展示名（读 `/actions/preset_labels.json`，**唯一真源**）。
  ///
  /// 取不到就是空表：调试面板回落显示稳定 id（不硬编码第二套中文标签）。
  PresetLabelTable _presetLabels = PresetLabelTable.empty;

  /// 动作幅度即时预览的防抖下发器（2026-09-16）。
  ///
  /// 为什么是 State 的字段而不是扩展里的字段：Dart 的扩展不能声明实例字段。
  /// 行为与回归见 `live2d/action_scales_sync.dart`。
  late final ActionScalesSyncer _actionScalesSyncer = ActionScalesSyncer(
    (Map<String, double> scales) =>
        _stageKey.currentState?.applyActionScales(scales),
  );

  /// 本模型覆盖编辑的**合并防抖**（阶段5 D40 修，2026-09-27）。
  ///
  /// 滑条一次拖动会连发几十次 `onChanged`，而每次 PATCH 都要 toml 原子写 +
  /// GET 回读 + supervisor 热重载；这里把同一模型的连续编辑收成一帧，
  /// 且只发用户真动过的键（未动的键保持「未覆盖」）。
  late final ModelOverrideCoalescer _modelOverrideCoalescer =
      ModelOverrideCoalescer(onFlush: _flushModelOverride);

  /// 本模型覆盖开关的即时意图。`null` = 跟服务端。
  ///
  /// 开关是受控的：不记这一下，拨完会弹回旧状态，直到 PATCH 回读结束。
  bool? _modelOverrideIntent;

  /// 覆盖 PATCH 的**串行**队列。
  ///
  /// 防抖只解决「合并」，不解决乱序：先拖 head、再点「恢复跟随全局」时
  /// 两次 PATCH 仍可能乱序到达服务端（HTTP 不保证完成顺序），结果就
  /// 变成「点了删除，覆盖又回来了」。串行化后服务端看到的顺序 = 用户操作顺序。
  Future<void> _modelOverrideChain = Future<void>.value();

  // ── 动作子系统已于 2026-09-11 移除（用户裁决：LLM 无工具、只做对话）──
  //
  // 这里曾经有 `_performingAction`（当前正在演的动作协议名）与 `_onActionState`
  // （把 WS `action_state` 帧转成渲染面的 `action-state`）。随
  // `live2d_perform_action` 工具、`action_state` 帧与 `/api/v1/commands`
  // 端点一起删除——服务端不再发这一帧，前端也就不再有任何动作通道。

  /// 渲染面回报的实际缩放（`stage-ack`；未收到时为 null，**不猜**）。
  double? _stageScaleFromAck;

  // ── 背景轮播（2026-09-27）──
  //
  // 状态住在**宿主**（这里）而不是 `AppShell`：它是「跨整个壳」的一段时间轴，
  // 而 `AppShell` 会被 WS 事件频繁重建（`ListenableBuilder` 的 builder 里
  // 直接 new 一个），定时器住进去会跟着不停重启。
  final ShellSlideshow _slideshow = ShellSlideshow();

  /// 当前播到背景库第几项（**不落盘**：刷新后从头开始符合直觉）。
  ///
  /// 它是**唯一**的运行时索引真源（F-0001-2）：`_jumpBackground` 与
  /// `_onBackgroundAdvance` 都只改它，并同步推给 `_slideshow`。
  int _backgroundIndex = 0;

  // ── 下发给渲染面的**同值去重**（F-0002-1，2026-09-28）──
  //
  // 为什么记忆住在宿主而不是渲染面客户端：这条漏斗（`_applyPrefs`）是
  // 「任意偏好变更」唯一要过的地方，而音量滑杆过去每帧都过它一次。
  // 记忆放在这里，就能在**构造帧之前**把重复挡掉（连字符串都不必再过一遍）。

  /// 最近一次真的下发给渲染面的舞台背景串（`null` = 没有背景）。
  ///
  /// **只按值去重，不带桥身份**：iframe 重建后的补发是 `Live2DStage._attach`
  /// 的职责（每次挂桥都无条件重发 `widget.stageImage`），所以这里按值去重
  /// 不会吞掉补发；反过来若带上桥身份，重挂之后会白推一帧 MB 级大串。
  String? _lastSentStageBg;

  /// 最近一次 `sync` 的**字段指纹**与**桥身份**（同桥 + 同值 ⇒ 不重发）。
  ///
  /// 为什么要带桥身份：`lipSync` / `idleEnabled` / `mouthSensitivity` /
  /// `clickEnabled` 四个字段**只有** `_applyPrefs` 会发（`_attach` 的 sync
  /// 不带它们）。iframe 重建（retry）会换一个新桥，指纹必须跟着作废——
  /// 否则重挂之后用户的口型 / 待机 / 灵敏度设置会静默失效。
  Live2DBridge? _lastSyncBridge;
  List<Object?>? _lastSyncKey;

  /// 偏好变了 → 背景库长度 / 随机 / 间隔可能都变了，**重新对表**。

  // ── P6：无障碍 ──
  /// 流式回复的**节流播报**（读屏用；`text_delta` 是毫秒级的，不节流会淹没读屏）。
  final LiveRegionThrottle _live = LiveRegionThrottle();

  /// 当前模型名（舞台语义标签用）。
  String _modelName = '';

  /// 渲染面阶段（`StageHost` 的覆盖层与舞台语义要用）。
  Live2DBridgePhase _stagePhase = Live2DBridgePhase.loading;

  /// 渲染面**加载进度**（0..1，`null` = 还不知道）。
  ///
  /// 为什么要有这个字段（2026-10-01，F-0001-4 / W1-e2）：进度画在 `StageHost`
  /// 的加载幕布里，而它吃的是一份**快照**。从前这里直接现读
  /// `_stageKey.currentState?.bridge?.progress`——那只有外壳自己重建时才刷新，
  /// 而桥的 progress 通知**不会**让外壳重建 ⇒ 幕布上的百分比会冻在加载开始时
  /// 那一帧。现在由 `Live2DStage.onProgress` 回流：只在值真的变了时 `setState`
  /// 这一个字段（加载期的进度帧可能很密，同值不重建）。
  /// 与 [_stagePhase] 那条 `onPhaseChanged` 是同一接法。
  double? _stageProgress;

  /// 开发者模式（来自 `GET /api/v1/app/status`，与 `--dev-mode` 启动参数一致）。
  ///
  /// 取不到就是 `false`：dev 分区多显示一项的风险，远小于「开发模式下少东西」。
  bool _devMode = false;

  /// 标记外壳需要重建（等价于 `setState(() {})`）。
  ///
  /// **为什么要有这一层**：`setState` 是 `State` 的 `@protected` 成员，只有
  /// State 子类的**实例成员**能调；本文件拆到 `part` 文件里的接线是**扩展方法**，
  /// 直接写 `setState` 会被 `invalid_use_of_protected_member` 拦下。接线片段统一
  /// 改调这里——先改字段、再置脏，与原来的 `setState(() { … })` 逐字等价。

  @override
  void initState() {
    super.initState();
    _initShell();
  }

  /// 记录一条**真的下发出去**的前端 preset 请求（C 栏左列）。纯记账：不发帧、
  /// 不落盘——观测缓冲只在内存里（阶段5 W5a）。

  @override
  void didUpdateWidget(ShellRoot oldWidget) {
    super.didUpdateWidget(oldWidget);
    _didUpdateShell(oldWidget);
  }

  @override
  void dispose() {
    _disposeShell();
    super.dispose();
  }

  /// 连通性自检：结果**内联在字段下方**（不用 toast，一闪而过看不清）。
  ///
  /// 结果落地前先对一次 [_resultEpoch]：用户在这几秒里切走了分区的话，
  /// 这条结果落在一个他已经离开的分区上，看起来会像是「刚测的」。

  /// 一次性结果代际。切分区时自增 ⇒ 在途的异步结果作废。
  int _resultEpoch = 0;

  /// 清掉所有「只对当下这一眼有效」的内联结果。
  ///
  /// 调用方：[_gotoSection]。**不要**在别处随手调——它会让进行中的
  /// 连通性自检结果被静默丢弃（那正是 [_resultEpoch] 想要的效果，
  /// 但只有在「用户已经离开那个分区」时才成立）。

  @override
  Widget build(BuildContext context) {
    // F-0005-2：宿主这一次重建 = 「外壳看不见的那批状态」（模型库 / Mod /
    // 诊断 / 自检结果 / 本模型覆盖…）可能变了。代际 +1，让设置面板的分区内容
    // 跟着刷新一次；**聊天增量不会走到这里**，所以那棵子树不会跟着它重跑。
    _settingsRevision++;
    // 正文一变就同步播报（节流在 `_live` 里做）。
    _syncLiveRegion();

    return ListenableBuilder(
      // 三处都并进来：聊天（消息/错误）、相位、音频解锁状态。
      listenable: Listenable.merge(<Listenable>[
        _live,
        _chat,
        _ui,
        _audio.unlockState,
        _settings,
        _voiceListen,
      ]),
      builder: (BuildContext context, Widget? _) {
        return AppShell(
          key: _shellKey,
          stage: Live2DStage(
            key: _stageKey,
            // 激活过的模型（null = 渲染面默认模型）。传它是为了让 retry /
            // 重建 iframe 之后不只是「没换皮」，而是回到用户选的那个。
            model: _activeModelUrl,
            dark: appPaletteOf(context).dark,
            stageColor: appPaletteOf(context).stageCss,
            // 背景图也是**舞台状态**：传进来后，一旦 iframe 重建（首帧 / retry）
            // 舞台自己就能补发，不再依赖「恰好有另一次偏好变更」。
            stageImage: DisplayPrefs.stageProjectionUrl(
              widget.prefs,
              _backgroundIndex,
            ),
            // 动作幅度（W7，2026-09-23 收口）：**唯一取值口** = syncer 的
            // `active()`（临时覆盖 > 草稿 > 磁盘值）。传它不是「先保存才生效」
            // 那一版——它由防抖监听器实时重算，传进来只为 iframe 重建 / 重挂后
            // 舞台能自愈（RESEARCH §2.3：这条通路过去永远拿到 null）。
            actionScales: _stageActionScales,
            // 预设标签表（显示用）：舞台的倒计时 ttl 与 director 面板的通道标签
            // 优先查它的 `channel`，取不到表才回落 kExpressionPresetIds。
            presetLabels: _presetLabels,
            // 就绪后补发显示偏好：首次挂载时桥还在 loading，
            // 以及在 retry 重建 iframe 之后（旧队列已随旧桥销毁）。
            onReady: _applyPrefs,
            onPhaseChanged: (Live2DBridgePhase phase) {
              if (mounted && phase != _stagePhase) {
                setState(() => _stagePhase = phase);
              }
            },
            // 加载进度 → 宿主持有一个 double? 快照（最小重建面：整棵壳不因为
            // 加载期的进度帧反复重建，只有这个字段变的那一次 setState）。
            // `onProgress` 自己已经做过「同值不回调」，这里不再防抖。
            onProgress: (double? v) {
              if (mounted && v != _stageProgress) {
                setState(() => _stageProgress = v);
              }
            },
            // 渲染面回执 → 缩放百分比。**首屏也要有值**：过去只在上一次
            // 放大/缩小时才读，于是初始状态一直显示「—」。
            // 渲染面事件级 ack（协议 §7）→ **喂导演的日志文本**（§7.3）。
            onRenderEvent: _directorLog.add,
            onAck: (StageAckEvent ack) {
              if (mounted && ack.scale != _stageScaleFromAck) {
                setState(() => _stageScaleFromAck = ack.scale);
              }
            },
          ),
          phase: _ui.phase,
          wsStatus: _ui.wsStatus,
          // 背景：库 + 索引 + 渲染参数全走偏好。判据（同步开 = 画舞台那张）
          // 收在 `AppShell._currentBackground` 一处，渲染层不自己判。
          prefs: widget.prefs,
          backgroundIndex: _backgroundIndex,
          // 9b：水合中把背景库的写操作禁掉——读回窗口里的写会与读回结果打架。
          backgroundHydrating: widget.backgroundHydrating,
          messages: _chat.messages,
          // 朗读高亮：正在播放的句号（与音频帧同一个数；null = 不高亮）。
          playingSentenceSeq: _chat.playingSentenceSeq,
          input: _input,
          onSend: () => unawaited(_sendWithCancellation()),
          onStop: () => unawaited(_stopWithCancellation()),
          onRetryConnection: _ws.ensureConnected,
          volume: widget.prefs.volume,
          muted: widget.prefs.muted,
          onVolumeChanged: (double v) => _updatePrefs(
            widget.prefs.copyWith(volume: DisplayPrefs.clampVolume(v)),
          ),
          onMutedChanged: (bool m) =>
              _updatePrefs(widget.prefs.copyWith(muted: m)),
          serverMuted: _serverMuted,
          audioUnlocked: _audio.unlocked,
          onEnableSound: _audio.unlock,
          onUserGesture: _audio.unlock,
          // 常态语音检测：聊天主界面常驻的「听」按钮（唤醒词缺省「小可爱」）。
          listenSupported: _voiceListen.supported,
          listening: _voiceListen.listening,
          listenStatus: _voiceListen.statusLine,
          listenError: _voiceListen.error,
          // 2026-10-08：产品路径 = 点一下开录、再点一下结束，定稿进输入框
          //（不打语音端点、不自动发送；不再有 PTT，也不再常驻等唤醒词）。
          onToggleListen: () => unawaited(_voiceListen.toggleDictation()),
          // 诚实性：Web Speech 是云端识别、需联网、音频会出本机。
          listenNote: _voiceListen.supported ? kVoiceWebSpeechNote : null,
          // L1 基座：Mod 变更后的统一提示（聊天区顶部常驻，可关）。
          modRestartNotice: _modRestartNotice,
          onDismissModRestart: _dismissModRestart,
          error: _ui.errorMessage ?? _chat.error,
          errorActions: errorActionsFor(
            _ui.errorMessage ?? _chat.error,
            // 顶部横幅（`_ui`）优先，所以它也优先提供码——两处都存了同一份。
            code: _ui.errorCode ?? _chat.errorCode,
            // F-0001-1（P1，2026-10-01 热补丁）：「去对话设置 / 去语音合成设置」
            // 过去只接 `_gotoSection`，而它**只换分区**——设置面板默认是**关着**
            // 的，于是用户点下去什么都看不见 =「按钮失灵」。所以换完分区还要
            // **真的把面板打开**（与快捷键那条同样的入口 `openSettings`）。
            //
            // **为什么多绕一层 `addPostFrameCallback`**（实测，不是保险起见）：
            // 在**同一帧**里「换分区 + 开浮层」会撞框架断言
            // `setState() or markNeedsBuild() called during build` ——
            // `_gotoSection` 的 `setState` 让外壳重建时，
            // `AppShell.didUpdateWidget`（`app_shell.dart:420`）会同步
            // `sectionNotifier`，而 medium 上刚推入的浮层里那个
            // `ValueListenableBuilder` 已经挂上并在监听它；它是 Overlay 下的
            // **兄弟**、不是宿主的后代，所以这次通知不被允许（expanded 的
            // 内联侧板不触发，因为它不是浮层路由）。
            // 延到本帧之后：外壳先按新分区重建（那一刻还没有订阅者），再开面板。
            onGoto: (SettingsSection next, String? group) {
              _gotoSection(next, group: group);
              WidgetsBinding.instance.addPostFrameCallback((Duration _) {
                unawaited(
                  _shellKey.currentState?.openSettings() ??
                      Future<void>.value(),
                );
              });
            },
            // busy 的出路是**两件事**（F-0007-2）：先打断在飞的那一轮，
            // 再把那条消息重发出去。合成一份交给 `errorActionsFor`，
            // 顺序与「两步都做」收在 `interruptAndResend` 里（VM 可测）。
            onInterruptAndResend: () {
              unawaited(_interruptAndResend());
            },
            // 无码兜底「重试」= **重发上一条用户消息**（2026-10-06 裁决）：
            // 旧实现接的 _sendWithCancellation 读输入框，而一次失败之后输入框
            // 早已被 send() 清空 ⇒ 按下去毫无反应（静默 no-op）。没有上一条
            // 可重发时传 null，让按钮**不出现**（errorActionsFor 里那条）。
            onResendLast: _chat.lastUserMessageText == null
                ? null
                : () => unawaited(_resendLastUserMessage()),
          ),
          onDismissError: () {
            _ui.clearError();
            _chat.clearError();
          },
          // 「重试」= **重发上一条用户消息**（2026-10-06 裁决，R4-T5）：旧形态接
          // `_sendWithCancellation`（读输入框），而一次失败之后输入框已被
          // `send()` 清空 ⇒ 按下去毫无反应（静默 no-op）。没有上一条可重发时
          // 传 **null**：状态胶囊不可点、失败气泡不出按钮——两条消费路径都
          // 不摆假出路（判据同 `onResendLast` 那一处，是同一份真源）。
          onRetryLast: _chat.lastUserMessageText == null
              ? null
              : () => unawaited(_resendLastUserMessage()),
          // 2026-10-08：没开开发者模式时导航里没有「诊断」整项；
          // 「开发模式」自己永远在（它是打开它的唯一入口）。
          sections: visibleSections(devMode: _devMode),
          // 打开设置**就要**加载（不能只靠「换分区」顺带触发，
          // 否则 expanded/medium 直接点「设置」是个空壳）。
          onEnsureSectionLoaded: () => unawaited(_ensureSettingsLoaded()),
          // 浮层内容必须订阅设置数据：`showModalBottomSheet` 的 builder
          // 只跑一次，不订阅的话「加载中」的转圈会一直转下去。
          settingsChanges: _settings,
          settingsRevision: _settingsRevision,
          section: _section,
          onSectionChanged: _onSectionChanged,
          sectionBuilder: _buildSection,
          devMode: _devMode,
          // ── 设置草稿的生命周期（外壳只呈现与拦截，状态在 SettingsController） ──
          settingsDirty: _settings.dirty,
          settingsSaving: _settings.saving,
          settingsStatus: _settings.error ?? _settings.lastOutcome?.message,
          settingsStatusIsError:
              _settings.error != null ||
              _settings.lastOutcome == SaveOutcome.failed,
          onSaveSettings: () => unawaited(_saveSettings()),
          onDiscardSettings: _settings.discard,
          confirmDiscard: _confirmDiscard,
          // ── 舞台浮标：只剩缩放三键（原文件 `action_toolbar.dart` 已按实际内容
          //    改名为 `ui/stage_corner_controls.dart`；动作工具条早已移出成品） ──
          // ── 多会话（P-会话：**本地记录**，模型记忆待实现） ──
          sessions: _chat.sessions.byRecency,
          activeSessionId: _chat.sessions.activeId,
          // 浮层要在**自己的 builder 里**读此刻的列表 / 当前 id：
          // 只传快照会让「删掉一行，行还在」（见 ui/session_sheet.dart）。
          sessionsListenable: _chat,
          readSessions: () => _chat.sessions.byRecency,
          readActiveSessionId: () => _chat.sessions.activeId,
          onNewSession: _chat.newSession,
          onSelectSession: _chat.selectSession,
          onRenameSession: _chat.renameSession,
          onDeleteSession: _chat.deleteSession,
          stageCorner: StageCornerControls(
            onZoom: (String dir) => unawaited(_zoom(dir)),
            scaleFromAck: _stageScaleFromAck,
          ),
          // ── P6：无障碍 ──
          modelName: _modelName,
          stagePhase: _stagePhase,
          stageProgress: _stageProgress,
          stageError: _stageKey.currentState?.errorMessage,
          onRetryStage: () => _stageKey.currentState?.retry(),
          announcement: _live.hasAnnouncement ? _live.announcement : null,
          shortcuts: AppShortcutCallbacks(
            isMacOS: defaultTargetPlatform == TargetPlatform.macOS,
            onSend: () => unawaited(_sendWithCancellation()),
            onStop: () => unawaited(_stopWithCancellation()),
            // 覆盖层优先：它关掉了就不再停止本轮。
            onDismissOverlay: () {
              final AppShellState? shell = _shellKey.currentState;
              if (shell == null || !shell.settingsOpen) return false;
              unawaited(shell.closeSettings());
              return true;
            },
            onOpenSettings: () {
              unawaited(
                _shellKey.currentState?.openSettings() ?? Future<void>.value(),
              );
            },
            onHelp: () => unawaited(showShortcutHelpDialog(context)),
          ),
        );
      },
    );
  }
}
