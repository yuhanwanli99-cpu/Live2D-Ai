import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'support/source_scan.dart';

/// 裸值扫描的**豁免面**（越小越难被侵蚀）。
///
/// 只有设计令牌自己的两个声明文件可以出现裸色值/裸字号/裸圆角——
/// 它们是令牌的**定义处**，本来就该有具体数值。
const Set<String> kTokenDeclarationFiles = <String>{
  'lib/design/tokens.dart',
  'lib/design/typography.dart',
};

/// 协议常量豁免：`lib/api/`、`lib/audio/`、`lib/live2d/` 里的
/// `Duration(milliseconds:)` 是**协议参数**（WS 分片 20 ms、重连退避、
/// 渲染面桥的 ≤30 Hz 节流、IndexedDB 挂死兜底 3 s），不是 UI 动效时长。
///
/// `lib/data/` 是 2026-09-27 加进来的（背景字节库）：那里的时长是
/// 「等存储多久算挂死」，归到 `AppDurations` 会让「4 档动效」这个令牌
/// 背上它完全无关的语义。
bool isProtocolDurationPath(String path) =>
    path.startsWith('lib/api/') ||
    path.startsWith('lib/audio/') ||
    path.startsWith('lib/live2d/') ||
    path.startsWith('lib/data/');

/// **逐个点名**的时长豁免文件（不是整个目录）。
///
/// 豁免面越小越难被侵蚀——所以只列具体文件，不列目录。
/// 这里剩下的是「时序」但不是「UI 过渡时长」，所以不该逼它用 `AppDurations`
/// （那 4 档是为 hover/内容切换这类**交互过渡**定的）。
///
/// 2026-09-11：原先还有一条 `lib/actions/action_dispatch.dart`（动作兜底时长表，
/// 镜像渲染面 `choreography_total_ms`）。手动触发入口移出成品后它整条消失，
/// 豁免面随之缩小——这正是「豁免表只能变小」该有的样子。
const Set<String> kNamedTimingFiles = <String>{
  // 读屏播报的最小间隔：`text_delta` 是毫秒级的，不节流会把读屏淹没。
  // 这是**无障碍节奏**，是听觉可读性的下限，与视觉过渡无关。
  'lib/state/live_region.dart',
  // 语音识别会话结束后的重启防抖（Web Speech 静音会 onend，常驻监听要重启）。
  // 这是**识别会话的时序策略**，不是 UI 过渡时长——AppDurations 那 4 档是给
  // hover / 内容切换这类交互过渡定的，套在这里没有语义。
  'lib/voice/voice_listen_controller.dart',
};

/// 一条扫描规则。
class Rule {
  const Rule({
    required this.name,
    required this.pattern,
    required this.why,
    this.exempt,
  });

  final String name;
  final RegExp pattern;

  /// 命中即失败时给实现者看的解释。
  final String why;

  /// 该路径是否豁免本规则。
  final bool Function(String path)? exempt;

  bool appliesTo(String path) => !(exempt?.call(path) ?? false);
}

