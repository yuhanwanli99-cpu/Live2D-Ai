/// **跨语言默认值一致性**：唤醒词缺省两边必须逐字相同。
///
/// 前端按钮文案说「小可爱 ……」，服务端闸门也必须认「小可爱」——
/// 两边默认值漂移会变成「按钮让你说小可爱、服务端回 400 wake_phrase_required」，
/// 而且**断网 / 无日志时完全看不出来**。
///
/// 这里直接读 Rust 常量源码做交叉核对（字体子集门禁同款「生成物 + 源码对账」思路）。
/// 找不到仓库结构（例如包被单独拷出去跑）→ 明确 skip，不伪造通过。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/voice/voice_listen_controller.dart';

/// Rust 侧常量所在文件（相对包根 = shell/flutter）。
const String _rustGatePath =
    '../../crates/live2d-ai-mod-voice-input/src/gate.rs';

void main() {
  test('Dart kDefaultWakePhrase 与 Rust DEFAULT_WAKE_PHRASE 逐字一致', () {
    final File file = File(_rustGatePath);
    if (!file.existsSync()) {
      // 包被单独拷走 / 仓布局变了：如实跳过。
      // ignore: avoid_print
      print('SKIP: 找不到 $_rustGatePath（非仓库内运行？）');
      return;
    }
    final String source = file.readAsStringSync();
    final RegExpMatch? m = RegExp(
      r'pub const DEFAULT_WAKE_PHRASE:\s*&str\s*=\s*"([^"]+)"',
    ).firstMatch(source);
    expect(m, isNotNull, reason: 'Rust 侧必须定义 DEFAULT_WAKE_PHRASE');
    expect(
      m!.group(1),
      kDefaultWakePhrase,
      reason: '两端缺省唤醒词漂移：Dart=$kDefaultWakePhrase Rust=${m.group(1)}',
    );
  });
}
