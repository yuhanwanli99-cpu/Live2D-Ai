/// F-0013-1 / F-0001-3（P1）· 启动水合窗口：窗口期用户的改动**一律不许被回滚**。
///
/// # 现场（三件事在同一个窗口里同时发生）
///
/// 1. 窗口里用户**导入了一张图**：`storeBackground(真库, id, url)` 成功、
///    偏好里加了一项、界面提示「已加进背景库」；
/// 2. 窗口里用户**改了音量**（`_updatePrefs`）——任何偏好改动都算；
/// 3. 水合完成。旧实现把**水合开始时刻的快照**整体盖回去
///    （`_prefs = hydrated.prefs`），且 `migrated` 时还把这份旧对象写回盘；
///    同时 `retainOnly(快照里的 id)` 会把窗口内新加那张的**字节**当孤儿删掉。
///
/// # 为什么这个文件要分两层测
///
/// `main.dart` **在 VM 测试里加载不了**（它经 `app/browser_io.dart` 依赖
/// `package:web`）。所以这里把水合拆成两段、都用**生产函数**：
///
/// - `hydrateBackgrounds(...)`：与存储打交道（读回字节 / 搬旧档 / 剪枝）；
/// - `applyBackgroundHydration(latest, hydration)`：把结果落到**最新**偏好上
///   ——这正是 `main.dart` 的落点，也是本条的修法所在。
///
/// 场景与断言从红到绿**逐字未变**；变的只有「落到最新偏好」那一步的写法
/// （RED：`hydrated.prefs`，即旧的 `main.dart` 那一行；GREEN：生产合并函数）。
library;

import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/data/background_hydration.dart';
import 'package:live2d_ai_shell/data/background_store.dart';
import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';

// 复用仓库既有的「剥掉注释与字符串字面量」工具（`diff` 里同款）：
// 结构性守卫必须扫**代码**，否则注释里提一句旧实现就把它自己判红。
import 'design_tokens_test.dart' show stripCommentsAndStrings;

/// A 的字节（水合开始前就在库里）。
const String _aUrl = 'data:image/png;base64,AAAA';

/// B 的字节（**窗口内**才导入的那张）。
const String _bUrl = 'data:image/png;base64,BBBB';

String get _aId => backgroundIdOf(_aUrl);
String get _bId => backgroundIdOf(_bUrl);

/// 生产里「把水合结果落到偏好」的那一步（`main.dart::_hydrateBackgrounds`）。
///
/// RED 阶段这里**逐字复刻旧的 main.dart**：`latest = h.prefs`（整体覆盖成
/// 水合开始时刻的快照）。GREEN 阶段这一行换成 `applyBackgroundHydration`——
/// 这就是本条要改的接口本身；**场景与断言逐字未变**。
DisplayPrefs _applyLikeMain(DisplayPrefs latest, BackgroundHydration h) =>
    applyBackgroundHydration(latest, h);

Set<String> _imageIds(DisplayPrefs prefs) => <String>{
  for (final BackgroundItem item in prefs.backgrounds)
    if (item is BackgroundImage) item.id,
};

/// 受控字节库：可以让某张图的 `readChecked` **挂住**（= 水合窗口开着），
/// 并如实记账 `retainOnly`（剪枝是会删字节的破坏性动作）。
class _GatedStore implements BackgroundStore {
  _GatedStore(this.seed);

  final Map<String, String> seed;

  /// 这一张的读会被 [gate] 挂住。
  final Set<String> gated = <String>{};

  /// 窗口闸门：complete 之后水合才继续。
  final Completer<void> gate = Completer<void>();

  /// 每次剪枝请求保留的 id（用来断言「窗口内新加的项有没有被当孤儿」）。
  final List<Set<String>> pruneRequests = <Set<String>>[];

  @override
  Future<bool> probe() async => true;

  @override
  Future<String?> read(String id) async => seed[id];

  @override
  Future<BackgroundRead> readChecked(String id) async {
    if (gated.contains(id)) await gate.future;
    final String? value = seed[id];
    return value == null
        ? const BackgroundRead.missing()
        : BackgroundRead.found(value);
  }

