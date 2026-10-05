# BATCH-0331 · ⭐ 还原规则被写成「**还原成什么**」而不是「会不会还原」—— 而两侧措辞一致

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— persona 面板的**停用还原承诺**（Mod 侧 B0220 已核）

## 跑的命令（全部只读）
```
grep -nE "停用|还原|基线|baseline|关闭开关" lib/settings/mods/persona_panel.dart
sed -n '389,398p' lib/settings/mods/persona_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **可核对的断言 vs 承诺**
```dart
SectionHeader(
  title: '角色卡（人设）',
  description: '导入一张卡，之后打开开关即生效，**关闭开关会还原成主链 system_prompt**。
  默认绑定当前会话；也可以选择写入全局人设（所有…）',                      // :393-394
```
四个可核点：
1. ⭐⭐ **它说清了「关闭开关会还原成什么」** —— 不是「会还原」，而是「**还原成主链 `system_prompt`**」
   ⇒ ⇒ **可核对** ⇒ ⇒ 与 **B0220 已核的 Mod 侧**（停用 → 还原到 `persona-mod-base.txt` 基线
   **即还原成主链那份**）**完全一致**
   ⇒ ⇒⇒ **两侧措辞也一致**（B0324 那条「同一件事两侧各说一句」的又一实例 —— **这次是同句**）
2. ⭐⭐ **整条链被写成一句话的顺序**：「导入一张卡 → 之后打开开关即生效 → 关闭开关会还原成主链」
   ⇒ ⇒ 而面板头注（`:4`）说的**也是这同一条链** ⇒ ⇒ **头注与界面文案说的是同一件事**
3. ⭐ **「默认绑定当前会话」在同一段里** ⇒ ⇒ **作用域与还原规则放在同一句** ⇒ 用户一次读完
4. ⭐ `:131` `'has_base_snapshot': '**基线已记录**'` ⇒ ⇒ **「基线在不在」也是可见状态**
   （B0220 已核 Mod 侧有这个字段）

⇒ ⇒ **0 findings**；⇒ ⭐ 而本批的写法值得记：
**「用可核对的断言代替形容」** —— 「还原成主链 `system_prompt`」**可以被核对**，
而「关闭开关会还原」只是**一个承诺** ⇒ ⇒ 与 B0329 的「两个理由都不是性能」**同族**：
**把「我保证了什么」写成「可验证的那一句」**

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~460）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
