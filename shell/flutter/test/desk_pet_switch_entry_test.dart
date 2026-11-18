/// 「先进网页，再切桌宠」入口的**位置**回归（2026-11-18）。
///
/// # 这一份守什么
///
/// 2026-11-17 的纠正版（`c6b57de1`）把「切换到桌宠」从 AppBar 的
/// `actions` 里挪进 `StageCornerControls` 那一排——理由是顶栏原本被这颗
/// 按钮挤得放不下「设置」。布局改对之后**没有任何自动化断言**守着它，
/// 下一次有人「顺手把入口放回顶栏更显眼」不会有东西变红。这里补上三条：
///
/// 1. `StageCornerControls` 那一排**含**「切换到桌宠」（且只有一个）；
/// 2. AppBar 的 `actions` 子树里**没有**这四个字，但仍有「设置」；
/// 3. 接线是真的：`onSwitchToDeskPet` 为 null 时整项**不渲染**
///    （不做静默 no-op），非 null 时点它**真的回调**。
///
/// # 为什么还要一条源码守卫
///
/// 上面 1/2 是**渲染层**断言：它们只能证明「传给外壳的那个 stageCorner
/// 渲染对了」，证明不了 `main.dart` 真的把按钮传进了角标、也没有人把它
/// 又塞回顶栏。所以再钉一次**源码形状**：
///
/// - 顶栏所在的库（`lib/app/app_shell.dart` + 它的 parts）里**不得**出现
///   字符串字面量「切换到桌宠」；
/// - `lib/main.dart`（含 parts）里 `stageCorner` 那个实参**必须**把
///   `onSwitchToDeskPet` 接上。
///
/// 源码扫描走 `readLibrarySource`（库 + parts 并集），理由见
/// `test/support/dart_library.dart` 头注：只读库文件会**静默漏掉** part 里
/// 的同一段代码，断言看起来还绿。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/stage_corner_controls.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

/// 入口文案（**唯一真源**：这里只用于断言，产品文案在
/// `lib/ui/stage_corner_controls.dart`）。
const String kDeskPetLabel = '切换到桌宠';

/// 与 `main.dart` 同形：把缩放三键那一排交给外壳的 `stageCorner`。
Widget _shell({VoidCallback? onSwitchToDeskPet}) => MaterialApp(
  theme: buildAppTheme(),
  home: AppShell(
    prefs: const DisplayPrefs(),
    stage: const ColoredBox(color: Color(0xFF000000)),
    phase: UiPhase.idle,
    wsStatus: WsStatus.connected,
    messages: const <Never>[],
    input: TextEditingController(),
    onSend: () {},
    onRetryConnection: () {},
    volume: 0.8,
    muted: false,
    onVolumeChanged: (_) {},
    onMutedChanged: (_) {},
    sections: visibleSections(),
    section: SettingsSection.theme,
    onSectionChanged: (_) {},
    sectionBuilder: (BuildContext context, SettingsSection s) =>
        Text('PANE:${s.label}'),
    stagePhase: Live2DBridgePhase.ready,
    stageCorner: StageCornerControls(
      onZoom: (_) {},
      scaleFromAck: 1,
      onSwitchToDeskPet: onSwitchToDeskPet,
    ),
  ),
);

Future<void> _pump(WidgetTester tester, {VoidCallback? onSwitchToDeskPet}) async {
  // 必须用真实布局宽度：断点判据来自 `LayoutBuilder` 拿到的约束，
  // 套 `MediaQuery` 改不动它（见 `app_shell_layout_test.dart` 的踩坑记录）。
  await tester.binding.setSurfaceSize(const Size(1400, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(_shell(onSwitchToDeskPet: onSwitchToDeskPet));
  await tester.pump();
}

void main() {
  group('渲染层：按钮在舞台角标，不在顶栏', () {
    testWidgets('舞台角标那一排含「切换到桌宠」', (WidgetTester tester) async {
      await _pump(tester, onSwitchToDeskPet: () {});

      // 限定在角标那一排里找：整页恰好一处（顶栏若也放了一颗，这里会变 2，
      // 而下面那条「顶栏没有」会先把原因说清楚）。
      expect(
        find.descendant(
          of: find.byType(StageCornerControls),
          matching: find.text(kDeskPetLabel),
        ),
        findsOneWidget,
        reason: '「切换到桌宠」必须落在舞台角标那一排（缩放三键旁边）',
      );
      expect(
        find.text(kDeskPetLabel),
        findsOneWidget,
        reason: '整页只能有一个入口；两个入口会让用户不知道哪个是真的',
      );
    });

    testWidgets('顶栏 actions 里没有这四个字，且仍有「设置」', (
      WidgetTester tester,
    ) async {
      await _pump(tester, onSwitchToDeskPet: () {});

      expect(
        find.descendant(
          of: find.byType(AppBar),
          matching: find.text(kDeskPetLabel),
        ),
        findsNothing,
        reason: '顶栏挤进这颗按钮就会把「设置」挤掉——2026-11-17 已挪走，不要放回来',
      );
      expect(
        find.descendant(of: find.byType(AppBar), matching: find.text('设置')),
        findsOneWidget,
        reason: '顶栏的「设置」入口必须还在（它是非 compact 下唯一入口）',
      );
    });

    testWidgets('没接线就整项不渲染（不做静默 no-op）', (WidgetTester tester) async {
      await _pump(tester);

      expect(
        find.byType(StageCornerControls),
        findsOneWidget,
        reason: '角标本身照常渲染（缩放三键与它无关）',
      );
      expect(
        find.text(kDeskPetLabel),
        findsNothing,
        reason: '回调为 null 时不该留一颗点了没反应的空按钮',
      );
      // 同一条「不静默」纪律的另一面：三键仍然在，别为了藏一颗按钮把整排藏掉。
      expect(find.text('缩小'), findsOneWidget);
      expect(find.text('放大'), findsOneWidget);
      expect(find.text('复位'), findsOneWidget);
    });

    testWidgets('点它真的回调（不是装饰）', (WidgetTester tester) async {
      int taps = 0;
      await _pump(tester, onSwitchToDeskPet: () => taps++);

      await tester.tap(find.text(kDeskPetLabel));
      await tester.pump();

      expect(taps, 1, reason: '看得见就要点得着，点得着就要真回调');
    });
  });

  group('源码形状：接线没有被搬回顶栏', () {
    test('顶栏所在的库（含 parts）里没有「切换到桌宠」字面量', () {
      final String shellSrc = stripCommentsKeepStrings(
        readLibrarySource('lib/app/app_shell.dart'),
      );
      expect(
        shellSrc.contains(kDeskPetLabel),
        isFalse,
        reason:
            'AppBar 与 app_shell_state.dart 属于这个库：出现这四个字就说明它又被放回顶栏了',
      );
    });

    test('main.dart（含 parts）把 onSwitchToDeskPet 接进 stageCorner', () {
      final String mainSrc = stripCommentsKeepStrings(
        readLibrarySource('lib/main.dart'),
      );
      expect(
        mainSrc.contains('onSwitchToDeskPet: _switchToDeskPet'),
        isTrue,
        reason: '角标那颗按钮的回调必须真的接到 _switchToDeskPet（否则是装饰）',
      );
      expect(
        mainSrc.contains('stageCorner: StageCornerControls('),
        isTrue,
        reason: '角标那一排仍由外壳的 stageCorner 槽位交给舞台',
      );
    });
  });
}
