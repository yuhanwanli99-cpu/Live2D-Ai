# BATCH-0383 · ⭐ **「空」与「读失败」落到同一个值** —— 而一句对外文案把它变成了**可见的**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 唤醒词的**重载路径**（B0382 见到 `wakePhrase` 引出的）

## 跑的命令（全部只读）
```
grep -nE "wakePhrase|_wakePhrase" shell/flutter/lib/voice/voice_listen_controller.dart | head -6
sed -n '303,320p' voice_listen_controller.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **一个不对称，被文案中和了**
```dart
String _wakePhrase = kDefaultWakePhrase;                                    // :109
String get wakePhrase => _wakePhrase;                                      // :135
return '**在听：说「$_wakePhrase ……」**';                                   // :151  ⭐⭐
…
Future<void> _reloadWakePhrase() async {
  try {
    final phrase = (await _loadWakePhrase()).trim();
    _wakePhrase = phrase.isNotEmpty ? phrase : **kDefaultWakePhrase**;    // :311  「空」⇒ 缺省
  } catch (_) {
    _wakePhrase = **kDefaultWakePhrase**;                                 // :313  「读失败」⇒ 缺省
  }
}
… else if (**containsWakePhrase(transcript, _wakePhrase)**) { … }          // :347
```
三个可核点：
1. ⭐⭐ **「空」与「读失败」落到同一个值（缺省）** ⇒⇒ **两者不可区分**
   ⇒ ⇒ 而「用户改了唤醒词、那次写失败了 ⇒ 下次重载静默回到缺省」**是一条可能成立的路径**
   ⇒ ⇒ ⚠ **但 `_loadWakePhrase` 从哪读（localStorage？）我未核** ⇒⇒ **按 B0274 规则不推断**
2. ⭐⭐ **匹配用的是重载后的值**（`:347`）⇒⇒ **不是初始化时抄的一份** ⇒⇒ **改完下一轮就生效**
3. ⭐⭐⭐ **而 `:151` 的对外文案把当前唤醒词嵌进去了**：「**在听：说「$_wakePhrase ……」**」
   ⇒ ⇒ **用户能看到当前用的是哪个词** ⇒⇒ **P31 家族**（把内部状态说出来）
   ⇒ ⇒⇒⭐⭐ **而这把第 1 点从「问题」降为「可观察的取舍」** ——
   **一次静默回落会立刻被用户看见**（文案会显示缺省那个词）⇒⇒ **所以那个不对称不伤害用户**
   ⇒ ⇒⇒ **只伤害排障者** ⇒⇒ 而排障者能查存储/日志 ⇒⇒ **不记发现**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 3 点**：
**一个内部的回落之所以不是问题，是因为**另有一处把它说出来了** ——
⇒⇒ **这与 B0352 的 P31 是同一件事的第二面**：
**P31 说「要让用户知道」，而这里说「正因为说出来了，那个不对称才可以存在」。**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `_loadWakePhrase` 的**读源**（localStorage？`DisplayPrefs`？）与**写入方**（设置面板那一格未核）
2. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
