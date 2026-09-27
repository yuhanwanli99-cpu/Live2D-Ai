/// 背景字节库与**启动对接**：读偏好 → 补字节 → 搬旧档 → 剪枝。
///
/// # 这组测试守的是什么
///
/// 背景字节搬去 IndexedDB 之后，「偏好里有一项」与「这项画得出来」成了
/// 两件事。中间那段对接代码很容易出三种静默缺陷：
///
/// 1. **搬了旧档却没写回偏好** → localStorage 里那份大 base64 永远留着，
///    每次启动重搬一次，而问题「看起来已经修好了」；
/// 2. **字节丢了还留着清单项** → 界面写着「3 项」，屏幕上一张都没有；
/// 3. **不剪枝** → 用户删掉的图永远占着磁盘，用几个月后浏览器吃掉几百 MB。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/data/background_hydration.dart';
import 'package:live2d_ai_shell/data/background_store.dart';
import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';

const String _a = 'data:image/png;base64,AAA';
const String _b = 'data:image/png;base64,BBB';

void main() {
  group('内存字节库（web 那份的语义定义）', () {
    test('存取删 + retainOnly', () async {
      final MemoryBackgroundStore store = MemoryBackgroundStore();
      expect(await store.read('x'), isNull);
      expect(await store.put('x', _a), isTrue);
      expect(await store.read('x'), _a);
      expect(await store.keys(), <String>{'x'});

      await store.put('y', _b);
      await store.retainOnly(<String>{'x'});
      expect(await store.keys(), <String>{'x'});
      expect(await store.read('y'), isNull, reason: '剪枝必须真的删掉');
    });

    test('delete 幂等（删不存在的键不算失败）', () async {
      final MemoryBackgroundStore store = MemoryBackgroundStore();
      await store.delete('never-existed');
      expect(await store.keys(), isEmpty);
    });
  });

  group('启动对接：清单 → 画得出来的图', () {
    test('只有 id 的项会被补上字节', () async {
      final MemoryBackgroundStore store = MemoryBackgroundStore(
        <String, String>{'bg0000001': _a},
      );
      const DisplayPrefs prefs = DisplayPrefs(
        backgrounds: <BackgroundItem>[BackgroundImage(id: 'bg0000001')],
      );

      final BackgroundHydration r = await hydrateBackgrounds(prefs, store);

      expect(r.prefs.backgrounds.single.isRenderable, isTrue);
      expect(r.prefs.backgrounds.single, const BackgroundImage(id: 'bg0000001'),
          reason: '补上字节后**仍然是同一项**（按 id 判等）');
      expect(r.migrated, isFalse, reason: '字节一直都在，不算「搬过」');
    });

    test('字节读不回来的项被**摘掉**（不留一个画不出来的空位）', () async {
      final MemoryBackgroundStore store = MemoryBackgroundStore();
      const DisplayPrefs prefs = DisplayPrefs(
        backgrounds: <BackgroundItem>[
          BackgroundImage(id: 'gone'),
          BackgroundPattern(BackgroundPatternId.grid),
        ],
      );

      final BackgroundHydration r = await hydrateBackgrounds(prefs, store);

      expect(r.prefs.backgrounds.length, 1);
      expect(r.prefs.backgrounds.single, const BackgroundPattern(
        BackgroundPatternId.grid,
      ));
      expect(r.migrated, isTrue, reason: '清单变了 → 必须写回盘');
    });

    test('旧档（dataUrl 在偏好里）被搬进字节库，并标记要写回', () async {
      final MemoryBackgroundStore store = MemoryBackgroundStore();
      final String id = backgroundIdOf(_a);
      final DisplayPrefs prefs = DisplayPrefs.fromJson(<String, Object?>{
        'backgrounds': <Object?>[
          <String, Object?>{'kind': 'image', 'dataUrl': _a},
        ],
      });
      expect((prefs.backgrounds.single as BackgroundImage).id, id);

      final BackgroundHydration r = await hydrateBackgrounds(prefs, store);

      expect(await store.read(id), _a, reason: '字节必须搬到库里');
      expect(r.prefs.backgrounds.single.isRenderable, isTrue);
      expect(r.migrated, isTrue,
          reason: '不写回的话 localStorage 里那份 base64 永远清不掉');
      // 写回之后的形态：只有 id。
      expect(r.prefs.toJson().toString().contains('base64'), isFalse);
    });

    test('剪枝：库里没人引用的记录被删掉', () async {
      final MemoryBackgroundStore store = MemoryBackgroundStore(
        <String, String>{'bg0000001': _a, 'orphan': _b},
      );
      const DisplayPrefs prefs = DisplayPrefs(
        backgrounds: <BackgroundItem>[BackgroundImage(id: 'bg0000001')],
      );

      await hydrateBackgrounds(prefs, store);

      expect(await store.keys(), <String>{'bg0000001'});
    });

    test('空库也会剪枝（用户清空背景库后，磁盘要真的空出来）', () async {
      final MemoryBackgroundStore store = MemoryBackgroundStore(
        <String, String>{'x': _a},
      );
      await hydrateBackgrounds(const DisplayPrefs(), store);
      expect(await store.keys(), isEmpty);
    });

  });

  group('P0-1/P0-2：存储故障时**绝不破坏数据**', () {
    test('瞬时读失败：项原样保留、migrated=false、一次剪枝都不做', () async {
      final String ia = backgroundIdOf(_a);
      final String ib = backgroundIdOf(_b);
      final _FailingReadsStore store = _FailingReadsStore(
        seed: <String, String>{ia: _a, ib: _b},
      );
      const DisplayPrefs prefs = DisplayPrefs(
        backgrounds: <BackgroundItem>[
          BackgroundImage(id: 'x'),
          BackgroundImage(id: 'y'),
        ],
      );
      final BackgroundHydration r = await hydrateBackgrounds(prefs, store);
      expect(
        r.prefs.backgrounds,
        prefs.backgrounds,
        reason: '读失败 ≠ 没有 → 一项都不许摘',
      );
      expect(
        r.migrated,
        isFalse,
        reason: 'migrated=true 会让 main.dart 把被剪过的清单写回盘',
      );
      expect(
        store.pruned,
        isFalse,
        reason: '健康探测没过 → retainOnly 一次都不许调用',
      );
      expect(store.seed.length, 2, reason: '字节一个都没少');
    });

    test('第二次启动（清单已空）也不删字节', () async {
      final _FailingReadsStore store = _FailingReadsStore(
        seed: <String, String>{'bg00000001': _a},
      );
      await hydrateBackgrounds(const DisplayPrefs(), store);
      expect(
        store.pruned,
        isFalse,
        reason: '空清单不是「用户清空了库」的充分条件',
      );
      expect(store.seed, <String, String>{'bg00000001': _a});
    });

    test('旧档搬家 put 失败：保留旧 dataUrl，不标记写回', () async {
      final _ReadButNoWriteStore store = _ReadButNoWriteStore();
      final String id = backgroundIdOf(_a);
      final DisplayPrefs legacy = DisplayPrefs.fromJson(<String, Object?>{
        'backgrounds': <Object?>[
          <String, Object?>{'kind': 'image', 'dataUrl': _a},
        ],
      });
      final BackgroundHydration r = await hydrateBackgrounds(legacy, store);
      expect(
        r.migrated,
        isFalse,
        reason: '没搬成就不能写回：toJson 只写 id，那份 base64 会被覆盖掉',
      );
      expect(
        (r.prefs.backgrounds.single as BackgroundImage).dataUrl,
        _a,
        reason: '旧 dataUrl 必须原样保留（P0-2）',
      );
      expect(await store.read(id), isNull, reason: '库里确实没写进去（这是前提）');
    });

    test('存储整个挂掉时**不抛**，且偏好**原样保留**（不做破坏性写回）', () async {
      const DisplayPrefs prefs = DisplayPrefs(
        backgrounds: <BackgroundItem>[BackgroundImage(id: 'x')],
      );
      final BackgroundHydration r = await hydrateBackgrounds(
        prefs,
        _ExplodingStore(),
      );
      expect(
        r.prefs.backgrounds,
        prefs.backgrounds,
        reason: '读失败 ≠ 没有 → 不许摘项',
      );
      expect(r.migrated, isFalse, reason: '存储故障时绝不能写回盘');
    });
  });

  group('删除路径：清单与字节必须同进同退', () {
    test('forgetBackground 真的把字节删了', () async {
      final MemoryBackgroundStore store = MemoryBackgroundStore();
      final String id = backgroundIdOf(_a);
      await storeBackground(store, id, _a);
      expect(await store.keys(), <String>{id});

      await forgetBackground(store, id);
      expect(await store.keys(), isEmpty,
          reason: '只删清单不删字节的话，磁盘上会一直留着孤儿');
    });

    test('storeBackground 写不进去时返回 false（界面据此如实说）', () async {
      expect(await storeBackground(_ExplodingStore(), 'x', _a), isFalse,
          reason: '返回 true 就等于骗用户「存下了」');
    });
  });
}

