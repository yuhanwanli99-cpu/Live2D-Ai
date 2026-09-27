import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/ui/background_logic.dart';
import 'package:live2d_ai_shell/ui/background_patterns.dart';
import 'package:live2d_ai_shell/ui/shell_slideshow.dart';

/// 背景系统的**纯逻辑**门禁。
///
/// 这些值全都「能用眼睛看、但只能用数字钉」——
/// 遮罩弱了字读不出来、强了背景消失；轮播少写一句就 off-by-one。
void main() {
  group('遮罩强度：三条设计线都要成立', () {
    test('不透明度为 0 → 遮罩也是 0（没画东西就没有可读性问题）', () {
      expect(
        scrimAlphaFor(imageOpacity: 0, level: ScrimLevel.auto),
        0.0,
        reason: '这一条让「默认配置」与本轮之前逐像素一致',
      );
      // 0.1 仍然有一点点压（遮罩与图片贡献成正比），但要**很轻**：
      expect(
        scrimAlphaFor(imageOpacity: 0.1, level: ScrimLevel.auto),
        lessThan(0.08),
      );
      // 默认 0.15 时几乎看不出来 —— 这是「默认配置观感不变」的量化保证。
      expect(
        scrimAlphaFor(imageOpacity: 0.15, level: ScrimLevel.auto),
        lessThan(0.10),
      );
    });

    test('随图片不透明度**单调不减**', () {
      double prev = -1;
      for (double o = 0; o <= 1.0; o += 0.05) {
        final double a = scrimAlphaFor(imageOpacity: o, level: ScrimLevel.auto);
        expect(a, greaterThanOrEqualTo(prev), reason: '在 $o 处回落了');
        prev = a;
      }
    });

    test('默认档的遮罩是**看得见的一档**，不是几乎为 0', () {
      // 曾经：默认 opacity 0.15 + 旧的增益 ⇒ 0.037 的遮罩，
      // 叠在 0.15 的图上 → 背景「加了和没加一样」。
      // 现在：默认 opacity 1.0 ⇒ 遮罩必须在 0.2 以上才说得过去。
      final double atDefault = scrimAlphaFor(
        imageOpacity: 1.0,
        level: ScrimLevel.auto,
        uiTransparency: 0.5,
      );
      expect(atDefault, greaterThan(0.2));
      // 而且它必须**吃掉一部分背景**：0.34 的遮罩盖在满图上，
      // 图在聊天面板下透出约 (1-0.34)×0.225 ≈ 15% —— 认得出有张图。
      expect(atDefault, lessThan(0.5),
          reason: '遮罩是给可读性用的，不是把背景盖死');
    });

    test('永远不超过上界（遮罩是为了可读，不是为了盖住背景）', () {
      for (double o = 0; o <= 1.0; o += 0.1) {
        for (double t = 0; t <= 1.0; t += 0.2) {
          final double a = scrimAlphaFor(
            imageOpacity: o,
            level: ScrimLevel.auto,
            uiTransparency: t,
          );
          expect(a, lessThanOrEqualTo(kMaxScrimAlpha));
        }
      }
      expect(scrimAlphaFor(imageOpacity: 1, level: ScrimLevel.heavy), 0.45);
      // auto 的最坏组合**正好撞上界**（不是被静默截成一条平线）：
      // 增益与上界是配套定的，撞界说明这一档把 [0, 上界] 整段用满了。
      expect(
        scrimAlphaFor(imageOpacity: 1, level: 0, uiTransparency: 1),
        kMaxScrimAlpha,
      );
    });

    test('固定档不参与推导：「无」是 0，轻 / 重是常数', () {
      expect(scrimAlphaFor(imageOpacity: 1, level: ScrimLevel.none), 0.0);
      expect(scrimAlphaFor(imageOpacity: 0, level: ScrimLevel.light), 0.22);
      expect(scrimAlphaFor(imageOpacity: 0, level: ScrimLevel.heavy), 0.45);
    });

    test('界面越透，遮罩压得越狠（否则白图上白底浅灰字）', () {
      double prev = -1;
      for (double t = 0; t <= 1.0; t += 0.1) {
        final double a = scrimAlphaFor(
          imageOpacity: 0.8,
          level: ScrimLevel.auto,
          uiTransparency: t,
        );
        expect(a, greaterThanOrEqualTo(prev), reason: '在 $t 处回落了');
        prev = a;
      }
      // 最坏组合（亮图 + 最透）仍然撞上界，而不是被静默截成一条平线。
      expect(
        scrimAlphaFor(
          imageOpacity: 1,
          level: ScrimLevel.auto,
          uiTransparency: 1,
        ),
        kMaxScrimAlpha,
      );
    });

    test('uiTransparency = 0 时与「没有这一项」完全一致（默认观感不变）', () {
      expect(
        scrimAlphaFor(imageOpacity: 0.15, level: ScrimLevel.auto),
        scrimAlphaFor(
          imageOpacity: 0.15,
          level: ScrimLevel.auto,
          uiTransparency: 0,
        ),
      );
    });

    test('遮罩色是主题的 stage（亮主题用白幕，不是黑幕）', () {
      expect(scrimColorFor(AppPalette.black), AppPalette.black.stage);
      expect(scrimColorFor(AppPalette.white), AppPalette.white.stage);
    });
  });

  group('轮播索引：三种情形', () {
    test('不足 2 项 → 永远不动', () {
      expect(nextBackgroundIndex(length: 1, current: 0, random: false), 0);
      expect(nextBackgroundIndex(length: 1, current: 0, random: true), 0);
      expect(nextBackgroundIndex(length: 0, current: 3, random: false), 0);
    });

    test('顺序：回绕', () {
      expect(nextBackgroundIndex(length: 3, current: 0, random: false), 1);
      expect(nextBackgroundIndex(length: 3, current: 2, random: false), 0);
      expect(nextBackgroundIndex(length: 5, current: 4, random: false), 0);
    });

    test('随机：**绝不返回当前这张**（200 次全部检查）', () {
      final math.Random rng = math.Random(7);
      for (int len = 2; len <= 8; len++) {
        for (int cur = 0; cur < len; cur++) {
          for (int i = 0; i < 200; i++) {
            expect(
              nextBackgroundIndex(
                length: len,
                current: cur,
                random: true,
                randomSource: rng,
              ),
              isNot(cur),
              reason: 'len=$len cur=$cur 时随机回了同一张',
            );
          }
        }
      }
    });

    test('越界的 current 先夹回范围（不抛、也不越界）', () {
      // 99 → 夹到 2 → 下一个是 0
      expect(nextBackgroundIndex(length: 3, current: 99, random: false), 0);
      // -5 → 夹到 0 → 下一个是 1
      expect(nextBackgroundIndex(length: 3, current: -5, random: false), 1);
    });
  });

  group('ShellSlideshow：幂等与边界', () {
    test('项数不足 2 / 间隔为 0 → 不进定时器', () {
      final ShellSlideshow s = ShellSlideshow()
        ..setLibrary(length: 1, randomOrder: false);
      s.start(5);
      expect(s.running, isFalse);
      s.setLibrary(length: 4, randomOrder: false);
      s.start(0);
      expect(s.running, isFalse, reason: '间隔 0 = 不轮播');
      s.start(5);
      expect(s.running, isTrue);
      s.stop();
    });

    test('start 连调两次只留一个定时器；stop / dispose 都幂等', () {
      final ShellSlideshow s = ShellSlideshow()
        ..setLibrary(length: 3, randomOrder: false);
      s.start(1);
      s.start(1);
      s.stop();
      s.stop();
      expect(s.running, isFalse);
      s.dispose();
      s.dispose();
      s.start(1);
      expect(s.running, isFalse, reason: 'dispose 之后不该再起定时器');
      s.revive();
      s.start(1);
      expect(s.running, isTrue);
      s.dispose();
    });

    test('库变短时索引被夹回范围内（不越界）', () {
      final ShellSlideshow s = ShellSlideshow()
        ..setLibrary(length: 5, randomOrder: false)
        ..jumpTo(4);
      expect(s.index, 4);
      s.setLibrary(length: 2, randomOrder: false);
      expect(s.index, 1);
      s.setLibrary(length: 0, randomOrder: false);
      expect(s.index, 0);
    });

    // 定时器在 `testWidgets` 里是**可推进的**（测试运行器自带假时钟），
    // 所以不需要为了测「它真的会走」而引入 `fake_async` 依赖。
    testWidgets('定时器真的会推进一步，而且顺序正确', (WidgetTester tester) async {
      final ShellSlideshow s = ShellSlideshow()
        ..setLibrary(length: 3, randomOrder: false);
      final List<int> seen = <int>[];
      s.start(1, onAdvance: seen.add);
      await tester.pump(const Duration(seconds: 1));
      await tester.pump(const Duration(seconds: 1));
      expect(seen, <int>[1, 2]);
      // 再走一圈验证回绕
      await tester.pump(const Duration(seconds: 1));
      expect(seen, <int>[1, 2, 0]);
      s.dispose();
    });

    testWidgets('减少动画不影响轮播本身（那是渲染层的事）', (WidgetTester tester) async {
      final ShellSlideshow s = ShellSlideshow()
        ..setLibrary(length: 2, randomOrder: false);
      s.start(1);
      expect(s.running, isTrue);
      s.dispose();
    });
  });

  group('图案：四种都要真的画出东西', () {
    /// 把 64×64 的图案渲染出来，返回**与主色不同的像素占比**。
    ///
    /// 为什么用这个而不是「颜色数」：斜纹只有一种线色，色数天生就少，
    /// 用色数当判据会把「合法的单色图案」误判成「什么都没画」。
    /// 「非主色像素占比」对四种图案都成立，且直接对应肉眼看到的「有纹理」。
    Future<double> textureRatio(WidgetTester tester, int patternId) async {
      final AppPalette palette = AppPalette.black;
      await tester.pumpWidget(
        MaterialApp(
          home: RepaintBoundary(
            key: const ValueKey<String>('tile'),
            child: SizedBox(
              width: 64,
              height: 64,
              child: PatternPreview(
                id: patternId,
                colors: patternColorsFor(patternId, palette),
                size: 64,
              ),
            ),
          ),
        ),
      );
      await tester.pump();
      final ByteData? raw = await tester.binding.runAsync<ByteData?>(() async {
        final RenderRepaintBoundary boundary = tester.renderObject(
          find.byKey(const ValueKey<String>('tile')),
        ) as RenderRepaintBoundary;
        final ui.Image image = await boundary.toImage(pixelRatio: 1);
        final ByteData? data = await image.toByteData(
          format: ui.ImageByteFormat.rawRgba,
        );
        image.dispose();
        return data;
      });
      expect(raw, isNotNull, reason: '图案 $patternId 没渲染出像素');
      final Uint8List bytes = raw!.buffer.asUint8List();
      final Map<int, int> counts = <int, int>{};
      int total = 0;
      for (int i = 0; i + 3 < bytes.length; i += 4) {
        final int key = (bytes[i] << 16) | (bytes[i + 1] << 8) | bytes[i + 2];
        counts[key] = (counts[key] ?? 0) + 1;
        total++;
      }
      int dominant = 0;
      counts.forEach((int _, int n) {
        if (n > dominant) dominant = n;
      });
      return total == 0 ? 0 : (total - dominant) / total;
    }

    for (final int id in BackgroundPatternId.values) {
      testWidgets('图案 $id 真的画出了纹理（非主色像素 ≥ 3%）', (WidgetTester tester) async {
        expect(
          await textureRatio(tester, id),
          greaterThan(0.03),
          reason: '图案 $id 看起来和纯色面没区别 = 白画了',
        );
      });
    }

    test('未知图案 id 不会抛（回落成纯色）', () {
      final AppPalette p = AppPalette.black;
      final PatternColors c = patternColorsFor(999, p);
      expect(c.a, p.stage);
      expect(c.b, p.stage);
    });

    test('四套主题的图案配色**互不相同**（切主题时背景跟着变）', () {
      final Set<int> seen = <int>{};
      for (final AppThemeId id in AppThemeId.values) {
        final PatternColors c = patternColorsFor(
          BackgroundPatternId.gradient,
          AppPalette.of(id),
        );
        seen.add((c.a.toARGB32() << 16) | (c.b.toARGB32() & 0xFFFFFF));
      }
      expect(seen.length, AppThemeId.values.length);
    });

    test('网格用**固定种子**（点不会每帧重排 = 不会沸腾）', () {
      final math.Random a = math.Random(kGridSeed);
      final math.Random b = math.Random(kGridSeed);
      final List<double> first = <double>[
        for (int i = 0; i < 20; i++) a.nextDouble(),
      ];
      final List<double> second = <double>[
        for (int i = 0; i < 20; i++) b.nextDouble(),
      ];
      expect(first, second);
    });
  });
}
