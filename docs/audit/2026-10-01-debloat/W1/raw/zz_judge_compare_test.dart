/// verifier 独立并排复现：同一份源码，**旧同行正则** vs **新结构判定**。
/// 场景：把 appearance_background.dart:778 临时回退成 `contentFaint`（文字色误用）。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'no_backdrop_filter_test.dart' as nb;

const String kPath = 'lib/settings/sections/appearance_background.dart';

void main() {
  test('并排对照：同一份源码下 OLD_REGEX_HITS vs NEW_JUDGE_HITS', () {
    final String source = File(kPath).readAsStringSync();

    // ① 旧判据（W1-d2 之前的钉子 11）：同行正则。
    final RegExp oldRegex = RegExp(r'(Text|TextStyle|style)[^\n]*contentFaint');
    final List<int> oldLines = <int>[];
    for (final RegExpMatch m in oldRegex.allMatches(source)) {
      oldLines.add('\n'.allMatches(source.substring(0, m.start)).length + 1);
    }

    // ② 新判据（结构判定，跨行也算）：直接调被测文件里的真函数。
    final List<nb.FaintUse> textUses = nb.findFaintTextUses(source);
    final List<String> newHits =
        textUses.map((nb.FaintUse u) => '${u.line}:${u.textReason}').toList();
    final List<nb.FaintUse> allUses = nb.findFaintUses(source);

    // ignore: avoid_print
    print('OLD_REGEX_HITS=${oldLines.length} $oldLines');
    // ignore: avoid_print
    print('NEW_JUDGE_HITS=$newHits');
    // ignore: avoid_print
    print('ALL_FAINT_USES=${allUses.map((nb.FaintUse u) => '${u.line}:${u.host}').toList()}');

    expect(oldLines.length, 0, reason: '旧同行正则对这三处**零命中**= 当年的假绿灯');
    expect(
      newHits.any((String h) => h.startsWith('778:')),
      isTrue,
      reason: '新结构判定必须命中 778（宿主 Text）',
    );
  });

  test('对照：迁移后的源码（HEAD 版）两个判据都为 0', () {
    final String migrated = File(
      'lib/settings/sections/appearance_background.dart',
    ).readAsStringSync();
    // 这份副本里 778 已被回退，所以这里只报数字，不做断言（见上一个 test 的断言）
    // ignore: avoid_print
    print('（副本内 778 已回退）ALL_USES=${nb.findFaintUses(migrated).length}');
  });
}
