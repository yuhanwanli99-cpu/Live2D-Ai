/// 背景图的**字节库**：偏好里只存身份（id），图本身住在这里。
///
/// # 为什么不再把 dataURL 塞进 localStorage（2026-09-27 改）
///
/// 原来 8 张图以 base64 形态写在 `live2d-ai.display-prefs` 这一个键里，
/// 于是有三条硬伤，**每一条都被用户实际撞到过**：
///
/// 1. **单张 400 KB 的上限让功能形同虚设**。dataURL 比原字节大 33%，
///    400 000 字符 ≈ 300 KB 原始数据——一张 1080p JPEG 轻松 1–3 MB。
///    结果是「用户选了 3 张图，一张都存不进去」。
/// 2. **配额是共享的**。同一个键里还有主题、音量、口型档位，
///    背景图一旦吃满，**整份偏好都写不进去**，连带丢掉别的设置。
/// 3. **落盘粒度太粗**。拖一次音量滑杆就要重写所有图片字节。
///
/// IndexedDB 一次解决三条：配额通常是**磁盘剩余空间的 60%**
/// （几百 MB 到几 GB），读写按记录粒度，与偏好那份小 JSON 完全解耦。
///
/// # 契约（三条纪律）
///
/// - **永不抛**：浏览器可能禁掉 IndexedDB（无痕模式、站点数据被清、
///   隐私设置）。这里所有方法都返回「成没成」而不是抛异常——
///   理由与 `browser_io.dart` 的 `saveDisplayPrefs` 同一条：
///   **写失败必须能被调用方如实转述**，而不是变成一个红屏。
/// - **读不出就是读不出**：[read] 返回 `null` 不等于「图没了」，
///   调用方据此把那一项从库里摘掉（见 `background_hydration.dart`）。
/// - **只有 `put` 可能失败**，`delete` 失败无害（幂等）。
library;

import 'background_store_stub.dart'
    if (dart.library.js_interop) 'background_store_web.dart'
    as impl;

export 'background_store_stub.dart' show MemoryBackgroundStore;

/// 背景字节库。
abstract class BackgroundStore {
  /// 取一张图的 dataURL；**没有 / 读失败都返回 `null`**。
  Future<String?> read(String id);

  /// 写一张图；返回**是否真的写进去了**（配额满 / 被禁 → `false`）。
  Future<bool> put(String id, String dataUrl);

  /// 删一张图。**幂等**：删不存在的键不算失败。
  Future<void> delete(String id);

  /// 库里现在有哪些 id。
  Future<Set<String>> keys();

  /// 只保留 [ids] 里的记录，其余删掉。
  ///
  /// 为什么要它：用户删掉背景库的一项之后，那份字节**没有任何人再引用**，
  /// 而 IndexedDB 不会自动回收——不剪枝的话，用几个月之后
  /// 「明明只剩 2 张图，浏览器却吃掉几百 MB」。剪枝是启动时的一次性动作。
  Future<void> retainOnly(Set<String> ids);
}

/// 打开本平台的字节库。
///
/// web 走 IndexedDB；其它平台（VM 单测、桌面调试）走内存实现。
BackgroundStore createBackgroundStore() => impl.createBackgroundStore();
