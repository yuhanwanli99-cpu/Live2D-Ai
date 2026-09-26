/// 设置模型与三态补丁：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 契约来源（读源码确认，不是猜的）：
/// - 响应形状：`crates/live2d-ai-runtime/src/settings/view.rs`（`SettingsView` 等）
/// - 请求形状：`crates/live2d-ai-runtime/src/settings/patch.rs`（`SettingsPatch` 等）
/// - 路由：`PATCH /api/v1/settings`（**是 PATCH 不是 PUT**，见 `web_api/mod.rs`
///   的 `match_route`；实测 `PUT→404` / `PATCH→200`）
library;

import 'dart:async';

/// 字段级**三态**值：区分「不修改」/「显式清空」/「设为目标值」。
///
/// # 为什么必须专门造一个类型
///
/// 服务端用 `Option<Option<T>>` 表达这三态（`patch.rs` 的 `LlmPatch` 等）：
///
/// | 前端想表达 | 服务端收到的 JSON |
/// | --- | --- |
/// | 不修改 | 键**不出现** |
/// | 显式清空 | `"field": null` |
/// | 设为某值 | `"field": "…"` |
///
/// Dart 里「键不出现」与「值为 null」都能用一个可空字段表示，**两者会混成一种**。
/// 一旦把「不修改」也序列化成 `null`，用户只改一个字段就会把同段其它字段**清空**——
/// 这是本层最容易实现错、且后果最严重的地方（静默丢配置），
/// 所以这里用显式三态把两者在类型上分开。
///
/// 用法：
/// ```dart
/// LlmSettingsPatch(model: Tri.set('deepseek-chat'));   // 只改 model
/// TtsSettingsPatch(model: Tri.clear());                // 显式清空某字段
/// LlmSettingsPatch();                                  // 什么都不改
/// ```
sealed class Tri<T> {
  const Tri();

  /// 不修改该字段（序列化时键不出现）。
  static Tri<T> keep<T>() => TriKeep<T>();

  /// 显式清空该字段（序列化为 `null`）。
  static Tri<T> clear<T>() => TriClear<T>();

  /// 设为该值。
  static Tri<T> set<T>(T value) => TriSet<T>(value);
}

/// 「不修改」。
final class TriKeep<T> extends Tri<T> {
  const TriKeep();
  @override
  String toString() => 'Tri.keep';
}

/// 「显式清空」。
final class TriClear<T> extends Tri<T> {
  const TriClear();
  @override
  String toString() => 'Tri.clear';
}

/// 「设为该值」。
final class TriSet<T> extends Tri<T> {
  const TriSet(this.value);
  final T value;
  @override
  String toString() => 'Tri.set($value)';
}

/// 把三态字段写进 [out]。
///
/// - `null`（字段没提供）与 [TriKeep] 都表示「不修改」→ **不写键**；
/// - [TriClear] → 写 `null`；
/// - [TriSet] → 写值。
void putTri<T>(Map<String, Object?> out, String key, Tri<T>? field) {
  switch (field) {
    case null:
    case TriKeep<T>():
      break;
    case TriClear<T>():
      out[key] = null;
    case TriSet<T>(:final T value):
      out[key] = value;
  }
}

/// `GET /api/v1/settings` 的 LLM 段。
///
/// 注意 `api_key_env` **不在响应里**（`view.rs::LlmView` 只有三个字段）——
/// 绑定名不从这里下发。[hasApiKey] 只回一个布尔（「声明了键名且值真读得到」，
/// P2 单一语义）。界面要在「密钥绑定」一行显示名字时，走**另一条已验证的路**：
/// `GET /api/v1/env` 的 `EnvKey.key`（见 `api/env_api.dart`）。P6 起变量名
/// 不再可在界面上编辑——改绑定只能改 `live2d-ai.toml` 的 `api_key_env`。
class LlmSettingsView {
  const LlmSettingsView({
    required this.baseUrl,
    required this.model,
    required this.hasApiKey,
    required this.maxTokens,
    required this.showReasoning,
  });

  final String baseUrl;
  final String model;

  /// P2 收敛后的单一语义：键名已声明**且**服务端真能解析到非空值。
  /// `GET` 与 `PATCH /api/v1/settings` 同口径；**密钥本身与变量名都永不下发**。
  final bool hasApiKey;

