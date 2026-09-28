/// Stage B · B-a（模型 / 渲染）的背景回归：**只装本轮新增或改语义的那些条目**。
///
/// # 为什么不塞进 `display_prefs_test.dart`
///
/// 那份已经 **882 行**（超过「测试文件 ≤800 行」的上限，是既有状态）。本轮
/// 新增的回归再塞进去只会把它推到 950+，C3 拆分时更难动。
///
/// # 这里守什么
///
/// 1. **旧档的档位 2 / 3**：`maxImageFit` 从 1 放开到 3 之后，存量里出现的
///    `imageFit: 2/3` 由「坏值回落 0」变成**合法档**——迁移语义要能被回归钉住；
/// 2. **DEC-1**：`slideInterval` 由「越界回落 0（= 静默关掉轮播）」改成
///    **端点夹持**（`0` 仍＝关，其余夹进 5–300）；
/// 3. **DEC-5**：`isRenderable` 从「非空串」收紧到真 dataURL 形态——
///    坏图不再让界面说「有背景」。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/shell_backdrop.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

void main() {
  group('fit 扩四档：旧档里的 2 / 3 由「坏值」变成合法档', () {
    test('存量 imageFit = 2（拉伸）原样读回，不再回落 0', () {
      expect(
        DisplayPrefs.fromJson(<String, Object?>{'imageFit': 2}).imageFit,
        2,
        reason: '2 = stretch 现在是合法档；回落 0 会把用户的「拉伸」静默改成「铺满」',
      );
    });

    test('存量 imageFit = 3（平铺）原样读回，不再回落 0', () {
      expect(
        DisplayPrefs.fromJson(<String, Object?>{'imageFit': 3}).imageFit,
        3,
        reason: '3 = tile 现在是合法档',
      );
    });

    test('上界是 3：4 / 42 / -1 / 非数字仍然回落默认 0', () {
      for (final Object? bad in <Object?>[4, 42, -1, 'stretch', null]) {
        expect(
          DisplayPrefs.fromJson(<String, Object?>{'imageFit': bad}).imageFit,
          DisplayPrefs.defaultImageFit,
          reason: '$bad 在 [0, 3] 之外（或不是 int），必须回落默认',
        );
      }
      expect(DisplayPrefs.maxImageFit, 3);
      expect(DisplayPrefs.defaultImageFit, 0);
      // _readInt 对 num 的**截断**是既有语义（所有 int 字段共用），本轮不动：
      // 3.5 落到 3，不是回落 0——那是「宽容读入」，与本轮的迁移语义无关。
      expect(
        DisplayPrefs.fromJson(<String, Object?>{'imageFit': 3.5}).imageFit,
        3,
      );
    });
  });

  group('DEC-1：slideInterval 改成端点夹持（0 仍＝关）', () {
    test('夹持对照表：0→0 / 1→5 / 5→5 / 300→300 / 301→300 / 3600→300 / 99999→300',
        () {
      const Map<int, int> table = <int, int>{
        0: 0,
        1: 5,
        5: 5,
        300: 300,
        301: 300,
        3600: 300,
        99999: 300,
      };
      table.forEach((int from, int to) {
        expect(
          DisplayPrefs.fromJson(<String, Object?>{'slideInterval': from})
              .slideInterval,
          to,
          reason: '存 $from 应夹到 $to',
        );
      });
    });

    test('0 是唯一的「关」：不能被夹到滑杆下限 5', () {
      expect(
        DisplayPrefs.fromJson(<String, Object?>{'slideInterval': 0})
            .slideInterval,
        0,
      );
      expect(DisplayPrefs.minSlideIntervalSeconds, 5);
      expect(DisplayPrefs.maxSlideIntervalSeconds, 300);
    });

    test('非数字 / 缺字段 → 仍是「关」（回落默认不是夹持）', () {
      expect(
        DisplayPrefs.fromJson(
          <String, Object?>{'slideInterval': 'soon'},
        ).slideInterval,
        0,
      );
      expect(DisplayPrefs.fromJson(const <String, Object?>{}).slideInterval, 0);
    });

    test('_clampInt 的语义没被动过：scrim / align 越界仍回落默认', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'backgroundScrim': 99,
        'imageAlign': -3,
      });
      expect(p.backgroundScrim, DisplayPrefs.defaultBackgroundScrim);
      expect(p.imageAlign, DisplayPrefs.defaultImageAlign);
    });
  });

  group('DEC-5：坏图不算「画得出来」', () {
    test('真 dataURL 形态才算（data:image/… + 逗号 + 非空 payload）', () {
      for (final String ok in <String>[
        'data:image/png;base64,AAA',
        'data:image/jpeg;base64,/9j/4AAQ',
        'data:image/webp;base64,AAAA',
        'data:image/svg+xml,%3Csvg/%3E',
      ]) {
        expect(
          BackgroundImage(id: 'a', dataUrl: ok).isRenderable,
          isTrue,
          reason: '$ok 是合法形态',
        );
      }
    });

    test('不是 dataURL 的串（含旧测试里的 x）一律不算画得出来', () {
      for (final String bad in <String>[
        'x',
        'https://example.com/a.png',
        '/home/user/a.png',
        'data:text/plain;base64,AAA',
        'data:image/png;base64',
        'data:image/png;base64,',
        'data:image/png,',
      ]) {
        expect(
          BackgroundImage(id: 'b', dataUrl: bad).isRenderable,
          isFalse,
          reason: '$bad 不是 dataURL 图片形态',
        );
      }
      // 字节还没读回来是**另一态**（不是坏图），同样画不出来。
      expect(const BackgroundImage(id: 'c').isRenderable, isFalse);
    });

    test('坏图不再让 hasBackground 为真（界面不许说「有背景」而屏幕上一张都没有）', () {
      final DisplayPrefs p = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[
          const BackgroundImage(id: 'x', dataUrl: 'x'),
        ],
      );
      expect(p.hasBackground, isFalse);
      expect(p.effectiveBackground, isNotNull, reason: '项还在，「画不出来」是另一回事');
    });
  });

  group('四档铺法：每档一条**不同**的渲染路径', () {
    test('boxFitFor 的四档互不相同，区间外回 cover', () {
      expect(boxFitFor(DisplayPrefs.fitCover), BoxFit.cover);
      expect(boxFitFor(DisplayPrefs.fitContain), BoxFit.contain);
      expect(boxFitFor(DisplayPrefs.fitStretch), BoxFit.fill);
      expect(boxFitFor(DisplayPrefs.fitTile), BoxFit.none);
      expect(boxFitFor(99), BoxFit.cover);
      final Set<BoxFit> seen = <BoxFit>{
        for (final int f in <int>[0, 1, 2, 3]) boxFitFor(f),
      };
      expect(seen.length, 4, reason: '有一档落进了别人的分支 = 「按了没区别」');
    });

    test('fitName 与 fit 常量一一对应（存储的 4 个数各有一个名字）', () {
      expect(DisplayPrefs.fitName(DisplayPrefs.fitCover), 'cover');
      expect(DisplayPrefs.fitName(DisplayPrefs.fitContain), 'contain');
      expect(DisplayPrefs.fitName(DisplayPrefs.fitStretch), 'stretch');
      expect(DisplayPrefs.fitName(DisplayPrefs.fitTile), 'tile');
      expect(DisplayPrefs.maxImageFit, DisplayPrefs.fitTile);
    });

    testWidgets('cover：Image 走 BoxFit.cover，不套 FittedBox', (WidgetTester tester) async {
      await tester.pumpWidget(_host(item: _pngItem, fit: DisplayPrefs.fitCover));
      expect(_imageFit(tester), BoxFit.cover);
      expect(_inBackdrop(find.byType(FittedBox)), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('contain：Image 走 BoxFit.contain（与 cover 是真的两档）', (WidgetTester tester) async {
      await tester.pumpWidget(_host(item: _pngItem, fit: DisplayPrefs.fitContain));
      expect(_imageFit(tester), BoxFit.contain);
      expect(_inBackdrop(find.byType(FittedBox)), findsNothing);
    });

    testWidgets('stretch：FittedBox(BoxFit.fill) 包一层（不是 cover 的同一条路）',
        (WidgetTester tester) async {
      await tester.pumpWidget(_host(item: _pngItem, fit: DisplayPrefs.fitStretch));
      final FittedBox box = tester.widget<FittedBox>(_inBackdrop(find.byType(FittedBox)));
      expect(box.fit, BoxFit.fill, reason: '拉伸必须真的拉伸（BoxFit.fill），不是 cover');
      expect(_inBackdrop(find.byType(Image)), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('tile：走平铺层（kShellTileKey），不出现单张 Image(fit:)',
        (WidgetTester tester) async {
      await tester.pumpWidget(_host(item: _pngItem, fit: DisplayPrefs.fitTile));
      expect(find.byKey(kShellTileKey), findsOneWidget, reason: 'tile 这一档必须走平铺层');
      expect(_inBackdrop(find.byType(Image)), findsNothing, reason: '平铺不走 Image 那条路');
      expect(tester.takeException(), isNull);
    });

    testWidgets('tile：贴片边长由 tileSize 换算（一块 = tileSize 逻辑像素）',
        (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(item: _pngItem, fit: DisplayPrefs.fitTile, tileSize: 24),
      );
      // 平铺层必须在；**画笔在真解码完成前是 null**（这一层不画，底色照旧）。
      expect(find.byKey(kShellTileKey), findsOneWidget);
      expect(_inBackdrop(find.byType(CustomPaint)), findsWidgets);
      // 真解码在 widget 测试的 fake async 里不会完成（runAsync 也推不动
      // 在 fake zone 里发起的 ImageStream）⇒ 像素级验证只能靠 Win 肉眼；
      // 这里钉住**唯一可确定的算术**：贴片宽 = tileSize。
      expect(tileScaleFor(640, 24), 640 / 24);
      expect(tileScaleFor(640, DisplayPrefs.defaultTileSize), 10.0);
      expect(tester.takeException(), isNull);
    });

    testWidgets('坏 dataURL / 没有字节 → 退回底色，不抛', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(
          item: const BackgroundImage(id: 'bad', dataUrl: 'data:image/png;base64,@@@'),
        ),
      );
      expect(tester.takeException(), isNull);
      expect(find.text('子树'), findsOneWidget);
      expect(_inBackdrop(find.byType(Image)), findsNothing);
      await tester.pumpWidget(
        _host(item: const BackgroundImage(id: 'pending')),
      );
      expect(tester.takeException(), isNull);
      expect(_inBackdrop(find.byType(Stack)), findsNothing, reason: '画不出来就连叠放层都不要');
    });
  });

  group('逐图样式覆盖：null = 回落全局（parity §6 的 perItemStyle）', () {
    testWidgets('逐图 fit 覆盖全局（全局 contain + 这一项 cover）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(
          item: _pngItem.copyWith(fit: DisplayPrefs.fitCover),
          fit: DisplayPrefs.fitContain,
        ),
      );
      expect(_imageFit(tester), BoxFit.cover);
    });

    testWidgets('逐图 align 覆盖全局（全局右下 + 这一项左上）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(item: _pngItem.copyWith(align: 0), align: 8),
      );
      expect(_imageAlign(tester), const Alignment(-1, -1));
    });

    testWidgets('逐图 opacity 覆盖全局（图与遮罩读同一个解析值）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(item: _pngItem.copyWith(opacity: 0.25), opacity: 0.9),
      );
      expect(_layerOpacity(tester), closeTo(0.25, 1e-9));
    });

    testWidgets('逐图 opacity = 0 → 这一项显式不画（只有底色）', (WidgetTester tester) async {
      await tester.pumpWidget(
        _host(item: _pngItem.copyWith(opacity: 0), opacity: 0.9),
      );
      expect(_inBackdrop(find.byType(Stack)), findsNothing);
      expect(find.text('子树'), findsOneWidget);
    });

    test('解析点唯一：effectiveImage* 与 hasBackgroundAt 读的是同一份解析', () {
      final BackgroundImage item = _pngItem.copyWith(opacity: 0.0);
      const DisplayPrefs prefs = DisplayPrefs(backgroundOpacity: 0.9);
      expect(DisplayPrefs.effectiveImageOpacity(item, 0.9), 0.0);
      expect(prefs.copyWith(backgrounds: <BackgroundItem>[item]).hasBackground,
          isFalse, reason: '逐图 0 覆盖 ⇒ 判据也必须说「没有背景」');
      expect(DisplayPrefs.effectiveImageFit(item.copyWith(fit: 2), 1), 2);
      expect(DisplayPrefs.effectiveImageAlign(item.copyWith(align: 7), 4), 7);
    });
  });

  group('BackgroundImage 结构枚举（漏字段 = 静默失效 · P4）', () {
    const BackgroundImage full = BackgroundImage(
      id: 'bg1',
      dataUrl: 'data:image/png;base64,AAA',
      opacity: 0.4,
      fit: 2,
      align: 7,
    );

    test('toValuesMap 覆盖全部字段（kStructuralFieldCount 对账）', () {
      expect(full.toValuesMap().keys.toSet(), <String>{
        'id',
        'dataUrl',
        'opacity',
        'fit',
        'align',
      });
      expect(full.toValuesMap().length, BackgroundImage.kStructuralFieldCount);
    });

    test('三个样式字段都被 copyWith 处理（改一个就必须不等、hashCode 跟着变）', () {
      final List<BackgroundImage> singles = <BackgroundImage>[
        full.copyWith(opacity: 0.9),
        full.copyWith(fit: 3),
        full.copyWith(align: 0),
      ];
      expect(singles.length, 3);
      for (final BackgroundImage one in singles) {
        expect(one, isNot(full), reason: '有样式字段被 copyWith 漏掉了');
        expect(one.hashCode, isNot(full.hashCode));
      }
    });

    test('dataUrl 仍然不参与 ==（水合前后是同一项），但确实被 copyWith 换了', () {
      final BackgroundImage hydrated =
          full.copyWith(dataUrl: 'data:image/png;base64,BBB');
      expect(hydrated.toValuesMap()['dataUrl'], 'data:image/png;base64,BBB');
      expect(hydrated, full, reason: '按 id 判等：水合不改身份');
      expect(hydrated.hashCode, full.hashCode);
    });

    test('往返保住**四个落盘字段**；dataUrl 刻意不落盘（读回是 null）', () {
      final BackgroundImage back = BackgroundImage.fromJson(full.toJson())!;
      expect(back.id, 'bg1');
      expect(back.opacity, 0.4);
      expect(back.fit, 2);
      expect(back.align, 7);
      expect(
        back.dataUrl,
        isNull,
        reason: '写了 dataURL 就等于把图又塞回 localStorage，整个搬库白做',
      );
      expect(back, full, reason: '== 不看 dataUrl ⇒ 水合前后仍是同一项');
    });

    test('toJson 只写**设过**的样式（null 不落盘、dataUrl 永不落盘）', () {
      expect(
        const BackgroundImage(id: 'a').toJson().keys.toSet(),
        <String>{'kind', 'id'},
      );
      expect(
        full.toJson().keys.toSet(),
        <String>{'kind', 'id', 'opacity', 'fit', 'align'},
      );
      expect(
        const BackgroundImage(id: 'a').toJson().containsKey('dataUrl'),
        isFalse,
      );
    });

    test('fromJson 的坏样式 = 没设过（回落全局），不抛', () {
      final BackgroundImage? item = BackgroundImage.fromJson(<Object?, Object?>{
        'kind': 'image',
        'id': 'a',
        'opacity': 2.5,
        'fit': 9,
        'align': -1,
      });
      expect(item, isNotNull);
      expect(item!.opacity, isNull, reason: '越界的不透明度当「没设过」，不是夹到 1');
      expect(item.fit, isNull);
      expect(item.align, isNull);
      final BackgroundImage? weird = BackgroundImage.fromJson(<Object?, Object?>{
        'kind': 'image',
        'id': 'b',
        'opacity': 'solid',
        'fit': 'cover',
      });
      expect(weird!.opacity, isNull);
      expect(weird.fit, isNull);
    });

    test('旧档（只有 dataUrl、没有 id）也能带上逐图样式', () {
      final BackgroundImage? item = BackgroundImage.fromJson(<Object?, Object?>{
        'kind': 'image',
        'dataUrl': 'data:image/png;base64,AAA',
        'fit': 3,
      });
      expect(item!.id, backgroundIdOf('data:image/png;base64,AAA'));
      expect(item.fit, 3);
    });

    test('copyWith(dataUrl:) 之后三个样式字段逐字不变（R6-a2 水合的硬要求）', () {
      final BackgroundImage hydrated =
          full.copyWith(dataUrl: 'data:image/png;base64,ZZZ');
      expect(hydrated.opacity, full.opacity);
      expect(hydrated.fit, full.fit);
      expect(hydrated.align, full.align);
      expect(hydrated.toValuesMap()['opacity'], 0.4);
      expect(hydrated.toValuesMap()['fit'], 2);
      expect(hydrated.toValuesMap()['align'], 7);
    });

    test('copyWith(clearStyle: true) 清掉三项覆盖（回到「回落全局」）', () {
      final BackgroundImage cleared = full.copyWith(clearStyle: true);
      expect(cleared.opacity, isNull);
      expect(cleared.fit, isNull);
      expect(cleared.align, isNull);
      expect(cleared.id, full.id);
      expect(cleared.dataUrl, full.dataUrl);
    });
  });

  group('全局 background.enabled（DEC-4）', () {
    test('默认 true；旧档缺字段 → 迁移成 true（老用户界面一个像素不变）', () {
      expect(const DisplayPrefs().backgroundEnabled, isTrue);
      expect(DisplayPrefs.defaultBackgroundEnabled, isTrue);
      expect(
        DisplayPrefs.fromJson(const <String, Object?>{}).backgroundEnabled,
        isTrue,
      );
      expect(
        DisplayPrefs.fromJson(
          <String, Object?>{'backgroundEnabled': false},
        ).backgroundEnabled,
        isFalse,
      );
    });

    test('写进 JSON 再读回来，且参与 == / hashCode', () {
      const DisplayPrefs off = DisplayPrefs(backgroundEnabled: false);
      expect(DisplayPrefs.fromJson(off.toJson()).backgroundEnabled, isFalse);
      expect(off, isNot(const DisplayPrefs()));
      expect(off.hashCode, isNot(const DisplayPrefs().hashCode));
      expect(off.copyWith(), off, reason: '缺省 copyWith 不许动它');
    });

    test('关掉 → hasBackgroundAt 一律 false（判据只此一处），且不动透明度', () {
      const DisplayPrefs off = DisplayPrefs(
        backgroundEnabled: false,
        backgrounds: <BackgroundItem>[_pngItem],
      );
      expect(off.hasBackground, isFalse);
      expect(off.hasBackgroundAt(_pngItem), isFalse);
      expect(
        off.backgroundOpacity,
        DisplayPrefs.defaultBackgroundOpacity,
        reason: '开关不许抹掉用户调好的透明度（那是「可逆」的全部意义）',
      );
      expect(off.effectiveBackground, isNotNull, reason: '项还在，只是不画');
    });

    testWidgets('ShellBackdrop(enabled: false) → 只有底色，连 Stack 都没有',
        (WidgetTester tester) async {
      await tester.pumpWidget(_host(item: _pngItem, enabled: false));
      expect(find.text('子树'), findsOneWidget);
      expect(_inBackdrop(find.byType(Stack)), findsNothing);
      expect(_inBackdrop(find.byType(Image)), findsNothing);
      expect(tester.takeException(), isNull);
    });

    testWidgets('app_shell 真的把开关传下去：关 → 不画图 + 脚手架底回不透明；开 → 透',
        (WidgetTester tester) async {
      await _pumpAppShell(
        tester,
        const DisplayPrefs(
          backgroundEnabled: false,
          backgrounds: <BackgroundItem>[_pngItem],
        ),
      );
      expect(
        _shellBackdrop(tester).enabled,
        isFalse,
        reason: 'app_shell 没把 prefs.backgroundEnabled 传下去',
      );
      expect(
        _scaffoldBackground(tester),
        isNull,
        reason: '没有背景 → 退回**不透明**的主题底（写反了背景就永远看不见）',
      );
      expect(_inBackdrop(find.byType(Image)), findsNothing, reason: '关掉就不许再画图');

      await _pumpAppShell(
        tester,
        const DisplayPrefs(backgrounds: <BackgroundItem>[_pngItem]),
      );
      expect(_shellBackdrop(tester).enabled, isTrue);
      expect(_scaffoldBackground(tester), Colors.transparent);
      expect(_inBackdrop(find.byType(Image)), findsOneWidget);
    });
  });

  group('tileSize：只对 tile 有效', () {
    test('默认 64、区间 [16, 256]；非有限数回落默认', () {
      expect(DisplayPrefs.defaultTileSize, 64.0);
      expect(DisplayPrefs.minTileSize, 16.0);
      expect(DisplayPrefs.maxTileSize, 256.0);
      expect(DisplayPrefs.clampTileSize(double.nan), 64.0);
      expect(DisplayPrefs.clampTileSize(double.infinity), 64.0);
      expect(DisplayPrefs.clampTileSize(-5), 16.0);
      expect(DisplayPrefs.clampTileSize(9999), 256.0);
    });

    test('写进 JSON 再读回来；越界夹到端点；参与 == / hashCode', () {
      const DisplayPrefs big = DisplayPrefs(tileSize: 200);
      expect(DisplayPrefs.fromJson(big.toJson()).tileSize, 200.0);
      expect(
        DisplayPrefs.fromJson(<String, Object?>{'tileSize': 8}).tileSize,
        16.0,
      );
      expect(
        DisplayPrefs.fromJson(<String, Object?>{'tileSize': 9999}).tileSize,
        256.0,
      );
      expect(
        DisplayPrefs.fromJson(<String, Object?>{'tileSize': 'wide'}).tileSize,
        64.0,
      );
      expect(big, isNot(const DisplayPrefs()));
      expect(big.hashCode, isNot(const DisplayPrefs().hashCode));
    });

    test('tileScaleFor：一块贴片的宽度 = tileSize（退化输入不抛）', () {
      expect(tileScaleFor(640, 64), 10.0);
      expect(tileScaleFor(100, 100), 1.0);
      expect(tileScaleFor(0, 64), 1.0);
      expect(tileScaleFor(-3, 64), 1.0);
      expect(tileScaleFor(640, 0), 1.0);
      expect(tileScaleFor(640, double.nan), 1.0);
    });

    testWidgets('app_shell 把 prefs.tileSize 传给 ShellBackdrop', (WidgetTester tester) async {
      await _pumpAppShell(tester, const DisplayPrefs(tileSize: 24));
      expect(_shellBackdrop(tester).tileSize, 24.0);
    });
  });

  group('DEC-5 负向：坏图在**字段**与**解码**两边都不算数', () {
    test('形态非法 ⇒ isRenderable false 且 decodeDataUrlBytes 也解不出字节', () {
      for (final String bad in <String>[
        'x',
        'data:image/png;base64,',
        'data:text/plain,abc',
      ]) {
        expect(BackgroundImage(id: 'b', dataUrl: bad).isRenderable, isFalse);
        expect(decodeDataUrlBytes(bad), isNull, reason: bad);
      }
      // 反例：形态合法时两边都成立（否则上面三条可能只是「恰好为真」）。
      const String ok = 'data:image/png;base64,iVBORw0KGgo=';
      expect(BackgroundImage(id: 'ok', dataUrl: ok).isRenderable, isTrue);
      expect(decodeDataUrlBytes(ok), isNotNull);
    });
  });
}