final List<Rule> kRules = <Rule>[
  Rule(
    name: '裸色字面量',
    // `\b` 边界很关键：`AppColors.of(...)` 里含有子串 `Colors.of`，
    // 不加边界会把它误报成裸色。
    //
    // `Colors.transparent` **豁免**（负向先行断言）：它表达的是「不画」，
    // 不携带任何配色信息——`surfaceTintColor: Colors.transparent` 说的是
    // 「别叠 Material 的色调层」，换成别的主题也不会变。把它也当成
    // 「颜色决策」会逼着实现去写一个语义上不存在的令牌。
    pattern: RegExp(r'\bColor\(0x|\bColors\.(?!transparent\b)[a-z]'),
    why:
        '颜色只能来自 ColorScheme 槽位或 AppPalette/AppColors'
        '（`Colors.transparent` 例外：它表示「不画」，不是配色）',
    exempt: kTokenDeclarationFiles.contains,
  ),
  Rule(
    name: '裸字号',
    pattern: RegExp(r'fontSize:'),
    why: '字号只能取 TextTheme 的 8 个槽位（见 AppFontSizes.registry）',
    exempt: kTokenDeclarationFiles.contains,
  ),
  Rule(
    name: '裸圆角',
    // `BorderRadius.circular(AppRadius.md)` 合法；只有数字字面量才拦。
    pattern: RegExp(r'BorderRadius\.circular\(\s*[0-9]'),
    why: '圆角只能取 AppRadius 的 6 档',
    exempt: kTokenDeclarationFiles.contains,
  ),
  Rule(
    name: '裸断点',
    pattern: kRawBreakpointPattern,
    why: '断点判断只能走 Breakpoints.sizeClassOf（否则无法单测）',
    exempt: (String p) => p == 'lib/design/breakpoints.dart',
  ),
  Rule(
    name: '裸间距',
    // **只扫 `EdgeInsets` 里的数字实参**，不扫 `SizedBox`/`width:`/`height:`。
    //
    // 为什么收窄（2026-09-11，P4-2 —— 这是实施时改的口径）：
    // `width:` / `height:` 在本项目里绝大多数是**元素自身尺寸**
    // （图标 18、发送键 40×40、数值列 44/52/56/110/150、描边 1 px、
    // 流式圆点 3–4 px），不是「元素之间的间距」。把它们也拦下来会逼着
    // 实现去写一堆语义不存在的令牌（把 1 px 描边写成 `Space.s1` = 4 是
    // **改设计**，不是归位），那样的门禁只会被当成噪音关掉。
    //
    // `EdgeInsets` 则是**没有歧义**的间距：它的每个数字都在说
    // 「这里离那边多远」。所以这一条拦得住真问题，又不会误伤。
    pattern: RegExp(
      r'EdgeInsets\.\w+\([^)]*(?<![\w.])(?!(?:Space|OpticalNudge)\.)\d',
    ),
    why:
        '间距只能取 Space 的 9 档（4 px 网格）；1–2 px 的基线微调取 '
        'OpticalNudge——两者是不同的概念，别混',
    exempt: kTokenDeclarationFiles.contains,
  ),
  Rule(
    name: '协议时长混用',
    pattern: RegExp(r'Duration\(milliseconds:'),
    why: 'UI 动效时长只能取 AppDurations 的 4 档',
    // 协议目录 + 令牌声明文件（`AppDurations` 本身就是在那里定义的）。
    exempt: (String p) =>
        isProtocolDurationPath(p) ||
        kNamedTimingFiles.contains(p) ||
        kTokenDeclarationFiles.contains(p),
  ),
];

/// **裸断点**的形态（2026-09-28，F-0006-3：扩大覆盖面）。
///
/// 原规则只锚字面量 `maxWidth`（`RegExp(r'maxWidth\s*[<>]=?')`），于是**最自然的
/// 旁路**根本不在拦截面内：
///
/// ```dart
/// if (MediaQuery.sizeOf(context).width >= 1280) { … }   // ← 旧规则零命中
/// if (size.width < 900) { … }                           // ← 旧规则零命中
/// ```
///
/// 现在两种「取宽度」的写法一起拦，且**原形态一条都没丢**：
///
/// - `\bmaxWidth\s*[<>]=?` —— 原覆盖面（`constraints.maxWidth >= 900`）原样保留
///   （收紧它属于**缩**覆盖面，不是本轮的事）；
/// - `\b(?:minWidth|width)\s*[<>]=?\s*(?:[0-9]|Breakpoints\.)` —— 新增。右边
///   **要求**是数字字面量或 `Breakpoints.*`：既拦住「魔法数字断点」，又不会把
///   `child.width > other.width` 这类**尺寸比较**误报成断点判断——门禁要拦真问题，
///   拦下所有 `.width` 只会被当成噪音关掉（与「裸间距」规则同一条取舍）。
final RegExp kRawBreakpointPattern = RegExp(
  r'(?:\bmaxWidth\s*[<>]=?'
  r'|\b(?:minWidth|width)\s*[<>]=?\s*(?:[0-9]|Breakpoints\.))',
);

/// **两跳旁路**：先把宽度存进局部变量，再和数字比较。
///
/// ```dart
/// final double w = MediaQuery.sizeOf(context).width;
/// if (w >= 1280) return …;        // ← 单行正则一个字符都匹配不到
/// ```
///
/// 这是「一行里看不出断点」的写法，正则的单行模型天生够不着，所以单列一条
/// **同文件数据流**检查：先收集「宽度别名」，再看别名有没有和数字 / `Breakpoints.*`
/// 比较。命中返回 `'<行号>: <该行源码>'`。
final RegExp kWidthAliasDeclaration = RegExp(
  r'\b(?:final|late\s+final|double|num|var)\s+(\w+)\s*=\s*[^;\n]*\.\s*(?:max|min)?[Ww]idth\b',
);

