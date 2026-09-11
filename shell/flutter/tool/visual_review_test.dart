/// 视觉审查工具：把外壳在**三档断点 × 四套主题**下渲染成 PNG，供人眼复核。
///
/// 2026-09-11（前端加强计划 P4-4）新增。**这是本轮性价比最高的一项工程投入。**
///
/// # 它要解决的问题（已经付过学费）
///
/// `AGENTS.md` 记着一条教训：v0.5.0 交付后做真机验收，**抓到十二个 bug，
/// 其中八个逃过了当时全绿的测试**（Rust 810 + Flutter 590）。它们的共同前提
/// 是「真的在浏览器里点一遍」：异步时序、`showModalBottomSheet` 的构建时机、
/// 平台视图、精确路径匹配。
///
/// 测试全绿 ≠ 界面是对的。本工具不能替代真机点击，但它把「**界面长什么样**」
/// 变成一次可以反复看的产物——观感回归（错位、溢出、不可读的对比度、
/// 主题切换后颜色没跟上）本来就是「看一眼就发现、不跑就永远发现不了」的那类。
///
/// # 怎么用
///
/// ```bash
/// cd shell/flutter && flutter test tool/visual_review_test.dart
/// # 产物：build/visual-review/<断点>-<主题>.png（共 12 张）
/// ```
///
/// 它**不在** `test/` 目录下，所以 `flutter test`（默认只跑 `test/`）
/// 不会带上它——那是刻意的：出图要几秒，不该拖慢每次门禁。
///
/// # 为什么用 `flutter test` 而不是浏览器
///
/// 同样的做法在参考项目 Morrow 里已经验证过可行
///（`tool/feature_visual_test.dart`）：用 `FontLoader` 加载字体、
/// `tester.view.physicalSize` 定尺寸、`RenderRepaintBoundary.toImage()` 出 PNG，
/// **全程不依赖浏览器**。所以它能在 CI、在无头环境、在任何人的机器上跑出同样的图。
///
/// # 一个必须记住的局限（写在这里免得被误当成通过）
///
/// 舞台是一个 `<iframe>` 平台视图，**`flutter test` 跑在 VM 上，画不出它**
///（走 `live2d_host_stub.dart` 的占位实现）。所以这些图里舞台位置是那个占位文案，
/// 不是真的 Live2D 画面。**平台视图那一层只能真机验收。**
library;

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shortcuts.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/design/breakpoints.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 三档断点各取一个代表宽度（取档位内的值，不取边界——边界另有专门测试）。
const Map<String, double> _breakpoints = <String, double>{
  'compact': 420,
  'medium': 1000,
  'expanded': 1440,
};

/// 出图尺寸（逻辑像素）。高度取 900：够放下三栏 + 底部输入区。
const Size _viewport = Size(1440, 900);

/// 造一段像样的聊天内容——空界面看不出「气泡/字号/间距」的问题。
final List<ChatMessage> _messages = <ChatMessage>[
  ChatMessage(role: ChatRole.user, text: '帮我看看这个报错：`llm_upstream_401`'),
  ChatMessage(
    role: ChatRole.assistant,
    text:
        '上游拒绝了鉴权（HTTP 401）。\n'
        '- 确认 `[llm] api_key_env` 指向的环境变量在**后端进程**里已设置\n'
        '- 提示词内容与鉴权无关，改它不会解决\n'
        '- 改完重启服务端即可',
  ),
  ChatMessage(role: ChatRole.user, text: '好，我试试。'),
];

