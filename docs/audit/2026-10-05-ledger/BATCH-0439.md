# BATCH-0439 · ⭐⭐⭐ **第二个实例也有自我证明** —— 而**形态不同** ⇒ 修正 B0438 那句「唯一一处」

Phase 4 · **证伪**（兑现 B0438 留的诚实标注：再核一个**结构测试**有没有自我证明）

## 跑的命令（全部只读）
```
grep -nE "^\s*///|^\s*test\(|group\(|expect\(" test/stage_overlay_single_test.dart | head -14
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**自我证明有两种形态，本仓两处都有**
| | `visual_language_test`（B0381 已核） | **本批 `stage_overlay_single_test`** |
|---|---|---|
| 手段 | **扫源码** + 合成样例 | ⭐ **真 widget** + **存在/不存在成对** |
| 自我证明 | 「扫描器能抓到违规」 | ⭐ **`find.textContaining('加载中') findsNothing`**（不该有的那个） |
| 承认脆弱性 | — | ⭐⭐⭐ **`reason: '找不到 `_StageBadge`，测试需要跟着改'`** |
```
:80-81  expect(find.text('模型加载失败'), findsOneWidget);                 // 正面：恰好一个
:95-96  expect(find.textContaining('模型加载中'), findsOneWidget);        // 正面
:109-110 expect(find.textContaining('加载中'), **findsNothing**);         // ⭐ 反面
        expect(find.text('模型加载失败'), **findsNothing**);              // ⭐ 反面
:112    expect(find.byType(Live2DStage), findsOneWidget);
:116-117 group('P0-2：装饰徽标不带自己的定位')
        test('FPS 徽标不再自带 `Align`（**否则 `Positioned` 的 `right`/`top` 是假的**）')
:124    expect(badge, greaterThan(0), reason: '找不到 `_StageBadge`，**测试需要跟着改**');
:35     /// 把**真的** `Live2DStage` 放进 `StageHost`（**这正是 `main.dart` 的接法**）。
```
四个可核点：
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 第二个核过的实例**（本文件）**有自我证明** ⇒⇒⇒⭐⭐⭐
   **⇒⇒⇒ ⇒⇒ ⇒ ⇒ 而它的方式和 `visual_language_test` 不同**（见上表）
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **「这条纪律在本仓有两种形态、都存在」**
   ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒ 这比 B0438 的「唯一一处」**更准确** —— 而那一句在此被修正**
2. ⭐⭐⭐⭐ **而 `:109-110` 是「唯一性」的**正反面** ⇒⇒⭐⭐⭐ **「恰好一个」+「另一个一个都没有」** ⇒⇒ **比只数正面更严**
3. ⭐⭐⭐⭐⭐⭐ **而 `:124` 的 `reason` 承认了这条测试的脆弱性**：「找不到 `_StageBadge`，**测试需要跟着改**」
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ 这是「自我证明」的另一种写法** ⇒⇒⇒
   **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ ⇒ 它不说「我���会坏」，它说「我坏了会这样说」** ⇒⇒⇒⭐⭐⭐⭐⭐
   **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒ 「测试会随重构一起坏」是结构测试的**固有属性**；
   ⇒⇒⇒⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ **本仓的写法是把这个固有属性写进 `reason`****
4. ⭐⭐⭐ **而 `:35` 明说「**这正是 `main.dart` 的接法**」** ⇒⇒⭐⭐ **「被谁用」被显式指出**
   ⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒ 这与 B0417 核的「不要在别处调」+「调用方：`[_gotoSection]`」同一族**

⇒ ⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ 而本批把 B0438 那句「唯一一处自我证明」**修正为「两种形态都有」** ⇒⇒⇒⭐⭐⭐

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. **剩余 28 个结构测试的自我证明强度未核**（**B0438 的分母仍在；本批只把「已核」从 2 增到 2+1、其中形态不同**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~455）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
