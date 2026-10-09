/// 设置草稿控制器：**改动集**模型 + dirty + 保存/放弃 + 三处拦截。
///
/// 规格 §4.3。**纯逻辑、无 web 依赖**（`package:web` 的 `onBeforeUnload`
/// 留在组合根），所以「只改一个字段会不会清空同段其它字段」这类致命问题
/// 可以在 VM 上单测。
///
/// # 为什么草稿是「改动集」而不是「完整副本」
///
/// 服务端三态补丁下，「把整份 view 回传」会把用户**没碰过**的字段也变成显式
/// 赋值（并可能把 `has_api_key` 这类只读派生字段当成可写字段发过去）。
/// 改动集 + [Tri] 三态是唯一能安全表达「我只改了 `tts.voice`」的形状。
///
/// # 三个必须做对的点
///
/// 1. **dirty 与键序无关**：`Tri.keep()` 不计入 dirty（它序列化后不产生键）。
/// 2. **保存用服务端回填的 view**，不用本地草稿——PATCH 可能被服务端归一化
///    （如 URL 去尾斜杠），本地草稿留着就会显示一个服务端并不认可的值。
/// 3. **保存失败不清空草稿**：失败后用户还得改，清掉等于让他重打一遍。
library;

import 'package:flutter/foundation.dart';

import '../api/api_client.dart';
import '../api/settings_models.dart';

/// 浮点草稿与远端原值是否等价（容差 1e-6）。
bool _almostEqual(double? a, double b) {
  if (a == null) return false;
  return (a - b).abs() < 1e-6;
}

/// 一段字段的编辑状态（改动集里的一项）。
///
/// 用泛型参数而不是 `Object?`：取值要进 JSON，`Object?` 会让
/// `Tri.set(1)` 与 `Tri.set('1')` 看起来一样。
typedef DraftField<T> = Tri<T>;

/// 设置草稿（**可变**，用户每敲一下改一次）。
///
/// 只记录**用户真的动过**的字段。`null` 与 [TriKeep] 都表示「没动」，
/// 序列化时都不产生键——[dirty] 因此天然与键序无关。
class SettingsDraft {
  SettingsDraft();

  // ── LLM ──
  String? llmBaseUrl;
  String? llmModel;
  DraftField<int>? llmMaxTokens;

  /// 展示思考总闸（三态）。`null` = 没动；服务端缺省 false。
  DraftField<bool>? llmShowReasoning;

  // ── TTS ──
  /// 语音来源（`local` / `cloud`，2026-10-09）。null = 没动。
  String? ttsMode;
  String? ttsBaseUrl;
  String? ttsModel;
  String? ttsVoice;

  /// 密钥**变量名**的草稿（只有「切到云端且本机还没绑定」时会被设一次）。
  Tri<String>? ttsApiKeyEnv;
  int? ttsSampleRate;
  int? ttsChannels;

  // ── persona（M5.1：只剩系统提示词与历史轮数；卡字段已迁出主链） ──
  String? personaSystemPrompt;
  int? personaMaxHistoryPairs;

  // ── action（动作幅度倍率，2026-09-16；产品设置，保存后生效） ──
  double? actionHeadScale;
  double? actionBodyScale;
  double? actionExpressionScale;

  // ── 顶层 ──
  bool? devMode;

  /// 是否什么都没改。
  ///
  /// 判据是「编译出来的补丁是否为空」而**不是**「有没有字段被 set」——
  /// 这样「把值改回原值」也会被正确地当成没改（由调用方先比原值，
  /// 见 [SettingsController.setField] 的说明）。
  bool get isEmpty => toPatch().isEmpty;

  /// 编译成三态补丁。
  ///
  /// **空段不写键**：`{"llm":{}}` 在服务端是 no-op，但写出来会让
  /// 「什么都没改」也发一次网络请求。
  SettingsPatch toPatch() {
    final LlmSettingsPatch? llm = _llmPatch();
    final TtsSettingsPatch? tts = _ttsPatch();
    final PersonaSettingsPatch? persona = _personaPatch();
    final ActionSettingsPatch? action = _actionPatch();
    return SettingsPatch(
      llm: llm,
      tts: tts,
      persona: persona,
      action: action,
      devMode: devMode == null ? null : Tri.set(devMode!),
    );
  }

