/// 内存字节库：非 web 平台（VM 单测）与 IndexedDB 打不开时的退路。
///
/// 它同时是**接口的行为定义**——[BackgroundStore] 的三条纪律
/// （永不抛 / 读不出返回 null / delete 幂等）在这里被逐字实现，
/// IndexedDB 那份必须与它**语义一致**，否则「在浏览器里测不出来」的东西
/// 会从测试缝里漏过去（这正是本项目反复吃过的亏）。
library;

import 'package:flutter/foundation.dart' show visibleForTesting;

import 'background_store.dart';

class MemoryBackgroundStore implements BackgroundStore {
  MemoryBackgroundStore([Map<String, String>? seed])
    : _items = <String, String>{...?seed};

  final Map<String, String> _items;

  @override
  Future<String?> read(String id) async => _items[id];

  @override
  Future<bool> put(String id, String dataUrl) async {
    _items[id] = dataUrl;
    return true;
  }

  @override
  Future<void> delete(String id) async {
    _items.remove(id);
  }

  @override
  Future<Set<String>> keys() async => _items.keys.toSet();

  @override
  Future<void> retainOnly(Set<String> ids) async {
    _items.removeWhere((String key, _) => !ids.contains(key));
  }

  /// 测试用的窥视口（生产路径不该用）。
  @visibleForTesting
  int get length => _items.length;
}

/// 非 web 平台的工厂：内存实现。
BackgroundStore createBackgroundStore() => MemoryBackgroundStore();