/// 一条**每条操作都失败**的字节库：模拟无痕模式 / 站点数据被禁。
class _ExplodingStore implements BackgroundStore {
  @override
  Future<String?> read(String id) async => throw StateError('disabled');

  /// `implements` 连具体方法也要实现：这里照旧**抛**，
  /// 由对接层的 `_safe` 收敛成「读失败」。
  @override
  Future<BackgroundRead> readChecked(String id) async =>
      throw StateError('disabled');

  @override
  Future<bool> probe() async => throw StateError('disabled');

  @override
  Future<bool> put(String id, String dataUrl) async => throw StateError('no quota');

  @override
  Future<void> delete(String id) async => throw StateError('disabled');

  @override
  Future<Set<String>> keys() async => throw StateError('disabled');

  @override
  Future<void> retainOnly(Set<String> ids) async => throw StateError('disabled');
}

/// 读**失败**的字节库：与 IndexedDB「打不开 / 超时 / 被禁」同形（P0-1）。
///
/// 老契约的 [read] 只能回 `null`；[readChecked] 才把「这次读不到」说清楚。
class _FailingReadsStore implements BackgroundStore {
  _FailingReadsStore({Map<String, String>? seed})
    : seed = <String, String>{...?seed};

  final Map<String, String> seed;

  /// [retainOnly] 被调用过没有——**本轮一次都不该被调用**。
  bool pruned = false;

