/// **字体子集门禁**：界面文案里不得出现自托管子集之外的字符。
///
/// # 为什么需要这道门禁（2026-09-11，无头浏览器真机点火时抓到）
///
/// 项目的硬约束是「中文字体必须自托管，否则**断网即豆腐块**」——Flutter Web 的
/// CanvasKit 取不到设备字体，`fontFamilyFallback` 也不会命中系统字体，缺字时
/// 引擎只会去 `fonts.gstatic.com` 下载回退字体。**有网时完全看不出来。**
///
/// 真机点火时实测到一个外部请求：
///
/// ```text
/// GET https://fonts.gstatic.com/s/notosanssc/v37/…88.woff2  [200]
/// ```
///
/// 排查（用 fontTools 读字体真实 cmap）发现界面里有**两个字符不在子集里**
/// （子集 22 036 个码点）：
///
/// | 字符 | 位置 | 何时上屏 |
/// | --- | --- | --- |
/// | `▍` U+258D | `ui/message_bubble.dart` 流式光标 | **每次流式回复** |
/// | `⌘` U+2318 | `app/app_shortcuts.dart` macOS 前缀 | 打开快捷键帮助 |
///
/// 两者都已改成不依赖字形的实现（竖条 / ASCII `Cmd`）。本文件是**门禁**：
/// 以后谁再往界面里写子集外的字符，`flutter test` 直接红，而不是等断网才暴露。
///
/// # 覆盖表为什么是「生成文件 + 哈希」
///
/// 覆盖表从字体文件派生（`scripts/font_subset_ranges.py`）。若有人换了字体却没
/// 重新生成覆盖表，测试就会拿旧表放行——**比没有门禁更危险**。所以文件头记录了
/// 字体字节数与 FNV-1a(32) 哈希，这里重新计算比对，不一致就要求重新生成。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 字体与覆盖表（相对包根；`flutter test` 的 CWD 就是包根）。
const List<String> _fontStems = <String>[
  'assets/fonts/NotoSansSC-AiSubset-Regular',
  'assets/fonts/NotoSansSC-AiSubset-Bold',
];

/// FNV-1a 32 位。与 `scripts/font_subset_ranges.py` 的实现**逐字节等价**。
int fnv1a32(List<int> bytes) {
  int h = 0x811C9DC5;
  for (final int b in bytes) {
    h ^= b;
    h = (h * 0x01000193) & 0xFFFFFFFF;
  }
  return h;
}

/// 覆盖表：字体身份（字节数 + 哈希）与码点区间。
class SubsetRanges {
  SubsetRanges({
    required this.bytes,
    required this.hash,
    required this.codepoints,
    required this.ranges,
  });

  final int bytes;
  final int hash;
  final int codepoints;
  final List<(int, int)> ranges;

  bool covers(int cp) {
    for (final (int lo, int hi) in ranges) {
      if (cp < lo) return false; // 区间有序，早停
      if (cp <= hi) return true;
    }
    return false;
  }
}

/// 解析生成文件（头注 + 区间行）。
SubsetRanges parseRanges(String text) {
  int bytes = -1;
  int hash = -1;
  int codepoints = -1;
  final List<(int, int)> ranges = <(int, int)>[];
  for (final String raw in text.split('\n')) {
    final String line = raw.trim();
    if (line.isEmpty) continue;
    if (line.startsWith('#')) {
      final Match? mBytes = RegExp(r'^#\s*bytes:\s*(\d+)').firstMatch(line);
      if (mBytes != null) bytes = int.parse(mBytes.group(1)!);
      final Match? mHash = RegExp(r'^#\s*fnv1a32:\s*([0-9a-f]+)').firstMatch(line);
      if (mHash != null) hash = int.parse(mHash.group(1)!, radix: 16);
      final Match? mCp = RegExp(r'^#\s*codepoints:\s*(\d+)').firstMatch(line);
      if (mCp != null) codepoints = int.parse(mCp.group(1)!);
      continue;
    }
    final List<String> parts = line.split('-');
    final int lo = int.parse(parts[0], radix: 16);
    final int hi = parts.length > 1 ? int.parse(parts[1], radix: 16) : lo;
    ranges.add((lo, hi));
  }
  return SubsetRanges(
    bytes: bytes,
    hash: hash,
    codepoints: codepoints,
    ranges: ranges,
  );
}

