/// 背景域的**运行时**回归：DEC-6（当前项）/ DEC-7a（位置显隐）/
/// DEC-2（两套轮播互斥 + stagePlaylist 守护）/ D1（预览可达）/
/// DEC-5 的 UI 侧（坏图要说话）/ 交接项 9b（水合中）。
///
/// # 为什么单独一个文件
///
/// 这些断言全都依赖**外壳注入的运行时上下文**（BackgroundRuntimeScope）——
/// 它们测的是「设置页说的是不是画面正在做的那一件事」。
/// 单独放一处，是因为 appearance_background_library_test.dart 管的是
/// 库面板自己的交互（勾选 / 预览 / 图案），职责不同。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/sections/appearance_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 1×1 真 PNG 的 dataURL（会真的过 Image.memory 与 decodeDataUrlBytes）。
const String _png =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

const String _png2 =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==';

BackgroundImage _img(
  String id, {
  String? url,
  int? fit,
  int? align,
  double? opacity,
}) => BackgroundImage(
  id: id,
  dataUrl: url,
  fit: fit,
  align: align,
  opacity: opacity,
);

Widget _pane({
  required DisplayPrefs prefs,
  required BackgroundItem? current,
  int index = 0,
  bool hydrating = false,
  ValueChanged<DisplayPrefs>? onChanged,
  ValueChanged<int>? onPreview,
  ValueChanged<List<int>>? onRemoveMany,
  VoidCallback? onAddToPlaylist,
  VoidCallback? onClearPlaylist,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: BackgroundRuntimeScope(
        index: index,
        current: current,
        hydrating: hydrating,
        child: AppearanceSection(
          prefs: prefs,
          onPrefsChanged: onChanged ?? (DisplayPrefs _) {},
          onPickShellImage: () {},
          onClearShellImage: () {},
          onRemoveBackground: (int _) {},
          onRemoveBackgrounds: onRemoveMany,
          onReorderBackground: (int _, int _) {},
          onPreviewBackground: onPreview,
          onAddPattern: (int _) {},
          onAddToPlaylist: onAddToPlaylist,
          onClearPlaylist: onClearPlaylist,
        ),
      ),
    ),
  ),
);

/// 文案断言：EmphasizedText 走的是 Text.rich，所以比对的是**纯文本**
/// （强调标记 `**` 已经被它当分隔符吃掉了）。
Finder _rich(String needle) => find.textContaining(needle, findRichText: true);

