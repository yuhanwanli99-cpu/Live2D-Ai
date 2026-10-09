part of 'settings_models.dart';

/// LLM 段补丁（字段级三态）。
class LlmSettingsPatch {
  const LlmSettingsPatch({
    this.baseUrl,
    this.model,
    this.maxTokens,
    this.showReasoning,
  });

  final Tri<String>? baseUrl;
  final Tri<String>? model;

  /// 输出 token 上限（三态）。
  ///
  /// **`Tri.set(0)` 与 `Tri.clear()` 是两件不同的事**，不能合并：
  /// - `Tri.set(0)` → JSON `max_tokens: 0` → 不限制；
  /// - `Tri.clear()` → JSON `max_tokens: null` → 清除，回落服务端默认；
  /// - 字段留 `null` → JSON 里**不出现**该键 → 保持原值。
  final Tri<int>? maxTokens;

  /// **展示思考总闸**（三态）：`Tri.set(true|false)` 显式设值；
  /// `Tri.clear()` → `show_reasoning: null` → 回落服务端缺省（false）。
  final Tri<bool>? showReasoning;

  Map<String, Object?> toJson() {
    final Map<String, Object?> out = <String, Object?>{};
    putTri<String>(out, 'base_url', baseUrl);
    putTri<String>(out, 'model', model);
    putTri<int>(out, 'max_tokens', maxTokens);
    putTri<bool>(out, 'show_reasoning', showReasoning);
    return out;
  }

  bool get isEmpty => toJson().isEmpty;
}

/// TTS 段补丁（字段级三态）。
class TtsSettingsPatch {
  const TtsSettingsPatch({
    this.baseUrl,
    this.model,
    this.voice,
    this.sampleRate,
    this.channels,
  });

  final Tri<String>? baseUrl;
  final Tri<String>? model;
  final Tri<String>? voice;
  final Tri<int>? sampleRate;
  final Tri<int>? channels;

  Map<String, Object?> toJson() {
    final Map<String, Object?> out = <String, Object?>{};
    putTri<String>(out, 'base_url', baseUrl);
    putTri<String>(out, 'model', model);
    putTri<String>(out, 'voice', voice);
    putTri<int>(out, 'sample_rate', sampleRate);
    putTri<int>(out, 'channels', channels);
    return out;
  }

  bool get isEmpty => toJson().isEmpty;
}

/// persona 段补丁（字段级三态；`Tri.clear()` = 清空该字段）。
///
/// # 只允许这两个键（M5.1 主链收敛）
///
/// 服务端 PATCH 的 persona 也只接受 system_prompt / max_history_pairs；
/// 酒馆卡字段迁到 Mod 后，这里再出现 name/first 之类的键就是**协议外字段**
/// （服务端会拒或静默忽略）。类型上只暴露这两个字段，从根上堵住「顺手加一个」。
class PersonaSettingsPatch {
  const PersonaSettingsPatch({this.systemPrompt, this.maxHistoryPairs});

  final Tri<String>? systemPrompt;
  final Tri<int>? maxHistoryPairs;

  Map<String, Object?> toJson() {
    final Map<String, Object?> out = <String, Object?>{};
    putTri<String>(out, 'system_prompt', systemPrompt);
    putTri<int>(out, 'max_history_pairs', maxHistoryPairs);
    return out;
  }

  bool get isEmpty => toJson().isEmpty;
}

/// action 段补丁（字段级三态）。
///
/// 三项都是 double 三态：`Tri.set(0.9)` = 设为该值；`Tri.clear()` = 显式
/// `null`，服务端把它解释为「回落出厂默认」（不是 0）。范围由服务端钳到
/// `[0.2, 2.2]`；前端滑条也只在这个区间里取值。
///
/// 另带**每模型覆盖** [models]（阶段5 D40）：值 `null` = 删该模型覆盖，
/// 对象 = 逐键三态合并。三者可以同时出现在同一个补丁里（互不干扰）。
class ActionSettingsPatch {
  const ActionSettingsPatch({
    this.headScale,
    this.bodyScale,
    this.expressionScale,
    this.models,
  });

  final Tri<double>? headScale;
  final Tri<double>? bodyScale;
  final Tri<double>? expressionScale;

  /// **每模型覆盖**（阶段5 D40）：键 = 模型 id。
  ///
  /// - 值为 `null` → JSON `"<id>": null` = **删除该模型覆盖**（回落全局）；
  /// - 值为对象 → 逐键三态合并（键 `null` = 删该键；数值 = 钳后写入）；
  /// - 整个字段为 `null` → **不写 models 键** = 不动整张表。
  ///
  /// 注意：`models: {}`（空表）与不写等价，[toJson] 不发空对象——
  /// 发了只会让服务端白跑一趟、还会让 [isEmpty] 误报「有改动」。
  final Map<String, ActionModelOverridePatch?>? models;

