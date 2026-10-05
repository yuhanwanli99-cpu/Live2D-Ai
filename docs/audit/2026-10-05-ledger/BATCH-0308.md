# BATCH-0308 · ⭐ 判据是**纯函数 + 具名 + 两项**；两个 getter 都写了「**误判会让用户以为什么**」

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `mustReleaseTurnOnWsLoss` / `WsStatus` 的两个判据（B0307 留）

## 跑的命令（全部只读）
```
sed -n '/bool mustReleaseTurnOnWsLoss/,/^}/p' lib/chat/turn_liveness.dart
sed -n '/void stop()/,/^  }/p' lib/chat/chat_controller.dart ; grep -n "void stop" lib/chat/chat_controller.dart
sed -n '48,60p' lib/api/ws_status.dart ; grep -rn "isProblem|isUsable" lib/ --include=*.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **判据本身也守同一纪律**
```dart
// turn_liveness.dart:100-103
bool mustReleaseTurnOnWsLoss({required bool turnInFlight, required WsStatus status})
    => **turnInFlight && status.isProblem**;
```
```dart
// ws_status.dart:48-56
/// 是否「能用」：**只有连上才算**。
/// `connecting` **不算**可用 —— 把「正在连」当可用会让 UI 显示「已连接」
/// **而消息其实还发不出去**（**用户看到的是「发出去没回应」**）。
bool get isUsable => this == WsStatus.connected;
/// 是否处于「**用户应该看到问题**」的状态（供 `ConnectionBadge` 上 danger 色）。
bool get isProblem => this == WsStatus.disconnected || this == WsStatus.closed;
```
四个可核点：
1. ⭐ **判据是**纯函数 + 具名 + 只有两项** ⇒ ⇒ **无副作用、可单测、可复用** ⇒ 与 B0264 家族同形
2. ⭐⭐ **两个 getter 都写明了「误判会让用户以为什么」** ——
   `isUsable`：把「正在连」当可用 ⇒ **UI 显示「已连接」而消息发不出去** ⇒
   `isProblem`：判据是「**用户应该看到问题**」⇒ **不是「技术上坏没坏」**
   ⇒ ⇒ **他们不只定义状态，还定义了「错误解读的后果」**
3. ⭐⭐ **最难判的那个状态被单独点名**：`connecting` 被**显式排除**在「可用」之外
   ⇒ ⇒ **三个状态里没有灰区**
4. ⭐ `isProblem` 两处使用（`connection_badge.dart:46` / `turn_liveness.dart:103`）
   ⇒ ⇒ **判据共享、表现分离** ⇒ P2 形状

### ⚠ 而本批有一条**未兑现的「指路」**（如实记）
B0306 引了「用户自己按的停止已经在上面的 **`stop()`** 里先收口了」
⇒ 而 **`grep -n "void stop" chat_controller.dart` 零命中** ⇒ ⇒ **它可能是别的名字**
（`main.dart` 里有 `_stopWithCancellation`）
⇒ ⇒ **那句话指向何处，未核** ⇒ ⇒ **「注释指路」在这里可能指了个不存在/改名的符号**
⇒ ⇒ 按 **B0271 的规矩**：**不外推、不删除该引用**，只记录「未兑现」

## 未核实项
1. ⭐ **`stop()` 的真实定义**（改名了？在别的类？）—— B0306 引用了它，**而我在 `chat_controller.dart` 找不到**
2. `kTurnFailedWithoutDetailMessage` 的**完整后半句**被 `cut` 截断
3. `live2d_bridge.dart` 余面未读 · `main.dart:1240+` 余面未读 · `ApiClient.setActiveSession` 说明未读
4. `message_bubble.dart` 余约 460 · `live2d_stage` 余约 600 · `shell_admin` 余面 · `env_key_field` 余约 110 未读
5. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
6. `main.rs` 余约 930 行未读
7. 真实动作计划的 token 长度（F-0020-01 永久敞口）
8. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
9. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
10. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
11. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
12. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
13. **两侧措辞同步无机制**（B0260 敞口）
14. Mod crates 42 未读；`mod-system` 余 8 文件未读
15. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
16. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
17. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
