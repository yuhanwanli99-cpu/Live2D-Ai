/// Mod 配置表单的草稿（2026-09-28，F-0003-6）。
///
/// # 两条被守住的用户可见缺陷
///
/// 1. **外部 `config` 变化吞掉未提交草稿**。宿主保存后会 `_loadAdmin()`
///    重取列表，服务端每次回的都是**新的 Map 实例**；旧的 `didUpdateWidget`
///    按 `identical` 判断，于是「后台刷新一次 = 用户填了一半的表单被静默
///    重置回服务端值」。分区里同时还有他处刷新（启停 / 重载）这条路径。
/// 2. **保存结果离开分区即丢**。分区切换会把整个 pane 子树卸掉
///    （`shell_settings.dart` 的 `switch` 每次建新的 pane 类型），卡片 State
///    连同草稿、连同带错误码的失败文案一起消失。现在的真源是**路由级**的
///    `PageStorage` 桶（每个 `ModalRoute` 自带一个）：子树死了它还在。
///
/// # 测试怎么造「离开分区」
///
/// **同一个 `MaterialApp` / 同一条路由**，只把 pane 换成另一块占位内容再换
/// 回来（`_PaneHost`）——这正是设置面板切分区时发生的事。整棵 `MaterialApp`
/// 重新 pump 会新建路由、新建桶，那样测的就不是这条缺陷了。
///
/// # 焦点这件事（为什么有一堆 `unfocus`）
///
/// `TextFieldRow` 自己有一条「用户正在编辑时不同步外部值」的保护
/// （`!_hasFocus`）——它挡的是光标被打断，但**不**挡「草稿被重灌」：一旦
/// 失焦（用户点去别处看一眼再回来、或输入完切分区）落后值就会盖上来。
/// 所以这里在触发外部刷新前显式失焦，测的才是「草稿有没有被保住」。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 本文件专用的 Mod id（与 `mods_section_test.dart` 的 `demo-mod` 区分开，
/// 免得看日志时以为串了）。
const String kModId = 'k6-mod';
const String kModLabel = '六号实验 Mod';

/// 初始的服务端 config（草稿的起点）。
const Map<String, Object?> kInitialConfig = <String, Object?>{
  'command': 'run-a',
  'mode': 'chat',
  'port': 11434,
};

ModSettingsSpec _spec() => ModSettingsSpec(
  modId: kModId,
  title: kModLabel,
  version: 1,
  fields: <ModSettingField>[
    const ModSettingField(
      kind: ModFieldKind.string,
      key: 'command',
      label: '启动命令',
    ),
    const ModSettingField(
      kind: ModFieldKind.select,
      key: 'mode',
      label: '模式',
      options: <ModSelectOption>[
        ModSelectOption(value: 'chat', label: '对话模式'),
        ModSelectOption(value: 'work', label: '工作模式'),
      ],
    ),
    const ModSettingField(
      kind: ModFieldKind.number,
      key: 'port',
      label: '服务端口',
      min: 1024,
      max: 65535,
    ),
    const ModSettingField(
      kind: ModFieldKind.string,
      key: 'api_key',
      label: '密钥',
      secret: true,
    ),
  ],
);

ModInfo _mod(Map<String, Object?> config) => ModInfo(
  id: kModId,
  name: kModLabel,
  version: '0.1.0',
  apiVersion: 1,
  enabled: true,
  status: 'running',
  config: config,
  settingsSpec: _spec(),
);

/// 运行态读取的假实现（避免卡片在展开时自建 `ModsApi()` 打网络）。
Future<ModStateResult> _stubState(String id) async =>
    ModStateResult(id: id, enabled: true, state: const <String, Object?>{});

/// 「设置面板」：同一个路由里持有 Mod 分区 + 另一块分区内容。
///
/// `setConfig` = 宿主保存后 `_loadAdmin()` 拿回的新列表（新 Map 实例）；
/// `showMods(false/true)` = 切到别的分区 / 切回来。
class _PaneHost extends StatefulWidget {
  const _PaneHost({required this.save, super.key});

  final Future<ModConfigResult> Function(String, Map<String, Object?>)? save;

  @override
  State<_PaneHost> createState() => PaneHostState();
}

@visibleForTesting
class PaneHostState extends State<_PaneHost> {
  Map<String, Object?> config = kInitialConfig;
  bool showMods = true;
  Map<String, Object?>? lastSaved;

  void setConfig(Map<String, Object?> next) => setState(() => config = next);

  void switchSection({required bool toMods}) =>
      setState(() => showMods = toMods);

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: showMods
          ? SingleChildScrollView(
              child: ModsSection(
                mods: <ModInfo>[_mod(config)],
                loading: false,
                onLoadState: _stubState,
                onSaveConfig: widget.save == null
                    ? null
                    : (String id, Map<String, Object?> cfg) {
                        lastSaved = cfg;
                        return widget.save!(id, cfg);
                      },
              ),
            )
          : const Center(child: Text('其它分区')),
    );
  }
}