void main() {
  group('DEC-6：当前项取**运行时索引**，不是「永远第 0 项」', () {
    testWidgets('轮播走到第 2 张：设置页「当前」标记与铺法 / 位置区显示的是第 2 张的属性', (
      WidgetTester tester,
    ) async {
      final List<BackgroundItem> items = <BackgroundItem>[
        _img('a', url: _png, fit: DisplayPrefs.fitCover),
        _img('b', url: _png2, fit: DisplayPrefs.fitContain),
      ];
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: items,
        imageFit: DisplayPrefs.fitCover,
      );
      // 外壳此刻画的是第 2 项（运行时索引 1）。
      await tester.pumpWidget(
        _pane(prefs: prefs, index: 1, current: items[1]),
      );
      await tester.pump();

      expect(find.textContaining('图片 2 · '), findsOneWidget);
      expect(
        find.textContaining('· 当前'),
        findsOneWidget,
        reason: '「当前」标记只能有一个，且必须落在运行时那一项上',
      );
      expect(
        tester.widget<Text>(find.textContaining('图片 2 · ')).data,
        contains('当前'),
        reason: '第 2 张才是当前项（旧判据看的是 effectiveBackground = 第 0 项）',
      );

      // 铺法 / 位置区读的也必须是第 2 张：它 contain ⇒ 位置垫出现。
      // 第 1 张是 cover ⇒ 若还看第 0 项，这里会**找不到位置垫**（这条就是旧形态）。
      expect(
        find.byKey(kBackgroundAlignPadKey),
        findsOneWidget,
        reason:
            '第 2 张的生效铺法是 contain ⇒ 位置垫必须出现；'
            '看第 0 项（cover）时它不会出现',
      );
      expect(
        _rich('当前这一项单独设过铺法'),
        findsOneWidget,
        reason: '第 2 张单独设过铺法：全局选择器要禁用并说清去哪儿改',
      );
    });

    testWidgets('运行时索引变了，显隐跟着变（同一个库、只换索引）', (
      WidgetTester tester,
    ) async {
      final List<BackgroundItem> items = <BackgroundItem>[
        _img('a', url: _png, fit: DisplayPrefs.fitCover),
        _img('b', url: _png2, fit: DisplayPrefs.fitContain),
      ];
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: items,
      );
      await tester.pumpWidget(
        _pane(prefs: prefs, index: 1, current: items[1]),
      );
      await tester.pump();
      expect(find.byKey(kBackgroundAlignPadKey), findsOneWidget);

      await tester.pumpWidget(
        _pane(prefs: prefs, index: 0, current: items[0]),
      );
      await tester.pump();
      expect(
        find.byKey(kBackgroundAlignPadKey),
        findsNothing,
        reason: '索引回到第 1 项（cover）之后位置垫必须收起',
      );
    });
  });

  group('DEC-7a：位置区只在「完整」铺法下显示（条件与注释原来正好相反）', () {
    testWidgets('四档逐档断言：只有 contain 摆位置垫', (WidgetTester tester) async {
      const Map<int, bool> expectPad = <int, bool>{
        DisplayPrefs.fitCover: false,
        DisplayPrefs.fitContain: true,
        DisplayPrefs.fitStretch: false,
        DisplayPrefs.fitTile: false,
      };
      for (final MapEntry<int, bool> e in expectPad.entries) {
        final BackgroundItem item = _img('a', url: _png);
        final DisplayPrefs prefs = const DisplayPrefs().copyWith(
          backgrounds: <BackgroundItem>[item],
          imageFit: e.key,
        );
        await tester.pumpWidget(_pane(prefs: prefs, current: item));
        await tester.pump();
        expect(
          find.byKey(kBackgroundAlignPadKey),
          e.value ? findsOneWidget : findsNothing,
          reason:
              '${DisplayPrefs.fitName(e.key)} 档下位置垫的显隐错了：'
              'cover / stretch / tile 都把整块铺满，对齐没有可见效果',
        );
        expect(
          find.text('位置'),
          e.value ? findsOneWidget : findsNothing,
          reason: '位置那一行的小标题要跟着垫一起显隐（不能留个空标题）',
        );
      }
    });
  });

  group('DEC-2 / DEC-7b：两个轮播块按来源互斥', () {
    testWidgets('来源 = 舞台那张：只有「舞台单图轮播」，壳背景轮播块不出现', (
      WidgetTester tester,
    ) async {
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        stageImage: _png,
        backgroundSource: DisplayPrefs.backgroundSourceStageImage,
        backgrounds: <BackgroundItem>[_img('a', url: _png)],
        slideInterval: 30,
      );
      await tester.pumpWidget(
        _pane(prefs: prefs, current: prefs.effectiveBackground),
      );
      await tester.pump();

      expect(find.text('舞台单图轮播'), findsOneWidget);
      expect(
        find.text('壳背景轮播'),
        findsNothing,
        reason:
            '来源 = 舞台那张时它转的是背景库（此时不参与渲染）⇒ 必须收起，'
            '不能「控件转、画面不转」',
      );
      expect(
        find.text('轮播间隔'),
        findsNothing,
        reason: '空转的间隔滑杆必须一起收起（旧形态：控件转、画面不转）',
      );
      expect(
        _rich('壳背景轮播转的是'),
        findsOneWidget,
        reason: 'P4：不能静默消失，要有一句说明去哪儿找舞台那张的轮播',
      );
    });

    testWidgets('来源 = 背景库：只有「壳背景轮播」，舞台块不出现', (
      WidgetTester tester,
    ) async {
      final BackgroundItem item = _img('a', url: _png);
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        stageImage: _png,
        backgrounds: <BackgroundItem>[item],
        stagePlaylist: <String>[_png],
      );
      await tester.pumpWidget(_pane(prefs: prefs, current: item));
      await tester.pump();

      expect(find.text('壳背景轮播'), findsOneWidget);
      expect(
        find.text('舞台单图轮播'),
        findsNothing,
        reason: '来源 = 背景库时舞台那套列表与通道都不参与渲染',
      );
      expect(find.text('加入轮播'), findsNothing);
    });

    testWidgets('两者**从不同时**出现（同一份 prefs 只可能满足一边）', (
      WidgetTester tester,
    ) async {
      for (final int source in <int>[
        DisplayPrefs.backgroundSourceLibrary,
        DisplayPrefs.backgroundSourceStageImage,
      ]) {
        final DisplayPrefs prefs = const DisplayPrefs().copyWith(
          stageImage: _png,
          backgroundSource: source,
          backgrounds: <BackgroundItem>[_img('a', url: _png)],
          stagePlaylist: <String>[_png],
        );
        await tester.pumpWidget(
          _pane(prefs: prefs, current: prefs.effectiveBackground),
        );
        await tester.pump();
        final int shell = find.text('壳背景轮播').evaluate().length;
        final int stage = find.text('舞台单图轮播').evaluate().length;
        expect(
          shell + stage,
          1,
          reason:
              '来源 $source 下两个轮播块必须**恰好出现一个**'
              '（同时出现就是 rc.5 §9.2 那个重叠）',
        );
      }
    });

    testWidgets('stagePlaylist 守护网：编辑器的「当前」标记与三种编辑都上报新列表', (
      WidgetTester tester,
    ) async {
      // 这一份列表在 rc.5 里是**零测试覆盖**的（DEC-2：保留它就必须先补守护测试）。
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        stageImage: _png2,
        backgroundSource: DisplayPrefs.backgroundSourceStageImage,
        stagePlaylist: <String>[_png, _png2],
      );
      final List<DisplayPrefs> seen = <DisplayPrefs>[];
      await tester.pumpWidget(
        _pane(
          prefs: prefs,
          current: prefs.effectiveBackground,
          onChanged: seen.add,
        ),
      );
      await tester.pump();

      // 当前那张是 playlist[1] ⇒ 「第 2 张」带「当前」，第 1 张不带。
      expect(find.textContaining('第 1 张'), findsOneWidget);
      expect(find.textContaining('第 2 张'), findsOneWidget);
      expect(
        find.textContaining('· 当前'),
        findsOneWidget,
        reason: '「当前」标记按 stagePlaylistIndexOf 算，落在舞台那张上',
      );
      expect(
        find.textContaining('第 2 张').evaluate().single.widget,
        isA<Text>(),
      );
      expect(
        (tester.widget<Text>(find.textContaining('第 2 张'))).data,
        contains('当前'),
      );

      // 上移第 2 张 → 列表交换顺序。
      await tester.ensureVisible(find.text('上移').last);
      await tester.pump();
      await tester.tap(find.text('上移').last);
      await tester.pump();
      expect(seen, hasLength(1));
      expect(seen.single.stagePlaylist, <String>[_png2, _png]);
      seen.clear();

      // 删除第 1 张 → 只删它。
      await tester.ensureVisible(find.text('删除').first);
      await tester.pump();
      await tester.tap(find.text('删除').first);
      await tester.pump();
      expect(seen, hasLength(1));
      expect(seen.single.stagePlaylist, <String>[_png2]);
      seen.clear();

      // 加入轮播 / 清空轮播：两条手动维护入口都要真的接线。
      final List<int> addCalls = <int>[];
      final List<String> clearCalls = <String>[];
      await tester.pumpWidget(
        _pane(
          prefs: prefs,
          current: prefs.effectiveBackground,
          onChanged: seen.add,
          onAddToPlaylist: () => addCalls.add(1),
          onClearPlaylist: () => clearCalls.add('clear'),
        ),
      );
      await tester.pump();
      await tester.ensureVisible(find.text('加入轮播'));
      await tester.pump();
      await tester.tap(find.text('加入轮播'));
      await tester.pump();
      expect(addCalls, hasLength(1));
      await tester.ensureVisible(find.text('清空轮播'));
      await tester.pump();
      await tester.tap(find.text('清空轮播'));
      await tester.pump();
      expect(clearCalls, hasLength(1));
    });
  });

  group('D1：预览在任意库大小下都可达', () {
    for (final int count in <int>[1, 2, 8]) {
      testWidgets('库 $count 项：默认点一行 = 预览（不是勾选）', (
        WidgetTester tester,
      ) async {
        final List<BackgroundItem> items = <BackgroundItem>[
          for (int i = 0; i < count; i++) _img('i$i', url: _png),
        ];
        final DisplayPrefs prefs = const DisplayPrefs().copyWith(
          backgrounds: items,
        );
        final List<int> previewed = <int>[];
        await tester.pumpWidget(
          _pane(prefs: prefs, current: items.first, onPreview: previewed.add),
        );
        await tester.pump();

        expect(
          find.byType(Checkbox),
          findsNothing,
          reason: '默认是预览模式（管理要显式点「管理」）——D1',
        );
        await tester.ensureVisible(find.textContaining('图片 1 · '));
        await tester.pump();
        await tester.tap(find.textContaining('图片 1 · '));
        await tester.pump();
        expect(
          previewed,
          <int>[0],
          reason: '库 $count 项时预览都必须可达；旧判据在 ≥2 项时让它永远不可达',
        );
      });
    }
  });

  group('DEC-5 的 UI 侧：坏图与未读回的字节都要说话', () {
    testWidgets('字节形态坏：界面上明说画不出来', (WidgetTester tester) async {
      final BackgroundItem bad = BackgroundImage(
        id: 'bad',
        dataUrl: 'x-not-a-data-url',
      );
      await tester.pumpWidget(
        _pane(
          prefs: const DisplayPrefs().copyWith(
            backgrounds: <BackgroundItem>[bad],
          ),
          current: bad,
        ),
      );
      await tester.pump();
      expect(
        _rich('画不出来'),
        findsOneWidget,
        reason: '坏图不许静默给个空块（DEC-5：isRenderable 收紧之后 UI 要跟上）',
      );
    });

    testWidgets('字节还没读回来：说「还在读回」，不是「坏了」', (
      WidgetTester tester,
    ) async {
      final BackgroundItem pending = _img('p', url: null);
      await tester.pumpWidget(
        _pane(
          prefs: const DisplayPrefs().copyWith(
            backgrounds: <BackgroundItem>[pending],
          ),
          current: pending,
        ),
      );
      await tester.pump();
      expect(_rich('还在读回'), findsOneWidget);
      expect(_rich('画不出来'), findsNothing);
    });
  });

  group('交接项 9b：水合中', () {
    testWidgets('说明 + 背景库写操作禁用（不做一个按了没反应的按钮）', (
      WidgetTester tester,
    ) async {
      final BackgroundItem item = _img('a', url: _png);
      await tester.pumpWidget(
        _pane(
          prefs: const DisplayPrefs().copyWith(
            backgrounds: <BackgroundItem>[item],
          ),
          current: item,
          hydrating: true,
        ),
      );
      await tester.pump();
      expect(_rich('正在读回背景库'), findsOneWidget);
      final FilledButton add = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, '添加图片'),
      );
      expect(add.onPressed, isNull, reason: '水合中禁用写操作（它会与读回结果打架）');
      final TextButton clear = tester.widget<TextButton>(
        find.widgetWithText(TextButton, '清空背景库'),
      );
      expect(clear.onPressed, isNull);
    });

    testWidgets('水合结束（hydrating = false）→ 说明消失、按钮可用', (
      WidgetTester tester,
    ) async {
      final BackgroundItem item = _img('a', url: _png);
      await tester.pumpWidget(
        _pane(
          prefs: const DisplayPrefs().copyWith(
            backgrounds: <BackgroundItem>[item],
          ),
          current: item,
          hydrating: false,
        ),
      );
      await tester.pump();
      expect(_rich('正在读回背景库'), findsNothing);
      final FilledButton add = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, '添加图片'),
      );
      expect(add.onPressed, isNotNull);
    });
  });
}
