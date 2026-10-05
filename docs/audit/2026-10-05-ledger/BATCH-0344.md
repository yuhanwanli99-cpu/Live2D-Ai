# BATCH-0344 · ⭐⭐⭐ **这道门禁防的不是违规，是「过期的合规」**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `test/font_subset_test.dart`(341) 这道字体子集门禁

## 跑的命令（全部只读）
```
wc -l test/font_subset_test.dart ; grep -nE "scan|lib/|ranges|expect|glob|RegExp" test/font_subset_test.dart
grep -nE "字体字节数|fnv|覆盖表|不符|换字体|expect\(" test/font_subset_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **本审计最强的门禁样本**
> 「# 覆盖表为什么是「**生成文件 + 哈希**」                                        // :26
> 覆盖表从字体文件派生（`scripts/font_subset_ranges.py`）。若有人**换了字体又忘了**重新生成覆盖表，
> 测试就会**拿旧表放行——比没有门禁更危险**。所以这里**记下字体字节数与 FNV-1a(32) 哈希，
> 这里重新计算比对，不一致就要求重新生成**。」                                   // :28-30
```
四个可核点：
1. ⭐⭐⭐ **它防的不是「违规」，而是「过期的合规」**
   ⇒ ⇒ 「**拿旧表放行 —— 比没有门禁更危险**」⇒ ⇒ **这句判断本身就是可迁移的**
2. ⭐⭐⭐ **解法是让生成物携带自己的出处**：覆盖表**记下字体的字节数 + FNV-1a(32) 哈希**，
   测试**重新计算比对** ⇒ ⇒ **换字体而不重生表 ⇒ 测试变红**
   ⇒ ⇒ 与 B0204 的 `toValuesMap()` / B0292 的 `AppDurations.registry` / B0335 的穷举 switch **同族**：
   **生成物自带出处 ⇒ 陈旧被检测而不是被信任**
3. ⭐⭐ **哈希算法在测试里被逐字节复刻**（`:43-44` `fnv1a32` + 注释「与 `scripts/font_subset_ranges.py`
   的实现**逐字节等价**」）⇒ ⇒ **不是「读同一个文件」而是「重算同一个函数」** ⇒ ⇒ **两侧实现漂移会被发现**
4. ⭐⭐ **B0343 的修法被一条测试钉住**（`:227-228`）：
   `expect(lits, contains('正文'));` + `expect(lits.any((String s) => s.contains('▍')), isFalse);`
   ⇒ ⇒ **那个字符被明确禁止回来** ⇒ ⇒ **「纯绘制」这个修法不会悄悄退化**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批回答了一个我此前没问的问题**：
**一道门禁值多少，取决于它防的是哪一类失效** ——
**防「有人写错」的门禁很多；防「自己过期了」的门禁罕见，而后者更危险**（因为它给出假绿）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~450 / `chat_panel` ~440）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
