# BATCH-0072 · background_hydration.dart —— ⚠⚠ **撤回 B0070 的证伪；F-0034-01 恢复并升级**

Phase 1 · 域覆盖 · 前端

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/data/background_hydration.dart` — 255（定点 90-139：
   `BackgroundHydration.prefs` 的禁用警告 / `applyBackgroundHydration` 全函数）

## 跑过的命令（全部只读）
```
wc -l shell/flutter/lib/data/background_hydration.dart
grep -n "styleOverrides|逐图|copyWith(id|class |Map<String" background_hydration.dart
sed -n '90,139p' background_hydration.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**撤回 B0070 的证伪**，**F-0034-01 恢复并升级为 P3（机制更准）**
B0070 我用「逐图样式在**独立字段**」作前提证伪了 F-0034-01，并写了三行「不成立」表。
**那个前提是我推的，不是读到的，而且它是承重墙。** 实测：
`background_hydration.dart:134-136` 写明「**必须走 copyWith**：直接 `new BackgroundImage(id: …)`
会把用户给这张图设的 **opacity / fit / align** 一起清空」
⇒ **样式住在 `BackgroundImage` 内部**，不是独立字段。

⇒ 重写后的机制（每步已核）：`sameAs` 只比 id（对**去重**正确）→ 但 `DisplayPrefs.==`
（`display_prefs.dart:968-971`）**复用它做变更检测** → 而对象里**含逐项可变子状态**
⇒ **纯样式变更被 `==` 判成「没变」** → `shell_prefs.dart:146` **整条丢弃**。
⇒ 准确说法：**不是「id-only 判等错了」，而是「为去重设计的身份比较被复用为变更检测判据」**（接缝，模式 C）。
⇒ 仍属**潜在**：`appearance_background.dart:1264` 无回调时样式按钮**不渲染**（B0034 已核）。
修法两行：`==` 的 backgrounds 比较改用逐项 `==`（`BackgroundImage` 的 `==`/`hashCode` **已对称**，见 :385），
或给 `sameAs` 改名让两种用途在名字上分家。

## ⭐ 教训（本批最重要产出）
**做证伪时必须逐句核对自己引用的每个前提**，不能因为「我上轮读过相关文件」就认为前提成立。
B0070 的错误不是结论错、而是**用了未读的前提**；若无人复核，它会变成一条**假证伪**留在账本里。

## 未核实项
1. `BackgroundImage.==` 的实现未读（B0072 建议①依赖它「已对称」）—— 建议里已注明依据是
   `:385` 的 `hashCode`（含 id/opacity/fit/align），**`==` 本体未看**，属**未核实**
2. `appearance_background.dart`(1551) 未读（只核过 :1264）
3. `settings_controller.dart`(409) / `app_shell.dart` 余段未读
4. `background_hydration.dart:140-255` 未读（`hydrateBackgrounds` / `forgetBackground`）
5. 其余 Mod 面板未读

## 本批新增
**0 条新发现** ｜ **1 次「证伪的证伪」** ｜ F-0034-01 恢复（P3，机制重写）
