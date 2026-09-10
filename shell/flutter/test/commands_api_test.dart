import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/commands_api.dart';

/// **真实抓包**的 `GET /api/v1/commands` 响应（本机 v0.4.6，2026-09-10）。
///
/// **不要凭印象改写**：前端不做动作名映射表，中文名的唯一权威来源就是它。
/// 这份夹具的作用是让「服务端改了什么形状」在测试里立刻可见，
/// 而不是让前端悄悄显示起内部 id（`nod`）。
const String kRealCommandsJson = '''
{
 "commands": [
  {"id":"live2d_perform_action_nod","name":"点头","description":"表演点头动作（同意 / 应和）","params_schema":{"additionalProperties":false,"properties":{"strength":{"default":2,"description":"动作强度 1..=3（默认 2）","maximum":3,"minimum":1,"type":"integer"}},"type":"object"},"allowed_sources":["user_command"],"kind":"single","target_action":"nod","step_count":1},
  {"id":"live2d_perform_action_shake_no","name":"摇头","description":"表演摇头动作（否定）","params_schema":{"additionalProperties":false,"properties":{"strength":{"default":2,"description":"动作强度 1..=3（默认 2）","maximum":3,"minimum":1,"type":"integer"}},"type":"object"},"allowed_sources":["user_command"],"kind":"single","target_action":"shake_no","step_count":1},
  {"id":"live2d_perform_action_tilt","name":"歪头","description":"表演歪头动作（疑惑 / 好奇）","params_schema":{"additionalProperties":false,"properties":{"strength":{"default":2,"description":"动作强度 1..=3（默认 2）","maximum":3,"minimum":1,"type":"integer"}},"type":"object"},"allowed_sources":["user_command"],"kind":"single","target_action":"tilt","step_count":1},
  {"id":"live2d_perform_action_look_around","name":"环视","description":"表演环视 / 视线游移动作","params_schema":{"additionalProperties":false,"properties":{"strength":{"default":2,"description":"动作强度 1..=3（默认 2）","maximum":3,"minimum":1,"type":"integer"}},"type":"object"},"allowed_sources":["user_command"],"kind":"single","target_action":"look_around","step_count":1},
  {"id":"live2d_perform_action_listen","name":"倾听","description":"表演倾听姿态","params_schema":{"additionalProperties":false,"properties":{"strength":{"default":2,"description":"动作强度 1..=3（默认 2）","maximum":3,"minimum":1,"type":"integer"}},"type":"object"},"allowed_sources":["user_command"],"kind":"single","target_action":"listen","step_count":1},
  {"id":"live2d_perform_action_surprise","name":"惊讶","description":"表演惊讶动作","params_schema":{"additionalProperties":false,"properties":{"strength":{"default":2,"description":"动作强度 1..=3（默认 2）","maximum":3,"minimum":1,"type":"integer"}},"type":"object"},"allowed_sources":["user_command"],"kind":"single","target_action":"surprise","step_count":1}
 ]
}
''';