/// 从 Dart 源码里抽出**字符串字面量**（跳过注释与代码）。
///
/// 必须跳过注释：本仓库的注释里**故意**写着 `▍` 与 `⌘`（解释为什么不能用它们），
/// 把注释算进来就会自己把自己判红。
///
/// 支持：单/双引号、三引号、`r'...'` 原始字符串、`\uXXXX` / `\u{...}` / `\xHH`
/// 转义（转义写法的字符同样会触发缺字回退，所以必须解码出来一起检查）。
List<String> stringLiterals(String src) {
  final List<String> out = <String>[];
  int i = 0;
  final int n = src.length;

  bool startsWith(String s, int at) => src.startsWith(s, at);

  while (i < n) {
    // 行注释
    if (src[i] == '/' && i + 1 < n && src[i + 1] == '/') {
      while (i < n && src[i] != '\n') {
        i++;
      }
      continue;
    }
    // 块注释（Dart 可嵌套）
    if (src[i] == '/' && i + 1 < n && src[i + 1] == '*') {
      int depth = 1;
      i += 2;
      while (i < n && depth > 0) {
        if (startsWith('/*', i)) {
          depth++;
          i += 2;
        } else if (startsWith('*/', i)) {
          depth--;
          i += 2;
        } else {
          i++;
        }
      }
      continue;
    }

    final bool raw = src[i] == 'r' &&
        i + 1 < n &&
        (src[i + 1] == "'" || src[i + 1] == '"');
    final int quoteAt = raw ? i + 1 : i;
    if (src[quoteAt] == "'" || src[quoteAt] == '"') {
      final String q = src[quoteAt];
      final bool triple = startsWith(q * 3, quoteAt);
      final int qLen = triple ? 3 : 1;
      int j = quoteAt + qLen;
      final StringBuffer buf = StringBuffer();
      while (j < n) {
        if (startsWith(q * qLen, j)) break;
        if (src[j] == r'\' && !raw) {
          final int esc = _decodeEscape(src, j);
          if (esc > 0) {
            buf.writeCharCode(esc);
            j += _escapeLength(src, j);
            continue;
          }
          if (j + 1 < n) buf.write(src[j + 1]);
          j += 2;
          continue;
        }
        if (!triple && src[j] == '\n') break; // 未闭合：兜底
        buf.write(src[j]);
        j++;
      }
      out.add(buf.toString());
      i = j + qLen;
      continue;
    }
    i++;
  }
  return out;
}

/// `\` 处转义序列的长度（不含反斜杠的字符数 + 1）。
int _escapeLength(String s, int at) {
  if (at + 1 >= s.length) return 1;
  final String c = s[at + 1];
  if (c == 'u' && at + 2 < s.length && s[at + 2] == '{') {
    final int close = s.indexOf('}', at + 3);
    return close < 0 ? 2 : close - at + 1;
  }
  if (c == 'u') return 6;
  if (c == 'x') return 4;
  return 2;
}

/// 解出转义序列表示的码点；不是码点转义（如 `\n`）返回 0。
int _decodeEscape(String s, int at) {
  if (at + 1 >= s.length) return 0;
  final String c = s[at + 1];
  try {
    if (c == 'u' && at + 2 < s.length && s[at + 2] == '{') {
      final int close = s.indexOf('}', at + 3);
      if (close < 0) return 0;
      return int.parse(s.substring(at + 3, close), radix: 16);
    }
    if (c == 'u' && at + 6 <= s.length) {
      return int.parse(s.substring(at + 2, at + 6), radix: 16);
    }
    if (c == 'x' && at + 4 <= s.length) {
      return int.parse(s.substring(at + 2, at + 4), radix: 16);
    }
  } on FormatException {
    return 0;
  }
  return 0;
}

