# BATCH-0384 · ⭐⭐⭐ **B0383 那条「不对称」现在完全清楚了** —— 它是**一个值**，不是两个

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `_loadWakePhrase` 的**读源**（B0383 留）

## 跑的命令（全部只读）
```
grep -nB 3 "_loadWakePhrase()" shell/flutter/lib/voice/voice_listen_controller.dart | head -8
grep -rn "_loadWakePhrase\b" -A 6 shell/flutter/lib/ --include=*.dart | head -10
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0383 的疑问结清**（0 条新发现）
```dart
// lib/main.dart:982-988
Future<String> _loadWakePhrase() async {
  final List<ModInfo> mods = await _modsApi.list();
  for (final ModInfo m in mods) {
    if (m.id != '**voice-input**') continue;
    final Object? raw = m.config['**wake_phrase**'];
    if (raw is String && raw.trim().isNotEmpty) return raw.trim();
  }
  …
}
// lib/voice/voice_listen_controller.dart:78-79
_loadWakePhrase = loadWakePhrase;   _loadModEnabled = loadModEnabled;   // ⭐ 两个依赖一起注入
```
四个可核点：
1. ⭐⭐⭐ **唤醒词是 `voice-input` Mod 的 `config['wake_phrase']`** ⇒⇒ **不是 `DisplayPrefs`**
   ⇒ ⇒⇒ **B0383 那条「用户改了、写失败 ⇒ 静默回落」**改成了**
   **「Mod 配置读不到 ⇒ 回落缺省」** ⇒⇒⇒ **而「空」与「读失败」不是两个偶然，是同一个有意的形状**：
   **「`voice-input` 没启用」与「它的配置里没这个词」都意味着「没有自定义唤醒词」** ⇒⇒ **一个值足矣**
2. ⭐⭐ **`_loadWakePhrase` 是注入的**（`:78`）⇒⇒ **控制器不 import API** ⇒⇒ **可单测**
   ⇒⇒ **同 B0361 核的「面板不 import `app/browser_io.dart`」族**
3. ⭐⭐ **而 `_loadModEnabled` 也在同一处注入**（`:79`）⇒⇒ **B0382 那道「同一道预检」用的就是它**
   ⇒⇒⇒ **闸门与取值是同一家的** ⇒⇒ **改一处，两处都跟着**
4. ⭐⭐ **它读的是 `m.config`**（B0355 核过 `GET /api/v1/mods` 带 `config`、**secret 脱敏**）
   ⇒⇒ **`wake_phrase` 不是 secret** ⇒⇒ **能读到不奇怪** ⇒⇒ **它的可见性是契约决定的**
   ⇒⇒ **又一个「契约决定呈现」**（B0358 的 `apiVersion` 可见同族）

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 1 点**：
**上一个批我记的是「一个不对称」；这一批知道它是**语义上就应该是同一个值** ——
⇒⇒ **而 B0383 之所以看起来不对称，是因为我只看到了「空 vs 抛异常」，没看到「这两个来自不同的世界」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `voice-input` Mod 的 `settings_spec` 里 **`wake_phrase` 的 kind 与 min/max**（B0364 核过 spec 驱动的表单）
2. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
