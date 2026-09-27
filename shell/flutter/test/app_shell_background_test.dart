/// 壳根背景的**接线方向**（2026-09-27 补；2026-09-27 晚改成行为断言）。
///
/// # 为什么单独一个文件
///
/// 背景这条链有四层，每一层单看都对，**接错方向就不报错、只是「看不见」**：
///
/// ```text
/// ShellBackdrop（图 + 遮罩）
///   └─ Scaffold.backgroundColor   ← 必须是 transparent 才透得上来
///        └─ ChatPanel 的面        ← 读 AppColors.panelAlpha
///             └─ StagePointerInterceptor（舞台 iframe 在更上面）
/// ```
///
/// 2026-09-27 这一天里，这一层被写反过一次：
/// `_hasBackground ? null : Colors.transparent` —— 正好把「有图」和「透明」
/// 摆到了相反的分支上。`null` 会退回 `ThemeData.scaffoldBackgroundColor`
/// （= `palette.stage`，**不透明**），于是整块 Scaffold 把背景盖死，
/// 而**没有任何测试会红**（widget 树是合法的、颜色也是合法值）。
///
/// 只靠肉眼抓不到（本轮就是靠 A/B 截图的两态只差一个亮度档才发现）。
/// 所以这里把两个分支的方向**逐条钉死**。
///
/// # 为什么 pump 真 widget，而不是扫源码字符串
///
/// 初版断言 `src.contains('backgroundColor: _hasBackground ? Colors.transparent : null')`：
/// 它只证明**那行字还在**——实现被改坏、格式化一动、变量改名都会让断言与
/// 行为脱钩（而且两条测试查的是**同一个**字符串）。现在改成 pump [AppShell]
/// 之后读 `Scaffold.backgroundColor` 的**实际取值**：实现写反就当场红。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 1×1 透明 PNG 的 dataURL（**真图**，别用假 base64——这里会真的过 Image.memory）。
const String _onePixelPng =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

/// 画得出来的一项（`hasBackgroundAt` 只要求 dataUrl 非空）。
const BackgroundItem _item = BackgroundImage(id: 'a', dataUrl: _onePixelPng);

DisplayPrefs _prefs({required bool withBackground}) => withBackground
    ? const DisplayPrefs().copyWith(backgrounds: <BackgroundItem>[_item])
    : const DisplayPrefs();

Future<void> _pumpShell(
  WidgetTester tester, {
  required bool withBackground,
}) async {
  await tester.binding.setSurfaceSize(const Size(1400, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: AppShell(
        prefs: _prefs(withBackground: withBackground),
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

/// 外壳自己那个 `Scaffold` 的**实际**底色（`null` = 退回主题底）。
Color? _scaffoldBackground(WidgetTester tester) =>
    tester.widget<Scaffold>(find.byType(Scaffold).first).backgroundColor;

void main() {
  testWidgets('有背景 → 脚手架底是 `Colors.transparent`（让背景透上来）',
      (WidgetTester tester) async {
    await _pumpShell(tester, withBackground: true);
    expect(_scaffoldBackground(tester), Colors.transparent);
  });

  testWidgets('没有背景 → `null`（退回不透明的面，与「只有底色」一致）',
      (WidgetTester tester) async {
    await _pumpShell(tester, withBackground: false);
    expect(_scaffoldBackground(tester), isNull);
  });

  testWidgets('**绝不能**写成反过来的方向（没有背景却给 transparent）',
      (WidgetTester tester) async {
    await _pumpShell(tester, withBackground: false);
    expect(
      _scaffoldBackground(tester),
      isNot(Colors.transparent),
      reason: '这一态必须是 `null`（→ 不透明的 ThemeData.scaffoldBackgroundColor）；'
          '给了 transparent 说明两个分支写反了，有图时反而会盖死背景',
    );
  });

  test('`null` 确实是不透明的主题底（说明上面那条为什么致命）', () {
    final ThemeData theme = buildAppTheme(AppThemeId.black);
    expect(theme.scaffoldBackgroundColor.a, 1.0);
    expect(theme.scaffoldBackgroundColor, AppPalette.black.stage);
  });

  group('偏好侧：库里有图 ⇒ 一定判定为「有背景」', () {
    test('新用户加了图就可见（这条是本缺陷的守门人）', () {
      final DisplayPrefs p = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[
          const BackgroundImage(id: 'a', dataUrl: 'data:image/png;base64,AAA'),
          const BackgroundImage(id: 'b', dataUrl: 'data:image/png;base64,BBB'),
          const BackgroundImage(id: 'c', dataUrl: 'data:image/png;base64,CCC'),
        ],
      );
      expect(p.backgrounds.length, 3);
      expect(p.hasBackground, isTrue);
      expect(p.effectiveBackground, isNotNull);
    });

    test('**满不透明度依然算「有背景」**（判据是「画得出来」，不是透明度高低）',
        () {
      // 曾经这里多了一条 `opacity < 1`：滑杆推到 100% 面板就退回不透明，
      // 而设置页还写着「聊天面板会跟着透」——界面在骗人，且 99%→100% 会跳变。
      for (final double o in <double>[0.15, 0.5, 0.99, 1.0]) {
        final DisplayPrefs p = const DisplayPrefs().copyWith(
          backgrounds: <BackgroundItem>[
            BackgroundImage(id: 'x', dataUrl: 'x'),
          ],
          backgroundOpacity: o,
        );
        expect(p.hasBackground, isTrue, reason: '不透明度 $o 时判成了「没有背景」');
      }
      // 只有 0 才是「显式关掉背景」的那个旋钮。
      final DisplayPrefs off = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[
          BackgroundImage(id: 'x', dataUrl: 'x'),
        ],
        backgroundOpacity: 0,
      );
      expect(off.hasBackground, isFalse);
    });
  });
}
