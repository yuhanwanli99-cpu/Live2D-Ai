# BATCH-0326 · ⭐ 两种布局两个入口，而**判据写在字段的文档上** —— 免得「grep 不到」被当成「漏了」

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 会话列表入口（`selectSession` 的 UI 侧）

## 跑的命令（全部只读）
```
grep -nE "onOpenSessions|会话|Session" lib/ui/chat_panel.dart
grep -rn "onOpenSessions" lib/ --include=*.dart | grep -v chat_panel
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **「空态被定义」又一例**
```dart
/// 会话列表入口（**只有 compact 传**；**非 compact 的入口在 AppBar**）。   // chat_panel:93
final VoidCallback? onOpenSessions;                                        // :94
…
onOpenSessions: compact ? () => unawaited(openSessions()) : null,          // app_shell:945
```
三个可核点：
1. ⭐⭐ **两种布局、两个入口，而判据写在字段的文档上**
   ⇒ ⇒ **不是「忘了传」，是「另一处在 AppBar」**
   ⇒ ⇒ ⇒ **于是 `grep onOpenSessions` 的人不会把「非 compact 路径没命中」误读成「那里缺了」**
   ⇒ ⇒ **P22**（规则要写明「它不适用于谁、为什么」）的又一例
2. ⭐⭐ **`null` 是「这里没有入口」的合法值**（`compact ? … : null`）
   ⇒ ⇒ 与 B0245 的 `onDismiss` 可空族同形（「`null` 时**不显示**关闭按钮」）
   ⇒ ⇒ **又一例「空态被定义、不是被忽略」**
3. ⭐ **`unawaited(openSessions())`** ⇒ 入口是 **fire-and-forget**
   ⇒ ⇒ 与 B0230 的 `_busy` 闩**不冲突**（那是**变更类**操作；开面板是**导航**）
   ⇒ ⇒ **连「该加闩的加、不该加的不加」都成立**

⇒ ⇒ ⭐ 而这与 B0228 核过的那条形成一组：**同族面板都声明了「我管什么、另一处在哪」**
（`persona_panel` 头部 · `error_banner` 的「外形只有 `InlineNotice` 一处」· 本条）

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~595 / `memory_panel` ~540 /
   `persona_panel` ~515 / `message_bubble` ~460 / `chat_panel` ~460）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
