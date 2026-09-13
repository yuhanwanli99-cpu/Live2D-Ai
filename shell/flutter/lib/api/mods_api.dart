/// Mod 管理 API（`GET /api/v1/mods` + `POST /api/v1/mods/{id}/{action}`）。
///
/// 契约来源：`crates/live2d-ai-desktop/src/web_api/mods_routes.rs`（读源码确认）。
///
/// # 「启用」这件事在 Mod 边界上是敏感的
///
/// Mod 是本项目**唯一**允许往里堆功能的地方（用户裁决：「堆砌就往 mod 方向
/// 仅提供接口即可」）。所以这个面板的职责是**如实呈现**每个 Mod 的状态与
/// 配置入口，而**不是**替 Mod 创造核心能力。
///
/// # 2026-09-13（rc.4 M2）：`settings_spec` 发现闭环
///
/// `GET /api/v1/mods` 现在可以带出每个 Mod 自描述的 `settings_spec`：
/// 前端**不再为每个 Mod 硬编码表单**，而是按 spec 渲染 Bool / String /
/// Number / Select 四种基础控件（`crates/live2d-ai-mod-system` 的注册契约）。
/// 旧 Mod 没有 spec → `settingsSpec == null`，面板保持「只有开关」的现状。
///
/// 两条容错纪律：
/// - **未知 kind 跳过、不崩**：服务端将来新增类型时，旧前端只是少一个字段；
/// - `secret=true` 的 String 服务端**不回值**，界面渲染为密码框，空 = 不修改。
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'api_client.dart';

/// `settings_spec.fields[].options[]` 的一项（select 用）。
class ModSelectOption {
  const ModSelectOption({required this.value, required this.label});

  final String value;
  final String label;
}

/// 设置字段的基础类型。**协议里将来可能有更多**——解析时未知取值直接跳过。
enum ModFieldKind { bool, string, number, select }

/// `settings_spec.fields[]` 的一项（四种基础类型的最小子集）。
///
/// 用一个扁平类而不是四个子类：字段的公共部分（key/label/default）远多于
/// 差异部分，扁平结构让「未知 kind 跳过」与「按 kind 分派」都只需一处判断。
class ModSettingField {
  const ModSettingField({
    required this.kind,
    required this.key,
    required this.label,
    this.secret = false,
    this.defaultValue,
    this.min,
    this.max,
    this.options = const <ModSelectOption>[],
  });

  final ModFieldKind kind;
  final String key;
  final String label;

  /// 仅 String 有意义：true = 密钥，服务端不回值、界面用密码框。
  final bool secret;

  /// spec 声明的默认值（config 缺该键时用它回填）。
  final Object? defaultValue;

  /// 仅 number 有意义。
  final num? min;
  final num? max;

  /// 仅 select 有意义。
  final List<ModSelectOption> options;

  /// 解析一项；**不认识的 kind / 无法渲染的项返回 null**，由调用方跳过。
  static ModSettingField? fromJson(Map<String, Object?> json) {
    final String key = _str(json['key']);
    if (key.isEmpty) return null;
    final ModFieldKind? kind = switch (_str(json['kind'])) {
      'bool' => ModFieldKind.bool,
      'string' => ModFieldKind.string,
      'number' => ModFieldKind.number,
      'select' => ModFieldKind.select,
      // 未知 kind：跳过，不崩。旧前端 + 新服务端是正常组合。
      _ => null,
    };
    if (kind == null) return null;

    final List<ModSelectOption> options = <ModSelectOption>[];
    final Object? rawOptions = json['options'];
    if (rawOptions is List) {
      for (final Object? raw in rawOptions) {
        if (raw is! Map) continue;
        final Map<String, Object?> o = _stringKeys(raw);
        final String value = _str(o['value']);
        if (value.isEmpty) continue;
        final String label = _str(o['label']);
        options.add(ModSelectOption(
          value: value,
          label: label.isEmpty ? value : label,
        ));
      }
    }
    // 没有可选项的 select 渲染不出来（Dropdown 会断言失败）——跳过。
    if (kind == ModFieldKind.select && options.isEmpty) return null;

    final String label = _str(json['label']);
    return ModSettingField(
      kind: kind,
      key: key,
      label: label.isEmpty ? key : label,
      secret: json['secret'] == true,
      defaultValue: json['default'],
      min: json['min'] is num ? json['min']! as num : null,
      max: json['max'] is num ? json['max']! as num : null,
      options: options,
    );
  }
}

/// `settings_spec`：Mod 自描述的表单。
class ModSettingsSpec {
  const ModSettingsSpec({
    required this.modId,
    required this.title,
    required this.version,
    required this.fields,
  });

  final String modId;
  final String title;
  final int version;
  final List<ModSettingField> fields;

  /// 解析 `spec` 对象；**不是对象返回 null**（旧 Mod 没有 spec）。
  static ModSettingsSpec? fromJson(Map<String, Object?>? json) {
    if (json == null) return null;
    final List<ModSettingField> fields = <ModSettingField>[];
    final Object? rawFields = json['fields'];
    if (rawFields is List) {
      for (final Object? raw in rawFields) {
        if (raw is! Map) continue;
        final ModSettingField? f = ModSettingField.fromJson(_stringKeys(raw));
        if (f != null) fields.add(f);
      }
    }
    return ModSettingsSpec(
      modId: _str(json['mod_id']),
      title: _str(json['title']),
      version: _int(json['version']),
      fields: fields,
    );
  }
}

/// `POST /api/v1/mods/{id}/config` 的结果。
///
/// 服务端回 `{"ok":true,"restarted":true}` 或 `{"ok":true,"enabled":false}`。
class ModConfigResult {
  const ModConfigResult({
    required this.ok,
    this.restarted = false,
    this.enabled,
  });

