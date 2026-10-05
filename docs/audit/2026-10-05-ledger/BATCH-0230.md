# BATCH-0230 · `memory_panel.dart`：**所有变更走同一条路**，而那条路上有**重入闩**与**四处 `mounted`**

Phase 1 · 域覆盖 · 前端 `.dart`（40/218）—— `lib/settings/mods/memory_panel.dart`(694)

## 跑的命令（全部只读）
```
grep -nE "onCommand|\"edit\"|\"delete\"|\"import\"|index|where\(|firstWhere" memory_panel.dart
sed -n '246,300p' memory_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**两处纪律核验通过**
### ① 所有主动变更**走同一条路**（P2 形状）
```dart
/// 所有主动动作走同一条路：命令 → 成功/失败文案 → 刷新运行态 + 刷新列表
/// 通知宿主「行为变了，可能要重启/重新点火」。            // :283-284
Future<void> _send(String action, String command, Map<String, Object?> args, …) async { … }
```
⇒ ⇒ **一个漏斗** ⇒ ⇒ 「变更后要刷新」这件事**不可能只对某一类动作做到**
⇒ 且它的**尾巴包含两件必做的事**：刷新运行态 + 刷新列表 + **通知宿主「可能要重启/重新点火」**
⇒ ⇒ ⭐ 而这第三件**正是 Mod 契约要求的**（Mod 改完配置 ⇒ 宿主要重载）⇒ **面板与契约在同一个漏斗上对齐**

### ② ⭐ 而那个漏斗上有**重入闩**，且每个 await 后都有 `mounted` 检查
```dart
if (_busy) return;              // :292  ⭐ 重入闩：一次命令在飞时，后续动作被丢弃
…
if (!mounted) return;            // :250 / :258 / :269 / :296   **四处** async 方法各一
```
⇒ ⇒ **「双击」在 UI 层就被挡住** ⇒ ⇒ 与后端自己的守卫**互补**（不是替代）
⇒ ⇒ 而 `mounted` 检查**四处都有** ⇒ ⇒ **「await 之后组件已卸载」这条纪律是成规模应用的**，不是某一处顺手写的

### ③ ⭐ 而这让 B0192 的那个观察**变得完整**（我当时的记录不够准）
B0192 我记「`:224` 按钮**从不禁用**（无 `loading ? null`）」⇒ 看上去是缺口。
本批看到 `_send` **有** `if (_busy) return;` ⇒ ⇒ ⇒ **闩出现在「会改状态」的地方**，
而**不出现在「只读」的地方**（`load()` 只写 `_remote`/`_draft`，但那个入口的按钮不涉及重复**变更**）
⇒ ⇒ **这不是不一致，是一个连贯的设计** ⇒ ⇒ **修正我 B0192 记录的措辞**
（**B0192 那个观察本身仍成立**：F-0189-01 的竞态在 `load()` 上**确实**没有被闩住）

## 未核实项
1. `memory_panel.dart` 余约 600 行未读（记录行的编辑对话框、删除确认、导入、清空整库、桶降级说明的 UI）
2. `persona_panel.dart`(608) **本体未读**（导入链 UI · 坏卡失败处置 · 与 memory 的 last-writer-wins 呈现）
3. `message_bubble.dart` 余约 500 行 · `chat_panel.dart` 余 540 行 · `error_banner.dart` 余 45 行未读
4. `director_observer_section.dart`(668) · `live2d_stage.dart`(666) 未读
5. 前端 `.dart` 仍 178 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
