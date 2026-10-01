/// 连通性自检的**成败判定**与**成功渲染槽**回归（审计 F-0012-1 / F-0003-2）。
///
/// # 缺陷本身（2026-10-01）
///
/// `llm_section.dart:179-183` 与 `tts_section.dart:177-179` 曾这样判成败：
///
/// ```dart
/// resultIsError: testResult != null &&
///     !testResult!.toLowerCase().contains('ok') &&
///     !testResult!.contains('毫秒') &&
///     !testResult!.contains('ms'),
/// ```
///
/// 判据是**展示文案的子串**——直接违反红线「前端不得从显示文案猜错误类型」。
/// 两个方向都坏：
/// ① **上游错误里带 `ms`/`ok` 时失败被当成成功**；
/// ② **成功那条没有渲染槽**：`_FieldShell` 只在 `error` 非空时渲染，于是
///    点「测试连接」成功时界面**毫无反应**。
///
/// 修法：成败只认服务端已序列化的 `TestOutcome.ok`（`SettingsTestOutcome.ok`），
/// 与文案一起装进 `FieldTestResult`；成功走独立的成功槽。
///
/// 本文件的断言全部落在**真泵**（真 `LlmSection` / `TtsSection` / `FieldActionRow`）
/// 与**具体槽位**（`kFieldSuccessNoticeKey` / `kFieldErrorNoticeKey`）上——
/// 不扫源码、不扫文案猜语义。修复前必须红（那时成功什么都不渲染，
/// 且含 "ok" 的失败文案被判成成功）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';
import 'package:live2d_ai_shell/settings/sections/llm_section.dart';
import 'package:live2d_ai_shell/settings/sections/tts_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart';
import 'package:live2d_ai_shell/ui/inline_notice.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 服务端真相的最小快照（两个分区各给必需字段，其余走 fromJson 默认）。
SettingsView _view() => SettingsView.fromJson(<String, Object?>{
  'llm': <String, Object?>{
    'base_url': 'http://127.0.0.1:11434/v1',
    'model': 'qwen2.5:7b',
    'has_api_key': false,
  },
  'tts': <String, Object?>{
    'base_url': 'http://127.0.0.1:8080/v1',
    'voice': 'skystar',
    'has_api_key': false,
    'sample_rate': 24000,
    'channels': 1,
  },
});

SettingsController _controller() =>
    SettingsController(api: ApiClient(base: 'http://127.0.0.1:18080'));

