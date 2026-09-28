/// 启动时的**背景字节对接**：把偏好里那一串 id 换成能画的图。
///
/// # 谁调用、什么时候调用
///
/// [hydrateBackgrounds] 由应用根在 `initState` 之后调一次（`main.dart`）。
/// 它是**唯一**允许「偏好里的图还没有字节」存在的地方：
/// [DisplayPrefs.fromJson] 读出来的图片项只有 `id`，`dataUrl` 是 `null`。
///
/// # 三件事，一次做完
///
/// 1. **补字节**：`dataUrl == null` 的项去字节库里读；
/// 2. **搬旧档**：`dataUrl != null` 的项（2026-09-27 之前存进 localStorage 的）
///    写进字节库，然后把偏好改写成「只有 id」的清单；
/// 3. **剪枝**：库里没人引用的记录删掉（删掉的图不会一直占着磁盘）。
///
/// # 一条铁律：**读不到 ≠ 没有**（P0-1，2026-09-27）
///
/// 存储会**瞬时故障**（IndexedDB 打不开 / 超时 / 被隐私设置挡掉）。
/// 老实现里 [BackgroundStore.read] 把「故障」与「键不存在」都返回 `null`，
/// 于是对接层把**还在库里的图**当成孤儿摘掉、置 `migrated`，
/// 调用方再把这份清单写回盘；下次启动清单为空 → `retainOnly({})` →
/// **字节真被删掉**。整条链的入口就是「把读失败当成没有」。
///
/// 现在的规矩：
///
/// - 读用 [BackgroundStore.readChecked]（三态）：只有 `missing` 才摘项；
///   `failed` 一律**原样保留**，并且**不置 `migrated`**；
/// - 搬旧档看 [BackgroundStore.put] 的返回值：写不进去就保留旧 `dataUrl`，
///   绝不改写成 id-only（那等于把字节从两处一起抹掉，P0-2）；
/// - [BackgroundStore.retainOnly] 是**破坏性**动作，只在
///   [BackgroundStore.probe] 健康**且本轮没有任何失败**时才调用。
///
/// # 为什么不把这些塞进 `DisplayPrefs.fromJson`
///
/// `fromJson` 是**纯同步**的（它得能在任何地方被直接调用，包括测试），
/// 而读字节库必然是异步的；把异步塞进去会让「读偏好」这条最热的路径
/// 变成 Future，而它其实只该做反序列化。
library;

import '../design/background_item.dart';
import '../settings/display_prefs.dart';
import 'background_store.dart';

/// 一次对接的结果——**背景域的增量**，不是整份偏好（F-0013-1，2026-09-28）。
///
/// # 为什么必须是增量
///
/// 水合是异步的（开 IndexedDB + 每张图一次 `readChecked`），窗口长度 ∝ 库体积，
/// 上限 3 s。窗口里用户完全可能改主题 / 音量 / 透明度，或者导入 / 删除 / 重排
/// 背景库。旧实现交回**整份偏好**（派生自水合开始时刻的快照），调用方
/// `_prefs = hydrated.prefs` 一盖，窗口期的改动全部回滚；`migrated` 时还会把
/// 这份旧对象写回盘（F-0001-3）。所以这里只描述「背景域要做什么改动」，
/// 由调用方应用到**此刻**的偏好上（[applyBackgroundHydration]）。
class BackgroundHydration {
  const BackgroundHydration({
    this.bytes = const <String, String>{},
    this.pruned = const <String>{},
    this.migrated = false,
    this.source,
  });

  /// 水合**读回来**的字节：`id → dataURL`（补字节）。
  ///
  /// 旧档（字节还在偏好里）搬成功后**不进这里**：那些项本身就已经带着
  /// `dataUrl`，只是序列化时不再写出去（`BackgroundImage.toJson` 只写 id）。
  final Map<String, String> bytes;

  /// 存储**明确回答「没有这一项」**的 id（要摘掉）。
  ///
  /// `readChecked` 的 `failed` **不在里面**——「读失败 ≠ 没有」是 P0-1 的铁律。
  final Set<String> pruned;