  Map<String, Object?> toJson() {
    final Map<String, Object?> out = <String, Object?>{};
    putTri<double>(out, 'head_scale', headScale);
    putTri<double>(out, 'body_scale', bodyScale);
    putTri<double>(out, 'expression_scale', expressionScale);
    if (models != null && models!.isNotEmpty) {
      final Map<String, Object?> byId = <String, Object?>{};
      models!.forEach((String id, ActionModelOverridePatch? ov) {
        // **`null` 必须写成 JSON null**（= 删除该模型覆盖），不能跳过——
        // 跳过在服务端是「不动」，用户点「恢复跟随全局」就永远删不掉。
        byId[id] = ov?.toJson();
      });
      out['models'] = byId;
    }
    return out;
  }

  bool get isEmpty => toJson().isEmpty;
}

/// 单个模型覆盖的字段级三态补丁（`[action.models.<id>]` 的一行）。
///
/// 与 [ActionSettingsPatch] 的三键同款 `Tri` 语义：不给键 = 不改该键 /
/// `Tri.clear()` = `null` = 删除该键（逐键回落全局）/
/// `Tri.set(v)` = 设为 v（服务端钳进 `[0.2, 2.2]`）。
///
/// **只写给出字段**是硬要求：新建一个「只覆盖 head」的模型时，
/// body / expression 必须保持「没覆盖」（否则它们会被钉死成当时的全局值，
/// 之后改全局不再跟随）。
class ActionModelOverridePatch {
  const ActionModelOverridePatch({
    this.headScale,
    this.bodyScale,
    this.expressionScale,
  });

  final Tri<double>? headScale;
  final Tri<double>? bodyScale;
  final Tri<double>? expressionScale;

  Map<String, Object?> toJson() {
    final Map<String, Object?> out = <String, Object?>{};
    putTri<double>(out, 'head_scale', headScale);
    putTri<double>(out, 'body_scale', bodyScale);
    putTri<double>(out, 'expression_scale', expressionScale);
    return out;
  }

  bool get isEmpty => toJson().isEmpty;
}

/// 本模型覆盖编辑的**合并防抖窗口**（阶段5 D40，2026-09-27）。
///
/// 250ms 与 `kActionScalesDebounce` 同量级：滑条一次拖动（divisions=46）
/// 会连发几十次 `onChanged`，每 250ms 收成一帧——用户松手前就能看到写回，
/// 又不会把「每次回调一次 PATCH（toml 原子写 + GET 回读 + supervisor 热重载）」
/// 打成几十次。
const Duration kModelOverrideDebounce = Duration(milliseconds: 250);

/// 把本模型覆盖的连续编辑合并成**一帧**三态补丁（纯逻辑，可在 VM 上单测）。
///
/// # 两条硬要求（阶段5 D40）
///
/// 1. **只记被改过的键**：未动的键保持「未覆盖」而继续逐键回落全局。
///    三条一起发会把 body / expression 钉死成当时的全局值，之后改全局不再跟随。
/// 2. **同一模型一次拖动只落一帧 PATCH**：窗口内反复 [record] 只重置定时器；
///    换模型 / [flush] 时才把待发键交给 [onFlush]。
///
/// 它**不碰**任何网络或设置控制器：宿主把 [onFlush] 接到自己的 PATCH 上。
class ModelOverrideCoalescer {
  ModelOverrideCoalescer({
    required this.onFlush,
    this.window = kModelOverrideDebounce,
  });

  /// 到点 / 主动刷新时交付「模型 id + 只含被改键的三态补丁 + 这一帧的代际」。
  ///
  /// 代际交给宿主：回读完成后 [acknowledge] 同一个数，滑条才放开「还没写回」
  /// 的显示。中途又拖过一帧时，旧回读的代际对不上，不会把新位置清掉。
  final void Function(String modelId, ActionModelOverridePatch patch, int epoch)
  onFlush;

  /// 防抖窗口。
  final Duration window;

  String? _modelId;
  final Map<String, double> _keys = <String, double>{};
  Timer? _timer;

  /// 滑条上还要显示、但服务端回读还没追上的键。
  ///
  /// [flush] 把待发键交出去之后这里**留着**：清早了，滑条会在回读完成前
  /// 弹回旧值。回读完成后由宿主 [acknowledge]。
  String? _latchedModel;
  final Map<String, double> _latched = <String, double>{};
  int _epoch = 0;

  /// 当前还显示在滑条上的模型（没有则 `null`）。
  String? get latchedModelId => _latchedModel;

  /// 还没被回读盖掉的键（`head_scale` / `body_scale` / `expression_scale`）。
  Map<String, double> get latchedKeys =>
      Map<String, double>.unmodifiable(_latched);

  /// 代际。每次 [record] / [cancel] 加一，用来认出过期的回读。
  int get epoch => _epoch;

  /// 是否有未到点的待发键（测试 / 排障用）。
  bool get hasPending => _modelId != null && _keys.isNotEmpty;

  /// 当前待发模型 id（没有则 `null`）。
  String? get pendingModelId => _modelId;