/// 1×1 透明 PNG 的 dataURL（**真图**：widget 测试里会真的过 `Image.memory`）。
const String _onePixelPng =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

/// 一项「画得出来」的图片（不含逐图样式）。
const BackgroundImage _pngItem = BackgroundImage(id: 'p', dataUrl: _onePixelPng);

/// 只挂 [ShellBackdrop] 的最小宿主（不拉整棵 AppShell：慢，且与本组断言无关）。
Widget _host({
  DisplayPrefs prefs = const DisplayPrefs(),
  BackgroundItem? item,
  bool? enabled,
  double? opacity,
  int? fit,
  int? align,
  double? tileSize,
  int? scrim,
}) => MaterialApp(
  // 用**真的**主题：`ShellBackdrop` 的遮罩颜色读 AppColors/AppPalette。
  theme: buildAppTheme(prefs.theme),
  home: ShellBackdrop(
    baseColor: AppPalette.of(prefs.theme).stage,
    item: item,
    enabled: enabled ?? prefs.backgroundEnabled,
    opacity: opacity ?? prefs.backgroundOpacity,
    fit: fit ?? prefs.imageFit,
    align: align ?? prefs.imageAlign,
    tileSize: tileSize ?? prefs.tileSize,
    scrim: scrim ?? prefs.backgroundScrim,
    child: const Text('子树'),
  ),
);

