# BATCH-0365 · ⭐⭐⭐ **「不回调」配「看得见界」** —— 越界不是静默的

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `field_row.dart` 的 `NumberField`（B0364 引用、刚挂的未核项）

## 跑的命令（全部只读）
```
f=$(grep -rl "class NumberField" lib/ | head -1)          # ⇒ lib/ui/field_row.dart
grep -nE "min|max|clamp|onChanged" lib/ui/field_row.dart | sed -n '20,38p'
sed -n '424,440p' lib/ui/field_row.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **B0364 引用的那句「越界不回调」确实成立**
```dart
onChanged: (String raw) {
  final int? parsed = **int.tryParse(raw.trim())**;
  if (parsed == null) return;                                        // ① 解析不出 → 不回调
  if (widget.min != null && parsed < widget.min!) return;            // ② 越下界 → 不回调
  if (widget.max != null && parsed > widget.max!) return;            // ③ 越上界 → 不回调
  widget.onChanged(parsed);
},
decoration: InputDecoration(
  isDense: true, border: const OutlineInputBorder(),
  helperText: widget.min == null && widget.max == null ? null
    : '${widget.min ?? '−∞'} – ${widget.max ?? '∞'}'),               // ④ 界可见
```
五个可核点：
1. ⭐⭐⭐ **B0364 说的「越界不回调」确实成立** —— **三道 `return` 各挡一类**
2. ⭐⭐ **检查顺序是「先能解析 → 再比界」** ⇒ ⇒ **非法输入在第一道就被挡**，不会走到比较
3. ⭐⭐ **`int.tryParse(raw.trim())`** ⇒ ⇒ **前后空格不导致误判**
   （同 B0120 核过 `secrets.rs` 的 parse 严格度；**同一个细致度**）
4. ⭐⭐⭐ **而越界不是静默的**：`helperText` **把范围显示出来**（`'${min ?? '−∞'} – ${max ?? '∞'}'`）
   ⇒ ⇒⇒ **「不回调」配「看得见界」** ⇒ ⇒ **用户知道自己被挡在哪儿**
   ⇒ ⇒⇒ ⭐ **而 min/max 都为 null 时 `helperText` 是 null（不显示）**
   ⇒⇒ **没有界就不画界** ⇒ ⇒ **同 B0364 的「spec 没说边界就没有边界」**
   ⇒⇒⇒ **界面的「显示」与「判断」用同一个来源**
5. ⇒ ⇒ **「让缺陷无法发生」又一处**，而这次是「**无法发生 + 看得见原因**」
   ⇒ ⇒⇒ **两者成对**（B0353 只有前者）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `AdminEmpty` 本体 ·
   `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1800 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425)
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
