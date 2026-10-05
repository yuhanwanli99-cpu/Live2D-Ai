# BATCH-0028 · rc.6 背景域 —— 核验团队已关闭项的残留

Phase 1 · 域覆盖 → 前端层 · 背景域（rc.6 已登记 10 条）

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/ui/shell_backdrop.dart` — 458（读 1-130 + 250-262）
2. `shell/flutter/lib/data/background_decode_cache.dart` — 105（读 1-80）
3. `shell/flutter/lib/settings/sections/appearance_background.dart` — 1551（定点：930 / 1460-1478）

## 跑过的命令（全部只读）
```
git ls-files 'shell/flutter/lib/main.dart' 'shell/flutter/lib/app/' 'shell/flutter/lib/ui/shell_backdrop.dart' 'shell/flutter/lib/settings/' | xargs wc -l | sort -rn
wc -l background_decode_cache.dart ; sed -n '1,130p' shell_backdrop.dart
sed -n '1,80p' background_decode_cache.dart
grep -rn "decodeDataUrlBytes|decodeDataUrlBytesCached|DataUrlBytesCache" shell/flutter/lib/
grep -n "ListView.builder|GridView.builder|itemCount|children:" appearance_background.dart
grep -n "_ImageTile|ImageTile(" appearance_background.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 D（重渲染 / 同步重计算）：**核心**——逐 delta 的解码路径
- 维度 A（真源唯一性）：两条路径共用一个缓存实例，容量口径与真实工作集不符 → F-0028-01
- 跨审计核验：团队 rc.6 的 4 条抽查（2 已修 / 2 未核 / 1 条已修但有残留）

## 未核实项
1. `main.dart`(1344) **未读**——团队 rc.6 的 F-0001-2 / F-0001-3 / F-0013-1 / F-0002-2 / F-0002-3
   五条**全部**落在这里（含两条 P1），是当前**单文件最高风险的未核区**
2. `shell_prefs.dart`(500) / `audio_bar.dart` 未读——F-0002-1（P1）落点
3. `appearance_background.dart`(1551) 只读了两处定点；它是任务书 §9 登记的**超长文件**之一
4. 本批**未读** `display_prefs.dart`(1157) 的持久化面（`backgroundSource` / `fitName` /
   `alignTable` / `tileSize` 的真源与钳位）

## 本批新增
P0 0 · P1 0 · **P2 1**（团队已关闭 P1 的残留）· P3 0
