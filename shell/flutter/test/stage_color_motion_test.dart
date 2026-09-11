/// 舞台底色的**主题同步**（2026-09-11，前端加强计划 P1-3）。
///
/// # 被修掉的是什么
///
/// 切主题时 `MaterialApp` 用 200 ms 把整套配色插值过去，而舞台底是渲染面
/// （iframe）自己画的、只认一个终色。过去 `_applyPrefs` 一按就下发终色，
/// 于是**界面还在渐变、舞台已经跳到终色**——看起来像「舞台先闪了一下」。
///
/// 现在舞台自己跑一条同样 200 ms 的动画、逐帧把插值色发给渲染面，
/// 同时**兜底那一层**（iframe 未加载完时露出来的 `ColoredBox`）读同一个值。
///
/// # 为什么这里验的是 `ColoredBox` 而不是「发出去的帧」
///
/// `flutter test` 跑在 VM 上，`buildLive2DHost` 走 `_stub.dart`（占位实现，
/// 不回调 `onTransport`），所以**没有 bridge**，发帧那条路在这里是空的。
/// 但两层底**读的是同一个插值值**（`_displayedStageColor`），
/// 所以验兜底层的颜色 = 验插值本身。发帧路径由真机验收（清单 §12）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 挂在树里的舞台 + 一个能从外面换色的把手。
class _Harness extends StatefulWidget {
  const _Harness({required this.onReady});

  final void Function(void Function(String)) onReady;

  @override
  State<_Harness> createState() => _HarnessState();
}

class _HarnessState extends State<_Harness> {
  String color = '#000000';

  @override
  void initState() {
    super.initState();
    widget.onReady((String next) => setState(() => color = next));
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: buildAppTheme(),
    home: Scaffold(body: Live2DStage(stageColor: color)),
  );
}

/// 舞台兜底那一层的颜色（iframe 还没加载完时露出来的就是它）。
Color? backdropColor(WidgetTester tester) {
  final Finder boxes = find.descendant(
    of: find.byType(Live2DStage),
    matching: find.byType(ColoredBox),
  );
  if (boxes.evaluate().isEmpty) return null;
  return tester.widget<ColoredBox>(boxes.first).color;
}

Future<void Function(String)> mountStage(WidgetTester tester) async {
  late void Function(String) setColor;
  await tester.pumpWidget(_Harness(onReady: (f) => setColor = f));
  await tester.pump();
  return setColor;
}

void main() {
  group('P1-3：舞台底跟着主题**插值**，不是一步跳过去', () {
    testWidgets('中途一定有「既不是起点也不是终点」的颜色', (WidgetTester tester) async {
      final void Function(String) setColor = await mountStage(tester);
      expect(backdropColor(tester), const Color(0xFF000000));

      setColor('#ffffff');
      await tester.pump(); // 让 didUpdateWidget 跑起来（动画从 0 开始）
      await tester.pump(AppDurations.base ~/ 2); // 推进一半

      final Color? mid = backdropColor(tester);
      expect(mid, isNotNull);
      expect(
        mid,
        isNot(const Color(0xFF000000)),
        reason: '半程时还停在起点 —— 说明根本没在插值',
      );
      expect(
        mid,
        isNot(const Color(0xFFFFFFFF)),
        reason: '半程时已经到终点 —— 那还是「一步跳过去」，只是晚了一帧',
      );
    });

    testWidgets('动画跑完落在终色上（不许差一档）', (WidgetTester tester) async {
      final void Function(String) setColor = await mountStage(tester);
      setColor('#ffffff');
      await tester.pumpAndSettle();
      expect(backdropColor(tester), const Color(0xFFFFFFFF));
    });

    testWidgets('**首帧不播**底色动画（页面第一次出现不该有一段渐变）', (
      WidgetTester tester,
    ) async {
      await mountStage(tester);
      // 首帧就该是终色本身，不是某个中间值。
      expect(backdropColor(tester), const Color(0xFF000000));
    });

    testWidgets('连续切两次主题时从**当前显示值**续接（不跳回起点）', (
      WidgetTester tester,
    ) async {
      final void Function(String) setColor = await mountStage(tester);
      setColor('#ffffff');
      await tester.pump();
      await tester.pump(AppDurations.base ~/ 2);

      final Color? before = backdropColor(tester);
      expect(before, isNotNull);

      // 半路上改主意，切到另一个色。
      setColor('#000080');
      await tester.pump();

      final Color? justAfter = backdropColor(tester);
      // 续接 = 起手那一帧几乎等于刚才显示的色，而不是回到 #000000。
      expect(
        justAfter,
        isNot(const Color(0xFF000000)),
        reason: '第二次切换从起点重来了 —— 底色会往回跳一下',
      );
    });
  });

  group('P1-3：舞台底色串的解析（渲染面严格校验这个格式）', () {
    test('#rrggbb 解析正确', () {
      expect(parseStageColorCss('#123456'), const Color(0xFF123456));
      expect(parseStageColorCss('  #AABBCC '), const Color(0xFFAABBCC));
    });

    test('#rgb 展开正确', () {
      expect(parseStageColorCss('#abc'), const Color(0xFFAABBCC));
    });

    test('认不出来的一律返回 null（宁可不发，也不发一个会被静默拒掉的串）', () {
      for (final String? bad in <String?>[
        null,
        '',
        '123456', // 没有 #
        '#12345', // 位数不对
        '#1234567',
        '#gggggg', // 不是十六进制
        'rgb(1,2,3)',
        '#123456;background:url(x)', // 注入尝试
      ]) {
        expect(parseStageColorCss(bad), isNull, reason: '不该接受：$bad');
      }
    });

    test('与 stageColorCss 往返一致（拼法与解析法不能各写一套）', () {
      for (final AppPalette palette in AppPalette.registry.values) {
        expect(parseStageColorCss(palette.stageCss), palette.stage);
      }
    });
  });
}
