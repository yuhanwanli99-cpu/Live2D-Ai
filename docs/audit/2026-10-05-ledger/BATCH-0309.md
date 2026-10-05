# BATCH-0309 · ⭐ **指路兑现了**，而 `stop()` 的注释是 B0306 那条判据的**前向应用**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_controller.stop()`（B0308 记的「未兑现的指路」）

## 跑的命令（全部只读）
```
grep -rn "stop" lib/chat/chat_controller.dart | grep -E "void|Future|onStop"
sed -n '206,232p' lib/chat/chat_controller.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **B0308 那条「未兑现」兑现了**
```dart
/// **先本地收口，再发请求**（**顺序有讲究**）：
/// ① 停止路径在后端走 `RootEffect::TurnAborted`，而它**不投影任何 WS 帧**
///    （`supervisor/handlers.rs`）—— **也就是说这一轮的收口只能由这里做**，
///    服务端不会补一个 `turn_state`；
/// ② 先收口还能避免下一行那个误判：服务端收到 stop 后会推进 epoch 并广播 `new_epoch`，
///    而 `new_epoch` 在本文件的语义是「上一轮被**外部**中断」（见 `_onWsEvent`）——
///    **若不先收口，用户自己按的停止会被当成外来抢占**。                    // :207-213
Future<void> stop() async {                                              // :214
  _finishTurn(stopped: true);                                            // :215  ⭐ 先本地
  try { await api.stopChat(); }                                          // :216    再发请求
  …
}
```
### ① 顺序要求被**两个独立机制**支撑
- ⭐ **① 后端的 abort 路径不投影任何 WS 帧** ⇒ ⇒ **这一轮的收口只能由客户端做** ⇒ ⇒
  「先收口」**不是偏好，是唯一可行** ⇒ ⇒ 且它**点名了服务端那个文件**（又一次「指路」）
- ⭐⭐⭐ **② 而顺序还避免了一个「未来的误读」**：服务端收到 stop 会推进 epoch 并广播 `new_epoch`，
  而**本文件里 `new_epoch` 的语义是「上一轮被外部中断」** ⇒ ⇒
  **若不先收口，用户自己按的停止会被读成「外来抢占」**
  ⇒ ⇒⇒ ⭐⭐ **这是 B0306 那条判据的「前向应用」**：
  B0306 核的是「`_releaseTurn` 怎么把**外部中断**认出来」；
  **本条说的是「怎么不去制造那个『外部』信号」** ⇒ ⇒ **同一判据，两个方向都用上了**

### ② 失败不被吞（`:217-222`）
两个分支都写 `_error` 并 `notifyListeners()`（`ApiException` 一支写 `error.toString()`、
另一支写「停止失败：$error」）⇒ ⇒ 与 B0306/B0308「失败被归因到正确的那半」**同向**

⇒ ⇒ **0 findings**；⇒ **这条链（B0305–B0309）现在首尾闭合**：
**`stop()`（本批）→ `_finishTurn(stopped:true)` → abort 不投影帧 ⇒ 收口只能客户端做
→ 且先收口可避免制造「外部」信号 → `new_epoch`/`_releaseTurn`（B0306）判外部 → 文案要说准（B0307）**
⇒ ⇒ **五批五层，从「函数怎么分」一路到「文案写什么」**

## 未核实项
1. `_finishTurn` 的**本体**（`:429`）仍未读 —— 而它是这条链的**枢纽**（`stopped: true` 从哪进）
2. `kTurnFailedWithoutDetailMessage` 的完整后半句被 `cut` 截断
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
