/// 背景域的**运行时**回归：DEC-6（当前项跟运行时索引）/
/// 位置垫与「舞台单图轮播」**已删**（2026-10-07，负向断言）/
/// 轮播开关只在库 >= 2 项时出现 / D1（预览可达）/
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
import 'package:live2d_ai_shell/ui/field_row.dart';
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
}) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: BackgroundRuntimeScope(
        index: index,
        current: current,
        hydrating: hydrating,
        child: ThemeSection(
          prefs: prefs,
          onPrefsChanged: onChanged ?? (DisplayPrefs _) {},
          onPickShellImage: () {},
          onClearShellImage: () {},
          onRemoveBackground: (int _) {},
          onRemoveBackgrounds: onRemoveMany,
          onReorderBackground: (int _, int _) {},
          onPreviewBackground: onPreview,
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
    testWidgets('轮播走到第 2 张：设置页的「当前」标记落在运行时那一项上', (
      WidgetTester tester,
    ) async {
      final List<BackgroundItem> items = <BackgroundItem>[
        _img('a', url: _png),
        _img('b', url: _png2),
      ];
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: items,
      );
      // 外壳此刻画的是第 2 项（运行时索引 1）。
      await tester.pumpWidget(_pane(prefs: prefs, index: 1, current: items[1]));
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
    });

    testWidgets('运行时索引换了，「当前」标记跟着换（同一个库、只换索引）', (
      WidgetTester tester,
    ) async {
      final List<BackgroundItem> items = <BackgroundItem>[
        _img('a', url: _png),
        _img('b', url: _png2),
      ];
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: items,
      );

      await tester.pumpWidget(_pane(prefs: prefs, index: 0, current: items[0]));
      await tester.pump();
      expect(
        tester.widget<Text>(find.textContaining('图片 1 · ')).data,
        contains('当前'),
      );

      await tester.pumpWidget(_pane(prefs: prefs, index: 1, current: items[1]));
      await tester.pump();
      expect(
        tester.widget<Text>(find.textContaining('图片 2 · ')).data,
        contains('当前'),
      );
      expect(
        tester.widget<Text>(find.textContaining('图片 1 · ')).data,
        isNot(contains('当前')),
        reason: '索引换到第 2 项之后，第 1 项不该还带着「当前」',
      );
    });
  });

  group('DEC-7a：位置垫与铺法控件已随外观收口删除（2026-10-07）', () {
    testWidgets('四档铺法下都找不到位置垫（它在界面上不存在了）', (
      WidgetTester tester,
    ) async {
      for (final int fit in <int>[
        DisplayPrefs.fitCover,
        DisplayPrefs.fitContain,
        DisplayPrefs.fitStretch,
        DisplayPrefs.fitTile,
      ]) {
        final BackgroundItem item = _img('a', url: _png);
        final DisplayPrefs prefs = const DisplayPrefs().copyWith(
          backgrounds: <BackgroundItem>[item],
        );
        await tester.pumpWidget(_pane(prefs: prefs, current: item));
        await tester.pump();
        expect(
          find.byKey(const ValueKey<String>('bg-align-pad')),
          findsNothing,
          reason: '位置垫（旧 kBackgroundAlignPadKey）已删除（fit=$fit）',
        );
      }
    });
  });

  group('轮播：只剩背景库这一份，且仅库 >= 2 项时出现', () {
    testWidgets('「舞台单图轮播 / 加入轮播 / 清空轮播」都不再出现', (
      WidgetTester tester,
    ) async {
      final BackgroundItem item = _img('a', url: _png);
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[item],
      );
      await tester.pumpWidget(_pane(prefs: prefs, current: item));
      await tester.pump();
      for (final String gone in <String>['舞台单图轮播', '加入轮播', '清空轮播']) {
        expect(find.text(gone), findsNothing, reason: '$gone 已随舞台图收口删除');
      }
    });

    testWidgets('库 1 项：没有「轮播」开关', (WidgetTester tester) async {
      final BackgroundItem item = _img('a', url: _png);
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[item],
      );
      await tester.pumpWidget(_pane(prefs: prefs, current: item));
      await tester.pump();
      expect(
        find.widgetWithText(ToggleField, '轮播'),
        findsNothing,
        reason: '一项没有「下一张」可换，开关不该出现',
      );
    });

    testWidgets('库 2 项：出现「轮播」开关，打开后才有间隔与随机顺序', (
      WidgetTester tester,
    ) async {
      final List<BackgroundItem> items = <BackgroundItem>[
        _img('a', url: _png),
        _img('b', url: _png2),
      ];
      final DisplayPrefs off = const DisplayPrefs().copyWith(
        backgrounds: items,
      );
      await tester.pumpWidget(_pane(prefs: off, current: items[0]));
      await tester.pump();
      expect(find.widgetWithText(ToggleField, '轮播'), findsOneWidget);
      expect(
        find.widgetWithText(SliderField, '轮播间隔'),
        findsNothing,
        reason: '轮播关着时不该有间隔滑杆',
      );

      final DisplayPrefs on = off.copyWith(slideInterval: 30);
      await tester.pumpWidget(_pane(prefs: on, current: items[1]));
      await tester.pump();
      expect(find.widgetWithText(SliderField, '轮播间隔'), findsOneWidget);
      expect(find.widgetWithText(ToggleField, '随机顺序'), findsOneWidget);
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