  final bool ok;

  /// 配置生效需要服务端重启该 Mod。
  final bool restarted;

  /// 保存后该 Mod 是否仍启用；`null` = 服务端没回这个键。
  final bool? enabled;
}

/// `GET /api/v1/mods` 的列表项。
class ModInfo {
  const ModInfo({
    required this.id,
    required this.name,
    required this.version,
    required this.apiVersion,
    required this.enabled,
    required this.status,
    this.config = const <String, Object?>{},
    this.settingsSpec,
  });

  final String id;
  final String name;
  final String version;
  final int apiVersion;

  /// 是否已启用。
  final bool enabled;

  /// 服务端上报的运行状态（`running` / `disabled` / 其它）。
  final String status;

  /// 服务端当前落盘的配置（表单回填用；密钥字段不在里面）。
  final Map<String, Object?> config;

  /// 自描述表单；**旧 Mod 为 null**（面板只显示开关）。
  final ModSettingsSpec? settingsSpec;

  /// 是否正在运行。
  bool get isRunning => status == 'running';

  /// 状态的中文标签（**每个状态都要有文字**，不靠颜色）。
  String get statusLabel => switch (status) {
    'running' => '运行中',
    'disabled' => '已停用',
    'error' => '出错',
    'starting' => '启动中',
    _ => status.isEmpty ? '未知' : status,
  };

  factory ModInfo.fromJson(Map<String, Object?> json) => ModInfo(
    id: _str(json['id']),
    name: _str(json['name']),
    version: _str(json['version']),
    apiVersion: _int(json['api_version']),
    enabled: json['enabled'] == true,
    status: _str(json['status']),
    config: _obj(json['config']) ?? const <String, Object?>{},
    settingsSpec: ModSettingsSpec.fromJson(_obj(json['settings_spec'])),
  );
}

class ModsApi {
  ModsApi({String? base, http.Client? client})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String _base;
  final http.Client _client;

  Uri _uri(String path) => Uri.parse('$_base$path');

  /// `GET /api/v1/mods`。
  Future<List<ModInfo>> list() async {
    final http.Response response = await _guard(
      () => _client.get(_uri('/api/v1/mods')),
    );
    if (response.statusCode != 200) throw _error(response);
    final Object? raw = _decode(response.body)['mods'];
    if (raw is! List) return const <ModInfo>[];
    return raw
        .whereType<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> m) => ModInfo.fromJson(_stringKeys(m)))
        .where((ModInfo m) => m.id.isNotEmpty)
        .toList();
  }

  /// `POST /api/v1/mods/{id}/{action}`；`action` ∈ `enable` / `disable`。
  Future<void> setEnabled(String id, bool enabled) async {
    final http.Response response = await _guard(
      () => _client.post(
        _uri('/api/v1/mods/${Uri.encodeComponent(id)}/${enabled ? 'enable' : 'disable'}'),
      ),
    );
    if (response.statusCode != 200) throw _error(response);
  }

  /// `POST /api/v1/mods/{id}/config`，body `{"config": {…}}`。
  ///
  /// **必须显式带 `Content-Type: application/json`**：服务端对 mutating 请求
  /// 校验 Content-Type 与 loopback Origin（浏览器会自动带 Origin；
  /// Content-Type 得由客户端给，缺了就是 415/403）。
  Future<ModConfigResult> setConfig(
    String id,
    Map<String, Object?> config,
  ) async {
    final http.Response response = await _guard(
      () => _client.post(
        _uri('/api/v1/mods/${Uri.encodeComponent(id)}/config'),
        headers: const <String, String>{'Content-Type': 'application/json'},
        body: jsonEncode(<String, Object?>{'config': config}),
      ),
    );
    if (response.statusCode != 200) throw _error(response);
    final Map<String, Object?> body = _decode(response.body);
    return ModConfigResult(
      ok: body['ok'] != false,
      restarted: body['restarted'] == true,
      enabled: body['enabled'] is bool ? body['enabled']! as bool : null,
    );
  }

  Future<http.Response> _guard(Future<http.Response> Function() run) async {
    try {
      return await run();
    } catch (error) {
      throw ApiException('network_error', '无法连接后端：$error');
    }
  }

  ApiException _error(http.Response response) {
    final Map<String, Object?> body = _decode(response.body);
    final Object? error = body['error'];
    if (error is Map) {
      return ApiException(
        _str(error['code']).isEmpty ? 'error' : _str(error['code']),
        _str(error['message']),
        status: response.statusCode,
      );
    }
    return ApiException(
      'http_${response.statusCode}',
      response.body.isEmpty ? '请求失败' : response.body,
      status: response.statusCode,
    );
  }

  void dispose() => _client.close();
}

String _normalize(String raw) {
  var value = raw.trim();
  while (value.endsWith('/')) {
    value = value.substring(0, value.length - 1);
  }
  return value.isEmpty ? Uri.base.origin : value;
}

Map<String, Object?> _decode(String body) {
  try {
    final Object? decoded = jsonDecode(body);
    if (decoded is Map) return _stringKeys(decoded);
  } catch (_) {
    // 坏 JSON → 空对象。
  }
  return const <String, Object?>{};
}

Map<String, Object?>? _obj(Object? value) {
  if (value is! Map) return null;
  return _stringKeys(value);
}

Map<String, Object?> _stringKeys(Map<Object?, Object?> raw) {
  final Map<String, Object?> out = <String, Object?>{};
  raw.forEach((Object? k, Object? v) {
    if (k is String) out[k] = v;
  });
  return out;
}

String _str(Object? value) => value is String ? value : '';
int _int(Object? value) => value is num ? value.toInt() : 0;
