/// **通用 Mod 运行态面**的回归（`GET /api/v1/mods/{id}/state` + `ModsSection`
/// 的运行态块 + `state_json` 取值/取键的纯函数）。
///
/// # 文件名与真实范围（D6，2026-10-05 改写）
///
/// 这个文件原先是「director 软闭环」的 Flutter 证据（Rust 生产者
/// `live2d-ai-mod-pet-desktop` 与它的 `pet_desktop_settings_spec()`）。**该 crate
/// 已于 W2-A 物理删除**（`docs/architecture/ARCHIVED-mods.md` §2.4；恢复点是
/// tag `checkpoint/pre-d1-dormant`）⇒ 那份「跨语言契约」不再有 Rust 端可核，
/// 而手抄形状的夹具在 crate 删掉之后**照样绿**——这正是 D6 点名的假绿灯
///（「不得当覆盖证据」）。
///
/// 改法：不再声称校验已删的生产者，把这批断言放回它们真正的对象——
/// **前端仍在产品路径上的通用运行态面**。夹具的 Mod id 换成**在册**的
/// `director`，并由 [registeredModIds]（读 `main.rs` 的
/// `AVAILABLE_MOD_FACTORIES`，唯一真源）交叉核对：**crate 被删就红**，
/// 而不是继续绿着骗人。
///
/// state_json 的键仍是**通用协议键**（`kModStateLabels` 的兜底表：
/// `window` / `opened` / `reason` 等）——前端对**任意** Mod 的 state 都按这套
/// 渲染，夹具不是在抄某个 Mod 的真实产出。
///
/// `window.reason` 的跨语言逐字一致（曾经的 Rust 测试
/// `window_reason_string_is_stable_and_ascii`）随着 crate 一起冻结在
/// ARCHIVED-mods.md，不再由本文件声称。
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

import 'support/registered_mods.dart';

