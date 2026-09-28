/// 背景域的**样式面**回归：四档铺法 + tileSize（§5.3 第 1 条）、
/// 逐图样式覆盖（§5.3 第 2 条 / DEC-5 的 UI 侧）、
/// 滑杆「停手才落盘」（交接项 9a）、批量删除按**身份**（F-0003-3）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/data/background_reorder.dart';
import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/sections/appearance_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

const String _png =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

const String _png2 =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==';

const String _png3 =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==';

BackgroundImage _img(String id, {String? url, int? fit, int? align, double? opacity}) =>
    BackgroundImage(id: id, dataUrl: url, fit: fit, align: align, opacity: opacity);

/// 一个**真的回灌偏好**的宿主：onPrefsChanged 之后 prefs 变成新值。
///
/// 为什么要它：9a 的验收是「落盘值 == 滑杆终值 == 界面读数」——而滑杆显示的是
/// 「宿主值，否则草稿」；宿主不回灌的话，这个等式在测试里永远看不到。
class _Host extends StatefulWidget {
  const _Host({
    required this.initial,
    this.current,
    this.onPrefs,
    this.onRemoveMany,
  });

  final DisplayPrefs initial;
  final BackgroundItem? current;
  final ValueChanged<DisplayPrefs>? onPrefs;
  final void Function(List<int> indices)? onRemoveMany;

  @override
  State<_Host> createState() => _HostState();
}

class _HostState extends State<_Host> {
  late DisplayPrefs prefs = widget.initial;

  @override
  void didUpdateWidget(_Host oldWidget) {
    super.didUpdateWidget(oldWidget);
    // 宿主（测试）换了一份偏好（例如模拟拖动排序）⇒ 跟着换。
    if (widget.initial != oldWidget.initial) prefs = widget.initial;
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: buildAppTheme(),
    home: Scaffold(
      body: SingleChildScrollView(
        child: BackgroundRuntimeScope(
          index: 0,
          current: widget.current,
          hydrating: false,
          child: AppearanceSection(
            prefs: prefs,
            onPrefsChanged: (DisplayPrefs next) {
              setState(() => prefs = next);
              widget.onPrefs?.call(next);
            },
            onPickShellImage: () {},
            onClearShellImage: () {},
            onRemoveBackground: (int _) {},
            onRemoveBackgrounds: widget.onRemoveMany,
            onReorderBackground: (int _, int _) {},
            onPreviewBackground: (int _) {},
            onAddPattern: (int _) {},
          ),
        ),
      ),
    ),
  );
}

Finder _rich(String needle) => find.textContaining(needle, findRichText: true);

/// 把测试视口拉高。
///
/// 外观分区整块比默认的 800x600 测试面高，控件会落在视口外——那样
/// `tap()` 会「点不中」而失败。真实用户是滚动去看的，而这里要断的是**接线**，
/// 所以直接把视口拉长（比在每个用例里 ensureVisible 更不容易漏）。
void _tall(WidgetTester tester) {
  tester.view.physicalSize = const Size(1000, 2600);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
}

/// 某个滑杆字段里的 Slider。
Finder _sliderOf(String label) => find.descendant(
  of: find.ancestor(of: find.text(label), matching: find.byType(SliderField)),
  matching: find.byType(Slider),
);