  /// **最终生效**的输出 token 上限；`0` = 不限制。
  ///
  /// 服务端回的是生效值而不是原始的 `Option`：界面要显示「现在到底卡在
  /// 多少」，回 `null` 对用户没有意义。
  final int maxTokens;

  /// 服务端在配置省略 `max_tokens` 时使用的默认上限。
  ///
  /// **必须与 `live2d_ai_runtime::settings::DEFAULT_MAX_TOKENS` 一致**
  /// （`crates/live2d-ai-runtime/src/settings.rs`，当前 = 4096）。这里曾错写成
  /// 512（P3 修复）：推理模型下思考与正文共用 `max_tokens`，上限过小时正文会被
  /// 挤成半句 → 切不出完整句 → 一个字都不上屏。
  ///
  /// 只用于界面文案（「默认 4096」）；真正生效的值以 [maxTokens] 为准——
  /// 不拿它做本地计算，否则前后端两个默认值会各自漂移。
  static const int defaultMaxTokens = 4096;

  /// **展示思考总闸的生效值**（`llm.show_reasoning`，服务端缺省 = false）。
  ///
  /// 这是产品开关（要不要看模型的内心独白），不是性能开关：关掉它
  /// 不省 token，也不改变「思考不进 TTS」这条既有纪律。
  final bool showReasoning;

  /// `maxTokens == 0` = 不限制（协议里请求体省略该字段）。
  bool get isMaxTokensUnlimited => maxTokens == 0;

  factory LlmSettingsView.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    return LlmSettingsView(
      baseUrl: _str(j['base_url']),
      model: _str(j['model']),
      hasApiKey: j['has_api_key'] == true,
      // 服务端缺失该字段（旧服务端）→ 回落默认；回落成 0 语义正好相反。
      maxTokens: j.containsKey('max_tokens')
          ? _int(j['max_tokens'])
          : defaultMaxTokens,
      // 旧服务端缺该字段 → false（与「缺省不展示思考」同口径，
      // 不会因为前端连了旧后端就突然把思考摆上屏）。
      showReasoning: j['show_reasoning'] == true,
    );
  }
}

/// `GET /api/v1/settings` 的 TTS 段。
class TtsSettingsView {
  const TtsSettingsView({
    required this.baseUrl,
    required this.model,
    required this.voice,
    required this.hasApiKey,
    required this.sampleRate,
    required this.channels,
  });

  final String baseUrl;

  /// 可为 null（协议里是 `Option<String>`）。
  final String? model;
  final String voice;

  /// 同 `LlmSettingsView.hasApiKey`：键名已声明且值真读得到（P2 单一语义）。
  final bool hasApiKey;
  final int sampleRate;
  final int channels;

  factory TtsSettingsView.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    return TtsSettingsView(
      baseUrl: _str(j['base_url']),
      model: j['model'] is String ? j['model']! as String : null,
      voice: _str(j['voice']),
      hasApiKey: j['has_api_key'] == true,
      sampleRate: _int(j['sample_rate']),
      channels: _int(j['channels']),
    );
  }
}

/// `GET /api/v1/settings` 的 persona 段。
///
/// # 2026-09-13（rc.4 M5.1）：只剩两个字段
///
/// 酒馆角色卡的 name / description / personality / scenario / first 已从
/// **主链**迁出，改由标准 Mod live2d-ai-mod-persona 承担
/// （docs/plans/PLAN-rc4-mod-product-chain-2026-09-13.md §9.1）。
/// 服务端 View 与 PATCH 都只认 system_prompt 与 max_history_pairs；
/// 这里**刻意不解析**旧的卡字段——解析了界面就会又长回去。
class PersonaSettingsView {
  const PersonaSettingsView({
    required this.systemPrompt,
    required this.maxHistoryPairs,
  });

  final String systemPrompt;

  /// 历史轮数上限（会话基建；0 = 不带历史）。
  final int maxHistoryPairs;

  factory PersonaSettingsView.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    return PersonaSettingsView(
      systemPrompt: _str(j['system_prompt']),
      maxHistoryPairs: _int(j['max_history_pairs']),
    );
  }
}

