/// 共享扫描工具的**契约**与**反复制门禁**（2026-10-01 W3-D3 首版；
/// 2026-10-05 D6 扩到「每个共享定义点唯一」）。
///
/// # 这个文件守两件事
///
/// 1. **契约**：`test/support/source_scan.dart` 里的扫描工具真的做它说的事——
///    `stripCommentsAndStrings` 剥掉注释与字面量、`stripCommentsKeepStrings`
///    只剥注释（字面量原文留给消费点比）、块注释按 Dart 语义可嵌套、
///    括号配平取段（`balancedFrom` / `closureBodyAfter`）不会把范围外的东西算进来。
/// 2. **反复制**：`test/**` 里 [kSingleDefinitionHelpers] 列出的每个定义
///    **只允许一个定义点**。从前 `stripCommentsAndStrings` 有 8 份，其中
///    `design_tokens_test.dart` 那份漂移成了占位符 `S`；2026-10-05 之前
///    `_stripComments` 还各有两份私有副本（**同名不同义**）——重复已经漂过
///    两次，这条门禁就是那两次事故的产物。
///
/// **防零命中空转**：判据是「顶格定义」正则（[definitionPattern]），
/// 不是 `contains`。若定义写法变了导致正则一个都扫不到，下面的断言会**红**
/// （期望值是有且仅有一处），而不是安静地「零命中=通过」。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'support/source_scan.dart';

/// **只允许一个定义点**的共享工具：`名字 → 唯一定义所在文件`。
///
/// 只收「被多个测试消费、抄一份就会漂」的工具。某个测试文件里只此一处用的
/// 私有小 helper 不进表——那不算重复。
const Map<String, String> kSingleDefinitionHelpers = <String, String>{
  'stripCommentsAndStrings': 'test/support/source_scan.dart',
  'stripCommentsKeepStrings': 'test/support/source_scan.dart',
  'balancedFrom': 'test/support/source_scan.dart',
  'closureBodyAfter': 'test/support/source_scan.dart',
  'matchBracket': 'test/support/source_scan.dart',
};

/// 一个**定义**长什么样（不是「提到过这个名字」）。
///
/// 顶格写、返回类型 + 名字 + `(`：注释里提一句、调用点（缩进）、`show` 子句
/// 都不会命中。模式写在 raw string 里（带 `\s`），所以本文件自己
/// （也在 `test/**` 下）不会被它命中——这一点很关键，否则门禁会把自己数进去。
RegExp definitionPattern(String name) => RegExp(
  r'^[A-Za-z_][\w<>,?\[\] ]*\s+' + RegExp.escape(name) + r'\s*\(',
  multiLine: true,
);

