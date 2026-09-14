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
import 'api/ws_client.dart';
import 'app/app_shell.dart';
import 'app/app_shortcuts.dart';
import 'app/browser_io.dart';
import 'app/shortcut_help_dialog.dart';
import 'audio/audio_player.dart';
import 'chat/chat_controller.dart';
import 'design/tokens.dart';
import 'live2d/live2d_bridge.dart';
import 'live2d/live2d_stage.dart';
import 'settings/display_prefs.dart';
import 'settings/sections/appearance_section.dart';
import 'settings/sections/dev_tools_section.dart';
import 'settings/sections/llm_section.dart';
import 'settings/sections/persona_section.dart';
import 'settings/sections/tts_section.dart';
import 'settings/settings_controller.dart';
import 'settings/settings_sections.dart';
import 'state/live_region.dart';
import 'state/ui_state_tracker.dart';
import 'ui/error_actions.dart';
import 'ui/stage_corner_controls.dart';
import 'ui/theme.dart';

part 'app/shell_admin.dart';
part 'app/shell_chat.dart';
part 'app/shell_prefs.dart';
part 'app/shell_settings.dart';

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
class Live2DShellApp extends StatefulWidget {
  const Live2DShellApp({super.key});

  @override
  State<Live2DShellApp> createState() => _Live2DShellAppState();
}

class _Live2DShellAppState extends State<Live2DShellApp> {
  /// 本地显示偏好（主题 / 缩放 / 口型 / 音量 / 静音），持久化在 localStorage。
  DisplayPrefs _prefs = const DisplayPrefs();

  @override
  void initState() {
    super.initState();
    // 读盘只在启动时一次：之后的每一次写都是 `_update` 的副作用。
    _prefs = loadDisplayPrefs();
  }

  /// 更新并**立即持久化**；返回**是否真的写进了本机存储**。
  ///
  /// 值没变时直接返回 `true`（没有需要写的东西）。写失败（无痕 / 配额满 /
  /// 存储被禁）由调用方决定怎么如实告诉用户——这里不再静默吞掉。
  bool _update(DisplayPrefs next) {
    if (next == _prefs) return true;
    setState(() => _prefs = next);
    return saveDisplayPrefs(next);
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'Live2D Ai',
    debugShowCheckedModeBanner: false,
    theme: buildAppTheme(_prefs.theme),
    home: ShellRoot(prefs: _prefs, onPrefsChanged: _update),
  );
}

/// 组合根：持有全部长生命周期对象，把回调接到新外壳上。
class ShellRoot extends StatefulWidget {
  const ShellRoot({
    required this.prefs,
    required this.onPrefsChanged,
    super.key,
  });

  /// 本地显示偏好（**由根持有**；见 `Live2DShellApp` 的说明）。
  final DisplayPrefs prefs;

  /// 上报偏好变更；返回**是否成功落盘**（失败时调用方给一句可执行文案）。
  final bool Function(DisplayPrefs) onPrefsChanged;

  @override
  State<ShellRoot> createState() => _ShellRootState();
}

class _ShellRootState extends State<ShellRoot> {
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

  /// 当前设置分区（受控；外壳只上报意图）。
  SettingsSection _section = SettingsSection.appearance;

  /// 服务端静音观测值（WS `audio.muted`，**只读**）。
  bool _serverMuted = false;

  // ── P4：设置草稿 + 四个管理面 ──
  late final SettingsController _settings;
  late final ModelsApi _modelsApi;
  late final EnvApi _envApi;
  late final ModsApi _modsApi;
  late final DiagnosticsApi _diagApi;

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
  String? _llmTest;   bool _llmTesting = false;
  String? _ttsTest;   bool _ttsTesting = false;
  /// 舞台背景图的提示（选图与其它通道的失败原因完全不同）。
  String? _stageImageMessage; bool _stageImageFailed = false;

  /// 壳背景图的提示（2026-09-14，rc.5）：与舞台那条**分开**，
  /// 否则在壳那行选完图会在舞台那行冒出一句话。
  String? _shellImageMessage; bool _shellImageFailed = false;
  bool _copied = false;
  bool _settingsLoadedOnce = false;

  // ── 动作子系统已于 2026-09-11 移除（用户裁决：LLM 无工具、只做对话）──
  //
  // 这里曾经有 `_performingAction`（当前正在演的动作协议名）与 `_onActionState`
  // （把 WS `action_state` 帧转成渲染面的 `action-state`）。随
  // `live2d_perform_action` 工具、`action_state` 帧与 `/api/v1/commands`
  // 端点一起删除——服务端不再发这一帧，前端也就不再有任何动作通道。

  /// 渲染面回报的实际缩放（`stage-ack`；未收到时为 null，**不猜**）。
  double? _stageScaleFromAck;

  // ── P6：无障碍 ──
  /// 流式回复的**节流播报**（读屏用；`text_delta` 是毫秒级的，不节流会淹没读屏）。
  final LiveRegionThrottle _live = LiveRegionThrottle();

  /// 当前模型名（舞台语义标签用）。
  String _modelName = '';

