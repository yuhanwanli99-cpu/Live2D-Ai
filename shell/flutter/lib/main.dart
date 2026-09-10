/// 应用入口：**只做装配**（组合根）。
///
/// 这里是唯一允许 `import 'package:web'` 的地方之一（localStorage 偏好），
/// 所以本文件里的东西**测不到**——因此所有能测的逻辑都已经搬出去了：
///
/// | 逻辑 | 去处 | 测试 |
/// |---|---|---|
/// | 帧解析 | `api/ws_frame.dart` | `ws_frame_test.dart` |
/// | 动作来源 / 历史 | `actions/*` | `action_*_test.dart` |
/// | 界面相位派生 | `state/ui_phase.dart` + `state/ui_state_tracker.dart` | 两个 `ui_*_test.dart` |
/// | 主题 / 令牌 / 断点 | `ui/theme.dart`、`design/*` | `theme_test.dart` 等 |
/// | 外壳与布局 | `app/app_shell.dart` | `app_shell_layout_test.dart`、`stage_keepalive_test.dart` |
///
/// 本文件剩下的只有「谁持有谁」「哪个回调接哪个方法」——那部分没有分支逻辑。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:js_interop';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:web/web.dart' as web;

import 'actions/action_dispatch.dart';
import 'actions/action_event.dart';
import 'api/api_client.dart';
import 'api/commands_api.dart';
import 'api/diagnostics_api.dart';
import 'api/models_api.dart';
import 'api/mods_api.dart';
import 'api/settings_models.dart';
import 'api/ws_client.dart';
import 'audio/audio_player.dart';
import 'chat/chat_controller.dart';
import 'app/app_shell.dart';
import 'app/app_shortcuts.dart';
import 'live2d/live2d_bridge.dart';
import 'live2d/live2d_stage.dart';
import 'settings/display_prefs.dart';
import 'settings/sections/actions_section.dart';
import 'settings/sections/dev_tools_section.dart';
import 'settings/sections/llm_section.dart';
import 'settings/sections/persona_section.dart';
import 'settings/sections/tts_section.dart';
import 'settings/settings_controller.dart';
import 'settings/settings_sections.dart';
import 'state/live_region.dart';
import 'state/ui_state_tracker.dart';
import 'ui/action_grid.dart';
import 'ui/action_toolbar.dart';
import 'ui/error_banner.dart';
import 'design/tokens.dart';
import 'ui/theme.dart';

/// 本地显示偏好的存储键（localStorage）。
const String kDisplayPrefsKey = 'live2d-ai.display-prefs';

/// 读取本地显示偏好：**任何异常都回落默认**（坏存储不该让舞台渲染不出来）。
DisplayPrefs loadDisplayPrefs() {
  try {
    final raw = web.window.localStorage.getItem(kDisplayPrefsKey);
    if (raw == null || raw.isEmpty) return const DisplayPrefs();
    final decoded = jsonDecode(raw);
    if (decoded is! Map) return const DisplayPrefs();
    return DisplayPrefs.fromJson(decoded.cast<String, Object?>());
  } catch (_) {
    return const DisplayPrefs();
  }
}

/// 写入本地显示偏好（失败静默：无痕模式等场景下 localStorage 可能抛错）。
void saveDisplayPrefs(DisplayPrefs prefs) {
  try {
    web.window.localStorage.setItem(
      kDisplayPrefsKey,
      jsonEncode(prefs.toJson()),
    );
  } catch (_) {
    // 忽略：偏好丢失不影响本次会话。
  }
}

