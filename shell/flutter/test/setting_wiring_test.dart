/// **断链守卫**：UI 暴露的每一个枚举值，都必须在渲染层有一条**真正不同**的分支。
///
/// # 为什么需要这个文件
///
/// 2026-09-27 一天之内抓到了三个同一类的缺陷，全部是「合法但无效」：
///
/// | 缺陷 | 症状 |
/// | --- | --- |
/// | `Scaffold.backgroundColor` 两分支写反 | 有背景也看不见 |
/// | `slideTransition` 字段只写不读 | UI 有「无 / 淡入」，但没有任何代码读它 |
/// | `imageFit == 2`「拉伸」 | `boxFitFor(2)` 与 `0` 是同一条分支，按了没区别 |
///
/// 前两个是**代码方向**错，第三个是**枚举没有对应实现**——后者最隐蔽：
/// 值能选、能存、能读回来，只是读到之后走的是别人的分支。
///
/// 所以这里不测「字段被读了没有」（那是 grep 能做的），而测
/// **「UI 给出的每个值，渲染结果两两不同」**。
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/ui/background_logic.dart';
import 'package:live2d_ai_shell/ui/shell_backdrop.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

/// 外观分区的源码 = 库文件 + 它 `part` 进来的每个文件。
///
/// 抽取是「只搬不改」：这些源码扫描断言关心的是「外观分区里有没有这个东西」，
/// 所以扫描面取**并集**——抽取前后断言强度不变。2026-10-06（R4-T2）起改用
/// [readLibrarySource]：背景域从 1 个 part 变成 3 个，手写相加必漏。
String appearanceSectionSource() =>
    readLibrarySource('lib/settings/sections/appearance_section.dart');

