/// F-0034-01（P2，审计 2026-09-28 B0087 升级为**在线**）+
/// F-0074-01（总闸 `DisplayPrefs.==` 零测试覆盖）。
///
/// # 缺陷
///
/// 逐图样式（不透明度 / 铺法 / 位置）的编辑**是接上的**（
/// `appearance_background.dart` 的 `onItemChanged` → `AppearanceSection` →
/// 组合根），但总闸 `DisplayPrefs.==` 把逐项比较**委托给了 id-only 的
/// `BackgroundImage.sameAs`** ⇒「只改了样式」被判成「什么都没变」⇒
/// `if (next == widget.prefs) return;`（`shell_prefs._updatePrefs` /
/// `main.dart._update`）**静默丢掉**这次编辑：界面不动、刷新还原、
/// **没有任何错误**。
///
/// `sameAs` 本身是对的——它的语义是「字节读回前后算同一项」（水合去重），
/// 不是通用判等。本文件把两件事分开钉：
///
/// - **聚合层**：`DisplayPrefs.==` 必须看见逐图样式（F-0034-01）；
/// - **sameAs 语义**：同 id 但样式不同仍算「同一项」——**有意为之**，
///   别再当通用判据用（F-0074-01）。
///
/// # 为什么单独一个文件
///
/// `display_prefs_test.dart` 已经 892 行（>800 是既有债，D2 才拆），
/// 再往里塞只会让它更难动。
///
/// # 为什么接缝那组是「真泵 + 同形闸门」
///
/// 闸门本体在 `shell_prefs.dart` / `main.dart`（`package:web` + `part`，
/// VM 里 import 不了）。所以用一个**与它同形**的宿主
/// （`if (next == prefs) return;` + 记下「本该落盘」的每一份）驱动**真**
/// `AppearanceSection`——旧实现下这条会红（样式编辑被闸门吃掉），
/// 而不是靠扫源码文本。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/sections/appearance_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 1×1 真 PNG 的 dataURL（会真的过 `isRenderable` 与 `Image.memory`）。
///
/// 与其它背景测试各自带一份是**既有状态**（`_png` 在 3 个测试文件里重复）；
/// 这里不引共享常量是因为它只是一段固定字节的夹具，抽出去反而多一层间接。
const String _png =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

BackgroundImage _img(
  String id, {
  String? url,
  double? opacity,
  int? fit,
  int? align,
}) => BackgroundImage(
  id: id,
  dataUrl: url,
  opacity: opacity,
  fit: fit,
  align: align,
);

DisplayPrefs _oneItem(BackgroundItem item) =>
    const DisplayPrefs().copyWith(backgrounds: <BackgroundItem>[item]);

/// 与 `shell_prefs._updatePrefs` / `main.dart._update` **同形**的宿主。
///
/// 那道闸就是 F-0034-01 的落点：判「没变」→ 直接 return（既不 setState
/// 也不落盘）。`saved` = 生产里真正写进本机存储的那几份。
class _GateHost extends StatefulWidget {
  const _GateHost({super.key, required this.initial});

  final DisplayPrefs initial;

  @override
  State<_GateHost> createState() => _GateHostState();
}

class _GateHostState extends State<_GateHost> {
  late DisplayPrefs prefs = widget.initial;

  /// 本该落盘的每一份（生产里 = `saveDisplayPrefs(next)`）。
  final List<DisplayPrefs> saved = <DisplayPrefs>[];

  void _onChanged(DisplayPrefs next) {
    if (next == prefs) return; // ← 那道闸（F-0034-01 的现场）
    setState(() => prefs = next);
    saved.add(next);
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: buildAppTheme(),
    home: Scaffold(
      body: SingleChildScrollView(
        child: BackgroundRuntimeScope(
          index: 0,
          current: prefs.effectiveBackground,
          hydrating: false,
          child: AppearanceSection(
            prefs: prefs,
            onPrefsChanged: _onChanged,
            onPickShellImage: () {},
            onClearShellImage: () {},
            onRemoveBackground: (int _) {},
            onReorderBackground: (int _, int _) {},
            onPreviewBackground: (int _) {},
            onAddPattern: (int _) {},
          ),
        ),
      ),
    ),
  );
}