/// 让用户挑一个本地文件并读成字节（**组合根专有**：需要 `package:web`）。
///
/// 为什么不用文件选择器插件：一次 `<input type=file>` 就够了，
/// 引一个插件只为这个属于典型的「顺手加一个」。
/// 用户取消选择时返回 `null`（**不当成错误**）。
Future<Uint8List?> pickLocalFile() async {
  final web.HTMLInputElement input =
      web.document.createElement('input') as web.HTMLInputElement;
  input.type = 'file';
  // 酒馆卡常见两种：`.json` 与内嵌人设的 `.png`。
  input.accept = '.json,.png,application/json,image/png';
  final Completer<Uint8List?> done = Completer<Uint8List?>();
  input.onchange = ((web.Event _) {
    final web.FileList? files = input.files;
    if (files == null || files.length == 0) {
      if (!done.isCompleted) done.complete(null);
      return;
    }
    final web.File file = files.item(0)!;
    final web.FileReader reader = web.FileReader();
    reader.onload = ((web.Event _) {
      final Object? result = reader.result;
      if (result == null) {
        if (!done.isCompleted) done.complete(null);
        return;
      }
      final ByteBuffer buffer =
          (result as JSArrayBuffer).toDart.asUint8List().buffer;
      if (!done.isCompleted) {
        done.complete(buffer.asUint8List());
      }
    }).toJS;
    reader.onerror = ((web.Event _) {
      if (!done.isCompleted) done.complete(null);
    }).toJS;
    reader.readAsArrayBuffer(file);
  }).toJS;
  // **取消选择也要能收口**：`oncancel` 在旧浏览器上可能不触发，
  // 所以这里不做超时兜底——用户若一直不选，这个 Future 就挂着，
  // 而它没有任何副作用（不占资源、不改状态）。
  input.oncancel = ((web.Event _) {
    if (!done.isCompleted) done.complete(null);
  }).toJS;
  input.click();
  return done.future;
}

void main() {
  runApp(const Live2DShellApp());
}

/// 应用根：深色 Material 3 主题。
class Live2DShellApp extends StatelessWidget {
  const Live2DShellApp({super.key});

  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'Live2D Ai',
    debugShowCheckedModeBanner: false,
    theme: buildAppTheme(),
    home: const ShellRoot(),
  );
}

/// 组合根：持有全部长生命周期对象，把回调接到新外壳上。
class ShellRoot extends StatefulWidget {
  const ShellRoot({super.key});

  @override
  State<ShellRoot> createState() => _ShellRootState();
}

class _ShellRootState extends State<ShellRoot> {
  final GlobalKey<Live2DStageState> _stageKey = GlobalKey<Live2DStageState>();
  final GlobalKey<AppShellState> _shellKey = GlobalKey<AppShellState>();
  final TextEditingController _input = TextEditingController();

  late final ApiClient _api;
  late final CommandsApi _commandsApi;
  late final WsClient _ws;
  late final AudioPlayer _audio;
  late final ChatController _chat;

  /// 界面相位派生（WS 信号 + 渲染面状态 → `UiPhase`）。**纯逻辑，可单测。**
  final UiStateTracker _ui = UiStateTracker();

  StreamSubscription<double>? _levelSubscription;
  StreamSubscription<WsStatus>? _statusSubscription;
  StreamSubscription<WsEvent>? _eventSubscription;

  /// 本地显示偏好（模型缩放 / 口型灵敏度 / 音量 / 静音）；持久化在 localStorage。
  DisplayPrefs _prefs = const DisplayPrefs();

  /// 当前设置分区（受控；外壳只上报意图）。
  SettingsSection _section = SettingsSection.actions;

  /// 服务端静音观测值（WS `audio.muted`，**只读**）。
  bool _serverMuted = false;

  // ── P4：设置草稿 + 四个管理面 ──
  late final SettingsController _settings;
  late final ModelsApi _modelsApi;
  late final ModsApi _modsApi;
  late final DiagnosticsApi _diagApi;

  List<ModelInfo> _models = const <ModelInfo>[];
  List<ModInfo> _mods = const <ModInfo>[];
  List<LogLine> _logs = const <LogLine>[];
  String? _logsError;
  Map<String, Object?> _status = const <String, Object?>{};
  AppCapabilities _capabilities = const AppCapabilities();
  bool _adminLoading = false;
  String? _adminError;
  String? _busyId;
  String? _actionMessage;
  String? _llmTest;   bool _llmTesting = false;
  String? _ttsTest;   bool _ttsTesting = false;
  String? _importMessage; bool _importFailed = false;
  bool _copied = false;
  bool _settingsLoadedOnce = false;

  // ── P5：动作通道 ──
  /// 动作触发的本地状态（幂等提示 + 「正在演」游标）。
  final ActionDispatch _dispatch = ActionDispatch();

  /// 服务端给的可触发动作目录（`GET /api/v1/commands`）。
  CommandCatalog _catalog = const CommandCatalog.empty();

  /// 动作强度档位（1..3，全局一份）。
  int _strength = 2;

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