  @override
  Future<bool> put(String id, String dataUrl) async {
    seed[id] = dataUrl;
    return true;
  }

  @override
  Future<void> delete(String id) async => seed.remove(id);

  @override
  Future<Set<String>> keys() async => seed.keys.toSet();

  @override
  Future<void> retainOnly(Set<String> ids) async {
    pruneRequests.add(ids);
    seed.removeWhere((String id, _) => !ids.contains(id));
  }
}

/// 跑一遍「水合窗口」的完整现场，交回三样东西供断言。
///
/// 时序（与启动时逐跳一致）：
/// 1. `hydrateBackgrounds(start, store)` 开始 —— A 的 `readChecked` 被挂住；
/// 2. **窗口内**：`storeBackground(store, B)`（生产函数）+ 偏好加项 + 改音量；
/// 3. 放闸 → 水合完成；
/// 4. 落到最新偏好（`_applyLikeMain`）。
Future<({_GatedStore store, DisplayPrefs latest, BackgroundHydration h})>
_windowScenario() async {
  final _GatedStore store = _GatedStore(<String, String>{_aId: _aUrl});
  store.gated.add(_aId); // A 的读挂住 ⇒ 窗口开着

  // 水合开始时的那份偏好（A 只有 id，字节在库里；音量是用户当时的 0.8）。
  final DisplayPrefs start = const DisplayPrefs(volume: 0.8).copyWith(
    backgrounds: <BackgroundItem>[BackgroundImage(id: _aId)],
  );
  DisplayPrefs latest = start;

  // 剪枝的保留集读「此刻」的偏好（生产的 `_hydrateSafely` 传的就是这个闭包）。
  final Future<BackgroundHydration> hydration = hydrateBackgrounds(
    start,
    store,
    liveKeptIds: () => _imageIds(latest),
  );

  // ── 窗口内：导入 B（**生产函数**，与 `_pickImage` 走的是同一条） ──
  expect(await storeBackground(store, _bId, _bUrl), isTrue);
  latest = latest.copyWith(
    backgrounds: <BackgroundItem>[
      ...latest.backgrounds,
      BackgroundImage(id: _bId, dataUrl: _bUrl),
    ],
  );
  // ── 窗口内：改音量（任何偏好改动都算） ──
  latest = latest.copyWith(volume: 0.31);

  store.gate.complete();
  return (store: store, latest: latest, h: await hydration);
}

