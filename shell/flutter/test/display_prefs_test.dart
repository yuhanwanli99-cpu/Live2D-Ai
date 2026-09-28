import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';

/// 外观分区的源码 = 主文件 + 它的 part。
///
/// 2026-09-28（Stage B · R6-b 第 0 步）：背景域被**原样抽取**到
/// `appearance_background.dart`（那是 `appearance_section.dart` 的 `part`）。
/// 这两条源码扫描断言关心的是「外观分区里有没有读这个字段」，不是「它在哪个
/// 文件里」——所以扫描面取**并集**，抽取前后断言强度不变。
String appearanceSectionSource() =>
    File('lib/settings/sections/appearance_section.dart').readAsStringSync() +
    File('lib/settings/sections/appearance_background.dart').readAsStringSync();

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
      expect(prefs.muted, isFalse, reason: '产品默认就该能听见，否则用户会以为 TTS 坏了');
      expect(prefs.volume, DisplayPrefs.defaultVolume, reason: '默认满音量');
      // 静音与口型是**两个独立开关**——静音不得连带关掉口型。
      expect(prefs.lipSync, isTrue, reason: '静音不应关闭口型');
    });

    test('静音与音量正交：静音不重置音量', () {
      const prefs = DisplayPrefs(volume: 0.35, muted: true);
      final unmuted = prefs.copyWith(muted: false);
      expect(unmuted.volume, 0.35, reason: '取消静音后应恢复用户原来的音量，而不是跳回 100%');
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
      expect(
        DisplayPrefs.fromJson(<String, Object?>{'tier': 10000}).tier,
        8192,
      );
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
      final DisplayPrefs p = const DisplayPrefs().copyWith(
        allowDragZoom: false,
        tier: 16384,
      );
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

  group('主题（2026-09-11 用户裁决：黑/白/蓝/灰，默认黑）', () {
    test('默认是黑（保留产品原本的观感）', () {
      expect(const DisplayPrefs().theme, AppThemeId.black);
      expect(const DisplayPrefs().theme, AppThemeId.fallback);
    });

    test('四套主题都能写进 JSON 再读回来', () {
      for (final AppThemeId id in AppThemeId.values) {
        const DisplayPrefs base = DisplayPrefs();
        final DisplayPrefs round = DisplayPrefs.fromJson(
          base.copyWith(theme: id).toJson(),
        );
        expect(round.theme, id, reason: '$id 往返丢了');
      }
    });

    test('旧存档（没有 theme 字段）→ 默认黑，而不是「假装用户选过白」', () {
      final Map<String, Object?> old = const DisplayPrefs().toJson()
        ..remove('theme');
      expect(DisplayPrefs.fromJson(old).theme, AppThemeId.black);
    });

    test('坏 theme 值（数字 / 大小写不符 / 未知词）→ 默认黑，不抛', () {
      for (final Object? bad in <Object?>[3, 'BLACK', 'purple', true, null]) {
        final Map<String, Object?> json = const DisplayPrefs().toJson()
          ..['theme'] = bad;
        expect(DisplayPrefs.fromJson(json).theme, AppThemeId.black);
      }
    });

    test('换主题不碰其它字段（主题是纯叠加的一维）', () {
      const DisplayPrefs base = DisplayPrefs(muted: true, volume: 0.4);
      final DisplayPrefs white = base.copyWith(theme: AppThemeId.white);
      expect(white.theme, AppThemeId.white);
      expect(white.muted, isTrue);
      expect(white.volume, 0.4);
      expect(white.scale, base.scale);
    });

    test('主题参与 == / hashCode（否则 `if (next == _prefs) return;` 会吞掉换肤）', () {
      const DisplayPrefs black = DisplayPrefs();
      final DisplayPrefs gray = black.copyWith(theme: AppThemeId.gray);
      expect(gray == black, isFalse);
      expect(gray.hashCode == black.hashCode, isFalse);
      expect(gray == black.copyWith(theme: AppThemeId.gray), isTrue);
    });
  });

  group('舞台背景图（2026-09-11：用户自定义展台图，与纯色底叠放）', () {
    const String tiny = 'data:image/png;base64,iVBORw0KGgo=';

    test('默认没有背景图（默认是纯色舞台）', () {
      expect(const DisplayPrefs().stageImage, isNull);
    });

    test('写进 JSON 再读回来', () {
      final DisplayPrefs p = const DisplayPrefs().copyWith(stageImage: tiny);
      expect(DisplayPrefs.fromJson(p.toJson()).stageImage, tiny);
    });

    test('clearStageImage 是**独立**开关：不用它就无法把图设回 null', () {
      final DisplayPrefs withImage = const DisplayPrefs().copyWith(
        stageImage: tiny,
      );
      // 只传 `stageImage: null` 应该**什么都不改**（否则「不改」与「清除」
      // 会被同一个缺省值混成同一件事）。
      expect(withImage.copyWith(stageImage: null).stageImage, tiny);
      expect(withImage.copyWith(clearStageImage: true).stageImage, isNull);
    });

    test('换主题不会顺手清掉背景图（两个字段正交）', () {
      final DisplayPrefs p = const DisplayPrefs().copyWith(stageImage: tiny);
      expect(p.copyWith(theme: AppThemeId.blue).stageImage, tiny);
    });

    test('超限的图**读不进来**（写不进去的存储不该在读取时装作有）', () {
      final Map<String, Object?> json = const DisplayPrefs().toJson()
        ..['stageImage'] = 'x' * (kStageImageMaxChars + 1);
      expect(DisplayPrefs.fromJson(json).stageImage, isNull);
    });

    test('刚好等于上限的图**读得进来**（边界是闭区间）', () {
      final String atLimit = 'x' * kStageImageMaxChars;
      final Map<String, Object?> json = const DisplayPrefs().toJson()
        ..['stageImage'] = atLimit;
      expect(DisplayPrefs.fromJson(json).stageImage, atLimit);
    });

    test('坏值（数字 / 空串 / null）→ 没有背景图，不抛', () {
      for (final Object? bad in <Object?>[3, '', null, <String>[], true]) {
        final Map<String, Object?> json = const DisplayPrefs().toJson()
          ..['stageImage'] = bad;
        expect(DisplayPrefs.fromJson(json).stageImage, isNull);
      }
    });

    test('背景图参与 == / hashCode（否则换图不会下发到渲染面）', () {
      const DisplayPrefs none = DisplayPrefs();
      final DisplayPrefs withImage = none.copyWith(stageImage: tiny);
      expect(withImage == none, isFalse);
      expect(withImage.hashCode == none.hashCode, isFalse);
      expect(withImage.copyWith(clearStageImage: true) == none, isTrue);
    });
  });

  // ───────────────────────────────────────────────────────────────────────────
  // P0-3（2026-09-11）：区间常量必须有**唯一真源**
  //
  // `DisplayPrefs.minScale/maxScale/minMouthSensitivity/maxMouthSensitivity`
  // 同时被两处消费：① `clampScale` / `clampMouthSensitivity`（协议侧，
  // 真正决定发给渲染面的值）；② 「外观与互动」分区的滑杆 `min:` / `max:`
  // （用户看到并能拖的范围）。
  //
  // 滑杆曾经把这四个数**再写一遍字面量**，于是会有「滑杆能拖到 2.0，
  // 但 clamp 只到 1.8」这种只有手拖到底才会发现的错位。
  // 这条测试直接扫源码，钉死滑杆不许再写字面量。
  // ───────────────────────────────────────────────────────────────────────────
  group('P0-3：滑杆区间与 clamp 共用同一组常量', () {
    test('appearance_section 的滑杆不写字面量区间', () {
      final String src = File('lib/settings/sections/appearance_section.dart')
          .readAsStringSync();
      // ⚠️ 字面量里的 `.` **必须转义**：不转的话 `3.0` 会匹配 `300`
      //（`.` 是通配符），这条守卫就会在「合法地写了 300」时误报。
      for (final String literal in <String>['0.5', '2.0', '0.2', '3.0']) {
        expect(
          RegExp('(min|max):\\s*${literal.replaceAll('.', r'\.')}\\b')
              .hasMatch(src),
          isFalse,
          reason: '滑杆又写死了区间字面量 $literal，应当读 DisplayPrefs 常量',
        );
      }
    });

    test('滑杆确实读了那四个常量（防「删掉字面量但也没接上」）', () {
      final String src = File('lib/settings/sections/appearance_section.dart')
          .readAsStringSync();
      for (final String name in <String>[
        'DisplayPrefs.minScale',
        'DisplayPrefs.maxScale',
        'DisplayPrefs.minMouthSensitivity',
        'DisplayPrefs.maxMouthSensitivity',
      ]) {
        expect(src.contains(name), isTrue, reason: '滑杆没有读 $name');
      }
    });
  });

  group('材质旋钮（2026-09-27 减法后：只剩描边强度）', () {
    test('圆角幅度**不再是偏好**（web 端无需繁杂设置）', () {
      const DisplayPrefs p = DisplayPrefs();
      // 圆角由 `AppMaterial.kFixedRadiusScale` 固定，不经偏好，所以三处都没有它：
      // ① 序列化；② 偏好类本身；③ 设置界面里的滑杆。
      expect(p.toJson().containsKey('radiusScale'), isFalse);
      expect(
        File('lib/settings/display_prefs.dart').readAsStringSync(),
        isNot(contains('radiusScale')),
      );
      expect(
        File('lib/settings/sections/appearance_section.dart')
            .readAsStringSync(),
        isNot(contains("label: '圆角幅度'")),
      );
    });

    test('描边强度：默认 1.0，越界夹到区间', () {
      const DisplayPrefs p = DisplayPrefs();
      expect(p.edgeStrength, DisplayPrefs.defaultEdgeStrength);
      expect(p.edgeStrength, 1.0);
      expect(DisplayPrefs.clampEdgeStrength(double.nan), 1.0);
      expect(DisplayPrefs.clampEdgeStrength(0), DisplayPrefs.minEdgeStrength);
      expect(DisplayPrefs.clampEdgeStrength(99), DisplayPrefs.maxEdgeStrength);
    });

    test('描边强度写进 JSON 再读回来，且参与 ==', () {
      final DisplayPrefs p = const DisplayPrefs().copyWith(edgeStrength: 1.3);
      expect(
        DisplayPrefs.fromJson(p.toJson()).edgeStrength,
        closeTo(1.3, 1e-9),
      );
      expect(p, isNot(const DisplayPrefs()));
    });

    test('外观分区读的是同一组常量（不写死区间）', () {
      final String src = appearanceSectionSource();
      for (final String name in <String>[
        'DisplayPrefs.minEdgeStrength',
        'DisplayPrefs.maxEdgeStrength',
      ]) {
        expect(src.contains(name), isTrue, reason: '滑杆没有读 $name');
      }
    });
  });

  group('背景库（2026-09-27：有序图库 + 内置图案 + 轮播参数）', () {
    const String tiny = 'data:image/png;base64,iVBORw0KGgo=';
    final String big = 'data:image/png;base64,${'A' * 100}';

    test('缺省就是「没有背景」——与本轮之前「没选图」的观感一致', () {
      const DisplayPrefs p = DisplayPrefs();
      expect(p.backgrounds, isEmpty);
      expect(p.hasBackground, isFalse);
      expect(p.backgroundSource, DisplayPrefs.backgroundSourceLibrary);
      expect(p.backgroundOpacity, DisplayPrefs.defaultBackgroundOpacity);
      // 默认 1.0：用户亲手挑的图就该是看到的图（0.15 是 rc.5 装饰底纹的遗留值）。
      expect(p.backgroundOpacity, 1.0);
      expect(p.backgroundBlur, 0.0);
      expect(p.backgroundScrim, DisplayPrefs.defaultBackgroundScrim);
      expect(p.imageFit, DisplayPrefs.defaultImageFit);
      expect(p.imageAlign, DisplayPrefs.defaultImageAlign);
      expect(p.slideRandom, isFalse);
    });

    test('写入再读回，**顺序**必须原样保留（轮播按序）', () {
      final DisplayPrefs p = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[
          BackgroundImage(id: 'tiny', dataUrl: tiny),
          BackgroundPattern(BackgroundPatternId.grid),
          BackgroundImage(id: 'big', dataUrl: big),
        ],
      );
      final DisplayPrefs back = DisplayPrefs.fromJson(p.toJson());
      expect(back.backgrounds.length, 3);
      expect(back.backgrounds[0], const BackgroundImage(id: 'tiny', dataUrl: tiny));
      expect(back.backgrounds[1].kind, 'pattern');
      expect(back.backgrounds[2], BackgroundImage(id: 'big', dataUrl: big));
    });

    test('旧存档的 shellImage（单张）迁移成一项', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'shellImage': tiny,
      });
      expect(p.backgrounds.length, 1);
      // id 由内容算 ⇒ 与背景库里同一张图会被认成同一项。
      expect(
        p.backgrounds.first,
        BackgroundImage(id: backgroundIdOf(tiny), dataUrl: tiny),
      );
      // 写回时**不再**写旧键 —— 迁移只发生一次。
      expect(p.toJson().containsKey('shellImage'), isFalse);
    });

    test('坏项被丢弃且**不抛**（非 map / 未知 kind / 图案 id 非法）', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'backgrounds': <Object?>[
          'not a map',
          <String, Object?>{'kind': 'nope'},
          <String, Object?>{'kind': 'image'}, // 缺 dataUrl
          <String, Object?>{'kind': 'image', 'dataUrl': 42},
          <String, Object?>{'kind': 'pattern', 'id': 999},
          <String, Object?>{'kind': 'pattern', 'id': BackgroundPatternId.glow},
        ],
      });
      expect(p.backgrounds.length, 1);
      expect(
        p.backgrounds.first,
        const BackgroundPattern(BackgroundPatternId.glow),
      );
    });

    test('清单里只有 id 也读得回来（字节在 IndexedDB，不在偏好里）', () {
      // 2026-09-27 改：图字节搬去了 IndexedDB，偏好里只剩 id。
      // 这条守着「读偏好不碰字节库」——那是 `fromJson` 保持同步纯逻辑的前提。
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'backgrounds': <Object?>[
          <String, Object?>{'kind': 'image', 'id': 'bg0000001'},
          <String, Object?>{'kind': 'image', 'id': 'bg0000002'},
          for (int i = 0; i < kBackgroundMaxCount + 3; i++)
            <String, Object?>{
              'kind': 'pattern',
              'id': BackgroundPatternId.grid,
            },
        ],
      });
      // 两张图都留着（**没有**逐项大小上限了），但项数仍然截断。
      expect(p.backgrounds.length, kBackgroundMaxCount);
      expect(p.backgrounds[0], const BackgroundImage(id: 'bg0000001'));
      expect(p.backgrounds[1], const BackgroundImage(id: 'bg0000002'));
      // 字节没在手 ⇒ 这一项**画不出来**（`isRenderable` 为 false），
      // 界面据此显示占位块，而不是给 `Image.memory` 传空。
      expect(p.backgrounds[0].isRenderable, isFalse);
    });

    test('旧档的 dataUrl 读成「带字节的同一项」（一次启动完成搬家）', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'backgrounds': <Object?>[
          <String, Object?>{'kind': 'image', 'dataUrl': tiny},
        ],
      });
      final BackgroundImage item = p.backgrounds.single as BackgroundImage;
      expect(item.dataUrl, tiny, reason: '旧档的字节必须原样带出来');
      expect(item.id, backgroundIdOf(tiny), reason: 'id 由内容决定');
      // **写回时只写 id** —— 这是整个搬库的关键一行。
      final Map<String, Object?> written = item.toJson();
      expect(written.containsKey('dataUrl'), isFalse);
      expect(written['id'], backgroundIdOf(tiny));
    });

    test('toJson 产出的清单里**一个 dataUrl 都不许有**', () {
      // 有了它，图就又回到 localStorage，整个搬库白做。
      final DisplayPrefs p = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[
          BackgroundImage(id: 'a', dataUrl: tiny),
          BackgroundImage(id: 'b'),
          const BackgroundPattern(BackgroundPatternId.grid),
        ],
      );
      final String dumped = jsonEncode(p.toJson());
      expect(dumped.contains('dataUrl'), isFalse);
      expect(dumped.contains('base64'), isFalse);
    });

    test('canAddBackground：只拦项数与单张体积（**没有总预算**）', () {
      final List<BackgroundItem> one = <BackgroundItem>[
        const BackgroundImage(id: 'tiny', dataUrl: tiny),
      ];
      expect(
        DisplayPrefs.canAddBackground(
          one,
          const BackgroundImage(id: 'tiny2', dataUrl: tiny),
        ),
        isTrue,
      );
      // 图案零成本：除非项数满了，否则永远拦不住它。
      expect(
        DisplayPrefs.canAddBackground(one, const BackgroundPattern(0)),
        isTrue,
      );
      // 一张普通桌面壁纸（≈3 MB）必须能加进去 ——
      // 这正是 2026-09-27 之前「3 张图一张都存不进去」的那个坑。
      expect(
        DisplayPrefs.canAddBackground(
          one,
          BackgroundImage(
            id: 'wall',
            dataUrl: 'data:image/png;base64,${'A' * (3 * 1024 * 1024)}',
          ),
        ),
        isTrue,
        reason: '3 MB 的图曾被 400 KB 的上限拒掉，那不是安全边际是事故',
      );
      // 真正超大（> 24 MB）才拦：那是防手抖，不是配额。
      expect(
        DisplayPrefs.canAddBackground(
          one,
          BackgroundImage(
            id: 'huge',
            dataUrl: 'data:image/png;base64,${'A' * (kBackgroundImageMaxBytes * 2)}',
          ),
        ),
        isFalse,
      );
      // 项数满
      final List<BackgroundItem> full = <BackgroundItem>[
        for (int i = 0; i < kBackgroundMaxCount; i++)
          const BackgroundPattern(0),
      ];
      expect(
        DisplayPrefs.canAddBackground(full, const BackgroundPattern(1)),
        isFalse,
      );
    });

    test('越界 / 非有限数一律 clamp 到区间（模糊不许变负）', () {
      expect(DisplayPrefs.clampBackgroundBlur(double.nan), 0.0);
      expect(DisplayPrefs.clampBackgroundBlur(-5), 0.0);
      expect(
        DisplayPrefs.clampBackgroundBlur(99),
        DisplayPrefs.maxBackgroundBlur,
      );
      // 非有限数（含 ±Infinity）一律回落默认，与 clampVolume 等同一条纪律。
      expect(
        DisplayPrefs.clampBackgroundOpacity(double.infinity),
        DisplayPrefs.defaultBackgroundOpacity,
      );
      expect(DisplayPrefs.clampBackgroundOpacity(-1), 0.0);
    });

    test('枚举字段越界**回落默认**而不是夹到端点', () {
      // 端点有语义（0=auto / 1=无），把坏值夹到 1 会让背景不可读。
      // **不含 slideInterval**：DEC-1（2026-09-28）把它改成了端点夹持，
      // 回归在 `display_prefs_background_fit_test.dart` 的 DEC-1 组。
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'backgroundScrim': 99,
        'imageAlign': -3,
        'imageFit': 42,
      });
      expect(p.backgroundScrim, DisplayPrefs.defaultBackgroundScrim);
      expect(p.imageAlign, DisplayPrefs.defaultImageAlign);
      expect(p.imageFit, DisplayPrefs.defaultImageFit);
    });

    test('背景库参与 == / hashCode（改一张图会被判成「变了」）', () {
      final DisplayPrefs base = const DisplayPrefs();
      final DisplayPrefs withOne = base.copyWith(
        backgrounds: <BackgroundItem>[const BackgroundImage(id: 'tiny', dataUrl: tiny)],
      );
      expect(withOne, isNot(base));
      expect(withOne.copyWith(backgrounds: const <BackgroundItem>[]), base);
      // 内容相同但实例不同 → 必须相等（否则每次反序列化都会触发重建）
      expect(
        base.copyWith(
          backgrounds: <BackgroundItem>[const BackgroundPattern(0)],
        ),
        base.copyWith(
          backgrounds: <BackgroundItem>[const BackgroundPattern(0)],
        ),
      );
      expect(
        base
            .copyWith(backgrounds: <BackgroundItem>[const BackgroundPattern(0)])
            .hashCode,
        base
            .copyWith(backgrounds: <BackgroundItem>[const BackgroundPattern(0)])
            .hashCode,
      );
    });

    test('九宫格表：4 必须是居中，且 9 档两两不同', () {
      expect(DisplayPrefs.alignTable.length, 9);
      expect(DisplayPrefs.alignTable[4], (x: 0, y: 0));
      final Set<({double x, double y})> cells = DisplayPrefs.alignTable.toSet();
      expect(cells.length, 9);
    });

    test('fitName 的四个取值都翻译得出（cover 是兜底）', () {
      expect(DisplayPrefs.fitName(0), 'cover');
      expect(DisplayPrefs.fitName(1), 'contain');
      expect(DisplayPrefs.fitName(2), 'stretch');
      expect(DisplayPrefs.fitName(3), 'tile');
      expect(DisplayPrefs.fitName(999), 'cover');
    });

    test('外观分区真的接上了新的旋钮（不写死区间）', () {
      final String src = appearanceSectionSource();
      for (final String name in <String>[
        // 可见控件
        'prefs.backgroundScrim',
        'prefs.imageFit',
        'prefs.uiTransparency',
        'DisplayPrefs.minBackgroundOpacity',
        'DisplayPrefs.maxBackgroundOpacity',
        'DisplayPrefs.maxUiTransparency',
        'DisplayPrefs.backgroundSourceLibrary',
        // 折叠/条件显示的控件
        'DisplayPrefs.minSlideIntervalSeconds',
        'DisplayPrefs.maxSlideIntervalSeconds',
      ]) {
        expect(src.contains(name), isTrue, reason: '设置里没有读 $name');
      }
    });

    test('界面透明程度：缺省 0.5（**不是 0** —— 否则背景被面板全遮住）', () {
      const DisplayPrefs p = DisplayPrefs();
      expect(p.uiTransparency, DisplayPrefs.defaultUiTransparency);
      expect(p.uiTransparency, 0.5);
      expect(p.uiTransparency, greaterThan(0.0));
      // 0.5 → 面板 alpha 0.775：设置面板、组卡片、聊天面板、顶栏都能透出背景。
    });

    test('界面透明程度写进 JSON 再读回来', () {
      final DisplayPrefs p = const DisplayPrefs().copyWith(uiTransparency: 0.7);
      expect(
        DisplayPrefs.fromJson(p.toJson()).uiTransparency,
        closeTo(0.7, 1e-9),
      );
    });

    test('旧存档没有 uiTransparency → 默认 0（不是「假装用户调过」）', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'theme': 'blue',
      });
      expect(p.uiTransparency, DisplayPrefs.defaultUiTransparency);
    });

    test('越界 / 非有限数 → 夹到区间（非有限数回落**默认 0.5**）', () {
      expect(DisplayPrefs.clampUiTransparency(double.nan), 0.5);
      expect(DisplayPrefs.clampUiTransparency(-3), 0.0);
      expect(DisplayPrefs.clampUiTransparency(9), 1.0);
    });

    test('它参与 == （改透明度会被判成「变了」，从而重建主题）', () {
      final DisplayPrefs base = const DisplayPrefs();
      expect(base.copyWith(uiTransparency: 0.4), isNot(base));
    });

    test('外观分区读的是同一组常量（不写死区间）', () {
      final String src = appearanceSectionSource();
      for (final String name in <String>[
        'DisplayPrefs.maxUiTransparency',
        'prefs.uiTransparency',
        'DisplayPrefs.minSlideIntervalSeconds',
        'DisplayPrefs.maxSlideIntervalSeconds',
        // 模糊与位置垫在 2026-09-27 的「减肥」里被折起来了（模糊移到「更多」，
        // 位置只在「完整」铺法下出现），所以它们**不该**再出现在主控件里。
      ]) {
        expect(src.contains(name), isTrue, reason: '设置里没有读 $name');
      }
    });
  });

  group('背景来源（2026-09-27 修：加了图却什么都没变）', () {
    test('缺省来源 = 背景库（新用户加了图就能看见）', () {
      const DisplayPrefs p = DisplayPrefs();
      expect(p.backgroundSource, DisplayPrefs.backgroundSourceLibrary);
      expect(DisplayPrefs.backgroundSourceLibrary, 0);
    });

    test('**库里非空 = 一定有东西要画**（这条就是本缺陷的守门人）', () {
      // 原缺陷：`syncShellStageBg` 默认 true 时，壳去读从没设过的
      // `stageImage` → null → 用户加进背景库的 3 张图一张都不显示。
      final DisplayPrefs p = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[
          const BackgroundImage(id: 'a', dataUrl: 'data:image/png;base64,AAA'),
          const BackgroundImage(id: 'b', dataUrl: 'data:image/png;base64,BBB'),
        ],
      );
      expect(p.effectiveBackground, isNotNull);
      expect(p.hasBackground, isTrue);
    });

    test('只有真把不透明度调到 0 才「没有背景」', () {
      final DisplayPrefs p = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[const BackgroundImage(
          id: 'x',
          dataUrl: 'data:image/png;base64,AAA',
        )],
      );
      expect(p.hasBackground, isTrue);
      expect(
        p.copyWith(backgroundOpacity: 0).hasBackground,
        isFalse,
        reason: '「不透明度 0」是唯一能显式关掉背景的旋钮',
      );
    });

    test('来源 = 舞台那张时，才轮到 stageImage 说话', () {
      final DisplayPrefs lib = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[const BackgroundImage(id: 'lib', dataUrl: 'lib')],
        stageImage: 'stage',
      );
      final DisplayPrefs stage = lib.copyWith(
        backgroundSource: DisplayPrefs.backgroundSourceStageImage,
      );
      expect(lib.effectiveBackground, const BackgroundImage(id: 'lib', dataUrl: 'lib'));
      // 「舞台那张」用固定 id：它不走字节库（走渲染面 `stage-bg` 协议），
      // 用内容哈希只会让每帧都对一张几 MB 的 dataURL 重算一遍。
      expect(
        stage.effectiveBackground,
        const BackgroundImage(
          id: DisplayPrefs.kStageImageItemId,
          dataUrl: 'stage',
        ),
      );
    });

    test('来源 = 舞台那张但从没设过图 → 没有背景（而不是回退去画库）', () {
      final DisplayPrefs p = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[const BackgroundImage(id: 'lib', dataUrl: 'lib')],
        backgroundSource: DisplayPrefs.backgroundSourceStageImage,
      );
      expect(p.effectiveBackground, isNull);
      expect(p.hasBackground, isFalse);
    });

    test('旧存档迁移：设过舞台图的老用户**界面不变**', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'syncShellStageBg': true,
        'stageImage': 'data:image/png;base64,AAA',
      });
      expect(
        p.backgroundSource,
        DisplayPrefs.backgroundSourceStageImage,
        reason: '「壳跟随舞台」的老用户看到的界面必须一个像素都不变',
      );
    });

    test('旧存档迁移：只加过背景库图的（绝大多数）→ 迁到背景库', () {
      // 这正是本缺陷现场：syncShellStageBg 默认 true，stageImage 为 null。
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'syncShellStageBg': true,
        'backgrounds': <Object?>[
          <String, Object?>{
            'kind': 'image',
            'dataUrl': 'data:image/png;base64,AAA',
          },
          <String, Object?>{
            'kind': 'image',
            'dataUrl': 'data:image/png;base64,BBB',
          },
        ],
      });
      expect(p.backgroundSource, DisplayPrefs.backgroundSourceLibrary);
      expect(p.hasBackground, isTrue, reason: '迁移后这 3 张图立刻可见');
    });

    test('旧的 syncShellStageBg 键不再被写回（迁移只发生一次）', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'syncShellStageBg': true,
        'stageImage': 'data:image/png;base64,AAA',
      });
      expect(p.toJson().containsKey('syncShellStageBg'), isFalse);
      expect(p.toJson().containsKey('backgroundSource'), isTrue);
    });

    test('来源字段参与 == / hashCode', () {
      final DisplayPrefs base = const DisplayPrefs();
      expect(
        base.copyWith(
          backgroundSource: DisplayPrefs.backgroundSourceStageImage,
        ),
        isNot(base),
      );
    });
  });
}
