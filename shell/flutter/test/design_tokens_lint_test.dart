import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'design_tokens_test.dart' show stripCommentsAndStrings;

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
/// 渲染面桥的 ≤30 Hz 节流），不是 UI 动效时长。
bool isProtocolDurationPath(String path) =>
    path.startsWith('lib/api/') ||
    path.startsWith('lib/audio/') ||
    path.startsWith('lib/live2d/');

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
    why: '颜色只能来自 ColorScheme 槽位或 AppPalette/AppColors'
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
    pattern: RegExp(r'maxWidth\s*[<>]=?'),
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
    why: '间距只能取 Space 的 9 档（4 px 网格）；1–2 px 的基线微调取 '
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
        sources.map((File f) => f.path).where(
          (String p) => p.endsWith('main.dart'),
        ),
        isNotEmpty,
        reason: '连 main.dart 都没扫到，说明扫描根路径不对',
      );
      expect(sources.length, greaterThanOrEqualTo(10));
    });

    for (final Rule rule in kRules) {
      test('规则「${rule.name}」：${rule.why}', () {
        final List<String> hits = <String>[];
        for (final File file in sources) {
          final String path = file.path;
          if (!rule.appliesTo(path)) continue;
          final List<String> lines = stripCommentsAndStrings(
            file.readAsStringSync(),
          ).split('\n');
          for (int i = 0; i < lines.length; i++) {
            if (rule.pattern.hasMatch(lines[i])) {
              hits.add('$path:${i + 1}: ${lines[i].trim()}');
            }
          }
        }
        expect(
          hits,
          isEmpty,
          reason: '命中「${rule.name}」（${rule.why}）：\n${hits.join('\n')}',
        );
      });
    }
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
      expect(
        kRules.any((Rule r) => r.name.contains('毛玻璃')),
        isFalse,
      );
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