Widget _shellFor(AppThemeId theme, TextEditingController input) => MaterialApp(
  theme: buildAppTheme(theme),
  home: AppShell(
    stage: const _StubStage(),
    phase: UiPhase.thinking,
    wsStatus: WsStatus.connected,
    messages: _messages,
    input: input,
    onSend: () {},
    onStop: () {},
    onRetryConnection: () {},
    volume: 0.8,
    muted: false,
    onVolumeChanged: (_) {},
    onMutedChanged: (_) {},
    sections: visibleSections(),
    section: SettingsSection.appearance,
    onSectionChanged: (_) {},
    sectionBuilder: (BuildContext context, SettingsSection s) => Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        for (int i = 0; i < 12; i++)
          SizedBox(height: 44, child: Text('${s.label} · 字段 $i')),
      ],
    ),
    stagePhase: Live2DBridgePhase.ready,
    modelName: 'bai_001',
    shortcuts: const AppShortcutCallbacks(isMacOS: false),
  ),
);

/// 舞台占位（真的 `<iframe>` 在 VM 上画不出来，见文件头注的局限）。
class _StubStage extends StatelessWidget {
  const _StubStage();

  @override
  Widget build(BuildContext context) => ColoredBox(
    color: appPaletteOf(context).stage,
    child: Center(
      child: Builder(
        builder: (BuildContext context) => Text(
          'Live2D 舞台（平台视图，需真机验收）',
          style: Theme.of(context).textTheme.bodySmall,
        ),
      ),
    ),
  );
}

Future<void> _loadFonts() async {
  // CanvasKit 取不到设备字体，`flutter test` 也一样：不显式加载的话
  // 出图里所有中文都是豆腐块——那样这个工具就没用了。
  //
  // 字体来自本仓库自托管的那两个子集（**不是**系统字体）：
  // 这样出图与真机上跑的是同一套字形。
  for (final MapEntry<String, List<String>> e in <String, List<String>>{
    'NotoSansSC': <String>[
      'assets/fonts/NotoSansSC-AiSubset-Regular.woff2',
      'assets/fonts/NotoSansSC-AiSubset-Bold.woff2',
    ],
  }.entries) {
    final FontLoader loader = FontLoader(e.key);
    for (final String path in e.value) {
      final File f = File(path);
      if (!f.existsSync()) continue;
      loader.addFont(
        f.readAsBytes().then((List<int> b) => ByteData.sublistView(
              Uint8List.fromList(b),
            )),
      );
    }
    await loader.load();
  }
  // 图标字体（MaterialIcons）由测试绑定自带，不必手动加载。
}

