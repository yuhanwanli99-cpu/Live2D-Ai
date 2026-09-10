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
///
/// # 这里为什么**没有** `invoke`（2026-09-11 用户裁决）
///
/// 本文件曾经还有 `POST /api/v1/commands/{id}/invoke`（手动触发）。
/// 本轮边界收窄：手动触发入口移出成品，写入路径一并删除，
/// 实现见分支 `archive/action-trigger-p5` 与
/// `docs/design/web-action-trigger-archive.md`。
/// **目录读取保留**——动作日志要靠它把协议名翻成中文名。
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'api_client.dart';

/// 一条动作指令（服务端 DTO 的忠实镜像）。
///
/// 字段比当前前端用到的多（`kind` / `step_count` / 强度 schema 等）——
/// 这是**刻意的**：它是协议形状，不是「需要什么抄什么」的裁剪视图，
/// 少一个字段就会让下一次接线重新猜服务端发的是什么。
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

  void dispose() => _client.close();
}

int? _intOrNull(Object? value) {
  if (value is num) return value.toInt();
  if (value is String) return int.tryParse(value);
  return null;
}

String _str(Object? value) => value is String ? value : '';
