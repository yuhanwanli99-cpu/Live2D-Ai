/// 「Mod 变更 → 需重新点火 / 重启后生效」统一提示的回归（L1 基座）。
///
/// 为什么值得单独测文案：这条提示是**用户唯一的处置线索**。它退化成
/// 「请重启」而没有入口、或者悄悄不提「重启」，用户在设置里点完就再也
/// 对不上账——比没有提示更坏。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/api/mods_api.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/restart_notice.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

void main() {
  group('文案', () {
    test('完整提示必须同时给出「重新点火」「重启」与可执行入口', () {
      final String text = modRestartNoticeText('已启用 persona');
      expect(text, contains('已启用 persona'));
      expect(text, contains('重新点火'));
      expect(text, contains('重启'));
      // 入口 = 命令本身（用户能直接复制去跑）。
      expect(text, contains(kIgniteCommand));
      // 还有第二条路：刷新页面。
      expect(text, contains('F5'));
    });

    test('SnackBar 短文案同样带入口，且是一行', () {
      final String text = modRestartSnackText('已清空记忆库');
      expect(text, contains('已清空记忆库'));
      expect(text, contains('重新点火'));
      expect(text, contains(kIgniteCommand));
      expect(text.contains('\n'), isFalse);
    });

    test('空动作不产出「。。」这种坏标点', () {
      final String text = modRestartNoticeText('   ');
      expect(text, startsWith('Mod 变更已提交。'));
      expect(text.contains('。。'), isFalse);
      expect(text, contains('重新点火'));
    });
  });

  group('ModsSection', () {
    testWidgets('有提示时把它渲染在 Mod 分区顶部，且可关闭', (WidgetTester tester) async {
      int dismissed = 0;
      await tester.pumpWidget(
        MaterialApp(
          // 必须用真主题：SectionHeader / InlineNotice 都从主题扩展里取 AppColors，
          // 裸 MaterialApp 会直接断言失败（与 mods_section_test 同一写法）。
          theme: buildAppTheme(),
          home: Scaffold(
            body: SingleChildScrollView(
              child: ModsSection(
                mods: const <ModInfo>[],
                loading: false,
                restartNotice: modRestartNoticeText('已启用 persona'),
                onDismissRestart: () => dismissed++,
              ),
            ),
          ),
        ),
      );
      expect(find.textContaining('重新点火'), findsOneWidget);
      await tester.tap(find.byTooltip('关闭提示'));
      await tester.pump();
      expect(dismissed, 1);
    });

    testWidgets('没有提示时不渲染任何提示', (WidgetTester tester) async {
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: const Scaffold(
            body: SingleChildScrollView(
              child: ModsSection(mods: <ModInfo>[], loading: false),
            ),
          ),
        ),
      );
      expect(find.textContaining('重新点火'), findsNothing);
    });
  });

  group('源码守卫（2026-10-09）：配置保存不再挂常驻提示，启停仍然挂', () {
    /// 判据必须落在**函数体**上。只查「整库有没有 `_notifyModChanged`」是
    /// 恒真的（启停那条路径也用它）；从调用点（`onSaveConfig: _saveModConfig`）
    /// 起切同样恒真。所以下面每条断言都带非平凡守卫。
    test('_saveModConfig 体内没有 _notifyModChanged，_toggleMod 体内仍有', () {
      final String src = stripCommentsKeepStrings(
        readLibrarySource('lib/main.dart'),
      );
      expect(
        src.contains('_notifyModChanged'),
        isTrue,
        reason: '整库都没有这个词 ⇒ 下面的「没有」是空转（不是「已移除」）',
      );

      final String? saveBody = closureBodyAfter(src, '_saveModConfig');
      final String? toggleBody = closureBodyAfter(src, '_toggleMod');
      expect(saveBody, isNotNull, reason: '切不出 _saveModConfig 的函数体');
      expect(toggleBody, isNotNull, reason: '切不出 _toggleMod 的函数体');

      // 非平凡守卫：切出来的确实是对应那一个函数（各认它自己调的 API）。
      expect(saveBody!, contains('_modsApi.setConfig'));
      expect(toggleBody!, contains('_modsApi.setEnabled'));

      expect(
        saveBody.contains('_notifyModChanged'),
        isFalse,
        reason: '保存配置成功不该挂「需重新点火」常驻条（服务端已经 restart 过）',
      );
      expect(
        toggleBody.contains('_notifyModChanged'),
        isTrue,
        reason: '启停是真的运行行为变更，必须继续挂常驻条',
      );
    });
  });
}
