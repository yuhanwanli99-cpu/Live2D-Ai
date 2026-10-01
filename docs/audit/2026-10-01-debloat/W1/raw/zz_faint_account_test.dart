/// 全仓 `contentFaint` 用法账（verifier 独立复算，复用被判据文件的真函数）。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'no_backdrop_filter_test.dart' as nb;

void main() {
  test('全仓 contentFaint 用法：文字 vs 装饰 逐条列出', () {
    final List<String> text = <String>[];
    final List<String> deco = <String>[];
    int total = 0;
    for (final FileSystemEntity e in Directory('lib').listSync(recursive: true)) {
      if (e is! File || !e.path.endsWith('.dart')) continue;
      final String src = e.readAsStringSync();
      if (!src.contains('contentFaint')) continue;
      for (final nb.FaintUse u in nb.findFaintUses(src)) {
        total++;
        final String row = '${e.path}:${u.line}  host=${u.host}  reason=${u.textReason}';
        (u.isText ? text : deco).add(row);
      }
    }
    // ignore: avoid_print
    print('TOTAL=$total  text=${text.length}  decoration=${deco.length}');
    for (final String r in text) {
      // ignore: avoid_print
      print('  TEXT  $r');
    }
    for (final String r in deco) {
      // ignore: avoid_print
      print('  DECO  $r');
    }
  });
}
