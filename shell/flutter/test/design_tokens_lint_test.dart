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
    pattern: RegExp(r'\bColor\(0x|\bColors\.[a-z]'),
    why: '颜色只能来自 ColorScheme 槽位或 AppPalette/AppColors',
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
    name: '毛玻璃',
    pattern: RegExp(r'BackdropFilter'),
    // **无豁免**：BackdropFilter 模糊不到 iframe platform view（官方 issue
    // #184996 / #89889），却照付离屏渲染代价。GlassPanel 用半透明纯色 + 描边。
    why: '本项目不做毛玻璃（模糊不到 iframe，还照付代价）',
  ),
  Rule(
    name: '协议时长混用',
    pattern: RegExp(r'Duration\(milliseconds:'),
    why: 'UI 动效时长只能取 AppDurations 的 4 档',
    // 协议目录 + 令牌声明文件（`AppDurations` 本身就是在那里定义的）。
    exempt: (String p) =>
        isProtocolDurationPath(p) || kTokenDeclarationFiles.contains(p),
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

    test('毛玻璃规则没有任何豁免', () {
      final Rule frosted = kRules.firstWhere((Rule r) => r.name == '毛玻璃');
      expect(frosted.exempt, isNull, reason: '毛玻璃不该有豁免');
    });

    test('协议时长豁免只覆盖 api/ 与 audio/ 两个目录', () {
      expect(isProtocolDurationPath('lib/api/ws_client.dart'), isTrue);
      expect(isProtocolDurationPath('lib/audio/schedule.dart'), isTrue);
      expect(isProtocolDurationPath('lib/ui/display_panel.dart'), isFalse);
      expect(isProtocolDurationPath('lib/design/tokens.dart'), isFalse);
    });
  });
}