  /// 偏好**变了**（搬了旧档或摘掉了确认没有的孤儿）⇒ 调用方要把它写回盘。
  ///
  /// 为什么要回写：旧档里那张图如果不从 localStorage 里搬走，
  /// 它会**永远留在偏好里**（下次启动又被搬一次），localStorage 也就
  /// 永远清不掉那份大 base64——那正是这次要解决的病根。
  ///
  /// **存储故障时永远是 `false`**：读不到 / 写不进都不算「偏好变了」，
  /// 写回盘只会把故障固化下来（P0-1 的第二段）。
  final bool migrated;

  /// 水合开始时刻的那份偏好（**只给 [prefs] 这个派生视图用**）。
  ///
  /// 它不参与线上的合并：窗口期用户的改动不在它里面。
  final DisplayPrefs? source;

  /// **派生视图**：把本增量应用到 [source]（水合开始时刻的快照）。
  ///
  /// ⚠️ 线上路径**不要**用它——窗口期用户的改动不在 [source] 里，
  /// 拿它去覆盖就是 F-0013-1 的病根。它只服务两件事：
  ///
  /// 1. `background_store_test.dart` 那批 P0-1/P0-2 回归读的就是「水合算出了什么」；
  /// 2. 排障时看一眼本次水合的净效果。
  ///
  /// 新代码一律用 [applyBackgroundHydration] 合并到**最新**偏好。
  DisplayPrefs get prefs =>
      applyBackgroundHydration(source ?? const DisplayPrefs(), this);
}

/// 把一次水合的结果**合并到最新的偏好**上（只动背景域）。
///
/// | 增量 | 对最新偏好做什么 |
/// | --- | --- |
/// | [BackgroundHydration.bytes] | 同名项补 `dataUrl`（走 `copyWith`，**逐图样式一字不动**）|
/// | [BackgroundHydration.pruned] | 摘掉「存储明确说没有」的项 |
/// | 其余字段（主题 / 音量 / 透明度 / 轮播 / 背景来源…） | **一律不碰** |
///
/// 窗口里新导入的项既不在 [BackgroundHydration.pruned] 里、也不在
/// [BackgroundHydration.bytes] 里，所以**原样留在清单里**；它的字节是从
/// **真库**写进去的（`_store` 从 `initState` 起就是真库，见 F-0002-2），
/// 水合的剪枝也会因为调用方传了 `liveKeptIds` 而留住它。
DisplayPrefs applyBackgroundHydration(
  DisplayPrefs latest,
  BackgroundHydration hydration,
) {
  if (hydration.bytes.isEmpty && hydration.pruned.isEmpty) return latest;
  final List<BackgroundItem> next = <BackgroundItem>[];
  bool changed = false;
  for (final BackgroundItem item in latest.backgrounds) {
    if (item is! BackgroundImage) {
      next.add(item);
      continue;
    }
    if (hydration.pruned.contains(item.id)) {
      changed = true;
      continue;
    }
    final String? url = hydration.bytes[item.id];
    if (url == null || url == item.dataUrl) {
      next.add(item);
      continue;
    }
    // **必须走 copyWith**：直接 `new BackgroundImage(id: …)` 会把用户给这张图
    // 设的 opacity / fit / align 一起清空（F-0013-1 的硬要求）。
    next.add(item.copyWith(dataUrl: url));
    changed = true;
  }
  if (!changed) return latest;
  return latest.copyWith(backgrounds: List<BackgroundItem>.unmodifiable(next));
}

