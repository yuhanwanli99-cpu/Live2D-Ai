/// `GET /api/v1/commands`：动作目录（**中文名的唯一权威来源**）。
///
/// # 为什么前端要取它，而不是内建一张映射表
///
/// 动作的协议名（`nod` / `shake_no` / …）与中文名（点头 / 摇头 / …）都定义在
/// Rust 侧的 `web_api/command_registry.rs`，并由这个端点下发。在 Flutter 侧
/// 再抄一份表有两个后果：服务端加/改动作时前端**静默**显示内部 id；以及
/// AGENTS.md 明令禁止的「在 Flutter 侧复制核心逻辑」。
///
/// 缺省行为（取不到目录）是**显示协议名**——看得懂，且绝不编造。
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'api_client.dart';

/// 一条动作指令。
class CommandInfo {
  const CommandInfo({
    required this.id,
    required this.name,
    required this.description,
    required this.targetAction,
    required this.kind,
    required this.allowedSources,
    this.strengthDefault,
    this.strengthMin,
    this.strengthMax,
    this.stepCount,
  });

  /// 指令 id（如 `live2d_perform_action_nod`）——`POST /api/v1/commands/{id}/invoke`
  /// 用的就是它。
  final String id;

  /// 中文短名（「点头」）。**这就是动作日志里显示的名字。**
  final String name;

  /// 中文长描述（设置/工具提示用）。
  final String description;

  /// 该指令最终表演的动作协议名（如 `nod`）；多步指令可能为 `null`。
  final String? targetAction;

  /// `single`（单动作）或 `sequence`（编排）。
  final String kind;

  /// 允许的触发来源（`user_command` / `llm_tool` / `rule_fallback`）。
  final List<String> allowedSources;

  /// 强度参数的默认/最小/最大（来自 `params_schema`；缺失为 `null`）。
  final int? strengthDefault;
  final int? strengthMin;
  final int? strengthMax;

  /// 编排指令的步数（单动作为 1）。
  final int? stepCount;

  /// 是否可被 UI 直接触发（`allowed_sources` 含 `user_command`）。
  ///
  /// 这一条决定动作网格里哪些卡片可点：只允许 LLM 触发的指令不该出现在
  /// 用户可点的网格里（否则点下去会被 core 拒绝，用户看到「点了没反应」）。
  bool get userInvocable => allowedSources.contains('user_command');

  /// 强度三档可用性：schema 给的上界至少到 3 才显示第三档。
  bool get supportsHighStrength => (strengthMax ?? 3) >= 3;

  factory CommandInfo.fromJson(Map<String, Object?> json) {
    final Object? schema = json['params_schema'];
    int? strengthDefault;
    int? strengthMin;
    int? strengthMax;
    if (schema is Map) {
      final Object? properties = schema['properties'];
      if (properties is Map) {
        final Object? strength = properties['strength'];
        if (strength is Map) {
          strengthDefault = _intOrNull(strength['default']);
          strengthMin = _intOrNull(strength['minimum']);
          strengthMax = _intOrNull(strength['maximum']);
        }
      }
    }
    return CommandInfo(
      id: _str(json['id']),
      name: _str(json['name']),
      description: _str(json['description']),
      targetAction: json['target_action'] is String
          ? json['target_action']! as String
          : null,
      kind: _str(json['kind']),
      allowedSources: json['allowed_sources'] is List
          ? (json['allowed_sources']! as List).whereType<String>().toList()
          : const <String>[],
      strengthDefault: strengthDefault,
      strengthMin: strengthMin,
      strengthMax: strengthMax,
      stepCount: _intOrNull(json['step_count']),
    );
  }
}

/// `GET /api/v1/commands` 的结果。
class CommandCatalog {
  const CommandCatalog(this.commands);

  const CommandCatalog.empty() : commands = const <CommandInfo>[];

  final List<CommandInfo> commands;

  bool get isEmpty => commands.isEmpty;

  /// 动作协议名 → 中文名。**缺失时查不到**（调用方回退到协议名）。
  ///
  /// 多步/无 `target_action` 的指令不进这张表：它们没有单一动作名可对应。
  Map<String, String> get labelByAction {
    final Map<String, String> out = <String, String>{};
    for (final CommandInfo command in commands) {
      final String? action = command.targetAction;
      if (action == null || action.isEmpty) continue;
      // 同名动作多条指令时**先到者胜**（服务端顺序稳定）；不覆盖。
      out.putIfAbsent(action, () => command.name);
    }
    return out;
  }

  /// 可被 UI 触发的动作（含中文名），按服务端返回顺序。
  List<CommandInfo> get userInvocable =>
      commands.where((CommandInfo c) => c.userInvocable).toList();

  factory CommandCatalog.fromJson(Map<String, Object?> json) {
    final Object? raw = json['commands'];
    if (raw is! List) return const CommandCatalog.empty();
    return CommandCatalog(
      raw
          .whereType<Map<Object?, Object?>>()
          .map(
            (Map<Object?, Object?> m) =>
                CommandInfo.fromJson(Map<String, Object?>.from(m)),
          )
          .where((CommandInfo c) => c.id.isNotEmpty)
          .toList(),
    );
  }
}

