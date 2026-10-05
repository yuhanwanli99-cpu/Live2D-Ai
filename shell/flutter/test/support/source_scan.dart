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
String stripCommentsAndStrings(String src) => _scan(src, keepStrings: false);

/// 剥掉注释，**保留字符串字面量**（含引号）。
///
/// # 为什么需要第二档（2026-10-05，D6）
///
/// 两个消费点要断言的**就是字面量本身**：
/// - `chat_bubble_labels_test.dart` 断言 `chat_controller.dart` 里不再出现
///   `'本轮没有文字输出…'` 那句伪台词、且仍有 `'（生成失败）'`；
/// - `director_observer_test.dart` 把「字符串里出现持久化名字」也当落盘信号。
/// 它们过去各自带一份私有的 `_stripComments`（同名不同义：一份认识字符串、
/// 一份不认识），两份都在 `test/` 下——正是反复制门禁要防的形态。
/// 现在合并到这一处：**字符串认识**（更严：`'http://x'` 里的 `//` 不再被
/// 当成注释起点，藏在字符串后半段的标识符不会被静默丢掉），块注释按 Dart
/// 语义**可嵌套**。
///
/// 与 [stripCommentsAndStrings] 共用同一个扫描器（只是字面量留不留），
/// 所以两档的注释语义逐字一致——不存在「同一件事两套解释」。
String stripCommentsKeepStrings(String src) => _scan(src, keepStrings: true);

/// 扫描器本体：剥注释；字符串按 [keepStrings] 决定留不留。
String _scan(String src, {required bool keepStrings}) {
  final StringBuffer out = StringBuffer();
  int i = 0;
  while (i < src.length) {
    final String c = src[i];
    final bool rawString = c == 'r' && _opensRawString(src, i);
    if (c == "'" || c == '"' || rawString) {
      // raw 前缀（`r`）与字面量一起丢：它只在字面量里有意义，留着会让
      // `RegExp(r)` 这种残渣看起来像代码。（`keepStrings` 时整段照抄，
      // 包括 `r` 前缀与两侧引号——消费点比的是字面量原文。）
      final int literalStart = i;
      final int end = _skipStringLiteral(
        src,
        rawString ? i + 1 : i,
        raw: rawString,
      );
      if (keepStrings) out.write(src.substring(literalStart, end));
      i = end;
      continue;
    }
    if (c == '/' && i + 1 < src.length && src[i + 1] == '/') {
      while (i < src.length && src[i] != '\n') {
        i++;
      }
      continue;
    }
    if (c == '/' && i + 1 < src.length && src[i + 1] == '*') {
      // 块注释按 Dart 语义**可嵌套**（`director_observer_test.dart` 的私有版
      // 一直这么做）。旧的严格版在第一个 `*/` 收尾，会把嵌套注释的尾部当成
      // 源码——那是**假命中**的方向（本该零命中的门禁可能因此变红）。
      int depth = 1;
      i += 2;
      while (i < src.length && depth > 0) {
        if (src[i] == '/' && i + 1 < src.length && src[i + 1] == '*') {
          depth++;
          i += 2;
        } else if (src[i] == '*' && i + 1 < src.length && src[i + 1] == '/') {
          depth--;
          i += 2;
        } else {
          i++;
        }
      }
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

/// 取 [anchor] 处那个**平衡括号**包起来的整段（含括号本身）。
///
/// 用来把扫描范围收在**这一个调用**上，而不是 `whole file contains`
/// （后者会让「文件里别处也调了同一个东西」冒充这条接线）。
///
/// 2026-10-05（D6）：从 `error_action_opens_settings_test.dart` 的私有实现
/// 搬到这里——F-0007-2 的结构守卫要用同一份判据，抄一份就是又一次「同一件事
/// 两套解释」（那条门禁见 `test/source_scan_test.dart`）。
String balancedFrom(String src, String anchor, String open, String close) {
  final int at = src.indexOf(anchor);
  if (at < 0) throw StateError('找不到 `$anchor`');
  final int start = src.indexOf(open, at);
  if (start < 0) throw StateError('`$anchor` 后面没有 `$open`');
  final int end = matchBracket(src, start, open, close);
  if (end < 0) throw StateError('`$anchor` 的括号不平衡');
  return src.substring(start, end + 1);
}

/// 取 [anchor]（形如 `onGoto:`）之后那个**闭包体** `{ … }`（含花括号本身）。
///
/// 支持 `(参数) { … }` 与 `() async { … }`；值不是闭包（例如裸 tear-off）返回 null。
String? closureBodyAfter(String src, String anchor) {
  final int at = src.indexOf(anchor);
  if (at < 0) return null;
  int i = at + anchor.length;
  while (i < src.length && _isSpace(src[i])) {
    i++;
  }
  if (i < src.length && src[i] == '(') {
    final int end = matchBracket(src, i, '(', ')');
    if (end < 0) return null;
    i = end + 1;
  }
  while (i < src.length && _isSpace(src[i])) {
    i++;
  }
  if (i < src.length && src.startsWith('async', i)) i += 'async'.length;
  while (i < src.length && _isSpace(src[i])) {
    i++;
  }
  if (i >= src.length || src[i] != '{') return null;
  final int end = matchBracket(src, i, '{', '}');
  return end < 0 ? null : src.substring(i, end + 1);
}

bool _isSpace(String c) => c == ' ' || c == '\n' || c == '\r' || c == '\t';

/// [open] 在 [start] 处的配对位置（下标）；不平衡返回 -1。
int matchBracket(String src, int start, String open, String close) {
  int depth = 0;
  for (int i = start; i < src.length; i++) {
    final String c = src[i];
    if (c == open) {
      depth++;
    } else if (c == close) {
      depth--;
      if (depth == 0) return i;
    }
  }
  return -1;
}
