# BATCH-0034 · 序列化方向 + 判等面

Phase 1 · 域覆盖 → 前端层

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/display_prefs.dart` — 1157（读 96-170 / 537-632 / 865-880 / 942-990）
2. `shell/flutter/lib/design/background_item.dart` — （读 77-96 / 355-435）
3. `shell/flutter/lib/settings/sections/appearance_background.dart` — 1551（定点 946-972 / 1200-1300）
4. `shell/flutter/lib/settings/sections/appearance_section.dart` — 710（定点 206-240）
5. `shell/flutter/lib/main.dart` — （定点 237-241 落盘门）

## 跑过的命令（全部只读）
```
grep -n "Map<String, Object?> toJson" -A 42 display_prefs.dart
grep -n "factory DisplayPrefs.fromJson" -A 45 display_prefs.dart
grep -n "operator ==" -A 32 display_prefs.dart ; grep -n "int get hashCode" -A 8 display_prefs.dart
grep -n "sameAs" -A 18 design/background_item.dart
grep -rn "copyWith(opacity|onItemChanged|onStyleChanged|updateBackgroundItem" shell/flutter/lib/
grep -rn "onItemChanged" shell/flutter/lib/ | grep -v appearance_background.dart   # → 零命中
grep -rn "onReorderItem:" shell/flutter/lib/
sed -n '212,239p' appearance_section.dart ; sed -n '1200,1300p' appearance_background.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 E（契约一致性）：`toJson` ↔ 构造参数 **24/24 对齐**；`==` **24/24 覆盖**
- 维度 A（真源）：落盘门 `if (next == _prefs) return true` 的正确性依赖 `==` 完整 —— **已证完整**
- 维度 H/I：逐图样式入口在无回调时不渲染（优雅降级）✔
- 潜伏陷阱：F-0034-01（P3）

## 未核实项
1. **是否有测试钉住「样式控件在无回调时不出现」或「样式改动会被吞」**——若已覆盖，
   F-0034-01 应降为纯备忘（Dart 测试面尚未系统查）
2. `display_prefs.dart` 的 `copyWith` 家族未读（`copyWith` 少一个字段是**编译期**问题，
   优先级低于 `==`，故本批未查）
3. `hashCode` 只读到 :984（确认它是**子集**哈希，缺字段只降低分布、不违反 `==` 契约）
4. `main.dart` 仍有约 1000 行未读

## 本批新增
P0 0 · P1 0 · P2 0 · **P3 1**
