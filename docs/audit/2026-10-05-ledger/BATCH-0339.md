# BATCH-0339 · ⭐⭐⭐ **它拒绝的是一个具体的 widget 名** —— 而**两个理由并列**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `streaming_indicator.dart`(121)

## 跑的命令（全部只读）
```
wc -l lib/ui/streaming_indicator.dart ; sed -n '1,20p' lib/ui/streaming_indicator.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **B0337 的镜像：同一原则、不同媒介**
> 「规格 §5.2 的一条约束：**不用 `CircularProgressIndicator` 做装饰** —— 那是一个
> **Ticker 驱动的持续动画，语义上表示「进度」**，用一个转圈表示「正在生成」
> **既费电又不告诉用户任何进展**。
> 这里做的是**三点呼吸**：**只在 `thinking` 相位渲染**，且**尊重 reduced-motion**
> （**静止三点**）。**「说话中」不显示它** —— **口型已经在动了，再加一个指示**…」  // :3-9
四个可核点：
1. ⭐⭐⭐ **拒绝的是「一个通用指示器被挪用到它不表示的语义上」**
   ⇒ ⇒ **① 语义不匹配**（`CircularProgressIndicator` 表示「进度」，而这里不是进度）
   **② 费电** ⇒ ⇒⇒ **两个理由并列**：**成本**与**信息量**（「既费电又不告诉用户任何进展」）
   ⇒ ⇒ **与 B0337 同一原则、不同媒介**（B0337 是**图标**要对上相位；这里是**动画**要对上语义）
2. ⭐⭐ **替代方案「三点呼吸」尊重 `reduced-motion`，降级是「静止三点」而不是「不显示」**
   ⇒ ⇒ **无障碍第三次出现**（B0327 角色标签 · B0337 语义标签 · **本批 reduced-motion**）
   ⇒ ⇒⇒ ⭐ **这是 P30 的第三个实例**：「通道不同、答案不同」——
   **运动偏好不同 ⇒ 降级方式也不同**；而**信息被保留**（仍是三点，只是不动）
3. ⭐⭐ **「只在 `thinking` 渲染」** ⇒ **直接用 B0333 那条链的第 ⑤ 位**
   ⇒ ⇒ **而「说话中」不显示的理由是「口型已经在动了」**
   ⇒ ⇒⇒ ⭐ **「已经有一个动的东西在表达这件事，就不要再加一个」**
   ⇒ ⇒ **与 B0305–B0311「原因/主信号优先」同族**：**避免用两个信号说同一件事**
4. ⭐ B0337 那六个标签里的**四个静止相位**在这里**一个都不显示** ⇒ **「不显示」也是一种正确的呈现**

⇒ ⇒ **0 findings**；⇒ ⭐ **本批与 B0337 构成一组**：
**同一原则（「承载物要对上它要表达的语义」）、两种媒介（图标 / 动画）、**两处都点名了被否的那个东西**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~455）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
