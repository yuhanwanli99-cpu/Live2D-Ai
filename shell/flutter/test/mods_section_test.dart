/// M2（rc.4）：Mod `settings_spec` 发现闭环——解析、按 spec 渲染、保存。
///
/// 契约来自计划 `PLAN-rc4-mod-product-chain-2026-09-13.md` §6：
/// `GET /api/v1/mods` 带 `settings_spec`；`POST /api/v1/mods/{id}/config`
/// 必须带 `Content-Type: application/json`（服务端校验 mutating 的 CT + Origin）。
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// **合成**夹具：一个带 spec 的 Mod + 若干容错项（未知 kind / 空 select）。
///
/// 它**不对应任何在册 crate**（id 是 `demo-mod`）：本文件测的是
/// `settings_spec` 的解析与渲染，与哪个 Mod 无关。原先这里写着 `demo-mod`
/// ——那个 crate 已于 W2-A 物理删除，留着一个已删 id 会让人以为
/// 「后端真的会回这个 Mod」（D6 要清的假绿灯形态）。
const String kModsWithSpecJson = '''
{"mods":[{"id":"demo-mod","name":"演示 Mod","version":"0.1.0",
"api_version":1,"status":"running","enabled":true,"config":{"port":11434},
"settings_spec":{"mod_id":"demo-mod","title":"演示 Mod","version":1,
"fields":[
 {"kind":"bool","key":"auto_start","label":"自动启动推理进程","default":true},
 {"kind":"string","key":"command","label":"推理进程命令","secret":false},
 {"kind":"string","key":"api_key","label":"密钥","secret":true},
 {"kind":"number","key":"port","label":"服务端口","min":1024,"max":65535},
 {"kind":"select","key":"mode","label":"模式",
  "options":[{"value":"chat","label":"对话"}]},
 {"kind":"future_kind","key":"x","label":"未来类型"},
 {"kind":"select","key":"empty","label":"空选择","options":[]}
]}}]}
''';

