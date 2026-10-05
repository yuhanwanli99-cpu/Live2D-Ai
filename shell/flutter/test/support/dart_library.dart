/// 读一个 Dart **库**的完整源码：库文件 + 它声明的全部 `part` 文件（递归）。
///
/// # 为什么要有这一层（2026-10-06，R4-T2）
///
/// `test/` 里有一批**源码扫描守卫**（断链守卫 / 结构守卫 / 不许复活守卫）。
/// 它们锚的语义是「这一段源码里有没有这个东西」，而 `part` 拆分把**同一段
/// 代码**搬到了同库的另一个文件里：只读库文件本身，守卫会**静默失效**
/// （读到的是一半，断言看起来还绿）。所以扫描面取**并集**：库 + 它的 parts。
///
/// 先例：`setting_wiring_test.appearanceSectionSource()` 当年就是手写两个
/// 文件的字符串相加（2026-09-28）；拆分第二次之后手写会漏，于是抽到这里——
/// **一份实现**，与 `support/source_scan.dart` 同一条纪律（test/ 里只允许一个定义点）。
library;

import 'dart:io';

/// 库文件 [path] 的完整源码（含它 `part` 进来的每个文件，深度优先、去重）。
String readLibrarySource(String path, {Set<String>? seen}) {
  final Set<String> visited = seen ?? <String>{};
  final File file = File(path);
  final String abs = file.absolute.path;
  if (!visited.add(abs)) return '';
  final String src = file.readAsStringSync();
  final StringBuffer out = StringBuffer(src);
  final RegExp partDirective = RegExp(r"^part\s+'([^']+)';", multiLine: true);
  for (final RegExpMatch m in partDirective.allMatches(src)) {
    final Uri resolved = file.parent.uri.resolve(m.group(1)!);
    out
      ..writeln()
      ..write(readLibrarySource(resolved.toFilePath(), seen: visited));
  }
  return out.toString();
}
