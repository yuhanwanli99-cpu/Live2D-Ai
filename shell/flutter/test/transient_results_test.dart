/// 内联结果不得跨分区存活（2026-09-11，前端加强计划 P2-2）。
///
/// # 被修掉的是什么
///
/// 四个「一次性结果」挂在 `MainApp` 的 State 上：
/// `_llmTest` / `_ttsTest` / `_adminMessage` / `_stageImageMessage`
/// （2026-09-13 M5.1：`_importMessage` 随角色卡导入迁出主链而删除）。
/// 它们过去只在**下一次同类操作**时才被覆盖，
/// 切分区不重置。于是：用户测出「失败：401」→ 修好配置 → 切走 → 切回来，
/// **那句失效的结论还在**，而它描述的已经是上一套配置了。
///
/// （`_adminMessage` 原名 `_actionMessage`，2026-09-11 随动作子系统移除时
/// 改名：它服务的其实只是「模型激活 / Mod 启停」的管理结果，与动作无关，
/// 旧名字会让人误以为它属于那条已删除的链路。清理行为完全不变。）
///
/// 还有一层更细的：连点「测试连接」之后立刻切走，响应几秒后才回来——
/// 那条结果会落在**用户已经离开的分区**上，看起来像是刚测的。
///
/// # 为什么这里用源码扫描而不是 widget 测试
///
/// `MainApp` 的 State 是私有的，而它依赖 `http` 客户端做真实请求；
/// 为了这一条断言去搭一套 HTTP 打桩，成本远大于收益，还会把测试绑死在
/// 组合根的内部结构上。这里钉的是**结构性事实**（这一层不该有第二条
/// 改分区的路），它恰好是这个 bug 当初能出现的根本原因：
/// 两个地方各自 `setState(() => _section = …)`。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 剥掉注释与字符串（否则本文件自己的注释就会把断言判红）。
String stripCommentsAndStrings(String src) {
  final StringBuffer out = StringBuffer();
  int i = 0;
  while (i < src.length) {
    final String c = src[i];
    if (c == "'" || c == '"') {
      final bool triple =
          i + 2 < src.length && src[i + 1] == c && src[i + 2] == c;
      final String quote = triple ? c + c + c : c;
      i += quote.length;
      while (i < src.length) {
        if (src[i] == r'\') {
          i += 2;
          continue;
        }
        if (src.startsWith(quote, i)) {
          i += quote.length;
          break;
        }
        i++;
      }
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

String mainSource() =>
    stripCommentsAndStrings(File('lib/main.dart').readAsStringSync());

void main() {
  group('P2-2：切分区只有一条路', () {
    test('`_section =` 在 main.dart 里只出现在 `_gotoSection` 一处', () {
      // 只数**赋值**，不数字段声明（`SettingsSection _section = …`）。
      final RegExp assignment = RegExp(r'(?<!SettingsSection )_section\s*=');
      final List<String> lines = mainSource().split('\n');
      final List<String> writers = <String>[
        for (int i = 0; i < lines.length; i++)
          if (assignment.hasMatch(lines[i]))
            '${i + 1}: ${lines[i].trim()}',
      ];
      expect(
        writers.length,
        1,
        reason:
            '有第二处直接改分区 —— 那条路不会清一次性结果，'
            '用户切回来会看到上一次的过期结论。\n当前命中：\n${writers.join('\n')}',
      );
      expect(writers.single, contains('_section = next'));
    });

    test('`_gotoSection` 真的清了结果（不是只改分区）', () {
      final String src = mainSource();
      final int goto = src.indexOf('void _gotoSection(');
      expect(goto, greaterThan(0), reason: '找不到 _gotoSection');
      final String body = src.substring(goto, goto + 700);
      expect(
        body.contains('_clearTransientResults()'),
        isTrue,
        reason: '_gotoSection 没有清一次性结果',
      );
      expect(body.contains('if (next == _section) return;'), isTrue,
          reason: '同一个分区之间来回点不该白清一次结果');
    });
  });

  group('P2-2：四个一次性结果全在清理范围内', () {
    test('清的就是这四个（不是只清了一两个）', () {
      final String src = mainSource();
      final int start = src.indexOf('void _clearTransientResults()');
      expect(start, greaterThan(0));
      final String body = src.substring(start, start + 700);
      for (final String field in <String>[
        '_llmTest',
        '_ttsTest',
        '_adminMessage',
        '_stageImageMessage',
      ]) {
        expect(
          body.contains('$field = null'),
          isTrue,
          reason: '$field 没有被清理 —— 它会跨分区存活',
        );
      }
      // 配套的布尔位也要复位，否则「测试中…」会永远转下去。
      for (final String flag in <String>[
        '_llmTesting',
        '_ttsTesting',
        '_stageImageFailed',
      ]) {
        expect(body.contains('$flag = false'), isTrue, reason: '$flag 没有复位');
      }
    });

    test('**在途的**异步结果也会作废（`_resultEpoch` 对账）', () {
      final String src = mainSource();
      // 两个有真实网络往返的自检都要对代际。
      for (final String fn in <String>['_testLlm', '_testTts']) {
        final int at = src.indexOf('Future<void> $fn()');
        expect(at, greaterThan(0), reason: '找不到 $fn');
        final String body = src.substring(at, at + 1400);
        expect(
          body.contains('final int epoch = _resultEpoch;'),
          isTrue,
          reason: '$fn 没有在 await 前记下代际',
        );
        expect(
          body.contains('epoch != _resultEpoch'),
          isTrue,
          reason: '$fn 没有在落地结果前对代际 —— 切走之后回来的响应会显示在'
              '用户已经离开的分区上',
        );
      }
      // 清结果时也要抬代际，否则「代际」这个概念不成立。
      final int clear = src.indexOf('void _clearTransientResults()');
      expect(
        src.substring(clear, clear + 700).contains('_resultEpoch++'),
        isTrue,
      );
    });
  });

  group('P2-2：结果行渐入渐出（不是「啪」地多出一行）', () {
    test('字段行的结果槽包了 `SoftSwap`', () {
      final String src = stripCommentsAndStrings(
        File('lib/ui/field_row.dart').readAsStringSync(),
      );
      expect(
        src.contains('SoftSwap('),
        isTrue,
        reason: '结果行直接条件插入 —— 出现时会把它下面的字段整体推下去，'
            '用户正在看的字段会跳',
      );
    });
  });
}
