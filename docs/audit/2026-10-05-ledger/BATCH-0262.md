# BATCH-0262 · 消息列表**已虚拟化**，而**「不给 itemExtent」也被说明**（P26-b 第二例）

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_panel.dart` 的消息列表

## 跑的命令（全部只读）
```
grep -nE "ListView|itemCount|itemExtent|visible\.length|children:" lib/ui/chat_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ 三点，其中一点是 **P26-b 的第二例**
```dart
: ListView.builder(
    // 从底部开始：IM 的常规。**不**给 itemExtent（**气泡高度可变**）   // :233-234
    …
    itemCount: visible.length,                                            // :237
    … visible[visible.length - 1 - index]                                  // :240
```
1. ⭐ **已虚拟化**（`ListView.builder`）⇒ ⇒ 与 B0227 核过的日志列表**同一纪律**
2. ⭐⭐ **而「不给 `itemExtent`」这个「更快的做法被有意拒绝」被说明了**（气泡高度可变 ⇒ 固定高度会**错**）
   ⇒ ⇒ **P26-b 的第二例**（B0235 之后）⇒ ⇒ 该模式**从观察变成惯例**
3. ⭐ **`visible[length - 1 - index]` = 列表倒序（index 0 = 最新）**
   ⇒ ⇒ 与 B0246 核过的记忆面板「**最新在前**」**同一排序约定** ⇒ ⇒ **约定层面的 P2**（跨两个特性）
4. ⭐ 而 `:25` 里上限的理由（上限存在）与这里的虚拟化**是同一个关切**：
   「500 条……不至于让 `ListView` 的**语义树**与**读屏浏览**退化」
   ⇒ ⇒ **两个决定其实是同一个决定**（正因为它是**带语义树的真列表**，才需要上限）

## 未核实项
1. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~615 · `memory_panel` ~560 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~470 · `error_banner` ~15
2. `ErrorResponse` 定义未读；`MutatingCheckError::as_message()` 未读
3. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
4. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **iframe 重建后宿主是否主动发 `clear`**（B0251 留）—— 未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
