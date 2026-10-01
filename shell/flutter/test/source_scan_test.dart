/// 共享词法器 `stripCommentsAndStrings` 的**契约**与**反复制门禁**
/// （2026-10-01，W3-D3）。
///
/// # 这个文件守两件事
///
/// 1. **契约**：`test/support/source_scan.dart` 里那份词法器真的能剥掉注释与
///    字符串字面量——三引号、`//`、`/* */`、转义、raw string 都按文档走。
///    它是**唯一**一份（下面第二条门禁），所以它的语义就是全部 11 个消费点
///    的语义。
/// 2. **反复制**：`test/**` 里这个函数**只允许一个定义点**。从前有 8 份，
///    其中 `design_tokens_test.dart` 那份漂移成了占位符 `S`（其余 7 份整段
///    丢弃）——重复**已经**漂过一次，这条门禁就是那次事故的产物：新抄一份
///    就会红，而不是等它漂。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'support/source_scan.dart';

/// 一个**定义**长什么样（不是「出现过这个名字」）。
///
/// 顶格写、带完整签名、紧跟 `{`：注释里提一句、调用点、`show` 子句都不会命中。
/// 正则写在 raw string 里（带 `\s+`），所以本文件自己的源码**不会**被它自己
/// 命中——这一点很关键，否则门禁会把自己数进去，恒为 2。
final RegExp kStripDefinition = RegExp(
  r'^String\s+stripCommentsAndStrings\s*\(\s*String\s+\w+\s*\)\s*\{',
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
      expect(stripCommentsAndStrings(r"final a = 'it\'s'; final b = 1;"), 'final a = ; final b = 1;');
      expect(stripCommentsAndStrings('final a = "say \\"hi\\""; final b = 2;'), 'final a = ; final b = 2;');
    });

    test('三引号（单行与跨行都一样整段丢）', () {
      expect(stripCommentsAndStrings("x = '''a'b''' ;"), 'x =  ;');
      expect(stripCommentsAndStrings('x = """a\nb""" ;'), 'x =  ;');
      expect(stripCommentsAndStrings("x = '''a\n// 不是注释\nb''' ;"), 'x =  ;');
    });

    test('raw 字符串：反斜杠不是转义（`r` 前缀一起丢，不留残渣）', () {
      expect(stripCommentsAndStrings(r"final p = r'\d+'; final q = 1;"), 'final p = ; final q = 1;');
      // 无效 Dart 也照样安全：raw 里 `\'` 是「反斜杠 + 收尾引号」，
      // 后面的代码**不许**被吞掉（旧的「一律当转义」实现会吞掉整个文件尾部）。
      expect(
        stripCommentsAndStrings(r"final p = r'a\' ; final q = 1;"),
        'final p =  ; final q = 1;',
        reason: 'raw 字面量里把 `\\` 当转义 → 找不到收尾引号 → 吞掉后面整个文件',
      );
    });

    test('标识符里的 `r` 不是 raw 前缀', () {
      expect(stripCommentsAndStrings("var r = 1; final s = r'x'; var t = 2;"), 'var r = 1; final s = ; var t = 2;');
    });

    test('未闭合的字符串/注释不抛异常（返回原样尾部，交给 analyze 去喊）', () {
      expect(stripCommentsAndStrings("final a = 'x"), 'final a = ');
      expect(stripCommentsAndStrings('a /* b'), 'a ');
    });
  });

  group('反复制门禁：`test/**` 里只允许一个定义点', () {
    /// 全部测试源码（递归、含 support/）。
    List<File> testSources() => Directory('test')
        .listSync(recursive: true)
        .whereType<File>()
        .where((File f) => f.path.endsWith('.dart'))
        .toList();

    test('扫描确实覆盖到了测试源码（防路径写错导致「零命中=通过」）', () {
      // 最危险的假通过：路径写错 → 一个文件都没扫 → 门禁空转通过。
      final List<File> sources = testSources();
      expect(
        sources.length,
        greaterThan(30),
        reason: '扫到的测试源码太少 —— 路径写错了，下面的门禁是空转',
      );
    });

    test('唯一定义在 `test/support/source_scan.dart`（多一份就红）', () {
      final List<String> definitionSites = <String>[];
      for (final File file in testSources()) {
        if (kStripDefinition.hasMatch(file.readAsStringSync())) {
          definitionSites.add(file.path.replaceAll(r'\', '/'));
        }
      }
      expect(
        definitionSites,
        <String>['test/support/source_scan.dart'],
        reason:
            '`stripCommentsAndStrings` 又被抄了一份 —— 8 份副本曾经漂移过一次'
            '（`design_tokens_test.dart` 那份把字符串换成了占位符 `S`）。'
            '新代码请 import `support/source_scan.dart`。',
      );
    });

    test('门禁的判据是「定义」而不是「提到过这个名字」', () {
      // 自证：本文件、以及所有消费点都大量提到这个名字（注释 + 调用），
      // 但只有一处是**定义**。判据写松（比如 contains('stripCommentsAndStrings')）
      // 的话下面这条会红。
      final String own = File('test/source_scan_test.dart').readAsStringSync();
      expect(own.contains('stripCommentsAndStrings'), isTrue);
      expect(
        kStripDefinition.hasMatch(own),
        isFalse,
        reason: '门禁把自己也数进去了 —— 判据太松，会把「提到」当成「定义」',
      );
    });
  });
}
