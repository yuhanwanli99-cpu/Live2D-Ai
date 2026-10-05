# BATCH-0364 · ⭐⭐ **「未知项不隐藏」在字段「值」上又被应用了一次**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `dev_tools_section` 的 **Mod 配置表单取值/校验**

## 跑的命令（全部只读）
```
grep -nE "Select|Number|validator|校验|越界|clamp" lib/settings/sections/dev_tools_section.dart | head -8
sed -n '1012,1024p' lib/settings/sections/dev_tools_section.dart ; sed -n '688,696p' lib/settings/sections/dev_tools_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **同一原则的第二次应用**
```dart
String _selectValue(ModSettingField f) {                 // :690
  final raw = _stringValue(f);                            // 落盘的值
  if (f.options.any((o) => o.value == raw)) return raw;  // 匹配得上就用
  return **f.options.first.value**;                       // :693  ⭐ 落回第一项
}
…
NumberField(min: **f.min?.toInt()**, max: **f.max?.toInt()**,
  // 尊重 spec 的 min/max：`NumberField` 对**越界输入不回调**。            // :1016-1017
  onChanged: (int v) => setState(() => _values[f.key] = v)),
```
四个可核点：
1. ⭐⭐⭐ **落盘的值可能不在当前 spec 的 `options` 里**
   （spec 改过 · Mod 升级过 · 手改过 `mods.json`）
   ⇒ ⇒ 而 `_selectValue` **不把它塞进 dropdown**（那会渲染不出一个匹配项）⇒ **回退到第一项**
   ⇒ ⇒⇒ ⭐ **这是 B0285 那条「未知 key 不隐藏」的同族**：**一次在字段「名」上 · 一次在字段「值」上**
   ⇒ ⇒⇒ **同一原则的第二次应用**
2. ⭐⭐ **min/max 来自 spec**（`f.min?.toInt()`）⇒⇒ **界面的边界 = Mod 声明的边界** ⇒ **不是前端自己定的**
3. ⭐⭐ **「越界输入不回调」** ⇒ ⇒ **越界的值根本不会进 `_values`** ⇒⇒ **也不会被提交**
   ⇒ ⇒⇒ **又是「让缺陷无法发生」**（B0353 的结构预防族）—— **这一条是在**输入层**挡的**
4. ⭐ **`min` / `max` 是可空的** ⇒ ⇒ **spec 没说边界就没有边界** ⇒ ⇒ **不擅自加默认值**
   ⇒ ⇒ 同 B0309 核的 `isProblem` **不含 `connecting`**（**「没声明的就不管」**）

⇒ ⇒ **0 findings**；⇒ ⭐ **本批的收获是「同原则的第二处」**：
**B0285 让我知道「未知项要显形」· B0364 让我知道「已知项的旧值也要有地方落」** ——
**前者防「看不见」，后者防「渲染不出来」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `AdminEmpty` 本体 ·
   `NumberField` 本体（**「越界不回调」的实现未核**）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1800 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425)
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