  LlmSettingsPatch? _llmPatch() {
    // P6：不再有 `apiKeyEnv` 实参——变量名编辑入口已从界面移除，前端类型上也
    // 编不出这个键（服务端仍接受它，走 curl / 手改 live2d-ai.toml）。
    final LlmSettingsPatch patch = LlmSettingsPatch(
      baseUrl: _tri(llmBaseUrl),
      model: _tri(llmModel),
      maxTokens: llmMaxTokens,
      showReasoning: llmShowReasoning,
    );
    return patch.isEmpty ? null : patch;
  }

  TtsSettingsPatch? _ttsPatch() {
    // model 是可选字段：草稿里显式空串 = **清除**（`null`），不是「设成空串」——
    // 后者会让请求体带一个空的 model 字段。
    final String? model = ttsModel;
    final Tri<String>? modelTri = model == null
        ? null
        : (model.trim().isEmpty ? Tri.clear<String>() : Tri.set(model));
    final TtsSettingsPatch patch = TtsSettingsPatch(
      mode: ttsMode,
      baseUrl: _tri(ttsBaseUrl),
      model: modelTri,
      voice: _tri(ttsVoice),
      apiKeyEnv: ttsApiKeyEnv,
      sampleRate: ttsSampleRate == null ? null : Tri.set(ttsSampleRate!),
      channels: ttsChannels == null ? null : Tri.set(ttsChannels!),
    );
    return patch.isEmpty ? null : patch;
  }

  PersonaSettingsPatch? _personaPatch() {
    final PersonaSettingsPatch patch = PersonaSettingsPatch(
      systemPrompt: _tri(personaSystemPrompt),
      maxHistoryPairs: personaMaxHistoryPairs == null
          ? null
          : Tri.set(personaMaxHistoryPairs!),
    );
    return patch.isEmpty ? null : patch;
  }

  ActionSettingsPatch? _actionPatch() {
    final ActionSettingsPatch patch = ActionSettingsPatch(
      headScale: actionHeadScale == null ? null : Tri.set(actionHeadScale!),
      bodyScale: actionBodyScale == null ? null : Tri.set(actionBodyScale!),
      expressionScale: actionExpressionScale == null
          ? null
          : Tri.set(actionExpressionScale!),
    );
    return patch.isEmpty ? null : patch;
  }

  /// `null` → 没动；否则 `Tri.set`（清空由调用方显式传空串走正常路径）。
  static Tri<String>? _tri(String? value) =>
      value == null ? null : Tri.set(value);

  void clear() {
    llmBaseUrl = null;
    llmModel = null;
    llmMaxTokens = null;
    llmShowReasoning = null;
    ttsMode = null;
    ttsBaseUrl = null;
    ttsModel = null;
    ttsVoice = null;
    ttsApiKeyEnv = null;
    ttsSampleRate = null;
    ttsChannels = null;
    personaSystemPrompt = null;
    personaMaxHistoryPairs = null;
    actionHeadScale = null;
    actionBodyScale = null;
    actionExpressionScale = null;
    devMode = null;
  }
}

/// 保存结果（供 UI 选择文案，规格 §4.4）。
enum SaveOutcome {
  /// 真的写了盘且已生效。
  savedApplied,

  /// 真的写了盘，但**需要重启**。
  savedRestartRequired,

  /// 真的写了盘，reload 排队中。
  savedQueued,

  /// 真的写了盘，但没有 supervisor（下次启动生效）。
  savedNoSupervisor,

  /// 服务端认为**没有变化**（`persisted: false`）。
  noChange,

  /// 失败（网络 / 400 / 超时）。
  failed,
}

extension SaveOutcomeMessage on SaveOutcome {
  /// 给用户看的一句话。**按 `apply_status` 分流**——不区分就会
  /// 「提示已热重载但实际 503」。
  String get message => switch (this) {
    // 2026-10-09：保存已经是**一步**——写盘 + 热重载 / 重启 18080 上的服务，
    // 然后页面自动重新加载。文案不再停在「请自行重新点火」。
    SaveOutcome.savedApplied => '已保存并生效，页面将重新加载',
    SaveOutcome.savedRestartRequired => '已保存；服务端正在重启，页面将重新加载',
    SaveOutcome.savedQueued => '已保存，正在生效，页面将重新加载',
    SaveOutcome.savedNoSupervisor => '已保存，配置将在下次启动后生效',
    SaveOutcome.noChange => '没有变化',
    SaveOutcome.failed => '保存失败',
  };

