# BATCH-0029 · `main.dart` 水合路径 —— 核验团队 rc.6 的两条 P1

Phase 1 · 域覆盖 → 前端层 · 背景域

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/main.dart` — 1344（**读 141-260**（`main()`/`_Live2DShellAppState`/`_hydrateBackgrounds`/
   `_hydrateSafely`/`_update`）+ :122-140 定位 + 定点 536-538）
2. `shell/flutter/lib/data/background_hydration.dart` — 255（读 53-169：`BackgroundHydration` /
   `applyBackgroundHydration` / `hydrateBackgrounds` 头段）
3. `shell/flutter/lib/ui/audio_bar.dart` — （读 63-110：滑杆提交路径）

## 跑过的命令（全部只读）
```
grep -n "_hydrateBackgrounds|_store|void main()|ShellSlideshow|jumpTo|class _" main.dart
sed -n '141,260p' main.dart
grep -n "applyBackgroundHydration" -A 55 data/background_hydration.dart
sed -n '120,169p' data/background_hydration.dart
grep -n "onVolumeChanged|onMutedChanged" -A 6 ui/audio_bar.dart ; sed -n '100,120p' ui/audio_bar.dart
grep -n "bool saveDisplayPrefs" -A 25 settings/display_prefs.dart     # → 无命中（签名不同，未追）
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 A（状态真源唯一性）：`_store` 的 final 单实例 = 背景字节库的唯一真源 ✔
- 维度 C（竞态）：水合窗口 vs 用户改动的竞态 —— **已修**，三处改动闭合
- 维度 D：`audio_bar` 的拖动/提交分离 = 消除「滑杆每像素一次」✔
- 跨审计核验：5 条（4 已修 / 1 部分核）

## 未核实项
1. `main.dart` 只读了 141-260（119 行 / 1344）。**其余 1225 行未读**，包括：
   `_ShellRootState`（:295 起，含 `_slideshow` / `jumpTo` —— 团队 `F-0001-2`「轮播双索引漂移」的落点）、
   聊天/WS 接线路径（:687「`text_delta` 毫秒级，逐条写会把主线程拖垮」的节流是否在）、
   `settingsRevision` 的实际传值（**验证他们 F-0005-2 修复是否真的不跟 delta 走**）
2. `F-0002-1` 的「全量落盘 / 全量重发 stage-bg」半边未核（需 `shell_prefs.dart` + `live2d_bridge.dart`）
3. `F-0001-2`（轮播双索引）未核
4. `saveDisplayPrefs` 的实现未读到（签名不叫 `bool saveDisplayPrefs`）

## 本批新增
**0 条**（净产出：4 条已登记项确认已修 + 1 条部分核 + 一条可借鉴的判据）
