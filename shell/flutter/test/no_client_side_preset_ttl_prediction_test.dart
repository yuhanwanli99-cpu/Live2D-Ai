/// 阶段4d / D4：源码扫描 —— **前端不得猜 preset 剩余时长**（V8）。
///
/// # 断言方式（D14 口径，必须写清）
///
/// **不是**字面 grep 清单，而是 `Directory` 扫描 `lib/**/*.dart` 后按**语义**
/// 判定：对每个文件先算两个布尔量，**只有两者同时为真才算违规**——
///
/// - `hasApplyPreset`：文件里出现 `applyPreset`（声明或调用）；
/// - `hasLocalTtlPrediction`：文件里同时有①**定时器**（`Timer(` /
///   `Timer.periodic(`）与②**ttl/时刻推演**（`.ttl` / `startedAt` /
///   `remaining(` 相减），**或**直接出现 `remaining(...)` 形式的剩余时长换算。
///
/// 这正是判据原文：「**同一文件同时含「本地计时器推演 preset 剩余时长」与
/// applyPreset 即红**」。它守的是 v0 那个 `_presetTimer = Timer.periodic(200ms)`
/// 在 `applyPreset` 之后不断把「还剩多少毫秒」算一遍、到点把状态清成 null 的
/// 实现（前端镜像 + 本地状态机）。阶段4d 已删；本扫描防它混回来。
///
/// 为什么不用「全仓搜 Timer 计数」：显示用的 DTO 换算（调试面板的倒计时数字）
/// 与「本地计时器驱动状态」是两件事——红线只禁后者（V8 的原文是「不做每帧
/// 状态流、不做前端镜像」）。因此判据钉在「**同一文件**里既有驱动者
/// （applyPreset）又有本地推算」。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'support/source_scan.dart';

/// 剥掉注释与字符串（否则本文件自己的注释、以及日志文案都会把自己判红）。
///
/// 词法器在 `support/source_scan.dart`（W3-D3：8 份副本合并为 1 份——
/// 其中 `design_tokens_test.dart` 那份已经漂移成占位符 `S`）。
/// 违规判定：同一文件同时含「本地计时器推演 preset 剩余时长」与 `applyPreset`。
bool hasApplyPreset(String src) => RegExp(r'\bapplyPreset\b').hasMatch(src);

bool hasLocalTtlPrediction(String src) {
  final bool timer = RegExp(r'Timer(\.periodic)?\s*\(').hasMatch(src);
  final bool ttlMath =
      RegExp(r'\.ttl\b|startedAt|remaining\s*\(').hasMatch(src);
  final bool remaining = RegExp(r'remaining\s*\(').hasMatch(src);
  return remaining || (timer && ttlMath);
}

void main() {
  test('no_client_side_preset_ttl_prediction', () {
    final List<File> files = Directory('lib')
        .listSync(recursive: true)
        .whereType<File>()
        .where((File f) => f.path.endsWith('.dart'))
        .toList();
    expect(files.length, greaterThan(50), reason: '扫描根路径不对，扫不到 lib/**');

    final List<String> violations = <String>[];
    int checked = 0;
    for (final File f in files) {
      final String src = stripCommentsAndStrings(f.readAsStringSync());
      if (!hasApplyPreset(src)) continue;
      checked += 1;
      if (hasLocalTtlPrediction(src)) violations.add(f.path);
    }
    expect(checked, greaterThan(0), reason: '必须真的扫到 applyPreset 所在文件');
    expect(
      violations,
      isEmpty,
      reason: '同一文件同时含「本地计时器推演 preset 剩余时长」与 applyPreset = '
          '前端在猜时长（V8 禁止建立前端镜像 / 本地状态机）；违规文件：'
          '$violations',
    );
  });
}
