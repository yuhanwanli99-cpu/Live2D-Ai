import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/ui/theme.dart';

void main() {
  final ThemeData theme = buildAppTheme();

  group('字体：必须解析到自托管子集（断网可用）', () {
    test('正文/标题/标签样式都用打包的中文字体', () {
      final TextTheme t = theme.textTheme;
      final Map<String, TextStyle?> styles = <String, TextStyle?>{
        'bodyMedium': t.bodyMedium,
        'bodySmall': t.bodySmall,
        'titleMedium': t.titleMedium,
        'labelLarge': t.labelLarge,
      };
      styles.forEach((String name, TextStyle? style) {
        expect(
          style?.fontFamily,
          kAppFontFamily,
          reason: '$name 没有用自托管字体——CanvasKit 取不到设备字体，会去 gstatic 下载',
        );
      });
    });

    test('CJK（ScriptCategory.dense）本地化之后仍然是自托管字体', () {
      // 这是整条字体修复里最容易**静默失效**的一环：
      // MaterialApp 会按 scriptCategory 取一套几何主题
      // （`Typography.dense` 里写死 `fontFamily: 'Roboto'`），再经
      // `ThemeData.localize` 合并进当前主题。若合并方向反了，中文就悄悄落回
      // Roboto → CanvasKit 找不到 CJK 字形 → 去 fonts.gstatic.com 下载
      // → 断网变豆腐块。整个「打包中文字体」的投入会因此白做，而且
      // **有网时完全看不出来**。
      final TextTheme dense = theme.typography.geometryThemeFor(
        ScriptCategory.dense,
      );
      final ThemeData localized = ThemeData.localize(theme, dense);
      expect(
        localized.textTheme.bodyMedium?.fontFamily,
        kAppFontFamily,
        reason: 'dense/CJK 本地化把字体覆盖回 Roboto 了——断网会缺字',
      );
      expect(localized.textTheme.titleMedium?.fontFamily, kAppFontFamily);
      expect(localized.textTheme.bodySmall?.fontFamily, kAppFontFamily);
    });

    test('是深色 Material 3（与设计基线一致）', () {
      expect(theme.brightness, Brightness.dark);
      expect(theme.useMaterial3, isTrue);
    });
  });

  group('pubspec 声明与主题一致（防重命名 / 漏资产）', () {
    // flutter test 的 cwd 就是包根，所以可以直接读源文件。
    final String pubspec = File('pubspec.yaml').readAsStringSync();

    test('用同一家族名声明了 400/700 两个字重', () {
      expect(
        pubspec.contains('family: $kAppFontFamily'),
        isTrue,
        reason: 'pubspec.yaml 的 fonts.family 与 kAppFontFamily 不一致',
      );
      expect(pubspec.contains('weight: 400'), isTrue);
      expect(pubspec.contains('weight: 700'), isTrue);
    });

    test('声明的字体资产真实存在于磁盘', () {
      for (final String f in const <String>[
        'assets/fonts/NotoSansSC-AiSubset-Regular.woff2',
        'assets/fonts/NotoSansSC-AiSubset-Bold.woff2',
      ]) {
        expect(pubspec.contains(f), isTrue, reason: 'pubspec 未声明 $f');
        expect(File(f).existsSync(), isTrue, reason: '资产文件缺失：$f');
      }
    });

    test('OFL 许可随附（OFL-1.1 条件 2：再分发必须带版权声明与许可）', () {
      expect(pubspec.contains('assets/fonts/OFL.txt'), isTrue);
      expect(File('assets/fonts/OFL.txt').existsSync(), isTrue);
    });

    test('子集体量守住下限——防退化成「只留 UI 用字」', () {
      // 本应用显示 LLM 的**任意**输出，子集必须覆盖整个 CJK 统一表意区；
      // 若有人改成「只保留界面文案里出现过的字」，体积会骤降到几百 KB，
      // 用户一看到人名/生僻词就变豆腐块。这条是那个反例的守卫。
      final int size = File(
        'assets/fonts/NotoSansSC-AiSubset-Regular.woff2',
      ).lengthSync();
      expect(
        size,
        greaterThan(2000000),
        reason: '子集过小（${size ~/ 1024} KB），疑似退化成只留 UI 用字',
      );
    });
  });
}
