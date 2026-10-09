/// 玻璃边缘高光（P3-1/P3-2，2026-09-11）**已下线**（2026-10-08）。
///
/// # 这一份现在守什么
///
/// 维护者口径：外壳不再包那圈跟着指针走的边缘高光（`GlassRim`），也**不要**
/// 换成另一圈渐变或阴影。子组件留在原地。所以本文件从「守它怎么画」改成
/// 「守它确实不在了，而且没有被别的东西顶替」：
///
/// 1. `lib/ui/glass_rim.dart` 已删除，整仓零引用；
/// 2. 三处外壳（compact 整页设置 / 内联侧板 / 舞台角标）不再包 `GlassRim`；
/// 3. 那三处**没有**顺势引入 `BackdropFilter` / `ImageFilter` /
///    `ShaderMask`——舞台是 `<iframe>` 平台视图，模糊采不到它、代价却照付
///    （规格 §12-7，本项目最硬的一条红线）；
/// 4. 子组件（整页设置面 / 侧板描边盒 / 缩放角标）都还在；
/// 5. 压在舞台上的控件仍然套 `StagePointerInterceptor`；
/// 6. 整仓 `BackdropFilter` 命中数仍为 0。
///
/// `AppColors.glassBarrier` / `AppColors.rimHighlight` 是它唯一消费的两个
/// 令牌，现在**暂时没有产品面消费点**——如实登记在
/// `test/design_tokens_test.dart` 的未接线台账里，**没有**在这里补一个假消费点。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'support/source_scan.dart';

/// 三处曾经包 `GlassRim` 的外壳（相对包根）。
const List<String> kFormerRimShells = <String>[
  'lib/app/app_shell_state.dart',
  'lib/app/nav_host.dart',
  'lib/ui/stage_corner_controls.dart',
];

/// 扫描 `lib/**` 里**剥掉注释与字符串之后**仍含 [needle] 的文件。
List<String> scanLib(String needle) {
  final List<String> hits = <String>[];
  for (final FileSystemEntity e in Directory(
    'lib',
  ).listSync(recursive: true)) {
    if (e is! File || !e.path.endsWith('.dart')) continue;
    if (stripCommentsAndStrings(e.readAsStringSync()).contains(needle)) {
      hits.add(e.path);
    }
  }
  return hits;
}

void main() {
  test('glass_rim.dart 已删除，整仓零引用', () {
    expect(
      File('lib/ui/glass_rim.dart').existsSync(),
      isFalse,
      reason: '没有引用之后才删——这个文件不该还在',
    );
    expect(
      scanLib('GlassRim'),
      isEmpty,
      reason: '这些文件还在引用已删除的 GlassRim',
    );
  });

  test('三处外壳不再包 GlassRim，也没换成别的采样层', () {
    for (final String rel in kFormerRimShells) {
      final String src = stripCommentsAndStrings(
        File(rel).readAsStringSync(),
      );
      expect(src.contains('GlassRim'), isFalse, reason: '$rel 仍包着 GlassRim');
      for (final String banned in <String>[
        'BackdropFilter',
        'ImageFilter',
        'ShaderMask',
        'ImageFiltered',
      ]) {
        expect(
          src.contains(banned),
          isFalse,
          reason: '$rel 引入 $banned —— 压在舞台 iframe 上的红线',
        );
      }
    }
  });

  test('拿掉的是那圈高光，子组件留在原地', () {
    final String state = File('lib/app/app_shell_state.dart').readAsStringSync();
    expect(
      state.contains('_settingsSurface('),
      isTrue,
      reason: 'compact 整页设置的内容面不该跟着高光一起消失',
    );
    final String nav = File('lib/app/nav_host.dart').readAsStringSync();
    expect(nav.contains('StagePointerInterceptor('), isTrue);
    expect(nav.contains('ClipRRect('), isTrue);
    final String corner = File(
      'lib/ui/stage_corner_controls.dart',
    ).readAsStringSync();
    expect(corner.contains('DecoratedBox('), isTrue);
    expect(corner.contains('_ZoomTextButton('), isTrue);
  });

  test('整仓 BackdropFilter 命中数仍为 0（红线没被顺手破掉）', () {
    expect(
      scanLib('BackdropFilter'),
      isEmpty,
      reason: '这些文件引入了 BackdropFilter',
    );
  });
}
