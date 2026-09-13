/// 应用入口：**只做装配**（组合根）。
///
/// 这里是唯一允许 `import 'package:web'` 的地方之一（localStorage 偏好），
/// 所以本文件里的东西**测不到**——因此所有能测的逻辑都已经搬出去了：
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

import 'api/api_client.dart';
import 'api/diagnostics_api.dart';
import 'api/env_api.dart';
import 'api/models_api.dart';
import 'api/mods_api.dart';
import 'api/settings_models.dart';
import 'api/ws_client.dart';
import 'audio/audio_player.dart';
import 'chat/chat_controller.dart';
import 'chat/chat_session.dart';
import 'design/tokens.dart';
import 'app/app_shell.dart';
import 'app/app_shortcuts.dart';
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
import 'ui/error_banner.dart';
import 'ui/stage_corner_controls.dart';
import 'ui/theme.dart';

/// 本地显示偏好的存储键（localStorage）。
const String kDisplayPrefsKey = 'live2d-ai.display-prefs';

/// 聊天会话存档的存储键（localStorage）。
///
/// 与 `kDisplayPrefsKey` 分开：偏好是「小而常变」，会话是「大而不常变」。
/// 混在一个键里会让每次拖音量滑杆都重写整份聊天记录。
const String kChatSessionsKey = 'live2d-ai.chat-sessions';

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

/// 读取聊天会话存档：**任何异常都回落空 store**。
///
/// 与 `loadDisplayPrefs` 同一条纪律：一份被改坏 / 版本不符的存储不该让
/// 聊天区打不开——最坏情况是「历史没了」，而不是「页面崩了」。
ChatSessionStore loadChatSessions() {
  try {
    final String? raw = web.window.localStorage.getItem(kChatSessionsKey);
    if (raw == null || raw.isEmpty) return ChatSessionStore.empty();
    return ChatSessionStore.fromJson(jsonDecode(raw));
  } catch (_) {
    return ChatSessionStore.empty();
  }
}