  /// 上屏文案：失败时**带上服务端给的具体错误**（码 + 说明）。
  ///
  /// 2026-09-11 修：`SnackBar` 过去只显示 [message]，于是任何失败都塌缩成一句
  /// 「保存失败」——用户的原话是「后端出错无具体错误代码」（PATCH 的 400/500
  /// 响应体里其实一直带着 `code` + `message`，只是没人把它送上去）。
  ///
  /// `error` 取自 `SettingsController.error`（`ApiException.toString()`，
  /// 形如 `HTTP 400 invalid_payload：base_url 非法`）。非失败结局忽略它。
  String messageWith({String? error}) {
    if (this != SaveOutcome.failed) return message;
    final String detail = error?.trim() ?? '';
    return detail.isEmpty ? message : '$message：$detail';
  }

  /// 是否是「成功」类（绿）；`noChange` 与 `failed` 都不是。
  bool get isSuccess =>
      this == SaveOutcome.savedApplied ||
      this == SaveOutcome.savedRestartRequired ||
      this == SaveOutcome.savedQueued ||
      this == SaveOutcome.savedNoSupervisor;

  /// 是否该用 `warning` 色（写盘成功但没生效）。
  bool get needsAttention =>
      this == SaveOutcome.savedRestartRequired ||
      this == SaveOutcome.savedNoSupervisor ||
      this == SaveOutcome.failed;
}

class SettingsController extends ChangeNotifier {
  SettingsController({required this.api});

  final ApiClient api;

  SettingsView? _remote;
  SettingsDraft _draft = SettingsDraft();

  /// 是否正在加载 / 保存。
  bool _loading = false;
  bool _saving = false;

  /// 最近一次保存的结局（UI 据此弹提示）。
  SaveOutcome? _lastOutcome;

  /// 最近一次错误（加载或保存失败）。
  String? _error;

  /// 上次 GET/PATCH 成功后的**服务端真相**。
  SettingsView? get remote => _remote;

  /// 是否已加载过（决定要不要显示骨架）。
  bool get loaded => _remote != null;

  bool get loading => _loading;
  bool get saving => _saving;
  SaveOutcome? get lastOutcome => _lastOutcome;
  String? get error => _error;

  /// 有没有未保存的改动。
  bool get dirty => loaded && !_draft.isEmpty;

  /// 当前草稿（只读用途；改它请走各 `set*` 方法）。
  SettingsDraft get draft => _draft;

  /// 加载（或重新加载）服务端设置。**会丢弃草稿**——调用方要先问过用户。
  Future<void> load() async {
    _loading = true;
    _error = null;
    notifyListeners();
    try {
      _remote = await api.fetchSettings();
      _draft = SettingsDraft();
      _lastOutcome = null;
    } on ApiException catch (e) {
      _error = e.toString();
    } catch (e) {
      _error = '加载设置失败：$e';
    } finally {
      _loading = false;
      notifyListeners();
    }
  }

  /// 保存。失败时**保留草稿**。
  Future<SaveOutcome> save() async {
    final SettingsPatch patch = _draft.toPatch();
    if (patch.isEmpty) {
      _lastOutcome = SaveOutcome.noChange;
      _draft = SettingsDraft();
      notifyListeners();
      return SaveOutcome.noChange;
    }
    _saving = true;
    _error = null;
    _lastOutcome = null;
    notifyListeners();
    try {
      final SettingsPatchResult result = await api.patchSettings(patch);
      // 用**服务端回填的** view：它可能被服务端归一化（URL 去尾斜杠等）。
      _remote = result.settings;
      _draft = SettingsDraft();
      final SaveOutcome outcome = !result.persisted
          ? SaveOutcome.noChange
          : switch (result.applyStatus) {
              ApplyStatus.applied => SaveOutcome.savedApplied,
              ApplyStatus.restartRequired => SaveOutcome.savedRestartRequired,
              ApplyStatus.queued => SaveOutcome.savedQueued,
              ApplyStatus.noSupervisor => SaveOutcome.savedNoSupervisor,
              // 旧服务端 / 未识别：**不谎报已生效**，按「需重启」处理更保守。
              ApplyStatus.unknown => SaveOutcome.savedRestartRequired,
            };
      _lastOutcome = outcome;
      return outcome;
    } on ApiException catch (e) {
      _error = e.toString();
      _lastOutcome = SaveOutcome.failed;
      return SaveOutcome.failed;
    } catch (e) {
      _error = '保存失败：$e';
      _lastOutcome = SaveOutcome.failed;
      return SaveOutcome.failed;
    } finally {
      _saving = false;
      // 草稿只在成功时清空（上面已清）；失败时上面没赋值，仍保留。
      notifyListeners();
    }
  }

