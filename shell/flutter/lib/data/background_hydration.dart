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

  /// 接好字节的偏好（缺字节的项已被摘掉）。
  final DisplayPrefs prefs;

  /// 偏好**变了**（搬了旧档或摘掉了孤儿）⇒ 调用方要把它写回盘。
  ///
  /// 为什么要回写：旧档里那张图如果不从 localStorage 里搬走，
  /// 它会**永远留在偏好里**（下次启动又被搬一次），localStorage 也就
  /// 永远清不掉那份大 base64——那正是这次要解决的病根。
  final bool migrated;
}

/// 把 [prefs] 里的背景库接到 [store] 上。**永不抛**。
Future<BackgroundHydration> hydrateBackgrounds(
  DisplayPrefs prefs,
  BackgroundStore store,
) async {
  final List<BackgroundItem> items = prefs.backgrounds;
  if (items.isEmpty) {
    // 库是空的 → 顺手把字节库里没人要的东西清掉。
    await _safe(() => store.retainOnly(<String>{}));
    return BackgroundHydration(prefs, migrated: false);
  }

  final List<BackgroundItem> resolved = <BackgroundItem>[];
  final Set<String> keep = <String>{};
  bool changed = false;
  for (final BackgroundItem item in items) {
    if (item is! BackgroundImage) {
      resolved.add(item);
      continue;
    }
    keep.add(item.id);
    // 旧档：字节就在偏好里（那时它们住在 localStorage）。搬一次。
    final String? inline = item.dataUrl;
    if (inline != null) {
      changed = true;
      await _safe(() => store.put(item.id, inline));
      resolved.add(item);
      continue;
    }
    final String? bytes = await _safe<String?>(() => store.read(item.id));
    if (bytes == null) {
      // 字节库里没有这一项：要么浏览器清了站点数据，要么上一版写的
      // 清单指向一张已经不在的图。**摘掉它**，而不是留一个永远画不出来的
      // 空位——那会让「背景库：3 项」变成界面上的谎话。
      changed = true;
      continue;
    }
    resolved.add(BackgroundImage(id: item.id, dataUrl: bytes));
  }

  await _safe(() => store.retainOnly(keep));
  if (!changed) {
    // 内容没变，但 [dataUrl] 是补上去的 —— 仍然要交回新的 prefs，
    // 否则渲染层拿到的还是那些 `dataUrl == null` 的项。
    return BackgroundHydration(
      prefs.copyWith(
        backgrounds: List<BackgroundItem>.unmodifiable(resolved),
      ),
      migrated: false,
    );
  }
  return BackgroundHydration(
    prefs.copyWith(backgrounds: List<BackgroundItem>.unmodifiable(resolved)),
    migrated: true,
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