/// 聊天气泡的**轻量 Markdown** 解析（纯 Dart，零 import）。
///
/// # 为什么只做「一点点」（2026-09-11，P2-3）
///
/// LLM 的输出天然带 Markdown。过去这些字符是**原样进 `SelectableText`**
/// 的，于是用户在气泡里读到 `**重点**`、`- 第一条`、`` `code` `` 的字面符号
/// ——设置文案那边早就修过了（`ui/emphasized_text.dart`），但那条修复只覆盖
/// 硬编码文案，**运行时的模型输出不受它保护**。
///
/// 反面选项是引入 `flutter_markdown`，但那会：
/// ① 违反「运行时依赖 = 0」（`shell/README.md` §依赖约束）；
/// ② 带来一整套标题/引用/表格/嵌套样式，而气泡只有 320–340 px 宽，
///    那些排版在这里只会更糟。
///
/// 所以只支持**三类**，都是「不渲染就等于漏符号」的：
///
/// | 语法 | 渲染成 |
/// | --- | --- |
/// | `**粗体**` | 粗体 |
/// | `` `行内码` `` | 等宽 + 底色块 |
/// | `- ` / `* ` 开头的行 | 一个 `·` 开头的独立行（真的成行，不再挤成一坨） |
///
/// # 刻意**不**做的
///
/// - **标题（`#`）**：气泡里没有比正文更大的字级可用（字号阶梯只有 8 档，
///   且气泡内的层级不该由模型决定）。
/// - **代码块（```）**：那需要等宽多行 + 横向滚动 + 复制，属于「真的要做
///   Markdown」的范畴；本项目的边界是「不堆砌功能」（`spec-v3` §13.2②）。
/// - **链接**：气泡不可点（点开会把用户带离桌宠），渲染成链接色反而误导。
///
/// # 解析原则：宁可少认，不可吞字
///
/// 落单的 `**`、没闭合的 `` ` `` **一律原样保留**。理由同
/// `emphasized_text.dart`：文案（这里指模型输出）写错了要看得见，
/// 而不是被解析器静默吃掉一半——那会变成「模型说的话不见了」。
library;

/// 一段行内内容。
///
/// 三种形态互斥（`bold` / `code` 不会同时为真：`` **`x`** `` 里的反引号
/// 在粗体分支里不二次解析——嵌套只会让「模型想说什么」更难读）。
class MdSpan {
  const MdSpan(this.text, {this.bold = false, this.code = false});

  final String text;
  final bool bold;
  final bool code;

  @override
  bool operator ==(Object other) =>
      other is MdSpan &&
      other.text == text &&
      other.bold == bold &&
      other.code == code;

  @override
  int get hashCode => Object.hash(text, bold, code);

  @override
  String toString() =>
      'MdSpan(${code ? 'code' : bold ? 'bold' : 'text'}: ${text.replaceAll('\n', r'\n')})';
}

/// 一块内容：普通段落，或一条列表项。
class MdBlock {
  const MdBlock(this.spans, {this.bullet = false});

  final List<MdSpan> spans;

  /// `true` = 这是一个列表项（渲染时前面加 `·` 并独立成行）。
  final bool bullet;

  @override
  bool operator ==(Object other) =>
      other is MdBlock &&
      other.bullet == bullet &&
      _listEquals(other.spans, spans);

  @override
  int get hashCode => Object.hash(bullet, Object.hashAll(spans));

  @override
  String toString() => 'MdBlock(bullet: $bullet, $spans)';
}

