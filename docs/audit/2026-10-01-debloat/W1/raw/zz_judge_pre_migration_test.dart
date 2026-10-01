/// 复核「774 vs 778」的行号来源：对**迁移前**的源码（1d0c6e66 版）跑同一判据。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'no_backdrop_filter_test.dart' as nb;

void main() {
  test('迁移前源码（1d0c6e66）上的旧/新判据', () {
    final String src =
        File('test/zz_pre_migration_source.dart.txt').readAsStringSync();
    final RegExp old = RegExp(r'(Text|TextStyle|style)[^\n]*contentFaint');
    final List<int> oldLines = <int>[];
    for (final RegExpMatch m in old.allMatches(src)) {
      oldLines.add('\n'.allMatches(src.substring(0, m.start)).length + 1);
    }
    final List<String> neu = nb
        .findFaintTextUses(src)
        .map((nb.FaintUse u) => '${u.line}:${u.textReason}')
        .toList();
    // ignore: avoid_print
    print('PRE_MIGRATION OLD_REGEX_HITS=${oldLines.length} $oldLines');
    // ignore: avoid_print
    print('PRE_MIGRATION NEW_JUDGE_HITS=$neu');
    expect(oldLines.length, 0);
  });
}
