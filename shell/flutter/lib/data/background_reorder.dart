/// 背景库拖动排序的**纯逻辑**。
///
/// # 为什么单独成文件
///
/// 这段算术过去埋在 `app/shell_prefs.dart` 的私有方法里，**没有任何回归**，
/// 于是把 `ReorderableListView.onReorderItem` 的语义理解错了整整一版：
/// SDK 交来的 `newIndex` **已经是**「把被拖那一项移走之后」的目标下标，
/// 应用又减了一次 1——表现是「向下拖早一格、**向下拖一格完全没反应**」。
/// 纯函数放这里就能在 VM 上直接单测（`test/background_reorder_test.dart`）。
library;

import '../design/background_item.dart';

/// 把 [items] 里 [oldIndex] 的那一项移动到结果下标 [newIndex]。
///
/// [newIndex] 用的是 **`ReorderableListView.onReorderItem` 的语义**：
/// SDK 已经为「oldIndex 处少了一项」调整过它，所以这里**不做任何 ±1**，
/// 直接 `removeAt(oldIndex)` 再 `insert(newIndex)`。
///
/// 三种「什么都不用做」的情形都返回**同一个实例**（调用方据此跳过写盘）：
/// [oldIndex] 越界、结果位置与原地相同、或 [items] 为空。
List<BackgroundItem> reorderBackgroundItems(
  List<BackgroundItem> items,
  int oldIndex,
  int newIndex,
) {
  if (oldIndex < 0 || oldIndex >= items.length) return items;
  // **不做 ±1**：onReorderItem 交来的 newIndex 已经是结果下标。
  // 老实现这里的 `if (target > oldIndex) target -= 1;` 是双重调整（P0-3）。
  final int target = newIndex.clamp(0, items.length - 1);
  if (target == oldIndex) return items;
  final List<BackgroundItem> next = List<BackgroundItem>.of(items);
  final BackgroundItem moved = next.removeAt(oldIndex);
  next.insert(target, moved);
  return next;
}
