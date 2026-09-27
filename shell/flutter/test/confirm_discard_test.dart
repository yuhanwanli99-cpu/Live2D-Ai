/// 未保存改动确认框（P0，2026-09-20）：三个按钮的**行为**与**可点击层**。
///
/// # 覆盖的失败模式
///
/// 1. 三个按钮都真的接到回调（留下 / 保存并离开 / 放弃改动）；
/// 2. 「保存并离开」**看 SaveOutcome**：failed 时**不离开**且给出可见错误；
/// 3. saving 态禁用三键，防连点重复提交；
/// 4. 弹窗自己垫了 [StagePointerInterceptor]——舞台 iframe 一切正常时，
///    没有这层垫层三个按钮就是「看得见、点不着」（真机根因，见该 widget 头注）；
/// 5. 外壳层面：**关设置**与**换分区**都走同一个 confirmDiscard。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/live2d/stage_pointer_interceptor.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/confirm_discard_dialog.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 一个只为了弹出确认框的宿主：按钮 → showConfirmDiscardDialog → 记结果。
class _AskHost extends StatefulWidget {
  const _AskHost({required this.onSave, this.errorText});
  final Future<SaveOutcome> Function() onSave;
  final String? errorText;
  @override
  State<_AskHost> createState() => _AskHostState();
}