/// `GET /api/v1/settings` → `action.models.<model_id>` 的**单模型覆盖视图**。
///
/// # 契约（阶段5 D40，W5r 冻结，2026-09-26）
///
/// 三键**始终出现**；未覆盖的键 = `null`（**不是缺键**）。界面按
/// 「逐键 `override[key] ?? global[key]`」算本模型有效值，所以这里用
/// 可空的三个 `double?` 保留「没覆盖」这一态——回落成全局值会把
/// 「覆盖 = 全局」与「没覆盖」混成一种，用户点「恢复跟随全局」时看不出来。
class ActionModelOverrideView {
  const ActionModelOverrideView({
    required this.headScale,
    required this.bodyScale,
    required this.expressionScale,
  });

  final double? headScale;
  final double? bodyScale;
  final double? expressionScale;

  factory ActionModelOverrideView.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    return ActionModelOverrideView(
      headScale: _dblOrNull(j['head_scale']),
      bodyScale: _dblOrNull(j['body_scale']),
      expressionScale: _dblOrNull(j['expression_scale']),
    );
  }
}

/// 覆盖视图 → 渲染面载荷键（**只含非 null 的键**）。
///
/// 键名与 [toActionScalesPayload] 一致（`head` / `body` / `expression`）：
/// syncer 要在**同一份载荷**上做逐键替换，两套键名会立刻分叉。
Map<String, double> toOverridePayload(ActionModelOverrideView v) =>
    <String, double>{
      if (v.headScale != null) 'head': v.headScale!,
      if (v.bodyScale != null) 'body': v.bodyScale!,
      if (v.expressionScale != null) 'expression': v.expressionScale!,
    };

/// 覆盖表 → 逐模型载荷表（只留至少一键非 null 的模型）。
///
/// 空覆盖（三键全 `null`）**不进表**：它与「没有这个模型」在下游等价，
/// 留在表里只会让 syncer 多走一次不会改值的分支。
Map<String, Map<String, double>> toModelOverrideMap(
  Map<String, ActionModelOverrideView> models,
) {
  final Map<String, Map<String, double>> out = <String, Map<String, double>>{};
  models.forEach((String id, ActionModelOverrideView v) {
    final Map<String, double> payload = toOverridePayload(v);
    if (payload.isNotEmpty) out[id] = payload;
  });
  return out;
}

/// `GET /api/v1/settings` 的 `action` 段（2026-09-16，用户可调动作幅度）。
///
/// 三项独立倍率，作用于渲染面预设表内的幅值：
/// **实际写入 = 表值 × 峰值系数 × intensity × 倍率**，再按通道红线钳位
///（头 ParamAngle* ≤30、身 ParamBodyAngle* ≤10、五官 ≤4）。
///
/// 基准写清（2026-09-24 重标定）：head 出厂 0.75、body 0.80、expression 1.0；
/// 范围 [0.2, 2.2]（越界由服务端钳位）。**每个旋钮单独走满都不触上限**，
/// 但 intensity(3.0) 与倍率(2.2) 同时拉满会钳位——组合表见
/// `crates/l2d-wasm-demo/src/preset/scales.rs`。
/// 服务端缺 `action` 段（旧服务端）时回落这组出厂默认——不回落成 1.0，
/// 否则「升级到旧后端」会静默改变舞台观感。
class ActionSettingsView {
  const ActionSettingsView({
    required this.headScale,
    required this.bodyScale,
    required this.expressionScale,
    this.activeModelId = '',
    this.models = const <String, ActionModelOverrideView>{},
  });

  final double headScale;
  final double bodyScale;
  final double expressionScale;

  /// **当前生效的模型 id**（顶层 `active_model_id`，阶段5 D40）。
  ///
  /// 空串 = 服务端没给 / 未识别（egui 口径与旧服务端都会是空串）。
  /// 界面据此从 [models] 里取本模型覆盖，并显示「未识别当前模型」。
  final String activeModelId;

  /// 每模型覆盖（键 = 模型 id）。没有配置时是空表。
  final Map<String, ActionModelOverrideView> models;

  /// 倍率下限（与服务端 clamp_action_scale 同口径）。
  static const double minScale = 0.2;

  /// 倍率上限（与服务端 MAX_ACTION_SCALE 同口径；2026-09-24：2.5 → 2.2）。
  static const double maxScale = 2.2;

