/// **F-0016-1**（与 F-0006-1 同根因）：背景库缩略图不许每次 build 重解码。
///
/// # 这一份在验证什么
///
/// `MemoryImage.==` 比的是 bytes 的**对象身份**。旧实现每次 build 都
/// `base64Decode` 出一个**新** Uint8List ⇒ ImageCache 的键每次都变 ⇒ 恒 miss
/// ⇒ 每个流式增量都重解码可见的每一张缩略图（并让条目数一直涨、把在用的挤掉）。
///
/// R6-a2 把解码换成了带**进程内备忘**的实现
/// （`data/background_decode_cache.dart`，同串同实例、LRU 有界）。本文件的任务
/// 是**验证并钉住它覆盖到了缩略图路径**（不是只修了壳背景那条）：
///
/// 1. 同串同实例（缩略图与壳背景共用同一个函数）；
/// 2. N 次宿主重建之后，缩略图 `Image` 的 bytes 始终是同一实例；
/// 3. ImageCache 的条目数不随重建次数增长。
library;

import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/data/background_decode_cache.dart';
import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/sections/appearance_section.dart';
import 'package:live2d_ai_shell/ui/shell_backdrop.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

const String _png =
    'data:image/png;base64,'
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';


/// 三个**互不相同**的串，只用来验证 LRU 的「同串同实例 / 淘汰后重解」。
/// （它们不必是能画出来的图：这一组测的是备忘本身。）
const String _u1 = 'data:image/png;base64,AAAA';
const String _u2 = 'data:image/png;base64,BBBB';
const String _u3 = 'data:image/png;base64,CCCC';

Widget _library(DisplayPrefs prefs) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SingleChildScrollView(
      child: BackgroundRuntimeScope(
        index: 0,
        current: prefs.effectiveBackground,
        hydrating: false,
        child: AppearanceSection(
          prefs: prefs,
          onPrefsChanged: (DisplayPrefs _) {},
          onPickShellImage: () {},
          onClearShellImage: () {},
          onRemoveBackground: (int _) {},
          onRemoveBackgrounds: (List<int> _) {},
          onReorderBackground: (int _, int _) {},
          onPreviewBackground: (int _) {},
          onAddPattern: (int _) {},
        ),
      ),
    ),
  ),
);

/// 取出缩略图那份 `MemoryImage` 的字节。
///
/// `Image.memory(bytes, cacheWidth: …)` 会包一层 `ResizeImage`，所以要走进去看
/// 内层的 provider——**判据是内层 bytes 的身份**，不是外层 wrapper。
Uint8List _bytesOf(Image image) {
  final ImageProvider provider = image.image;
  final ImageProvider inner = provider is ResizeImage
      ? provider.imageProvider
      : provider;
  expect(inner, isA<MemoryImage>(), reason: '缩略图应当来自内存字节');
  return (inner as MemoryImage).bytes;
}

int _cacheEntries() =>
    PaintingBinding.instance.imageCache.currentSize +
    PaintingBinding.instance.imageCache.pendingImageCount;

void main() {
  group('解码备忘本身（F-0006-1 / F-0016-1 共用的那一层）', () {
    test('同一个 dataUrl 串永远返回同一个实例', () {
      final Uint8List? a = decodeDataUrlBytes(_png);
      final Uint8List? b = decodeDataUrlBytes(_png);
      expect(a, isNotNull);
      expect(
        identical(a, b),
        isTrue,
        reason: '同串不同实例 ⇒ MemoryImage 的键每次都变 ⇒ ImageCache 恒 miss',
      );
      // 缩略图与壳背景走的是**同一个**函数（备忘在函数内部，不是调用点各自加）。
      expect(identical(a, decodeDataUrlBytesCached(_png)), isTrue);
    });

    test('坏串仍然回 null，且不占缓存（语义一字未改）', () {
      expect(decodeDataUrlBytes('not-a-data-url'), isNull);
      expect(decodeDataUrlBytes(null), isNull);
      expect(decodeDataUrlBytes('data:image/png;base64,'), isNull);
    });

    test('LRU 有界：容量之外的老条目会被淘汰（不是内存泄漏）', () {
      final DataUrlBytesCache cache = DataUrlBytesCache(capacity: 2);
      final Uint8List? x = cache.decode(_u1);
      cache.decode(_u2);
      final Uint8List? newest = cache.decode(_u3);
      expect(cache.length, 2, reason: '容量是硬的');
      expect(identical(cache.decode(_u3), newest), isTrue, reason: '最近用的那条还在');
      expect(
        identical(cache.decode(_u1), x),
        isFalse,
        reason: '被淘汰的那条必须重新解一遍（不是无限留着）',
      );
    });
  });

  testWidgets('10 次宿主重建：缩略图 bytes 实例始终同一，ImageCache 条目数不增长', (
    WidgetTester tester,
  ) async {
    tester.view.physicalSize = const Size(1000, 2600);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    PaintingBinding.instance.imageCache
      ..clear()
      ..clearLiveImages();

    final DisplayPrefs prefs = const DisplayPrefs().copyWith(
      // 三张**同一个串**的图（id 不同）：这样 ImageCache 的键只有一份，
      // 「条目数不随重建增长」就是一条**确定性**的断言
      // （若每张串不同，条目数还取决于解码完成与否）。
      backgrounds: <BackgroundItem>[
        BackgroundImage(id: 'a', dataUrl: _png),
        BackgroundImage(id: 'b', dataUrl: _png),
        BackgroundImage(id: 'c', dataUrl: _png),
      ],
    );

    await tester.pumpWidget(_library(prefs));
    await tester.pump();

    List<Uint8List> thumbBytes() => <Uint8List>[
      for (final Image image in tester.widgetList<Image>(find.byType(Image)))
        _bytesOf(image),
    ];

    final List<Uint8List> first = thumbBytes();
    expect(first, hasLength(3), reason: '库里三张图都要有缩略图');
    final int entriesAfterFirst = _cacheEntries();

    for (int round = 0; round < 10; round++) {
      // 宿主重建（流式回复期间每个增量都会来一次）。
      await tester.pumpWidget(_library(prefs));
      await tester.pump();
      final List<Uint8List> again = thumbBytes();
      expect(again, hasLength(3));
      for (int i = 0; i < 3; i++) {
        expect(
          identical(again[i], first[i]),
          isTrue,
          reason: '第 ${round + 1} 次重建之后第 ${i + 1} 张缩略图的 bytes 换了实例'
              '——那就是每次重建都重解码（F-0016-1 的原症状）',
        );
      }
      expect(
        _cacheEntries(),
        entriesAfterFirst,
        reason: 'ImageCache 的条目数不许随重建次数增长'
            '（涨=键在抖=在用的条目会被 LRU 挤掉）',
      );
    }
  });
}
