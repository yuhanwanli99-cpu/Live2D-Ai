import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/env_api.dart';
import 'package:live2d_ai_shell/settings/sections/env_key_field.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// rc.2（2026-09-12）：密钥写入控件。
///
/// 钉的是三条：**能写**（以前只能在后端进程环境里设）、**写进去的是当前值**
/// （空值 = 清除）、**不留明文**（保存后立刻清空输入框）。
Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

void main() {
  testWidgets('已设置/未设置都是**文字**表意（不靠颜色）', (WidgetTester tester) async {
    await tester.pumpWidget(
      _wrap(
        EnvKeyField(
          sectionLabel: '对话模型',
          status: const EnvKey(
            section: 'llm',
            key: 'DEEPSEEK_API_KEY',
            set: true,
          ),
          onSave: (String k, String v) async {},
        ),
      ),
    );
    expect(find.text('已设置'), findsOneWidget);
    expect(find.text('DEEPSEEK_API_KEY'), findsOneWidget);
    // 输入框是密文，且不预填任何值（我们**没有**值可填）。
    final TextField field = tester.widget<TextField>(find.byType(TextField));
    expect(field.obscureText, isTrue);
    expect(field.controller?.text, isEmpty);
  });

  testWidgets('保存：回调收到 (key, 输入值)，并随即清空明文', (WidgetTester tester) async {
    final List<(String, String)> saved = <(String, String)>[];
    await tester.pumpWidget(
      _wrap(
        EnvKeyField(
          sectionLabel: '对话模型',
          status: const EnvKey(
            section: 'llm',
            key: 'DEEPSEEK_API_KEY',
            set: false,
          ),
          onSave: (String k, String v) async => saved.add((k, v)),
        ),
      ),
    );
    expect(find.text('未设置'), findsOneWidget);

    await tester.enterText(find.byType(TextField), '  sk-abc  ');
    await tester.tap(find.text('保存密钥'));
    await tester.pumpAndSettle();

    expect(saved, <(String, String)>[('DEEPSEEK_API_KEY', 'sk-abc')]);
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      isEmpty,
      reason: '写进 .env 之后没有理由把明文留在输入框里',
    );
  });

  testWidgets('留空保存 = 清除该键（不是「什么都没做」）', (WidgetTester tester) async {
    final List<(String, String)> saved = <(String, String)>[];
    await tester.pumpWidget(
      _wrap(
        EnvKeyField(
          sectionLabel: '对话模型',
          status: const EnvKey(
            section: 'llm',
            key: 'DEEPSEEK_API_KEY',
            set: true,
          ),
          onSave: (String k, String v) async => saved.add((k, v)),
        ),
      ),
    );
    await tester.tap(find.text('保存密钥'));
    await tester.pumpAndSettle();
    expect(saved, <(String, String)>[('DEEPSEEK_API_KEY', '')]);
  });

  testWidgets('保存失败：界面上说出来，且不谎报成功', (WidgetTester tester) async {
    await tester.pumpWidget(
      _wrap(
        EnvKeyField(
          sectionLabel: '对话模型',
          status: const EnvKey(section: 'llm', key: 'K', set: false),
          onSave: (String k, String v) async => throw Exception('boom'),
        ),
      ),
    );
    await tester.enterText(find.byType(TextField), 'x');
    await tester.tap(find.text('保存密钥'));
    await tester.pumpAndSettle();
    expect(find.textContaining('保存失败'), findsOneWidget);
  });

  testWidgets('配置里没声明键名 → 说明「未绑定」而不是给一个假输入框', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _wrap(
        const EnvKeyField(
          sectionLabel: '对话模型',
          status: null,
          onSave: null,
        ),
      ),
    );
    expect(find.text('未绑定环境变量'), findsOneWidget);
    expect(find.byType(TextField), findsNothing);
  });
}