void main() {
  group('铺法 / 遮罩 / 位置：渲染函数仍在，界面旋钮已删（2026-10-07）', () {
    test('boxFitFor 的四档给出**不同**的结果', () {
      expect(boxFitFor(0), BoxFit.cover);
      expect(boxFitFor(1), BoxFit.contain);
      expect(boxFitFor(2), BoxFit.fill);
      expect(boxFitFor(3), BoxFit.none);
      expect(
        <BoxFit>{boxFitFor(0), boxFitFor(1), boxFitFor(2), boxFitFor(3)}.length,
        4,
        reason: '有档位落到同一条渲染路径上就是「按了没区别」',
      );
    });

    test('scrimAlphaFor 四档两两不同（自动 / 无 / 轻 / 重）', () {
      final Map<int, double> seen = <int, double>{
        for (final int level in <int>[0, 1, 2, 3])
          level: scrimAlphaFor(imageOpacity: 0.6, level: level),
      };
      expect(seen.values.toSet().length, 4, reason: '有档位没有独立效果：$seen');
    });

    test('自动档随图变实、随界面变透而加强（两个真实自变量）', () {
      expect(
        scrimAlphaFor(imageOpacity: 0.6, level: 0),
        greaterThan(scrimAlphaFor(imageOpacity: 0.1, level: 0)),
        reason: '图越实，压得越狠',
      );
      expect(
        scrimAlphaFor(imageOpacity: 0.6, level: 0, uiTransparency: 1),
        greaterThan(scrimAlphaFor(imageOpacity: 0.6, level: 0)),
        reason: '面板越透，背景要压得越多，否则是浅底上的浅灰字',
      );
      // 自动档必须永远强于「无」——否则调它等于关掉它。
      expect(
        scrimAlphaFor(imageOpacity: 0.05, level: 0),
        greaterThan(scrimAlphaFor(imageOpacity: 0.05, level: ScrimLevel.none)),
      );
    });

    test('alignmentFor 九档给出 9 个不同的对齐', () {
      final Set<Alignment> seen = <Alignment>{
        for (int i = 0; i < 9; i++) alignmentFor(i),
      };
      expect(seen.length, 9);
    });

    test('界面里不再有铺法 / 遮罩档 / 位置垫这些控件（注释不算）', () {
      final String ui = stripCommentsKeepStrings(appearanceSectionSource());
      expect(
        ui.contains('铺法（图）'),
        isFalse,
        reason: '四档铺法是渲染面能力，不再是用户旋钮',
      );
      for (final int level in <int>[
        ScrimLevel.auto,
        ScrimLevel.none,
        ScrimLevel.light,
        ScrimLevel.heavy,
      ]) {
        expect(
          ui.contains('FieldOption<int>(value: $level,'),
          isFalse,
          reason: '遮罩档 $level 不该再出现在设置里',
        );
      }
      expect(
        ui.contains('bg-align-pad'),
        isFalse,
        reason: '位置垫（kBackgroundAlignPadKey 的键值）已删',
      );
      expect(ui.contains('kBackgroundAlignPadKey'), isFalse);
    });
  });

  group('每个背景字段都必须有渲染路径读它', () {
    test('（源码扫描）字段名在 lib/ 里除定义与 UI 之外还有读者', () {
      // 这是 grep 层面的「有没有人读」，与上面的「读到之后有没有区别」互补。
      const Map<String, String> fields = <String, String>{
        'backgroundOpacity': 'image layer opacity',
        'slideInterval': 'timer period',
        'slideRandom': 'random order',
        'uiTransparency': 'panelAlpha',
      };
      final Map<String, String> lib = <String, String>{
        for (final File f
            in Directory('lib')
                .listSync(recursive: true)
                .whereType<File>()
                .where((File f) => f.path.endsWith('.dart')))
          f.path: f.readAsStringSync(),
      };
      for (final MapEntry<String, String> e in fields.entries) {
        final List<String> readers = <String>[
          for (final MapEntry<String, String> f in lib.entries)
            // 排除**整个**偏好库（库 + 它的 part）：2026-10-06 拆 part 后
            // 只排除 display_prefs.dart 会把 5 个 part 里的定义当成「外部读者」，
            // 于是一个只写不读的死字段也能骗过这条守卫。
            if (!f.key.contains('display_prefs') &&
                !f.key.contains('/settings/sections/') &&
                RegExp('\\b${e.key}\\b').hasMatch(f.value))
              f.key,
        ];
        expect(
          readers,
          isNotEmpty,
          reason: '${e.key}（${e.value}）**只写不读** —— 是个死字段',
        );
      }
    });
  });

  group('2026-09-27 减法：删掉的字段不许复活', () {
    // 这几条是被用户口径删掉的（web 端无需繁杂设置 / 舞台跟着换直接删）。
    // 留一条「不许复活」的断言，是因为「重新加回去」是这类减法最常见的回退。
    // 库 + parts：`display_prefs` 2026-10-06 拆成 codec / derived / playlist
    // 三个 part，只扫库文件会让这条「不许复活」守卫静默失效。
    final String prefs = readLibrarySource('lib/settings/display_prefs.dart');
    for (final String dead in <String>[
      'radiusScale', // 圆角幅度 → 固定为 AppMaterial.kFixedRadiusScale
      'stageFollowsSlide', // 舞台跟着换 → 整条删除
      'slideTransition', // 换图过渡 → 从没被读过，是死控件
    ]) {
      test('`$dead` 不再出现在偏好里', () {
        expect(
          prefs.contains('this.$dead') || prefs.contains('$dead ='),
          isFalse,
          reason:
              '$dead 已在 2026-09-27 的减法里删掉；'
              '要加回来先说清它解决了什么问题',
        );
      });
    }

    /// 2026-10-07 外观卡片简化删掉的字段：构造函数里不得再声明它们。
    for (final String dead in <String>[
      'edgeStrength',
      'imageFit',
      'imageAlign',
      'tileSize',
      'backgroundBlur',
      'backgroundScrim',
      'backgroundSource',
      'stageImage',
      'stagePlaylist',
    ]) {
      test('已删字段 `this.$dead` 不许出现在偏好构造函数里', () {
        expect(
          prefs.contains('this.$dead'),
          isFalse,
          reason:
              '$dead 已在 2026-10-07 的外观收口里删除：'
              '产品路径固定铺满 / 居中 / 不模糊 / 遮罩自动，舞台图 = 背景库投影',
        );
      });
    }

    test('圆角现在固定在一个数上，且所有表面都走那个入口', () {
      expect(AppMaterial.kFixedRadiusScale, 1.0);
      expect(buildAppTheme().extension<AppColors>()!.radiusScale, 1.0);
      expect(buildAppTheme().extension<AppColors>()!.radius(20), 20);
    });
  });

  group('界面透明程度：面板真的有变透明（2026-09-27 修完的真缺陷）', () {
    test('panelAlpha 从 1.0 走到 0.55，且中途单调', () {
      expect(AppColors.panelAlphaFor(0), 1.0);
      double prev = 1.0;
      for (double t = 0; t <= 1.0; t += 0.05) {
        final double a = AppColors.panelAlphaFor(t);
        expect(a, lessThanOrEqualTo(prev + 1e-9));
        expect(a, greaterThanOrEqualTo(AppColors.kMinPanelAlpha));
        prev = a;
      }
    });

    test('**没有不透明的面把半透明的面盖住**（那等于滑杆没接）', () {
      // 原缺陷：用户拖「界面透明程度」，设置面板一个像素都不变。
      // 根因是外层宿主套了一张**完全不透明**的面：
      //   - inline 侧板 `InlineSettingsDock` 的 `colorScheme.surface`；
      //   - medium 浮层 `bottomSheetTheme.backgroundColor`。
      // 里面那张 `SettingsScaffold` 确实读了 panelAlpha，但它被彻底遮住。
      //
      // 这条断言读**源码**而不是渲染树：那种 bug 在 widget 测试里完全看不出来
      // （两张面都存在，只是上面那张是实的）。
      final String navHost = _codeOf('lib/app/nav_host.dart');
      final int dock = navHost.indexOf('class InlineSettingsDock');
      expect(dock, greaterThan(0), reason: '类被改名/挪走了，这条断言要跟着改');
      final String dockBody = navHost.substring(dock);
      expect(
        dockBody.contains('colorScheme.surface'),
        isFalse,
        reason: '外层再套一张不透明的面 ⇒「界面透明程度」对设置面板无效',
      );
      expect(
        dockBody.contains('Colors.transparent'),
        isTrue,
        reason: '底色归 SettingsScaffold 独占，这里应当是透明的',
      );

      // 判据是「浮层宿主与里面的面板读**同一个** panelAlpha」，
      // 而不是某个具体数值——那样才与用户的滑杆位置无关。
      for (final double t in <double>[0, 0.5, 1]) {
        final AppMaterial m = AppMaterial(uiTransparency: t);
        final double expected = AppColors.panelAlphaFor(t);
        final ThemeData theme = buildAppTheme(AppThemeId.fallback, m);
        expect(
          theme.bottomSheetTheme.backgroundColor!.a,
          closeTo(expected, 1e-9),
          reason: '浮层宿主在界面透明 $t 时没有跟上',
        );
        expect(
          theme.dialogTheme.backgroundColor!.a,
          closeTo(expected, 1e-9),
        );
        expect(
          theme.extension<AppColors>()!.panelAlpha,
          closeTo(expected, 1e-9),
        );
      }
      // 拉满时它们**真的**是半透明的（不是名义上读了那个数）。
      final AppMaterial maxed = AppMaterial(uiTransparency: 1);
      expect(
        buildAppTheme(AppThemeId.fallback, maxed)
            .bottomSheetTheme
            .backgroundColor!
            .a,
        AppColors.kMinPanelAlpha,
      );
    });

    test('默认档就**不是**全不透明（否则背景加了等于没加）', () {
      // 这是「背景做了和没做一样」的真正原因：背景铺在壳根，
      // 而壳根上面压着 AppBar / 聊天面板 / 设置面板，全是面。
      // 界面透明程度为 0 时那些面完全不透明，背景只在缝隙里露几个像素。
      expect(DisplayPrefs.defaultUiTransparency, greaterThan(0.0));
      expect(
        AppColors.panelAlphaFor(DisplayPrefs.defaultUiTransparency),
        lessThan(1.0),
      );
    });
  });

  group('背景不透明度：默认就该看得见', () {
    test('默认 1.0（0.15 那个值让「用户选的图」变成一层淡影）', () {
      expect(DisplayPrefs.defaultBackgroundOpacity, 1.0);
    });

    test('**「有没有背景」全仓库只有一个判据**', () {
      // 曾经有两份：`DisplayPrefs.hasBackground`（opacity > 0）与
      // `AppShell._hasBackground`（0 < opacity < 1）。滑杆推到 100% 的那一刻
      // 设置页还写着「聊天面板会跟着透」，实际面板已退回不透明——
      // **界面在骗人**（P4）。第二份已经删掉。
      expect(
        _codeOf('lib/app/app_shell.dart'),
        isNot(contains('opacity >= 1')),
        reason: '那条「满不透明就让面板变实」的特例会与文案打架，'
            '且在 99%→100% 之间造成可见的跳变',
      );
      // 判据本身：只有 0 才算「没有背景」。
      const DisplayPrefs withImage = DisplayPrefs(
        backgrounds: <BackgroundItem>[BackgroundImage(
          id: 'a',
          dataUrl: 'data:image/png;base64,AAA',
        )],
      );
      expect(withImage.hasBackground, isTrue);
      expect(withImage.copyWith(backgroundOpacity: 1).hasBackground, isTrue,
          reason: '满不透明**仍然算有背景** —— 面板该更透，不是更实');
      expect(withImage.copyWith(backgroundOpacity: 0).hasBackground, isFalse);
      // 字节没在手 ⇒ 画不出来 ⇒ 不算有背景（否则界面在吹牛）。
      const DisplayPrefs pending = DisplayPrefs(
        backgrounds: <BackgroundItem>[BackgroundImage(id: 'a')],
      );
      expect(pending.hasBackground, isFalse);
    });
  });

  group('背景字节：偏好里不许再存 dataUrl（2026-09-27 搬去 IndexedDB）', () {
    test('清单序列化里不出现 dataUrl / base64', () {
      const DisplayPrefs prefs = DisplayPrefs(
        backgrounds: <BackgroundItem>[
          BackgroundImage(id: 'a', dataUrl: 'data:image/png;base64,AAA'),
        ],
      );
      expect(prefs.toJson().toString(), isNot(contains('base64')));
    });

    test('那份**假的**字符预算不许复活', () {
      // 400 000 字符（≈300 KB 原图）这个上限让「选一张 1080p 照片」永远失败，
      // 实测 3 张图一张都存不进去。它不是安全边际，是事故。
      final String prefs = _codeOf('lib/settings/display_prefs.dart');
      expect(prefs.contains('kBackgroundImageMaxChars'), isFalse);
      expect(prefs.contains('kBackgroundTotalMaxChars'), isFalse);
      expect(prefs.contains('backgroundsChars'), isFalse);
    });

    test('单张上限按**字节**算，且远大于一张真实壁纸', () {
      expect(kBackgroundImageMaxBytes, greaterThan(8 * 1024 * 1024));
      // 3 MB 的桌面壁纸必须能加进去。
      expect(
        DisplayPrefs.canAddBackground(
          const <BackgroundItem>[],
          BackgroundImage(
            id: 'w',
            dataUrl: 'data:image/png;base64,${'A' * (3 * 1024 * 1024)}',
          ),
        ),
        isTrue,
      );
    });
  });
}

/// 读一个源码文件，**只留代码**（去掉 `//` 与 `///` 注释）。
///
/// 为什么要去注释：这些守门测试要证明的是「代码里没有某个东西」，
/// 而本项目的习惯正是**把被删掉的东西连同它的死因一起写进注释**
///（那是这个仓库最值钱的部分）。不去注释的话，
/// 「我们记得为什么删掉它」会被误判成「它还在」。
String _codeOf(String path) => readLibrarySource(path)
    .split('\n')
    .where((String line) => !line.trimLeft().startsWith('//'))
    .join('\n');
