/// 设置模型与三态补丁：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 契约来源（读源码确认，不是猜的）：
/// - 响应形状：`crates/live2d-ai-runtime/src/settings/view.rs`（`SettingsView` 等）
/// - 请求形状：`crates/live2d-ai-runtime/src/settings/patch.rs`（`SettingsPatch` 等）
/// - 路由：`PATCH /api/v1/settings`（**是 PATCH 不是 PUT**，见 `web_api/mod.rs`
///   的 `match_route`；实测 `PUT→404` / `PATCH→200`）
library;

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
/// LlmSettingsPatch(apiKeyEnv: Tri.clear());            // 显式清空 env 绑定
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
/// 即界面**无法回显**当前绑定的是哪个环境变量，只能显示 [hasApiKey]。
/// 这是服务端契约的一个缺口，已记录待裁决；前端不要假装知道那个名字。
class LlmSettingsView {
  const LlmSettingsView({
    required this.baseUrl,
    required this.model,
    required this.hasApiKey,
    required this.maxTokens,
  });

  final String baseUrl;
  final String model;

  /// 服务端是否已能解析到密钥。**密钥本身永远不下发**。
  final bool hasApiKey;

  /// **最终生效**的输出 token 上限；`0` = 不限制。
  ///
  /// 服务端回的是生效值而不是原始的 `Option`：界面要显示「现在到底卡在
  /// 多少」，回 `null` 对用户没有意义。
  final int maxTokens;

  /// 服务端在配置省略 `max_tokens` 时使用的默认上限。
  ///
  /// 只用于界面文案（「默认 512」）；真正生效的值以 [maxTokens] 为准——
  /// 不拿它做本地计算，否则前后端两个默认值会各自漂移。
  static const int defaultMaxTokens = 512;

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

/// `GET /api/v1/settings` 的 persona 段（酒馆式角色卡）。
class PersonaSettingsView {
  const PersonaSettingsView({
    required this.systemPrompt,
    required this.maxHistoryPairs,
    required this.name,
    required this.description,
    required this.personality,
    required this.scenario,
    required this.first,
  });

  final String systemPrompt;
  final int maxHistoryPairs;
  final String name;
  final String description;
  final String personality;
  final String scenario;

  /// 开场白。
  final String first;

  factory PersonaSettingsView.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    return PersonaSettingsView(
      systemPrompt: _str(j['system_prompt']),
      maxHistoryPairs: _int(j['max_history_pairs']),
      name: _str(j['name']),
      description: _str(j['description']),
      personality: _str(j['personality']),
      scenario: _str(j['scenario']),
      first: _str(j['first']),
    );
  }
}

/// `GET /api/v1/settings` 的完整响应。
class SettingsView {
  const SettingsView({
    required this.llm,
    required this.tts,
    required this.persona,
    required this.devMode,
  });

  final LlmSettingsView llm;
  final TtsSettingsView tts;
  final PersonaSettingsView persona;

  /// 开发模式：开启后界面才显示高级/诊断分区（渐进披露的第二层）。
  final bool devMode;

  factory SettingsView.fromJson(Map<String, Object?>? json) {
    final Map<String, Object?> j = json ?? const <String, Object?>{};
    return SettingsView(
      llm: LlmSettingsView.fromJson(_obj(j['llm'])),
      tts: TtsSettingsView.fromJson(_obj(j['tts'])),
      persona: PersonaSettingsView.fromJson(_obj(j['persona'])),
      devMode: j['dev_mode'] == true,
    );
  }
}

/// LLM 段补丁（字段级三态）。
class LlmSettingsPatch {
  const LlmSettingsPatch({
    this.baseUrl,
    this.model,
    this.apiKeyEnv,
    this.maxTokens,
    this.clearApiKey = false,
  });

  final Tri<String>? baseUrl;
  final Tri<String>? model;

  /// 绑定的**环境变量名**（不是密钥本身）。
  ///
  /// 治理红线：前端**不得持有密钥**（AGENTS.md）。所以设置界面只能让用户填
  /// 「环境变量名」，密钥由服务端从进程环境读取。
  final Tri<String>? apiKeyEnv;

