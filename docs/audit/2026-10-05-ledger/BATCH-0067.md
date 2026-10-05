# BATCH-0067 · min/max 两侧口径 + field_row 放大器复核

Phase 1 · 域覆盖 · 前端

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/sections/dev_tools_section.dart` — 1874（定点 1000-1033：四种 kind 的控件构造）
2. `shell/flutter/lib/ui/field_row.dart` — （定点 600-625：`result` / `resultIsError` / `error:` 绑定）
3. 跨语言对照（已读）：`mod-system/src/settings.rs:74-80`（`validate` 只查 key 重复）

## 跑过的命令（全部只读）
```
grep -n "\.min\b|\.max\b" dev_tools_section.dart
grep -rn "clamp" mod-system/src/settings.rs mod-external-input/src/lib.rs
sed -n '1000,1039p' dev_tools_section.dart
sed -n '600,625p' ui/field_row.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**F-0067-01（P3）** + 一条独立复核
1. **F-0067-01（P3）**：`min`/`max` **前端强制**（`NumberField` 对越界输入**不回调** ⇒
   `_values` 进不了越界值 ⇒ `_buildConfig` 不发）—— 我原先怀疑「未钳位」**证伪**；
   但**宿主侧零强制**（`clamp` 零命中 + `validate` 只查 key 重复 + 整份替换不校验）
   ⇒ 不变式「盘上数字在 spec 范围内」**只在所有写入都经该 UI 时成立**。
   定 P3（无安全/数据影响，且**无**任何契约声明说宿主会校验 —— 与 F-0060-01 不同类）
2. **团队 F-0012-1（P1）放大器在 HEAD 仍成立**：`field_row.dart:617`
   `error: resultIsError ? result : null` + `result` 全文件只此一处
   ⇒ 上游误判成功 ⇒ 界面**什么都不显示**，与「通过」「没测过」不可区分

## 未核实项
1. `NumberField` 的实现未读（我据其**注释与调用点**判定「不回调」；
   **若它内部实际是 clamp 后回调**，则前端会**静默改写**用户输入 —— 那是另一个问题，
   **未核实**，已标进 STATE）
2. `ui/field_row.dart` 其余部分未读；`settings_controller.dart`(409) 未读
3. `display_prefs.dart`(1157) / `appearance_*.dart` 未读
4. `app/` 侧未读（模式 G 前端侧排 B0068）

## 本批新增
P0 0 · P1 0 · **P2 0** · **P3 1** ｜ 另：团队 F-0012-1 放大器 HEAD 复核（仍成立）
