/// IndexedDB 字节库（**唯一**用 `package:web` 碰背景存储的实现）。
///
/// # 为什么是 IndexedDB 而不是 Cache Storage
///
/// 两者配额同源（都跟磁盘剩余空间挂钩），但 Cache Storage 的语义是
/// 「HTTP 响应缓存」——会被浏览器的「清除缓存」顺手清掉，而用户的背景图
/// **是设置的一部分**，不该被一个跟设置无关的按钮带走。IndexedDB
/// 落在「站点数据」里，与 localStorage 同级。
///
/// # 四条实现纪律
///
/// 1. **永不抛**：[BackgroundStore] 的契约要求「写失败要能被转述」。
///    `QuotaExceededError`、无痕模式、站点数据被禁都会抛——全部转成
///    `false` / `null`。
/// 2. **打开只开一次**（[_opening]）：否则启动时并发 8 次 [read]
///    会开出 8 个连接。
/// 3. **每条操作都自带超时**（[kIdbOpTimeoutMs]）：IndexedDB 在某些隐私
///    配置下**既不 resolve 也不 reject**（`onblocked` 且对端已消失），
///    那会把整个启动流程挂住——比抛异常更糟。
/// 4. **等 `transaction.oncomplete` 而不是只等 `request.onsuccess`**：
///    配额是在**提交时**才判定的，`onsuccess` 只说明「这条记录进了事务」，
///    事务随后 abort 的话 `onsuccess` 早就来过一轮了。
library;

import 'dart:async';
import 'dart:js_interop';

import 'package:web/web.dart' as web;

import 'background_store.dart';

/// 单条 IndexedDB 操作的挂死兜底（毫秒）。
///
/// 不是「防慢」而是「防挂死」：见头注第 3 条。正常读写在 1–20 ms。
const int kIdbOpTimeoutMs = 3000;

const String _dbName = 'live2d-ai';
const int _dbVersion = 1;
const String _storeName = 'backgrounds';

BackgroundStore createBackgroundStore() => _IdbBackgroundStore();

class _IdbBackgroundStore implements BackgroundStore {
  /// 正在进行的打开（只开一次）。`null` = 还没开过。
  Future<web.IDBDatabase?>? _opening;

  Future<web.IDBDatabase?> _open() {
    final Future<web.IDBDatabase?>? existing = _opening;
    if (existing != null) return existing;
    final Completer<web.IDBDatabase?> done = Completer<web.IDBDatabase?>();
    _opening = done.future;
    unawaited(_doOpen(done));
    return done.future;
  }

  Future<void> _doOpen(Completer<web.IDBDatabase?> done) async {
    web.IDBDatabase? db;
    try {
      final Completer<web.IDBDatabase> ready = Completer<web.IDBDatabase>();
      final web.IDBOpenDBRequest request = web.window.indexedDB.open(
        _dbName,
        _dbVersion,
      );
      request.onupgradeneeded = ((web.Event _) {
        final web.IDBDatabase target = request.result as web.IDBDatabase;
        if (!target.objectStoreNames.contains(_storeName)) {
          target.createObjectStore(_storeName);
        }
      }).toJS;
      request.onerror = ((web.Event _) {
        if (!ready.isCompleted) ready.completeError('idb open error');
      }).toJS;
      request.onblocked = ((web.Event _) {
        if (!ready.isCompleted) ready.completeError('idb open blocked');
      }).toJS;
      request.onsuccess = ((web.Event _) {
        if (!ready.isCompleted) {
          ready.complete(request.result as web.IDBDatabase);
        }
      }).toJS;
      db = await ready.future.timeout(
        const Duration(milliseconds: kIdbOpTimeoutMs),
      );
    } catch (_) {
      // 打不开（无痕 / 被禁 / 挂死）→ 记成「没有库」：
      // 每条操作都会返回 null / false，调用方据此**如实告诉用户写不进去**，
      // 而不是假装成功、刷新后才发现图没了。
      if (!done.isCompleted) done.complete(null);
      return;
    }
    if (!done.isCompleted) done.complete(db);
  }

  /// 跑一条事务，返回它的结果（`dartify` 过的 Dart 值）；失败返回 `null`。
  Future<Object?> _tx(
    String mode,
    web.IDBRequest Function(web.IDBObjectStore store) body,
  ) async {
    final web.IDBDatabase? db = await _open();
    if (db == null) return null;
    final Completer<Object?> value = Completer<Object?>();
    final Completer<void> done = Completer<void>();
    try {
      // `transaction` 的第一个参数是 `JSAny`（`DOMStringList` 或 `JSArray`）。
      // `jsify()` 把 `List<String>` 转成真正的 JS Array——
      // `List` 没有 `toJS` 扩展（那是 `dart:typed_data` 的那些类型的）。
      final JSAny storeNames = <String>[_storeName].jsify() as JSAny;
      final web.IDBTransaction tx = db.transaction(storeNames, mode);
      final web.IDBRequest request = body(tx.objectStore(_storeName));
      request.onsuccess = ((web.Event _) {
        if (!value.isCompleted) value.complete(request.result.dartify());
      }).toJS;
      request.onerror = ((web.Event _) {
        if (!value.isCompleted) value.complete(null);
      }).toJS;
      tx.oncomplete = ((web.Event _) {
        if (!done.isCompleted) done.complete();
      }).toJS;
      tx.onabort = ((web.Event _) {
        if (!value.isCompleted) value.complete(null);
        if (!done.isCompleted) done.complete();
      }).toJS;
      tx.onerror = ((web.Event _) {
        if (!value.isCompleted) value.complete(null);
        if (!done.isCompleted) done.complete();
      }).toJS;
      await done.future.timeout(const Duration(milliseconds: kIdbOpTimeoutMs));
      return value.isCompleted ? await value.future : null;
    } catch (_) {
      return null;
    }
  }

  @override
  Future<String?> read(String id) async {
    final Object? raw = await _tx(
      'readonly',
      (web.IDBObjectStore s) => s.get(id.toJS),
    );
    return raw is String && raw.isNotEmpty ? raw : null;
  }

  @override
  Future<bool> put(String id, String dataUrl) async {
    final Object? key = await _tx(
      'readwrite',
      (web.IDBObjectStore s) => s.put(dataUrl.toJS, id.toJS),
    );
    return key is String && key == id;
  }

  @override
  Future<void> delete(String id) async {
    await _tx('readwrite', (web.IDBObjectStore s) => s.delete(id.toJS));
  }

  @override
  Future<Set<String>> keys() async {
    final Object? raw = await _tx(
      'readonly',
      (web.IDBObjectStore s) => s.getAllKeys(),
    );
    if (raw is! List<Object?>) return <String>{};
    return <String>{
      for (final Object? key in raw)
        if (key is String) key,
    };
  }

  @override
  Future<void> retainOnly(Set<String> ids) async {
    final Set<String> present = await keys();
    for (final String id in present.difference(ids)) {
      await delete(id);
    }
  }
}
