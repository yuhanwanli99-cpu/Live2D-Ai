# BATCH-0386 · ⭐⭐⭐ **四个变体各自带着前置条件** —— 而那条纪律在**三处**被重复

Phase 1 · 域覆盖 · **Rust**（Mod crate）—— `gate::GateOutcome` 四态（B0385 留，这条链的最后一环）

## 跑的命令（全部只读）
```
wc -l crates/live2d-ai-mod-voice-input/src/gate.rs      # ⇒ 245
sed -n '/pub enum/,/^}/p' crates/live2d-ai-mod-voice-input/src/gate.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**这条链闭合**（0 条新发现）
```rust
pub enum GateOutcome {
  /// 放行；`text` 是**剥掉唤醒短语**后的正文（**可能为空串，由调用方按空文本拒绝**）。
  Allow { text: String },
  /// 手动闸关闭（`manual_enabled = false`）。
  ManualOff,
  /// 总闸关闭（`wake_phrase` **显式**为空 / 纯空白；**键缺失不算**）。
  GateClosed,
  /// 文本里没有唤醒短语（**含空输入**）。
  WakeRequired,
}
```
四个可核点：
1. ⭐⭐⭐ **每个变体自带它的前置条件** ⇒⇒ **而 `GateClosed` 把「显式空 / 纯空白」与「**键缺失不算**」写进**类型文档**
   ⇒ ⇒⇒⭐ **同一句纪律出现三处**：**handler 头注**（B0385「键缺失不在此列」）·
   **枚举变体文档**（本批）· **客户端收敛逻辑**（B0383 把两者都收敛成缺省词）
2. ⭐⭐ **`Allow { text }` 的 `text` 可能是空串，「由调用方按空文本拒绝」**
   ⇒⇒ **一个状态不负责下一层的判断** ⇒⇒ **职责边界写进类型** ⇒⇒ **同 B0349 的可空性家族**
   ⇒⇒ 而这与 `voice_routes` 第 4 步「命中 → 剥掉 → **归一化 → 空文本判定** → 长度」**对得上**
3. ⭐⭐ **`WakeRequired` 含空输入** ⇒⇒ **「没说」与「说了但没说对」是同一个态** ⇒⇒ **对调用方没有区别**（都不放行）
4. ⭐⭐⭐ **枚举只有四态、判据是纯函数** ⇒⇒ 而 `voice_routes` 说 **handler 不得内联等价判定**
   ⇒ ⇒⇒ **那条禁令现在有了它守护对象的完整清单** ⇒⇒ **同 B0299/B0335「判据集中」家族**

⇒ ⇒⭐⭐⭐ **而这条链现在闭合了**
| 环 | 位置 | 本次核出处 |
|---|---|---|
| ① 两个入口 · 同一道预检 | 客户端 `voice_listen_controller` | B0382 |
| ② 取值源（Mod config）· 注入 | `main.dart` + 控制器注入点 | B0384 |
| ③ 服务端四闸 · 顺序钉死 | `web_api/voice_routes.rs` | B0385 |
| ④ 判据四态 · 禁内联 | `live2d-ai-mod-voice-input/src/gate.rs` | **本批** |

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 1 点**：
**一条纪律在三处被独立说出** ⇒⇒ **任何一处被删掉，另两处还在**
⇒ ⇒⇒ **这比「一处写清楚 + 别处引用它」更抗改** ⇒⇒ **因为引用会随重构断掉，重复不会**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `evaluate_mode` 的**函数本体**（四态的判定顺序与剥除实现 · 245 行里我读了 10 行）
2. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
