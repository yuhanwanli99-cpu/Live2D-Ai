# BATCH-0410 · ⭐⭐⭐⭐ **我 B0350 记的判据是两项，真实判据是三项** —— 漏掉的那项**是最省的那个**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `LiveRegionThrottle.feed` **本体**（B0350 / B0409 都引用过、**未核**）

## 跑的命令（全部只读）
```
sed -n '/void feed(/,/^  }/p' lib/state/live_region.dart     # ⇒ 零命中（是 `bool feed(`）
grep -nE "feed|finish|minInterval|_lastAnnounce|SENTENCE" lib/state/live_region.dart | head -10
sed -n '5,20p' lib/state/live_region.dart ; sed -n '56,74p' lib/state/live_region.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**我的笔记少了一项**（0 条新发现）
> 「`text_delta` 是**毫秒级**到达的……如果每来一个 delta 就更新一次 `liveRegion`，读屏会被**淹没** ——
> 用户听到的是**一串互相打断的音节，比不说话更糟**。」                              // :5-8
> 「规格 §9.1 给的判据是『**≥1.5 s，或按整句到达播报**』。本项目选了**两条**：
> **整句到达**（`。！？…`）→ **立刻**播报（**这是有意义的语义边界**）；
> 否则按 [minInterval] 限速，且**只在正文真的变长时**才更新。」                     // :9-13
```dart
bool feed(String fullText) {
  if (fullText == _text) **return false;**                                 // ① 没变
  final sentenceDone = fullText.isNotEmpty && kSentenceEnders.contains(fullText[fullText.length - 1]);
  final intervalElapsed = _lastAnnounce == null || now.difference(_lastAnnounce!) >= minInterval;
  if (!sentenceDone && !intervalElapsed) return false;                     // ② 有句末标点 ③ 距上次 ≥ 1.5 s
  _text = fullText; _lastAnnounce = now; notifyListeners(); return true;
}
```
四个可核点：
1. ⭐⭐⭐ **「淹没」的代价被写成用户可感知的形态**（「一串**互相打断的音节，比不说话更糟**」）
   ⇒⇒ **同 B0389 的写法：不是「有点影响」，是「更糟」**
2. ⭐⭐⭐⭐⭐ **而限速那一级**还有第二个条件**：「**只在正文真的变长时**才更新」**（实现 = `fullText == _text → false`）
   ⇒⇒⭐⭐ **⇒ 这正是 B0350 那条「两级放行」没说清的一半** ⇒⇒⇒⭐⭐ **⇒ 我 B0350 记的是
   「句末标点 **或** 1.5 s」，而**真实判据是三项**：**① 文本没变（直接丢）② 有句末标点（立刻）
   ③ 距上次 ≥ 1.5 s（兜底）** ⇒⇒⇒ **⇒ 而漏掉的那一项恰恰是**最省**的那个**
3. ⭐⭐⭐ **「整句到达」被称作「有意义的语义边界」** ⇒⇒ **「一句一单元」红线在读屏侧的落点**（B0313 核过 TTS 侧、B0350 核过读屏侧）
   ⇒⇒ **而句末标点声明为「与 `live2d-ai-runtime` 的分句标点对齐」** ⇒⇒⇒ **跨语言对齐又一次**
4. ⭐⭐⭐ **`finish` 的理由又是两个**：不清空 → **下一轮先念上一轮残留**；
   不补播 → **最后一句（往往没句末标点，比如「好呀」）读屏用户永远听不到**
   ⇒⇒ **同 B0350 核的 `settleTurn`「最后没句号的那句」** ⇒⇒ **同一形态在音频与读屏两侧各有一个出口**

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐ **而本批最重要的产出是「我自己的笔记少了一项」**
⇒ ⇒ **⇒ 「两级放行」是我从**注释**里读的，而第三个条件写在注释的另一个位置**（`:11` vs `:57`）
⇒ ⇒⇒ **⇒ 我核了那个注释、没核那个函数** ⇒⇒⇒⭐ **⇒ 又是「读过两侧、没并排」**
—— **B0369 那条形态的第七次复发（这次的「两侧」是**同一段注释的两处**）**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `kSentenceEnders` 的**内容**（`:19-20` 说「与 runtime 的分句标点对齐」——
   ⚠ **B0350 核过 runtime 侧是 `。！？…`、而这里是否恰好同集合我没逐字比**）
2. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
