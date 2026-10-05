# BATCH-0031 · 对抗性核验 F-0005-2（放大器 P1）

Phase 1 · 域覆盖 → 前端层

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/main.dart` — 1344（读 700-750 / 1150-1182）
2. `shell/flutter/lib/app/app_shell.dart` — 998（读 580-641）

## 跑过的命令（全部只读）
```
grep -n "settingsRevision|_settingsRevision|_pane" main.dart app/*.dart
sed -n '1150,1182p' main.dart ; sed -n '700,750p' main.dart
grep -rn "_refresh()" main.dart app/*.dart | head -20
grep -n "setState(" main.dart | head -20
sed -n '580,641p' app/app_shell.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 D（重渲染）：**本批主题** —— 放大器的修复是否两头都对
- 维度 A：两个代际计数器分别覆盖「宿主不可见状态」与「设置数据」，后者**绕开外壳 build**
- 跨审计核验：F-0005-2（P1，放大器）**已修且修法质量高于本仓平均**

## 未核实项
1. `main.dart` 仍有约 1000 行未读（:295-460 的 `_ShellRootState` 前段、:750-1150 的 WS/设置接线、
   :1182-1344 的收尾）——含 `LiveRegionThrottle` 的节流参数是否真的在用
2. `test/app_shell_background_test.dart`（他们钉 InheritedWidget 约束的回归）**未读**——
   「约定与回归见」这句话我只验证了约定存在，**没验证那条回归真的能失败**
3. `_settingsTick` 的生产者（在 `SettingsController` 侧）未读，故「设置数据变化必定 +tick」
   这一环是**从消费侧反推**的，未从生产侧确认

## 本批新增
**0 条**（净产出：一条 P1 修复的对抗性核验通过 + 一处不记入发现的观察 + 一处新的未核实项）
