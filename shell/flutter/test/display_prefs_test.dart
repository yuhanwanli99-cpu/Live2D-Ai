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

    test('默认出声（2026-09-10 用户裁决：默认打开声音）', () {
      const prefs = DisplayPrefs();
      expect(
        prefs.muted,
        isFalse,
        reason: '产品默认就该能听见，否则用户会以为 TTS 坏了',
      );
      expect(
        prefs.volume,
        DisplayPrefs.defaultVolume,
        reason: '默认满音量',
      );
      // 静音与口型是**两个独立开关**——静音不得连带关掉口型。
      expect(prefs.lipSync, isTrue, reason: '静音不应关闭口型');
    });

    test('静音与音量正交：静音不重置音量', () {
      const prefs = DisplayPrefs(volume: 0.35, muted: true);
      final unmuted = prefs.copyWith(muted: false);
      expect(
        unmuted.volume,
        0.35,
        reason: '取消静音后应恢复用户原来的音量，而不是跳回 100%',
      );
    });

    test('静音与口型可独立切换（四象限都可表达）', () {
      for (final muted in [true, false]) {
        for (final lip in [true, false]) {
          final prefs = DisplayPrefs(muted: muted, lipSync: lip);
          expect(prefs.muted, muted);
          expect(prefs.lipSync, lip);
        }
      }
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
        'muted': 1,
        'volume': 'loud',
      });
      expect(prefs, const DisplayPrefs());
    });

    test('非有限数回落默认（不是夹到边界）', () {
      final prefs = DisplayPrefs.fromJson(<String, Object?>{
        'scale': double.nan,
        'mouthSensitivity': double.infinity,
        'volume': double.nan,
      });
      expect(prefs.scale, DisplayPrefs.defaultScale);
      expect(prefs.mouthSensitivity, DisplayPrefs.defaultMouthSensitivity);
      expect(prefs.volume, DisplayPrefs.defaultVolume);
    });

    test('越界值夹到区间内', () {
      final low = DisplayPrefs.fromJson(<String, Object?>{
        'scale': -5.0,
        'mouthSensitivity': 0.0,
        'volume': -3.0,
      });
      expect(low.scale, DisplayPrefs.minScale);
      expect(low.mouthSensitivity, DisplayPrefs.minMouthSensitivity);
      expect(low.volume, DisplayPrefs.minVolume);

      final high = DisplayPrefs.fromJson(<String, Object?>{
        'scale': 99.0,
        'mouthSensitivity': 99.0,
        'volume': 99.0,
      });
      expect(high.scale, DisplayPrefs.maxScale);
      expect(high.mouthSensitivity, DisplayPrefs.maxMouthSensitivity);
      expect(high.volume, DisplayPrefs.maxVolume);
    });

    test('合法值原样保留', () {
      final prefs = DisplayPrefs.fromJson(<String, Object?>{
        'scale': 1.4,
        'mouthSensitivity': 1.8,
        'lipSync': false,
        'idleEnabled': false,
        'muted': true,
        'volume': 0.42,
      });
      expect(prefs.scale, 1.4);
      expect(prefs.mouthSensitivity, 1.8);
      expect(prefs.lipSync, isFalse);
      expect(prefs.idleEnabled, isFalse);
      expect(prefs.muted, isTrue);
      expect(prefs.volume, 0.42);
    });

    test('旧版本存档（没有 volume/muted 字段）按新默认读', () {
      // 关键回归：volume 是后加字段，缺省必须回落到**满音量**，
      // 而不是 0——否则升级后老用户会突然变哑巴。
      final prefs = DisplayPrefs.fromJson(<String, Object?>{
        'scale': 1.2,
        'mouthSensitivity': 1.5,
      });
      expect(prefs.volume, DisplayPrefs.defaultVolume);
      expect(prefs.muted, isFalse, reason: '旧存档里 muted=true 是旧默认，不该继承');
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
        muted: true,
        volume: 0.6,
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
      expect(changed.muted, base.muted);
      expect(changed.volume, base.volume);
    });

    test('copyWith 能单独改音量与静音', () {
      const base = DisplayPrefs();
      expect(base.copyWith(volume: 0.25).volume, 0.25);
      expect(base.copyWith(volume: 0.25).scale, base.scale);
      expect(base.copyWith(muted: true).muted, isTrue);
      expect(base.copyWith(muted: true).volume, base.volume);
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
      expect(
        DisplayPrefs.defaultVolume,
        inInclusiveRange(DisplayPrefs.minVolume, DisplayPrefs.maxVolume),
      );
    });

    test('音量区间是 [0,1]（与 WebAudio 线性增益同量纲）', () {
      expect(DisplayPrefs.minVolume, 0.0);
      expect(DisplayPrefs.maxVolume, 1.0);
      expect(DisplayPrefs.defaultVolume, 1.0, reason: '默认满音量');
    });

    test('口型灵敏度上限不超过渲染面 clamp 上限 4.0', () {
      // 渲染面会把传入值 clamp 到 [0,4]；UI 上限必须 ≤ 它，否则滑杆顶部无效。
      expect(DisplayPrefs.maxMouthSensitivity, lessThanOrEqualTo(4.0));
    });
  });

  _p4NewFieldsTests();
}

