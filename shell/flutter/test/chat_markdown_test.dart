/// 聊天气泡的轻量 Markdown（2026-09-11，前端加强计划 P2-3）。
///
/// # 为什么需要它
///
/// 模型输出天然带 Markdown，而这些字符过去是**原样进 `SelectableText`**
/// 的：用户在气泡里读到的是 `**重点**`、`- 第一条`、`` `code` `` 的字面
/// 符号。设置文案那边早就修过了（`emphasized_text.dart`），但那条修复
/// 只覆盖**硬编码文案**，运行时的模型输出不受它保护。
///
/// # 本文件钉的两件事
///
/// 1. **三类语法真的被解释了**（粗体 / 行内码 / 列表）；
/// 2. **不认识的一律原样保留**——这是比第 1 条更重要的那条。解析器吞掉
///    半个 `**` 或半个反引号，用户看到的就是「模型说的话少了一截」，
///    而这种 bug 不会报错、只会让人困惑。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_markdown.dart';

/// 把解析结果压成便于断言的一行（`[b]`=粗体、`[c]`=行内码、`· `=列表项）。
String render(List<MdBlock> blocks) => blocks
    .map(
      (MdBlock b) =>
          '${b.bullet ? '· ' : ''}'
          '${b.spans.map((MdSpan s) => s.code ? '[c]${s.text}' : s.bold ? '[b]${s.text}' : s.text).join()}',
    )
    .join('\n');

String parse(String raw) => render(parseChatMarkdown(raw));

void main() {
  group('P2-3：三类语法真的被解释了', () {
    test('`**粗体**` 不再是两个星号', () {
      expect(parse('这是**重点**。'), '这是[b]重点。');
    });

    test('`` `行内码` `` 走 code 分支', () {
      expect(parse('跑 `flutter test` 即可'), '跑 [c]flutter test 即可');
    });

    test('`- ` / `* ` / `• ` 开头的行独立成行，记号被吃掉', () {
      expect(parse('- 第一条\n- 第二条'), '· 第一条\n· 第二条');
      expect(parse('* 甲\n• 乙'), '· 甲\n· 乙');
    });

    test('列表项里还能有强调（不因为进了列表就不解释）', () {
      expect(parse('- **重要**：先备份'), '· [b]重要：先备份');
    });

    test('多行文本按行成块（不再是挤成一坨）', () {
      final List<MdBlock> blocks = parseChatMarkdown('第一行\n第二行');
      expect(blocks.length, 2);
      expect(blocks.every((MdBlock b) => !b.bullet), isTrue);
    });

    test('行内码里的记号**不再二次解释**（按了反引号就该原样）', () {
      expect(parse('`a **b** c`'), '[c]a **b** c');
    });
  });

  group('P2-3：不认识的一律原样保留（比上面那组更重要）', () {
    test('落单的 `**` 原样留着（写错了要看得见，不能被吞）', () {
      expect(parse('半截 **标记'), '半截 **标记');
      expect(parse('末尾两个星号 **'), '末尾两个星号 **');
    });

    test('没闭合的反引号原样留着', () {
      expect(parse('半截 `code'), '半截 `code');
    });

    test('空强调 `****` **不认**（留着，而不是吞掉四个星号）', () {
      // 与 `emphasized_text.dart` 的取舍**不同**，是有意的：
      // 那边渲染的是我们自己写的硬编码文案（输入可控），这边是**模型输出**，
      // 主导风险是「静默改写模型说的话」。同一条取舍也保住了代码围栏
      // ``` ``` ```（放宽成「空也算行内码」会吃掉其中两个反引号）。
      expect(parse('前后****中间'), '前后****中间');
    });

    test('标题 / 代码块 / 链接**不解释**（边界：不做「真的 Markdown」）', () {
      // 这三条是**刻意**不支持的，写成测试是为了让「为什么这里还是星号」
      // 有据可查，而不是看起来像漏了。
      expect(parse('# 标题'), '# 标题');
      expect(parse('```\ncode\n```'), '```\ncode\n```');
      expect(parse('[文字](https://x)'), '[文字](https://x)');
    });

    test('纯文本原样（一个字符都不动）', () {
      const String plain = '今天天气不错，我们去散步吧。\n顺便买点东西。';
      expect(parse(plain), plain);
    });

    test('空串不炸', () {
      expect(parseChatMarkdown(''), isEmpty);
    });
  });

  group('P2-3：hasChatMarkdown —— 纯文本消息不必进 Text.rich', () {
    test('有**成对**记号才为真', () {
      expect(hasChatMarkdown('这是**重点**'), isTrue);
      expect(hasChatMarkdown('跑 `x` 一下'), isTrue);
      expect(hasChatMarkdown('- 列表项'), isTrue);
    });

    test('落单的记号不算（走纯文本路径，连解析都不做）', () {
      expect(hasChatMarkdown('半截 **标记'), isFalse);
      expect(hasChatMarkdown('半截 `code'), isFalse);
      expect(hasChatMarkdown('普通的句子'), isFalse);
    });

    test('空强调 `****` 不算（解析出来和原文一样）', () {
      expect(hasChatMarkdown('前后****中间'), isFalse);
    });
  });

  group('P2-3：等价性（结构相等是 == 的语义）', () {
    test('两次解析同样输入得到相等的结构', () {
      expect(parseChatMarkdown('**a** 与 `b`'), parseChatMarkdown('**a** 与 `b`'));
    });
  });
}
