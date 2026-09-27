/// 轮播间隔的存储 clamp 与背景索引口径（A3c：审计 C 组转正 / 台账）。
///
/// # 为什么单独一份，而不是塞进 `display_prefs_test.dart`
///
/// 那份已经 882 行（> 800 的测试文件上限，是既有状态）；A3c 的 4 条不该
/// 把它推到 978。这里只装 C 组：`slideInterval` 的 clamp 与背景索引语义。
///
/// # 仍未裁决的（**不在这里写断言**）
///
/// - **C2**：1–4 秒的存量值原样保留（toggle 判 `> 0` = 开着），而滑杆下限是 5
///   ⇒「开着但选不到」这一档；`ShellSlideshow.start(2)` 会真的按 2 秒跑，
///   而设计说「再小会让定时器疯狂重入」。夹到 5 还是保持原样**未裁决**。
/// - **C3**：`effectiveBackground` / `currentItemIsImage` 永远看背景库**第 0 项**，
///   而壳运行时按 `main.dart:_backgroundIndex` 轮播。「设置页该不该跟随当前
///   轮播下标」是产品语义决定，两种都说得通 ⇒ 不写断言。
/// - **C4**：`BackgroundImage.isRenderable` 只判「字节非空」，一段不是 dataURL
///   的串也返回 true ⇒ `hasBackground` 为 true 而渲染层回落到纯色底。
///   渲染层回落已被 `shell_backdrop_test` 覆盖；未定的是 `isRenderable`
///   该不该校验 dataURL 形状（要正则 / 解码，且与 `background_item.dart`
///   现在的文档口径一起改）。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/display_prefs.dart';

void main() {
  group('轮播间隔：存储 clamp 与滑杆共用同一上界（P1-6 回归）', () {
    test('存储上界 = 滑杆上界（300）：不再有第二套上界 3600', () {
      // 改动前 `DisplayPrefs` 另有一个 `maxSlideInterval = 3600`：存储允许 3600
      // 而滑杆只到 300，一份存量偏好会让**滑块顶在最右、读数却写 3600**。
      // A3b 删掉了那个常量，两端统一读 `maxSlideIntervalSeconds`。
      expect(DisplayPrefs.maxSlideIntervalSeconds, 300);
      // 3600 现在落在区间外。这里钉住的是**如实**的现状：`_clampInt` 的约定是
      // 「越界回落默认值」，所以读到 0（= 关掉轮播），不是 300，更不是 3600。
      // TODO(A3c)：`slideInterval` 是**数值区间**（像 scale / volume），而它们走
      // `.clamp` 夹到端点；`_clampInt` 是为 `scrim` / `imageFit` 那种「端点是
      // 枚举语义」准备的。3600 → 0（把老用户的轮播静默关掉）vs → 300
      // （夹到上界）**未裁决**，所以这条断言随时可以随裁决改。
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'slideInterval': 3600,
      });
      expect(p.slideInterval, DisplayPrefs.defaultSlideInterval);
      expect(DisplayPrefs.defaultSlideInterval, 0);
      expect(p.slideInterval, isNot(3600), reason: '绝不许再原样读回 3600');
    });

    test('区间内（含两端 0 与 300）的值原样保留', () {
      for (final int v in <int>[0, 1, 5, 30, 299, 300]) {
        expect(
          DisplayPrefs.fromJson(<String, Object?>{
            'slideInterval': v,
          }).slideInterval,
          v,
          reason: '$v 在 [0, 300] 内，不该被动过',
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
        final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
          'slideInterval': v,
        });
        expect(
          p.slideInterval,
          inInclusiveRange(0, DisplayPrefs.maxSlideIntervalSeconds),
          reason: '存 $v 时越界了',
        );
      }
    });

    test('0 仍然是「关掉轮播」（不能被夹到滑杆下限 5）', () {
      expect(
        DisplayPrefs.fromJson(<String, Object?>{
          'slideInterval': 0,
        }).slideInterval,
        0,
      );
      expect(DisplayPrefs.minSlideIntervalSeconds, 5);
    });
  });
}
