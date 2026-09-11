/// **轻量强调文本**：把 `**这样**` 渲染成粗体，而不是把两个星号画给用户看。
///
/// # 为什么需要它（2026-09-11，在真浏览器里看出来的）
///
/// 这个项目的说明文案一直是按 Markdown 习惯写的（`**只影响本机显示**`），
/// 因为最早那套原生 JS 前端是把它塞进 HTML 里渲染的。换成 Flutter 之后
/// 那些字符串进了 `Text`——**`Text` 不认 Markdown**，于是界面上
/// 到处是字面的两个星号。纯色背景上那两颗星很显眼，是最典型的
/// 「看起来像是没做完」。
///
/// 两种修法：把 15 处 `**` 从文案里删掉（丢强调），或者**真的渲染它**。
/// 选了后者——这些句子里被强调的正是**用户最容易误解的那半句**
/// （「只影响本机显示」「不会播放音频」「需重启服务端才生效」），
/// 去掉强调等于把信息量降一档。
///
/// # 解析规则（刻意简单）
///
/// - 成对的 `**…**` → 粗体；
/// - **落单的 `**` 原样保留**（不猜、不吞、不抛）——文案写错了要看得见，
///   而不是被静默吃掉一半；
/// - 空强调（`****`）不产生空 span。
///
/// 解析放在 [emphasisSpans]（纯函数、可在 VM 上单测），
/// [EmphasizedText] 只负责把它画出来。
library;

import 'package:flutter/material.dart';

/// 把 `raw` 按 `**` 拆成「普通 / 粗体」交替的 span 列表。
///
/// 不传 `style` 时只标 `fontWeight`，其余继承外层 `DefaultTextStyle`
/// （字号/颜色仍由调用方通过 `Text.rich` 的 `style` 决定）。
List<TextSpan> emphasisSpans(String raw, {TextStyle? base, TextStyle? bold}) {
  final List<TextSpan> out = <TextSpan>[];
  int index = 0;
  while (index < raw.length) {
    final int open = raw.indexOf('**', index);
    if (open < 0) {
      out.add(TextSpan(text: raw.substring(index), style: base));
      break;
    }
    final int close = raw.indexOf('**', open + 2);
    if (close < 0) {
      // 落单的 `**`：原样留着（写错了要看得见）。
      out.add(TextSpan(text: raw.substring(index), style: base));
      break;
    }
    if (open > index) {
      out.add(TextSpan(text: raw.substring(index, open), style: base));
    }
    final String inner = raw.substring(open + 2, close);
    if (inner.isNotEmpty) {
      out.add(
        TextSpan(
          text: inner,
          style: (base ?? const TextStyle()).merge(
            bold ?? const TextStyle(fontWeight: FontWeight.w600),
          ),
        ),
      );
    }
    index = close + 2;
  }
  if (out.isEmpty) out.add(TextSpan(text: raw, style: base));
  return out;
}

/// 渲染带 `**强调**` 的说明文本。
///
/// 用法与 `Text` 基本一致；`style` 是**基础**样式，粗体在其上叠加
/// （不会把调用方的字号/颜色覆盖掉）。
class EmphasizedText extends StatelessWidget {
  const EmphasizedText(
    this.text, {
    this.style,
    this.textAlign,
    this.maxLines,
    this.overflow,
    super.key,
  });

  final String text;
  final TextStyle? style;
  final TextAlign? textAlign;
  final int? maxLines;
  final TextOverflow? overflow;

  @override
  Widget build(BuildContext context) => Text.rich(
    TextSpan(children: emphasisSpans(text, base: style)),
    style: style,
    textAlign: textAlign,
    maxLines: maxLines,
    overflow: overflow,
  );
}
