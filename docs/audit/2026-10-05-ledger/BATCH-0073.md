# BATCH-0073 · F-0034-01 修法①核实 + 「正确判据就在旁边」

Phase 1 · 域覆盖 · 前端

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/design/background_item.dart` — （定点 377-385 `BackgroundImage.==`/`hashCode`；
   :430-434 `BackgroundPattern.==`/`hashCode`）—— **本批唯一目标：核 B0072 建议①引用的前提**

## 跑过的命令（全部只读）
```
grep -n "operator ==" -A 12 shell/flutter/lib/design/background_item.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**F-0034-01 修法①核实通过**，并把该发现升级为**完全指定的接缝**
- `BackgroundImage.==`（:377-382）= id + **opacity + fit + align**，`hashCode`（:385）覆盖
  **同样五个字段** ⇒ **教科书正确的值类型**，`design/` 层**无错**
- `BackgroundPattern.==`（:430-431）只比 id、`hashCode` 对称 ⇒ 同样正确
- ⇒ 缺陷因此变成：**正确的判据已经写在同一个文件里**，
  而 `display_prefs.dart:970` **调了更弱的 `sameAs`** ⇒ 修法 = **一行替换**
- 「潜在」定级不变（`appearance_background.dart:1264` 样式控件无回调时不渲染），
  但**两个各自很小的改动叠加**（1 行判据 + 一次常规控件接线）就得到在线的静默丢弃 ⇒ 模式 C 典型形状
- **B0072 的教训当场兑现**：我在建议里引用的 `==` 本批**先读后用**，没有再犯 B0070 的错

## 未核实项
1. `BackgroundImage.==`（含逐图样式）**是否有测试** —— 未核（本批发现的关键尾巴：
   若有测试却只测了值类型、没测 `DisplayPrefs.==` 的聚合，那是「测试选错了层」的又一例）
2. `background_hydration.dart:140-255` 未读
3. `settings_controller.dart`(409) / `app_shell.dart` 余段未读
4. `appearance_background.dart`(1551) 未读（只核过 :1264）
5. 其余 Mod 面板（4 个，共 2284 行）未读

## 本批新增
**0 条新发现** ｜ F-0034-01 修法①**核实通过** + 机制完全指定