  /// 放弃改动，回到服务端真相。
  void discard() {
    if (!dirty) return;
    _draft = SettingsDraft();
    _lastOutcome = null;
    notifyListeners();
  }

  /// 三处拦截的统一入口：**有未保存改动时先问用户**。
  ///
  /// 返回 `true` = 可以离开（用户选择放弃或本来就没有改动）。
  Future<bool> confirmLeave(Future<bool> Function() ask) async {
    if (!dirty) return true;
    final bool leave = await ask();
    if (leave) discard();
    return leave;
  }

  void clearError() {
    if (_error == null && _lastOutcome == null) return;
    _error = null;
    _lastOutcome = null;
    notifyListeners();
  }

  // ── 字段写入 ──────────────────────────────────────────────────────

  /// 改草稿（各 pane 的唯一入口）。
  ///
  /// 写完立刻 [pruneAgainstRemote]：**与远端原值相同就撤回该字段的改动**，
  /// 而不是记一条「改回原值」的补丁。这样 [dirty] 与「用户是否真的改了值」
  /// 一致，而不是「用户是否碰过控件」——否则「点开又改回去」会永久显示
  /// 「未保存」，用户只能靠保存一次来消掉这行提示。
  void edit(void Function(SettingsDraft draft) change) {
    change(_draft);
    pruneAgainstRemote(notify: false);
    notifyListeners();
  }

  /// 把「与远端相同」的字段撤回（等价于没改）。
  ///
  /// 公开是为了让测试能直接构造草稿后调用；[edit] 已经会自动调它。
  void pruneAgainstRemote({bool notify = true}) {
    final SettingsView? remote = _remote;
    if (remote == null) return;
    if (_draft.llmBaseUrl == remote.llm.baseUrl) _draft.llmBaseUrl = null;
    if (_draft.llmModel == remote.llm.model) _draft.llmModel = null;
    if (_draft.llmMaxTokens case TriSet<int>(:final int value)
        when value == remote.llm.maxTokens) {
      _draft.llmMaxTokens = null;
    }
    if (_draft.llmShowReasoning case TriSet<bool>(:final bool value)
        when value == remote.llm.showReasoning) {
      _draft.llmShowReasoning = null;
    }
    if (_draft.ttsMode == remote.tts.mode) _draft.ttsMode = null;
    if (_draft.ttsBaseUrl == remote.tts.baseUrl) _draft.ttsBaseUrl = null;
    if (_draft.ttsModel == remote.tts.model) _draft.ttsModel = null;
    if (_draft.ttsVoice == remote.tts.voice) _draft.ttsVoice = null;
    if (_draft.ttsSampleRate == remote.tts.sampleRate) {
      _draft.ttsSampleRate = null;
    }
    if (_draft.ttsChannels == remote.tts.channels) _draft.ttsChannels = null;
    if (_draft.personaSystemPrompt == remote.persona.systemPrompt) {
      _draft.personaSystemPrompt = null;
    }
    if (_draft.personaMaxHistoryPairs == remote.persona.maxHistoryPairs) {
      _draft.personaMaxHistoryPairs = null;
    }
    // 幅度倍率是浮点：滑条整格移动时用极小容差判断「改回原值」，
    // 避免 0.75 与 0.7500000001 之间永远显示「未保存」。
    if (_almostEqual(_draft.actionHeadScale, remote.action.headScale)) {
      _draft.actionHeadScale = null;
    }
    if (_almostEqual(_draft.actionBodyScale, remote.action.bodyScale)) {
      _draft.actionBodyScale = null;
    }
    if (_almostEqual(
      _draft.actionExpressionScale,
      remote.action.expressionScale,
    )) {
      _draft.actionExpressionScale = null;
    }
    if (_draft.devMode == remote.devMode) _draft.devMode = null;
    if (notify) notifyListeners();
  }
}
