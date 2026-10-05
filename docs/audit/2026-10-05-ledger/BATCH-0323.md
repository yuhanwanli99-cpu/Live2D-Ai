# BATCH-0323 · ✅ **`destroy` 与 `dispose` 的差别已结清**，而 `destroy` 的**顺序**就是防「卡在中间态」

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `live2d_bridge.destroy` vs `dispose`

## 跑的命令（全部只读）
```
grep -rn "destroy()" -B 4 lib/live2d/live2d_bridge.dart
sed -n '360,392p' lib/live2d/live2d_bridge.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；✅ **B0322 挂的「语义差别」已核**
```dart
/// 协议 v1 `destroy`：**通知渲染面释放**，并关闭桥。                // :374
Future<void> destroy() async {
  if (_disposed) return;
  _readyTimer?.cancel();  _mouthTimer?.cancel();
  _phase = Live2DBridgePhase.destroyed;
  notifyListeners();                                                   // :380  ① 先对外宣布
  try {
    await _transport.send({'version': 1, 'type': 'destroy', 'payload': {}});   // :381-385  ② 再通知渲染面
  } catch (error) {
    _errorMessage = '**发送 destroy 失败**：$error';                    // :386-387  ③ 失败被记下，不抛
  }
  dispose();                                                          // :390  ④ 最后才本地拆除
}
```
四个可核点：
1. ⭐ **`destroy` 是协议消息、`dispose` 是本地拆除** ⇒ ⇒ **`destroy()` 的最后一行就是 `dispose()`**
   ⇒ ⇒ **两者不是并列选项，而是有先后**
2. ⭐⭐ **顺序即设计**：**先 `notifyListeners()` 宣布 destroyed → 再发 `destroy` 给渲染面 → 最后 `dispose()`**
   ⇒ ⇒ ⭐ **这样即使消息发失败，本地也已经进入 `destroyed`**
   ⇒ ⇒⇒ **不会因为跨进程失败而卡在「看起来还在」的状态** ⇒ **顺序本身是防中间态的**
3. ⭐⭐ **发送失败被记成 `_errorMessage`、不抛** ⇒ ⇒ **「失败是数据不是异常」家族的第六层**
   （B0288 写成功但读状态失败 · B0302 存档失败 vs 本次会话 · B0316 连不上 = 一次成功自检 ·
   B0317 吞掉但下次发送会暴露 · B0313 `!mounted` 时归还 transport）
   ⇒ ⇒ 而文案**点了名**：「**发送 destroy 失败**：$error」
4. ⭐ **两个 Timer 都在这里被取消** ⇒ ⇒ **这一段就是「拆桥」的全部动作**

⇒ ⇒ **0 findings**；⇒ ⭐ **而「失败是数据」的家族已到六层**，
⇒ ⇒ **它们共同的形状是：失败被放在「用户或下一次动作真的会撞上它」的地方**

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~595 / `memory_panel` ~545 /
   `persona_panel` ~520 / `message_bubble` ~460 / `chat_panel` ~465）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