void main() {
  group('① 抽取字符串字面量的小词法器（门禁自身的钉子）', () {
    test('注释里的字符**不算**（否则注释解释缺字反而把门禁判红）', () {
      const String src = '''
// 这里提到 ▍ 与 ⌘ 都不该被算进去
/* 块注释 ▍ */
final String a = '正文';
''';
      final List<String> lits = stringLiterals(src);
      expect(lits, contains('正文'));
      expect(lits.any((String s) => s.contains('▍')), isFalse);
      expect(lits.any((String s) => s.contains('⌘')), isFalse);
    });

    test('三引号与原始字符串、以及插值相邻的字面量都能抽到', () {
      const String src = '''
final a = """多行\n正文""";
final b = r'原样\\n';
final c = '甲' '乙';
''';
      final List<String> lits = stringLiterals(src);
      expect(lits, contains('甲'));
      expect(lits, contains('乙'));
    });

    test('码点转义会被解出来（转义写法一样会触发缺字回退）', () {
      final List<String> lits = stringLiterals(r"final a = '\u258D';");
      expect(lits.single.runes.toList(), <int>[0x258D]);
    });
  });

  group('② 覆盖表与字体文件必须同源（防止换了字体却没重新生成）', () {
    for (final String stem in _fontStems) {
      test('$stem：字节数与 FNV-1a 哈希一致', () {
        final File font = File('$stem.woff2');
        expect(font.existsSync(), isTrue, reason: '字体文件不见了：${font.path}');
        final File rangesFile = File('$stem.ranges.txt');
        expect(
          rangesFile.existsSync(),
          isTrue,
          reason: '覆盖表缺失：${rangesFile.path}\n'
              '生成：python3 scripts/font_subset_ranges.py',
        );

        final List<int> bytes = font.readAsBytesSync();
        final SubsetRanges ranges = parseRanges(rangesFile.readAsStringSync());

        expect(
          ranges.bytes,
          bytes.length,
          reason: '字体换了但覆盖表没重新生成 —— 拿旧表放行比没有门禁更危险。\n'
              '重新生成：python3 scripts/font_subset_ranges.py',
        );
        expect(
          ranges.hash,
          fnv1a32(bytes),
          reason: '字体内容变了（哈希不符），覆盖表必须重新生成：\n'
              'python3 scripts/font_subset_ranges.py',
        );
        expect(ranges.codepoints, greaterThan(1000), reason: '覆盖表看起来被改坏了');
        expect(ranges.ranges, isNotEmpty);
      });
    }
  });

  group('③ 界面文案里不得出现子集外的字符', () {
    test('lib/**/*.dart 的字符串字面量全部被自托管子集覆盖', () {
      final List<SubsetRanges> subsets = <SubsetRanges>[
        for (final String stem in _fontStems)
          parseRanges(File('$stem.ranges.txt').readAsStringSync()),
      ];

      // 违规字符 → 出现在哪些文件
      final Map<int, Set<String>> offenders = <int, Set<String>>{};
      final List<File> sources = Directory('lib')
          .listSync(recursive: true)
          .whereType<File>()
          .where((File f) => f.path.endsWith('.dart'))
          .toList();

      for (final File file in sources) {
        for (final String literal in stringLiterals(file.readAsStringSync())) {
          for (final int cp in literal.runes) {
            if (cp < 0x80) continue; // ASCII 必然覆盖
            if (subsets.every((SubsetRanges r) => r.covers(cp))) continue;
            offenders.putIfAbsent(cp, () => <String>{}).add(file.path);
          }
        }
      }

      final String detail = offenders.entries
          .map(
            (MapEntry<int, Set<String>> e) =>
                '  ${String.fromCharCode(e.key)}  U+${e.key.toRadixString(16).toUpperCase().padLeft(4, '0')}'
                '  ← ${e.value.join(', ')}',
          )
          .join('\n');

      expect(
        offenders,
        isEmpty,
        reason: '这些字符不在自托管字体子集里。CanvasKit 会去 fonts.gstatic.com\n'
            '拉回退字体 —— **有网时看不出来，断网就是豆腐块**。\n'
            '两种修法：① 改成不依赖字形的实现（如流式光标画竖条）；\n'
            '② 把它加进子集（重新子集化后跑 python3 scripts/font_subset_ranges.py）。\n'
            '违规字符：\n$detail',
      );
    });

    test('已知的两个历史违规字符确实不在子集里（门禁的校准位）', () {
      final SubsetRanges regular =
          parseRanges(File('${_fontStems.first}.ranges.txt').readAsStringSync());
      // 它们曾导致真实的外部字体请求；若哪天它们进了子集，本断言会提醒我们
      // 可以简化实现（流式光标 / Cmd 前缀不再需要绕开字形）。
      expect(regular.covers(0x258D), isFalse, reason: '▍ 已进子集 → 可考虑恢复字形实现');
      expect(regular.covers(0x2318), isFalse, reason: '⌘ 已进子集 → 可考虑恢复字形实现');
    });
  });
}
