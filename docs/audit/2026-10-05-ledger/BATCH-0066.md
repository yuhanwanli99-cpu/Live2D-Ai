# BATCH-0066 · 前端换根：spec → 控件 → 提交 的往返链（**自我更正 F-0062-01**）

Phase 1 · 域覆盖 · ⭐ 换根 → 前端（217 个 Dart 文件，0 系统审）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/sections/dev_tools_section.dart` — 1874（**定点 680-716**：
   `_intValue` / `_stringValue` / `_selectValue` / `_buildConfig`）
2. `crates/live2d-ai-desktop/src/web_api/mods_routes.rs` — （定点 483-499：`redacted_config`）
3. `shell/flutter/lib/settings/` 目录规模 — 10184 行 / 11 个文件（`dev_tools_section` 1874 为前端最大 Dart 文件）

## 跑过的命令（全部只读）
```
git ls-files 'shell/flutter/lib/settings' | xargs wc -l | sort -rn
grep -n "ModSettingField::Number|ModSettingField::Select|ModSettingField::Bool|ModSettingField::String|clamp|options" dev_tools_section.dart
sed -n '680,715p' dev_tools_section.dart
grep -n "fn redacted_config" -A 30 mods_routes.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**自我更正 F-0062-01 的机制**（结论不变，机制更准）
我在 B0062 写「前端省略 secret 键 + 宿主整份替换 ⇒ 删除」，并说前端守卫「瞄错了失效形态」。
**错**：前端**做的是合并**（:699 从 `widget.mod.config` 出发，:698-700 的注释写得**完全正确**，
它确实保护了 spec 外的既有键）。
**真机制 = 三段接缝**，secret 死在**第一段**：`redacted_config` 的 `obj.remove(key)`（:494）
把值取走 ⇒ ② 合并基线已空 ⇒ ③ 整份替换 ⇒ 盘上那份也没了。
⇒ **结论不变且更硬**：② 在结构上**不可能**保留它 ⇒ **修法只能是宿主侧**，
且这条同时**否掉了我原本可能提的「前端修法」**。
⇒ **模式 C 升级**：三段各自局部正确，而「把值带过第二段」这个职责**从未被指派给任何人**
⇒ **三段都不需要改意图，只需要给接缝一个责任人**。

## 顺带核到（正面）
- `_selectValue`（:690-693）：**校验值必须属于 `options`**，否则回落 `options.first`
  ⇒ 前端不会提交一个 spec 里不存在的枚举值（与宿主侧的 select 校验同向）
- `_intValue`（:678-682）：`num → toInt` / `String → int.tryParse ?? 0` ⇒ 坏值回落 0 而非崩

## 未核实项
1. `Number` 的 `min`/`max` **前端是否钳位**未核（:680 附近只见到 `toInt`/`tryParse`，
   看起来**没有钳位** ⇒ 可能把越界值发给宿主，由宿主 `clamp`）——**本批未结论化**
2. `ui/field_row.dart`（团队 F-0012-1：自检结果被渲染成空）未读
3. `settings_controller.dart`(409) / `display_prefs.dart`(1157) 未读
4. `app/` 侧（`shell_admin` / `shell_prefs` / `app_shell`）未读 —— 模式 G 前端侧排 B0067
5. 其余 6 个 Mod 面板（`memory_panel` 694 / `voice_input_panel` 610 / `persona_panel` 608 …）未读

## 本批新增
**0 条新缺陷** ｜ **1 处自我更正**（F-0062-01 的机制描述）+ 模式 C 升级
