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

import 'package:flutter/material.dart';
import 'package:web/web.dart' as web;

import 'api/api_client.dart';
import 'api/commands_api.dart';
import 'api/ws_client.dart';
import 'audio/audio_player.dart';
import 'chat/chat_controller.dart';
import 'app/app_shell.dart';
import 'live2d/live2d_stage.dart';
import 'settings/display_prefs.dart';
import 'settings/settings_sections.dart';
import 'state/ui_state_tracker.dart';
import 'ui/action_history_view.dart';
import 'ui/display_panel.dart';
import 'ui/error_banner.dart';
import 'ui/settings_scaffold.dart';
import 'ui/section_header.dart';
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
      _chat.actionLog.useCatalog(await _commandsApi.fetchCommands());
    } catch (_) {
      // 忽略：保持恒等解析器。
    }
  }

  @override
  void dispose() {
    unawaited(_levelSubscription?.cancel());
    unawaited(_statusSubscription?.cancel());
    unawaited(_eventSubscription?.cancel());
    _ui.dispose();
    _chat.dispose();
    _ws.dispose();
    _audio.dispose();
    _api.dispose();
    _commandsApi.dispose();
    _input.dispose();
    super.dispose();
  }

  /// 把 [_prefs] 下发给渲染面（协议 v1 `sync`）。
  void _applyPrefs() {
    _stageKey.currentState?.sync(
      scale: _prefs.scale,
      lipSync: _prefs.lipSync,
      idleEnabled: _prefs.idleEnabled,
      mouthSensitivity: _prefs.mouthSensitivity,
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
  }

  Future<void> _stop() async {
    await _chat.stop();
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

  /// 分区内容构建器。P3 只有「动作与互动」是真内容，其余如实说明「还没接上」。
  Widget _buildSection(BuildContext context, SettingsSection section) {
    switch (section) {
      case SettingsSection.actions:
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            const SectionHeader(
              title: '舞台与口型',
              description: '这些参数只影响本机显示，实时下发到渲染面。',
            ),
            DisplayPanel(
              prefs: _prefs,
              onChanged: _updatePrefs,
              showHeader: false,
            ),
            const Divider(),
            const SectionHeader(
              title: '动作记录',
              description: '谁触发的动作会记在这里——你点的、还是 AI 自己做的。',
            ),
            SizedBox(
              height: 320,
              child: ListenableBuilder(
                listenable: _chat.actionLog,
                builder: (BuildContext context, Widget? _) => ActionHistoryView(
                  entries: _chat.actionLog.entries,
                  current: _chat.actionLog.current,
                  droppedCount: _chat.actionLog.droppedCount,
                  onClear: _chat.actionLog.clear,
                ),
              ),
            ),
          ],
        );
      case SettingsSection.persona:
      case SettingsSection.models:
      case SettingsSection.llm:
      case SettingsSection.tts:
      case SettingsSection.mods:
      case SettingsSection.diagnostics:
      case SettingsSection.developer:
        return SettingsPendingPane(section: section);
    }
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      // 三处都并进来：聊天（消息/错误）、相位、音频解锁状态。
      listenable: Listenable.merge(<Listenable>[_chat, _ui, _audio.unlockState]),
      builder: (BuildContext context, Widget? _) {
        return AppShell(
          stage: Live2DStage(
            key: _stageKey,
            dark: true,
            // 就绪后补发显示偏好：首次挂载时桥还在 loading，
            // 以及在 retry 重建 iframe 之后（旧队列已随旧桥销毁）。
            onReady: _applyPrefs,
            onError: (String message) =>
                _ui.onBridgePhase(message.isNotEmpty),
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
          onSectionChanged: (SettingsSection next) =>
              setState(() => _section = next),
          sectionBuilder: _buildSection,
          devMode: _devMode,
        );
      },
    );
  }
}
