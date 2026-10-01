/// 源码扫描的**共享词法器**（2026-10-01，W3-D3 重复消除）。
///
/// # 为什么会有这个文件
///
/// `stripCommentsAndStrings` 过去在 `test/` 下**有 8 份实现**，而重复已经
/// 真的漂了：
///
/// | 位置 | 字符串字面量怎么处理 |
/// | --- | --- |
/// | 另外 7 份（含 `motion_wiring_test.dart` 那份「先例」） | 整段丢掉（严格） |
/// | `design_tokens_test.dart`（外加 3 个 `show` 它的文件） | 写成一个占位符 `S` |
///
/// 第 8 份的漂移**不是有意为之**：它与另外 7 份同源于一个根提交
/// （`a11fe515`），既没有注释说明、也没有提交信息解释理由；而且没有任何
/// 消费点依赖那个 `S`——把 4 个设计规则文件切到严格版后全部仍然绿
/// （本次合并时逐一跑过）。所以这里取**最严的那份**当唯一定义，
/// 占位符写法不保留：留一个没人依赖的第二种语义，就是把「同一件事有两套
/// 解释」重新种回去，下一次漂移只是时间问题。
///
/// 反复制门禁在 `test/source_scan_test.dart`（`test/**` 里只允许这一个定义点）。
///
/// # 契约（严格版）
///
/// | 输入 | 输出 |
/// | --- | --- |
/// | `// 注释` | 丢到行尾；**换行保留**（行号不漂，报告 `路径:行号` 才有意义） |
/// | `/* 注释 */` | 整段丢掉；**内部换行也丢**——与合并前的 8 份逐字一致，刻意不改 |
/// | `'…'` / `"…"` / `'''…'''` / `"""…"""` | 整段丢掉（里面的 `//`、`/*`、`$` 都不再是代码） |
/// | 字符串里的 `\'` / `\"` | 不当收尾（单行与三引号都按转义走） |
/// | `r'…'` / `r"…"` / `r'''…'''` | 整段丢掉，且**反斜杠不是转义**；`r` 前缀一起丢（不留残渣） |
///
/// **已知边界（刻意不处理，与合并前的 8 份一致）**：不解析字符串插值里的
/// 嵌套引号——`'${m['k']}'` 会在内层引号处提前收尾。`lib/` 与 `test/`
/// 现在没有这种写法；真要支持得写完整词法器，代价不成比例。这份边界写在
/// 这里而不是藏着：扫描「调用形状」的测试若遇到它，命中行文本里看得见。
library;

/// 剥掉注释与字符串字面量（严格版）。
///
/// 传进来的应当是**未剥的原文**；返回文本里注释与字符串字面量都消失，
/// 但**代码部分的换行一个不多一个不少**（行注释只吃掉行内内容）。
String stripCommentsAndStrings(String src) {
  final StringBuffer out = StringBuffer();
  int i = 0;
  while (i < src.length) {
    final String c = src[i];
    final bool rawString = c == 'r' && _opensRawString(src, i);
    if (c == "'" || c == '"' || rawString) {
      // raw 前缀（`r`）与字面量一起丢：它只在字面量里有意义，留着会让
      // `RegExp(r)` 这种残渣看起来像代码。
      i = _skipStringLiteral(src, rawString ? i + 1 : i, raw: rawString);
      continue;
    }
    if (c == '/' && i + 1 < src.length && src[i + 1] == '/') {
      while (i < src.length && src[i] != '\n') {
        i++;
      }
      continue;
    }
    if (c == '/' && i + 1 < src.length && src[i + 1] == '*') {
      i += 2;
      while (i + 1 < src.length && !(src[i] == '*' && src[i + 1] == '/')) {
        i++;
      }
      i += 2;
      continue;
    }
    out.write(c);
    i++;
  }
  return out.toString();
}

/// `src[i]` 的 `r` 是不是紧挨着引号的 **raw 字符串前缀**（而不是标识符里那个字母）。
///
/// 判据：右边必须是引号；左边不能是标识符字符——`var r = 1;` 的 `r` 因此
/// 不会被误当成前缀（那条路会走进 `_skipStringLiteral` 的引号判断，而后面
/// 根本没有引号）。
bool _opensRawString(String src, int i) {
  if (i + 1 >= src.length) return false;
  final String next = src[i + 1];
  if (next != "'" && next != '"') return false;
  if (i == 0) return true;
  final String prev = src[i - 1];
  final int code = prev.codeUnitAt(0);
  final bool prevIsIdentifierChar =
      (code >= 0x30 && code <= 0x39) || // 0-9
      (code >= 0x41 && code <= 0x5A) || // A-Z
      (code >= 0x61 && code <= 0x7A) || // a-z
      prev == '_' ||
      prev == r'$';
  return !prevIsIdentifierChar;
}

/// 从 [start]（指向开引号）跳过整个字符串字面量，返回闭合引号**之后**的下标。
///
/// [raw] 为真时反斜杠不是转义（Dart 的 `r'…'`）。
/// 未闭合时返回 `src.length`：扫描器不抛异常（源码坏了该由 `flutter analyze`
/// 喊，不该让门禁测试自己崩），这与合并前的 8 份行为一致。
int _skipStringLiteral(String src, int start, {required bool raw}) {
  final String quote = src[start];
  final bool triple =
      start + 2 < src.length &&
      src[start + 1] == quote &&
      src[start + 2] == quote;
  final String closing = triple ? quote + quote + quote : quote;
  int i = start + closing.length;
  while (i < src.length) {
    if (!raw && src[i] == r'\') {
      i += 2;
      continue;
    }
    if (src.startsWith(closing, i)) return i + closing.length;
    i++;
  }
  return src.length;
}
