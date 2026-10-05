# BATCH-0358 · ⭐⭐ **「旧 Mod 保持一行 + 开关」被标为「M2 明确要求的兼容面」** —— 「刻意没有什么」第二例

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `dev_tools_section` 的 **Mod 列表行**

## 跑的命令（全部只读）
```
sed -n '520,545p' lib/settings/sections/dev_tools_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **「刻意降级」被声明为要求**
```dart
const AdminEmpty(icon: Icons.extension_outlined,
  title: '没有注册任何 Mod',
  hint: 'Mod 通过 **live2d-ai-mod-system 的 trait 注册中心**接入。')            // :520-524
…
if (m.settingsSpec == null || m.settingsSpec!.fields.isEmpty)
  AdminRow(
    title: m.name.isEmpty ? **m.id** : m.name,                              // :531
    subtitle: '${m.id} · v${m.version} · **api v${m.apiVersion}**',          // :532
    // **状态用文字**（「运行中」/「已停用」），**不靠颜色**。                 // :533
    badges: <String>[m.statusLabel],
    trailing: Switch(value: m.enabled,
      onChanged: busyId != null || onToggle == null ? null : (bool v) => onToggle!(m.id, v)),  // :536-538
  )
else _ModConfigTile(mod: m, busy: busyId != null, …)                        // :542
```
五个可核点：
1. ⭐⭐ **空态说明了「世界如何被填充」**：「Mod 通过 **`live2d-ai-mod-system` 的 trait 注册中心**接入」
   ⇒ ⇒ **不是「没有 Mod」，而是「Mod 从哪来」** ⇒ ⇒ **P1 家族**
2. ⭐⭐⭐ **「有 spec / 无 spec」两条路，而注释写明**：
   「旧 Mod（无 spec）保持『**一行 + 开关**』的现状 —— **这是 M2 明确要求的兼容面**」（`:527-528`）
   ⇒ ⇒ **一个降级形态被声明为「明确要求」** ⇒ ⇒ **不是「还没做」**
   ⇒ ⇒⇒ **「刻意没有什么」的第二例**（第一例：B0333 核的「**枚举里刻意没有 `listening`**」）
   ⇒ ⇒ **同 B0323「新通道承载新语义、旧路径不动」**（B0285 核的 `id` / `presetId` 关系族）
3. ⭐⭐ **「状态用文字（运行中/已停用），不靠颜色」** ⇒ ⇒ **B0245 那条纪律的第 N 次应用**
   （连接状态 · persona 提示 · **Mod 启停**）
4. ⭐ **`title: m.name.isEmpty ? m.id : m.name`** ⇒ ⇒ **不会渲染出一个空行**（B0236「空记录兜底」族）
5. ⭐ **`subtitle` 带 `apiVersion`** ⇒ ⇒ **契约版本是可见的** ⇒ ⇒ 与 B0220 核的
   `descriptor.api_version` **门禁**呼应（**Mod 侧有门禁 · UI 侧可见**）

⇒ ⇒ **0 findings**；⇒ ⭐ **本批与 B0333 构成「刻意减法」的成对样本**：
**一个从枚举里刻意不列 · 一个对旧 Mod 刻意降级** ⇒⇒ **两处都写明「这是要求、不是遗漏」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `AdminEmpty` 本体
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1810 / `live2d_stage` ~580 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~440 / `chat_panel` ~430）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