http.Response jsonResponse(String body, int status) => http.Response.bytes(
  utf8.encode(body),
  status,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

ModSettingField _field(
  ModFieldKind kind,
  String key,
  String label, {
  bool secret = false,
  Object? defaultValue,
  num? min,
  num? max,
  List<ModSelectOption> options = const <ModSelectOption>[],
}) => ModSettingField(
  kind: kind,
  key: key,
  label: label,
  secret: secret,
  defaultValue: defaultValue,
  min: min,
  max: max,
  options: options,
);

/// 本文件只测配置表单；运行态显式给一份空的 fake，避免 widget 在展开时走
/// 兜底自建的 `ModsApi()`（Wave 3 D 轨新增的运行态读取）。运行态那一面由
/// `mod_state_surface_test.dart` 覆盖（2026-10-05 D6 改名：原
/// `pet_desktop_state_test.dart` 的 pet-desktop 生产者已被物理删除）。
Future<ModStateResult> _stubState(String id) async =>
    ModStateResult(id: id, enabled: true, state: const <String, Object?>{});

ModInfo _modWithSpec() => ModInfo(
  id: 'demo-mod',
  name: '演示 Mod',
  version: '0.1.0',
  apiVersion: 1,
  enabled: true,
  status: 'running',
  config: const <String, Object?>{'port': 11434, 'auto_start': true},
  settingsSpec: ModSettingsSpec(
    modId: 'demo-mod',
    title: '演示 Mod',
    version: 1,
    fields: <ModSettingField>[
      _field(ModFieldKind.bool, 'auto_start', '自动启动推理进程', defaultValue: true),
      _field(ModFieldKind.string, 'command', '推理进程命令'),
      _field(ModFieldKind.string, 'api_key', '密钥', secret: true),
      _field(ModFieldKind.number, 'port', '服务端口', min: 1024, max: 65535),
      _field(
        ModFieldKind.select,
        'mode',
        '模式',
        options: const <ModSelectOption>[
          ModSelectOption(value: 'chat', label: '对话'),
        ],
      ),
    ],
  ),
);

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

Future<void> _expandFirstMod(WidgetTester tester) async {
  await tester.tap(find.text('演示 Mod'));
  await tester.pumpAndSettle();
}

/// 保存按钮在展开表单的最下面，800x600 的视口里通常在折叠线以下。
Future<void> _tapSave(WidgetTester tester) async {
  final Finder save = find.text('保存');
  await tester.ensureVisible(save);
  await tester.pumpAndSettle();
  await tester.tap(save);
  await tester.pumpAndSettle();
}

void main() {
  group('settings_spec 解析：未知 kind 跳过、不崩', () {
    test('字段类型 / secret / min-max / options 都解析出来', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient((_) async => jsonResponse(kModsWithSpecJson, 200)),
      );
      final ModInfo m = (await api.list()).single;
      expect(m.config['port'], 11434);
      final ModSettingsSpec spec = m.settingsSpec!;
      expect(spec.modId, 'demo-mod');
      expect(spec.title, '演示 Mod');
      expect(spec.version, 1);
      // 7 项里两项目无法渲染（未知 kind / 空 select）被跳过。
      expect(spec.fields.map((ModSettingField f) => f.key), <String>[
        'auto_start',
        'command',
        'api_key',
        'port',
        'mode',
      ]);
      expect(spec.fields[0].kind, ModFieldKind.bool);
      expect(spec.fields[0].defaultValue, true);
      expect(spec.fields[2].secret, isTrue);
      expect(spec.fields[3].min, 1024);
      expect(spec.fields[3].max, 65535);
      expect(spec.fields[4].options.single.value, 'chat');
    });

    test('旧 Mod 没有 settings_spec → settingsSpec 为 null（只显示开关）', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient(
          (_) async => jsonResponse(
            '{"mods":[{"id":"old","name":"旧","status":"disabled"}]}',
            200,
          ),
        ),
      );
      final ModInfo m = (await api.list()).single;
      expect(m.settingsSpec, isNull);
      expect(m.config, isEmpty);
    });
  });

  group('setConfig：端点 / Content-Type / body', () {
    test('打对路径与方法，body 是 {"config": …}', () async {
      late http.Request seen;
      final ModsApi api = ModsApi(
        base: 'http://127.0.0.1:18080',
        client: MockClient((http.Request r) async {
          seen = r;
          return jsonResponse('{"ok":true,"restarted":true}', 200);
        }),
      );
      final ModConfigResult result = await api.setConfig(
        'demo-mod',
        const <String, Object?>{'port': 12000},
      );
      expect(seen.method, 'POST');
      expect(seen.url.path, '/api/v1/mods/demo-mod/config');
      // 服务端对 mutating 请求校验 Content-Type——缺了就是 415/403。
      expect(seen.headers['Content-Type'], contains('application/json'));
      final Map<String, Object?> body =
          jsonDecode(seen.body) as Map<String, Object?>;
      expect(body.keys, <String>['config']);
      expect(
        (body['config']! as Map<String, Object?>)['port'],
        12000,
      );
      expect(result.ok, isTrue);
      expect(result.restarted, isTrue);
    });

    test('enabled:false 也能解析（服务端另一种成功形状）', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient(
          (_) async => jsonResponse('{"ok":true,"enabled":false}', 200),
        ),
      );
      final ModConfigResult result = await api.setConfig(
        'demo-mod',
        const <String, Object?>{'port': 1},
      );
      expect(result.restarted, isFalse);
      expect(result.enabled, isFalse);
    });

    test('400 带错误码 → ApiException（不吞成成功）', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient(
          (_) async => jsonResponse(
            '{"error":{"code":"bad_config","message":"端口非法"}}',
            400,
          ),
        ),
      );
      expect(
        () => api.setConfig('demo-mod', const <String, Object?>{}),
        throwsA(
          isA<ApiException>().having(
            (ApiException e) => e.code,
            'code',
            'bad_config',
          ),
        ),
      );
    });
  });

  group('Mod 面板：按 spec 渲染 + 保存', () {
    testWidgets('四种控件都画出来，保存把当前值发出去', (WidgetTester tester) async {
      String? savedId;
      Map<String, Object?>? savedConfig;
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: <ModInfo>[_modWithSpec()],
            loading: false,
            onLoadState: _stubState,
            onSaveConfig: (String id, Map<String, Object?> config) async {
              savedId = id;
              savedConfig = config;
              return const ModConfigResult(ok: true, restarted: true);
            },
          ),
        ),
      );
      await _expandFirstMod(tester);

      expect(find.text('自动启动推理进程'), findsOneWidget);
      expect(find.text('推理进程命令'), findsOneWidget);
      expect(find.text('服务端口'), findsOneWidget);
      expect(find.text('模式'), findsOneWidget);
      // 四种控件：开关 / 文本框 / 下拉。
      expect(find.byType(Switch), findsWidgets);
      expect(find.byType(TextField), findsWidgets);
      expect(find.byType(DropdownButton<String>), findsOneWidget);

      // 改端口 → 保存。
      final Finder portField = find.byWidgetPredicate(
        (Widget w) => w is TextField && w.controller?.text == '11434',
      );
      expect(portField, findsOneWidget);
      await tester.ensureVisible(portField);
      await tester.pumpAndSettle();
      await tester.enterText(portField, '12000');
      await _tapSave(tester);

      expect(savedId, 'demo-mod');
      expect(savedConfig!['port'], 12000);
      expect(savedConfig!['auto_start'], true);
      expect(savedConfig!['mode'], 'chat');
      // secret 留空 = 不修改：不能把空串写回去。
      expect(savedConfig!.containsKey('api_key'), isFalse);
      expect(find.textContaining('已保存'), findsOneWidget);
    });

    testWidgets('越界数字不回调（尊重 spec 的 min）', (WidgetTester tester) async {
      Map<String, Object?>? savedConfig;
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: <ModInfo>[_modWithSpec()],
            loading: false,
            onLoadState: _stubState,
            onSaveConfig: (String id, Map<String, Object?> config) async {
              savedConfig = config;
              return const ModConfigResult(ok: true);
            },
          ),
        ),
      );
      await _expandFirstMod(tester);
      final Finder portField = find.byWidgetPredicate(
        (w) => w is TextField && w.controller?.text == '11434',
      );
      await tester.ensureVisible(portField);
      await tester.pumpAndSettle();
      await tester.enterText(portField, '999'); // < min
      await _tapSave(tester);
      expect(savedConfig!['port'], 11434, reason: '越界输入不该写进配置');
    });

    testWidgets('secret 填了值就发出去', (WidgetTester tester) async {
      Map<String, Object?>? savedConfig;
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: <ModInfo>[_modWithSpec()],
            loading: false,
            onLoadState: _stubState,
            onSaveConfig: (String id, Map<String, Object?> config) async {
              savedConfig = config;
              return const ModConfigResult(ok: true);
            },
          ),
        ),
      );
      await _expandFirstMod(tester);
      final Finder secretField = find.byWidgetPredicate(
        (Widget w) => w is TextField && w.obscureText,
      );
      await tester.ensureVisible(secretField);
      await tester.pumpAndSettle();
      await tester.enterText(secretField, 'sk-1');
      await _tapSave(tester);
      expect(savedConfig!['api_key'], 'sk-1');
    });

    testWidgets('保存失败带错误码（不谎报成功）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: <ModInfo>[_modWithSpec()],
            loading: false,
            onLoadState: _stubState,
            onSaveConfig: (String id, Map<String, Object?> config) async {
              throw const ApiException('bad_config', '端口非法');
            },
          ),
        ),
      );
      await _expandFirstMod(tester);
      await _tapSave(tester);
      expect(find.textContaining('bad_config'), findsOneWidget);
      expect(find.textContaining('已保存'), findsNothing);
    });

    testWidgets('没有 onSaveConfig 时保存按钮禁用（不假装能保存）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: <ModInfo>[_modWithSpec()],
            loading: false,
            onLoadState: _stubState,
          ),
        ),
      );
      await _expandFirstMod(tester);
      final OutlinedButton button = tester.widget<OutlinedButton>(
        find.ancestor(
          of: find.text('保存'),
          matching: find.byType(OutlinedButton),
        ),
      );
      expect(button.onPressed, isNull);
    });

    testWidgets('无 spec 的旧 Mod：保持只有开关、没有保存按钮', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: const <ModInfo>[
              ModInfo(
                id: 'old',
                name: '旧 Mod',
                version: '0.1.0',
                apiVersion: 1,
                enabled: false,
                status: 'disabled',
              ),
            ],
            loading: false,
            onToggle: (String id, bool enabled) async {},
          ),
        ),
      );
      expect(find.text('旧 Mod'), findsOneWidget);
      expect(find.byType(Switch), findsOneWidget);
      expect(find.text('保存'), findsNothing);
    });
  });
}
