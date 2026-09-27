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

/// 一次对接的结果。
class BackgroundHydration {
  const BackgroundHydration(this.prefs, {required this.migrated});

  /// 接好字节的偏好。
  ///
  /// 能补的补上、存储明确回答「没有」的摘掉；**读失败的项原样保留**
  /// （暂时画不出来，但绝不因为一次故障就删掉）。
  final DisplayPrefs prefs;

  /// 偏好**变了**（搬了旧档或摘掉了确认没有的孤儿）⇒ 调用方要把它写回盘。
  ///
  /// 为什么要回写：旧档里那张图如果不从 localStorage 里搬走，
  /// 它会**永远留在偏好里**（下次启动又被搬一次），localStorage 也就
  /// 永远清不掉那份大 base64——那正是这次要解决的病根。
  ///
  /// **存储故障时永远是 `false`**：读不到 / 写不进都不算「偏好变了」，
  /// 写回盘只会把故障固化下来（P0-1 的第二段）。
  final bool migrated;
}

/// 把 [prefs] 里的背景库接到 [store] 上。**永不抛**。
Future<BackgroundHydration> hydrateBackgrounds(
  DisplayPrefs prefs,
  BackgroundStore store,
) async {
  // 剪枝（retainOnly）会**删字节**，所以先问一句「存储现在能读吗」。
  // 空清单可能只是「这一次读不到」，不是「用户清空了库」。
  final bool healthy = await _safe(() => store.probe()) ?? false;
  final List<BackgroundItem> items = prefs.backgrounds;
  if (items.isEmpty) {
    // 库是空的 → 顺手把字节库里没人要的东西清掉（**只在存储健康时**）。
    if (healthy) {
      await _safe(() => store.retainOnly(<String>{}));
    }
    return BackgroundHydration(prefs, migrated: false);
  }

  final List<BackgroundItem> resolved = <BackgroundItem>[];
  final Set<String> keep = <String>{};
  bool changed = false;
  bool storeFailed = false;
  for (final BackgroundItem item in items) {
    if (item is! BackgroundImage) {
      resolved.add(item);
      continue;
    }
    keep.add(item.id);
    // 旧档：字节就在偏好里（那时它们住在 localStorage）。搬一次。
    final String? inline = item.dataUrl;
    if (inline != null) {
      final bool stored =
          await _safe(() => store.put(item.id, inline)) ?? false;
      if (!stored) {
        // 搬不进去（配额满 / 无痕 / 被禁）→ **保留旧 dataUrl**，
        // 绝不改写成 id-only：那等于把字节从两处一起抹掉（P0-2）。
        storeFailed = true;
        resolved.add(item);
        continue;
      }
      changed = true;
      resolved.add(item);
      continue;
    }
    final BackgroundRead read =
        await _safe(() => store.readChecked(item.id)) ??
        const BackgroundRead.failed();
    switch (read.kind) {
      case BackgroundReadKind.found:
        resolved.add(BackgroundImage(id: item.id, dataUrl: read.dataUrl));
      case BackgroundReadKind.missing:
        // 存储**明确回答**「没有这一项」：要么浏览器清了站点数据，
        // 要么上一版写的清单指向一张已经不在的图。**摘掉它**，
        // 而不是留一个永远画不出来的空位——那会让「背景库：3 项」
        // 变成界面上的谎话。
        changed = true;
      case BackgroundReadKind.failed:
        // 读**失败** ≠ 没有（P0-1）：原样保留，等下一次启动再补。
        storeFailed = true;
        resolved.add(item);
    }
  }

  // 破坏性的剪枝只在「存储健康」且「本轮一次都没失败」时做。
  if (healthy && !storeFailed) {
    await _safe(() => store.retainOnly(keep));
  }
  // 内容没变时也**仍然要交回新的 prefs**——`dataUrl` 是补上去的，
  // 否则渲染层拿到的还是那些 `dataUrl == null` 的项。
  return BackgroundHydration(
    prefs.copyWith(backgrounds: List<BackgroundItem>.unmodifiable(resolved)),
    migrated: changed,
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