  @override
  void initState() {
    super.initState();
    _prefs = loadDisplayPrefs();
    _api = ApiClient();
    _commandsApi = CommandsApi();
    _modelsApi = ModelsApi();
    _modsApi = ModsApi();
    _diagApi = DiagnosticsApi();
    _settings = SettingsController(api: _api);
    _ws = WsClient();
    _audio = AudioPlayer();
    // 静音与音量是纯本机输出设置（不经渲染面）：**默认出声**。
    _audio.muted = _prefs.muted;
    _audio.volume = _prefs.volume;
    _chat = ChatController(api: _api, ws: _ws, audio: _audio);

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
      if (event is ActionStateEvent) {
        _onActionState(event);
      }
    });

    _ws.connect();
    unawaited(_loadActionCatalog());
    unawaited(_loadAppStatus());
  }

  /// 取一次应用状态（拿 `dev_mode`）。
  Future<void> _loadAppStatus() async {
    try {
      final Map<String, Object?> status = await _api.fetchStatus();
      final bool dev = status['dev_mode'] == true;
      if (mounted && dev != _devMode) setState(() => _devMode = dev);
    } catch (_) {
      // 忽略：保持 false。
    }
  }

  /// 取一次动作目录，把「协议名 → 中文名」灌进动作日志。
  ///
  /// **失败静默**：目录是可选资源。取不到就显示协议名（`nod` 而不是「点头」）——
  /// 看得懂，且不撒谎。权威来源是服务端 `GET /api/v1/commands`，
  /// 前端**不内建**映射表（AGENTS.md：不得复制核心逻辑）。
  Future<void> _loadActionCatalog() async {
    try {
      final CommandCatalog catalog = await _commandsApi.fetchCommands();
      _chat.actionLog.useCatalog(catalog);
      if (mounted) setState(() => _catalog = catalog);
    } catch (_) {
      // 忽略：保持恒等解析器（显示协议名）。
    }
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
    _modsApi.dispose();
    _diagApi.dispose();
    _chat.dispose();
    _ws.dispose();
    _audio.dispose();
    _api.dispose();
    _commandsApi.dispose();
    _input.dispose();
    super.dispose();
  }

  /// 把流式正文喂给节流播报器。
  ///
  /// **按「整句到达」优先**：末尾出现句末标点就立刻播报；否则按 1.5 s 限速。
  /// 两级放行的判据在 `LiveRegionThrottle.feed` 里，可单测。
  void _syncLiveRegion() {
    final ChatMessage? last = _chat.messages.isEmpty ? null : _chat.messages.last;
    if (last == null) return;
    if (last.role != ChatRole.assistant) return;
    if (!last.streaming) {
      // 收口：把最后一小段补播一次（末尾往往没有句末标点，比如「好呀」）。
      _live.finish(last.text);
      return;
    }
    _live.feed(last.text);
  }

  /// WS 的动作事件 → **真的让模型动** + 本地忙碌态。
  ///
  /// 这是 P5 之前**完全缺失**的一环：`action_state` 只进了动作日志，
  /// 没有任何代码把它转成渲染面的 `action-state`，所以「AI 触发了点头」
  /// 在日志里有记录、模型却一次都没动过。
  ///
  /// 用户手点与 AI 触发走的是**同一条**下行通道（渲染面不需要区分来源，
  /// 仲裁已经在 core 做完了）。
  void _onActionState(ActionStateEvent frame) {
    final ActionEvent event = ActionEvent.fromFrame(frame, receivedAt: DateTime.now());
    _dispatch.consume(event);
    final String? action = event.action;
    if (action != null) {
      unawaited(
        _stageKey.currentState?.sendActionState(
              action: action,
              state: event.isCease ? 'end' : 'start',
              strength: event.strength,
            ) ??
            Future<void>.value(),
      );
    } else if (event.isCease) {
      // `cease` 帧**不带动作名**（协议如此）。渲染面按 `state != "start"`
      // 停止当前动作，所以这里用一个空 action —— 但渲染面会先读根级
      // `action` 再判 state，空 action 会让它找不到要停的动作。
      // 因此用**当前正在演的那个**动作名去停（本地游标就是为此存在的）。
      final String? current = _dispatch.performing;
      if (current != null) {
        unawaited(
          _stageKey.currentState?.sendActionState(
                action: current,
                state: 'end',
              ) ??
              Future<void>.value(),
        );
      }
    }
  }

  /// 手动触发一个动作（工具条 / 网格）。
  ///
  /// **重复点击仍会发出 invoke**：服务端才是仲裁者，而且重复点击的代价只是
  /// 白跑一趟。本地只负责**给出解释**——否则用户看到的就是「点了没反应」。
  Future<void> _invokeAction(CommandInfo command) async {
    final String? action = command.targetAction;
    final bool duplicate = action == null
        ? false
        : _dispatch.recordInvoke(action, _strength);
    if (duplicate) {
      final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
      messenger.hideCurrentSnackBar();
      messenger.showSnackBar(
        SnackBar(content: Text(_dispatch.duplicateHint), duration: const Duration(seconds: 2)),
      );
    }
    try {
      final CommandInvokeResult result = await _commandsApi.invoke(
        command.id,
        strength: _strength,
      );
      if (!mounted) return;
      // `accepted: false` 也是合法响应（core 明确拒绝），如实提示。
      if (!result.accepted) {
        _dispatch.recordError('服务端未受理这次触发');
      }
    } on ApiException catch (e) {
      if (!mounted) return;
      _dispatch.recordError(e.message);
    }
  }

  /// 舞台角标三键 → 渲染面（渲染面自己算缩放，真值从 `stage-ack` 取）。
  Future<void> _zoom(String dir) async {
    await _stageKey.currentState?.sendStageZoom(dir);
    if (!mounted) return;
    setState(() => _stageScaleFromAck = _stageKey.currentState?.lastAck?.scale);
  }

  /// 把 [_prefs] 下发给渲染面（协议 v1 `sync`）。
  void _applyPrefs() {
    _stageKey.currentState?.sync(
      scale: _prefs.scale,
      lipSync: _prefs.lipSync,
      idleEnabled: _prefs.idleEnabled,
      mouthSensitivity: _prefs.mouthSensitivity,
      // 「允许拖动与缩放」下发到渲染面的 `sync.clickEnabled`。
      clickEnabled: _prefs.allowDragZoom,
      tier: _prefs.tier,
    );
  }

  /// 偏好变更：更新内存 + 落盘 + 立即下发。
  void _updatePrefs(DisplayPrefs next) {
    if (next == _prefs) return;
    setState(() => _prefs = next);
    saveDisplayPrefs(next);
    // 静音与音量是纯本机输出设置，不经渲染面——直接作用于 AudioPlayer。
    _audio.muted = next.muted;
    _audio.volume = next.volume;
    _applyPrefs();
  }

  Future<void> _send() async {
    final String text = _input.text;
    if (text.trim().isEmpty) return;
    if (_chat.streaming) return;
    _input.clear();
    _ui.markTurnAccepted();
    await _chat.send(text);
    _syncLiveRegion();
  }

  Future<void> _stop() async {
    await _chat.stop();
    _syncLiveRegion();
    // 停止后立刻收口并显示「已打断」（不等服务端的 turn_state，那是它的节奏）。
    _ui.markStopped();
  }

  /// 错误提示的「下一步」（规格 §6.6：失败必须有出路，不能只报告）。
  List<ErrorAction> _errorActions(String? message) {
    if (message == null) return const <ErrorAction>[];
    if (message.contains('busy') || message.contains('上一轮')) {
      return <ErrorAction>[
        ErrorAction(
          label: '打断并重发',
          onPressed: () => unawaited(_stop()),
        ),
      ];
    }
    if (message.contains('no_supervisor') || message.contains('未就绪')) {
      return <ErrorAction>[
        ErrorAction(
          label: '去 LLM 设置',
          onPressed: () => setState(() => _section = SettingsSection.llm),
        ),
      ];
    }
    return <ErrorAction>[
      ErrorAction(label: '重试', onPressed: () => unawaited(_send())),
    ];
  }

  // ── 管理面加载（模型 / Mod / 诊断） ───────────────────────────────

  /// 进设置面时懒加载一次。**不放在 initState**：这些数据多数时候用不到，
  /// 启动就打三个请求是白花钱（也拖慢首屏）。
  Future<void> _ensureSettingsLoaded() async {
    if (_settingsLoadedOnce) return;
    _settingsLoadedOnce = true;
    await Future.wait<void>(<Future<void>>[
      _settings.load(),
      _loadAdmin(),
    ]);
  }

  Future<void> _loadAdmin() async {
    setState(() {
      _adminLoading = true;
      _adminError = null;
    });
    try {
      final List<ModelInfo> models = await _modelsApi.list();
      final List<ModInfo> mods = await _modsApi.list();
      final Map<String, Object?> status = await _diagApi.status();
      final AppCapabilities caps = await _diagApi.capabilities();
      if (!mounted) return;
      final Object? activeModel = status['active_model_id'];
      setState(() {
        _models = models;
        _mods = mods;
        _status = status;
        if (activeModel is String) _modelName = activeModel;
        _capabilities = caps;
        _adminLoading = false;
      });
      // 日志只有 dev_mode 才可读——**分开取**，403 不该让整个面板失败。
      await _loadLogs();
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _adminError = e.toString();
        _adminLoading = false;
      });
    }
  }

  Future<void> _loadLogs() async {
    try {
      final List<LogLine> lines = await _diagApi.logs(limit: 200);
      if (!mounted) return;
      setState(() {
        _logs = lines;
        _logsError = null;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _logs = const <LogLine>[];
        // 403 dev_mode_required 是**预期**结果，原文照显（不伪装成空日志）。
        _logsError = e.code == 'dev_mode_required'
            ? '日志需要服务端以 dev_mode 启动才可读'
            : e.toString();
      });
    }
  }

  /// 「保存」：走 `SettingsController`，文案由 `apply_status` 决定。
  Future<void> _saveSettings() async {
    final SaveOutcome outcome = await _settings.save();
    if (!mounted) return;
    setState(() {});
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    messenger.hideCurrentSnackBar();
    messenger.showSnackBar(
      SnackBar(
        content: Text(outcome.message),
        backgroundColor: outcome.needsAttention
            ? AppPalette.warning
            : null,
      ),
    );
    // 保存的是 dev_mode 时，重新取一次状态（诊断面板要跟着变）。
    if (outcome.isSuccess) unawaited(_loadLogs());
  }

  /// 未保存改动的确认框。**三处拦截共用**（换分区 / 关浮层 / 刷新页面）。
  Future<bool> _confirmDiscard() async {
    if (!_settings.dirty) return true;
    final bool? leave = await showDialog<bool>(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: const Text('有未保存的改动'),
        content: const Text('离开会丢掉这些改动。要先保存吗？'),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('留下'),
          ),
          TextButton(
            onPressed: () async {
              await _saveSettings();
              if (dialogContext.mounted) {
                Navigator.of(dialogContext).pop(true);
              }
            },
            child: const Text('保存并离开'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('放弃改动'),
          ),
        ],
      ),
    );
    return leave ?? false;
  }

  /// 连通性自检：结果**内联在字段下方**（不用 toast，一闪而过看不清）。
  Future<void> _testLlm() async {
    setState(() => _llmTesting = true);
    try {
      final SettingsTestOutcome o = await _api.testLlm();
      if (!mounted) return;
      setState(() {
        _llmTesting = false;
        _llmTest = o.ok
            ? 'ok · ${o.latencyMs ?? '?'} ms'
                '${o.modelEcho == null || o.modelEcho!.isEmpty ? '' : ' · 模型：${o.modelEcho}'}'
            : '失败：${o.errorMessage ?? o.errorCode ?? '未知原因'}';
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _llmTesting = false;
        _llmTest = '失败：${e.message}';
      });
    }
  }

  Future<void> _testTts() async {
    setState(() => _ttsTesting = true);
    try {
      final SettingsTestOutcome o = await _api.testTts();
      if (!mounted) return;
      setState(() {
        _ttsTesting = false;
        _ttsTest = o.ok
            ? 'ok · ${o.latencyMs ?? '?'} ms'
            : '失败：${o.errorMessage ?? o.errorCode ?? '未知原因'}';
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _ttsTesting = false;
        _ttsTest = '失败：${e.message}';
      });
    }
  }

  /// 角色卡导入：**文件读取在组合根**（`dart:html`/`package:web` 只能在这里）。
  Future<void> _pickPersonaCard() async {
    final Uint8List? bytes = await pickLocalFile();
    if (bytes == null || !mounted) return;
    final ({String message, bool failed}) r = applyPersonaImport(
      controller: _settings,
      bytes: bytes,
    );
    setState(() {
      _importMessage = r.message;
      _importFailed = r.failed;
    });
  }

  Future<void> _activateModel(String id) async {
    setState(() => _busyId = id);
    try {
      final ActivateResult r = await _modelsApi.activate(id);
      if (!mounted) return;
      setState(() {
        _busyId = null;
        // **如实上报 requires_restart**：不谎报「已生效」。
        _actionMessage = r.requiresRestart
            ? '已切换到 ${r.activeId}，**需重启服务端**才生效'
            : '已切换到 ${r.activeId}';
      });
      await _loadAdmin();
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busyId = null;
        _actionMessage = '激活失败：${e.message}';
      });
    }
  }

  Future<void> _toggleMod(String id, bool enabled) async {
    setState(() => _busyId = id);
    try {
      await _modsApi.setEnabled(id, enabled);
      if (!mounted) return;
      setState(() {
        _busyId = null;
        _actionMessage = '${enabled ? '已启用' : '已停用'} $id';
      });
      await _loadAdmin();
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busyId = null;
        _actionMessage = '操作失败：${e.message}';
      });
    }
  }

  Future<void> _copyDiagnostics(DiagnosticsSnapshot snapshot) async {
    await copySnapshot(snapshot);
    if (!mounted) return;
    setState(() => _copied = true);
    await Future<void>.delayed(const Duration(seconds: 2));
    if (mounted) setState(() => _copied = false);
  }

  /// 分区内容构建器：8 个分区各自的 pane。
  Widget _buildSection(BuildContext context, SettingsSection section) {
    // 未加载出来时先显示骨架，而不是一个空面板让人以为坏了。
    if (!_settings.loaded && _settings.loading) {
      return const Padding(
        padding: EdgeInsets.symmetric(vertical: 64),
        child: Center(child: CircularProgressIndicator()),
      );
    }
    final SettingsView? view = _settings.remote;
    if (view == null) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 32),
        child: Column(
          children: <Widget>[
            Text('读不到服务端设置', style: Theme.of(context).textTheme.titleSmall),
            const SizedBox(height: 8),
            Text(
              _settings.error ?? '未知原因',
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 12),
            FilledButton.tonal(
              onPressed: () => unawaited(_settings.load()),
              child: const Text('重试'),
            ),
          ],
        ),
      );
    }

    switch (section) {
      case SettingsSection.persona:
        return PersonaSection(
          controller: _settings,
          view: view,
          devMode: _devMode,
          importMessage: _importMessage,
          importFailed: _importFailed,
          onImport: (_) => unawaited(_pickPersonaCard()),
        );
      case SettingsSection.models:
        return ModelsSection(
          models: _models,
          loading: _adminLoading,
          error: _adminError,
          devMode: _devMode,
          busyId: _busyId,
          activateMessage: _actionMessage,
          onActivate: _activateModel,
          onReload: _loadAdmin,
        );
      case SettingsSection.llm:
        return LlmSection(
          controller: _settings,
          view: view,
          devMode: _devMode,
          onTest: _testLlm,
          testResult: _llmTest,
          testing: _llmTesting,
        );
      case SettingsSection.tts:
        return TtsSection(
          controller: _settings,
          view: view,
          devMode: _devMode,
          serverMuted: _serverMuted,
          onTest: _testTts,
          testResult: _ttsTest,
          testing: _ttsTesting,
        );
      case SettingsSection.actions:
        return ActionsSection(
          prefs: _prefs,
          onPrefsChanged: _updatePrefs,
          actionLog: _chat.actionLog,
          devMode: _devMode,
          commands: _catalog.userInvocable,
          dispatch: _dispatch,
          onInvoke: (CommandInfo c) => unawaited(_invokeAction(c)),
          strength: _strength,
          onStrengthChanged: (int v) => setState(() => _strength = v),
        );
      case SettingsSection.mods:
        return ModsSection(
          mods: _mods,
          loading: _adminLoading,
          error: _adminError,
          busyId: _busyId,
          onToggle: _toggleMod,
          onReload: _loadAdmin,
        );
      case SettingsSection.diagnostics:
        return DiagnosticsSection(
          status: _status,
          capabilities: _capabilities,
          wsStatusLabel: _ui.wsStatus.description,
          logs: _logs,
          logsError: _logsError,
          devMode: _devMode,
          copied: _copied,
          onReload: _loadAdmin,
          onCopy: _copyDiagnostics,
        );
      case SettingsSection.developer:
        return DeveloperSection(
          devMode: _devMode,
          // dev_mode 走设置草稿（顶层三态），保存后才写盘。
          onDevModeChanged: (bool v) => _settings.edit(
            (SettingsDraft d) => d.devMode = v,
          ),
          // 启动参数强制开启时**不谎报可关**。
          forcedByLaunchFlag: false,
        );
    }
  }

  /// 工具条上的动作（**高频优先**，最多 8 槽）。
  ///
  /// 排序规则：`kHighFrequencyActions` 里的按该顺序排前面，其余按服务端顺序。
  /// **不硬编码哪 6 个动作存在**——`capabilities.actions` 才是真源。
  List<CommandInfo> get _toolbarCommands {
    final List<CommandInfo> invocable = _catalog.userInvocable;
    final List<CommandInfo> ordered = <CommandInfo>[];
    for (final String action in kHighFrequencyActions) {
      for (final CommandInfo c in invocable) {
        if (c.targetAction == action) ordered.add(c);
      }
    }
    for (final CommandInfo c in invocable) {
      if (!ordered.contains(c)) ordered.add(c);
    }
    return ordered;
  }

  /// 快捷键帮助（`Ctrl/Cmd + /`）。
  ///
  /// 内容来自 `shortcutHelp()`——**唯一文案来源**，改快捷键定义时帮助自动跟上。
  /// 第一版定义了它却没接任何入口，于是被 dart2js 当死代码裁掉。
  Future<void> _showShortcutHelp() async {
    final bool isMac = defaultTargetPlatform == TargetPlatform.macOS;
    await showDialog<void>(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: const Text('快捷键'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            for (final ShortcutHelpEntry e in shortcutHelp(isMacOS: isMac))
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 4),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    SizedBox(
                      width: 110,
                      child: Text(
                        e.keys,
                        style: Theme.of(dialogContext).textTheme.labelLarge,
                      ),
                    ),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(e.action),
                          if (e.note != null)
                            Text(
                              e.note!,
                              style: Theme.of(dialogContext).textTheme.bodySmall,
                            ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
          ],
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('知道了'),
          ),
        ],
      ),
    );
  }

  /// 打开设置前的兜底：**懒加载一次**。
  void _onSectionChanged(SettingsSection next) {
    setState(() => _section = next);
    unawaited(_ensureSettingsLoaded());
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
        _dispatch,
      ]),
      builder: (BuildContext context, Widget? _) {
        return AppShell(
          key: _shellKey,
          stage: Live2DStage(
            key: _stageKey,
            dark: true,
            // 就绪后补发显示偏好：首次挂载时桥还在 loading，
            // 以及在 retry 重建 iframe 之后（旧队列已随旧桥销毁）。
            onReady: _applyPrefs,
            onError: (String message) =>
                _ui.onBridgePhase(message.isNotEmpty),
            onPhaseChanged: (Live2DBridgePhase phase) {
              if (mounted && phase != _stagePhase) {
                setState(() => _stagePhase = phase);
              }
            },
          ),
          phase: _ui.phase,
          wsStatus: _ui.wsStatus,
          messages: _chat.messages,
          input: _input,
          onSend: () => unawaited(_send()),
          onStop: () => unawaited(_stop()),
          onRetryConnection: _ws.ensureConnected,
          volume: _prefs.volume,
          muted: _prefs.muted,
          onVolumeChanged: (double v) =>
              _updatePrefs(_prefs.copyWith(volume: DisplayPrefs.clampVolume(v))),
          onMutedChanged: (bool m) => _updatePrefs(_prefs.copyWith(muted: m)),
          serverMuted: _serverMuted,
          audioUnlocked: _audio.unlocked,
          onEnableSound: _audio.unlock,
          onUserGesture: _audio.unlock,
          error: _ui.errorMessage ?? _chat.error,
          errorActions: _errorActions(_ui.errorMessage ?? _chat.error),
          onDismissError: () {
            _ui.clearError();
            _chat.clearError();
          },
          onRetryLast: () => unawaited(_send()),
          sections: visibleSections(devMode: _devMode),
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
          // ── 舞台浮标：动作工具条（左下）+ 缩放三键（右下） ──
          // 工具条/角标**自己订阅**忙碌态，这里不必再包一层。
          stageToolbar: _toolbarCommands.isEmpty
              ? null
              : ActionToolbar(
                  commands: _toolbarCommands,
                  onInvoke: (CommandInfo c) => unawaited(_invokeAction(c)),
                  dispatch: _dispatch,
                ),
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
            onHelp: () => unawaited(_showShortcutHelp()),
          ),
        );
      },
    );
  }
}