http.Response jsonResponse(String body, int status) => http.Response.bytes(
  utf8.encode(body),
  status,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

/// `ModsApi.state` 那条**真端点契约**用的 id：必须是**在册** Mod
/// （`AVAILABLE_MOD_FACTORIES` 里真的有它）。crate 被删掉而这里还写着它 ⇒
/// 「夹具的 Mod id 必须在册」那组会红（D6）。
const String modStateBodyId = 'director';

/// `GET /api/v1/mods/{id}/state` 的响应夹具：`{id, enabled, state}` +
/// **通用** `state_json` 键（前端对任意 Mod 的 state 都按 `kModStateLabels`
/// 这套渲染）。
///
/// **不是在抄某个 Mod 的真实产出**：键是通用兜底表的键；id 只用 [modStateBodyId]
/// 这个在册值，交叉核对在下面那组。
String modStateBody({
  bool enabled = true,
  bool alwaysOnTop = true,
  bool clickThrough = false,
  double opacity = 0.95,
  bool voiceActive = false,
}) => jsonEncode(<String, Object?>{
  'id': modStateBodyId,
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

/// **合成**的运行态夹具：`ModsSection` 只按 kind / 键渲染，与任何 crate 无关。
///
/// id 刻意**不占**任何在册 Mod：在册的五个都带专用产品面板（`kModPanels`），
/// 面板会改变展开后的树（导演甚至把通用运行态块整块关掉）——那测的就不是
/// 通用块了。用合成 id 时这里**没有任何后端声明**（ModsSection 是注入式的），
/// 所以不存在「删 crate 后仍绿」的假绿灯。
ModInfo modWithState({Map<String, Object?>? config}) => ModInfo(
  id: 'demo-mod',
  name: '演示 Mod',
  version: '0.1.0',
  apiVersion: 1,
  enabled: true,
  status: 'running',
  config:
      config ??
      const <String, Object?>{
        'always_on_top': true,
        'click_through': false,
        'opacity': 0.95,
      },
  settingsSpec: ModSettingsSpec(
    modId: 'demo-mod',
    title: '演示 Mod',
    version: 1,
    fields: <ModSettingField>[
      _field(ModFieldKind.bool, 'always_on_top', '总在最前', defaultValue: true),
      _field(ModFieldKind.bool, 'click_through', '点击穿透', defaultValue: false),
      _field(
        ModFieldKind.number,
        'opacity',
        '窗口不透明度（0.1–1.0）',
        min: 0.1,
        max: 1.0,
      ),
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
  final List<Text> texts = tester
      .widgetList<Text>(find.descendant(of: row, matching: find.byType(Text)))
      .toList();
  return texts.last.data ?? '';
}

void main() {
  group('夹具的 Mod id 必须**在册**（D6：crate 被删就该红）', () {
    test('registeredModIds 从 Rust 静态注册表解出全部在册 Mod', () {
      final Set<String> ids = registeredModIds();
      expect(ids, isNotEmpty, reason: '真源一个 id 都没解出来 —— 门禁在空转（不是「后端没有 Mod」）');
      expect(
        ids,
        containsAll(<String>[
          'external-input',
          'persona',
          'voice-input',
          'memory',
          'director',
        ]),
        reason: '`AVAILABLE_MOD_FACTORIES` 变了（删 / 改 Mod）就必须在这里看见',
      );
    });

    test('**契约夹具**指向的 id 在册（pet-desktop / local-llm 那种已删 crate 的夹具会红）', () {
      final Set<String> ids = registeredModIds();
      expect(ids, isNotEmpty, reason: '空集 = 上面那条已经在空转');
      // `modStateBody()` 是 `ModsApi.state` 那条**真端点契约**的夹具：
      // 它声称「后端会回这个形状」，所以它的 id 必须真的在册。
      expect(
        ids,
        contains(modStateBodyId),
        reason:
            '契约夹具 $modStateBodyId 不在 `AVAILABLE_MOD_FACTORIES` 里 ——'
            '这正是 D6 说的「删 Rust 后仍绿」的假绿灯，必须红',
      );
    });
  });

  group('ModsApi.state：端点 / 解析 / 两种失败分开', () {
    test(
      'GET /api/v1/mods/\${modStateBodyId}/state 并解析 id/enabled/state',
      () async {
        late http.Request seen;
        final ModsApi api = ModsApi(
          base: 'http://127.0.0.1:18080',
          client: MockClient((http.Request r) async {
            seen = r;
            return jsonResponse(modStateBody(), 200);
          }),
        );
        final ModStateResult result = await api.state(modStateBodyId);
        expect(seen.method, 'GET');
        expect(seen.url.path, '/api/v1/mods/$modStateBodyId/state');
        expect(result.id, modStateBodyId);
        expect(result.enabled, isTrue);
        expect(result.state['always_on_top'], true);
        expect(result.state['click_through'], false);
        expect(result.state['opacity'], 0.95);
        expect(result.state['voice_active'], false);
        expect(result.state['window'], <String, Object?>{
          'opened': false,
          'reason': 'native_shell_dormant',
        });
      },
    );

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
        () => api.state('director'),
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
        throwsA(
          isA<ApiException>().having(
            (ApiException e) => e.code,
            'code',
            'not_found',
          ),
        ),
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
      expect(formatWindowState(const <String, Object?>{'opened': true}), '已打开');
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
    testWidgets('展开（合成夹具、无专用面板）→ 五个字段 + 仅状态文案', (WidgetTester tester) async {
      final List<String> asked = <String>[];
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: <ModInfo>[modWithState()],
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
      await _expand(tester, '演示 Mod');

      expect(asked, <String>['demo-mod'], reason: '展开才懒加载一次');
      expect(find.text('运行态（只读）'), findsOneWidget);
      expect(find.text('刷新运行态'), findsOneWidget);
      expect(_stateValue(tester, '总在最前'), '开');
      expect(_stateValue(tester, '点击穿透'), '关');
      expect(_stateValue(tester, '不透明度'), '0.95');
      expect(_stateValue(tester, '语音活跃'), '关');
      expect(_stateValue(tester, '窗口'), '窗口未开（原生壳休眠），此面仅状态');
    });

    testWidgets('保存配置 → 重取 state → 字段跟着变（热更新）', (WidgetTester tester) async {
      int calls = 0;
      Map<String, Object?>? saved;
      await tester.pumpWidget(
        _wrap(
          ModsSection(
            mods: <ModInfo>[modWithState()],
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
      await _expand(tester, '演示 Mod');
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
            mods: <ModInfo>[modWithState()],
            loading: false,
            onLoadState: (String id) async => throw const ApiException(
              'state_unavailable',
              '未启用',
              status: 503,
            ),
          ),
        ),
      );
      await _expand(tester, '演示 Mod');
      expect(find.textContaining('运行态暂时读不到'), findsOneWidget);
      expect(find.textContaining('state_unavailable'), findsOneWidget);
      expect(find.text('运行态（只读）'), findsOneWidget);
    });
  });
}
