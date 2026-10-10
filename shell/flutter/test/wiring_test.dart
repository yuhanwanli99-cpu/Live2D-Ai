import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';
import 'package:live2d_ai_shell/settings/sections/persona_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/dart_library.dart';

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
///    （2026-09-13 M5.1：角色卡导入整条迁到标准 Mod，这条守卫与函数一起删除；
///    2026-10-07 T6：导入入口按管理员口径**接回人设页**，于是下面那组断言
///    改成「两个导入按钮真的在、且语义分得开」——历史教训不变。）
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

  // 2026-10-10：本机偏好**草稿化**之后，凡是「改一个字段」的回调都必须从
  // 草稿（`_shownPrefs`）起稿。从已保存的 `widget.prefs` 起稿会把用户在这一轮
  // 里已经改过的其它草稿字段整份冲掉——实测「拨到 16K 再拖音量」→ 档位回 4K；
  // 「先静音再拖音量」→ 静音被清。这是纯接线缺陷，只有真机/源码守得住。
  group('本机偏好草稿：音量 / 静音从草稿起稿', () {
    test('onVolumeChanged / onMutedChanged 从 _shownPrefs.copyWith 起，不得用 widget.prefs', () {
      // `main.dart` 声明了多个 `part` ⇒ 必须走 readLibrarySource（读并集），
      // 否则 `dart_library_guard_test` 判红，且扫描会漏掉 part 里的接线。
      final String src = readLibrarySource('lib/main.dart');
      for (final String key in <String>['onVolumeChanged:', 'onMutedChanged:']) {
        final int i = src.indexOf(key);
        expect(i, greaterThanOrEqualTo(0), reason: '$key 必须仍在组合根接线');
        final String snippet = src.substring(
          i,
          (i + 180).clamp(0, src.length),
        );
        expect(
          snippet.contains('_shownPrefs.copyWith'),
          isTrue,
          reason:
              '$key 必须从草稿起稿（_shownPrefs.copyWith），否则会把用户已改过的'
              '其它本机草稿（档位 / 主题 / 背景…）整份冲掉',
        );
        expect(
          snippet.contains('widget.prefs.copyWith'),
          isFalse,
          reason: '$key 不得从已保存的 widget.prefs 起稿（会冲掉草稿）',
        );
      }
    });
  });

  group('必须被接线的构造（点名，每个都写清后果）', () {
    test('`StageHost` 被外壳用上（否则舞台语义与加载/错误覆盖层全部失效）', () {
      expect(
        referencedOutside('StageHost(', 'lib/ui/stage_host.dart'),
        isTrue,
        reason:
            '没有调用点 = P6 的「Live2D 舞台」语义标签、'
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

    // 2026-10-08：角色卡导入属**扩展**——通道由组合根接到扩展卡片
    // （`ModsSection.onCommand` + `pickCardFile` + `activeSessionId`），
    // 人设页不再有任何导入回调。这里守的是「适配器真的接上了」，
    // 而不是「控件画出来了」——后者正是本文件第 3 条历史教训的坑。
    test('角色卡导入通道由组合根接到扩展卡片（persona + 会话 + 文件选择器）', () {
      final String wiring = File(
        'lib/app/shell_settings.dart',
      ).readAsStringSync();
      expect(wiring, contains('ModsSection('));
      expect(
        wiring,
        contains('pickCardFile: pickPersonaCardFile'),
        reason: 'PNG 卡文件选择器必须仍然只从组合根注入',
      );
      expect(
        wiring,
        contains('activeSessionId: _chat.sessions.activeId'),
        reason: '会话作用域默认落在用户正看着的那个会话上',
      );
      expect(
        wiring.contains('onImportCard'),
        isFalse,
        reason: '人设页不再有命令通道（导入回到扩展卡片一处）',
      );
    });
  });

  group('PersonaSection：只剩主链两项（导入回到扩展卡片）', () {
    late _StubController controller;

    setUp(() {
      controller = _StubController();
    });

    Widget wrap(Widget child) => MaterialApp(
      theme: buildAppTheme(),
      home: Scaffold(body: SingleChildScrollView(child: child)),
    );

    /// 最小可渲染的设置视图（只要 `persona` 段可用即可）。
    SettingsView emptyView() =>
        SettingsView.fromJson(const <String, Object?>{});

    Widget section({bool devMode = false}) => wrap(
      PersonaSection(
        controller: controller,
        view: emptyView(),
        devMode: devMode,
      ),
    );

    testWidgets('两个字段在；导入控件一个都不在', (WidgetTester tester) async {
      await tester.pumpWidget(section());
      expect(find.text('系统提示词'), findsOneWidget);
      expect(find.text('记住几轮对话'), findsOneWidget);
      // 酒馆卡字段已迁到标准 Mod，主链分区里不该再把它们画回来。
      for (final String gone in <String>['名称', '描述', '性格', '场景', '开场白']) {
        expect(find.text(gone), findsNothing, reason: '$gone 不该还在主链人设分区');
      }
      // 2026-10-08：导入 / 清除 / 粘贴框全部搬回扩展卡片。
      for (final String gone in <String>[
        '导入并生效',
        '导入为全局人设',
        '选择 PNG 角色卡文件',
        '粘贴角色卡 JSON',
        '清除当前会话的卡',
        '清除全局导入卡',
      ]) {
        expect(
          find.textContaining(gone),
          findsNothing,
          reason: '人设页不得再有「$gone」',
        );
      }
    });

    testWidgets('记住几轮对话始终可见；开发模式也不再写配置键名（2026-10-09 全删）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(section());
      expect(find.text('记住几轮对话'), findsOneWidget);
      expect(find.textContaining('max_history_pairs'), findsNothing);

      await tester.pumpWidget(section(devMode: true));
      expect(find.text('记住几轮对话'), findsOneWidget);
      // 控件说明（原来是 devMode 下的「写进配置的键是 max_history_pairs」）已删。
      expect(find.textContaining('max_history_pairs'), findsNothing);
    });
  });

}

/// 只满足渲染与「有没有回读设置」的桩：**不碰网络**。
class _StubController extends SettingsController {
  _StubController() : super(api: ApiClient());

  /// `load()` 被调用了几次（T6：只有全局导入 + 草稿干净才该回读）。
  int loadCalls = 0;

  /// 测试用的 dirty 覆盖（真实现要 `_remote != null` 才可能 dirty）。
  bool dirtyOverride = false;

  @override
  bool get dirty => dirtyOverride;

  @override
  Future<void> load() async {
    loadCalls++;
  }
}