/// 同文件里「宽度别名 → 和数字 / `Breakpoints.*` 比较」的命中行。
List<String> findWidthAliasBreakpoints(String source) {
  final List<String> lines = stripCommentsAndStrings(source).split('\n');
  final Set<String> aliases = <String>{};
  for (final String line in lines) {
    for (final RegExpMatch m in kWidthAliasDeclaration.allMatches(line)) {
      aliases.add(m.group(1)!);
    }
  }
  if (aliases.isEmpty) return const <String>[];
  final RegExp comparison = RegExp(
    '\\b(?:${aliases.map(RegExp.escape).join('|')})'
    r'\s*[<>]=?\s*(?:[0-9]|Breakpoints\.)',
  );
  final List<String> hits = <String>[];
  for (int i = 0; i < lines.length; i++) {
    if (comparison.hasMatch(lines[i])) hits.add('${i + 1}: ${lines[i].trim()}');
  }
  return hits;
}

/// 对**一段源码**跑一条规则——真实扫描与合成用例走**同一条代码路径**。
///
/// 抽出来的理由（2026-09-28，F-0006-3）：只断言「`lib/` 现状零命中」的话，
/// 规则窄到拦不住旁路时测试**照样是绿的**（现状里本来就没有旁路写法）。
/// 合成源码用例只有与真实扫描共用这个函数，量的才是规则本身。
List<String> scanSource(Rule rule, String path, String source) {
  if (!rule.appliesTo(path)) return const <String>[];
  final List<String> lines = stripCommentsAndStrings(source).split('\n');
  final List<String> hits = <String>[];
  for (int i = 0; i < lines.length; i++) {
    if (rule.pattern.hasMatch(lines[i])) {
      hits.add('$path:${i + 1}: ${lines[i].trim()}');
    }
  }
  return hits;
}