void main() {
  group('§5.3 第 1 条：四档铺法 + tileSize', () {
    testWidgets('选到「平铺」→ 出现贴片滑杆，且值 = prefs.tileSize', (
      WidgetTester tester,
    ) async {
      _tall(tester);
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[_img('a', url: _png)],
        imageFit: DisplayPrefs.fitTile,
        tileSize: 96,
      );
      await tester.pumpWidget(
        _Host(initial: prefs, current: prefs.effectiveBackground),
      );
      await tester.pump();

      expect(find.text('平铺贴片'), findsOneWidget);
      expect(
        tester.widget<Slider>(_sliderOf('平铺贴片')).value,
        96.0,
        reason: '滑杆一开始就要显示 prefs.tileSize，不能是别的默认值',
      );
      expect(find.byKey(kBackgroundAlignPadKey), findsNothing);
    });

    testWidgets('其余三档都不出现贴片滑杆', (WidgetTester tester) async {
      for (final int fit in <int>[
        DisplayPrefs.fitCover,
        DisplayPrefs.fitContain,
        DisplayPrefs.fitStretch,
      ]) {
        final DisplayPrefs prefs = const DisplayPrefs().copyWith(
          backgrounds: <BackgroundItem>[_img('a', url: _png)],
          imageFit: fit,
        );
        await tester.pumpWidget(
          _Host(initial: prefs, current: prefs.effectiveBackground),
        );
        await tester.pump();
        expect(
          find.text('平铺贴片'),
          findsNothing,
          reason:
              '${DisplayPrefs.fitName(fit)} 档不该出现贴片滑杆（它只对 tile 有效）',
        );
      }
    });

    testWidgets('逐档点一遍：每档都真的能切过去（四档全可达）', (
      WidgetTester tester,
    ) async {
      _tall(tester);
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[_img('a', url: _png)],
      );
      final List<DisplayPrefs> seen = <DisplayPrefs>[];
      await tester.pumpWidget(
        _Host(
          initial: prefs,
          current: prefs.effectiveBackground,
          onPrefs: seen.add,
        ),
      );
      await tester.pump();
      for (final MapEntry<int, String> e in const <int, String>{
        DisplayPrefs.fitContain: '完整',
        DisplayPrefs.fitStretch: '拉伸',
        DisplayPrefs.fitTile: '平铺',
        DisplayPrefs.fitCover: '铺满',
      }.entries) {
        await tester.tap(find.text(e.value));
        await tester.pump();
        expect(seen.last.imageFit, e.key, reason: '点这一档必须上报它自己的值');
      }
    });
  });

  group('§5.3 第 2 条 / DEC-5：逐图样式覆盖', () {
    testWidgets('「跟随全局」是显式状态：设过 → 单独设置；清掉 → 回落全局', (
      WidgetTester tester,
    ) async {
      _tall(tester);
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[_img('a', url: _png)],
        imageFit: DisplayPrefs.fitCover,
        imageAlign: 4,
      );
      final List<DisplayPrefs> seen = <DisplayPrefs>[];
      await tester.pumpWidget(
        _Host(
          initial: prefs,
          current: prefs.effectiveBackground,
          onPrefs: seen.add,
        ),
      );
      await tester.pump();

      // 打开这一行的「样式」折叠区。
      await tester.tap(find.text('样式'));
      await tester.pump();
      expect(
        _rich('铺法：跟随全局（cover）'),
        findsOneWidget,
        reason: '没设过就用**文字**说「跟随全局」，不是拿数字冒充',
      );
      expect(_rich('位置：跟随全局'), findsOneWidget);
      expect(_rich('不透明度：跟随全局'), findsOneWidget);

      // 「单独设置」→ 拿全局值写成一个显式覆盖。
      await tester.tap(find.byKey(const ValueKey<String>('bg-style-set-fit')));
      await tester.pump();
      final BackgroundImage afterSet =
          seen.last.backgrounds.single as BackgroundImage;
      expect(afterSet.fit, DisplayPrefs.fitCover, reason: '显式覆盖 = 当前的全局值');
      expect(afterSet.dataUrl, _png, reason: 'dataUrl 必须原样带走');
      expect(
        _rich('铺法：单独设置'),
        findsOneWidget,
        reason: '设过之后状态要变成「单独设置」',
      );

      // 全局改成拉伸：这一项**不受影响**（它有自己的值）。
      expect(
        DisplayPrefs.effectiveImageFit(afterSet, DisplayPrefs.fitStretch),
        DisplayPrefs.fitCover,
        reason: '设了逐图值之后全局改动不影响该项',
      );

      // 「改回跟随全局」→ fit 回到 null。
      await tester.tap(find.byKey(const ValueKey<String>('bg-style-clear-fit')));
      await tester.pump();
      final BackgroundImage afterClear =
          seen.last.backgrounds.single as BackgroundImage;
      expect(afterClear.fit, isNull);
      expect(afterClear.dataUrl, _png);
      expect(
        DisplayPrefs.effectiveImageFit(afterClear, DisplayPrefs.fitStretch),
        DisplayPrefs.fitStretch,
        reason: '清掉逐图值之后回落全局',
      );
      expect(_rich('铺法：跟随全局（cover）'), findsOneWidget);
    });

    testWidgets('清一项不影响另外两项（只清一项是逐图编辑器的硬要求）', (
      WidgetTester tester,
    ) async {
      _tall(tester);
      final BackgroundImage item = _img(
        'a',
        url: _png,
        opacity: 0.33,
        fit: DisplayPrefs.fitTile,
        align: 8,
      );
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[item],
      );
      final List<DisplayPrefs> seen = <DisplayPrefs>[];
      await tester.pumpWidget(
        _Host(initial: prefs, current: item, onPrefs: seen.add),
      );
      await tester.pump();
      await tester.tap(find.text('样式'));
      await tester.pump();
      expect(_rich('铺法：单独设置'), findsOneWidget);
      expect(_rich('位置：单独设置'), findsOneWidget);
      expect(_rich('不透明度：单独设置'), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey<String>('bg-style-clear-fit')));
      await tester.pump();
      final BackgroundImage next =
          seen.last.backgrounds.single as BackgroundImage;
      expect(next.fit, isNull, reason: '这一项被清掉了');
      expect(next.opacity, 0.33, reason: '另外两项必须原样');
      expect(next.align, 8, reason: '另外两项必须原样');
    });

    test('逐图值参与「哪一项算有背景」的判据（全局 0 但这一项覆盖 > 0 照画）', () {
      final BackgroundImage item = _img('a', url: _png, opacity: 0.2);
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[item],
        backgroundOpacity: 0,
      );
      expect(
        prefs.hasBackgroundAt(item),
        isTrue,
        reason: '全局 0 而这一项显式覆盖成 0.2 ⇒ 照画（用户明确指令优先）',
      );
    });
  });

  group('交接项 9a：滑杆「拖动只动草稿、停手才落盘」', () {
    testWidgets('一次拖动**只提交一次**；拖动中读数跟手；提交值 == 界面读数', (
      WidgetTester tester,
    ) async {
      _tall(tester);
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[_img('a', url: _png)],
        // 不从 100% 起：那样往右拖不动（已经到顶），读数自然不变。
        backgroundOpacity: 0.5,
      );
      final List<DisplayPrefs> commits = <DisplayPrefs>[];
      await tester.pumpWidget(
        _Host(
          initial: prefs,
          current: prefs.effectiveBackground,
          onPrefs: commits.add,
        ),
      );
      await tester.pump();

      final Finder slider = _sliderOf('不透明度（图）');
      expect(slider, findsOneWidget);
      final double before = tester.widget<Slider>(slider).value;

      final TestGesture gesture = await tester.startGesture(
        tester.getCenter(slider),
      );
      await gesture.moveBy(const Offset(40, 0));
      await tester.pump();
      final double dragging = tester.widget<Slider>(slider).value;
      expect(dragging, isNot(before), reason: '拖动中读数要跟手');
      expect(
        commits,
        isEmpty,
        reason: '拖动中**一次都不许落盘**——旧形态是每帧一次整份 jsonEncode',
      );

      // 再拖几帧（模拟一次连续拖动），仍然一次都不提交。
      await gesture.moveBy(const Offset(20, 0));
      await tester.pump();
      await gesture.moveBy(const Offset(-10, 0));
      await tester.pump();
      final double settled = tester.widget<Slider>(slider).value;
      expect(commits, isEmpty);

      await gesture.up();
      await tester.pump();
      expect(
        commits,
        isEmpty,
        reason: '松手那一刻还在防抖窗口里（AppDurations.base）',
      );

      await tester.pump(AppDurations.base);
      expect(
        commits,
        hasLength(1),
        reason: '一次连续拖动只能提交一次（这就是 9a 的验收）',
      );
      expect(
        commits.single.backgroundOpacity,
        settled,
        reason: '落盘值必须 == 滑杆终值',
      );
      expect(
        tester.widget<Slider>(slider).value,
        settled,
        reason: '宿主回灌之后界面读数 == 落盘值',
      );
    });

    testWidgets('本地偏好型滑杆都走同一条约定（不许漏一个）', (
      WidgetTester tester,
    ) async {
      _tall(tester);
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[_img('a', url: _png)],
        imageFit: DisplayPrefs.fitTile,
        slideInterval: 30,
      );
      await tester.pumpWidget(
        _Host(initial: prefs, current: prefs.effectiveBackground),
      );
      await tester.pump();
      // 外观区里这几个「本地偏好型」滑杆的落点都是 _updatePrefs
      // （整份 jsonEncode + setItem + 下发渲染面），必须都包在提交层里。
      for (final String label in <String>[
        '描边强度',
        '不透明度（图）',
        '透明程度（界面）',
        '平铺贴片',
        '轮播间隔',
      ]) {
        final Finder field = find.ancestor(
          of: find.text(label),
          matching: find.byType(SliderField),
        );
        expect(field, findsOneWidget, reason: '找不到滑杆');
        expect(
          find.descendant(of: field, matching: find.byType(Slider)),
          findsOneWidget,
        );
      }
      // 模糊折在「更多外观」里：展开之后同样要在。
      await tester.tap(find.text('更多外观'));
      await tester.pump();
      expect(
        find.ancestor(
          of: find.text('模糊（图）'),
          matching: find.byType(SliderField),
        ),
        findsOneWidget,
      );
    });
  });

  group('F-0003-3：批量删除按**身份**，重排之后不删错', () {
    testWidgets('勾第 1 张 → 原第 3 张拖到第 1 位 → 「删除所选」删的仍是当初那张', (
      WidgetTester tester,
    ) async {
      _tall(tester);
      final List<BackgroundItem> order0 = <BackgroundItem>[
        _img('a', url: _png),
        _img('b', url: _png2),
        _img('c', url: _png3),
      ];
      List<BackgroundItem> current = order0;
      final List<int> removedIndices = <int>[];
      final List<String> removedIds = <String>[];
      Future<void> pump(DisplayPrefs p) async {
        await tester.pumpWidget(
          _Host(
            initial: p,
            current: p.effectiveBackground,
            onRemoveMany: (List<int> idx) {
              removedIndices.addAll(idx);
              for (final int i in idx) {
                removedIds.add((current[i] as BackgroundImage).id);
              }
            },
          ),
        );
        await tester.pump();
      }

      await pump(const DisplayPrefs().copyWith(backgrounds: order0));
      await tester.tap(find.byKey(kBackgroundManageToggleKey));
      await tester.pump();
      await tester.tap(find.textContaining('图片 1 · '));
      await tester.pump();
      expect(find.text('已选 1 项'), findsOneWidget);

      // 宿主侧拖动排序：把原第 3 项（c）拖到第 1 位。
      current = reorderBackgroundItems(order0, 2, 0);
      expect(
        <String>[
          for (final BackgroundItem b in current) (b as BackgroundImage).id,
        ],
        <String>['c', 'a', 'b'],
      );
      await pump(const DisplayPrefs().copyWith(backgrounds: current));
      // 重排之后**勾选跟着项走**：勾选标记落在 a（现在第 2 位）。
      expect(
        find.text('已选 1 项'),
        findsOneWidget,
        reason: '重排不该丢掉勾选（身份跟着项）',
      );

      await tester.ensureVisible(find.text('删除所选'));
      await tester.pump();
      await tester.tap(find.text('删除所选'));
      await tester.pump();

      expect(
        removedIds,
        <String>['a'],
        reason: '删的必须是用户当初勾选的那张（旧实现删的是「下标 0 现在那一项」= c）',
      );
      expect(
        removedIndices,
        <int>[1],
        reason: '它现在落在下标 1（身份 → 现在的下标）',
      );
    });
  });
}
