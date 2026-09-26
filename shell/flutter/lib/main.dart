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
import 'api/voice_api.dart';
import 'api/ws_client.dart';
import 'app/app_shell.dart';
import 'app/app_shortcuts.dart';
import 'app/browser_io.dart';
import 'app/shortcut_help_dialog.dart';
import 'audio/audio_player.dart';
import 'audio/stage_clock.dart';
import 'chat/chat_controller.dart';
import 'design/tokens.dart';
import 'live2d/action_scales_sync.dart';
import 'live2d/live2d_bridge.dart';
import 'live2d/live2d_stage.dart';
import 'live2d/render_events.dart';
import 'live2d/stage_cancel.dart';
import 'settings/display_prefs.dart';
import 'settings/preset_labels.dart';
import 'settings/sections/appearance_section.dart';
import 'settings/sections/dev_tools_section.dart';
import 'settings/sections/llm_section.dart';
import 'settings/sections/persona_section.dart';
import 'settings/sections/tts_section.dart';
import 'settings/settings_controller.dart';
import 'settings/settings_sections.dart';
import 'state/live_region.dart';
import 'state/ui_state_tracker.dart';
import 'voice/speech_recognizer.dart';
import 'voice/voice_listen_controller.dart';
import 'ui/confirm_discard_dialog.dart';
import 'ui/error_actions.dart';
import 'ui/restart_notice.dart';
import 'ui/stage_corner_controls.dart';
import 'ui/theme.dart';

