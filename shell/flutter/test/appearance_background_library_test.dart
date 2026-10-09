/// 「外观与互动 → 背景库」的 **UI 断链**回归（A3c）。
///
/// # 这一份是什么
///
/// 审计临时复现（只读源 `zz_audit_tmp_test.dart`）的 **D 组**按行为搬进正式测试：
/// 背景库管理面板（`_LibraryManager`）此前**没有任何 widget 级回归**——
/// 点缩略图 / 勾选 / 批量删 / 移除 / 铺法的显隐全靠肉眼看，
/// 而这类接线坏掉时不报错，只是「看得见、点不着」。
///
/// 2026-10-08：内置图案（渐变 / 光晕 / 网格 / 斜纹）整体下线——**入口与
/// 读回都没有了**，本文件据此把「加图案」那条换成「图案入口整体不在」。
///
/// # 仍未裁决的（**不在这里写断言**）
///
/// - **D1 预览在 ≥2 项时不可达**：`_managing = _selected.isNotEmpty || _items >= 2`，
///   于是库里 ≥2 项时点缩略图只会勾选，`onPreviewBackground` 一次都不触发，
///   而面板自己的文档把「点一下预览」列为四项能力之一。改它要定「预览与管理模式
///   如何共存」（长按？双击？右上角一个「预览」按钮？），**未裁决**。
/// - **D3 位置九宫格的显隐条件与注释相反**：`appearance_section.dart` 写的是
///   `if (prefs.imageFit != 1)`（=「铺满」时显示），而紧邻注释说「位置只在
///   『完整』下出现」——`imageFit == 1`（完整 / contain）恰恰是对齐真正可见的那一档。
///   修法要么翻转条件、要么改注释并论证「cover 下对齐也确实无效」，**未裁决**。
/// - **D4 来源 = 舞台那张时轮播三项仍显示、但不会生效**：`main.dart:syncSlideshow`
///   把库长强制成 0（定时器不起），而控件的说明仍写「按间隔换下一张」。
///   「切到别的来源时应隐藏 / 禁用轮播控件，还是保留（它们配置的是库本身）」
///   是个产品语义决定，**未裁决**。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/sections/appearance_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

const String _img = 'data:image/png;base64,AAAA';
const String _img2 = 'data:image/png;base64,BBBB';

Widget _pane({
  required DisplayPrefs prefs,
  ValueChanged<int>? onPreview,
  ValueChanged<int>? onRemove,
  ValueChanged<List<int>>? onRemoveMany,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: ThemeSection(
        prefs: prefs,
        onPrefsChanged: (DisplayPrefs _) {},
        onPickShellImage: () {},
        onClearShellImage: () {},
        onRemoveBackground: onRemove,
        onRemoveBackgrounds: onRemoveMany,
        onReorderBackground: (int _, int _) {},
        onPreviewBackground: onPreview,
      ),
    ),
  ),
);

