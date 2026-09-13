import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/models_api.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// A2（rc.2 2026-09-12）：模型库的**导入入口**与**激活入口**。
///
/// 为什么值得钉：DoD 的端到端是「导入 → 激活 → 皮套真的换了」。此前
/// `ModelsApi.import` 全仓**零调用方**，空态还写着「然后在这里导入」——
/// 界面上根本没有那个「这里」。这两条测试守的就是入口存在且真的接线。
ModelInfo _model(String id, {bool active = false}) => ModelInfo(
  id: id,
  displayName: id,
  version: 3,
  active: active,
  hasPhysics: true,
  hasDisplayInfo: false,
  sizeBytes: 2048,
  textureCount: 2,
  importedAt: '2026-09-12T00:00:00.000Z',
  moc3File: '$id.moc3',
);

Future<void> _pump(
  WidgetTester tester,
  ModelsSection section,
) async {
  await tester.pumpWidget(
    MaterialApp(
      // 必须用真主题：SectionHeader 等组件从 AppColors 扩展取色。
      theme: buildAppTheme(),
      home: Scaffold(body: SingleChildScrollView(child: section)),
    ),
  );
}

void main() {
  testWidgets('空态：说明「在这里导入」，且导入入口真的在界面上', (WidgetTester tester) async {
    await _pump(
      tester,
      ModelsSection(
        models: const <ModelInfo>[],
        loading: false,
        onImport: (String id) async {},
      ),
    );

    expect(find.text('还没有导入模型'), findsOneWidget);
    expect(find.text('导入'), findsOneWidget);
    expect(find.text('导入模型（目录名）'), findsOneWidget);
  });

  testWidgets('导入：填目录名 → 回调收到去空格的 id，输入框清空', (WidgetTester tester) async {
    final List<String> imported = <String>[];
    await _pump(
      tester,
      ModelsSection(
        models: const <ModelInfo>[],
        loading: false,
        onImport: (String id) async => imported.add(id),
      ),
    );

    await tester.enterText(find.byType(TextField), '  bai  ');
    await tester.tap(find.text('导入'));
    await tester.pumpAndSettle();

    expect(imported, <String>['bai'], reason: 'id 必须去空格后再提交');
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      isEmpty,
      reason: '成功后清空输入框，方便接着导入下一个',
    );
  });

  testWidgets('空输入不触发导入（不向后端发一个空 id）', (WidgetTester tester) async {
    final List<String> imported = <String>[];
    await _pump(
      tester,
      ModelsSection(
        models: const <ModelInfo>[],
        loading: false,
        onImport: (String id) async => imported.add(id),
      ),
    );

    await tester.tap(find.text('导入'));
    await tester.pumpAndSettle();
    expect(imported, isEmpty);
  });

  testWidgets('激活按钮只对未激活项出现，点击回调收到 id', (WidgetTester tester) async {
    final List<String> activated = <String>[];
    await _pump(
      tester,
      ModelsSection(
        models: <ModelInfo>[
          _model('bai', active: true),
          _model('neko'),
        ],
        loading: false,
        onActivate: (String id) async => activated.add(id),
        onImport: (String id) async {},
      ),
    );

    // 激活中的那一项没有按钮（否则「激活当前模型」是个无意义操作）。
    expect(find.text('激活'), findsOneWidget);
    expect(find.text('当前激活'), findsOneWidget);

    await tester.tap(find.text('激活'));
    await tester.pumpAndSettle();
    expect(activated, <String>['neko']);
  });

  testWidgets('激活进行中：按钮禁用并显示「激活中…」（防重复提交）', (WidgetTester tester) async {
    await _pump(
      tester,
      ModelsSection(
        models: <ModelInfo>[_model('neko')],
        loading: false,
        busyId: 'neko',
        onActivate: (String id) async {},
        onImport: (String id) async {},
      ),
    );

    expect(find.text('激活中…'), findsOneWidget);
    final FilledButton button = tester.widget<FilledButton>(
      find.widgetWithText(FilledButton, '激活中…'),
    );
    expect(button.onPressed, isNull, reason: '忙时不得再提交');
  });
}