  /// 出厂默认（与服务端 DEFAULT_*_SCALE 同口径）。
  static const double defaultHeadScale = 0.75;
  static const double defaultBodyScale = 0.80;
  static const double defaultExpressionScale = 1.0;

  /// `activeModelId` 来自**顶层** `active_model_id`（不在 action 段里）：
  /// 由 [SettingsView.fromJson] 透传；单独解析 action 段时缺省空串 =
  /// 按全局值算（与「服务端没给」同口径，不猜）。
  factory ActionSettingsView.fromJson(
    Map<String, Object?>? json, {
    String activeModelId = '',
  }) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    return ActionSettingsView(
      headScale: _dbl(j['head_scale'], defaultHeadScale),
      bodyScale: _dbl(j['body_scale'], defaultBodyScale),
      expressionScale: _dbl(j['expression_scale'], defaultExpressionScale),
      activeModelId: activeModelId,
      models: _modelViews(j['models']),
    );
  }

  /// 本模型覆盖（`models[activeModelId]`；空 id / 无覆盖 → `null`）。
  ActionModelOverrideView? get activeModelOverride =>
      activeModelId.isEmpty ? null : models[activeModelId];

  /// 本模型**有效**三键：`override[key] ?? global[key]`（**逐键**回落，D40）。
  ///
  /// 没有覆盖 → 返回自身（与今天逐字一致）；有覆盖 → 只替换非 null 的键。
  /// 这是 UI 三条滑条与 HUD 读数的同一份真源（syncer 侧再做一次同口径替换，
  /// 因为下发的是载荷 map 而不是这个 view）。
  ActionSettingsView get effectiveForActiveModel {
    final ActionModelOverrideView? ov = activeModelOverride;
    if (ov == null) return this;
    return ActionSettingsView(
      headScale: ov.headScale ?? headScale,
      bodyScale: ov.bodyScale ?? bodyScale,
      expressionScale: ov.expressionScale ?? expressionScale,
      activeModelId: activeModelId,
      models: models,
    );
  }
}

/// 渲染面 `sync.actionScales` 载荷（三项都必须是有限数）。
Map<String, double> toActionScalesPayload(ActionSettingsView v) =>
    <String, double>{
      'head': v.headScale,
      'body': v.bodyScale,
      'expression': v.expressionScale,
    };

/// 舞台当前该用哪一组幅度倍率（**即时预览的真源**，2026-09-16 修）。
///
/// # 为什么需要它（真实来历）
///
/// 产品滑条过去只改设置草稿，点「保存」才经 `GET /settings` 回填、再下发给
/// 渲染面——拖的时候舞台**毫无反应**，用户的原话是「滑条要先保存才生效」。
/// 这个函数把「草稿 / 磁盘」合成**一份**有效值：
///
/// - 草稿里有的字段（用户正在拖、还没保存）**优先**——即时预览；
/// - 没有的字段回落远端（= 磁盘上的 `[action]`）；
/// - 两者都没有（设置还没加载）→ `null`：**不下发**，渲染面用自己的出厂默认
///   （不谎报成 1.0，否则「升级到旧后端」会静默改观感）。
///
/// 传入的 `draftHead` 等是**改动集**语义（`null` = 没动），与
/// `SettingsDraft.actionHeadScale` 完全一致。
ActionSettingsView? effectiveActionScales({
  required ActionSettingsView? remote,
  double? draftHead,
  double? draftBody,
  double? draftExpression,
}) {
  if (remote == null && draftHead == null && draftBody == null && draftExpression == null) {
    return null;
  }
  final ActionSettingsView base = remote ??
      const ActionSettingsView(
        headScale: ActionSettingsView.defaultHeadScale,
        bodyScale: ActionSettingsView.defaultBodyScale,
        expressionScale: ActionSettingsView.defaultExpressionScale,
      );
  return ActionSettingsView(
    headScale: draftHead ?? base.headScale,
    bodyScale: draftBody ?? base.bodyScale,
    expressionScale: draftExpression ?? base.expressionScale,
  );
}

/// `GET /api/v1/settings` 的完整响应。
class SettingsView {
  const SettingsView({
    required this.llm,
    required this.tts,
    required this.persona,
    required this.action,
    required this.devMode,
  });

