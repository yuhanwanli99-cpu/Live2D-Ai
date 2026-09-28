/// 轮播间隔的存储 clamp 与背景索引口径（A3c：审计 C 组转正 / 台账）。
///
/// # 为什么单独一份，而不是塞进 `display_prefs_test.dart`
///
/// 那份已经 882 行（> 800 的测试文件上限，是既有状态）；A3c 的 4 条不该
/// 把它推到 978。这里只装 C 组：`slideInterval` 的 clamp 与背景索引语义。
///
/// # 裁决与落地（2026-09-28 · Stage B · B-a）
///
/// - **C1 / C2（越界回落 0、存量 1–4 秒选不到）→ DEC-1 端点夹持**：
///   `0` 仍＝关，其余夹进 `[5, 300]`（1–4 → 5，301–3600 → 300）。
///   完整对照表与红→绿在 `display_prefs_background_fit_test.dart`；
///   这里保留「能与滑杆上界对齐」这一条。
/// - **C4（坏 dataURL 仍 `isRenderable`）→ DEC-5 收紧**：见
///   `display_prefs_background_fit_test.dart` 的 DEC-5 组。
/// - **C3（`effectiveBackground` / `currentItemIsImage` 永远看背景库第 0 项）
///   仍未落地**：归 DEC-6 / R6-b（把运行时轮播下标传进外观区，
///   **不持久化**）——所以这里不写断言。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/display_prefs.dart';

/// 存 [v] 秒 → 读回来的值（唯一入口就是 `fromJson`）。
int _roundTrip(int v) =>
    DisplayPrefs.fromJson(<String, Object?>{'slideInterval': v}).slideInterval;

void main() {
  group('轮播间隔：存储 clamp 与滑杆共用同一上界（P1-6 回归）', () {
    test('存储上界 = 滑杆上界（300）：不再有第二套上界 3600', () {
      // 改动前 `DisplayPrefs` 另有一个 `maxSlideInterval = 3600`：存储允许 3600
      // 而滑杆只到 300，一份存量偏好会让**滑块顶在最右、读数却写 3600**。
      // A3b 删掉了那个常量；DEC-1 又把「越界回落 0」改成端点夹持。
      expect(DisplayPrefs.maxSlideIntervalSeconds, 300);
      // DEC-1：3600 夹到**上界**（继续开着轮播），不是回落 0（静默关掉）。
      expect(_roundTrip(3600), 300);
    });

    test('区间内（0 与 5–300）原样保留；1–4 夹到滑杆下限 5（DEC-1）', () {
      for (final int v in <int>[0, 5, 30, 299, 300]) {
        expect(_roundTrip(v), v, reason: '$v 在 [0, 300] 的合法档里，不该被动过');
      }
      // C2：存量 1–4 秒以前「开着但选不到」，现在夹到下限（仍然开着）。
      for (final int v in <int>[1, 2, 3, 4]) {
        expect(
          _roundTrip(v),
          DisplayPrefs.minSlideIntervalSeconds,
          reason: 'DEC-1：<5 夹到 5（不是关掉轮播）',
        );
      }
    });

    test('任意存储值读回来都落在 [0, maxSlideIntervalSeconds]', () {
      for (final int v in <int>[
        -100,
        -1,
        0,
        1,
        2,
        4,
        5,
        30,
        299,
        300,
        301,
        3600,
        100000,
      ]) {
        expect(
          _roundTrip(v),
          inInclusiveRange(0, DisplayPrefs.maxSlideIntervalSeconds),
          reason: '存 $v 时越界了',
        );
      }
    });

    test('0 仍然是「关掉轮播」（不能被夹到滑杆下限 5）', () {
      expect(_roundTrip(0), 0);
      expect(DisplayPrefs.minSlideIntervalSeconds, 5);
    });
  });
}