  /// 记一次编辑：只记**非 null** 的键。
  ///
  /// 换模型时先把上一个模型的待发键 [flush] 掉——否则它们会被并进
  /// 新模型的补丁里（串模型，比丢一次拖动更坏）。
  void record(
    String modelId, {
    double? headScale,
    double? bodyScale,
    double? expressionScale,
  }) {
    if (modelId.isEmpty) return;
    if (_modelId != null && _modelId != modelId) flush();
    if (_latchedModel != null && _latchedModel != modelId) {
      _latched.clear();
    }
    _latchedModel = modelId;
    _epoch++;
    _modelId = modelId;
    if (headScale != null) {
      _keys['head_scale'] = headScale;
      _latched['head_scale'] = headScale;
    }
    if (bodyScale != null) {
      _keys['body_scale'] = bodyScale;
      _latched['body_scale'] = bodyScale;
    }
    if (expressionScale != null) {
      _keys['expression_scale'] = expressionScale;
      _latched['expression_scale'] = expressionScale;
    }
    _timer?.cancel();
    _timer = Timer(window, flush);
  }

  /// 回读完成。代际还是这一帧才清掉滑条上的临时值；已经又拖过则留着。
  void acknowledge(int epoch) {
    if (epoch != _epoch) return;
    _latchedModel = null;
    _latched.clear();
  }

  /// 立刻交付待发键（定时器到点 / 离开分区 / 关闭前）。
  void flush() {
    _timer?.cancel();
    _timer = null;
    final String? modelId = _modelId;
    final Map<String, double> keys = Map<String, double>.of(_keys);
    _modelId = null;
    _keys.clear();
    if (modelId == null || keys.isEmpty) return;
    onFlush(
      modelId,
      ActionModelOverridePatch(
        headScale: keys['head_scale'] == null
            ? null
            : TriSet<double>(keys['head_scale']!),
        bodyScale: keys['body_scale'] == null
            ? null
            : TriSet<double>(keys['body_scale']!),
        expressionScale: keys['expression_scale'] == null
            ? null
            : TriSet<double>(keys['expression_scale']!),
      ),
      epoch,
    );
  }

  /// 丢掉未到点的待发键，并清掉滑条上还挂着的临时值。
  ///
  /// [modelId] 不为 null 时，只在该模型正是待发模型或临时显示的模型时才丢。
  /// 「恢复跟随全局 / 开启覆盖」必须先取消，否则迟到的 PATCH 会把
  /// 刚删掉的覆盖又写回来。代际一并加一，让已经发出去的那一帧回读
  /// 不能再把滑条清回旧位置之后的新拖动。
  void cancel([String? modelId]) {
    final bool keysMatch = modelId == null || _modelId == modelId;
    final bool latchMatch = modelId == null || _latchedModel == modelId;
    if (!keysMatch && !latchMatch) return;
    if (keysMatch) {
      _timer?.cancel();
      _timer = null;
      _modelId = null;
      _keys.clear();
    }
    if (latchMatch) {
      _epoch++;
      _latchedModel = null;
      _latched.clear();
    }
  }

  /// 释放（宿主 dispose）。等价 [cancel]。
  void dispose() => cancel();
}

/// `PATCH /api/v1/settings` 的请求体。
///
/// # 段级语义（刻意只暴露安全子集）
///
/// 服务端支持「段显式 `null` = 整段清空」（`SettingsPatch` 的 `Option<Option<..>>`）。
/// **本客户端刻意不提供这个能力**：把整段 LLM/TTS 配置一次抹掉是个破坏性操作，
/// 不该作为「改设置」的副作用被顺手发出去。要清空请用字段级 `Tri.clear()`。
/// 段为 `null` 一律表示「不动这一段」。
class SettingsPatch {
  const SettingsPatch({
    this.llm,
    this.tts,
    this.persona,
    this.action,
    this.devMode,
  });

  final LlmSettingsPatch? llm;
  final TtsSettingsPatch? tts;
  final PersonaSettingsPatch? persona;

  /// 动作幅度段（`[action]`）。
  final ActionSettingsPatch? action;

  /// 顶层 `dev_mode` 三态。
  final Tri<bool>? devMode;

  /// 序列化为请求体。
  ///
  /// 只写「真的要改」的东西：空段、空补丁、`TriKeep` 一律不出现。
  Map<String, Object?> toJson() {
    final Map<String, Object?> out = <String, Object?>{};
    // **空段不写键**。服务端把 `{"llm":{}}` 当 no-op，语义上等价于不写；
    // 但若照写，`isEmpty` 会变假，调用方就会为一个「什么都没改」的补丁
    // 白发一次网络往返（还会让服务端白跑一遍 apply_patch）。
    if (llm != null && !llm!.isEmpty) out['llm'] = llm!.toJson();
    if (tts != null && !tts!.isEmpty) out['tts'] = tts!.toJson();
    if (persona != null && !persona!.isEmpty) {
      out['persona'] = persona!.toJson();
    }
    if (action != null && !action!.isEmpty) {
      out['action'] = action!.toJson();
    }
    putTri<bool>(out, 'dev_mode', devMode);
    return out;
  }

  /// 是否什么都没改（调用方可据此跳过网络请求）。
  bool get isEmpty => toJson().isEmpty;
}
