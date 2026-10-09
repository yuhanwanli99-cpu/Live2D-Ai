/// **产品面文案门禁**（2026-09-22；2026-10-08 改口径）：防旧说法回潮。
///
/// 这一组守的是**用户看到的那两处**：
/// 1. 导演卡片上**只剩一句话**——「打开后，说话时会带上表情和轻微的头、颈动作。」；
///    旧的两条说明（按情绪触发 / 分段送 TTS / 接管 / 留空 / 两项都填）**不得回潮**；
/// 2. LLM 分区**不再有**「表演层（默认关）」那一行，也不再出现只说 / 只动 / noop /
///    `[performance]` 这些用户看不懂的词；2026-10-08 起「谁负责动作」那一行
///    也**整体撤掉**——导演按句出 cue 是产品行为，不需要在这里解释。
///
/// 断言是「**产品面找不到这些字符串**」（真泵 widget + 真泵源码），不是恒真。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/mods/director_panel.dart';
import 'package:live2d_ai_shell/settings/sections/llm_section.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/dart_library.dart';

/// 读一份 `lib/**` 源码（`flutter test` 的工作目录 = 包根）。
String _readLib(String rel) => readLibrarySource(rel);

/// 剥掉 `//` / `///` 行注释：源码扫描只关心**会被渲染或被引用的字符串**，
/// 解释性注释里提到旧词（例如「2026-10-08 撤掉了「表演层（默认关）」一行」）
/// 不算回潮。
String _code(String rel) => _readLib(rel)
    .split('\n')
    .where((String line) => !line.trimLeft().startsWith('//'))
    .join('\n');

SettingsView _view() => SettingsView.fromJson(<String, Object?>{
  'llm': <String, Object?>{
    'base_url': 'https://api.deepseek.com/v1',
    'model': 'deepseek-flash',
    'has_api_key': true,
  },
});

Future<void> _pumpLlm(WidgetTester tester, {required bool devMode}) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(
        body: SingleChildScrollView(
          child: LlmSection(
            controller: SettingsController(
              api: ApiClient(base: 'http://127.0.0.1:18080'),
            ),
            view: _view(),
            devMode: devMode,
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

void main() {
  group('导演卡片：只留一句人话', () {
    test('锚点句就是那一句，旧说法不得回潮', () {
      expect(
        kDirectorTakeoverNotice,
        '打开后，说话时会带上表情和轻微的头、颈动作。',
      );
      for (final String banned in <String>[
        '按情绪触发',
        '分段送 TTS',
        '接管',
        '留空',
        '两项都填',
        '只填一项',
        '不改原文',
      ]) {
        expect(
          kDirectorTakeoverNotice,
          isNot(contains(banned)),
          reason: '用户看不懂的旧说法「$banned」不得回到产品面板',
        );
      }
    });

    test('面板源码里不再有那批用户看不懂的词（真泵源码）', () {
      final String src = _code('lib/settings/mods/director_panel.dart');
      for (final String banned in <String>[
        'kDirectorPresetNotice',
        '接管状态',
        '规则预设',
        '实际下发',
        '本轮预设',
        '清空决策账本',
        'command_unavailable',
        '依据：',
      ]) {
        expect(
          src.contains(banned),
          isFalse,
          reason: '导演产品面板不得再出现「$banned」',
        );
      }
      // 8 个配置键声明为「产品面不渲染」。
      expect(src.contains('hiddenKeys'), isTrue);
      for (final String key in <String>[
        'preset_happy',
        'preset_sad',
        'preset_greeting',
        'staging_enabled',
        'staging_base_url',
        'staging_model',
        'staging_api_key_env',
        'staging_timeout_ms',
      ]) {
        expect(src.contains("'$key'"), isTrue, reason: '$key 必须列为隐藏键');
      }
    });
  });

  group('LLM 分区：思考开关（表演层与「谁负责动作」两行都已撤）', () {
    testWidgets('真泵：思考是唯一开关；老文案与表演层字样都不在', (WidgetTester tester) async {
      await _pumpLlm(tester, devMode: true);

      expect(find.text('思考'), findsOneWidget);
      // 2026-10-09：控件说明全删 ⇒ 原来的「默认关闭。打开后模型会先想再答」
      // 不在界面上了（开关的语义没变）。
      expect(find.text('默认关闭。打开后模型会先想再答'), findsNothing);
      // 2026-10-08：这一行整体删掉，产品面上不再解释「谁负责动作」。
      expect(find.textContaining('谁负责动作'), findsNothing);
      expect(find.textContaining('表情和点头'), findsNothing);

      for (final String banned in <String>[
        '展示思考（reasoning）',
        '关掉只是不显示',
        '思考照样生成',
        '表演层（默认关）',
        '表演层',
        '主模型不负责表演',
        '只说',
        '只动',
        'noop',
        '[performance]',
      ]) {
        expect(
          find.textContaining(banned),
          findsNothing,
          reason: 'LLM 分区不得再出现「$banned」',
        );
      }
    });

    test('源码里也不再有那两个表演层常量、那一行与「谁负责动作」（真泵源码）', () {
      final String src = _code('lib/settings/sections/llm_section.dart');
      for (final String banned in <String>[
        'kLlmPerformanceText',
        'kLlmPerformanceDescription',
        '表演层（默认关）',
        '[performance]',
        '去「Mod」里的导演',
        '谁负责动作',
        'kLlmActionOwnerText',
      ]) {
        expect(src.contains(banned), isFalse, reason: 'LLM 分区源码不得含「$banned」');
      }
    });
  });
}