  @override
  Future<String?> read(String id) async => null;

  @override
  Future<BackgroundRead> readChecked(String id) async =>
      const BackgroundRead.failed();

  @override
  Future<bool> probe() async => false;

  @override
  Future<bool> put(String id, String dataUrl) async => false;

  @override
  Future<void> delete(String id) async => seed.remove(id);

  @override
  Future<Set<String>> keys() async => seed.keys.toSet();

  @override
  Future<void> retainOnly(Set<String> ids) async {
    pruned = true;
    seed.removeWhere((String k, _) => !ids.contains(k));
  }
}

/// 读得到（明确回答「没有」）但**写不进去**的库：模拟配额满（P0-2）。
class _ReadButNoWriteStore implements BackgroundStore {
  final Map<String, String> _items = <String, String>{};

  @override
  Future<String?> read(String id) async => _items[id];

  @override
  Future<BackgroundRead> readChecked(String id) async => _items[id] == null
      ? const BackgroundRead.missing()
      : BackgroundRead.found(_items[id]!);

  @override
  Future<bool> probe() async => true;

  @override
  Future<bool> put(String id, String dataUrl) async => false;

  @override
  Future<void> delete(String id) async => _items.remove(id);

  @override
  Future<Set<String>> keys() async => _items.keys.toSet();

  @override
  Future<void> retainOnly(Set<String> ids) async =>
      _items.removeWhere((String k, _) => !ids.contains(k));
}
