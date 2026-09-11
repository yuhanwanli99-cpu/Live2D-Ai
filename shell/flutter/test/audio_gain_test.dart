import 'dart:math' as math;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/audio/gain.dart';

void main() {
  group('gainForVolume 端点', () {
    test('满音量 → 增益 1.0（不改动信号）', () {
      expect(gainForVolume(1.0), 1.0);
    });

    test('零音量 → 增益 0.0（真静音，不是「很小」）', () {
      expect(gainForVolume(0.0), 0.0);
    });
  });

  group('gainForVolume 是感知压缩（不是恒等映射）', () {
    test('内部点严格小于滑杆位置——这正是「感知压缩」的定义', () {
      // 如果谁把曲线换成恒等映射（return v），这条立刻变红。它守的是**手感**：
      // 线性振幅下滑杆前半段几乎听不出变化，后半段突然变响。
      for (final v in <double>[0.1, 0.25, 0.5, 0.75, 0.9, 0.99]) {
        expect(
          gainForVolume(v),
          lessThan(v),
          reason: 'v=$v 处的增益必须小于 v（感知压缩）',
        );
      }
    });

    test('半程 ≈ -12 dB（v² 的标定点）', () {
      final gain = gainForVolume(0.5);
      expect(gain, closeTo(0.25, 1e-12));
      final db = 20 * math.log(gain) / math.ln10;
      expect(db, closeTo(-12.04, 0.01));
    });

    test('单调不减（音量调大不可能更小声）', () {
      var previous = gainForVolume(0.0);
      for (var i = 1; i <= 100; i++) {
        final gain = gainForVolume(i / 100);
        expect(
          gain,
          greaterThanOrEqualTo(previous),
          reason: 'i=$i 处增益回落了',
        );
        previous = gain;
      }
    });
  });

  group('gainForVolume 输出恒在 [0,1] 且有限', () {
    test('越界与非有限输入都被夹住，绝不把 NaN 灌进 gain', () {
      // gain 里出现 NaN 会让整条音频图静默或爆音，所以这里逐个钉住。
      for (final raw in <double>[
        -1.0,
        -0.001,
        1.0001,
        99.0,
        double.nan,
        double.infinity,
        double.negativeInfinity,
      ]) {
        final gain = gainForVolume(raw);
        expect(gain.isFinite, isTrue, reason: 'gainForVolume($raw) 非有限');
        expect(
          gain,
          inInclusiveRange(0.0, 1.0),
          reason: 'gainForVolume($raw) 越界',
        );
      }
    });

    test('非有限输入回落默认（满音量），不把 NaN 灌进 gain', () {
      // 兜底而非主路径：JSON 本身无法表示 NaN/Infinity，localStorage 里读不出来，
      // 所以这里防的是**程序内部误传**。约定与 display_prefs 一致：非有限 → 默认。
      expect(gainForVolume(double.nan), 1.0);
      expect(gainForVolume(double.infinity), 1.0);
      expect(gainForVolume(double.negativeInfinity), 1.0);
    });
  });
}
