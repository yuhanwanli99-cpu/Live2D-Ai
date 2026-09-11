/// compact 整页设置过渡（2026-09-11，前端加强计划 P2-1）。
///
/// # 被换掉的是什么
///
/// compact 断点过去的设置入口是**两步跳**：弹一个 8 项抽屉，选完分区再弹
/// 底部浮层。观感上像「弹了两次」，而且抽屉选完就消失、用户还得再等一次。
///
/// 现在是 `PageCrossFade`：整个工作台淡出、设置页淡入，一次到位。
/// 披露层数从 **2 变成 1**。
///
/// # 本文件钉的五组断言（这五组比实现本身值钱）
///
/// 1. **互斥性**：任意时刻只有一页在画——中点附近两页不会同时可见；
/// 2. **保活**：关掉设置再打开，工作台的聊天输入内容与滚动位置都还在；
/// 3. **反向打断**：动画播到一半按 ✕，必须干净地回到工作台（不卡在中间）；
/// 4. **减少动画**：系统开了减少动画时**立刻到终态**（不是「不动」）；
/// 5. **指针**：设置页收起后舞台那一块必须还给 iframe。
///
/// 这五条的形状来自 Morrow 的 `compact_settings_test.dart`
///（`docs/plans/PLAN-frontend-strengthening-2026-09-11.md` §4 P4-6），
/// 按本项目的两页结构改写。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/app/collapsible_panel.dart';
import 'package:live2d_ai_shell/app/page_cross_fade.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/stage_pointer_interceptor.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 极窄的手机宽度（compact 档）。
const double kCompactWidth = 420;

/// 一页的不透明度（`PageCrossFade` 给两页各挂了一个 key）。
double opacityOf(WidgetTester tester, String key) => tester
    .widget<Opacity>(find.byKey(ValueKey<String>(key), skipOffstage: false))
    .opacity;

Widget shell({
  required TextEditingController input,
  SettingsSection section = SettingsSection.appearance,
  ValueChanged<SettingsSection>? onSectionChanged,
  VoidCallback? onEnsureSectionLoaded,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: AppShell(
    stage: const ColoredBox(color: Color(0xFF101010)),
    phase: UiPhase.idle,
    wsStatus: WsStatus.connected,
    messages: const <Never>[],
    input: input,
    onSend: () {},
    onStop: () {},
    onRetryConnection: () {},
    volume: 0.8,
    muted: false,
    onVolumeChanged: (_) {},
    onMutedChanged: (_) {},
    sections: visibleSections(),
    section: section,
    onSectionChanged: onSectionChanged ?? (_) {},
    onEnsureSectionLoaded: onEnsureSectionLoaded,
    // 桩内容刻意做得**够高**：设置页的滚动位置那条断言需要一个
    // 真的能滚起来的列表（只有一行文字是滚不动的）。
    sectionBuilder: (BuildContext context, SettingsSection s) => Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text('PANE:${s.label}'),
        for (int i = 0; i < 60; i++)
          SizedBox(height: 40, child: Text('${s.label} 字段 $i')),
      ],
    ),
    stagePhase: Live2DBridgePhase.ready,
  ),
);