  final LlmSettingsView llm;
  final TtsSettingsView tts;
  final PersonaSettingsView persona;

  /// 动作幅度段（`[action]`，2026-09-16）。
  final ActionSettingsView action;

  /// 开发模式：开启后界面才显示高级/诊断分区（渐进披露的第二层）。
  final bool devMode;

  factory SettingsView.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    return SettingsView(
      llm: LlmSettingsView.fromJson(_obj(j['llm'])),
      tts: TtsSettingsView.fromJson(_obj(j['tts'])),
      persona: PersonaSettingsView.fromJson(_obj(j['persona'])),
      // D40：本模型覆盖在 action 段里，但当前模型 id 在**顶层**。
      action: ActionSettingsView.fromJson(
        _obj(j['action']),
        activeModelId: _str(j['active_model_id']),
      ),
      devMode: j['dev_mode'] == true,
    );
  }
}

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

  /// 到点 / 主动刷新时交付「模型 id + 只含被改键的三态补丁」。
  final void Function(String modelId, ActionModelOverridePatch patch) onFlush;

  /// 防抖窗口。
  final Duration window;

  String? _modelId;
  final Map<String, double> _keys = <String, double>{};
  Timer? _timer;

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
    _modelId = modelId;
    if (headScale != null) _keys['head_scale'] = headScale;
    if (bodyScale != null) _keys['body_scale'] = bodyScale;
    if (expressionScale != null) _keys['expression_scale'] = expressionScale;
    _timer?.cancel();
    _timer = Timer(window, flush);
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
    );
  }

  /// 丢掉未到点的待发键。
  ///
  /// [modelId] 不为 null 时只在该模型正是待发模型时才丢——
  /// 「恢复跟随全局 / 开启覆盖」必须先取消，否则迟到的 PATCH 会把
  /// 刚删掉的覆盖又写回来。
  void cancel([String? modelId]) {
    if (modelId != null && _modelId != modelId) return;
    _timer?.cancel();
    _timer = null;
    _modelId = null;
    _keys.clear();
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

/// `POST /api/v1/settings/test/{llm,tts}` 的结果。
///
/// 契约：`web_api/settings_routes/test_endpoints.rs::TestOutcome`。
class SettingsTestOutcome {
  const SettingsTestOutcome({
    required this.ok,
    this.latencyMs,
    this.modelEcho,
    this.note,
    this.errorCode,
    this.errorMessage,
  });

  final bool ok;

  /// 往返耗时（毫秒）；服务端用 `u128`，这里收成 int。
  final int? latencyMs;

  /// 服务端回显的模型/音色名——用来确认「打到的确实是配置里那个」。
  final String? modelEcho;

  /// 成功但有话要说（例如「上游可达但未提供 /models」）。
  ///
  /// **只有真有解释价值时才非空**——服务端刻意不在成功路径上默认塞一句，
  /// 否则每次自检都多一句噪音，用户很快就不看它了（见服务端 `TestOutcome::note`）。
  final String? note;

  final String? errorCode;
  final String? errorMessage;

  factory SettingsTestOutcome.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    Map<String, Object?>? err;
    if (j['error'] is Map) {
      final Map<String, Object?> e = <String, Object?>{};
      (j['error']! as Map<Object?, Object?>).forEach((
        Object? k,
        Object? v,
      ) {
        if (k is String) e[k] = v;
      });
      err = e;
    }
    return SettingsTestOutcome(
      ok: j['ok'] == true,
      latencyMs: j['latency_ms'] is num
          ? (j['latency_ms']! as num).toInt()
          : null,
      modelEcho: j['model_echo'] is String ? j['model_echo']! as String : null,
      note: j['note'] is String ? j['note']! as String : null,
      errorCode: err?['code'] is String ? err!['code']! as String : null,
      errorMessage: err?['message'] is String
          ? err!['message']! as String
          : null,
    );
  }
}

/// `PATCH /api/v1/settings` 的响应。
///
/// 契约：服务端回 `{"persisted": bool, "settings": {…}}`。
/// 写盘后 supervisor 的装载/重载状态（`dto.rs::ApplyStatus`，`snake_case`）。
///
/// **必须消费它**：写盘成功不等于生效。不区分就会出现
/// 「提示已热重载但实际 503」的错位——用户按提示发了消息却收不到回复。
enum ApplyStatus {
  /// supervisor 动态创建成功 / reload 已触发 → 可以立刻对话。
  applied('applied'),