  /// 输出 token 上限（三态）。
  ///
  /// **`Tri.set(0)` 与 `Tri.clear()` 是两件不同的事**，不能合并：
  /// - `Tri.set(0)` → JSON `max_tokens: 0` → 不限制；
  /// - `Tri.clear()` → JSON `max_tokens: null` → 清除，回落服务端默认；
  /// - 字段留 `null` → JSON 里**不出现**该键 → 保持原值。
  final Tri<int>? maxTokens;

  /// 段级「清除密钥绑定」标志（`clear_api_key: true`）。
  ///
  /// **为什么光发 `api_key_env: null` 不够**：服务端的 P0-2 保护把
  /// 「不带 `clear_api_key` 的 `api_key_env: null`」当成**保持原值**
  /// （防止「只改一个字段却把别的字段清了」那类事故）。要真的清除，
  /// 必须同时给出这个显式意图。
  ///
  /// 与 [apiKeyEnv] 同时给出时**新值优先**（服务端 `apply_inject_rules` 规则 2）。
  final bool clearApiKey;

  Map<String, Object?> toJson() {
    final Map<String, Object?> out = <String, Object?>{};
    putTri<String>(out, 'base_url', baseUrl);
    putTri<String>(out, 'model', model);
    putTri<String>(out, 'api_key_env', apiKeyEnv);
    putTri<int>(out, 'max_tokens', maxTokens);
    // 段级清除意图：**不是**三态字段，是布尔标志。
    if (clearApiKey) out['clear_api_key'] = true;
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
    this.apiKeyEnv,
    this.sampleRate,
    this.channels,
    this.clearApiKey = false,
  });

  final Tri<String>? baseUrl;
  final Tri<String>? model;
  final Tri<String>? voice;
  final Tri<String>? apiKeyEnv;
  final Tri<int>? sampleRate;
  final Tri<int>? channels;

  /// 段级「清除密钥绑定」标志（语义见 `LlmSettingsPatch.clearApiKey`）。
  final bool clearApiKey;

  Map<String, Object?> toJson() {
    final Map<String, Object?> out = <String, Object?>{};
    putTri<String>(out, 'base_url', baseUrl);
    putTri<String>(out, 'model', model);
    putTri<String>(out, 'voice', voice);
    putTri<String>(out, 'api_key_env', apiKeyEnv);
    putTri<int>(out, 'sample_rate', sampleRate);
    putTri<int>(out, 'channels', channels);
    if (clearApiKey) out['clear_api_key'] = true;
    return out;
  }

  bool get isEmpty => toJson().isEmpty;
}

/// persona 段补丁（字段级三态；`Tri.clear()` = 清空该字段）。
class PersonaSettingsPatch {
  const PersonaSettingsPatch({
    this.systemPrompt,
    this.maxHistoryPairs,
    this.name,
    this.description,
    this.personality,
    this.scenario,
    this.first,
  });

  final Tri<String>? systemPrompt;
  final Tri<int>? maxHistoryPairs;
  final Tri<String>? name;
  final Tri<String>? description;
  final Tri<String>? personality;
  final Tri<String>? scenario;
  final Tri<String>? first;

  Map<String, Object?> toJson() {
    final Map<String, Object?> out = <String, Object?>{};
    putTri<String>(out, 'system_prompt', systemPrompt);
    putTri<int>(out, 'max_history_pairs', maxHistoryPairs);
    putTri<String>(out, 'name', name);
    putTri<String>(out, 'description', description);
    putTri<String>(out, 'personality', personality);
    putTri<String>(out, 'scenario', scenario);
    putTri<String>(out, 'first', first);
    return out;
  }

  bool get isEmpty => toJson().isEmpty;
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
  const SettingsPatch({this.llm, this.tts, this.persona, this.devMode});

  final LlmSettingsPatch? llm;
  final TtsSettingsPatch? tts;
  final PersonaSettingsPatch? persona;

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
    this.errorCode,
    this.errorMessage,
  });

  final bool ok;

  /// 往返耗时（毫秒）；服务端用 `u128`，这里收成 int。
  final int? latencyMs;

  /// 服务端回显的模型/音色名——用来确认「打到的确实是配置里那个」。
  final String? modelEcho;

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

/// 宽容取对象：不是对象就返回 null（交给各自的 `fromJson` 走默认值）。
Map<String, Object?>? _obj(Object? value) {
  if (value is! Map) return null;
  final Map<String, Object?> out = <String, Object?>{};
  value.forEach((Object? k, Object? v) {
    if (k is String) out[k] = v;
  });
  return out;
}
