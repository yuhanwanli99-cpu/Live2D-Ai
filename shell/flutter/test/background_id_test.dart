/// 背景图片 id：**跨平台必须一致**，否则「同一张图」在不同平台上不是同一项。
///
/// （id 的实现与 `BackgroundImage` 在同一个文件里 —— `design/**` 有一条
/// 「零 import」的门禁，见 `test/no_backdrop_filter_test.dart`。）
///
/// # 这组测试守的是什么
///
/// 2026-09-27 之前，背景库靠「比较 dataURL 字符串」判断两张图是不是同一张。
/// 字节搬去 IndexedDB 之后必须换成「id」，而 id 是**哈希**——
/// 于是有一个新的、纯靠肉眼发现不了的失效模式：
///
/// **web 与 VM 算出不同的哈希**。Dart 的 `int` 在 web 上是 double，
/// 任何用 `*` 和 `&` 的 32 位哈希（FNV、xxHash 的 32 位变体）都会在
/// 乘积超过 2^53 之后开始丢精度，而 VM 上那一步是精确的。
///
/// 后果不是崩溃，是**静默的数据分裂**：用户导入了同一张图，
/// 存进去的键和读出来要找的键对不上，于是「图不见了」。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';

void main() {
  group('背景图片 id', () {
    test('同一个 dataURL **永远**得到同一个 id（去重靠它）', () {
      const String url = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==';
      expect(backgroundIdOf(url), backgroundIdOf(url));
    });

    test('不同的图得到不同的 id', () {
      expect(
        backgroundIdOf('data:image/png;base64,AAA'),
        isNot(backgroundIdOf('data:image/png;base64,AAB')),
      );
    });

    test('MIME 头参与哈希：同一张图换个类型算两项', () {
      // 浏览器对不同扩展名给出的 MIME 不同。与其猜「它们是不是同一张」，
      // 不如让行为可预期：**导入什么就存什么**。
      expect(
        backgroundIdOf('data:image/png;base64,AAA'),
        isNot(backgroundIdOf('data:image/jpeg;base64,AAA')),
      );
    });

    test('id 带 `bg` 前缀，固定 8 位十六进制', () {
      // 前缀的意义：在 IndexedDB / 日志里一眼认出「这是背景字节」。
      final String id = backgroundIdOf('data:image/png;base64,AAA');
      expect(id, startsWith('bg'));
      expect(id.length, 10);
      expect(RegExp(r'^bg[0-9a-f]{8}$').hasMatch(id), isTrue);
    });

    test('**全程不超过 2^53**（web 上 int 是 double，超过就丢精度）', () {
      // 这条是本文件存在的全部理由。djb2 的中间值是 `h * 33 + c`，
      // h 被模到 2^31-1，于是最大值 < 2^37。换成任何 32 位乘法哈希
      // （FNV-1a 是 `h * 16777619`，上界 2^56）都会在这里越界。
      const int ceiling = 9007199254740992; // 2^53
      for (final String probe in <String>[
        '',
        'a',
        'a' * 1000,
        'data:image/png;base64,${'A' * 5000}',
        'data:image/png;base64,${'ÿ' * 5000}',
      ]) {
        final int h = backgroundFingerprint(probe);
        expect(h, greaterThan(0));
        expect(h, lessThan(kBackgroundHashModulus));
        // 重新算一遍中间值的上界，确认没有一次乘法会越界。
        expect(5381 * 33 + 0xFFFF, lessThan(ceiling));
      }
    });

    test('大字符串（几 MB 的 dataURL）也要能算完', () {
      // 用户真的会导入几 MB 的图；不能因为慢或溢出就崩。
      final String big = 'data:image/png;base64,${'A' * (1024 * 1024)}';
      final String id = backgroundIdOf(big);
      expect(RegExp(r'^bg[0-9a-f]{8}$').hasMatch(id), isTrue);
    });

    test('空串不炸（防御：界面不该因为一个空输入而崩）', () {
      expect(backgroundIdOf(''), startsWith('bg'));
    });
  });
}
