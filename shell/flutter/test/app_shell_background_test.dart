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

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/sections/appearance_section.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 1×1 透明 PNG 的 dataURL（**真图**，别用假 base64——这里会真的过 Image.memory）。
const String _onePixelPng =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

/// 画得出来的一项（DEC-5 之后 `hasBackgroundAt` 要求**真 dataURL 形态**）。
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

/// 泵一个带探针 sectionBuilder 的外壳：探针把注入的运行时上下文读出来。
///
/// 探针必须是 `Builder`：`sectionBuilder` 拿到的 context 在 scope **之上**，
/// 只有它**返回的** widget 才在 scope 之下（生产路径同理）。
Future<void> _pumpProbe(
  WidgetTester tester, {
  required int backgroundIndex,
  required bool hydrating,
}) async {
  await tester.binding.setSurfaceSize(const Size(1400, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: AppShell(
        prefs: _prefs(withBackground: true),
        backgroundIndex: backgroundIndex,
        backgroundHydrating: hydrating,
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
        sectionBuilder: (BuildContext context, SettingsSection s) => Builder(
          builder: (BuildContext inner) {
            final BackgroundRuntimeScope? scope =
                BackgroundRuntimeScope.maybeOf(inner);
            final String probe = scope == null
                ? 'none'
                : '${scope.index}/${scope.hydrating}';
            return Text('probe:$probe');
          },
        ),
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

  group('DEC-6 / 9b：运行时上下文由外壳**注入**（不是设置页自己猜）', () {
    testWidgets('AppShell 把 index 与 hydrating 原样注入设置面板', (
      WidgetTester tester,
    ) async {
      await _pumpProbe(tester, backgroundIndex: 2, hydrating: true);
      expect(
        find.text('probe:2/true'),
        findsOneWidget,
        reason: '设置面板必须拿到「轮播到第几项」与「是否还在水合」；'
            '拿不到就会退回「永远第 0 项」那条老判据',
      );

      await _pumpProbe(tester, backgroundIndex: 0, hydrating: false);
      expect(find.text('probe:0/false'), findsOneWidget);
    });
  });

  group('F-0001-5：两条死通路与悬空头注都不许复活', () {
    test('（源码扫描）那两个回调参数与那个不存在的主机类名：全仓零命中', () {
      // 为什么是源码扫描：`lib/main.dart` 依赖 package:web，VM 测试里 import 不了，
      // 而「一个命名参数还在不在」本来就没有运行期表达。
      final Map<String, String> lib = <String, String>{
        for (final File f
            in Directory('lib')
                .listSync(recursive: true)
                .whereType<File>()
                .where((File f) => f.path.endsWith('.dart')))
          f.path: f.readAsStringSync(),
      };
      for (final String dead in <String>[
        'onBackgroundIndex',
        'onBackgroundJump',
        'ShellBackgroundHost',
      ]) {
        final List<String> hits = <String>[
          for (final MapEntry<String, String> e in lib.entries)
            if (e.value.contains(dead)) e.key,
        ];
        expect(
          hits,
          isEmpty,
          reason:
              '$dead 必须全仓零命中：它是「第四条索引通路」的幻觉'
              '（参数全仓零调用 / 类名根本不存在），留着会误导下一位改背景域的人',
        );
      }
    });
  });

  group('F-0001-2：预览跳转与运行时索引同步（接线面）', () {
    test('（源码扫描）_jumpBackground 真的推给轮播控制器，且不新增任何帧', () {
      final String src = File('lib/main.dart').readAsStringSync();
      final int start = src.indexOf('void _jumpBackground(');
      expect(start, greaterThan(-1), reason: '找不到 _jumpBackground');
      final int end = src.indexOf('\n  }', start);
      expect(end, greaterThan(start));
      final String body = src.substring(start, end);
      expect(
        body.contains('_slideshow.jumpTo('),
        isTrue,
        reason: '预览跳转必须**同步**轮播控制器的索引，否则下一次 onAdvance 会把'
            '用户刚预览的那张切走（F-0001-2）',
      );
      // 资产 / 协议红线：预览只是运行时索引，不许因此多发一帧。
      for (final String forbidden in <String>[
        'sendStageBg',
        'sendSync',
        'postMessage',
        'stage-bg',
        'Live2DStage',
        'add(',
      ]) {
        expect(
          body.contains(forbidden),
          isFalse,
          reason: '预览跳转不许碰渲染面 / 协议（命中了 $forbidden）——'
              '它只改运行时索引，一个字节都不下发',
        );
      }
    });

    test('（源码扫描）syncSlideshow 把运行时索引夹回控制器的范围', () {
      final String src = File('lib/main.dart').readAsStringSync();
      final int start = src.indexOf('void syncSlideshow(');
      expect(start, greaterThan(-1));
      final int end = src.indexOf('\n  }', start);
      final String body = src.substring(start, end);
      expect(
        body.contains('_slideshow.index'),
        isTrue,
        reason: '单源化：合法范围由控制器说了算，宿主那一份只跟随',
      );
      expect(
        body.contains('_backgroundIndex = clamped'),
        isTrue,
        reason: '库清空 / 减项之后必须把 _backgroundIndex 一起夹回来'
            '（否则「当前」标记与铺法区会指到夹持后的项，而索引本身没变）',
      );
    });
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
            BackgroundImage(id: 'x', dataUrl: 'data:image/png;base64,AAA'),
          ],
          backgroundOpacity: o,
        );
        expect(p.hasBackground, isTrue, reason: '不透明度 $o 时判成了「没有背景」');
      }
      // 只有 0 才是「显式关掉背景」的那个旋钮。
      final DisplayPrefs off = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[
          BackgroundImage(id: 'x', dataUrl: 'data:image/png;base64,AAA'),
        ],
        backgroundOpacity: 0,
      );
      expect(off.hasBackground, isFalse);
    });
  });
}
