import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/breakpoints.dart';

void main() {
  group('sizeClassOf 边界（含等号）', () {
    test('两级阈值都在「含」的一侧', () {
      // 边界是断点逻辑最容易写错的地方：>= 与 > 差一个像素，
      // 表现是「窗口拖到这个宽度时布局闪一下」。
      expect(Breakpoints.sizeClassOf(900), SizeClass.medium, reason: '900 属于中屏');
      expect(
        Breakpoints.sizeClassOf(1280),
        SizeClass.expanded,
        reason: '1280 属于宽屏',
      );
    });

    test('阈值下方一个像素落到更低一档', () {
      expect(Breakpoints.sizeClassOf(899.99), SizeClass.compact);
      expect(Breakpoints.sizeClassOf(1279.99), SizeClass.medium);
    });

    test('阈值上方一个像素落到更高一档', () {
      expect(Breakpoints.sizeClassOf(900.01), SizeClass.medium);
      expect(Breakpoints.sizeClassOf(1280.01), SizeClass.expanded);
    });

    test('典型宽度落点', () {
      expect(Breakpoints.sizeClassOf(390), SizeClass.compact, reason: '手机宽');
      expect(Breakpoints.sizeClassOf(768), SizeClass.compact, reason: '平板竖屏');
      expect(Breakpoints.sizeClassOf(1024), SizeClass.medium);
      expect(Breakpoints.sizeClassOf(1440), SizeClass.expanded);
      expect(Breakpoints.sizeClassOf(2560), SizeClass.expanded);
    });

    test('退化输入不抛异常，一律当 compact', () {
      // 真实来源：首帧 LayoutBuilder 可能给出 0；异常数据可能给出 NaN/Infinity。
      // 这里宁可当窄屏（最保守的布局），也不要崩或算出未定义档位。
      expect(Breakpoints.sizeClassOf(0), SizeClass.compact);
      expect(Breakpoints.sizeClassOf(-100), SizeClass.compact);
      expect(Breakpoints.sizeClassOf(double.nan), SizeClass.compact);
      expect(Breakpoints.sizeClassOf(double.infinity), SizeClass.compact);
      expect(Breakpoints.sizeClassOf(double.negativeInfinity), SizeClass.compact);
    });
  });

  group('单调性（窗口变宽，档位不降）', () {
    test('从 0 扫到 3000，档位序号单调不减', () {
      int previous = -1;
      for (int w = 0; w <= 3000; w += 1) {
        final int rank = Breakpoints.sizeClassOf(w.toDouble()).index;
        expect(rank, greaterThanOrEqualTo(previous), reason: 'w=$w 处档位回退了');
        previous = rank;
      }
    });

    test('三档都被扫到（防某档永远不可达）', () {
      final Set<SizeClass> seen = <SizeClass>{
        for (int w = 0; w <= 3000; w += 7) Breakpoints.sizeClassOf(w.toDouble()),
      };
      expect(seen, SizeClass.values.toSet());
    });
  });

  group('SizeClassX 语义便利属性', () {
    test('hasSideChat：只有 compact 不能并排', () {
      expect(SizeClass.compact.hasSideChat, isFalse);
      expect(SizeClass.medium.hasSideChat, isTrue);
      expect(SizeClass.expanded.hasSideChat, isTrue);
    });

    test('hasInlineSettings：只有 expanded 能用内联侧板', () {
      // 依据：1280 下若把 rail(232) + 设置(400) + 聊天(380) 并列，
      // 舞台会被压到 ~268 px —— 而舞台是产品核心（设计原则 P1）。
      // 所以设置只在 expanded 走「浮在舞台列右缘」，其余档走全屏浮层。
      expect(SizeClass.compact.hasInlineSettings, isFalse);
      expect(SizeClass.medium.hasInlineSettings, isFalse);
      expect(SizeClass.expanded.hasInlineSettings, isTrue);
    });

    test('isXxx 与枚举值一一对应', () {
      for (final SizeClass sc in SizeClass.values) {
        expect(sc.isCompact, sc == SizeClass.compact);
        expect(sc.isMedium, sc == SizeClass.medium);
        expect(sc.isExpanded, sc == SizeClass.expanded);
      }
    });
  });

  group('登记表与唯一真源一致', () {
    test('registry 里就是 sizeClassOf 用的那两个阈值', () {
      // 防「测试读 registry、实现读字面量」这种双真源漂移。
      expect(Breakpoints.registry['mediumMin'], Breakpoints.mediumMin);
      expect(Breakpoints.registry['expandedMin'], Breakpoints.expandedMin);
      for (final double threshold in Breakpoints.registry.values) {
        expect(
          Breakpoints.sizeClassOf(threshold).index,
          greaterThan(
            Breakpoints.sizeClassOf(threshold - 1).index,
          ),
          reason: '阈值 $threshold 处档位没有跃迁——registry 与实现脱节了',
        );
      }
    });

    test('阈值顺序正确（中屏下限 < 宽屏下限）', () {
      expect(Breakpoints.mediumMin, lessThan(Breakpoints.expandedMin));
    });
  });
}
