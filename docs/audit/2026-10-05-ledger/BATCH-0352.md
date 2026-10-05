# BATCH-0352 · ⭐⭐⭐ **「文案要指界面上的开关」** —— 一个我没在本仓见过的**文案设计判据**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 诊断日志的**取数与 403 呈现**（B0321 核过 API 层，**消费层**未核）

## 跑的命令（全部只读）
```
grep -nE "logs\(\)|Timer|自动刷新|手动|refreshLog|_loadLogs" lib/settings/sections/dev_tools_section.dart   # ⇒ **无命中**
grep -rln "diagnostics_api|DiagnosticsApi" lib/ ; grep -rn "\.logs\(\)" lib/ --include=*.dart
sed -n '108,128p' lib/app/shell_admin.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **一条新的文案判据**
```dart
Future<void> _loadLogs() async {
  try { final lines = await _diagApi.logs(); if (!mounted) return; _logs = lines; _logsError = null; _refresh(); }
  on ApiException catch (e) {
    if (!mounted) return;
    _logs = const <LogLine>[];
    // 403 dev_mode_required 是**预期**结果，原文照显（**不伪装成空日志**）。
    _logsError = e.code == 'dev_mode_required'
        // **文案要指界面上的开关**：过去说「需服务端以 dev_mode 启动」，
        // 而**那时界面上根本打不开它**（**开关被自己所在的分区藏起来了**）。
```
四个可核点：
1. ⭐⭐⭐ **403 `dev_mode_required` 原文照显，「不伪装成空日志」**
   ⇒ ⇒ **与 B0236 的删除确认同族**（那里是「共用渲染函数」· 这里是「**错误原文不改写**」）
   ⇒ ⇒ 而分流是**按 `e.code` 比较**、**不是按 HTTP 状态** ⇒ ⇒ **B0136「按码分流」同族**
2. ⭐⭐⭐ **而文案的修正理由是一个已修的缺陷**：「过去说『需服务端以 dev_mode 启动』，
   而**那时界面上根本打不开它**（开关被自己所在的分区藏起来了）」
   ⇒ ⇒ **B0000 ⑭ 记过那个缺陷** ⇒ ⇒ **两处引用同一件事**
   ⇒ ⇒⇒ ⭐⭐⭐ **可提炼（P31）**：**错误提示应该指向用户能操作的那个东西** ——
   **不是指向启动参数、不是指向配置文件、而是指向界面上那个开关** ⇒
   **「指向哪里」和「说了什么」是两条独立的判据**
3. ⭐ **`_logs = []` 之后才设 `_logsError`** ⇒ **错误态与空态是两个字段**
   ⇒ ⇒ **不会「有错误但看起来像空」** ⇒ 与 B0325「降级不能压过错误」**同族**
4. ⭐ 两个 `if (!mounted) return;` ⇒ **异步统一守这一条**（**B0342 第四处**）

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的一个过程注记**：
**我先在 `dev_tools_section.dart` 里 grep `logs()`/`_loadLogs` ⇒ 零命中** ——
**差点据此判「日志刷新不在这里」** ⇒ ⇒ **换文件（`shell_admin.dart`）⇒ 命中**
⇒ ⇒ **又一次 B0316「没找到 ≠ 不存在，只等于换个人问」**（第四次）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~585 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~440 / `chat_panel` ~435）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
