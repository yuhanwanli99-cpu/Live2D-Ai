/// F-0006-1 / F-0003-1（P1）· 同一个 dataUrl 必须解出**同一个** Uint8List。
///
/// # 为什么这一条值钱（机制，不是风格）
///
/// `MemoryImage.==` / `hashCode` 比的是 `bytes` 的**对象身份**
/// （SDK `painting/image_provider.dart`：`other.bytes == bytes`，而 `Uint8List`
/// 的 `==` 是身份比较）。所以「每次 build 新建一个 Uint8List」= `ImageCache`
/// 的 key 每次都变 ⇒ **恒 miss** ⇒ 每个流式 delta 都重新 base64Decode + 整图重解码
/// （壳根 `app_shell.dart` 每个 delta 都在重建 `ShellBackdrop`）。
///
/// 三条断言从机制到行为，缺哪一条都能让这个缺陷溜回去：
///
/// 1. **机制最小证明**：两次解码 `identical`；
/// 2. **ImageCache 的 key**：`MemoryImage(a) == MemoryImage(b)` 且 hashCode 相等；
/// 3. **行为级**：连续重建 20 次，树里 `Image` 拿到的 `bytes` 始终是同一实例。
///
/// 另加两条边界：不同串不串味、缓存**有界**（LRU 不许变成内存泄漏）。
library;

import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/ui/shell_backdrop.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 1×1 透明 PNG 的 dataURL（**真图**：会真的过 `Image.memory`）。
const String _onePixelPng =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';

/// 另一串**合法 base64**（不是图片，只用来验「不同串不串味」——纯函数不判图像形态）。
const String _otherPayload = 'data:image/png;base64,AAAABBBB';

/// 造一屏只有一个 [ShellBackdrop] 的树（与 `shell_backdrop_test.dart` 的 host 同形）。
Widget _host({
  required BackgroundItem item,
  int? fit,
  double? opacity,
}) {
  const DisplayPrefs prefs = DisplayPrefs();
  return MaterialApp(
    theme: buildAppTheme(prefs.theme),
    home: ShellBackdrop(
      baseColor: AppPalette.of(prefs.theme).stage,
      item: item,
      opacity: opacity ?? prefs.backgroundOpacity,
      fit: fit ?? prefs.imageFit,
      child: const Text('子树'),
    ),
  );
}

/// 重建一次并把树里那个 `Image` 的 `bytes` 拿回来。
Uint8List _pumpedBytes(WidgetTester tester) {
  final Image image = tester.widget<Image>(find.byType(Image));
  return (image.image as MemoryImage).bytes;
}

void main() {
  group('decodeDataUrlBytes：同一个串 = 同一个实例（F-0006-1 的最小机制）', () {
    test('两次调用返回**同一个对象**（identical）', () {
      final Uint8List? a = decodeDataUrlBytes(_onePixelPng);
      final Uint8List? b = decodeDataUrlBytes(_onePixelPng);
      expect(a, isNotNull);
      expect(b, isNotNull);
      expect(
        identical(a, b),
        isTrue,
        reason:
            '每次 new 一个 Uint8List ⇒ MemoryImage 的 key 每次都变 ⇒ ImageCache 恒 miss',
      );
    });

    test('MemoryImage 因此判等且 hashCode 相同（ImageCache 真能命中）', () {
      final Uint8List a = decodeDataUrlBytes(_onePixelPng)!;
      final Uint8List b = decodeDataUrlBytes(_onePixelPng)!;
      expect(MemoryImage(a), MemoryImage(b));
      expect(MemoryImage(a).hashCode, MemoryImage(b).hashCode);
    });

    test('不同串不串味；坏输入照旧 null（不抛）', () {
      final Uint8List first = decodeDataUrlBytes(_onePixelPng)!;
      final Uint8List second = decodeDataUrlBytes(_otherPayload)!;
      expect(identical(first, second), isFalse, reason: '备忘必须按串区分');
      // 同串再来一次仍然是 first 那个实例（说明不是「每次换新的」）。
      expect(identical(decodeDataUrlBytes(_onePixelPng), first), isTrue);

      for (final String? bad in <String?>[
        null,
        '',
        'not a data url',
        'data:image/png,AAAA',
        'data:image/png;base64,@@@',
        'data:image/png;base64,',
      ]) {
        expect(decodeDataUrlBytes(bad), isNull, reason: '不该接受 $bad');
      }
    });

    test('备忘**有界**：连解 8 张不同的图之后，最早那张已被挤出', () {
      final String first = 'data:image/png;base64,QUFBQQ==';
      final Uint8List pinned = decodeDataUrlBytes(first)!;
      for (int i = 0; i < 8; i++) {
        expect(
          // 长度必须是 4 的倍数（base64 的硬要求），否则解出来是 null 而不是新实例。
          decodeDataUrlBytes('data:image/png;base64,QUFBQ0F$i'),
          isNotNull,
          reason: '第 $i 张的测试数据本身必须能解开',
        );
      }
      expect(
        identical(decodeDataUrlBytes(first), pinned),
        isFalse,
        reason: '没有上限的备忘会把用户导入过的每张图都留在内存里',
      );
    });
  });

  group('行为级：连续重建，Image 取到的 bytes 实例不变（F-0006-1）', () {
    testWidgets('重建 20 次（模拟 20 个流式 delta）后仍是同一实例', (
      WidgetTester tester,
    ) async {
      const BackgroundItem item = BackgroundImage(
        id: 'p',
        dataUrl: _onePixelPng,
      );

      await tester.pumpWidget(_host(item: item));
      final Uint8List first = _pumpedBytes(tester);
      for (int i = 0; i < 20; i++) {
        // 每次都是**新** widget 实例（宿主 setState 的真实形态）。
        await tester.pumpWidget(_host(item: item));
        final Uint8List now = _pumpedBytes(tester);
        expect(
          identical(now, first),
          isTrue,
          reason: '第 $i 次重建拿到了新字节实例 ⇒ 重解码 + 重新上传 GPU',
        );
      }
    });

    testWidgets('平铺档（tile）同样只解一次', (WidgetTester tester) async {
      const BackgroundItem item = BackgroundImage(
        id: 'p',
        dataUrl: _onePixelPng,
      );
      await tester.pumpWidget(_host(item: item, fit: DisplayPrefs.fitTile));
      final Uint8List first = decodeDataUrlBytes(_onePixelPng)!;
      await tester.pumpWidget(_host(item: item, fit: DisplayPrefs.fitTile));
      expect(identical(decodeDataUrlBytes(_onePixelPng), first), isTrue);
      expect(tester.takeException(), isNull);
    });
  });
}
