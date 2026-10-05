# BATCH-0079 · 偏好落盘侧：`onPrefsChanged` 的 bool 契约与唯一写入点

Phase 1 · 域覆盖 · 前端 `settings/` + `app/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/app/browser_io.dart` — （定点 22-63：`kDisplayPrefsKey` / `loadDisplayPrefs` / `saveDisplayPrefs`）
2. `shell/flutter/lib/main.dart` — （定点 263 / 274 / 289：`onPrefsChanged` 的 **bool** 签名）
3. `shell/flutter/lib/settings/sections/appearance_section.dart` — （定点 54 / 88 / 194-424：**9 个调用点**）
4. `shell/flutter/lib/app/shell_settings.dart` — （定点 288：`onPrefsChanged: _updatePrefs` 接线）
5. `shell/flutter/lib/app/shell_prefs.dart` — （B0069 已核 :145-153）

## 跑过的命令（全部只读）
```
grep -n "onPrefsChanged|saveDisplayPrefs|writeDisplayPrefs" settings_controller.dart
grep -rn "bool saveDisplayPrefs|saveDisplayPrefs(" -A 16 app/browser_io.dart
grep -rn "kDisplayPrefsKey" shell/flutter/lib/
grep -rn "onPrefsChanged" shell/flutter/lib/
```
未跑任何 cargo / flutter / trunk / pnpm 命令。**未执行任何 Dart 代码**。

## 本批产出：**0 条新发现**（三项核验通过）
1. **bool 契约被兑现**（`browser_io.dart:53-63`）：
   ```dart
   bool saveDisplayPrefs(DisplayPrefs prefs) {
     try { web.window.localStorage.setItem(kDisplayPrefsKey, jsonEncode(prefs.toJson())); return true; }
     catch (_) { return false; }
   }
   ```
   `catch (_)` 兜住**全部**异常（无痕 / 配额满 / 存储被禁）⇒ **只��真成功才返 `true`**；
   消费侧 `shell_prefs.dart:149-153` 已在 B0069 核过（失败给一句话，文案**既说因也果**：
   「本次会话有效，刷新会丢」）
2. **`kDisplayPrefsKey` 全仓只有一个写入点**（`:56`）⇒ 落盘路径**单一**，无旁路
   （读点 `:38` 一处）—— 这是「落盘失败可被观测」的前提，**成立**
3. **两个同名 `onPrefsChanged` 是合法的两层设计，不是重复定义**：
   - `main.dart:289` `final bool Function(DisplayPrefs) onPrefsChanged;` ← **外层**，返回落盘结果
   - `appearance_section.dart:88` `final ValueChanged<DisplayPrefs> onPrefsChanged;` ← **内层**，void
   接线：`shell_settings.dart:288 onPrefsChanged: _updatePrefs`（与 `shell_prefs.dart:145`
   同一个 `_updatePrefs`，`shell_settings.dart`/`shell_prefs.dart` 同库 part）
   ⇒ section 的 9 个调用点（:194/:206/:260/:279/:290/:300/:308/:321/:424）**不需要**知道落盘结果，
   因为**失败横幅由外壳层展示**（B0069 已核）⇒ 分层正确
   ⇒ **同名不同签名是潜在误读点，但类型系统会拦**（`bool Function` 与 `ValueChanged` 不可互换）——
   **记此备忘，不记发现**

## 未核实项
1. `settings_controller.dart`(409) 仍未读 —— **它不在 `onPrefsChanged` 这条链上**
   （`grep onPrefsChanged settings_controller.dart` 零命中）⇒ 它的职责另有其物，**下批专门读**
2. `main.dart` 其余 ~1000 行未读
3. `shell_admin.dart` 余段未读
4. `background_hydration.dart:140-255` 未读
5. `appearance_background.dart`(1551) 未读（`appearance_section.dart` 本批定点过 9 处调用点）
6. 其余 Mod 面板（4 个 / 2284 行）未读

## 本批新增
**0 条**
