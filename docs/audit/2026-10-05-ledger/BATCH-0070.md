# BATCH-0070 · 重核 F-0034-01（按 B0069 教训，不靠旧结论）

Phase 1 · 域覆盖 · 前端

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/display_prefs.dart` — 1157（定点 940-994：`==` 与 `hashCode`）
2. `shell/flutter/lib/design/background_item.dart` — （定点 60-80 / 274-332 / 365-366 / 385 / 426-434 / 494-495）
3. 跨文件对照（已读）：`shell/flutter/lib/app/shell_prefs.dart:475-482`（`sameAs` 的去重调用点）

## 跑过的命令（全部只读）
```
sed -n '950,994p' display_prefs.dart ; sed -n '940,952p' display_prefs.dart
grep -n "sameAs" -A 10 display_prefs.dart ; grep -rn "sameAs(" -A 16 shell/flutter/lib/
sed -n '60,80p' background_item.dart ; grep -n "id:" background_item.dart
grep -rn "String backgroundIdOf" -A 18 shell/flutter/lib/
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**F-0034-01 的「潜在陷阱」证伪**（不记发现）
`backgroundIdOf(dataUrl) = 'bg' + hex(指纹(dataUrl))` ⇒ **id 是内容指纹** ⇒
**同 id 必然同内容** ⇒ id-only 判等**正确**。
三处「我担心」逐一不成立：① 同 id 不同内容不可能；② 逐图样式存在**独立字段**（B0034 已核
`appearance_background.dart:1264` 证明无回调时样式控件不渲染 ⇒ 样式不随 `BackgroundImage` 走），
改样式走另一个字段、`==` 会在那里判出差异；③ `sameAs` 的唯一调用点 `shell_prefs.dart:475`
正是**去重**，与其文档注「用于去重」一致。
⇒ 附带确认 id 必须是**跨平台稳定**的（`Object.hash` 按平台而变 ⇒ 同一张图算出不同 id
⇒ 「图不见了」），这是自定义指纹而非 `Object.hash` 的原因，注释已写明。

## 未核实项
1. `backgroundFingerprint` 的实现未读（它是否真的对 dataUrl 内容敏感、
   是否有截断导致碰撞 —— 若只取前若干位，碰撞就会让两张不同的图判等 ⇒ **这是本条唯一的残留风险**）
2. `app_shell.dart` 余段未读
3. `settings_controller.dart`(409) / `appearance_*.dart`(2260) 未读
4. `stage_bg_resend_dedupe_test.dart` / `asset_guard_stage_attach_resend_test.dart` 未读
5. 其余 Mod 面板未读

## 本批新增
**0 条**（净产出：F-0034-01 证伪 + 一处不构成发现的 `hashCode` 非对称备忘）
