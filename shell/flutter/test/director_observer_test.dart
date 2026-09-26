/// 阶段5 W5a：设置 → 开发工具 →「导演可观测」四栏（A–D）的回归 + 负对照。
///
/// 五条判据：
/// ① A 栏逐键对上 GET /api/v1/mods/director/state 的 state_json（latest 七键 +
///    五个计数）；
/// ② C 栏同时可见「请求 intensity → 生效最终值 + clamped/degraded/reason」；
/// ③ D 栏两列并排 + 字符数 + 来源标注，(epoch,seq,text) 重复行去重；
/// ④ dev_mode=false 整块不在语义树；
/// ⑤ 源码扫描：观测面两个文件与禁止持久化标识符集合的交集为空。
///
/// 另加纯逻辑回归（见 test/render_events_test.dart 的同名组）。
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/live2d/render_events.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/settings/sections/director_observer_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

/// GET /api/v1/mods/director/state 的**同形**响应体（harness 只取其中的 .state）。
///
/// 键名与 crates/live2d-ai-mod-director/src/ledger.rs 的 state_json 逐字一致：
/// latest 七键（emotion / intent / suggested_tts.speed / suggested_tts.pitch /
/// preset_id / seq / turn）+ 五个计数（turns_seen / turns_ended / decisions /
/// silent / errors）。
///
/// 原文（测试①两边原文之一）：
/// {
///   "delivered": true, "channel": "preset", "presets_chosen": 3,
///   "turns_seen": 9, "turns_ended": 8, "decisions": 7, "silent": 2,
///   "errors": 1, "log_capacity": 20,
///   "latest": {
///     "seq": 7, "turn": "turn-42", "emotion": "happy", "intent": "greeting",
///     "suggested_tts": {"speed": 1.15, "pitch": 0.95},
///     "preset_id": "smile", "closed": true, "delivered": true
///   },
///   "recent_decisions": []
/// }
const Map<String, Object?> _stateJson = <String, Object?>{
  'delivered': true,
  'channel': 'preset',
  'presets_chosen': 3,
  'turns_seen': 9,
  'turns_ended': 8,
  'decisions': 7,
  'silent': 2,
  'errors': 1,
  'log_capacity': 20,
  'latest': <String, Object?>{
    'seq': 7,
    'turn': 'turn-42',
    'emotion': 'happy',
    'intent': 'greeting',
    'suggested_tts': <String, Object?>{'speed': 1.15, 'pitch': 0.95},
    'preset_id': 'smile',
    'closed': true,
    'delivered': true,
  },
  'recent_decisions': <Object?>[],
};

Future<Map<String, Object?>> _fakeState() async => _stateJson;

/// A 栏一行「值」的文本：按「键：」定位到那一行 Row，取行内最后一个 Text。
String _valueOf(WidgetTester tester, String label) {
  final Finder labelFinder = find.text('$label：');
  expect(labelFinder, findsOneWidget, reason: '键 $label 必须逐字可见（不许空面板）');
  final Finder row = find
      .ancestor(of: labelFinder, matching: find.byType(Row))
      .first;
  final List<Text> texts = tester
      .widgetList<Text>(find.descendant(of: row, matching: find.byType(Text)))
      .toList();
  return texts.last.data ?? '';
}

Set<String> _identifiersOf(String source) =>
    RegExp(r'[A-Za-z_][A-Za-z0-9_]*')
        .allMatches(source)
        .map((Match m) => m.group(0)!)
        .toSet();

/// 去掉注释（**保留字符串字面量**：字符串里出现持久化名字同样是落盘信号）。
String _stripComments(String src) {
  final StringBuffer out = StringBuffer();
  int i = 0;
  while (i < src.length) {
    if (src.startsWith('//', i)) {
      while (i < src.length && src[i] != '\n') {
        i++;
      }
      continue;
    }
    if (src.startsWith('/*', i)) {
      int depth = 1;
      i += 2;
      while (i < src.length && depth > 0) {
        if (src.startsWith('/*', i)) {
          depth++;
          i += 2;
        } else if (src.startsWith('*/', i)) {
          depth--;
          i += 2;
        } else {
          i++;
        }
      }
      continue;
    }
    out.write(src[i]);
    i++;
  }
  return out.toString();
}

