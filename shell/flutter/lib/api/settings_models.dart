/// 设置模型与三态补丁：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 契约来源（读源码确认，不是猜的）：
/// - 响应形状：`crates/live2d-ai-runtime/src/settings/view.rs`（`SettingsView` 等）
/// - 请求形状：`crates/live2d-ai-runtime/src/settings/patch.rs`（`SettingsPatch` 等）
/// - 路由：`PATCH /api/v1/settings`（**是 PATCH 不是 PUT**，见 `web_api/mod.rs`
///   的 `match_route`；实测 `PUT→404` / `PATCH→200`）
library;

import 'dart:async';

part 'settings_models_patch.dart';
part 'settings_models_result.dart';

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
  if (remote == null &&
      draftHead == null &&
      draftBody == null &&
      draftExpression == null) {
    return null;
  }
  final ActionSettingsView base =
      remote ??
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

/// 动作幅度滑条上该显示的数。
///
/// 全局三键：草稿压过磁盘。不这么做的话，拖动只改了草稿和舞台，
/// 滑条仍绑着磁盘值，手指一动滑块就弹回去。
/// 本模型覆盖：防抖窗口里还没回读完的键压过磁盘上的覆盖，键名是
/// `head_scale` / `body_scale` / `expression_scale`。
ActionSettingsView displayActionScales({
  required ActionSettingsView remote,
  double? draftHead,
  double? draftBody,
  double? draftExpression,
  String? pendingModelId,
  Map<String, double> pendingKeys = const <String, double>{},
}) {
  final double head = draftHead ?? remote.headScale;
  final double body = draftBody ?? remote.bodyScale;
  final double expression = draftExpression ?? remote.expressionScale;
  final String? pendingId = pendingModelId;
  final bool pendingHere =
      pendingId != null &&
      pendingId.isNotEmpty &&
      pendingId == remote.activeModelId &&
      pendingKeys.isNotEmpty;
  if (!pendingHere) {
    return ActionSettingsView(
      headScale: head,
      bodyScale: body,
      expressionScale: expression,
      activeModelId: remote.activeModelId,
      models: remote.models,
    );
  }
  final String id = pendingId;
  final ActionModelOverrideView existing =
      remote.models[id] ??
      const ActionModelOverrideView(
        headScale: null,
        bodyScale: null,
        expressionScale: null,
      );
  final Map<String, ActionModelOverrideView> models =
      Map<String, ActionModelOverrideView>.of(remote.models);
  models[id] = ActionModelOverrideView(
    headScale: pendingKeys.containsKey('head_scale')
        ? pendingKeys['head_scale']
        : existing.headScale,
    bodyScale: pendingKeys.containsKey('body_scale')
        ? pendingKeys['body_scale']
        : existing.bodyScale,
    expressionScale: pendingKeys.containsKey('expression_scale')
        ? pendingKeys['expression_scale']
        : existing.expressionScale,
  );
  return ActionSettingsView(
    headScale: head,
    bodyScale: body,
    expressionScale: expression,
    activeModelId: remote.activeModelId,
    models: models,
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