void _p4NewFieldsTests() {
  group('P4 新增字段：allowDragZoom 与 tier', () {
    test('默认值：允许拖动缩放 = true，档位 = 8192', () {
      const DisplayPrefs p = DisplayPrefs();
      expect(p.allowDragZoom, isTrue, reason: '保持既有手感，默认允许');
      expect(p.tier, 8192);
      expect(DisplayPrefs.tiers, <int>[4096, 8192, 16384]);
    });

    test('旧存档（没有这两个字段）→ 按新默认读，而不是 false/0', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'scale': 1.2,
        'volume': 0.5,
      });
      expect(p.allowDragZoom, isTrue);
      expect(p.tier, DisplayPrefs.defaultTier);
      expect(p.scale, 1.2);
    });

    test('tier 夹到**最近合法档位**（不是区间边界）', () {
      // 不是简单 clamp：10000 更接近 8192。
      expect(clampTier(10000), 8192);
      // 中点 12288：12000 距 8192 更近（3808 < 4384），13000 距 16384 更近。
      expect(clampTier(12000), 8192, reason: '12000 距 8192 更近');
      expect(clampTier(13000), 16384, reason: '13000 距 16384 更近');
      expect(clampTier(1), 4096);
      expect(clampTier(999999), 16384);
      expect(clampTier(8192), 8192);
      expect(DisplayPrefs.fromJson(<String, Object?>{'tier': 10000}).tier, 8192);
      expect(DisplayPrefs.fromJson(<String, Object?>{'tier': 'x'}).tier, 8192);
    });

    test('copyWith 能单独改这两个字段，且互不影响', () {
      const DisplayPrefs p = DisplayPrefs();
      final DisplayPrefs a = p.copyWith(allowDragZoom: false);
      expect(a.allowDragZoom, isFalse);
      expect(a.tier, p.tier);
      final DisplayPrefs b = p.copyWith(tier: 4096);
      expect(b.tier, 4096);
      expect(b.allowDragZoom, isTrue);
    });

    test('往返幂等', () {
      final DisplayPrefs p = const DisplayPrefs()
          .copyWith(allowDragZoom: false, tier: 16384);
      expect(DisplayPrefs.fromJson(p.toJson()), p);
    });

    // 这一条是「静默失效」的钉子：`AppShell` 的 `_updatePrefs` 用
    // `next == _prefs` 做短路，**漏字段就等于改了不生效**。
    test('== 与 hashCode 覆盖了新字段（漏了就会「改了不生效」）', () {
      const DisplayPrefs base = DisplayPrefs();
      expect(base == base.copyWith(allowDragZoom: false), isFalse);
      expect(base == base.copyWith(tier: 4096), isFalse);
      expect(
        base.copyWith(allowDragZoom: false).hashCode == base.hashCode,
        isFalse,
      );
      expect(base.copyWith(tier: 4096).hashCode == base.hashCode, isFalse);
      expect(base == base.copyWith(), isTrue);
    });
  });
}
