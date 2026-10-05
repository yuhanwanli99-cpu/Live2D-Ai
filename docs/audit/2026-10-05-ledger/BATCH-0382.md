# BATCH-0382 · ⭐⭐ **两个入口共用同一道 Mod 闸门** —— 而注释**先写明「同一道预检」再写调用**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 语音按住的**控制器侧**（B0341/B0342 核过 UI 侧）

## 跑的命令（全部只读）
```
grep -rn "onPressStart:|onPressRelease:|onToggleListen:" lib/ --include=*.dart | grep -v chat_panel
grep -nE "pressStart|pressRelease|bool get supported" -A 12 lib/voice/voice_listen_controller.dart | sed -n '1,30p'
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **「两道门不因为入口不同而不同」的语音侧实例**
```dart
bool get **supported** => _recognizer != null;                          // :123
bool get **suspendedForPlayback** => _suspended;   // 「因播报而暂停听（P0-4）」   // :132
Future<void> pressStart() async {
  if (_disposed || **_ptt**) return;                                     // :207  重入守卫
  final recognizer = _recognizer;
  if (recognizer == null) { _error = '**这个构建没有语音识别（需要桌面版 Chrome / Edge）**'; … }  // :210
  // **同一道预检：PTT 也走 voice-input 端点（手动闸 / 总闸不变）。**         // :215  ⭐⭐⭐
  final String? blocked = await _voiceModBlockReason();                  // :216
  if (_disposed) return;                                                 // :217  await 后查 mounted
}
Future<void> pressRelease() async { if (!**_ptt**) return; … }          // :240-241
```
五个可核点：
1. ⭐⭐⭐ **「同一道预检」被先写明、再调用** ⇒ ⇒ **`pressStart` 与 `toggle` 走**同一个** Mod 闸门**
   ⇒ ⇒⇒ **这与 B0361 核的 persona「**绝不偷偷改成全局导入**」同族**：
   ⇒⇒⇒ **两道门不因为入口不同而不同** ⇒⇒⇒ **而入口有两个（点按 / 按住）**
2. ⭐⭐ **「支不支持」是一个真值**（`supported => _recognizer != null`）⇒⇒
   **B0342 核的 `_enabled` 用的就是这个** ⇒⇒ **门面的禁用与服务端能力是同一个源**
3. ⭐⭐ **不支持时的文案点名了需要什么**：「**这个构建没有语音识别（需要桌面版 Chrome / Edge）**」
   ⇒ ⇒ **同 B0376 那条 P31**（错误提示指向可操作的东西）—— **这里更进一步：点名了具体的浏览器**
4. ⭐ **`pressStart` / `pressRelease` 各有各的守卫**（`_ptt` 正反两面）
   ⇒ ⇒ **B0342 核过 UI 侧成对；现在控制器侧也成对** ⇒⇒ **两层都成对**
5. ⭐ **`suspendedForPlayback`（P0-4）** ⇒⇒ **又一个「被谁暂停了」的状态可见**
   ⇒ ⇒ **同 B0313「谁造成了这一轮」的思路**（**原因要能被看见**）

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 1 点**：
**入口有两个、闸门只有一个** ⇒⇒ **而这件事被写在**两个入口之一**的注释里**
⇒⇒ **读者在 `pressStart` 这条路上就知道 `toggle` 那条也是同一道门**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
