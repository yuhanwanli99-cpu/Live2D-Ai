import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/display_prefs.dart';

void main() {
  group('DisplayPrefs 默认值', () {
    test('默认即实测标定值', () {
      const prefs = DisplayPrefs();
      expect(prefs.scale, 1.0);
      // 1.0 = 真实语音 p90 约开到 0.58 的标定值（见 mouth.rs 标定表）。
      expect(prefs.mouthSensitivity, 1.0);
      expect(prefs.lipSync, isTrue);
      expect(prefs.idleEnabled, isTrue);
    });
  });

  group('反序列化永不抛异常', () {
    test('null / 空 map 回落默认', () {
      expect(DisplayPrefs.fromJson(null), const DisplayPrefs());
      expect(DisplayPrefs.fromJson(<String, Object?>{}), const DisplayPrefs());
    });

    test('类型不符回落默认', () {
      final prefs = DisplayPrefs.fromJson(<String, Object?>{
        'scale': 'big',
        'mouthSensitivity': null,
        'lipSync': 1,
        'idleEnabled': 'yes',
      });
      expect(prefs, const DisplayPrefs());
    });

    test('非有限数回落默认（不是夹到边界）', () {
      final prefs = DisplayPrefs.fromJson(<String, Object?>{
        'scale': double.nan,
        'mouthSensitivity': double.infinity,
      });
      expect(prefs.scale, DisplayPrefs.defaultScale);
      expect(prefs.mouthSensitivity, DisplayPrefs.defaultMouthSensitivity);
    });

    test('越界值夹到区间内', () {
      final low = DisplayPrefs.fromJson(<String, Object?>{
        'scale': -5.0,
        'mouthSensitivity': 0.0,
      });
      expect(low.scale, DisplayPrefs.minScale);
      expect(low.mouthSensitivity, DisplayPrefs.minMouthSensitivity);

      final high = DisplayPrefs.fromJson(<String, Object?>{
        'scale': 99.0,
        'mouthSensitivity': 99.0,
      });
      expect(high.scale, DisplayPrefs.maxScale);
      expect(high.mouthSensitivity, DisplayPrefs.maxMouthSensitivity);
    });

    test('合法值原样保留', () {
      final prefs = DisplayPrefs.fromJson(<String, Object?>{
        'scale': 1.4,
        'mouthSensitivity': 1.8,
        'lipSync': false,
        'idleEnabled': false,
      });
      expect(prefs.scale, 1.4);
      expect(prefs.mouthSensitivity, 1.8);
      expect(prefs.lipSync, isFalse);
      expect(prefs.idleEnabled, isFalse);
    });

    test('整数形式的 JSON 数值也能读（localStorage 常见）', () {
      final prefs = DisplayPrefs.fromJson(<String, Object?>{
        'scale': 1,
        'mouthSensitivity': 2,
      });
      expect(prefs.scale, 1.0);
      expect(prefs.mouthSensitivity, 2.0);
    });
  });

  group('往返与 copyWith', () {
    test('toJson → fromJson 幂等', () {
      const original = DisplayPrefs(
        scale: 1.25,
        mouthSensitivity: 1.7,
        lipSync: false,
        idleEnabled: true,
      );
      expect(DisplayPrefs.fromJson(original.toJson()), original);
    });

    test('copyWith 只改指定字段', () {
      const base = DisplayPrefs();
      final changed = base.copyWith(mouthSensitivity: 2.0);
      expect(changed.mouthSensitivity, 2.0);
      expect(changed.scale, base.scale);
      expect(changed.lipSync, base.lipSync);
      expect(changed.idleEnabled, base.idleEnabled);
    });
  });

  group('区间常量自洽', () {
    test('默认值落在各自区间内', () {
      expect(
        DisplayPrefs.defaultScale,
        inInclusiveRange(DisplayPrefs.minScale, DisplayPrefs.maxScale),
      );
      expect(
        DisplayPrefs.defaultMouthSensitivity,
        inInclusiveRange(
          DisplayPrefs.minMouthSensitivity,
          DisplayPrefs.maxMouthSensitivity,
        ),
      );
    });

    test('口型灵敏度上限不超过渲染面 clamp 上限 4.0', () {
      // 渲染面会把传入值 clamp 到 [0,4]；UI 上限必须 ≤ 它，否则滑杆顶部无效。
      expect(DisplayPrefs.maxMouthSensitivity, lessThanOrEqualTo(4.0));
    });
  });
}
