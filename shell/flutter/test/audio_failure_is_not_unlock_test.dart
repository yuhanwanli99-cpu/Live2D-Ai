/// S-005（2026-10-16）：**一句音频坏了**不许被讲成「你还没启用声音」。
///
/// `AudioPlayer` 是 web-only（`dart:js_interop`），`flutter test` 的 VM 里 import
/// 不了它——所以按本仓对它的既有口径用**源码守卫**钉住判据（先例
/// `stage_clock_test.dart` 同样读 `lib/audio/audio_player.dart`）。三条：
///
/// 1. DOM `error` 处理器（解码 / 加载失败）**不得**改解锁状态：那会把「这一句
///    坏了」讲成「你还没启用声音」，而那个按钮修不回来；
/// 2. 判据必须是**媒体错误**（`element.error`），不是「play() 的 promise 被拒」
///    ——后者把 autoplay 闸与坏音频混为一谈；
/// 3. 坏的那一句仍然**被跳过**（`_finish`），队列不停。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// 取 `header` 之后那对花括号里的整段（够用的朴素配平）。
String _block(String source, String header) {
  final int start = source.indexOf(header);
  expect(start, greaterThanOrEqualTo(0), reason: '找不到锚点：$header');
  final int open = source.indexOf('{', start);
  expect(open, greaterThan(start), reason: '锚点之后没有函数体：$header');
  int depth = 0;
  for (int i = open; i < source.length; i++) {
    final String ch = source[i];
    if (ch == '{') depth++;
    if (ch == '}') {
      depth--;
      if (depth == 0) return source.substring(open, i + 1);
    }
  }
  fail('括号不配平：$header');
}

void main() {
  final String src = File('lib/audio/audio_player.dart').readAsStringSync();

  test('DOM error（坏音频）不得改解锁状态', () {
    final String body = _block(src, 'void _onError(_Playback item)');
    expect(
      body.contains('_setUnlocked'),
      isFalse,
      reason: '一句解码失败不是「你还没启用声音」：$body',
    );
    expect(
      body.contains('_finish(item)'),
      isTrue,
      reason: '坏了的那一句仍要跳过（队列不停）：$body',
    );
  });

  test('play() 被拒要按「有没有媒体错误」分流', () {
    expect(
      src.contains('element.error != null'),
      isTrue,
      reason: '判据必须是媒体错误，而不是「promise 被拒」',
    );
    // 两条分支都要在：媒体坏了 → 跳过；否则 → 亮「启用声音」。
    final String play = _block(src, 'void _play(_Playback item)');
    expect(play.contains('_finish(item)'), isTrue, reason: play);
    expect(play.contains('_setUnlocked(false)'), isTrue, reason: play);
  });
}
