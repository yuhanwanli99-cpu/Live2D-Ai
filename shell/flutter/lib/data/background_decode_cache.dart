/// dataURL → 图片字节的**进程内备忘**（F-0006-1 / F-0003-1，2026-09-28）。
///
/// # 为什么必须备忘（这不是「顺手优化」）
///
/// `MemoryImage.==` / `hashCode` 比的是 `bytes` 的**对象身份**
/// （SDK `painting/image_provider.dart`：`other.bytes == bytes`，而 `Uint8List`
/// 的 `==` 是身份比较）。于是「每次 build 新建一个 Uint8List」= `ImageCache`
/// 的 key 每次都变 ⇒ **恒 miss** ⇒ 每次重建都重新 base64Decode
/// （协议预算 ≤1.5 M 字符）+ 整张图重解码 + 重新上传纹理。
///
/// 而壳根 `ShellBackdrop` 是**每个流式 delta** 重建一次的
/// （`app_shell.dart` 的 `AppShell.build`，由 `ChatController.notifyListeners`
/// 驱动）。所以这不是偶发浪费，而是「设了壁纸的用户，回复期间的每一帧都重解一张图」。
///
/// # 契约
///
/// - **同一个 dataUrl 串 → 同一个 `Uint8List` 实例**（`identical` 为真）；
/// - **LRU 有界**（[capacity] 条，缺省 3）：用户导入过的图不会全留在内存里；
/// - **只按串判等**，不引入第二套内容判等——`MemoryImage` 的 key 本来就是
///   bytes 身份，这里只是让它**稳定**；
/// - 坏输入**照旧返回 `null` 且不入缓存**，沿用
///   `decodeDataUrlBytes` 那条「一份被塞坏的存储不该让整个壳渲染不出来」。
///
/// # 谁在共用
///
/// 壳背景图（`ui/shell_backdrop.dart`）与背景库缩略图
/// （`settings/sections/appearance_section.dart` 的 `_ImageTile`）调的是**同一个**
/// 函数，所以备忘放在函数内部，两条路径一起修好
/// （缩略图那条的行为回归由 R6-b 在 appearance 侧验证）。
library;

import 'dart:convert';
import 'dart:typed_data';

/// 缺省容量：**3 条**。
///
/// 为什么要上限而不是无限 Map：用户可能导入过几十张图，全留着就是几十 MB
/// 常驻内存（这正是「性能修复」变成「内存泄漏」的经典形态）。
/// 3 条覆盖壳背景 + 背景库里最近点开的两张，实测使用足够。
const int kBackgroundDecodeCacheCapacity = 3;

/// 按 dataUrl 串备忘解码结果的 **LRU**。
class DataUrlBytesCache {
  DataUrlBytesCache({this.capacity = kBackgroundDecodeCacheCapacity})
    : assert(capacity > 0);

  /// 最多留几条。
  final int capacity;

  /// **插入有序**（Dart 的 `Map` 保证）：命中时先删再插 = 提到最近使用，
  /// 于是 `keys.first` 永远是最久未用的那条。
  final Map<String, Uint8List> _byUrl = <String, Uint8List>{};

  /// 当前缓存条数（排障 / 测试用）。
  int get length => _byUrl.length;

  /// 解一串 dataURL；**同一个串永远返回同一个实例**，坏输入回 `null`。
  Uint8List? decode(String? dataUrl) {
    if (dataUrl == null) return null;
    final Uint8List? hit = _byUrl[dataUrl];
    if (hit != null) {
      _byUrl
        ..remove(dataUrl)
        ..[dataUrl] = hit;
      return hit;
    }
    final Uint8List? bytes = _decodeUncached(dataUrl);
    if (bytes == null) return null; // 坏输入不占位（下次仍然只是解一遍就失败）
    _byUrl[dataUrl] = bytes;
    while (_byUrl.length > capacity) {
      _byUrl.remove(_byUrl.keys.first);
    }
    return bytes;
  }

  /// 清空（测试之间互不串味用）。
  void clear() => _byUrl.clear();
}

/// 解码本体：**永不抛**。
///
/// 判据与原来逐字相同：没有逗号 / 元数据里没有 `base64` / 解不开 /
/// 空字节 —— 一律 `null`，由调用方退回「只有底色」。
Uint8List? _decodeUncached(String dataUrl) {
  final int comma = dataUrl.indexOf(',');
  if (comma <= 0) return null;
  final String meta = dataUrl.substring(0, comma);
  if (!meta.contains('base64')) return null;
  try {
    final Uint8List bytes = base64Decode(dataUrl.substring(comma + 1));
    return bytes.isEmpty ? null : bytes;
  } catch (_) {
    return null;
  }
}

/// 进程内**共用**的那一份备忘（壳背景 + 背景库缩略图同源）。
final DataUrlBytesCache _sharedDecodeCache = DataUrlBytesCache();

/// [DataUrlBytesCache.decode] 的共用入口。
Uint8List? decodeDataUrlBytesCached(String? dataUrl) =>
    _sharedDecodeCache.decode(dataUrl);

/// 当前共用备忘里的条数（排障用；不许拿它当业务判据）。
int get backgroundDecodeCacheLength => _sharedDecodeCache.length;