  /// 写盘成功，但无法动态创建 supervisor（配置不全）→ **需重启**。
  restartRequired('restart_required'),

  /// reload 已排队、异步生效中。
  ///
  /// 服务端注释说明当前实现下**实际不会产生**该值（装配/重建都在请求线程内
  /// 完成），保留作未来异步路径。前端仍然要处理它——协议里有的取值都要兜住。
  queued('queued'),

  /// 写盘成功但**没有** supervisor（dev-only 控制平面 / 关掉了对话引擎）。
  noSupervisor('no_supervisor'),

  /// 未识别取值（协议新增而前端还旧）。**不静默当成 applied**。
  unknown('unknown');

  const ApplyStatus(this.wire);

  final String wire;

  static ApplyStatus fromWire(String? raw) {
    for (final ApplyStatus value in values) {
      if (value != unknown && value.wire == raw) return value;
    }
    return unknown;
  }

  /// 是否「已经生效、可以直接对话」。
  bool get isLive => this == ApplyStatus.applied;
}

class SettingsPatchResult {
  const SettingsPatchResult({
    required this.persisted,
    required this.settings,
    this.applyStatus = ApplyStatus.unknown,
  });

  /// 服务端是否**真的写回了磁盘**。
  ///
  /// `false` 表示补丁与当前值完全一致（`PatchOutcome::NoChange`）——
  /// 界面可以据此显示「没有变化」而不是「已保存」，两者对用户不是一回事。
  final bool persisted;

  /// 应用补丁**之后**的服务端权威设置（用它回填界面，不要用本地草稿）。
  final SettingsView settings;

  /// 写盘后的生效状态（缺省 `unknown`：旧服务端 / 字段缺失）。
  final ApplyStatus applyStatus;

  factory SettingsPatchResult.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    return SettingsPatchResult(
      persisted: j['persisted'] == true,
      settings: SettingsView.fromJson(_obj(j['settings'])),
      applyStatus: ApplyStatus.fromWire(
        j['apply_status'] is String ? j['apply_status']! as String : null,
      ),
    );
  }
}

/// 宽容取字符串：非字符串一律空串（响应缺字段时界面显示空，而不是崩）。
String _str(Object? value) => value is String ? value : '';

/// 宽容取整数：JSON 里 `24000` 与 `24000.0` 都合法。
int _int(Object? value) {
  if (value is int) return value;
  if (value is num) return value.toInt();
  return 0;
}

/// 宽容取 double：JSON 里 `0.75` 与 `1` 都合法；非数 → 给定缺省。
double _dbl(Object? value, double fallback) {
  if (value is num && value.isFinite) return value.toDouble();
  return fallback;
}

/// 宽容取**可空** double：非数字 / 非有限数 / 缺键 → `null`（= 未覆盖）。
///
/// 与 [_dbl] 的区别就是这层语义：覆盖里 `null` 是**有意义的**「回落全局」，
/// 不能像全局值那样回落到 0.75/0.80/1.0（那会把「没覆盖」写成一个覆盖）。
double? _dblOrNull(Object? value) {
  if (value is num && value.isFinite) return value.toDouble();
  return null;
}

/// 宽容解析 `action.models`：非对象 / 非法条目一律忽略（不抛）。
Map<String, ActionModelOverrideView> _modelViews(Object? raw) {
  if (raw is! Map) return const <String, ActionModelOverrideView>{};
  final Map<String, ActionModelOverrideView> out =
      <String, ActionModelOverrideView>{};
  raw.forEach((Object? k, Object? v) {
    if (k is! String || v is! Map) return;
    out[k] = ActionModelOverrideView.fromJson(_obj(v));
  });
  return out;
}

/// 宽容取对象：不是对象就返回 null（交给各自的 `fromJson` 走默认值）。
Map<String, Object?>? _obj(Object? value) {
  if (value is! Map) return null;
  final Map<String, Object?> out = <String, Object?>{};
  value.forEach((Object? k, Object? v) {
    if (k is String) out[k] = v;
  });
  return out;
}
