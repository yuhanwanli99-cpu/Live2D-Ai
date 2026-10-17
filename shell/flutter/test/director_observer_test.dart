/// 阶段5 W5a：一级「导演」卡里「导演可观测」四栏（A–D）的回归 + 负对照。
///
/// rc4（2026-10-17）后的判据：
/// ① A 栏**静态**：「核心常驻（无决策账本）」——退出状态端点读取，不再有 403/404 文案；
/// ② C 栏同时可见「请求 intensity → 生效最终值 + clamped/degraded/reason」；
/// ③ D 栏两列并排 + 字符数 + 来源标注，(epoch,seq,text) 重复行去重；
/// ④ dev_mode=false 整块不在语义树；
/// ⑤ 源码扫描：观测面两个文件与禁止持久化标识符集合的交集为空；观测面不读
///    Mod 状态端点（`ModsApi` / `mods/director/state` 零出现）。
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/live2d/render_events.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/settings/sections/director_observer_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/source_scan.dart';

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

Set<String> _identifiersOf(String source) =>
    RegExp(r'[A-Za-z_][A-Za-z0-9_]*')
        .allMatches(source)
        .map((Match m) => m.group(0)!)
        .toSet();

void main() {
  setUp(() => DirectorObserverFeed.instance.clear());

  testWidgets('① A 栏静态：核心常驻（无决策账本），不发状态端点请求', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _wrap(const DirectorObserverSection(loadLogLines: _noLogs2)),
    );
    await tester.pumpAndSettle();

    expect(find.text(kDirectorCoreResidentText), findsOneWidget);
    expect(find.text('核心常驻（无决策账本）'), findsOneWidget);
    // 旧 A 栏（服务端 state_json 的决策参数 / 刷新按钮 / 403 文案）整块下架。
    expect(find.text('刷新决策参数'), findsNothing);
    expect(find.textContaining('读不到决策参数'), findsNothing);
    expect(find.textContaining('词表记录'), findsNothing);
    expect(find.textContaining('state_json'), findsNothing);
    expect(find.textContaining('403 mod_disabled'), findsNothing);
    expect(find.textContaining('404 not_found'), findsNothing);
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
      _wrap(const DirectorObserverSection(loadLogLines: _noLogs2)),
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
        DirectorObserverSection(loadLogLines: () async => logs),
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

  testWidgets('B 栏：按 type 真的过滤（取消勾选后该类型的事件不显示）', (
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
      _wrap(const DirectorObserverSection(loadLogLines: _noLogs2)),
    );
    await tester.pump();
    expect(find.textContaining('[ws] text_delta'), findsOneWidget);
    expect(find.textContaining('[render] preset-applied'), findsOneWidget);

    // 取消勾选「预设生效（preset-applied）」→ **该类型**的行必须消失；
    // 别的类型照看（语义是「不看的」而不是「只看这些」）。
    final Finder chip = find.widgetWithText(FilterChip, '预设生效（preset-applied）');
    await tester.ensureVisible(chip);
    await tester.pumpAndSettle();
    await tester.tap(chip);
    await tester.pump();
    expect(
      find.textContaining('[render] preset-applied'),
      findsNothing,
      reason: '取消勾选该类型必须真的不列出（不是摆设）',
    );
    expect(find.textContaining('[ws] text_delta'), findsOneWidget);
    expect(find.textContaining('过滤：当前：默认'), findsNothing);
    expect(find.textContaining('过滤：当前：'), findsOneWidget);

    // 再点一次 → 回到默认档（两者都在）。
    await tester.tap(chip);
    await tester.pump();
    expect(find.textContaining('[render] preset-applied'), findsOneWidget);
    expect(find.textContaining('[ws] text_delta'), findsOneWidget);
    expect(find.textContaining('过滤：当前：默认'), findsOneWidget);
  });

  testWidgets('B 栏（T3）：芯片是中文 + 括号里的线名；未知类型仍用线名', (
    WidgetTester tester,
  ) async {
    DirectorObserverFeed.instance.pushWsEvent(
      UnknownWsEvent(type: 'brand_new', data: <String, Object?>{'x': 1}),
    );
    await tester.pumpWidget(
      _wrap(const DirectorObserverSection(loadLogLines: _noLogs2)),
    );
    await tester.pump();

    for (final String label in <String>[
      '动作（action_cue）',
      '正文（text_delta）',
      '未收尾（text_fallback）',
      '出错（error）',
      '预设生效（preset-applied）',
      '预设丢弃（preset-dropped）',
      '段结束（segment-ended）',
      '口型时钟（stage-clock）',
    ]) {
      expect(
        find.widgetWithText(FilterChip, label),
        findsOneWidget,
        reason: '芯片必须是「中文（线名）」：$label',
      );
    }
    // 未知类型照记、芯片用线名（不猜中文）。
    expect(find.widgetWithText(FilterChip, 'brand_new'), findsOneWidget);
  });

  testWidgets('B 栏（T3）：默认不列出 stage-clock 与段结束；点芯片可打开', (
    WidgetTester tester,
  ) async {
    final DirectorObserverFeed feed = DirectorObserverFeed.instance;
    feed.pushWsEvent(TextDeltaEvent(epoch: 0, text: '甲', seq: 1));
    feed.pushStageClock(seg: 1, posMs: 120, playing: true);
    await tester.pumpWidget(
      _wrap(const DirectorObserverSection(loadLogLines: _noLogs2)),
    );
    await tester.pump();

    // ① 默认就不列出口型时钟（30ms 一条，不挡着的话动作帧会被挤掉）。
    expect(find.textContaining('[ws] text_delta'), findsOneWidget);
    expect(find.textContaining('[audio] stage-clock'), findsNothing);
    expect(find.textContaining('过滤：当前：默认'), findsOneWidget);

    // ② 口型时钟芯片默认未勾；点一下打开，行就出现。
    final Finder chip = find.widgetWithText(FilterChip, '口型时钟（stage-clock）');
    await tester.ensureVisible(chip);
    await tester.pumpAndSettle();
    await tester.tap(chip);
    await tester.pump();
    expect(find.textContaining('[audio] stage-clock'), findsOneWidget);
  });

  testWidgets('B 栏（T3）：点一行在列表下方展开全文，再点收起；不新增路由', (
    WidgetTester tester,
  ) async {
    DirectorObserverFeed.instance.pushWsEvent(
      TextDeltaEvent(epoch: 0, text: '甲', seq: 1),
    );
    await tester.pumpWidget(
      _wrap(const DirectorObserverSection(loadLogLines: _noLogs2)),
    );
    await tester.pump();

    final Finder row = find.textContaining('[ws] text_delta');
    expect(row, findsOneWidget);
    // 还没点：只有一行（行里含时间戳，精确匹配全文不会命中）。
    expect(find.text('text_delta 1 字符'), findsNothing);

    // 观测区在 A 栏下面，800x600 里落在折叠线以下：先滚进来再点。
    await tester.ensureVisible(row);
    await tester.pumpAndSettle();
    await tester.tap(row);
    await tester.pump();
    expect(
      find.text('text_delta 1 字符'),
      findsOneWidget,
      reason: '点一行要在列表下方展开该条全文',
    );

    await tester.ensureVisible(row);
    await tester.pumpAndSettle();
    await tester.tap(row);
    await tester.pump();
    expect(find.text('text_delta 1 字符'), findsNothing, reason: '再点一次收起');
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
      reason: 'dev_mode=false 时开发模式页里不得有「导演可观测」',
    );
  });

  group('⑤ 源码扫描：观测面不引用持久化标识符、也不读 Mod 状态端点', () {
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
        // 词法器只有一份（`support/source_scan.dart`，反复制门禁在
        // `source_scan_test.dart`）；字符串保留是本消费点的语义。
        final String source = stripCommentsKeepStrings(
          File(path).readAsStringSync(),
        );
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

    test('A 栏降级后不再读 Mod 状态端点（rc4）', () {
      // 只扫代码（注释里说明「读取路径已删」不算复活）。
      final String src = stripCommentsKeepStrings(
        File('lib/settings/sections/director_observer_section.dart')
            .readAsStringSync(),
      );
      for (final String banned in <String>[
        'ModsApi',
        'mods_api',
        'mods/director/state',
        'directorStateErrorText',
        'kDirectorObserverModId',
      ]) {
        expect(
          src.contains(banned),
          isFalse,
          reason: '观测面不得再出现「$banned」——A 栏已是静态结论',
        );
      }
    });
  });
}

Future<List<String>> _noLogs2() async => const <String>[];