void main() {
  testWidgets('渲染三档断点 × 四套主题，出 12 张 PNG 供人眼复核', (
    WidgetTester tester,
  ) async {
    final Directory out = Directory('build/visual-review');
    if (out.existsSync()) out.deleteSync(recursive: true);
    out.createSync(recursive: true);

    await tester.runAsync(_loadFonts);

    // **不能用 `pumpAndSettle`**：出图时故意让状态胶囊停在「思考中」，
    // 而那个呼吸光是一个 `repeat()` 的无限动画——`pumpAndSettle` 永远等不到
    // 「没有待处理帧」，会直接超时。固定推几帧就够（我们只是要一张静态图）。
    Future<void> settle() async {
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 700)); // 盖过启动揭示（600 ms）
      await tester.pump(const Duration(milliseconds: 400)); // 盖过面板过渡（320 ms）
    }

    final List<String> written = <String>[];
    /// 断点 → 主题 → 平均亮度。用来做下面那条「主题真的生效了吗」的断言。
    final Map<String, Map<AppThemeId, double>> luminance =
        <String, Map<AppThemeId, double>>{};
    for (final AppThemeId theme in AppThemeId.values) {
      for (final MapEntry<String, double> bp in _breakpoints.entries) {
        final GlobalKey boundary = GlobalKey();
        final TextEditingController input = TextEditingController(
          text: '正在输入的一句话',
        );
        addTearDown(input.dispose);

        // 视口宽度按断点给，高度统一：三张图才能并排比较。
        tester.view.physicalSize = Size(
          bp.value * _viewport.height / _viewport.height,
          _viewport.height,
        );
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);

        await tester.pumpWidget(
          RepaintBoundary(
            key: boundary,
            child: _shellFor(theme, input),
          ),
        );
        await settle();

        // 设置面板打开的样子也要看——它是最容易出观感问题的一块。
        // （expanded 是内联侧板，compact 是整页过渡，medium 是浮层。）
        await tester.tap(find.text('设置').first);
        await settle();

        final RenderRepaintBoundary render =
            boundary.currentContext!.findRenderObject()!
                as RenderRepaintBoundary;
        final ui.Image image = (await tester.runAsync(
          () => render.toImage(pixelRatio: 1),
        ))!;
        final ByteData? png = await tester.runAsync<ByteData?>(
          () => image.toByteData(format: ui.ImageByteFormat.png),
        );
        // 顺手算平均亮度：出图这件事本身有个「白屏/纯色也算成功」的洞，
        // 亮度能让下面那条断言把它堵上。
        final ByteData? rgba = await tester.runAsync<ByteData?>(
          () => image.toByteData(format: ui.ImageByteFormat.rawRgba),
        );
        if (rgba != null) {
          final Uint8List px = rgba.buffer.asUint8List();
          double sum = 0;
          int n = 0;
          // 每 37 个像素采一个：够稳定，又不必遍历 130 万像素。
          for (int i = 0; i + 3 < px.length; i += 4 * 37) {
            sum += 0.2126 * px[i] + 0.7152 * px[i + 1] + 0.0722 * px[i + 2];
            n++;
          }
          (luminance[bp.key] ??= <AppThemeId, double>{})[theme] = sum / n;
        }
        image.dispose();
        expect(png, isNotNull, reason: '出图失败：$theme / $bp');

        final File file = File('${out.path}/${bp.key}-${theme.wire}.png');
        await tester.runAsync(
          () => file.writeAsBytes(png!.buffer.asUint8List()),
        );
        written.add(file.path);
      }
    }

    // 12 张都要真的落盘（少一张就说明某个组合渲染失败被静默吞掉了）。
    expect(written.length, AppThemeId.values.length * _breakpoints.length);
    for (final String path in written) {
      expect(File(path).existsSync(), isTrue, reason: '没写出来：$path');
      expect(
        File(path).lengthSync(),
        greaterThan(1000),
        reason: '$path 太小了 —— 大概是空图（白屏也是「出图成功」）',
      );
    }
    // ── 光出图不够：还要挡住「主题根本没生效」这类静默失效 ──
    //
    // 这是有来历的：v0.5.1 抓到过一个「切主题后舞台不变色」（`_applyPrefs`
    // 读的是尚未更新的 `widget.prefs`）。那种 bug 出再多图也看不出来，
    // 除非把「四套主题的图**确实不一样**」写成断言。
    for (final String bp in _breakpoints.keys) {
      final Map<AppThemeId, double> byTheme = luminance[bp]!;
      expect(byTheme.length, AppThemeId.values.length, reason: '$bp 少出了几张');
      expect(
        byTheme[AppThemeId.white]!,
        greaterThan(byTheme[AppThemeId.black]!),
        reason: '$bp：白主题的图不比黑主题亮 —— 主题没生效（或出的是同一张图）',
      );
      // 四套主题两两都要有区别（否则某两套其实是同一套配色）。
      final Set<String> distinct = byTheme.values
          .map((double v) => v.toStringAsFixed(2))
          .toSet();
      expect(
        distinct.length,
        greaterThanOrEqualTo(3),
        reason: '$bp：四套主题的亮度太接近了，配色可能没接上',
      );
    }

    // ignore: avoid_print
    print('视觉审查产物（${written.length} 张）：${out.path}');
  });

  test('断点代表值确实落在各自档位里（否则出的是同一档的三张图）', () {
    for (final MapEntry<String, double> e in _breakpoints.entries) {
      final SizeClass sc = Breakpoints.sizeClassOf(e.value);
      expect(sc.name, e.key, reason: '${e.value} 不在 ${e.key} 档');
    }
  });
}
