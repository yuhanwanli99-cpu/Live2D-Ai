# BATCH-0084 · `background_hydration.dart:140-255` —— **0 条新发现**（本审计最细的错误处理推理之一）

Phase 1 · 域覆盖 · 前端 `data/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/data/background_hydration.dart` — 255（**读 140-255**：`hydrateBackgrounds` 尾段 /
   `storeBackground` / `forgetBackground` / `_safe`）

## 跑过的命令（全部只读）
```
sed -n '196,255p' background_hydration.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；四个决策各自**点名它防的用户可见症状**
### ① `missing` 与 `failed` 是**两件事**（:200-211）
```dart
case BackgroundReadKind.missing:
  // 存储**明确回答**「没有这一项」……**摘掉它**，
  // 而不是留一个永远画不出来的空位——**那会让「背景库：3 项」变成界面上的谎话**。
  pruned.add(item.id);
case BackgroundReadKind.failed:
  // 读**失败** ≠ 没有（P0-1）：增量里什么都不加（项原样保留），等下一次启动再补。
  storeFailed = true;
```
⇒ **读失败不被当成「不存在」** —— 这正是「一次存储抖动 ⇒ 用户的图集体消失」的那类错误。
### ② 破坏性剪枝**双重门控**（:214-218）：`if (healthy && !storeFailed)`
⇒ 存储不健康或本轮有失败时，**不删任何字节** ⇒ 抖动不可能造成批量删除
### ③ 写入侧有 bool 契约 + 理由（:230-233）
```dart
/// 返回 `false` 时调用方**必须如实告诉用户**（配额满 / 无痕 / 站点数据被禁），
/// 绝不能先更新偏好再假装成功——**那会得到「界面上有、刷新后消失」**。
```
⇒ 与 ① 读侧构成本条链的**读写两端**（① 防「库里有、界面说没有」；③ 防「界面有、库里没有」）
### ④ 吞异常**与返回信号成对**（:244-255）
```dart
/// 理由与 [BackgroundStore] 头注同一条：存储是**平台能力**，不是业务逻辑。
/// 它挂了（隐私模式、被禁、磁盘满）时，正确的反应是「这次存不下」，
/// 而不是把异常抛穿整个启动流程。
Future<T?> _safe<T>(Future<T> Function() action) async { try { … } catch (_) { return null; } }
```
⇒ 关键在**成对**：`Future<T?>` 让调用方**能**察觉 ⇒ 这是「吞掉并上报」，
**不是**「吞掉并说谎」。`forgetBackground` 另标**幂等**（:240）。

## ⭐ 由此记一条**正面模式**（第 3 个样本，已成风格）
> **理由的写法：每个非平凡决策都配一句「不这么做的话，用户会看到什么」。**
> 样本三处（跨两批两文件）：
> ① `background_hydration.dart:203` 「那会让『背景库：3 项』**变成界面上的谎话**」
> ② `background_hydration.dart:233」**那会得到「界面上有、刷新后消失」**
> ③ `app_shell.dart:664`（B0083）实际面板已退回不透明——「**界面在骗人**」
> ⇒ 这比「请保持一致 / 请妥善处理」有用得多：**它让下一个读代码的人能判断这条约束还在不在**。
> （与模式 D 恰好互为镜像：D 是注释与事实脱节；这条是注释**主动**指向可核的用户可见症状。）

## 未核实项
1. `background_hydration.dart:1-90` 未读（头注 + `BackgroundHydration` 字段定义 + `prefs` getter）
2. `settings_controller.dart:120-322` 未读（本批被 hydration 占满）
3. `BackgroundStore` 本体（`retainOnly` / `put` / `delete` / `read` 的实现）未读 ——
   **注意**：上面 ①②③④ 核的是**调用侧的处置**，`retainOnly` 是否真按传入集合删除，**未核实**
4. `app_shell.dart` 全文未读；`shell_admin.dart` 余段 / `main.dart` 其余 ~1000 行未读
5. `appearance_background.dart`(1551) 未读；其余 Mod 面板（4 个 / 2284 行）未读

## 本批新增
**0 条**（+ 一条正面模式的第 3 个样本）