bool _listEquals(List<MdSpan> a, List<MdSpan> b) {
  if (a.length != b.length) return false;
  for (int i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

/// 列表项打头的记号（`- ` / `* ` / `• `）。
final RegExp _bulletPrefix = RegExp(r'^\s*[-*•]\s+');

/// 把模型输出解析成块列表。
///
/// **行是唯一的块边界**：不处理「同一行里既结束了列表又开了段落」这种
/// 模糊情况——模型输出里它几乎不出现，而处理它会让规则复杂到没人敢改。
List<MdBlock> parseChatMarkdown(String raw) {
  if (raw.isEmpty) return const <MdBlock>[];

  final List<MdBlock> blocks = <MdBlock>[];
  for (final String line in raw.split('\n')) {
    final bool bullet = _bulletPrefix.hasMatch(line);
    final String body = bullet ? line.replaceFirst(_bulletPrefix, '') : line;
    blocks.add(MdBlock(_parseInline(body), bullet: bullet));
  }
  return blocks;
}

/// 解析一行里的 `**粗体**` 与 `` `行内码` ``。
///
/// 两种记号**从左到右一起扫**，谁先出现谁先生效——这样
/// `` `a **b** c` `` 里的星号会被当成代码内容原样保留（这是对的：
/// 用户按了行内码，就不该再解释里面的记号）。
List<MdSpan> _parseInline(String line) {
  final List<MdSpan> out = <MdSpan>[];
  final StringBuffer plain = StringBuffer();
  int i = 0;

  void flushPlain() {
    if (plain.isEmpty) return;
    out.add(MdSpan(plain.toString()));
    plain.clear();
  }

  while (i < line.length) {
    final bool isBoldOpen = line.startsWith('**', i);
    final bool isCodeOpen = line[i] == '`';

    if (isCodeOpen) {
      final int close = line.indexOf('`', i + 1);
      // **要求内容非空**（`close > i + 1`）。
      //
      // 这一条比它看起来重要：放宽成「空也算」之后，模型输出里的
      // 代码围栏 ``` 会被吃掉两个反引号（`` 先被当成空行内码消费），
      // 只剩一个 `` ` `` ——那是**静默改写了模型说的话**，正是本文件
      // 最想避免的失败方式。空行内码本来也没有意义，不认它最安全。
      if (close > i + 1) {
        flushPlain();
        out.add(MdSpan(line.substring(i + 1, close), code: true));
        i = close + 1;
        continue;
      }
      // 没闭合 / 空的：原样留着（见文件头注「宁可少认，不可吞字」）。
      plain.write(line[i]);
      i++;
      continue;
    }

    if (isBoldOpen) {
      final int close = line.indexOf('**', i + 2);
      // 同理：空强调 `****` 也**不认**（留着比吞掉好）。
      if (close > i + 2) {
        flushPlain();
        out.add(MdSpan(line.substring(i + 2, close), bold: true));
        i = close + 2;
        continue;
      }
      // 落单的 `**`：原样留着。
      plain.write('**');
      i += 2;
      continue;
    }

    plain.write(line[i]);
    i++;
  }
  flushPlain();
  if (out.isEmpty) out.add(const MdSpan(''));
  return out;
}

/// 一行里有没有**任何**记号需要解释。
///
/// # 实现方式：解析一遍，看结果和原文一不一样
///
/// 这比「用正则猜有没有成对记号」慢一点，但**不会和解析器说两套话**——
/// 后者是这类捷径的经典失败方式：判据说「没有记号」而解析器其实会改字，
/// 于是消息走了纯文本路径、记号原样显示给用户，而且**不报错**。
/// 气泡里的文本都很短，付这点代价换来「判据与行为天然一致」是划算的。
///
/// 调用方用它走捷径：纯文本消息不必进 `Text.rich`（`SelectableText.rich`
/// 与 `SelectableText` 在选择/复制行为上有细微差别，纯文本就让它保持纯文本）。
bool hasChatMarkdown(String raw) => _plainText(parseChatMarkdown(raw)) != raw;

/// 把块列表还原成「不带任何记号的文本」。
String _plainText(List<MdBlock> blocks) => blocks
    .map(
      (MdBlock b) =>
          '${b.bullet ? '· ' : ''}${b.spans.map((MdSpan s) => s.text).join()}',
    )
    .join('\n');