/// 写入聊天会话存档（失败静默：无痕模式 / 配额满都会抛）。
///
/// **配额满不是假设**：localStorage 常见上限 5 MB，而 `ChatSessionStore`
/// 用 `maxSessions` × `kMaxMessagesPerSession` 双重夹持把体积压在配额内。
/// 即便如此，写失败也只丢「这次之后的新记录」，不该影响正在进行的对话。
void saveChatSessions(ChatSessionStore store) {
  try {
    web.window.localStorage.setItem(
      kChatSessionsKey,
      jsonEncode(store.toJson()),
    );
  } catch (_) {
    // 忽略：存储不可用不影响本次会话。
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

/// 让用户挑一张本地图片，读成 **dataURL**（组合根专有：需要 `package:web`）。
///
/// 用 `FileReader.readAsDataURL` 而不是自己拼 base64：它按文件的真实 MIME
/// 产出 `data:image/png;base64,…`，浏览器直接认。
///
/// **不做缩放**（刻意的）：裁剪要走 canvas，而 canvas 只能在浏览器里跑、
/// 本仓库的 `flutter test` 覆盖不到——本轮不引入无法回归的代码。
/// 代价是「大图不写盘」，由调用方按 [kStageImageMaxChars] 如实告知用户。
Future<String?> pickImageDataUrl() async {
  final web.HTMLInputElement input =
      web.document.createElement('input') as web.HTMLInputElement;
  input.type = 'file';
  input.accept = 'image/*';
  final Completer<String?> done = Completer<String?>();
  input.onchange = ((web.Event _) {
    final web.FileList? files = input.files;
    if (files == null || files.length == 0) {
      if (!done.isCompleted) done.complete(null);
      return;
    }
    final web.FileReader reader = web.FileReader();
    reader.onload = ((web.Event _) {
      if (done.isCompleted) return;
      final Object? result = reader.result;
      done.complete(result is String && result.isNotEmpty ? result : null);
    }).toJS;
    // 读失败当作「取消」：不弹错误，用户看到的就是没变化。
    reader.onerror = ((web.Event _) {
      if (!done.isCompleted) done.complete(null);
    }).toJS;
    reader.readAsDataURL(files.item(0)!);
  }).toJS;
  input.oncancel = ((web.Event _) {
    if (!done.isCompleted) done.complete(null);
  }).toJS;
  input.click();
  return done.future;
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

  /// 更新并**立即持久化**。值没变时直接返回（避免无意义的整树重建）。
  void _update(DisplayPrefs next) {
    if (next == _prefs) return;
    setState(() => _prefs = next);
    saveDisplayPrefs(next);
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
  final ValueChanged<DisplayPrefs> onPrefsChanged;

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
  String? _importMessage; bool _importFailed = false;
  /// 舞台背景图的提示（与角色卡导入分开：两条通道的失败原因完全不同）。
  String? _stageImageMessage; bool _stageImageFailed = false;
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

  // ── 动作子系统已于 2026-09-11 整条移除（用户裁决：LLM 无工具、只做对话）──
  //
  // 这里曾经是 `_onActionState`：把 WS 的 `action_state` 帧投影成 `ActionEvent`，
  // 再转成渲染面的 `action-state`（含 `cease` 帧要靠 `_performingAction` 游标
  // 找回动作名才能停）。随着 `live2d_perform_action` 工具、`action_state` 帧与
  // `/api/v1/commands` 端点一起删除——服务端不再发这一帧，前端也就没有任何
  // 动作通道可接了。触发与日志的归档见分支 `archive/action-trigger-p5`。

  /// 舞台角标三键 → 渲染面（渲染面自己算缩放，真值从 `stage-ack` 取）。
  Future<void> _zoom(String dir) async {
    await _stageKey.currentState?.sendStageZoom(dir);
    if (!mounted) return;
    setState(() => _stageScaleFromAck = _stageKey.currentState?.lastAck?.scale);
  }

  /// 把 [ShellRoot.prefs] 下发给渲染面（协议 v1 `sync`）。
  /// 把显示偏好下发给渲染面（协议 v1 `sync`）。
  ///
  /// [override] 是**即将生效**的偏好。为什么需要它：改偏好的流程是
  /// 「先通知宿主 → 宿主 `setState` → 本 State 的 `widget.prefs` 才更新」，
  /// 而这里是在 `setState` **之前**同步调用的——读 `widget.prefs` 会拿到
  /// **上一次的值**。切主题时这个时序错误的表现是「界面变白了、舞台还是黑的」：
  /// 发下去的是旧主题的 `stageColor`，而之后再没人重发。
  void _applyPrefs([DisplayPrefs? override]) {
    final DisplayPrefs prefs = override ?? widget.prefs;
    final AppPalette palette = AppPalette.of(prefs.theme);
    // **`stageColor` 不在这里发**（2026-09-11，P1-3）。
    //
    // 它过去由这里下发：切主题时一按就发终色 ⇒ 界面还在 200 ms 里插值，
    // 舞台已经跳到终色（「舞台先闪一下」）。现在舞台底由
    // [Live2DStage] 自己跑一条**同样 200 ms** 的动画逐帧下发，
    // 两边的观感才是一起变的。
    //
    // 仍然保留的是「首帧 / 重建 iframe 后的补发」：那条路在
    // `Live2DStage._attach()` 里（它用的是 `widget.stageColor`），
    // 所以这里不需要也不该再发一次。
    _stageKey.currentState?.sync(
      scale: prefs.scale,
      dark: palette.dark,
      lipSync: prefs.lipSync,
      idleEnabled: prefs.idleEnabled,
      mouthSensitivity: prefs.mouthSensitivity,
      // 「允许拖动与缩放」下发到渲染面的 `sync.clickEnabled`。
      clickEnabled: prefs.allowDragZoom,
      tier: prefs.tier,
    );
    // 背景图与纯色底是**叠放**关系（有图盖住底色），所以单独走 `stage-bg`。
    unawaited(
      _stageKey.currentState?.sendStageBg(prefs.stageImage) ??
          Future<void>.value(),
    );
  }

  /// 偏好变更：更新内存 + 落盘 + 立即下发。
  void _updatePrefs(DisplayPrefs next) {
    if (next == widget.prefs) return;
    widget.onPrefsChanged(next);
    // 静音与音量是纯本机输出设置，不经渲染面——直接作用于 AudioPlayer。
    _audio.muted = next.muted;
    _audio.volume = next.volume;
    // 传 `next` 而不是让它去读 `widget.prefs`（那还是旧值）。
    _applyPrefs(next);
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
  ///
  /// **优先按 `code` 分支**（2026-09-11）：码是契约、文案会改，拿显示文案做
  /// `contains` 判断迟早会漂移。最典型的是 `llm_upstream_401`——后端提示
  /// 「去后端进程环境设 `api_key_env`」，界面上能做的下一步就是**跳到 LLM
  /// 分区**看端点/密钥配置。只有码缺失（旧服务端 / 非链路错误）时才回退到
  /// 原来的文案启发式。
  List<ErrorAction> _errorActions(String? message, {String? code}) {
    if (message == null && code == null) return const <ErrorAction>[];
    ErrorAction goto(SettingsSection section, String label) => ErrorAction(
      label: label,
      // 走唯一入口：切分区会清掉一次性结果（P2-2）。
      onPressed: () => _gotoSection(section),
    );
    if (code != null) {
      if (code.startsWith('llm_') || code == 'no_supervisor') {
        return <ErrorAction>[goto(SettingsSection.llm, '去 LLM 设置')];
      }
      if (code.startsWith('tts_') || code.startsWith('decode_')) {
        return <ErrorAction>[goto(SettingsSection.tts, '去语音合成设置')];
      }
      if (code == 'busy') {
        return <ErrorAction>[
          ErrorAction(label: '打断并重发', onPressed: () => unawaited(_stop())),
        ];
      }
    }
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
      return <ErrorAction>[goto(SettingsSection.llm, '去 LLM 设置')];
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
      // 密钥真源状态（键名 + 是否已设置）。**永不包含值**。
      // 失败不连坐：`/env` 挂了不该让整个管理面板报错，所以单独兜底。
      EnvStatus env = _envStatus;
      try {
        env = await _envApi.list();
      } on ApiException {
        env = _envStatus;
      }
      if (!mounted) return;
      final Object? activeModel = status['active_model_id'];
      setState(() {
        _models = models;
        _mods = mods;
        _status = status;
        if (activeModel is String) _modelName = activeModel;
        _capabilities = caps;
        _envStatus = env;
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

  /// 保存一个密钥到 `.env`（`PUT /api/v1/env`），写完**立即生效**。
  ///
  /// 契约（rc.2）：`.env` 是唯一密钥真源；后端写完刷新快照 + 重建 LLM/TTS client，
  /// 所以这里不需要（也不该）提示「重启」或「重跑 ignite.sh」。
  ///
  /// 值**只在参数里出现**：不写进 state、不进日志、不回显。保存后重读一次
  /// `GET /api/v1/env` 只为刷新「已设置/未设置」这个布尔。
  Future<void> _saveEnvKey(String key, String value) async {
    await _envApi.write(key, value);
    if (!mounted) return;
    try {
      final EnvStatus env = await _envApi.list();
      if (!mounted) return;
      setState(() => _envStatus = env);
    } on ApiException {
      // 写成功了但读状态失败：不动现有状态（`EnvKeyField` 自己的提示仍然准确）。
    }
  }

  Future<void> _loadLogs() async {
    try {
      final List<LogLine> lines = await _diagApi.logs();
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
            // 文案要指**界面上的开关**：过去说「需服务端以 dev_mode 启动」，
            // 而那时界面上根本打不开它（开关被自己所在的分区藏起来了）。
            ? '日志需要先打开「开发模式」（设置 → 开发模式）'
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
        // 失败时把服务端给的 `code：message` 一起上屏（`messageWith`）——
        // 只显示「保存失败」等同于「没有错误代码」（用户 2026-09-11 报）。
        content: Text(outcome.messageWith(error: _settings.error)),
        backgroundColor: outcome.needsAttention
            ? appPaletteOf(context).warning
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
            ? 'ok · ${o.latencyMs ?? '?'} ms'
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

  /// 选一张舞台背景图。
  ///
  /// # 两条结果，都要如实说
  ///
  /// - 在 [kStageImageMaxChars] 以内 → 写进偏好（重开页面还在）。
  /// - 超限 → **只在本次会话生效**，不写盘。UI 必须说清「重开就没了」，
  ///   否则用户会以为它坏了。
  Future<void> _pickStageImage() async {
    final String? dataUrl = await pickImageDataUrl();
    if (dataUrl == null || !mounted) return;
    if (dataUrl.length <= kStageImageMaxChars) {
      _updatePrefs(widget.prefs.copyWith(stageImage: dataUrl));
      setState(() {
        _stageImageMessage = '已应用背景图（${_kb(dataUrl.length)}，会记住）';
        _stageImageFailed = false;
      });
      return;
    }
    // 超限：不经偏好，直接下发到渲染面（本次会话有效）。
    unawaited(_stageKey.currentState?.sendStageBg(dataUrl) ?? Future<void>.value());
    setState(() {
      _stageImageMessage =
          '图太大了（${_kb(dataUrl.length)}，上限 ${_kb(kStageImageMaxChars)}）——'
          '本次有效，**重新打开页面会丢失**。换一张小一点的图就能记住。';
      _stageImageFailed = true;
    });
  }

  /// 清掉背景图（回到纯色舞台）。
  void _clearStageImage() {
    _updatePrefs(widget.prefs.copyWith(clearStageImage: true));
    setState(() {
      _stageImageMessage = '已清除背景图，回到纯色舞台';
      _stageImageFailed = false;
    });
  }

  static String _kb(int chars) => '约 ${(chars / 1024).round()} KB';

  /// 激活模型：**改 registry + 让舞台真的换皮**，两件都做完才算成功。
  ///
  /// 契约（rc.2 冻结）：后端 `POST /models/{id}/activate` 只登记 + 回一个可 GET 的
  /// `model_url`，**不推送、不通知渲染面**；换模由这里 `sendSync(model:)` 发起，
  /// 并以渲染面 `loaded` 回执为准。**收条之前一律不说「已切换」**——
  /// 那正是「激活了但没换皮」最伤人的地方：用户看界面说成功了，舞台还是旧皮。
  Future<void> _activateModel(String id) async {
    setState(() {
      _busyId = id;
      _adminMessage = '正在切换模型…';
    });
    try {
      final ActivateResult r = await _modelsApi.activate(id);
      if (!mounted) return;
      if (r.modelUrl.isEmpty) {
        // 没给可加载地址 = 换不了皮，如实说，别报成功。
        setState(() {
          _busyId = null;
          _adminMessage = '已登记 ${r.activeId}，但服务端没有返回可加载的 model_url，舞台未切换';
        });
        await _loadAdmin();
        return;
      }
      // 先记住：retry / 重建 iframe 后仍加载这个模型。
      setState(() => _activeModelUrl = r.modelUrl);
      final bool swapped =
          await _stageKey.currentState?.swapModel(r.modelUrl) ?? false;
      if (!mounted) return;
      setState(() {
        _busyId = null;
        if (swapped) {
          _adminMessage = '已切换到 ${r.activeId}（舞台已确认生效）';
        } else if (r.requiresRestart) {
          _adminMessage = '已切换到 ${r.activeId}，但渲染面未确认；服务端称需重启生效';
        } else {
          _adminMessage =
              '已登记 ${r.activeId}，但渲染面未回执：舞台可能仍是上一个模型（可重试或看舞台错误层）';
        }
      });
      await _loadAdmin();
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busyId = null;
        _adminMessage = '激活失败：${e.message}';
      });
    }
  }

  /// 导入一个已在 `assets/models/<id>/` 下的模型目录。
  ///
  /// 后端不做文件上传（ZIP 上传是后置项，且此前被能力快照谎报为 supported）：
  /// 用户放好文件，这里登记 + 校验。**先刷新 registry 视图，再让用户点激活**——
  /// 不自动激活：激活会让舞台换皮，属于用户可见的动作，不该由一次「导入」代劳。
  Future<void> _importModel(String id) async {
    setState(() {
      _busyId = id;
      _adminMessage = '正在导入 $id…';
    });
    try {
      await _modelsApi.import(id);
      if (!mounted) return;
      setState(() {
        _busyId = null;
        _adminMessage = '已导入 $id，点「激活」即可换皮';
      });
      await _loadAdmin();
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busyId = null;
        _adminMessage = '导入失败：${e.message}';
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
        _adminMessage = '${enabled ? '已启用' : '已停用'} $id';
      });
      await _loadAdmin();
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busyId = null;
        _adminMessage = '操作失败：${e.message}';
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
        padding: EdgeInsets.symmetric(vertical: Space.s8),
        child: Center(child: CircularProgressIndicator()),
      );
    }
    final SettingsView? view = _settings.remote;
    if (view == null) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: Space.s7),
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
          activateMessage: _adminMessage,
          onActivate: _activateModel,
          onImport: _importModel,
          onReload: _loadAdmin,
        );
      case SettingsSection.llm:
        return LlmSection(
          controller: _settings,
          view: view,
          devMode: _devMode,
          envKey: _envStatus.forSection('llm'),
          envFile: _envStatus.envFile,
          onSaveKey: _saveEnvKey,
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
          envKey: _envStatus.forSection('tts'),
          envFile: _envStatus.envFile,
          onSaveKey: _saveEnvKey,
          onTest: _testTts,
          testResult: _ttsTest,
          testing: _ttsTesting,
        );
      case SettingsSection.appearance:
        return AppearanceSection(
          prefs: widget.prefs,
          onPrefsChanged: _updatePrefs,
          devMode: _devMode,
          onPickStageImage: () => unawaited(_pickStageImage()),
          onClearStageImage: _clearStageImage,
          stageImageMessage: _stageImageMessage,
          stageImageFailed: _stageImageFailed,
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
                padding: const EdgeInsets.symmetric(vertical: Space.s1),
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
  void _onSectionChanged(SettingsSection next) => _gotoSection(next);

  /// 切换设置分区（**唯一入口**）。
  ///
  /// # 为什么必须走这一个入口（2026-09-11，P2-2）
  ///
  /// 过去有两个地方各自 `setState(() => _section = …)`：分区 chip（这里）
  /// 与错误横幅的「去 LLM 设置 / 去语音合成设置」按钮。于是**内联结果**
  /// （`_llmTest` / `_ttsTest` / `_adminMessage` / `_importMessage` /
  /// `_stageImageMessage`）会一直挂在 State 上：用户测出「失败：401」，
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
    _importMessage = null;
    _importFailed = false;
    _stageImageMessage = null;
    _stageImageFailed = false;
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
          errorActions: _errorActions(
            _ui.errorMessage ?? _chat.error,
            // 顶部横幅（`_ui`）优先，所以它也优先提供码——两处都存了同一份。
            code: _ui.errorCode ?? _chat.errorCode,
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
            onHelp: () => unawaited(_showShortcutHelp()),
          ),
        );
      },
    );
  }
}