  /// 渲染面阶段（`StageHost` 的覆盖层与舞台语义要用）。
  Live2DBridgePhase _stagePhase = Live2DBridgePhase.loading;

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
  void _refresh() => setState(() {});

  @override
  void initState() {
    super.initState();
    _api = ApiClient();
    _modelsApi = ModelsApi();
    _envApi = EnvApi();
    _modsApi = ModsApi();
    _diagApi = DiagnosticsApi();
    _settings = SettingsController(api: _api);
    _ws = WsClient();
    _audio = AudioPlayer();
    // 静音与音量是纯本机输出设置（不经渲染面）：**默认出声**。
    _audio.muted = widget.prefs.muted;
    _audio.volume = widget.prefs.volume;
    // 会话存档：启动时读一次，之后每次变动写回。
    //
    // **落盘时机由 `ChatController` 决定**（一轮结束 / 会话操作），
    // 不是每条消息——`text_delta` 是毫秒级的，逐条写会把主线程拖垮。
    _chat = ChatController(
      api: _api,
      ws: _ws,
      audio: _audio,
      sessions: loadChatSessions(),
      persistSessions: saveChatSessions,
    );

    // 音频 RMS → 舞台口型（30 Hz 高频信号，**不进 Widget 树**，原则 P6）。
    _levelSubscription = _audio.levels.listen((level) {
      _stageKey.currentState?.setMouth(level);
    });

    // 相位跟踪器自己订阅 WS（只读消费，与 ChatController 互不干扰）。
    _statusSubscription = _ws.statuses.listen(_ui.onWsStatus);
    _eventSubscription = _ws.events.listen((WsEvent event) {
      final bool wasMuted = _serverMuted;
      _ui.consume(event);
      if (event is AudioEvent && event.muted != wasMuted) {
        setState(() => _serverMuted = event.muted);
      }
    });

    _ws.connect();
    unawaited(_loadAppStatus());
  }

  @override
  void dispose() {
    unawaited(_levelSubscription?.cancel());
    unawaited(_statusSubscription?.cancel());
    unawaited(_eventSubscription?.cancel());
    _ui.dispose();
    _live.dispose();
    _settings.dispose();
    _modelsApi.dispose();
    _envApi.dispose();
    _modsApi.dispose();
    _diagApi.dispose();
    _chat.dispose();
    _ws.dispose();
    _audio.dispose();
    _api.dispose();
    _input.dispose();
    super.dispose();
  }

  /// 连通性自检：结果**内联在字段下方**（不用 toast，一闪而过看不清）。
  ///
  /// 结果落地前先对一次 [_resultEpoch]：用户在这几秒里切走了分区的话，
  /// 这条结果落在一个他已经离开的分区上，看起来会像是「刚测的」。
  Future<void> _testLlm() async {
    final int epoch = _resultEpoch;
    setState(() => _llmTesting = true);
    try {
      final SettingsTestOutcome o = await _api.testLlm();
      if (!mounted || epoch != _resultEpoch) return;
      setState(() {
        _llmTesting = false;
        _llmTest = o.ok
            ? 'ok · ${o.latencyMs ?? '?'} ms'
                '${o.modelEcho == null || o.modelEcho!.isEmpty ? '' : ' · 模型：${o.modelEcho}'}'
            : '失败：${o.errorMessage ?? o.errorCode ?? '未知原因'}';
      });
    } on ApiException catch (e) {
      if (!mounted || epoch != _resultEpoch) return;
      setState(() {
        _llmTesting = false;
        _llmTest = '失败：${e.message}';
      });
    }
  }

  Future<void> _testTts() async {
    final int epoch = _resultEpoch;
    setState(() => _ttsTesting = true);
    try {
      final SettingsTestOutcome o = await _api.testTts();
      if (!mounted || epoch != _resultEpoch) return;
      setState(() {
        _ttsTesting = false;
        _ttsTest = o.ok
            // 成功时把服务端的 note 也带上（例如「上游可达但未提供 /models」）
            // ——否则用户会以为「自检通过 = 合成没问题」，而下一次合成失败时
            // 又回到「明明通过了却不行」的困惑。
            ? 'ok · ${o.latencyMs ?? '?'} ms${o.note == null ? '' : ' · ${o.note}'}'
            : '失败：${o.errorMessage ?? o.errorCode ?? '未知原因'}';
      });
    } on ApiException catch (e) {
      if (!mounted || epoch != _resultEpoch) return;
      setState(() {
        _ttsTesting = false;
        _ttsTest = '失败：${e.message}';
      });
    }
  }

  /// 打开设置前的兜底：**懒加载一次**。
  void _onSectionChanged(SettingsSection next) => _gotoSection(next);

