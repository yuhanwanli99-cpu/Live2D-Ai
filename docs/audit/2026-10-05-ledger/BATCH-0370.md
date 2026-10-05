# BATCH-0370 · ⭐⭐ **一个显式的「乐观记账」** —— 而它与 B0271 的判据**方向相反、却同样有理由**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `live2d_bridge.sendStageBg` 的**去重实现**（B0323 核了字段注释，**本体**未核）

## 跑的命令（全部只读）
```
grep -nE "sendStageBg|_lastSentStageBg" shell/flutter/lib/live2d/live2d_bridge.dart | head -6
sed -n '279,300p' live2d_bridge.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **一个与既有判据反向、但同样有理由的决定**
```dart
Future<void> sendStageBg(String? dataUrl) {
  final String value = dataUrl ?? '';
  if (_phase == Live2DBridgePhase.ready) {
    if (_lastSentStageBg == value) return Future<void>.value();        // :282  同值不重发
    // **记在真正交给传输面之前**：下面这条路径是同步 `_send`（await 后失败
    // 走 `_fail` 把阶段打到 error，**那时整条下行都已经不可信**）。        // :283-284
    _lastSentStageBg = value;                                        // :285  ⭐ 乐观记账
  }
  return _enqueueOrSend('stage-bg', <String, Object?>{'dataUrl': value});
}
```
四个可核点：
1. ⭐⭐ **同值去重只在 `ready` 阶段生效** ⇒ **不 ready 时照发（入队）**
   ⇒ ⇒ **B0369 的「两层」在此合拢**：**ready 之后才算送达**（去重）· **ready 之前会被重放**（B0290）
   ⇒ ⇒⇒ **两处规则各管一段，互不冲突**
2. ⭐⭐⭐ **而「记在哪」是被显式选择、且给了理由的**：「**记在真正交给传输面之前**……
   走 `_fail` 把阶段打到 error，**那时整条下行都已经不可信**」
   ⇒ ⇒ **一个「乐观记账」**（先记、后发）⇒ ⇒ **失败模式被点名**：**下行已死时，记账准不准是次要的**
   ⇒ ⇒⇒ ⭐⭐ **而它与 B0271 定的判据**方向相反**（B0271「不确定时选保守」）
   ⇒ ⇒⇒ **这里选的是「下行已死时记账的精度无所谓」** ⇒⇒ **两处都给了理由** ⇒⇒
   ⇒⇒⇒ **判据不能脱离场景套用** —— **「保守」是默认值，不是公理**
3. ⭐ **`?? ''` 把 `null` 归一为空串**，而 `_lastSentStageBg` 初值是 `null`（`:141`）
   ⇒ ⇒ **「设成空」与「从没设过」在第一次比较时被区分**（`null != ''`）⇒ ⇒ ⭐ **一个刻意的 `?? ''`**
4. ⭐ `:296-298` preset 契约：「**不认识的 id 静默忽略**，模型能力不足不会报错」
   ⇒ ⇒ **又一次「静默」被写明** —— 而这是 **B0293「别假装有话说」的对面**：
   **这里没有话可说，所以静默是对的**（且**写明了为什么是静默**）

⇒ ⇒ **0 findings**；⇒ ⭐⭐ **而本批最值钱的是第 2 点**：
> **判据不能脱离场景套用。** B0271 立的是「不确定时选保守」——
> **那是一条默认值，不是一条公理**；而这里在**知道失败模式**的前提下选了相反的一边，
> **并且把那个失败模式写在了旁边**。

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `_fail` 把阶段打到 error 后，**那条「不可信的下行」会不会被 `retry()` 重建掩盖**
   （B0357 核过 `retry()` 清三样：旧桥 / 旧相位 / 旧错误；**`_lastSentStageBg` 不在其中** ⇒ **待核**）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1790 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
