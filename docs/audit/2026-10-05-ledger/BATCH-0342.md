# BATCH-0342 · ⭐ **「禁用」在输入层生效**、而「取消」与「点按」**不共用出口**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `_ListenButtonState` 的**手势处理本体**（B0341 留）

## 跑的命令（全部只读）
```
sed -n '420,455p' lib/ui/chat_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **两条「看起来会怎样就怎样」的反面**
```dart
bool get _enabled => widget.supported && widget.onToggle != null;          // :420
void _onDown(_) {
  if (!_enabled) return;                     // :422  ⭐ 门在**按下**处
  _holding = false;                          // :423  ⭐ 每次按下先复位
  _holdTimer = Timer(kVoiceTapThreshold, () { if (!mounted) return; … }); // :424-430
}
void _onUp(_)   { … if (_holding) { _holding = false; widget.onPressRelease?.call(); }  // :435-437
                   else { widget.onToggle?.call(); } }                               // :439  ⭐ 点按 = 切换
void _onCancel(){ … if (_holding) { _holding = false; widget.onPressRelease?.call(); } }  // :447-449  ⭐ 只 release
```
五个可核点：
1. ⭐⭐ **`_enabled` 在 `_onDown` 里就检查** ⇒ **禁用态连定时器都不启动**
   ⇒ ⇒ **「禁用」不是外观灰掉，而是**输入层不发生** ⇒ ⇒ **同 B0245「不假装能保存」**
2. ⭐⭐ **`_onDown` 里先 `_holding = false`** ⇒ **每次按下都复位**
   ⇒ ⇒ **上一次的状态不会漏进这一次** ⇒ ⇒ 避免「上次按到 release、这次以为还在 hold」
3. ⭐⭐ **`_onCancel` 只 `onPressRelease`、不 `onToggle`**
   ⇒ ⇒ **「拖走/取消」不会误触发「点按」** ⇒ ⇒ **两条路径不共用出口**
   ⇒ ⇒ 而 `_onUp` 里 hold 与 tap 是 **if/else 二选一** ⇒ **一次按下只产生一个动作**
4. ⭐ **定时器回调里有 `if (!mounted) return;`** ⇒ ⇒ **异步回调统一守这一条**
   ⇒ ⇒ **B0230 `shell_admin` · B0230 `_sendEnvKey` · B0290 `_attach` · **本批** = **第四处**
5. ⭐ **`kVoiceTapThreshold` 是具名常量**（`voice_listen_controller.dart`）
   ⇒ ⇒ **阈值不内联** ⇒ ⇒ 同 B0242 的「阈值写在决策点」

⇒ ⇒ **0 findings**；⇒ ⭐ **1 + 3 合起来是同一句话**：
**「看起来会怎样就怎样」的两个反面** —— 禁用**真的**不输入、取消**真的**不走另一出口

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~440）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
