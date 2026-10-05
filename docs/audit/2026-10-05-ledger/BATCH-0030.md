# BATCH-0030 · 轮播索引 + 偏好下发漏斗 —— 核验 F-0001-2 / F-0002-1

Phase 1 · 域覆盖 → 前端层 · 背景域（收尾批）

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/main.dart` — 1344（读 460-574）
2. `shell/flutter/lib/app/shell_prefs.dart` — 500（**读 70-154**）
3. `shell/flutter/lib/live2d/action_scales_sync.dart` — （读 1-80 定点 + 133-172）

## 跑过的命令（全部只读）
```
grep -n "_applyPrefs" shell/flutter/lib/ | grep -v main.dart
grep -rn "_applyPrefs" shell/flutter/lib/live2d/ shell/flutter/lib/app/ | head
grep -n "_slideshow|jumpTo|_backgroundIndex" main.dart
grep -n "void syncNow|bool force|_lastSent|identical|void schedule" -A 22 action_scales_sync.dart
grep -rn "ActionScalesSyncer(" -A 6 shell/flutter/lib/
sed -n '460,574p' main.dart ; sed -n '70,154p' shell_prefs.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 A：`_backgroundIndex` 是「唯一运行时索引真源」（main.dart:475-477 注明），双向同步 ✔
- 维度 D：三条下发通道的同值去重 —— 两条有、一条被 `force` 绕过 → F-0030-01
- 跨审计核验：F-0001-2 已修（两形态）；F-0002-1 主体已修、**有残留**

## 未核实项
1. `main.dart` 仍有约 1090 行未读（:575-1344）：聊天/WS 接线路径、
   `settingsRevision` 的实际传值（**验证他们 F-0005-2 的修复是否真不跟 delta 走**）、
   `LiveRegionThrottle` 的节流参数
2. 是否有测试钉住「音量拖动不逐帧下发」——**未核实**，需翻 `shell/flutter/test/`
3. `_updatePrefs` 之后的落盘路径（`saveDisplayPrefs` 的真实签名与写入方式）未读

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0