void main() {
  testWidgets('只有 1 项时点缩略图 = 预览（onPreviewBackground 收到下标）', (
    WidgetTester tester,
  ) async {
    final List<int> previewed = <int>[];
    await tester.pumpWidget(
      _pane(
        prefs: const DisplayPrefs(
          backgrounds: <BackgroundItem>[
            BackgroundImage(id: 'i1', dataUrl: _img),
          ],
        ),
        onPreview: previewed.add,
      ),
    );
    await tester.tap(find.textContaining('图片 1'));
    await tester.pump();
    expect(previewed, <int>[0], reason: '1 项时预览也是可达的（D1：任意库大小）');
    expect(
      find.byType(Checkbox),
      findsNothing,
      reason: '默认是**预览**模式；管理要显式点「管理」才进（D1）',
    );
  });

  testWidgets('管理模式（显式进）：点一行 = 勾选；「删除所选」上报**升序**下标', (
    WidgetTester tester,
  ) async {
    final List<int> previewed = <int>[];
    List<int>? removed;
    await tester.pumpWidget(
      _pane(
        prefs: const DisplayPrefs(
          backgrounds: <BackgroundItem>[
            BackgroundImage(id: 'i1', dataUrl: _img),
            BackgroundImage(id: 'i2', dataUrl: _img2),
          ],
        ),
        onPreview: previewed.add,
        onRemoveMany: (List<int> v) => removed = v,
      ),
    );
    await tester.pump();
    // D1（2026-09-28 R6-b）：管理不再是「≥2 项自动进」——那条判据让预览在
    // 库里满 2 项之后**永远不可达**。现在点「管理」才算进管理模式。
    expect(
      find.byType(Checkbox),
      findsNothing,
      reason: '默认是预览模式：≥2 项也不再自动进管理',
    );
    await tester.tap(find.byKey(kBackgroundManageToggleKey));
    await tester.pump();
    expect(find.byType(Checkbox), findsNWidgets(2), reason: '进了管理模式才有复选框');

    await tester.tap(find.textContaining('图片 2'));
    await tester.pump();
    await tester.tap(find.textContaining('图片 1'));
    await tester.pump();
    expect(find.text('已选 2 项'), findsOneWidget);

    // 库面板比 800x600 的测试面高：先把按钮滚进视口再点（真实用户也要滚）。
    await tester.ensureVisible(find.text('删除所选'));
    await tester.pump();
    await tester.tap(find.text('删除所选'));
    await tester.pump();
    expect(removed, <int>[0, 1], reason: '批量删除必须上报升序下标（宿主按序删）');
    expect(previewed, isEmpty, reason: '管理模式里点行是勾选，不是预览');
  });

  testWidgets('移除按钮（×）点一下上报**该项**下标', (WidgetTester tester) async {
    final List<int> removed = <int>[];
    await tester.pumpWidget(
      _pane(
        prefs: const DisplayPrefs(
          backgrounds: <BackgroundItem>[
            BackgroundImage(id: 'i1', dataUrl: _img),
            BackgroundImage(id: 'i2', dataUrl: _img2),
          ],
        ),
        onRemove: removed.add,
      ),
    );
    await tester.pump();
    await tester.tap(find.byTooltip('移除这一项').last);
    await tester.pump();
    expect(removed, <int>[1], reason: '× 必须指到它所在的那一行');
  });

  testWidgets('内置图案入口整体下线：渐变 / 光晕 / 网格 / 斜纹都不在', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_pane(prefs: const DisplayPrefs()));
    await tester.pump();
    for (final String banned in <String>['渐变', '光晕', '网格', '斜纹']) {
      expect(
        find.textContaining(banned),
        findsNothing,
        reason: '背景区不得再出现内置图案「$banned」（2026-10-08）',
      );
    }
    expect(
      find.byType(ActionChip),
      findsNothing,
      reason: '内置图案那一排 ActionChip 已删',
    );
    // 2026-10-09：三级功能介绍全删 ⇒ 原来那句「放你自己的图…」也不在界面上了
    //（它曾用来证明「说明句只谈自己的图和纯色」；现在改成断言这句话不在）。
    expect(find.textContaining('放你自己的图'), findsNothing);
    expect(find.textContaining('选了内置图案时'), findsNothing);
  });

  testWidgets('已删的铺法 / 舞台图控件在界面上找不到（图片 / 图案都一样）', (
    WidgetTester tester,
  ) async {
    for (final BackgroundItem item in <BackgroundItem>[
      const BackgroundImage(id: 'i1', dataUrl: _img),
      const BackgroundPattern(BackgroundPatternId.grid),
    ]) {
      await tester.pumpWidget(
        _pane(prefs: DisplayPrefs(backgrounds: <BackgroundItem>[item])),
      );
      await tester.pump();
      expect(
        find.text('铺法（图）'),
        findsNothing,
        reason: '四档铺法是渲染面能力，不再是用户旋钮（2026-10-07）',
      );
      expect(find.text('换一张'), findsNothing);
      expect(find.text('清除舞台背景图'), findsNothing);
    }
  });

  testWidgets('有库时仍有「添加图片」；「换一张 / 清除舞台背景图」不再存在', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _pane(
        prefs: const DisplayPrefs(
          backgrounds: <BackgroundItem>[
            BackgroundImage(id: 'i1', dataUrl: _img),
          ],
        ),
      ),
    );
    await tester.pump();
    expect(find.text('添加图片'), findsOneWidget);
    expect(
      find.text('换一张'),
      findsNothing,
      reason: '舞台图 = 背景库当前项的投影，没有第二份「换一张」入口',
    );
    expect(find.text('清除舞台背景图'), findsNothing);
    expect(find.textContaining('图片 1'), findsOneWidget);
  });
}