Future<void> _pumpHost(
  WidgetTester tester,
  GlobalKey<PaneHostState> key, {
  Future<ModConfigResult> Function(String, Map<String, Object?>)? save,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: _PaneHost(key: key, save: save),
    ),
  );
  await tester.pumpAndSettle();
}

/// 展开卡片（分区切回来后 ExpansionTile 是收起的，要再点一次）。
Future<void> _expand(WidgetTester tester) async {
  await tester.tap(find.text(kModLabel));
  await tester.pumpAndSettle();
}

/// 当前 `TextField` 里文本正好是 [text] 的那个字段。
Finder _fieldWithText(String text) => find.byWidgetPredicate(
  (Widget w) => w is TextField && w.controller?.text == text,
);

Future<void> _type(WidgetTester tester, String from, String to) async {
  final Finder field = _fieldWithText(from);
  expect(field, findsOneWidget, reason: '找不到文本为「$from」的输入框');
  await tester.enterText(field, to);
  await tester.pump();
  // 失焦：模拟「用户输入完去看别处 / 切分区」，此时 TextFieldRow 的
  // 「正在编辑」保护不再生效——草稿存不存得住才见真章。
  FocusManager.instance.primaryFocus?.unfocus();
  await tester.pump();
}

Future<void> _save(WidgetTester tester) async {
  final Finder save = find.text('保存');
  await tester.ensureVisible(save);
  await tester.pumpAndSettle();
  await tester.tap(save);
  await tester.pumpAndSettle();
}

/// 下拉框当前显示的值。
String _selectedMode(WidgetTester tester) =>
    tester.widget<DropdownButton<String>>(
      find.byType(DropdownButton<String>),
    ).value!;

