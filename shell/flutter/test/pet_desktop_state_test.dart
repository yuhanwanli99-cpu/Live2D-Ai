/// Wave 3 轨 D：pet-desktop **软闭环**（仅状态面）——Flutter 侧的闭环证据。
///
/// # 这份测试在证明什么（逐条对齐 `PARALLEL-WAVE3-2026-09-14.md` §3D）
///
/// 1. `ModsApi.state(id)` 打的是 `GET /api/v1/mods/{id}/state`，并解析
///    `{id, enabled, state}`；`503 state_unavailable` / `404 not_found`
///    **两种失败分开**（界面处置不同，不能都当「不存在」）。
/// 2. 「Mod 管理」展开卡片能看到 `always_on_top` / `click_through` / `opacity` /
///    `voice_active` / `window`（`opened=false` + `reason`），且**逐字**写出
///    「窗口未开（原生壳休眠），此面仅状态」。
/// 3. **配置热更新**：保存配置 → 重取 state → 字段跟着变（注入 fake，零网络）。
///
/// 契约真源：`crates/live2d-ai-desktop/src/web_api/mods_routes.rs`
/// （`handle_mod_state_get`）+ `docs/architecture/pet-desktop-mod-v0.md` §4/§5。
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

http.Response jsonResponse(String body, int status) => http.Response.bytes(
  utf8.encode(body),
  status,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

/// pet-desktop 的真实快照形状（键序 = serde_json 默认的字母序）。
String petStateBody({
  bool enabled = true,
  bool alwaysOnTop = true,
  bool clickThrough = false,
  double opacity = 0.95,
  bool voiceActive = false,
}) => jsonEncode(<String, Object?>{
  'id': 'pet-desktop',
  'enabled': enabled,
  'state': <String, Object?>{
    'always_on_top': alwaysOnTop,
    'click_through': clickThrough,
    'opacity': opacity,
    'voice_active': voiceActive,
    'window': <String, Object?>{
      'opened': false,
      'reason': 'native_shell_dormant',
    },
  },
});

ModSettingField _field(
  ModFieldKind kind,
  String key,
  String label, {
  Object? defaultValue,
  num? min,
  num? max,
}) => ModSettingField(
  kind: kind,
  key: key,
  label: label,
  defaultValue: defaultValue,
  min: min,
  max: max,
);

/// 与 Rust `pet_desktop_settings_spec()` 同形的三字段 schema。
ModInfo petDesktopMod({Map<String, Object?>? config}) => ModInfo(
  id: 'pet-desktop',
  name: '桌宠窗口',
  version: '0.1.0',
  apiVersion: 1,
  enabled: true,
  status: 'running',
  config: config ??
      const <String, Object?>{
        'always_on_top': true,
        'click_through': false,
        'opacity': 0.95,
      },
  settingsSpec: ModSettingsSpec(
    modId: 'pet-desktop',
    title: '桌宠窗口',
    version: 1,
    fields: <ModSettingField>[
      _field(ModFieldKind.bool, 'always_on_top', '总在最前', defaultValue: true),
      _field(ModFieldKind.bool, 'click_through', '点击穿透', defaultValue: false),
      _field(ModFieldKind.number, 'opacity', '窗口不透明度（0.1–1.0）', min: 0.1, max: 1.0),
    ],
  ),
);

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

Future<void> _expand(WidgetTester tester, String title) async {
  await tester.tap(find.text(title));
  await tester.pumpAndSettle();
}

Future<void> _tapSave(WidgetTester tester) async {
  final Finder save = find.text('保存');
  await tester.ensureVisible(save);
  await tester.pumpAndSettle();
  await tester.tap(save);
  await tester.pumpAndSettle();
}

/// 读「运行态」块里某个标签对应的值（标签与值在同一个 Row 内）。
String _stateValue(WidgetTester tester, String label) {
  final Finder row = find
      .ancestor(of: find.text('$label：'), matching: find.byType(Row))
      .first;
  final List<Text> texts = tester.widgetList<Text>(
    find.descendant(of: row, matching: find.byType(Text)),
  ).toList();
  return texts.last.data ?? '';
}

void main() {
  group('ModsApi.state：端点 / 解析 / 两种失败分开', () {
    test('GET /api/v1/mods/pet-desktop/state 并解析 id/enabled/state', () async {
      late http.Request seen;
      final ModsApi api = ModsApi(
        base: 'http://127.0.0.1:18080',
        client: MockClient((http.Request r) async {
          seen = r;
          return jsonResponse(petStateBody(), 200);
        }),
      );
      final ModStateResult result = await api.state('pet-desktop');
      expect(seen.method, 'GET');
      expect(seen.url.path, '/api/v1/mods/pet-desktop/state');
      expect(result.id, 'pet-desktop');
      expect(result.enabled, isTrue);
      expect(result.state['always_on_top'], true);
      expect(result.state['click_through'], false);
      expect(result.state['opacity'], 0.95);
      expect(result.state['voice_active'], false);
      expect(result.state['window'], <String, Object?>{
        'opened': false,
        'reason': 'native_shell_dormant',
      });
    });

    test('503 state_unavailable 透传错误码（不是「不存在」）', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient(
          (_) async => jsonResponse(
            '{"error":{"code":"state_unavailable","message":"未启用"}}',
            503,
          ),
        ),
      );
      expect(
        () => api.state('pet-desktop'),
        throwsA(
          isA<ApiException>()
              .having((ApiException e) => e.code, 'code', 'state_unavailable')
              .having((ApiException e) => e.status, 'status', 503),
        ),
      );
    });

    test('404 not_found 透传错误码', () async {
      final ModsApi api = ModsApi(
        base: 'http://x',
        client: MockClient(
          (_) async => jsonResponse(
            '{"error":{"code":"not_found","message":"不在注册表"}}',
            404,
          ),
        ),
      );
      expect(
        () => api.state('nope'),
        throwsA(isA<ApiException>().having((ApiException e) => e.code, 'code', 'not_found')),
      );
    });
  });

  group('运行态文案：纯函数（软闭环的那句话）', () {
    test('window 休眠 → 逐字「窗口未开（原生壳休眠），此面仅状态」', () {
      expect(
        formatWindowState(const <String, Object?>{
          'opened': false,
          'reason': kWindowReasonNativeShellDormant,
        }),
        '窗口未开（原生壳休眠），此面仅状态',
      );
      expect(
        formatWindowState(const <String, Object?>{'opened': true}),
        '已打开',
      );
      expect(
        formatWindowState(const <String, Object?>{
          'opened': false,
          'reason': 'some_future_reason',
        }),
        '未打开（some_future_reason）',
      );
    });

    test('reason 常量与 Rust 侧逐字一致（跨语言守卫）', () {
      expect(kWindowReasonNativeShellDormant, 'native_shell_dormant');
    });

    test('已知字段有中文标签，未知字段不隐藏、不丢', () {
      expect(modStateLabel('always_on_top'), '总在最前');
      expect(modStateLabel('click_through'), '点击穿透');
      expect(modStateLabel('opacity'), '不透明度');
      expect(modStateLabel('voice_active'), '语音活跃');
      expect(modStateLabel('window'), '窗口');
      expect(modStateLabel('future_field'), 'future_field');

      final Map<String, Object?> state = <String, Object?>{
        'voice_active': false,
        'future_field': 7,
        'always_on_top': true,
      };
      expect(orderedModStateKeys(state), <String>[
        'always_on_top',
        'voice_active',
        'future_field',
      ]);
      expect(formatModStateValue('always_on_top', true), '开');
      expect(formatModStateValue('voice_active', false), '关');
      expect(formatModStateValue('opacity', 0.3), '0.3');
      expect(formatModStateValue('opacity', 1.0), '1');
      expect(formatModStateValue('note', ''), '（空）');
      expect(formatModStateValue('items', <Object?>[1, 2]), '2 项');
    });
  });

  group('Mod 管理：展开运行态 + 配置热更新', () {
    testWidgets('展开 pet-desktop → 五个字段 + 仅状态文案', (WidgetTester tester) async {
      final List<String> asked = <String>[];
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: <ModInfo>[petDesktopMod()],
            loading: false,
            onLoadState: (String id) async {
              asked.add(id);
              return ModStateResult(
                id: id,
                enabled: true,
                state: <String, Object?>{
                  'always_on_top': true,
                  'click_through': false,
                  'opacity': 0.95,
                  'voice_active': false,
                  'window': <String, Object?>{
                    'opened': false,
                    'reason': kWindowReasonNativeShellDormant,
                  },
                },
              );
            },
          ),
        ),
      );
      await _expand(tester, '桌宠窗口');

      expect(asked, <String>['pet-desktop'], reason: '展开才懒加载一次');
      expect(find.text('运行态（只读）'), findsOneWidget);
      expect(find.text('刷新运行态'), findsOneWidget);
      expect(_stateValue(tester, '总在最前'), '开');
      expect(_stateValue(tester, '点击穿透'), '关');
      expect(_stateValue(tester, '不透明度'), '0.95');
      expect(_stateValue(tester, '语音活跃'), '关');
      expect(
        _stateValue(tester, '窗口'),
        '窗口未开（原生壳休眠），此面仅状态',
      );
    });

    testWidgets('保存配置 → 重取 state → 字段跟着变（热更新）', (WidgetTester tester) async {
      int calls = 0;
      Map<String, Object?>? saved;
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: <ModInfo>[petDesktopMod()],
            loading: false,
            onSaveConfig: (String id, Map<String, Object?> config) async {
              saved = config;
              return const ModConfigResult(ok: true, restarted: true);
            },
            onLoadState: (String id) async {
              calls++;
              // 第一次 = 展开时的旧配置；保存后第二次 = 服务端归一化后的新值。
              return ModStateResult(
                id: id,
                enabled: true,
                state: calls == 1
                    ? <String, Object?>{
                        'always_on_top': true,
                        'click_through': false,
                        'opacity': 0.95,
                        'voice_active': false,
                        'window': <String, Object?>{
                          'opened': false,
                          'reason': kWindowReasonNativeShellDormant,
                        },
                      }
                    : <String, Object?>{
                        'always_on_top': false,
                        'click_through': true,
                        'opacity': 0.3,
                        'voice_active': true,
                        'window': <String, Object?>{
                          'opened': false,
                          'reason': kWindowReasonNativeShellDormant,
                        },
                      },
              );
            },
          ),
        ),
      );
      await _expand(tester, '桌宠窗口');
      expect(_stateValue(tester, '总在最前'), '开');
      expect(_stateValue(tester, '点击穿透'), '关');
      expect(_stateValue(tester, '不透明度'), '0.95');

      // 改「总在最前」→ 保存。
      // Switch 顺序（树序）：0 = 卡片头的「启用」开关，1 = 总在最前，2 = 点击穿透。
      await tester.tap(find.byType(Switch).at(1));
      await tester.pumpAndSettle();
      await _tapSave(tester);

      expect(saved!['always_on_top'], false, reason: '保存发出去的是表单当前值');
      expect(find.textContaining('已保存'), findsOneWidget);
      expect(calls, 2, reason: '保存成功后必须重取运行态');
      expect(_stateValue(tester, '总在最前'), '关', reason: '字段跟着配置变');
      expect(_stateValue(tester, '点击穿透'), '开');
      expect(_stateValue(tester, '不透明度'), '0.3');
      expect(_stateValue(tester, '语音活跃'), '开');
      expect(
        _stateValue(tester, '窗口'),
        '窗口未开（原生壳休眠），此面仅状态',
        reason: '配置变了窗口依旧休眠（软闭环的边界）',
      );
    });

    testWidgets('503 → 说「暂时读不到」并带码，不谎报成功', (WidgetTester tester) async {
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: <ModInfo>[petDesktopMod()],
            loading: false,
            onLoadState: (String id) async =>
                throw const ApiException('state_unavailable', '未启用', status: 503),
          ),
        ),
      );
      await _expand(tester, '桌宠窗口');
      expect(find.textContaining('运行态暂时读不到'), findsOneWidget);
      expect(find.textContaining('state_unavailable'), findsOneWidget);
      expect(find.text('运行态（只读）'), findsOneWidget);
    });
  });
}
