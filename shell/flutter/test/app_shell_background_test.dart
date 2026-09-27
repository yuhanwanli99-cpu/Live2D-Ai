/// 壳根背景的**接线方向**（2026-09-27 补）。
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
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

void main() {
  /// `Scaffold.backgroundColor` 在这两种偏好下**应该**是什么。
  ///
  /// 直接从 `app_shell.dart` 的字面量上读，而不是把那段逻辑复制一份到测试里
  /// ——复制的话，改错了实现、测试还在绿，等于没钉。
  final String src = File('lib/app/app_shell.dart').readAsStringSync();

  test('有背景 → 脚手架底是 `Colors.transparent`（让背景透上来）', () {
    expect(
      src.contains(
        'backgroundColor: _hasBackground ? Colors.transparent : null',
      ),
      isTrue,
    );
  });

  test('没有背景 → `null`（退回不透明的面，与「只有底色」一致）', () {
    expect(
      src.contains(
        'backgroundColor: _hasBackground ? Colors.transparent : null',
      ),
      isTrue,
    );
  });

  test('**绝不能**写成反过来的方向（那会把背景整块盖死）', () {
    expect(
      src.contains(
        'backgroundColor: _hasBackground ? null : Colors.transparent',
      ),
      isFalse,
      reason:
          '`null` = ThemeData.scaffoldBackgroundColor（不透明），'
          '它会把 [ShellBackdrop] 整块盖掉 —— 这正是 2026-09-27 的那个 bug',
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