Future<void> _pumpLlm(WidgetTester tester, FieldTestResult? result) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(
        body: SingleChildScrollView(
          child: LlmSection(
            controller: _controller(),
            view: _view(),
            devMode: false,
            onTest: () async {},
            testResult: result,
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

Future<void> _pumpTts(WidgetTester tester, FieldTestResult? result) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(
        body: SingleChildScrollView(
          child: TtsSection(
            controller: _controller(),
            view: _view(),
            devMode: false,
            onTest: () async {},
            testResult: result,
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

Future<void> _pumpActionRow(WidgetTester tester, FieldTestResult? result) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(
        body: FieldActionRow(
          label: '连通性自检',
          icon: Icons.wifi_tethering,
          actionLabel: '测试连接',
          onPressed: () {},
          result: result,
        ),
      ),
    ),
  );
  await tester.pump();
}

void main() {
  group('FieldActionRow：成败只认 ok，文案不许参与判定', () {
    testWidgets('ok=true + 文案含 "ms" → **成功槽**上屏', (WidgetTester tester) async {
      await _pumpActionRow(
        tester,
        const FieldTestResult(ok: true, message: 'ok · 12 ms'),
      );
      expect(find.byKey(kFieldSuccessNoticeKey), findsOneWidget);
      expect(find.byKey(kFieldErrorNoticeKey), findsNothing);
      expect(find.text('ok · 12 ms'), findsOneWidget);
    });

    testWidgets('ok=false + 文案含 "ok" 与 "ms" → **失败槽**（旧判据会判成成功）', (
      WidgetTester tester,
    ) async {
      const String adversarial = '失败：上游 12 ms 后返回 ok=false 之外的 400';
      await _pumpActionRow(
        tester,
        const FieldTestResult(ok: false, message: adversarial),
      );
      expect(find.byKey(kFieldErrorNoticeKey), findsOneWidget);
      expect(find.byKey(kFieldSuccessNoticeKey), findsNothing);
      expect(find.text(adversarial), findsOneWidget);
    });

    testWidgets('result=null（还没测过）→ 两个槽都不出现', (WidgetTester tester) async {
      await _pumpActionRow(tester, null);
      expect(find.byKey(kFieldSuccessNoticeKey), findsNothing);
      expect(find.byKey(kFieldErrorNoticeKey), findsNothing);
    });

    testWidgets('成功槽用的是**行内提示**的信息档（不新造第二套成功样式）', (
      WidgetTester tester,
    ) async {
      await _pumpActionRow(
        tester,
        const FieldTestResult(ok: true, message: 'ok · 3 ms'),
      );
      final InlineNotice notice = tester.widget<InlineNotice>(
        find.byKey(kFieldSuccessNoticeKey),
      );
      expect(notice.severity, NoticeSeverity.info);
      expect(notice.message, 'ok · 3 ms');
    });
  });

  group('LlmSection：真泵，服务端 ok 直接决定槽位', () {
    testWidgets('ok=true（成功）→ 成功槽 + 文案原样上屏（旧行为：什么都不显示）', (
      WidgetTester tester,
    ) async {
      const String message = 'ok · 12 ms · 模型：qwen2.5:7b';
      await _pumpLlm(tester, const FieldTestResult(ok: true, message: message));

      expect(
        find.byKey(kFieldSuccessNoticeKey),
        findsOneWidget,
        reason: 'F-0003-2：成功必须有渲染槽（过去这里什么都没有，用户以为按钮坏了）',
      );
      expect(find.byKey(kFieldErrorNoticeKey), findsNothing);
      expect(find.text(message), findsOneWidget);
    });

    testWidgets('ok=false 且文案含 "ok" → 失败槽（旧判据把它判成成功且不渲染）', (
      WidgetTester tester,
    ) async {
      const String message = '失败：上游返回 401（响应体含 ok 字样）';
      await _pumpLlm(tester, const FieldTestResult(ok: false, message: message));

      expect(find.byKey(kFieldErrorNoticeKey), findsOneWidget);
      expect(find.byKey(kFieldSuccessNoticeKey), findsNothing);
      expect(find.text(message), findsOneWidget);
    });

    testWidgets('ok=false 且文案含 "ms" → 失败槽（F-0012-1 的原始误判）', (
      WidgetTester tester,
    ) async {
      const String message = '失败：连接被拒（12 ms 后返回）';
      await _pumpLlm(tester, const FieldTestResult(ok: false, message: message));

      expect(find.byKey(kFieldErrorNoticeKey), findsOneWidget);
      expect(find.byKey(kFieldSuccessNoticeKey), findsNothing);
    });

    testWidgets('还没测过 → 两个槽都不出现', (WidgetTester tester) async {
      await _pumpLlm(tester, null);
      expect(find.byKey(kFieldSuccessNoticeKey), findsNothing);
      expect(find.byKey(kFieldErrorNoticeKey), findsNothing);
    });
  });

  group('TtsSection：同款（两个分区不许各自一套判据）', () {
    testWidgets('ok=true（成功，带服务端 note）→ 成功槽 + 文案原样上屏', (
      WidgetTester tester,
    ) async {
      const String message = 'ok · 5 ms · 上游可达但未提供 /models';
      await _pumpTts(tester, const FieldTestResult(ok: true, message: message));

      expect(find.byKey(kFieldSuccessNoticeKey), findsOneWidget);
      expect(find.byKey(kFieldErrorNoticeKey), findsNothing);
      expect(find.text(message), findsOneWidget);
    });

    testWidgets('ok=false 且文案含 "ok" → 失败槽', (WidgetTester tester) async {
      const String message = '失败：上游返回 500（含 ok 字样）';
      await _pumpTts(tester, const FieldTestResult(ok: false, message: message));

      expect(find.byKey(kFieldErrorNoticeKey), findsOneWidget);
      expect(find.byKey(kFieldSuccessNoticeKey), findsNothing);
      expect(find.text(message), findsOneWidget);
    });
  });
}
