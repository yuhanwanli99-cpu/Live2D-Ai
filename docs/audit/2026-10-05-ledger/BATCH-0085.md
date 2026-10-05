# BATCH-0085 · `BackgroundStore` 调用侧的第二半：保留集构造 —— **0 条新发现**（接缝关闭）

Phase 1 · 域覆盖 · 前端 `data/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/data/background_hydration.dart` — 255（**读 140-195**：`hydrateBackgrounds` 头段
   与逐项循环；B0084 已读 196-255）

## 跑过的命令（全部只读）
```
sed -n '140,195p' background_hydration.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；B0084 留的接缝**关闭**（且保护是**结构性**的）
### ① 保留集**在任何 I/O 之前**就建好 ⇒ 读失败的项**不可能**被 `retainOnly` 删掉字节
```dart
for (final BackgroundItem item in items) {
  if (item is! BackgroundImage) { continue; }
  keep.add(item.id);                       // :179 ← **先入保留集**，再做任何读/写
  …
  final BackgroundRead read = await _safe(() => store.readChecked(item.id)) ?? …  // :194-195
```
⇒ B0084 核到的是「`healthy && !storeFailed` 这道**闸**」；
本批核到的是**更靠里的一层**：`keep` 与读操作**无关** ⇒ 即使闸被误改，
**读失败的项仍在其 `keep` 里** ⇒ 保护**不依赖**那道闸。**这才是双保险。**

### ② `liveKeptIds` 解决的是**快照与实时 UI 的竞态**（:145-149）
> 「水合窗口里用户可能刚导入一张图，而剪枝是**真的删字节**——只按快照算保留集，
> 就会把这张刚导入、界面已经说『已加进背景库』的图当孤儿删掉（**F-0013-1 的另一半**）」

⇒ 解法不是「小心一点」，而是让调用方**传一个读『此刻』偏好的函数**（而不是用快照）
⇒ 且 `null` 时**退化成旧语义**（:149「既有调用点 / 单测的语义一字不变」）⇒ 向后兼容有保证

### ③ `healthy` 先探，且**区分「空清单」与「库空」**（:155-157）
> 「剪枝（`retainOnly`）会**删字节**，所以先问一句『存储现在能读吗』。
> **空清单可能只是『这一次读不到』，不是『用户清空了库』**。」

### ④ 内联字节迁移失败时**不动该项**（:181-192）
```dart
final bool stored = await _safe(() => store.put(item.id, inline)) ?? false;
if (!stored) {
  // 搬不进去（配额满 / 无痕 / 被禁）→ **不动这一项**（增量里什么都不加），
  // 绝不改写成 id-only：**那等于把字节从两处一起抹掉**（P0-2）。
```
⇒ 与 B0084 的 ③（写入侧 bool 契约）合起来：**搬不进去 ⇒ 偏好里仍留内联字节 ⇒ 刷新后还在**
⇒ 该区至少经过 **P0-1 / P0-2 / F-0013-1 两半** 四次真实事故，**推理全部保留在原地**。

### ⑤ `BackgroundPattern` 被 `continue` 跳过（:176-178）⇒ 图案无字节、不会被误删

## 未核实项（**仍是那条缝**：`BackgroundStore` 真正的实现）
1. `BackgroundStore.retainOnly` / `put` / `delete` / `readChecked` / `probe` **本体未读**
   ⇒ 「`retainOnly` 是否真按传入集合删除」**仍未核实**（连续两批明确标出）
2. `settings_controller.dart:120-322` / `:372-409` 未读
3. `appearance_background.dart`(1551) 未读；`app_shell.dart` 全文未读
4. `shell_admin.dart` 余段 / `main.dart` 其余 ~1000 行未读；其余 Mod 面板（4 个 / 2284 行）未读

## 本批新增
**0 条**