/// 动作目录客户端。
///
/// 与 [ApiClient] 分离的理由：目录是**只读、可缓存、失败可忽略**的资源，
/// 而 `ApiClient` 承担对话/设置这些「失败必须让用户知道」的动作。混在一起会
/// 让「取不到目录」被迫走同一套错误呈现。
class CommandsApi {
  CommandsApi({String? base, http.Client? client})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String _base;
  final http.Client _client;

  static String _normalize(String raw) {
    var value = raw.trim();
    while (value.endsWith('/')) {
      value = value.substring(0, value.length - 1);
    }
    return value.isEmpty ? Uri.base.origin : value;
  }

  /// 取动作目录。失败时抛 [ApiException]（由调用方决定是否忽略）。
  Future<CommandCatalog> fetchCommands() async {
    final http.Response response;
    try {
      response = await _client.get(Uri.parse('$_base/api/v1/commands'));
    } catch (error) {
      throw ApiException('network_error', '无法连接后端：$error');
    }
    if (response.statusCode != 200) {
      throw ApiException(
        'http_${response.statusCode}',
        '取动作目录失败',
        status: response.statusCode,
      );
    }
    try {
      final Object? decoded = jsonDecode(response.body);
      if (decoded is Map) {
        return CommandCatalog.fromJson(Map<String, Object?>.from(decoded));
      }
    } catch (_) {
      // 坏 JSON：当作空目录（回落显示协议名），**不抛**——目录是可选资源。
    }
    return const CommandCatalog.empty();
  }

  /// `POST /api/v1/commands/{id}/invoke` → 让 core 仲裁并触发。
  ///
  /// # 两个必须知道的事实
  ///
  /// 1. **重复点击会回 `accepted: true`**——服务端对「动作+强度+来源完全相同」
  ///    返回 `Dropped(AlreadyActive)` 幂等，但**响应里看不出来**（实测两次
  ///    invoke 的响应一字不差）。所以「同一动作正在播放」的提示**只能本地做**。
  /// 2. `strength` 越界是 **400 `invalid_params`**（实测 `9` → 400）。
  ///    分段按钮只给 1/2/3，前端不会发越界值；但错误路径仍要能显示。
  Future<CommandInvokeResult> invoke(String id, {int? strength}) async {
    final http.Response response;
    try {
      response = await _client.post(
        Uri.parse('$_base/api/v1/commands/${Uri.encodeComponent(id)}/invoke'),
        headers: const <String, String>{'Content-Type': 'application/json'},
        body: jsonEncode(<String, Object?>{'strength': ?strength}),
      );
    } catch (error) {
      throw ApiException('network_error', '无法连接后端：$error');
    }
    if (response.statusCode != 200) {
      final Map<String, Object?> body = _decode(response.body);
      final Object? err = body['error'];
      throw ApiException(
        err is Map && err['code'] is String
            ? err['code']! as String
            : 'http_${response.statusCode}',
        err is Map && err['message'] is String
            ? err['message']! as String
            : '触发失败',
        status: response.statusCode,
      );
    }
    return CommandInvokeResult.fromJson(_decode(response.body));
  }

  void dispose() => _client.close();

  /// 宽容取对象（坏 JSON → 空对象）。
  Map<String, Object?> _decode(String body) {
    try {
      final Object? decoded = jsonDecode(body);
      if (decoded is Map) {
        final Map<String, Object?> out = <String, Object?>{};
        decoded.forEach((Object? k, Object? v) {
          if (k is String) out[k] = v;
        });
        return out;
      }
    } catch (_) {
      // 坏 JSON → 空对象。
    }
    return const <String, Object?>{};
  }
}

/// `POST /api/v1/commands/{id}/invoke` 的响应（真实抓包形状）。
class CommandInvokeResult {
  const CommandInvokeResult({
    required this.accepted,
    required this.epoch,
    required this.commandId,
    required this.kind,
    required this.steps,
    this.action,
  });

  final bool accepted;
  final int epoch;
  final String commandId;

  /// `single` 或 `sequence`。
  final String kind;

  /// 编排的步数（单动作为 1）。
  final int steps;

  /// 最终表演的动作协议名（多步指令可能为 `null`）。
  final String? action;

  factory CommandInvokeResult.fromJson(Map<String, Object?> json) =>
      CommandInvokeResult(
        accepted: json['accepted'] == true,
        epoch: _intOrNull(json['epoch']) ?? 0,
        commandId: _str(json['command_id']),
        kind: _str(json['kind']),
        steps: _intOrNull(json['steps']) ?? 1,
        action: json['action'] is String ? json['action']! as String : null,
      );
}

int? _intOrNull(Object? value) {
  if (value is num) return value.toInt();
  if (value is String) return int.tryParse(value);
  return null;
}

String _str(Object? value) => value is String ? value : '';