void main() {
  setUp(() => DirectorObserverFeed.instance.clear());

  testWidgets('① A 栏逐键对上 state_json（latest 七键 + 五个计数）', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _wrap(
        DirectorObserverSection(loadState: _fakeState, loadLogLines: _noLogs),
      ),
    );
    await tester.pumpAndSettle();

    // 期望值就是上面 _stateJson 里的原文（键 → 该键在 UI 上的文本）。
    final Map<String, String> expected = <String, String>{
      'latest.emotion': '开心（happy）',
      'latest.intent': '打招呼（greeting）',
      'latest.suggested_tts.speed': '1.15',
      'latest.suggested_tts.pitch': '0.95',
      'latest.preset_id': '表情（smile）',
      'latest.seq': '7',
      'latest.turn': 'turn-42',
      'turns_seen': '9',
      'turns_ended': '8',
      'decisions': '7',
      'silent': '2',
      'errors': '1',
    };
    // 两边原文（测试输出可见）：
    // state_json: $_stateJson
    for (final MapEntry<String, String> e in expected.entries) {
      expect(
        _valueOf(tester, e.key),
        e.value,
        reason: '键 ${e.key} 的值必须与服务端 state_json 一致',
      );
    }
  });

  testWidgets('① A 栏取不到时如实说明（403 mod_disabled），不许空面板', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _wrap(
        DirectorObserverSection(
          loadState: () async =>
              throw const ApiException('mod_disabled', 'disabled', status: 403),
          loadLogLines: _noLogs,
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.textContaining('403 mod_disabled'), findsOneWidget);
    expect(find.textContaining('读不到决策参数'), findsOneWidget);
  });

  testWidgets('② C 栏：请求 intensity=1 → 生效 y=0.240 clamped=false degraded=true reason=…', (
    WidgetTester tester,
  ) async {
    final DirectorObserverFeed feed = DirectorObserverFeed.instance;
    feed.pushPresetRequest(
      PresetRequest(
        id: '',
        source: 'director',
        ts: DateTime(2026, 9, 26, 14, 33),
        intensity: 1,
        field: 'head',
        y: 0.30,
        hold: false,
        at: 'now',
        seq: 1,
        sentenceSeq: 1,
        epoch: 0,
      ),
    );
    feed.pushRenderEvent(
      parseRenderEvent('preset-applied', <String, Object?>{
        'epoch': 0,
        'ts_ms': 900,
        'seq': 1,
        'field': 'head',
        'y': 0.240,
        'intensity': 1,
        'clamped': false,
        'degraded': true,
        'reason': 'ParamBodyAngleY',
      })!,
    );
    // 第二条请求没有对应 ack → 必须如实写「尚无 ack」。
    feed.pushPresetRequest(
      PresetRequest(
        id: '',
        source: 'director',
        ts: DateTime(2026, 9, 26, 14, 33, 1),
        intensity: 2,
        field: 'body',
        y: 0.10,
        seq: 9,
      ),
    );

    await tester.pumpWidget(
      _wrap(
        DirectorObserverSection(loadState: _fakeState, loadLogLines: _noLogs),
      ),
    );
    await tester.pump();

    expect(
      find.textContaining('请求 field=head intensity=1 y=0.30'),
      findsOneWidget,
      reason: '请求侧必须能看到请求的 intensity 与轴值',
    );
    expect(
      find.textContaining(
        '生效 y=0.240 clamped=false degraded=true reason=ParamBodyAngleY',
      ),
      findsOneWidget,
      reason: '生效侧必须同时看到最终轴值与 clamped/degraded/reason',
    );
    expect(find.textContaining('生效 尚无 ack'), findsOneWidget);
  });

  testWidgets('③ D 栏两列并排 + 字符数 + 来源标注；重复 (epoch,seq,text) 行去重', (
    WidgetTester tester,
  ) async {
    const String sentenceReady =
        '2026-09-26T14:33:18.582037Z  INFO mod: external-input '
        '收到事件 sentence_ready: '
        '{"epoch":0,"sentence_seq":1,"text":"你好呀","ts_ms":893}';
    final List<String> logs = <String>[
      // 同一句被三个 Mod 各记一行：必须去重成一条。
      sentenceReady,
      sentenceReady.replaceFirst('external-input', 'persona'),
      sentenceReady.replaceFirst('external-input', 'memory'),
      '2026-09-26T14:33:19.1Z  INFO mod: external-input '
          '收到事件 sentence_ready: '
          '{"epoch":0,"sentence_seq":2,"text":"今天天气不错","ts_ms":1600}',
      // 非 sentence_ready / 坏 JSON：忽略。
      '2026-09-26T14:33:20Z  INFO mod: external-input 收到事件 turn_ended: {}',
      'not json at all',
      '收到事件 sentence_ready: 没有 JSON payload',
    ];
    DirectorObserverFeed.instance.pushWsEvent(
      TextDeltaEvent(epoch: 0, text: '你好呀', seq: 1),
    );

    await tester.pumpWidget(
      _wrap(
        DirectorObserverSection(
          loadState: _fakeState,
          loadLogLines: () async => logs,
        ),
      ),
    );
    await tester.pumpAndSettle();

    // 两列并排：两个列标题在同一个 Row 下。
    final Finder leftTitle = find.text('净化文本（text_delta，前端已收）');
    final Finder rightTitle = find.text('原始段（日志端点 sentence_ready）');
    expect(leftTitle, findsOneWidget);
    expect(rightTitle, findsOneWidget);
    final Finder columnsRow = find
        .ancestor(of: leftTitle, matching: find.byType(Row))
        .first;
    expect(
      find.descendant(of: columnsRow, matching: rightTitle),
      findsOneWidget,
      reason: '左右两列必须并排（同一个 Row）',
    );
    // 来源标注。
    expect(find.textContaining('来源=日志端点'), findsOneWidget);
    expect(find.textContaining('不承诺逐句精确匹配'), findsOneWidget);
    // 字符数：左列 1 条 + 右列去重后 1 条 = 2 条「你好呀（3 字符）」。
    expect(
      find.text('你好呀（3 字符）'),
      findsNWidgets(2),
      reason: '三行重复的 sentence_ready 必须去重成一条（否则这里会是 4）',
    );
    expect(find.text('今天天气不错（6 字符）'), findsOneWidget);
  });

  testWidgets('B 栏：按 type 真的过滤（点 chip 后非该类型的事件不显示）', (
    WidgetTester tester,
  ) async {
    final DirectorObserverFeed feed = DirectorObserverFeed.instance;
    feed.pushWsEvent(TextDeltaEvent(epoch: 0, text: '甲', seq: 1));
    feed.pushRenderEvent(
      parseRenderEvent('preset-applied', <String, Object?>{
        'field': 'head',
        'y': 0.1,
      })!,
    );
    await tester.pumpWidget(
      _wrap(
        DirectorObserverSection(loadState: _fakeState, loadLogLines: _noLogs),
      ),
    );
    await tester.pump();
    expect(find.textContaining('[ws] text_delta'), findsOneWidget);
    expect(find.textContaining('[render] preset-applied'), findsOneWidget);

    // 选中 preset-applied → text_delta 行必须消失（过滤真的生效）。
    final Finder chip = find.widgetWithText(FilterChip, 'preset-applied');
    await tester.ensureVisible(chip);
    await tester.pumpAndSettle();
    await tester.tap(chip);
    await tester.pump();
    expect(
      find.textContaining('[ws] text_delta'),
      findsNothing,
      reason: '按 type 过滤必须真的过滤（不是摆设）',
    );
    expect(find.textContaining('[render] preset-applied'), findsOneWidget);
    expect(find.textContaining('过滤：preset-applied'), findsOneWidget);

    // 再点一次取消 → 回到全部。
    await tester.tap(chip);
    await tester.pump();
    expect(find.textContaining('[ws] text_delta'), findsOneWidget);
  });

  testWidgets('④ dev_mode=false 整块不在语义树', (WidgetTester tester) async {
    await tester.pumpWidget(
      _wrap(const DirectorObserverSection(devMode: false)),
    );
    await tester.pump();
    expect(find.text('导演可观测'), findsNothing);
    expect(find.byType(TextButton), findsNothing);

    await tester.pumpWidget(
      _wrap(
        DeveloperSection(
          devMode: false,
          onDevModeChanged: (_) {},
          forcedByLaunchFlag: false,
        ),
      ),
    );
    await tester.pump();
    expect(
      find.text('导演可观测'),
      findsNothing,
      reason: 'dev_mode=false 时 DeveloperSection 里不得有「导演可观测」',
    );
  });

  group('⑤ 源码扫描：观测面不得引用任何持久化标识符（标识符集合交集断言）', () {
    const Set<String> forbidden = <String>{
      'localStorage',
      'SharedPreferences',
      'SharedPreferencesAsync',
      'saveDisplayPrefs',
      'loadDisplayPrefs',
      'saveChatSessions',
      'loadChatSessions',
      'window.localStorage',
    };
    const List<String> guardedFiles = <String>[
      'lib/live2d/render_events.dart',
      'lib/settings/sections/director_observer_section.dart',
    ];

    test('两个观测面文件的标识符集合 ∩ 禁止集合 = 空', () {
      for (final String path in guardedFiles) {
        final String source = _stripComments(File(path).readAsStringSync());
        final Set<String> identifiers = _identifiersOf(source);
        final Set<String> forbiddenIds = <String>{
          for (final String name in forbidden) ...name.split('.'),
        };
        final Set<String> hit = identifiers.intersection(forbiddenIds);
        expect(
          hit,
          isEmpty,
          reason: '$path 引用了持久化标识符：$hit\n'
              '这是**标识符集合的交集断言**（不是 grep 行命中）：命中即说明'
              '观测缓冲可能被写盘。',
        );
      }
    });
  });
}

Future<List<String>> _noLogs() async => const <String>[];