void main() {
  group('水合窗口：窗口内的导入 / 偏好改动不许被回滚（F-0013-1 / F-0001-3）', () {
    test('窗口期改的偏好不许被水合快照盖回去', () async {
      final ({_GatedStore store, DisplayPrefs latest, BackgroundHydration h})
      scene = await _windowScenario();
      final DisplayPrefs applied = _applyLikeMain(scene.latest, scene.h);

      expect(
        applied.volume,
        0.31,
        reason: '窗口期用户改的音量被水合快照盖回去了（F-0001-3）',
      );
      expect(
        _imageIds(applied),
        containsAll(<String>[_aId, _bId]),
        reason: '窗口内导入的那一项被水合快照抹掉了（F-0013-1）',
      );
    });

    test('窗口内导入的字节必须落在真库里、且不许被剪枝当孤儿删掉', () async {
      final ({_GatedStore store, DisplayPrefs latest, BackgroundHydration h})
      scene = await _windowScenario();
      _applyLikeMain(scene.latest, scene.h);

      expect(
        await scene.store.read(_bId),
        _bUrl,
        reason: '剪枝的保留集若读的是水合开始时的快照，这张刚导入的图就被真删了',
      );
      expect(
        scene.store.pruneRequests.every(
          (Set<String> keep) => keep.contains(_bId),
        ),
        isTrue,
        reason: '剪枝的保留集必须读**此刻**的偏好，不是水合开始时的快照',
      );
    });

    test('水合仍然要把 A 的字节补回来（修复没有把「补字节」也去重掉）', () async {
      final ({_GatedStore store, DisplayPrefs latest, BackgroundHydration h})
      scene = await _windowScenario();
      final DisplayPrefs applied = _applyLikeMain(scene.latest, scene.h);
      final BackgroundImage a = applied.backgrounds.firstWhere(
        (BackgroundItem b) => b is BackgroundImage && b.id == _aId,
      ) as BackgroundImage;

      expect(a.dataUrl, _aUrl, reason: '水合要把 A 的字节补回来');
      expect(applied.backgrounds, hasLength(2), reason: 'A、B 各一份，不许重复也不许丢');
    });
  });

  group('F-0013-1 硬要求：水合补字节不许清掉逐图样式', () {
    test('opacity / fit / align 逐字不变（`copyWith(dataUrl: …)` 而不是 new）', () async {
      final MemoryBackgroundStore store = MemoryBackgroundStore(
        <String, String>{_aId: _aUrl},
      );
      final DisplayPrefs prefs = const DisplayPrefs().copyWith(
        backgrounds: <BackgroundItem>[
          BackgroundImage(
            id: _aId,
            opacity: 0.42,
            fit: DisplayPrefs.fitTile,
            align: 7,
          ),
        ],
      );

      final BackgroundHydration h = await hydrateBackgrounds(prefs, store);
      final BackgroundImage item =
          h.prefs.backgrounds.single as BackgroundImage;

      expect(item.dataUrl, _aUrl, reason: '字节要补上');
      expect(item.opacity, 0.42, reason: '用户给这张图设的透明度被水合清空了');
      expect(item.fit, DisplayPrefs.fitTile, reason: '铺法被清空了');
      expect(item.align, 7, reason: '位置被清空了');
    });
  });

  group('F-0002-2 / F-0002-3：接线顺序与「假兜底」（结构性守卫）', () {
    // ⚠️ 这几条只能扫源码：`main.dart` 在 VM 里加载不了（依赖 `package:web`），
    // 所以「水合前暴露的是不是占位内存库」这种行为无法在 VM 里直接跑。
    // 扫的是**调用形状**（不是某一行字），而且先**剥掉注释与字符串**——
    // 否则注释里提一句旧实现就会把守卫自己判红。改坏接线断言就红。
    final String src = stripCommentsAndStrings(
      File('lib/main.dart').readAsStringSync(),
    );

    test('F-0002-2：_store 一开始就是真库（不是 MemoryBackgroundStore 占位）', () {
      expect(
        src.contains('_store = createBackgroundStore()'),
        isTrue,
        reason: 'createBackgroundStore 是**同步**构造（open 惰性在实现内部），'
            '根本不必等水合——占位内存库一旦暴露给可写路径，'
            '窗口内导入的字节就进了这个即将被丢弃的实例',
      );
      expect(
        src.contains('MemoryBackgroundStore('),
        isFalse,
        reason: '主入口里不该再有内存占位库（代码路径，不是注释里提一句）',
      );
    });

    test('F-0001-3：落点是「合并到最新偏好」，不是整体覆盖', () {
      expect(
        src.contains('applyBackgroundHydration'),
        isTrue,
        reason: '水合结果只许回填背景域，其余字段保留当前最新值',
      );
      expect(
        src.contains('_prefs = hydrated.prefs'),
        isFalse,
        reason: '这一行就是「窗口期改动被整体抹掉」的元凶',
      );
    });

    test('F-0002-3：删掉 `Future.value(...).timeout(3s)` 这层假兜底', () {
      expect(
        src.contains('Future<BackgroundStore>.value'),
        isFalse,
        reason: 'web 实现是同步 new、不抛也不 await ⇒ 这层 timeout 是死代码，'
            '而它旁边的注释承诺了一个线上不可能发生的「退路内存库」',
      );
      expect(
        src.contains('_hydrateTimeout'),
        isTrue,
        reason: '水合**读**的上限要留着（IndexedDB 可能既不 resolve 也不 reject）',
      );
    });
  });
}