part 'app/shell_admin.dart';
part 'app/shell_chat.dart';
part 'app/shell_prefs.dart';
part 'app/shell_settings.dart';

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
  StreamSubscription<int?>? _sentenceCueSubscription;
  StreamSubscription<StageClockSample>? _stageClockSubscription;

  /// 渲染面事件 → **喂导演的日志文本**（协议 §7.3）。
  ///
  /// 四条 ack + `segment-ended` 由 `Live2DStage.onRenderEvent` 汇进来；前端
  /// **不据此维护镜像状态**（唯一例外是调试面板的显示快照，同样只由 ack 驱动）。
  final DirectorEventLog _directorLog = DirectorEventLog();

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
  SettingsSection _section = SettingsSection.appearance;

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
  /// 由三类动作写入：Mod 启停（`_toggleMod`）、Mod 配置保存（`_saveModConfig`）、
  /// 以及各 Mod 产品面板自己的动作（导入角色卡 / 导入或清空记忆 / 改语音闸）。
  /// 文案与处置入口的唯一来源是 `ui/restart_notice.dart`——不要在调用点各写一份。
  String? _modRestartNotice;
  String? _llmTest;   bool _llmTesting = false;
  String? _ttsTest;   bool _ttsTesting = false;
  /// 舞台背景图的提示（选图与其它通道的失败原因完全不同）。
  String? _stageImageMessage; bool _stageImageFailed = false;

  /// 壳背景图的提示（2026-09-14，rc.5）：与舞台那条**分开**，
  /// 否则在壳那行选完图会在舞台那行冒出一句话。
  String? _shellImageMessage; bool _shellImageFailed = false;
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
    // 常态语音检测：读 voice-input Mod 的 wake_phrase（缺省「小可爱」），
    // 命中后把原文交给 /api/v1/voice/transcript（服务端剥词，一份真相）。
    _voiceApi = VoiceApi();
    _voiceListen = VoiceListenController(
      recognizer: createSpeechRecognizer(),
      // B1（L1，2026-09-16）：服务端受理后**立刻**把用户句上屏并开一轮。
      // 以前这里直接透传 `_voiceApi.sendTranscript`：链路（turn_prompt / 回答）
      // 起来了，聊天区却没有用户那句话——用户以为「识别了但没发出去」。
      send: (String text, {bool ptt = false}) async {
        final VoiceTranscriptResult result = await _voiceApi.sendTranscript(
          text,
          ptt: ptt,
        );
        if (result.ok) {
          // 语音注入也是「新消息」：先同步取消上一轮编排（V10 §9.3），再上屏。
          _cancelStageForTurnBoundary('voice');
          // 上屏用服务端剥词 / 归一化后的正文（与真正喂给 LLM 的**同一份**）。
          _chat.acceptInjectedUserTurn(
            result.text.isNotEmpty ? result.text : text,
          );
        }
        return result;
      },
      loadWakePhrase: _loadWakePhrase,
      // Mod 未启用 → 先给红字（不要等 403 回来）。
      loadModEnabled: _voiceInputModEnabled,
      // busy 不排队：把识别到的正文落回输入框，让用户改字重发（P0-4）。
      onBusyResult: _onVoiceBusyResult,
    );
    _settings = SettingsController(api: _api);
    // 动作幅度的**即时预览**（2026-09-16 修）：三滑条改的是设置草稿，
    // 草稿一变就防抖下发渲染面——不必先点「保存」。
    // 监听放在这里（而不是 ListenableBuilder 的 builder 里）的理由：
    // builder 只在「有东西重建」时跑，而拖动滑条恰好**只改草稿**；
    // 监听器与「保存 / 放弃 / 重新加载」共用同一条路（那三条都会 notify）。
    _settings.addListener(_scheduleActionScalesSync);
    _ws = WsClient();
    _audio = AudioPlayer();
    // 静音与音量是纯本机输出设置（不经渲染面）：**默认出声**。
    _audio.muted = widget.prefs.muted;
    _audio.volume = widget.prefs.volume;
    // 停止 / 新消息的取消纪律（V7 §6.6 / V10 §9.3）：四个出口都在本 State 上。
    // 顺序由 [StageCancellation] 钉住（纯逻辑、VM 可测），这里只提供实体。
    _stageCancellation = StageCancellation(
      clearCuePlan: (String reason) =>
          _directorCues.replace(const <ActionCue>[]),
      revokeStage: (String reason) => unawaited(
        _stageKey.currentState?.applyPreset('none', source: reason),
      ),
      dropPendingAudio: (String reason) => _audio.interrupt(),
      returnToBaseline: _requestSessionBaseline,
    );
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

    // 导演 cue（P1-3）：一句音频**开始播放**时按 seq apply（动作与声音同拍）。
    _sentenceCueSubscription = _audio.sentenceStarts.listen(
      _applyDirectorCueForSeq,
    );

    // stage-clock（协议 §6.5 / O13）：**段内播放位置 30ms** 下发渲染面，让编排
    // 锚在音频时钟（V7 §6.1）而不是 performance.now()。节奏由 AudioPlayer 的
    // 30ms ticker 保证；停止后它只发一次 playing:false，不补帧。
    _stageClockSubscription = _audio.stageClock.listen((StageClockSample s) {
      _stageKey.currentState?.sendStageClock(
        seg: s.seg,
        posMs: s.posMs,
        playing: s.playing,
      );
    });

    // 相位跟踪器自己订阅 WS（只读消费，与 ChatController 互不干扰）。
    _statusSubscription = _ws.statuses.listen(_ui.onWsStatus);
    _eventSubscription = _ws.events.listen((WsEvent event) {
      final bool wasMuted = _serverMuted;
      _ui.consume(event);
      if (event is AudioEvent && event.muted != wasMuted) {
        setState(() => _serverMuted = event.muted);
      }
      // P0-4：角色播报期间**暂停听**（自己的声音 / 环境人声会误触发），
      // 播报结束自动恢复常驻；PTT 进行中不抢。
      if (event is RuntimeStatusEvent) {
        if (event.event == 'voice_started') {
          unawaited(_voiceListen.suspendForPlayback());
        } else if (event.event == 'voice_ended') {
          unawaited(_voiceListen.resumeAfterPlayback());
        }
      }
      // 导演按句 cue（P1-3，2026-09-16）：整体替换当前计划；缺省忽略 = 兼容。
      // 空 cues = 清空计划 = **本轮不动**（不归零；归零只走 preset_id='none'）。
      // **唯一驱动通道**——阶段3 起不再有 text_delta / text_fallback 触发的
      // 「拉状态面 latest.preset_id」分支（通道 B 退役，见 D10–D13）。
      if (event is ActionCueEvent) {
        _directorEpoch = event.epoch;
        _directorCues.replace(event.cues);
      }
    });

    _ws.connect();
    unawaited(_loadAppStatus());
    // 启动就取一次 voice-input 的启停：主界面「听」按钮旁要能**先**给红字
    // （而不是等用户说完才由 403 回来说）。
    unawaited(_refreshVoiceModState());
  }

  /// 音频开始播放时按 sentence_seq 应用导演 cue（一次性；没 cue 什么都不做）。
  ///
  /// 锚点选**音频开始**（不是句子提交时刻）：慢 TTS 下前者才与声音同拍；
  /// 迟到 / 缺 cue 一律安静丢弃（导演是附加表演能力，不往聊天链路抛错误）。
  ///
  /// **唯一驱动通道 = WS `action_cue`（阶段3 D10–D13）**：不再拉
  /// director 的只读运行态（通道 B 已退役），`state_json.latest`
  /// 仅供 director 面板只读展示。撤销语义已并入 cue 层——`preset_id == 'none'`
  /// 照常下发（渲染面把 `none` 翻成两槽 `Revoke`）；`preset_id` 为空串仍是
  /// 「本轮不动」。整表替换 + 取用即移除由 [`DirectorCuePlan`] 负责，同 seq
  /// 重复帧只会应用一次。
  void _applyDirectorCueForSeq(int? seq) {
    unawaited(
      _directorCues.applyForSeq(seq, (ActionCue cue) {
        // v1 三族字段原样透传（编排者冻结的集成细节）：消息仍是既有 `preset`，
        // 旧键一个不动、只增可选键；缺 field 时语义与今天逐字相同。
        return _stageKey.currentState?.applyPreset(
              cue.presetId,
              source: 'director',
              intensity: cue.intensity.toDouble(),
              ttlMs: cue.ttlMs.toDouble(),
              field: cue.field,
              x: cue.x,
              y: cue.y,
              z: cue.z,
              hold: cue.hold,
              at: cue.at,
              seq: cue.seq,
              epoch: _directorEpoch,
              sentenceSeq: cue.sentenceSeq,
            ) ??
            Future<void>.value();
      }),
    );
  }

  /// 停止键：**先**同步取消编排，再走既有 stop（V7 §6.6 / V10 §9.3）。
  ///
  /// 顺序不能反：等到 stop 的服务端回执（new_epoch）才清动作，就会在没有声音时
  /// 继续把旧 cue 演完——那正是「追着播 / 补帧」。
  Future<void> _stopWithCancellation() async {
    _cancelStageForTurnBoundary('stop');
    await _stop();
  }

  /// 发送（含重试 / 语音注入）：新消息同样先取消上一轮编排。
  Future<void> _sendWithCancellation() async {
    _cancelStageForTurnBoundary('new-message');
    await _send();
  }

  /// 停止 / 新消息的统一取消入口：清动作 + 表情 + TTS 待播 + 回 baseline。
  ///
  /// **同步取消、不补帧**：调用 [StageCancellation.cancel] 即刻生效；计划的
  /// 整表替换 + 音频队列清空保证之后不会重放任何旧 cue（V7 §6.6 / V10）。
  void _cancelStageForTurnBoundary(String reason) {
    _stageCancellation.cancel(reason);
  }

  /// 回该会话 baseline（V10 §9.3）。
  ///
  /// **host 侧接口未冻结**：4e 在 `session_scope` 上按会话回落 baseline，并随
  /// stop / new-message 的**同一个**请求生效；前端不另发请求、也不新造第二套
  /// 会话表。这里保留调用点（[StageCancellation.returnToBaseline] 的第 ④ 步），
  /// 端点冻结后在此接线。见报告未决。
  void _requestSessionBaseline(String reason) {
    return;
  }

  /// 主链忙时，把识别到的正文**落回输入框**（不排队、不静默丢弃）。
  ///
  /// 不 `setState`：`TextEditingController` 自己会通知输入框重建，这里只改值。
  void _onVoiceBusyResult(String text) {
    if (!mounted || text.trim().isEmpty) return;
    _input.value = TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
    );
  }

  /// voice-input Mod 是否启用（「听」按钮预检；读失败 → `true` 不误拦）。
  ///
  /// 缓存命中直接返回；首次（或用户在设置里改过启停后清缓存）走一次
  /// `GET /api/v1/mods`。**读不到不拦**——真发出去时服务端仍会如实回 403。
  Future<bool> _voiceInputModEnabled() async {
    final bool? cached = _voiceInputEnabled;
    if (cached != null) return cached;
    try {
      final List<ModInfo> mods = await _modsApi.list();
      if (mounted) {
        _mods = mods;
        _voiceInputEnabled = _enabledOf(mods, 'voice-input');
        _refresh();
      }
      return _enabledOf(mods, 'voice-input');
    } catch (_) {
      return true;
    }
  }

  /// 读当前生效的唤醒词：voice-input Mod 的 `config.wake_phrase`。
  ///
  /// 键缺失 / Mod 不在册 / 读失败 → 产品缺省「小可爱」（与 Rust 侧
  /// `gate::DEFAULT_WAKE_PHRASE` 逐字一致）。**显式空**（键存在且为空）
  /// 也回落缺省：服务端此时会拒一切转写，按钮仍能把这条错误如实显示出来；
  /// 本地不需要复刻「空 = 总闸关」的第二套判定。
  Future<String> _loadWakePhrase() async {
    final List<ModInfo> mods = await _modsApi.list();
    for (final ModInfo m in mods) {
      if (m.id != 'voice-input') continue;
      final Object? raw = m.config['wake_phrase'];
      if (raw is String && raw.trim().isNotEmpty) return raw.trim();
    }
    return kDefaultWakePhrase;
  }

  @override
  void dispose() {
    unawaited(_levelSubscription?.cancel());
    unawaited(_statusSubscription?.cancel());
    unawaited(_eventSubscription?.cancel());
    unawaited(_sentenceCueSubscription?.cancel());
    unawaited(_stageClockSubscription?.cancel());
    _actionScalesSyncer.dispose();
    _settings.removeListener(_scheduleActionScalesSync);
    _ui.dispose();
    _live.dispose();
    _settings.dispose();
    _modelsApi.dispose();
    _envApi.dispose();
    _modsApi.dispose();
    _diagApi.dispose();
    _voiceListen.dispose();
    _voiceApi.dispose();
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

  /// 记录一次 Mod 变更：挂常驻提示 + 弹一条带入口的 SnackBar。
  ///
  /// **为什么两处都要**：SnackBar 会消失（用户可能正好没看屏幕），常驻提示
  /// （Mod 分区顶部 + 聊天区顶部）才是「我重启了没有」的备忘。两者文案同源
  /// （`modRestartNoticeText` / `modRestartSnackText`），不会互相矛盾。
  ///
  /// 刻意**不**在 `_clearTransientResults` 里清掉它：切分区不是「已经重启」。
  void _notifyModChanged(String what) {
    setState(() => _modRestartNotice = modRestartNoticeText(what));
    final ScaffoldMessengerState? messenger = ScaffoldMessenger.maybeOf(context);
    messenger
      ?..clearSnackBars()
      ..showSnackBar(
        SnackBar(
          content: Text(modRestartSnackText(what)),
          duration: const Duration(seconds: 8),
        ),
      );
  }

  /// 用户关掉常驻提示（只有他能判断「已经重启过了」）。
  void _dismissModRestart() {
    if (_modRestartNotice == null) return;
    setState(() => _modRestartNotice = null);
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
            stageImage: widget.prefs.stageImage,
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
          // 壳全局背景：同步开时就是舞台那张图（一份真相），关时用壳自己的。
          shellImage: widget.prefs.effectiveShellImage,
          messages: _chat.messages,
          input: _input,
          onSend: () => unawaited(_sendWithCancellation()),
          onStop: () => unawaited(_stopWithCancellation()),
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
          // 常态语音检测：聊天主界面常驻的「听」按钮（唤醒词缺省「小可爱」）。
          listenSupported: _voiceListen.supported,
          listening: _voiceListen.listening,
          listenStatus: _voiceListen.statusLine,
          listenError: _voiceListen.error,
          onToggleListen: () => unawaited(_voiceListen.toggle()),
          // P0-4：一个按钮三态——点按 = 常驻开/关，按住 = PTT（松手提交）。
          pttActive: _voiceListen.pttActive,
          onPressStart: () => unawaited(_voiceListen.pressStart()),
          onPressRelease: () => unawaited(_voiceListen.pressRelease()),
          // 诚实性：Web Speech 是云端识别、需联网、音频会出本机。
          listenNote: _voiceListen.supported ? kVoiceWebSpeechNote : null,
          // B1（L1）：Mod 未启用时**常驻红字**，不要等用户说完才由 403 回来说。
          listenBlockedReason: _voiceInputEnabled == false
              ? kVoiceModDisabledMessage
              : null,
          // L1 基座：Mod 变更后的统一提示（聊天区顶部常驻，可关）。
          modRestartNotice: _modRestartNotice,
          onDismissModRestart: _dismissModRestart,
          error: _ui.errorMessage ?? _chat.error,
          errorActions: errorActionsFor(
            _ui.errorMessage ?? _chat.error,
            // 顶部横幅（`_ui`）优先，所以它也优先提供码——两处都存了同一份。
            code: _ui.errorCode ?? _chat.errorCode,
            onGoto: _gotoSection,
            onStop: () => unawaited(_stopWithCancellation()),
            onSend: () => unawaited(_sendWithCancellation()),
          ),
          onDismissError: () {
            _ui.clearError();
            _chat.clearError();
          },
          onRetryLast: () => unawaited(_sendWithCancellation()),
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