/// 造一个 JSON 响应。
///
/// **必须显式声明 `charset=utf-8`**：`http.Response(String body, int status)`
/// 默认按 latin1 编码 body，body 里一有中文就直接抛
/// `Invalid argument (string): Contains invalid characters`。真实链路之所以
/// 没问题，是因为服务端确实带了这个头（`web_api/responses.rs`）。
http.Response jsonResponse(String body, int status) => http.Response.bytes(
  utf8.encode(body),
  status,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

CommandsApi apiWith(MockClient client) =>
    CommandsApi(base: 'http://127.0.0.1:18080', client: client);

void main() {
  group('真实目录（实抓夹具）', () {
    test('解析出 6 个动作，中文名与协议名一一对应', () {
      final CommandCatalog catalog = CommandCatalog.fromJson(
        jsonDecode(kRealCommandsJson) as Map<String, Object?>,
      );
      expect(catalog.commands, hasLength(6));
      final Map<String, String> labels = catalog.labelByAction;
      expect(labels, <String, String>{
        'nod': '点头',
        'shake_no': '摇头',
        'tilt': '歪头',
        'look_around': '环视',
        'listen': '倾听',
        'surprise': '惊讶',
      });
    });

    test('每一条都能读出 id / name / target_action / 强度上下界', () {
      final CommandCatalog catalog = CommandCatalog.fromJson(
        jsonDecode(kRealCommandsJson) as Map<String, Object?>,
      );
      final CommandInfo nod = catalog.commands.first;
      expect(nod.id, 'live2d_perform_action_nod');
      expect(nod.name, '点头');
      expect(nod.targetAction, 'nod');
      expect(nod.kind, 'single');
      expect(nod.stepCount, 1);
      expect(nod.strengthDefault, 2);
      expect(nod.strengthMin, 1);
      expect(nod.strengthMax, 3);
      expect(nod.allowedSources, <String>['user_command']);
      expect(nod.userInvocable, isTrue);
      expect(nod.supportsHighStrength, isTrue);
    });

    test('userInvocable 只认 allowed_sources（不许 UI 点 LLM-only 的动作）', () {
      final CommandCatalog catalog = CommandCatalog.fromJson(<String, Object?>{
        'commands': <Object?>[
          <String, Object?>{
            'id': 'a',
            'name': '可点',
            'target_action': 'nod',
            'allowed_sources': <Object?>['user_command', 'llm_tool'],
          },
          <String, Object?>{
            'id': 'b',
            'name': '只给LLM',
            'target_action': 'tilt',
            'allowed_sources': <Object?>['llm_tool'],
          },
        ],
      });
      expect(catalog.userInvocable.map((CommandInfo c) => c.id), <String>['a']);
      expect(
        catalog.labelByAction.keys,
        containsAll(<String>['nod', 'tilt']),
        reason: '中文名映射不受「可不可点」影响——AI 触发的动作也要显示中文名',
      );
    });
  });

  group('宽容解析：目录是可选资源，坏数据不许打挂界面', () {
    test('缺 commands / 不是列表 / 空列表 → 空目录', () {
      expect(CommandCatalog.fromJson(const <String, Object?>{}).isEmpty, isTrue);
      expect(
        CommandCatalog.fromJson(const <String, Object?>{'commands': 'nope'}).isEmpty,
        isTrue,
      );
      expect(
        CommandCatalog.fromJson(const <String, Object?>{'commands': <Object?>[]}).isEmpty,
        isTrue,
      );
    });

    test('坏条目被跳过，好条目保留', () {
      final CommandCatalog catalog = CommandCatalog.fromJson(<String, Object?>{
        'commands': <Object?>[
          'not-an-object',
          <String, Object?>{'name': '没有 id'},
          <String, Object?>{'id': 'ok', 'name': '好', 'target_action': 'nod'},
        ],
      });
      expect(catalog.commands, hasLength(1));
      expect(catalog.commands.single.id, 'ok');
    });

    test('多步/无 target_action 的指令不进中文名表（没有单一动作可对应）', () {
      final CommandCatalog catalog = CommandCatalog.fromJson(<String, Object?>{
        'commands': <Object?>[
          <String, Object?>{
            'id': 'seq',
            'name': '组合',
            'kind': 'sequence',
            'target_action': null,
            'step_count': 3,
          },
        ],
      });
      expect(catalog.labelByAction, isEmpty);
      expect(catalog.commands.single.stepCount, 3);
    });

    test('同名动作多条指令 → 先到者胜（顺序稳定，不随机覆盖）', () {
      final CommandCatalog catalog = CommandCatalog.fromJson(<String, Object?>{
        'commands': <Object?>[
          <String, Object?>{'id': 'a', 'name': '甲', 'target_action': 'nod'},
          <String, Object?>{'id': 'b', 'name': '乙', 'target_action': 'nod'},
        ],
      });
      expect(catalog.labelByAction['nod'], '甲');
    });
  });

  group('HTTP 契约（MockClient，VM 可跑）', () {
    test('打的是 GET /api/v1/commands', () async {
      late http.Request seen;
      final CommandsApi api = apiWith(
        MockClient((http.Request request) async {
          seen = request;
          return jsonResponse(kRealCommandsJson, 200);
        }),
      );
      final CommandCatalog catalog = await api.fetchCommands();
      expect(seen.method, 'GET');
      expect(seen.url.path, '/api/v1/commands');
      expect(catalog.commands, hasLength(6));
    });

    test('base 尾斜杠被规范化（不会出现 //api）', () async {
      late Uri seen;
      final CommandsApi api = CommandsApi(
        base: 'http://127.0.0.1:18080/',
        client: MockClient((http.Request request) async {
          seen = request.url;
          return jsonResponse(kRealCommandsJson, 200);
        }),
      );
      await api.fetchCommands();
      expect(seen.path, '/api/v1/commands');
    });

    test('500 → 抛 ApiException（由调用方决定是否忽略）', () async {
      final CommandsApi api = apiWith(
        MockClient((http.Request request) async => jsonResponse('{}', 500)),
      );
      expect(
        () => api.fetchCommands(),
        throwsA(
          isA<ApiException>().having(
            (ApiException e) => e.status,
            'status',
            500,
          ),
        ),
      );
    });

    test('坏 JSON → 空目录，**不抛**（目录是可选资源）', () async {
      final CommandsApi api = apiWith(
        MockClient((http.Request request) async => jsonResponse('{not json', 200)),
      );
      expect((await api.fetchCommands()).isEmpty, isTrue);
    });

    test('网络层失败 → 抛 network_error', () async {
      final CommandsApi api = apiWith(
        MockClient((http.Request request) async => throw const SocketExceptionStub()),
      );
      expect(
        () => api.fetchCommands(),
        throwsA(isA<ApiException>().having((ApiException e) => e.code, 'code', 'network_error')),
      );
    });
  });
}

/// 一个最小的「连接失败」异常（不 import dart:io，保持 VM/Web 同构）。
class SocketExceptionStub implements Exception {
  const SocketExceptionStub();
  @override
  String toString() => 'connection refused';
}