void main() {
  group('F-0003-6①：外部 config 变化不许覆盖未提交草稿', () {
    testWidgets('新实例、同内容（宿主保存后重取列表）→ 草稿原样保留', (WidgetTester tester) async {
      final GlobalKey<PaneHostState> host = GlobalKey<PaneHostState>();
      await _pumpHost(tester, host, save: (_, _) async => const ModConfigResult(ok: true));
      await _expand(tester);
      await _type(tester, 'run-a', 'run-b');

      // 宿主重取列表：**新 ModInfo、新 Map 实例**，内容一字不差。
      host.currentState!.setConfig(<String, Object?>{
        'command': 'run-a',
        'mode': 'chat',
        'port': 11434,
      });
      await tester.pumpAndSettle();

      expect(
        _fieldWithText('run-b'),
        findsOneWidget,
        reason: '后台重取列表把用户正在输入的草稿重灌回服务端值了（identity 比较的老毛病）',
      );
    });

    testWidgets('内容真的变了：没碰过的字段跟服务端，改过的字段保留草稿', (
      WidgetTester tester,
    ) async {
      final GlobalKey<PaneHostState> host = GlobalKey<PaneHostState>();
      await _pumpHost(tester, host, save: (_, _) async => const ModConfigResult(ok: true));
      await _expand(tester);
      await _type(tester, 'run-a', 'run-b');

      // 服务端侧 `mode` 被别人改了（如他处启停/重载），`command` 没变。
      host.currentState!.setConfig(<String, Object?>{
        'command': 'run-a',
        'mode': 'work',
        'port': 11434,
      });
      await tester.pumpAndSettle();

      expect(
        _selectedMode(tester),
        'work',
        reason: '用户没碰过的字段应当跟随服务端（整份不动 = 界面停在旧值）',
      );
      expect(
        _fieldWithText('run-b'),
        findsOneWidget,
        reason: '服务端内容变了就把用户改过的字段一起重灌了',
      );
    });

    testWidgets('没有未提交草稿时，服务端新值照旧整份回填', (WidgetTester tester) async {
      final GlobalKey<PaneHostState> host = GlobalKey<PaneHostState>();
      await _pumpHost(tester, host, save: (_, _) async => const ModConfigResult(ok: true));
      await _expand(tester);
      // 一个字都没改 → 服务端重取后应当看到新值（别把「保草稿」做成「永不刷新」）。
      expect(_fieldWithText('run-a'), findsOneWidget);

      host.currentState!.setConfig(<String, Object?>{
        'command': 'run-z',
        'mode': 'work',
        'port': 11434,
      });
      await tester.pumpAndSettle();

      expect(_fieldWithText('run-z'), findsOneWidget);
      expect(_selectedMode(tester), 'work');
    });
  });

  group('F-0003-6②：保存结果与草稿要活过「离开分区」', () {
    testWidgets('保存成功后切走再切回来：值与「已保存」都还在', (WidgetTester tester) async {
      final GlobalKey<PaneHostState> host = GlobalKey<PaneHostState>();
      // 宿主**不**重取列表：活不活得下来只取决于草稿有没有住在 State 之外。
      await _pumpHost(tester, host, save: (_, _) async => const ModConfigResult(ok: true));
      await _expand(tester);
      await _type(tester, 'run-a', 'run-b');
      await _save(tester);
      expect(find.textContaining('已保存'), findsOneWidget);

      host.currentState!.switchSection(toMods: false);
      await tester.pumpAndSettle();
      expect(find.text('其它分区'), findsOneWidget);

      host.currentState!.switchSection(toMods: true);
      await tester.pumpAndSettle();
      await _expand(tester);

      expect(
        _fieldWithText('run-b'),
        findsOneWidget,
        reason: '保存结果离开分区就丢了（草稿只住卡片 State，切分区即销毁）',
      );
      expect(
        find.textContaining('已保存'),
        findsOneWidget,
        reason: '保存结果文案离开分区就丢了',
      );
    });

    testWidgets('保存失败：草稿与带错误码的文案一起跨分区存活', (WidgetTester tester) async {
      final GlobalKey<PaneHostState> host = GlobalKey<PaneHostState>();
      await _pumpHost(
        tester,
        host,
        save: (_, _) async => throw const ApiException('bad_config', '端口非法'),
      );
      await _expand(tester);
      await _type(tester, 'run-a', 'run-b');
      await _save(tester);

      expect(find.textContaining('bad_config'), findsOneWidget);
      expect(
        _fieldWithText('run-b'),
        findsOneWidget,
        reason: '保存失败后草稿被重灌回服务端值 —— 用户得从头再填一遍',
      );

      host.currentState!.switchSection(toMods: false);
      await tester.pumpAndSettle();
      host.currentState!.switchSection(toMods: true);
      await tester.pumpAndSettle();
      await _expand(tester);

      expect(
        find.textContaining('bad_config'),
        findsOneWidget,
        reason: '「拿界面上的码去日志里搜」的前提是那个码还在屏上',
      );
      expect(_fieldWithText('run-b'), findsOneWidget);
    });

    testWidgets('保存成功且宿主重取了归一化值 → 采用服务端值（不是用户原样输入）', (
      WidgetTester tester,
    ) async {
      final GlobalKey<PaneHostState> host = GlobalKey<PaneHostState>();
      await _pumpHost(
        tester,
        host,
        save: (_, _) async {
          // 服务端归一化（如钳位）后的值，经宿主 `_loadAdmin()` 回来。
          host.currentState!.setConfig(<String, Object?>{
            'command': 'run-b-normalized',
            'mode': 'chat',
            'port': 11434,
          });
          await Future<void>.delayed(const Duration(milliseconds: 10));
          return const ModConfigResult(ok: true, restarted: true);
        },
      );
      await _expand(tester);
      await _type(tester, 'run-a', 'run-b');
      await _save(tester);

      expect(find.textContaining('已保存'), findsOneWidget);
      expect(
        _fieldWithText('run-b-normalized'),
        findsOneWidget,
        reason: '保存成功后服务端归一化值才是真相（别把用户原样输入当结果留着）',
      );
      expect(_fieldWithText('run-b'), findsNothing);
    });

    testWidgets('密钥草稿不进路由桶：切回来后按「未填」处理，保存也不会抹掉已存密钥', (
      WidgetTester tester,
    ) async {
      final GlobalKey<PaneHostState> host = GlobalKey<PaneHostState>();
      await _pumpHost(tester, host, save: (_, _) async => const ModConfigResult(ok: true));
      await _expand(tester);

      final Finder secret = find.byWidgetPredicate(
        (Widget w) => w is TextField && w.obscureText,
      );
      expect(secret, findsOneWidget);
      await tester.enterText(secret, 'sk-1');
      await tester.pump();
      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pump();

      host.currentState!.switchSection(toMods: false);
      await tester.pumpAndSettle();
      host.currentState!.switchSection(toMods: true);
      await tester.pumpAndSettle();
      await _expand(tester);

      expect(
        _fieldWithText('sk-1'),
        findsNothing,
        reason: '密钥草稿被写进了路由存储桶 —— secret 值不该进任何持久化面',
      );
      // 而「留空 = 不修改」这条契约必须还在：切回来再保存不能把密钥抹掉。
      await _save(tester);
      expect(
        host.currentState!.lastSaved!.containsKey('api_key'),
        isFalse,
        reason: '空密钥被当成「清空」提交了 —— 用户的密钥会这样丢掉',
      );
    });
  });
}
