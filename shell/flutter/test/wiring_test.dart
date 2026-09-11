import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';
import 'package:live2d_ai_shell/settings/sections/persona_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// **接线守卫**：盯住「代码写好了但没人用」这一类静默失效。
///
/// # 为什么需要这个文件（有真实来历）
///
/// P6 收口时我用「在构建产物 `main.dart.js` 里搜特征字符串」的办法核对交付，
/// 结果发现三处**功能存在于源码、但应用里根本点不到**：
///
/// 1. `StageHost`（P6 的舞台语义 + 加载/错误覆盖层）**没有任何调用点** ——
///    因为 `main.dart` 直接把裸 `Live2DStage` 交给了外壳；
/// 2. `shortcutHelp()`（快捷键帮助）**没有任何调用点** —— 内容被 dart2js
///    当死代码裁掉，`Ctrl+/` 什么都不会发生；
/// 3. `PersonaSection.onImport` **声明了却从没被调用** —— 角色卡导入
///    （P4 的差异化功能）在界面上没有按钮。
///
/// 三者都**编译通过、测试全绿**——因为没有测试问「它被用上了吗」。
/// 这个文件就是那个问题。
///
/// # 为什么不做成通用「死代码检测」
///
/// 通用检测要么靠反射（Web 上不可用），要么误报一片（公开 API 本来就没人调）。
/// 这里只**逐个点名**那些「一旦没接线就是用户可见功能缺失」的东西——
/// 每加一条都要写清为什么它必须被用上。
void main() {
  /// 某个标识符是否在 `lib/**` 里被**本文件之外**的地方引用过。
  bool referencedOutside(String symbol, String ownFile) {
    final List<File> sources = Directory('lib')
        .listSync(recursive: true)
        .whereType<File>()
        .where((File f) => f.path.endsWith('.dart'))
        .where((File f) => !f.path.endsWith(ownFile))
        .toList();
    for (final File file in sources) {
      if (file.readAsStringSync().contains(symbol)) return true;
    }
    return false;
  }

  group('必须被接线的构造（点名，每个都写清后果）', () {
    test('`StageHost` 被外壳用上（否则舞台语义与加载/错误覆盖层全部失效）', () {
      expect(
        referencedOutside('StageHost(', 'lib/ui/stage_host.dart'),
        isTrue,
        reason: '没有调用点 = P6 的「Live2D 舞台」语义标签、'
            '「模型加载中/加载失败 + 重试」覆盖层都不在成品里',
      );
    });

    test('`shortcutHelp` 被用上（否则 Ctrl+/ 什么都不会发生）', () {
      expect(
        referencedOutside('shortcutHelp(', 'lib/app/app_shortcuts.dart'),
        isTrue,
        reason: '定义了却没入口 → 被 dart2js 当死代码裁掉，用户按了没反应',
      );
    });

    test('`applyPersonaImport` 被用上（否则角色卡导入点了没反应）', () {
      expect(
        referencedOutside(
          'applyPersonaImport(',
          'lib/settings/sections/persona_section.dart',
        ),
        isTrue,
      );
    });

    test('`LiveRegionThrottle` 被用上（否则流式播报根本不会挂）', () {
      expect(
        referencedOutside('LiveRegionThrottle(', 'lib/state/live_region.dart'),
        isTrue,
      );
    });

    test('`EpochGate` 的判据被 `ChatController` 用上（钉子 13 真的生效）', () {
      expect(
        referencedOutside('mustInterruptAudio(', 'lib/audio/epoch_gate.dart'),
        isTrue,
      );
      expect(
        referencedOutside('decideAudioFrame(', 'lib/audio/epoch_gate.dart'),
        isTrue,
      );
    });
  });

  group('PersonaSection 的导入入口**真的画出来了**', () {
    Widget wrap(Widget child) => MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(body: SingleChildScrollView(child: child)),
    );

    /// 最小可渲染的设置控制器（只要 `loaded` 为真、`draft` 可用即可）。
    SettingsView emptyView() => SettingsView.fromJson(const <String, Object?>{});

    testWidgets('有「选择角色卡文件」按钮，且点击会触发 onImport', (
      WidgetTester tester,
    ) async {
      int taps = 0;
      await tester.pumpWidget(
        wrap(
          PersonaSection(
            controller: _StubController(),
            view: emptyView(),
            devMode: false,
            onImport: (_) => taps++,
          ),
        ),
      );
      final Finder button = find.textContaining('选择角色卡文件');
      expect(button, findsOneWidget, reason: '没有按钮 = 用户点不到导入功能');
      // 角色卡分区很长，按钮在 800x600 的测试视口里在折叠线以下 → 先滚进来。
      await tester.ensureVisible(button);
      await tester.pump();
      await tester.tap(button);
      expect(taps, 1);
    });

    testWidgets('没有 onImport 时按钮禁用（而不是点了没反应）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          PersonaSection(
            controller: _StubController(),
            view: emptyView(),
            devMode: false,
          ),
        ),
      );
      final Finder button = find.textContaining('选择角色卡文件');
      await tester.ensureVisible(button);
      await tester.pump();
      final OutlinedButton btn = tester.widget<OutlinedButton>(
        find.ancestor(of: button, matching: find.byType(OutlinedButton)),
      );
      expect(btn.onPressed, isNull);
    });
  });
}

/// 只满足渲染需要的桩：**不碰网络**（`PersonaSection` 只读 `draft`）。
class _StubController extends SettingsController {
  _StubController() : super(api: ApiClient());
}