void main() {
  final List<File> sources = Directory('lib')
      .listSync(recursive: true)
      .whereType<File>()
      .where((File f) => f.path.endsWith('.dart'))
      .toList();

  group('裸值扫描（引用侧兜底）', () {
    test('扫描确实覆盖到了源码（防 Directory 路径写错导致「零命中=通过」）', () {
      // 这是最危险的假通过：路径写错 → 一个文件都没扫 → 全部规则空转通过。
      expect(sources, isNotEmpty, reason: 'lib/ 下没扫到任何 .dart');
      expect(
        sources
            .map((File f) => f.path)
            .where((String p) => p.endsWith('main.dart')),
        isNotEmpty,
        reason: '连 main.dart 都没扫到，说明扫描根路径不对',
      );
      expect(sources.length, greaterThanOrEqualTo(10));
    });

    for (final Rule rule in kRules) {
      test('规则「${rule.name}」：${rule.why}', () {
        final List<String> hits = <String>[
          for (final File file in sources)
            ...scanSource(rule, file.path, file.readAsStringSync()),
        ];
        expect(
          hits,
          isEmpty,
          reason: '命中「${rule.name}」（${rule.why}）：\n${hits.join('\n')}',
        );
      });
    }

    test('断点别名旁路（先存局部宽度、再和数字比较）在真实 lib/ 里零命中', () {
      final List<String> hits = <String>[];
      for (final File file in sources) {
        // 与「裸断点」规则同一条豁免：断点阈值表的定义处允许比较宽度。
        if (file.path == 'lib/design/breakpoints.dart') continue;
        for (final String hit in findWidthAliasBreakpoints(
          file.readAsStringSync(),
        )) {
          hits.add('${file.path}:$hit');
        }
      }
      expect(
        hits,
        isEmpty,
        reason:
            '命中「断点别名旁路」（先 `final w = …width` 再 `if (w >= 1280)`）：\n'
            '${hits.join('\n')}',
      );
    });
  });

  group('断点规则覆盖面（合成源码，与真实扫描同一条代码路径）', () {
    final Rule rule = kRules.firstWhere((Rule r) => r.name == '裸断点');
    List<String> scan(String src) =>
        scanSource(rule, 'lib/ui/_synthetic_probe.dart', src);

    /// 旧规则（F-0006-3 修之前那条），只用来**自证判别力**：它必须漏掉。
    final RegExp oldPattern = RegExp(r'maxWidth\s*[<>]=?');

    test('旁路写法 `MediaQuery.sizeOf(context).width >= 1280` 被拦下', () {
      const String bypass =
          'if (MediaQuery.sizeOf(context).width >= 1280) return _wide();';
      expect(scan(bypass), isNotEmpty, reason: '最自然的旁路必须被拦住');
      // 判别力自证：旧规则对这段源码**零命中**——不是「本来就能拦」。
      expect(oldPattern.hasMatch(bypass), isFalse);
    });

    test('旁路写法 `size.width < 900` 被拦下', () {
      const String bypass = 'if (size.width < 900) return _narrow();';
      expect(scan(bypass), isNotEmpty);
      expect(oldPattern.hasMatch(bypass), isFalse);
    });

    test('旁路写法 `constraints.minWidth > 900` 被拦下', () {
      const String bypass = 'final bool wide = constraints.minWidth > 900;';
      expect(scan(bypass), isNotEmpty);
      expect(oldPattern.hasMatch(bypass), isFalse);
    });

    test('原形态没被改丢：`constraints.maxWidth >= 900` 仍然命中', () {
      const String original = 'if (constraints.maxWidth >= 900) return _mid();';
      expect(scan(original), isNotEmpty);
      expect(oldPattern.hasMatch(original), isTrue, reason: '旧规则本来就拦这个');
    });

    test('和 Breakpoints 常量比较也算断点判断（拦）', () {
      expect(scan('if (size.width >= Breakpoints.mediumMin) …'), isNotEmpty);
    });

    test('合规写法零命中：`Breakpoints.sizeClassOf(size.width)`', () {
      expect(
        scan('final SizeClass sc = Breakpoints.sizeClassOf(size.width);'),
        isEmpty,
        reason: '合规形态被误报会让这条门禁被关掉',
      );
    });

    test('尺寸比较不是断点判断（`child.width > other.width` 零命中）', () {
      expect(scan('if (child.width > other.width) …'), isEmpty);
    });

    test('两跳旁路：别名声明 + 别名比较被 `findWidthAliasBreakpoints` 拦下', () {
      const String twoHop = '''
final double w = MediaQuery.sizeOf(context).width;
if (w >= 1280) return _wide();
''';
      final List<String> hits = findWidthAliasBreakpoints(twoHop);
      expect(hits, isNotEmpty, reason: '两跳写法必须被拦住');
      expect(hits.single, startsWith('2:'), reason: '命中的是**比较**那一行');
      // 判别力自证：单行规则对整段源码零命中（两跳是它够不着的那一类）。
      expect(oldPattern.hasMatch(twoHop), isFalse);
      expect(rule.pattern.hasMatch(twoHop), isFalse);
    });

    test('两跳旁路的反例：别名只和另一个宽度比（不是断点）零命中', () {
      const String notBreakpoint = '''
final double w = size.width;
if (w > other.width) return _shrink();
''';
      expect(findWidthAliasBreakpoints(notBreakpoint), isEmpty);
    });

    test('别名声明被注释/字符串包住时不算（防误报）', () {
      const String inComment = '''
// final double w = size.width;
const String note = 'final double w = size.width;';
if (w >= 1280) return _wide();
''';
      expect(findWidthAliasBreakpoints(inComment), isEmpty);
    });
  });

  group('豁免面本身要小且有效', () {
    test('色值/字号/圆角的豁免恰好是两个令牌声明文件', () {
      expect(kTokenDeclarationFiles.length, 2);
      for (final String p in kTokenDeclarationFiles) {
        expect(File(p).existsSync(), isTrue, reason: '豁免文件不存在：$p');
      }
    });

    test('本文件不重复实现毛玻璃规则（归属 no_backdrop_filter_test.dart）', () {
      // 同一条红线的两处实现迟早会漂移（一处改了另一处没改），所以只留一处。
      // 毛玻璃 + ImageFilter.blur + contentFaint + 导入守卫都在
      // `no_backdrop_filter_test.dart` 里，那个文件名就说明了它的职责。
      expect(kRules.any((Rule r) => r.name.contains('毛玻璃')), isFalse);
    });

    test('协议时长豁免只覆盖 api/ 与 audio/ 两个目录', () {
      expect(isProtocolDurationPath('lib/api/ws_client.dart'), isTrue);
      expect(isProtocolDurationPath('lib/audio/schedule.dart'), isTrue);
      // 这里原本拿 `lib/ui/display_panel.dart` 当反例；该文件是死代码，已于
      // 2026-09-11 删除（前端加强计划 P0-1），反例改用仍然存在的 UI 文件。
      expect(isProtocolDurationPath('lib/ui/state_pill.dart'), isFalse);
      expect(isProtocolDurationPath('lib/design/tokens.dart'), isFalse);
    });
  });
}