/// 把测试视口拉高：外观分区比默认 800×600 高，控件落在视口外就点不中。
void _tall(WidgetTester tester) {
  tester.view.physicalSize = const Size(1000, 2600);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
}

void main() {
  group('聚合层：DisplayPrefs.== 必须看见逐图样式（F-0034-01 / F-0074-01）', () {
    // 三种逐图样式各来一条。表驱动而不是手写三条断言：加样式字段时只需要
    // 往表里加一行，不容易漏（F-0074-01 的「参数化」建议）。
    final Map<String, BackgroundItem> styledAxis = <String, BackgroundItem>{
      '不透明度': _img('a', url: _png, opacity: 0.4),
      '铺法': _img('a', url: _png, fit: DisplayPrefs.fitContain),
      '位置': _img('a', url: _png, align: 8),
    };

    for (final MapEntry<String, BackgroundItem> e in styledAxis.entries) {
      test('仅逐图「${e.key}」不同 ⇒ 判为「变了」（旧实现走 sameAs ⇒ 判为没变）', () {
        final DisplayPrefs base = _oneItem(_img('a', url: _png));
        final DisplayPrefs next = _oneItem(e.value);
        expect(
          next,
          isNot(base),
          reason:
              '只改了这一张图的${e.key}：`==` 判等 ⇒ 组合根那道 '
              '`if (next == prefs) return;` 会**静默丢掉**这次编辑（F-0034-01）',
        );
        expect(
          next.hashCode == base.hashCode,
          isFalse,
          reason:
              '`==` 与 `hashCode` 必须对称：相等才允许同哈希，'
              '反过来「不等却同哈希」会让 Set/Map 行为与直觉不符',
        );
      });
    }

    test('全字段相同（含逐图样式）⇒ 相等（否则每次重建都当「变了」）', () {
      final DisplayPrefs a = _oneItem(
        _img('a', url: _png, opacity: 0.5, fit: DisplayPrefs.fitTile, align: 2),
      );
      final DisplayPrefs b = _oneItem(
        _img('a', url: _png, opacity: 0.5, fit: DisplayPrefs.fitTile, align: 2),
      );
      expect(a, b);
      expect(a.hashCode, b.hashCode);
      // 字节不同（同一 id、dataUrl 不同）**不算变**：`==`/`hashCode` 都不看
      // `dataUrl`——那是水合补字节的语义（补完不能当成「用户改了偏好」）。
      expect(
        a,
        _oneItem(_img('a', opacity: 0.5, fit: DisplayPrefs.fitTile, align: 2)),
      );
    });

    test('样式不同 ⇒ 聚合层 !=，而 sameAs 仍判「同一项」：两种判据各司其职', () {
      final BackgroundItem plain = _img('a', url: _png);
      final BackgroundItem styled = _img(
        'a',
        url: _png,
        fit: DisplayPrefs.fitContain,
      );
      expect(_oneItem(plain), isNot(_oneItem(styled)));
      expect(
        plain.sameAs(styled),
        isTrue,
        reason:
            'sameAs 的语义是「字节读回前后算同一项」（只看 id），'
            '不是「变了没有」——把它当通用判据正是 F-0034-01',
      );
    });
  });

  group('sameAs 的语义（钉成有意为之，别第三次被当通用判据）', () {
    test('同 id、样式不同 ⇒ sameAs 为真，但 BackgroundImage.== 为假', () {
      final BackgroundImage plain = _img('a');
      final BackgroundImage styled = _img('a', opacity: 0.3, fit: 1, align: 8);
      expect(plain.sameAs(styled), isTrue);
      expect(styled.sameAs(plain), isTrue, reason: '对称');
      expect(plain == styled, isFalse, reason: '`==` 看样式，`sameAs` 不看');
      expect(plain.hashCode == styled.hashCode, isFalse);
    });

    test('id 不同 ⇒ sameAs 为假（哪怕样式一样）', () {
      expect(_img('a', opacity: 1).sameAs(_img('b', opacity: 1)), isFalse);
    });

    test('同 id、一个有字节一个没有 ⇒ sameAs 与 == 都判「同一项」', () {
      final BackgroundImage withBytes = _img('a', url: _png);
      final BackgroundImage pending = _img('a');
      expect(withBytes.sameAs(pending), isTrue);
      expect(withBytes == pending, isTrue, reason: '水合补字节不能算「用户改了偏好」');
    });

    test('内置图案：同 id 判等、不同 id 不判等、与图片项永不相等', () {
      const BackgroundPattern grid = BackgroundPattern(
        BackgroundPatternId.grid,
      );
      const BackgroundPattern glow = BackgroundPattern(
        BackgroundPatternId.glow,
      );
      expect(
        grid.sameAs(const BackgroundPattern(BackgroundPatternId.grid)),
        isTrue,
      );
      expect(grid.sameAs(glow), isFalse);
      expect(grid == glow, isFalse);
      expect(grid.sameAs(_img('0')), isFalse, reason: '身份含 kind，图案 ≠ 图片');
      expect(grid == _img('0'), isFalse);
    });
  });

  group('变更检测接缝（真泵 AppearanceSection + 与生产同形的闸门）', () {
    testWidgets('只改一张图的铺法 → 偏好确实变化，且写成 JSON 再读回来仍在', (
      WidgetTester tester,
    ) async {
      _tall(tester);
      final GlobalKey<_GateHostState> key = GlobalKey<_GateHostState>();
      final DisplayPrefs initial = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[_img('a', url: _png)],
        imageFit: DisplayPrefs.fitCover,
      );
      await tester.pumpWidget(_GateHost(key: key, initial: initial));
      await tester.pump();
      expect(key.currentState!.saved, isEmpty, reason: '前提：还什么都没改');

      // 逐图样式折叠区 → 「单独设置」铺法（只动这一项，不换图）。
      await tester.tap(find.text('样式'));
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey<String>('bg-style-set-fit')));
      await tester.pump();

      expect(
        key.currentState!.saved,
        isNotEmpty,
        reason:
            'F-0034-01 的现场：旧实现里 `next == prefs` 为真（sameAs 只看 id）'
            '⇒ 这道闸直接 return，用户改了样式而界面 / 存储 / 渲染面一处都没动',
      );

      // ① 偏好确实变了：这张图有了显式覆盖（= 点下去那一刻的全局值）。
      final DisplayPrefs next = key.currentState!.saved.single;
      final BackgroundImage item = next.backgrounds.single as BackgroundImage;
      expect(item.fit, DisplayPrefs.fitCover);
      expect(item.dataUrl, _png, reason: 'dataUrl 原样带走（只改样式，不换图）');

      // ② 持久化：写进 JSON 再读回来，样式还在（这就是「刷新还原」的反面）。
      final DisplayPrefs round = DisplayPrefs.fromJson(next.toJson());
      expect(
        (round.backgrounds.single as BackgroundImage).fit,
        DisplayPrefs.fitCover,
      );
      expect(round, next, reason: '往返之后仍然是「同一份偏好」');
    });

    testWidgets('再改回去（清掉逐图值）同样算「变了」，不是第二次被吞', (WidgetTester tester) async {
      _tall(tester);
      final GlobalKey<_GateHostState> key = GlobalKey<_GateHostState>();
      final DisplayPrefs initial = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[
          _img('a', url: _png, fit: DisplayPrefs.fitContain),
        ],
        imageFit: DisplayPrefs.fitCover,
      );
      await tester.pumpWidget(_GateHost(key: key, initial: initial));
      await tester.pump();

      await tester.tap(find.text('样式'));
      await tester.pump();
      await tester.tap(
        find.byKey(const ValueKey<String>('bg-style-clear-fit')),
      );
      await tester.pump();

      expect(key.currentState!.saved, hasLength(1));
      final BackgroundImage item =
          key.currentState!.saved.single.backgrounds.single as BackgroundImage;
      expect(item.fit, isNull, reason: '清掉逐图值 = 回落全局');
      expect(item.dataUrl, _png);
      expect(
        DisplayPrefs.effectiveImageFit(item, DisplayPrefs.fitCover),
        DisplayPrefs.fitCover,
      );
    });
  });
}