  /// 切换设置分区（**唯一入口**）。
  ///
  /// # 为什么必须走这一个入口（2026-09-11，P2-2）
  ///
  /// 过去有两个地方各自 `setState(() => _section = …)`：分区 chip（这里）
  /// 与错误横幅的「去 LLM 设置 / 去语音合成设置」按钮。于是**内联结果**
  /// （`_llmTest` / `_ttsTest` / `_adminMessage` / `_stageImageMessage`）
  /// 会一直挂在 State 上：用户测出「失败：401」，
  /// 修好配置、切到别的分区、再回来——**那句失效的结论还在**，
  /// 而它描述的已经是上一套配置了。
  ///
  /// 现在切分区**先清掉这些一次性结果**，并且用 [_resultEpoch] 把
  /// 「切走之后才回来的响应」也丢掉（否则那个迟到的结果会落在用户已经
  /// 离开的分区上，看起来像是刚测的）。
  void _gotoSection(SettingsSection next) {
    if (next == _section) return;
    setState(() {
      _section = next;
      _clearTransientResults();
    });
    unawaited(_ensureSettingsLoaded());
  }

  /// 一次性结果代际。切分区时自增 ⇒ 在途的异步结果作废。
  int _resultEpoch = 0;

  /// 清掉所有「只对当下这一眼有效」的内联结果。
  ///
  /// 调用方：[_gotoSection]。**不要**在别处随手调——它会让进行中的
  /// 连通性自检结果被静默丢弃（那正是 [_resultEpoch] 想要的效果，
  /// 但只有在「用户已经离开那个分区」时才成立）。
  void _clearTransientResults() {
    _resultEpoch++;
    _llmTest = null;
    _llmTesting = false;
    _ttsTest = null;
    _ttsTesting = false;
    _adminMessage = null;
    _stageImageMessage = null;
    _stageImageFailed = false;
    _shellImageMessage = null;
    _shellImageFailed = false;
    _copied = false;
    _adminError = null;
  }

  @override
  Widget build(BuildContext context) {
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
            stageImage: widget.prefs.stageImage,
            // 就绪后补发显示偏好：首次挂载时桥还在 loading，
            // 以及在 retry 重建 iframe 之后（旧队列已随旧桥销毁）。
            onReady: _applyPrefs,
            onPhaseChanged: (Live2DBridgePhase phase) {
              if (mounted && phase != _stagePhase) {
                setState(() => _stagePhase = phase);
              }
            },
            // 渲染面回执 → 缩放百分比。**首屏也要有值**：过去只在上一次
            // 放大/缩小时才读，于是初始状态一直显示「—」。
            onAck: (StageAckEvent ack) {
              if (mounted && ack.scale != _stageScaleFromAck) {
                setState(() => _stageScaleFromAck = ack.scale);
              }
            },
          ),
          phase: _ui.phase,
          wsStatus: _ui.wsStatus,
          // 壳全局背景：同步开时就是舞台那张图（一份真相），关时用壳自己的。
          shellImage: widget.prefs.effectiveShellImage,
          messages: _chat.messages,
          input: _input,
          onSend: () => unawaited(_send()),
          onStop: () => unawaited(_stop()),
          onRetryConnection: _ws.ensureConnected,
          volume: widget.prefs.volume,
          muted: widget.prefs.muted,
          onVolumeChanged: (double v) =>
              _updatePrefs(
                widget.prefs.copyWith(
                  volume: DisplayPrefs.clampVolume(v),
                ),
              ),
          onMutedChanged: (bool m) =>
              _updatePrefs(widget.prefs.copyWith(muted: m)),
          serverMuted: _serverMuted,
          audioUnlocked: _audio.unlocked,
          onEnableSound: _audio.unlock,
          onUserGesture: _audio.unlock,
          error: _ui.errorMessage ?? _chat.error,
          errorActions: errorActionsFor(
            _ui.errorMessage ?? _chat.error,
            // 顶部横幅（`_ui`）优先，所以它也优先提供码——两处都存了同一份。
            code: _ui.errorCode ?? _chat.errorCode,
            onGoto: _gotoSection,
            onStop: () => unawaited(_stop()),
            onSend: () => unawaited(_send()),
          ),
          onDismissError: () {
            _ui.clearError();
            _chat.clearError();
          },
          onRetryLast: () => unawaited(_send()),
          sections: visibleSections(),
          // 打开设置**就要**加载（不能只靠「换分区」顺带触发，
          // 否则 expanded/medium 直接点「设置」是个空壳）。
          onEnsureSectionLoaded: () => unawaited(_ensureSettingsLoaded()),
          // 浮层内容必须订阅设置数据：`showModalBottomSheet` 的 builder
          // 只跑一次，不订阅的话「加载中」的转圈会一直转下去。
          settingsChanges: _settings,
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
          stageProgress: _stageKey.currentState?.bridge?.progress,
          stageError: _stageKey.currentState?.errorMessage,
          onRetryStage: () => _stageKey.currentState?.retry(),
          announcement: _live.hasAnnouncement ? _live.announcement : null,
          shortcuts: AppShortcutCallbacks(
            isMacOS: defaultTargetPlatform == TargetPlatform.macOS,
            onSend: () => unawaited(_send()),
            onStop: () => unawaited(_stop()),
            // 覆盖层优先：它关掉了就不再停止本轮。
            onDismissOverlay: () {
              final AppShellState? shell = _shellKey.currentState;
              if (shell == null || !shell.settingsOpen) return false;
              shell.closeSettings();
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