void main() {
  group('词法器契约：注释与字符串都要真的消失', () {
    test('行注释丢到行尾，**换行保留**（报告 `路径:行号` 才不漂）', () {
      expect(
        stripCommentsAndStrings('a // 注释 b\nc\n'),
        'a \nc\n',
        reason: '行注释把换行也吃掉的话，后面所有行号都会错位',
      );
    });

    test('块注释整段丢掉（含内部换行——与合并前的 8 份逐字一致）', () {
      expect(stripCommentsAndStrings('a/* x\ny */b'), 'ab');
      expect(stripCommentsAndStrings('a /* x */ b'), 'a  b');
    });

    test('字符串字面量整段丢掉（里面的 `//` 不是注释起点）', () {
      expect(
        stripCommentsAndStrings("final s = 'http://x'; // 尾注释\n"),
        'final s = ; \n',
        reason: "`'http://x'` 里的 `//` 被当成注释，会把这一行后半段也吃掉",
      );
    });

    test('字符串里的令牌名不算引用（词法器存在的唯一理由）', () {
      final String stripped = stripCommentsAndStrings(
        "// Space.s1\nconst String doc = 'Space.s1';\nfinal v = Space.s1;\n",
      );
      expect(RegExp(r'Space\.s1').allMatches(stripped), hasLength(1));
    });

    test('转义引号不当收尾（单引号与双引号都在字面量内部）', () {
      expect(
        stripCommentsAndStrings(r"final a = 'it\'s'; final b = 1;"),
        'final a = ; final b = 1;',
      );
      expect(
        stripCommentsAndStrings('final a = "say \\"hi\\""; final b = 2;'),
        'final a = ; final b = 2;',
      );
    });

    test('三引号（单行与跨行都一样整段丢）', () {
      expect(stripCommentsAndStrings("x = '''a'b''' ;"), 'x =  ;');
      expect(stripCommentsAndStrings('x = """a\nb""" ;'), 'x =  ;');
      expect(stripCommentsAndStrings("x = '''a\n// 不是注释\nb''' ;"), 'x =  ;');
    });

    test('raw 字符串：反斜杠不是转义（`r` 前缀一起丢，不留残渣）', () {
      expect(
        stripCommentsAndStrings(r"final p = r'\d+'; final q = 1;"),
        'final p = ; final q = 1;',
      );
      // 无效 Dart 也照样安全：raw 里 `\'` 是「反斜杠 + 收尾引号」，
      // 后面的代码**不许**被吞掉（旧的「一律当转义」实现会吞掉整个文件尾部）。
      expect(
        stripCommentsAndStrings(r"final p = r'a\' ; final q = 1;"),
        'final p =  ; final q = 1;',
        reason: 'raw 字面量里把 `\\` 当转义 → 找不到收尾引号 → 吞掉后面整个文件',
      );
    });

    test('标识符里的 `r` 不是 raw 前缀', () {
      expect(
        stripCommentsAndStrings("var r = 1; final s = r'x'; var t = 2;"),
        'var r = 1; final s = ; var t = 2;',
      );
    });

    test('未闭合的字符串/注释不抛异常（返回原样尾部，交给 analyze 去喊）', () {
      expect(stripCommentsAndStrings("final a = 'x"), 'final a = ');
      expect(stripCommentsAndStrings('a /* b'), 'a ');
    });
  });

  group('保留字面量那一档（D6 合并后的唯一形态）', () {
    test('字符串字面量整段保留（含引号）', () {
      expect(
        stripCommentsKeepStrings("final s = '（生成失败）'; // 说明\n"),
        "final s = '（生成失败）'; \n",
        reason: '消费点比的就是字面量原文，引号也要在',
      );
    });

    test('字符串里的 `//` 不是注释起点（比旧私有版更严，不许丢后半行）', () {
      // 旧 `director_observer_test` 的私有版**不认识字符串**：`'http://x'`
      // 会被从 `//` 起吃掉，行尾的标识符一起消失——而那一行恰好是
      // 「有没有写盘」的证据，丢了就是**假绿灯**。
      expect(
        stripCommentsKeepStrings("const u = 'http://x'; saveDisplayPrefs();\n"),
        "const u = 'http://x'; saveDisplayPrefs();\n",
      );
    });

    test('注释仍被剥掉（两档的注释语义逐字一致）', () {
      expect(stripCommentsKeepStrings('a // b\nc'), 'a \nc');
      expect(stripCommentsKeepStrings("a /* 'x' */ b"), 'a  b');
    });

    test('raw 字符串整段保留（`r` 前缀 + 引号）', () {
      expect(
        stripCommentsKeepStrings(r"final p = r'\d+'; final q = 1;"),
        r"final p = r'\d+'; final q = 1;",
      );
    });

    test('块注释可嵌套（两档一致；旧严格版在第一个 `*/` 就收尾）', () {
      // 嵌套注释的尾部若被当成源码，就会**假命中**本该零命中的门禁。
      // 判别力：非嵌套语义会留下 ` z */b`（本断言就是那件事的反面）。
      expect(stripCommentsAndStrings('a/* x /* y */ z */b'), 'ab');
      expect(stripCommentsKeepStrings('a/* x /* y */ z */b'), 'ab');
      expect(
        stripCommentsAndStrings('a/* x /* y */ z */b').contains('z'),
        isFalse,
        reason: '嵌套注释的尾部不许漏成源码',
      );
    });
  });

  group('括号配平取段（结构守卫共用的判据）', () {
    test('balancedFrom 只取那一个调用的实参（文件里别处的不算）', () {
      const String src = 'f(a); g(x, h(y)); z();';
      expect(balancedFrom(src, 'g(', '(', ')'), '(x, h(y))');
    });

    test('balancedFrom 对找不到 / 不平衡的锚点抛（不静默返回空串）', () {
      expect(() => balancedFrom('f(a', 'f(', '(', ')'), throwsStateError);
      expect(() => balancedFrom('xx', 'f(', '(', ')'), throwsStateError);
    });

    test('closureBodyAfter 认 `() {…}` / `() async {…}`，裸 tear-off 返回 null', () {
      expect(closureBodyAfter('onGoto: () { a(); }', 'onGoto:'), '{ a(); }');
      expect(
        closureBodyAfter('onGoto: () async { a(); }', 'onGoto:'),
        '{ a(); }',
      );
      expect(closureBodyAfter('onGoto: handle,', 'onGoto:'), isNull);
      expect(closureBodyAfter('nope', 'onGoto:'), isNull);
    });
  });

  group('反复制门禁：每个共享工具只允许一个定义点', () {
    /// 全部测试源码（递归、含 support/）。
    List<File> testSources() =>
        Directory('test')
            .listSync(recursive: true)
            .whereType<File>()
            .where((File f) => f.path.endsWith('.dart'))
            .toList();

    List<String> definitionSites(List<File> sources, String name) {
      final RegExp pattern = definitionPattern(name);
      return <String>[
        for (final File file in sources)
          if (pattern.hasMatch(file.readAsStringSync()))
            file.path.replaceAll(r'\', '/'),
      ];
    }

    test('扫描确实覆盖到了测试源码（防路径写错导致「零命中=通过」）', () {
      // 最危险的假通过：路径写错 → 一个文件都没扫 → 门禁空转通过。
      final List<File> sources = testSources();
      expect(
        sources.length,
        greaterThan(30),
        reason: '扫到的测试源码太少 —— 路径写错了，下面的门禁是空转',
      );
      expect(
        sources.any((File f) => f.path.endsWith('support/source_scan.dart')),
        isTrue,
        reason: '连共享工具自己的文件都没扫到 —— 门禁不可能有判别力',
      );
    });

    test('每个共享定义恰好出现在 `test/support/source_scan.dart`', () {
      final List<File> sources = testSources();
      expect(kSingleDefinitionHelpers, isNotEmpty, reason: '空表 = 门禁空转');
      for (final MapEntry<String, String> e
          in kSingleDefinitionHelpers.entries) {
        final List<String> sites = definitionSites(sources, e.key);
        expect(
          sites,
          <String>[e.value],
          reason:
              '`${e.key}` 的定义点不是「恰好一处」。**零命中同样判红**——'
              '那说明判据正则与源码形状脱节了，门禁正在空转（比多一份副本更危险）。'
              '多一份副本则说明又抄了一遍：8 份 `stripCommentsAndStrings` 与 '
              '2 份 `_stripComments` 都**真的漂过**，请 import `support/source_scan.dart`。',
        );
      }
    });

    test('门禁的判据是「定义」而不是「提到过这个名字」', () {
      // 自证：本文件、以及所有消费点都大量提到这些名字（注释 + 调用 +
      // 上面的表），但只有一处是**定义**。判据写松（比如 contains）就会红。
      final String own = File('test/source_scan_test.dart').readAsStringSync();
      for (final String name in kSingleDefinitionHelpers.keys) {
        expect(own.contains(name), isTrue, reason: '本文件应当提到 $name');
        expect(
          definitionPattern(name).hasMatch(own),
          isFalse,
          reason: '门禁把自己数进去了 —— 判据太松，会把「提到」当成「定义」（$name）',
        );
      }
    });
  });
}