/// pump 真 `AppShell`（只开这一个背景，别让它替我们承担别的断言）。
Future<void> _pumpAppShell(WidgetTester tester, DisplayPrefs prefs) async {
  await tester.binding.setSurfaceSize(const Size(1400, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: AppShell(
        prefs: prefs,
        stage: const ColoredBox(color: Color(0xFF000000)),
        phase: UiPhase.idle,
        wsStatus: WsStatus.connected,
        messages: const <Never>[],
        input: TextEditingController(),
        onSend: () {},
        onStop: () {},
        onRetryConnection: () {},
        volume: 0.8,
        muted: false,
        onVolumeChanged: (_) {},
        onMutedChanged: (_) {},
        sections: visibleSections(),
        sectionBuilder: (BuildContext context, SettingsSection s) =>
            Text('PANE:${s.label}'),
        stagePhase: Live2DBridgePhase.ready,
      ),
    ),
  );
  await tester.pump();
}

Finder _inBackdrop(Finder matching) =>
    find.descendant(of: find.byType(ShellBackdrop), matching: matching);

ShellBackdrop _shellBackdrop(WidgetTester tester) =>
    tester.widget<ShellBackdrop>(find.byType(ShellBackdrop));

/// 外壳自己那个 `Scaffold` 的**实际**底色（`null` = 退回主题底）。
Color? _scaffoldBackground(WidgetTester tester) =>
    tester.widget<Scaffold>(find.byType(Scaffold).first).backgroundColor;

BoxFit? _imageFit(WidgetTester tester) =>
    tester.widget<Image>(_inBackdrop(find.byType(Image)).first).fit;

Alignment _imageAlign(WidgetTester tester) =>
    tester.widget<Image>(_inBackdrop(find.byType(Image)).first).alignment
        as Alignment;

double _layerOpacity(WidgetTester tester) =>
    tester.widget<Opacity>(_inBackdrop(find.byType(Opacity)).first).opacity;