/// 把 [prefs] 里的背景库接到 [store] 上。**永不抛**。
///
/// [liveKeptIds] 是**剪枝时的额外保留集**，由调用方提供并读「此刻」的偏好
/// （不是 [prefs] 这份快照）。为什么需要它：水合窗口里用户可能刚导入一张图，
/// 而剪枝是**真的删字节**——只按快照算保留集，就会把这张刚导入、界面已经说
/// 「已加进背景库」的图当孤儿删掉（F-0013-1 的另一半）。
/// 缺省 `null` = 只按快照算（既有调用点 / 单测的语义一字不变）。
Future<BackgroundHydration> hydrateBackgrounds(
  DisplayPrefs prefs,
  BackgroundStore store, {
  Set<String> Function()? liveKeptIds,
}) async {
  // 剪枝（retainOnly）会**删字节**，所以先问一句「存储现在能读吗」。
  // 空清单可能只是「这一次读不到」，不是「用户清空了库」。
  final bool healthy = await _safe(() => store.probe()) ?? false;
  final List<BackgroundItem> items = prefs.backgrounds;
  if (items.isEmpty) {
    // 库是空的 → 顺手把字节库里没人要的东西清掉（**只在存储健康时**）。
    // 保留集仍要带上「此刻」的 id：窗口里刚导入的那张就在里面。
    if (healthy) {
      await _safe(
        () => store.retainOnly(liveKeptIds?.call() ?? const <String>{}),
      );
    }
    return BackgroundHydration(source: prefs);
  }

  final Map<String, String> bytes = <String, String>{};
  final Set<String> pruned = <String>{};
  final Set<String> keep = <String>{};
  bool changed = false;
  bool storeFailed = false;
  for (final BackgroundItem item in items) {
    if (item is! BackgroundImage) {
      continue;
    }
    keep.add(item.id);
    // 旧档：字节就在偏好里（那时它们住在 localStorage）。搬一次。
    final String? inline = item.dataUrl;
    if (inline != null) {
      final bool stored =
          await _safe(() => store.put(item.id, inline)) ?? false;
      if (!stored) {
        // 搬不进去（配额满 / 无痕 / 被禁）→ **不动这一项**（增量里什么都不加），
        // 绝不改写成 id-only：那等于把字节从两处一起抹掉（P0-2）。
        storeFailed = true;
        continue;
      }
      changed = true;
      continue;
    }
    final BackgroundRead read =
        await _safe(() => store.readChecked(item.id)) ??
        const BackgroundRead.failed();
    switch (read.kind) {
      case BackgroundReadKind.found:
        bytes[item.id] = read.dataUrl!;
      case BackgroundReadKind.missing:
        // 存储**明确回答**「没有这一项」：要么浏览器清了站点数据，
        // 要么上一版写的清单指向一张已经不在的图。**摘掉它**，
        // 而不是留一个永远画不出来的空位——那会让「背景库：3 项」
        // 变成界面上的谎话。
        pruned.add(item.id);
        changed = true;
      case BackgroundReadKind.failed:
        // 读**失败** ≠ 没有（P0-1）：增量里什么都不加（项原样保留），
        // 等下一次启动再补。
        storeFailed = true;
    }
  }

  // 破坏性的剪枝只在「存储健康」且「本轮一次都没失败」时做。
  if (healthy && !storeFailed) {
    final Set<String> live = liveKeptIds?.call() ?? const <String>{};
    await _safe(() => store.retainOnly(<String>{...keep, ...live}));
  }
  // 注意：`bytes` 里带着补回来的字节，所以即使 `migrated == false`，
  // 调用方也仍然要**合并**（[applyBackgroundHydration]）——否则渲染层拿到的
  // 还是那些 `dataUrl == null` 的项。
  return BackgroundHydration(
    bytes: bytes,
    pruned: pruned,
    migrated: changed,
    source: prefs,
  );
}

/// 存一张新图；返回**是否真的存进去了**。
///
/// 返回 `false` 时调用方必须**如实告诉用户**（配额满 / 无痕 / 站点数据被禁），
/// 绝不能先更新偏好再假装成功——那会得到「界面上有、刷新后消失」。
Future<bool> storeBackground(
  BackgroundStore store,
  String id,
  String dataUrl,
) async => await _safe(() => store.put(id, dataUrl)) ?? false;

/// 删一张图。**幂等**：字节库里没有也不报错。
Future<void> forgetBackground(BackgroundStore store, String id) =>
    _safe(() => store.delete(id));

/// 跑一条存储操作，**把任何异常压成 `null`**。
///
/// 理由与 [BackgroundStore] 头注同一条：存储是**平台能力**，不是业务逻辑。
/// 它挂了（隐私模式、被禁、磁盘满）时，正确的反应是「这次存不下」，
/// 而不是把异常抛穿整个启动流程。
Future<T?> _safe<T>(Future<T> Function() action) async {
  try {
    return await action();
  } catch (_) {
    return null;
  }
}