Future<void> pumpCompact(
  WidgetTester tester, {
  TextEditingController? input,
  ValueChanged<SettingsSection>? onSectionChanged,
  VoidCallback? onEnsureSectionLoaded,
}) async {
  await tester.binding.setSurfaceSize(const Size(kCompactWidth, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    shell(
      input: input ?? TextEditingController(),
      onSectionChanged: onSectionChanged,
      onEnsureSectionLoaded: onEnsureSectionLoaded,
    ),
  );
  await tester.pumpAndSettle();
}

/// 进设置页（compact 下入口在聊天面板头部）。
Future<void> openSettings(WidgetTester tester) async {
  await tester.tap(find.text('设置'));
  await tester.pumpAndSettle();
}

void main() {
  group('P2-1：一次过渡到位（不再是抽屉 + 浮层两步跳）', () {
    testWidgets('compact 走的是 PageCrossFade，不是抽屉/浮层', (WidgetTester tester) async {
      await pumpCompact(tester);
      expect(find.byType(PageCrossFade), findsOneWidget);

      await openSettings(tester);
      expect(find.byType(ListTile), findsNothing);
      expect(find.byType(BottomSheet), findsNothing);
      expect(find.byType(Drawer), findsNothing);
    });

    testWidgets('分区导航窄屏是**单行**（不是换 3 行的 Wrap）', (WidgetTester tester) async {
      await pumpCompact(tester);
      await openSettings(tester);
      // 8 个 chip 都在，但只占一行高：横向滚动容器的高度应该接近一个 chip。
      final Size chip = tester.getSize(find.byType(ChoiceChip).first);
      final Finder scroller = find
          .ancestor(
            of: find.byType(ChoiceChip).first,
            matching: find.byType(SingleChildScrollView),
          )
          .first;
      expect(
        tester.getSize(scroller).height,
        lessThan(chip.height * 1.5),
        reason: '导航换行了 —— 窄屏里那几行本来要留给字段',
      );
    });

    testWidgets('打开设置会触发加载（compact 过去靠「先选分区」掩盖了这条）', (
      WidgetTester tester,
    ) async {
      int ensure = 0;
      await pumpCompact(tester, onEnsureSectionLoaded: () => ensure++);
      await openSettings(tester);
      expect(ensure, 1, reason: '打开设置必须直接触发加载');
    });
  });

  group('P2-1 ①：两页**永不同时可见**', () {
    testWidgets('过渡中每一帧都不透明度配对为 (1,0)（中点两页不同时可见）', (
      WidgetTester tester,
    ) async {
      await pumpCompact(tester);
      await tester.tap(find.text('设置'));
      // 手动推进：不要 `pumpAndSettle`（那会跳过整个动画）。
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 80));

      // 半程之前：工作台在淡出，设置页还没开始画。
      expect(opacityOf(tester, 'page-first-opacity'), lessThan(1));
      expect(
        tester
            .widget<Offstage>(
              find
                  .ancestor(
                    of: find.byKey(
                      const ValueKey<String>('page-second-opacity'),
                      skipOffstage: false,
                    ),
                    matching: find.byType(Offstage, skipOffstage: false),
                  )
                  .first,
            )
            .offstage,
        isTrue,
        reason: '前半段设置页就已经在画了 —— 两页叠在一起会花屏',
      );

      await tester.pumpAndSettle();
      expect(opacityOf(tester, 'page-second-opacity'), 1);
    });
  });

  group('P2-1 ②：工作台保活（关掉再打开，输入与滚动都还在）', () {
    testWidgets('输入框里的半句话不会因为开关设置而丢', (WidgetTester tester) async {
      final TextEditingController input = TextEditingController();
      addTearDown(input.dispose);
      await pumpCompact(tester, input: input);

      // 先在聊天里打一半字。
      final Finder field = find.descendant(
        of: find.byType(AppShell),
        matching: find.byType(TextField),
      );
      await tester.enterText(field.first, '打到一半的话');
      await tester.pumpAndSettle();

      await openSettings(tester);
      expect(find.text('PANE:外观与互动'), findsOneWidget);

      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();

      // 输入内容必须还在（`PageCrossFade` 用 `Offstage` 而不是条件插入）。
      expect(input.text, '打到一半的话');
      expect(find.text('打到一半的话'), findsOneWidget);
    });

    testWidgets('设置页的滚动位置也留住（两页都常驻在树里）', (WidgetTester tester) async {
      await pumpCompact(tester);
      await openSettings(tester);

      // 设置页里的内容槽是可滚动的；往下滚一段。
      final Finder paneScroll = find
          .descendant(
            of: find.byType(PageCrossFade),
            matching: find.byType(Scrollable),
          )
          .last;
      await tester.drag(paneScroll, const Offset(0, -120));
      await tester.pumpAndSettle();
      final double scrolled = tester
          .state<ScrollableState>(paneScroll)
          .position
          .pixels;
      expect(scrolled, greaterThan(0), reason: '先得真的滚下去');

      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      await openSettings(tester);
      expect(
        tester.state<ScrollableState>(paneScroll).position.pixels,
        closeTo(scrolled, 0.5),
        reason: '再打开时回到顶部 —— 设置页被重建了',
      );
    });
  });

  group('P2-1 ③：反向打断干净收敛', () {
    testWidgets('过渡中「关掉」→ 干净地回到工作台（不卡在中间）', (
      WidgetTester tester,
    ) async {
      // # 为什么这里不是「点 ✕」
      //
      // `PageCrossFade` 在**动画进行中**给两页都套了
      // `IgnorePointer(ignoring: isAnimating)`（照抄 Morrow）：正在移动的
      // 东西被点中，用户的意图和实际命中的控件会对不上，所以过渡期间
      // 点击是被**刻意**屏蔽的。真正能在动画中途打断的是 Esc 与系统返回
      //（它们走根部，不受那层影响）——而三者最终都调到
      // `AppShellState.closeSettings()`，所以这里直接调它，验的是
      // `PageCrossFade` 自己的**反向收敛**。
      //
      // 「✕ / Esc 真的接到了这个函数」由 `app_shell_layout_test.dart`
      // 与 `main.dart` 的 `onDismissOverlay` 各自覆盖。
      await pumpCompact(tester);
      final AppShellState shellState = tester.state<AppShellState>(
        find.byType(AppShell),
      );

      await tester.tap(find.text('设置'));
      await tester.pump();
      // 推到后半段：此时设置页已经画出来了（`settingsVisible` 过半才换树），
      // 打断才是有意义的「反悔」。
      await tester.pump(const Duration(milliseconds: 200));
      expect(opacityOf(tester, 'page-second-opacity'), greaterThan(0));

      shellState.closeSettings();
      await tester.pumpAndSettle();

      expect(opacityOf(tester, 'page-first-opacity'), 1);
      expect(
        tester
            .widget<Offstage>(
              find
                  .ancestor(
                    of: find.byKey(
                      const ValueKey<String>('page-second-opacity'),
                      skipOffstage: false,
                    ),
                    matching: find.byType(Offstage, skipOffstage: false),
                  )
                  .first,
            )
            .offstage,
        isTrue,
        reason: '反向收敛之后设置页还在画 —— 卡在中间了',
      );
      expect(
        tester
            .widget<Offstage>(
              find
                  .ancestor(
                    of: find.byKey(
                      const ValueKey<String>('page-first-opacity'),
                      skipOffstage: false,
                    ),
                    matching: find.byType(Offstage, skipOffstage: false),
                  )
                  .first,
            )
            .offstage,
        isFalse,
      );
    });
  });

  group('P2-1 ④：减少动画时**立刻到终态**（不是「不动」）', () {
    testWidgets('打开设置：一帧之内就到位，而不是停在半透明', (WidgetTester tester) async {
      await tester.pumpWidget(
        MediaQuery(
          data: const MediaQueryData(disableAnimations: true),
          child: shell(input: TextEditingController()),
        ),
      );
      await tester.binding.setSurfaceSize(const Size(kCompactWidth, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pump();

      await tester.tap(find.text('设置'));
      await tester.pump(); // 只推一帧

      expect(
        opacityOf(tester, 'page-second-opacity'),
        1,
        reason: '减少动画时没有立刻到终态 —— 界面会卡在半透明',
      );
      expect(opacityOf(tester, 'page-first-opacity'), 0);
    });
  });

  group('P2-1 ⑤：设置页收起后，舞台那一块要还给 iframe', () {
    testWidgets('垫层跟着开关一起收起（否则 compact 永远拖不动模型）', (
      WidgetTester tester,
    ) async {
      await pumpCompact(tester);

      StagePointerInterceptor interceptorOfPage() => tester.widget(
        find
            .descendant(
              of: find.byType(PageCrossFade),
              matching: find.byType(StagePointerInterceptor),
            )
            .first,
      );

      expect(interceptorOfPage().enabled, isFalse, reason: '收起时垫层还在吃舞台指针');

      await openSettings(tester);
      expect(interceptorOfPage().enabled, isTrue);

      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      expect(interceptorOfPage().enabled, isFalse);
    });
  });

  group('P2-1：medium / expanded 不受影响（只动 compact）', () {
    testWidgets('medium 仍然是底部浮层', (WidgetTester tester) async {
      await tester.binding.setSurfaceSize(const Size(1000, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(shell(input: TextEditingController()));
      await tester.pumpAndSettle();

      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(find.byType(BottomSheet), findsOneWidget);
      expect(find.byType(PageCrossFade), findsNothing);
    });

    testWidgets('expanded 仍然是内联侧板，且**没有** PageCrossFade', (
      WidgetTester tester,
    ) async {
      await tester.binding.setSurfaceSize(const Size(1400, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(shell(input: TextEditingController()));
      await tester.pumpAndSettle();

      await tester.tap(find.text('设置'));
      await tester.pumpAndSettle();
      expect(find.byType(CollapsiblePanel), findsWidgets);
      expect(find.byType(PageCrossFade), findsNothing);
      expect(find.byType(BottomSheet), findsNothing);
    });
  });
}