class _AskHostState extends State<_AskHost> {
  bool? result;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(
        body: Builder(
          builder: (BuildContext ctx) => Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                TextButton(
                  onPressed: () async {
                    final bool leave = await showConfirmDiscardDialog(
                      ctx,
                      onSave: widget.onSave,
                      errorOf: () => widget.errorText,
                    );
                    if (mounted) setState(() => result = leave);
                  },
                  child: const Text('ask'),
                ),
                if (result != null) Text('result:$result'),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// 外壳集成宿主：把 confirmDiscard / dirty 喂给 AppShell。
class _ShellHost extends StatefulWidget {
  const _ShellHost({required this.confirm, required this.dirty});
  final Future<bool> Function()? confirm;
  final bool dirty;
  @override
  State<_ShellHost> createState() => _ShellHostState();
}

class _ShellHostState extends State<_ShellHost> {
  SettingsSection section = SettingsSection.appearance;

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: buildAppTheme(),
    home: AppShell(
      prefs: const DisplayPrefs(),
      stage: const ColoredBox(color: Color(0xFF000000)),
      phase: UiPhase.idle,
      wsStatus: WsStatus.connected,
      messages: const <Never>[],
      input: TextEditingController(),
      onSend: () {},
      onStop: () {},
      onRetryConnection: () {},
      volume: 0.8,
      muted: false,
      onVolumeChanged: (_) {},
      onMutedChanged: (_) {},
      sections: visibleSections(),
      section: section,
      onSectionChanged: (SettingsSection s) => setState(() => section = s),
      sectionBuilder: (BuildContext c, SettingsSection s) => Text('PANE:${s.label}'),
      settingsDirty: widget.dirty,
      confirmDiscard: widget.confirm,
      // 不传的话默认是 loading → 不确定进度条会一直转，pumpAndSettle 永不收敛。
      stagePhase: Live2DBridgePhase.ready,
    ),
  );
}

void main() {
  group('确认框组件：三按钮语义', () {
    testWidgets('「留下」→ 不离开', (WidgetTester tester) async {
      await tester.pumpWidget(
        _AskHost(onSave: () async => SaveOutcome.savedApplied),
      );
      await tester.tap(find.text('ask'));
      await tester.pumpAndSettle();
      expect(find.text('有未保存的改动'), findsOneWidget);

      await tester.tap(find.text('留下'));
      await tester.pumpAndSettle();
      expect(find.text('有未保存的改动'), findsNothing);
      expect(find.text('result:false'), findsOneWidget);
    });

    testWidgets('「放弃改动」→ 离开', (WidgetTester tester) async {
      await tester.pumpWidget(
        _AskHost(onSave: () async => SaveOutcome.savedApplied),
      );
      await tester.tap(find.text('ask'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('放弃改动'));
      await tester.pumpAndSettle();
      expect(find.text('result:true'), findsOneWidget);
    });

    testWidgets('弹窗压在舞台上 → 必须自己垫 StagePointerInterceptor（P0 根因）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _AskHost(onSave: () async => SaveOutcome.savedApplied),
      );
      await tester.tap(find.text('ask'));
      await tester.pumpAndSettle();
      expect(
        find.ancestor(
          of: find.byType(AlertDialog),
          matching: find.byType(StagePointerInterceptor),
        ),
        findsWidgets,
        reason: '弹窗画在舞台 iframe 之上却没有垫层 = 三个按钮全点不着',
      );
    });
  });

  group('确认框组件：「保存并离开」看 SaveOutcome', () {
    testWidgets('保存失败 → 留在弹窗里 + 可见错误（带服务端 code）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _AskHost(
          onSave: () async => SaveOutcome.failed,
          errorText: 'HTTP 400 invalid_payload：base_url 非法',
        ),
      );
      await tester.tap(find.text('ask'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('保存并离开'));
      await tester.pumpAndSettle();

      expect(find.text('有未保存的改动'), findsOneWidget, reason: '失败绝不能离开');
      expect(find.byKey(const Key('confirm-discard-error')), findsOneWidget);
      expect(find.textContaining('invalid_payload'), findsOneWidget);
      expect(find.textContaining('result:'), findsNothing, reason: '还没离开');
    });

    testWidgets('保存成功 → 离开', (WidgetTester tester) async {
      await tester.pumpWidget(
        _AskHost(onSave: () async => SaveOutcome.savedRestartRequired),
      );
      await tester.tap(find.text('ask'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('保存并离开'));
      await tester.pumpAndSettle();
      expect(find.text('result:true'), findsOneWidget);
    });

    testWidgets('保存中 → 三键禁用 + 文案「保存中…」（防连点）', (
      WidgetTester tester,
    ) async {
      final Completer<SaveOutcome> gate = Completer<SaveOutcome>();
      int saves = 0;
      await tester.pumpWidget(
        _AskHost(
          onSave: () {
            saves++;
            return gate.future;
          },
        ),
      );
      await tester.tap(find.text('ask'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('保存并离开'));
      await tester.pump();

      expect(find.text('保存中…'), findsOneWidget);
      // 三个按钮各自的实际类型：两个 TextButton + 一个 FilledButton。
      expect(
        tester.widget<TextButton>(find.widgetWithText(TextButton, '留下')).onPressed,
        isNull,
        reason: '保存中「留下」必须禁用',
      );
      expect(
        tester
            .widget<TextButton>(find.widgetWithText(TextButton, '保存中…'))
            .onPressed,
        isNull,
        reason: '保存中「保存并离开」必须禁用',
      );
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, '放弃改动'))
            .onPressed,
        isNull,
        reason: '保存中「放弃改动」必须禁用',
      );
      // 连点：不会发起第二次保存。
      await tester.tap(find.text('保存中…'), warnIfMissed: false);
      await tester.pump();
      expect(saves, 1);

      gate.complete(SaveOutcome.savedApplied);
      await tester.pumpAndSettle();
      expect(find.text('result:true'), findsOneWidget);
    });
  });

  group('外壳集成：关设置 / 换分区走同一个 confirmDiscard', () {
    Future<void> pumpShell(
      WidgetTester tester, {
      required Future<bool> Function() confirm,
      bool dirty = true,
    }) async {
      await tester.binding.setSurfaceSize(const Size(1400, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(_ShellHost(confirm: confirm, dirty: dirty));
      await tester.pump();
      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
    }

    testWidgets('脏草稿 + 关设置：问过用户才关（返回 false → 不关）', (
      WidgetTester tester,
    ) async {
      int asked = 0;
      await pumpShell(
        tester,
        confirm: () async {
          asked++;
          return false;
        },
      );
      final AppShellState shell = tester.state<AppShellState>(find.byType(AppShell));
      expect(shell.settingsOpen, isTrue);

      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      expect(asked, 1, reason: '关设置必须走 confirmDiscard');
      expect(shell.settingsOpen, isTrue, reason: '用户选择留下 → 不关');
    });

    testWidgets('脏草稿 + 关设置：返回 true → 关闭', (WidgetTester tester) async {
      await pumpShell(tester, confirm: () async => true);
      final AppShellState shell = tester.state<AppShellState>(find.byType(AppShell));
      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      expect(shell.settingsOpen, isFalse);
    });

    testWidgets('不脏 → 不问，直接关', (WidgetTester tester) async {
      int asked = 0;
      await pumpShell(
        tester,
        dirty: false,
        confirm: () async {
          asked++;
          return false;
        },
      );
      final AppShellState shell = tester.state<AppShellState>(find.byType(AppShell));
      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      expect(asked, 0);
      expect(shell.settingsOpen, isFalse);
    });

    testWidgets('脏草稿 + 换分区：返回 false → 分区不动；true → 切过去', (
      WidgetTester tester,
    ) async {
      bool leave = false;
      await pumpShell(
        tester,
        confirm: () async => leave,
      );
      await tester.tap(find.text('LLM'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<AppShell>(find.byType(AppShell)).section,
        SettingsSection.appearance,
        reason: '用户选择留下 → 不换分区',
      );

      leave = true;
      await tester.tap(find.text('LLM'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<AppShell>(find.byType(AppShell)).section,
        SettingsSection.llm,
      );
    });
  });
}
