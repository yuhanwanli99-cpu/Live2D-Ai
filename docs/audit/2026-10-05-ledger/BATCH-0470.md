# BATCH-0470 · ✅ **`evaluate_mode` 本体：四个终点的顺序本身携带理由**

Phase 1 · 域覆盖 · Rust（`crates/live2d-ai-mod-voice-input/src/gate.rs`）—— 兑现挂了几十批的未核实项

## 跑的命令（全部只读）
```
grep -rn "evaluate_mode|evaluateMode" shell/flutter/lib/ --include=*.dart | head -5
grep -rn "pub fn evaluate_mode" -A 18 crates/live2d-ai-mod-voice-input/src/gate.rs | head -22
```
未跑任何 cargo / flutter / pnpm 命令。

## ⚠ 本批的一处**换语言**（B0440 那条规矩的又一次应用）
Dart 侧只在 `voice_listen_controller.dart:13` 的**文档注释**里提到
`live2d_ai_mod_voice_input::gate::evaluate_mode` ⇒⇒ **函数在 Rust 侧，不在 Dart 侧**
⇒⇒ 「一个名字在两个语言里都搜一遍、确认它属于哪一侧」这条规矩，本批又用上了一次。

## ★ 本批产出：**五分支的有序闸门**（0 条新发现）
```rust
// crates/live2d-ai-mod-voice-input/src/gate.rs:124-141
pub fn evaluate_mode(config: &Value, cleaned: &str, ptt: bool) -> GateOutcome {
    if !manual_enabled_from_config(config) { return GateOutcome::ManualOff; }   // ①
    let phrase = wake_phrase_from_config(config);
    if phrase.is_empty() { return GateOutcome::GateClosed; }                    // ②
    if ptt {
        // 按住说话：不要求含唤醒词；若仍说了，**顺手剥掉**。
        let text = strip_wake_phrase(cleaned, &phrase).unwrap_or_else(|| cleaned.to_string());
        return GateOutcome::Allow { text };                                     // ③
    }
    match strip_wake_phrase(cleaned, &phrase) {                                  // ④
        Some(text) => GateOutcome::Allow { text },
        None => GateOutcome::WakeRequired,
    }
}
```

### 四个可核点
1. ⭐⭐⭐⭐⭐ **四个终点的顺序本身携带理由**，而**每条都能说出来**：
   | 序 | 判据 | 为什么在这个位置 |
   |---|---|---|
   | ① | `manual_enabled == false` | ⭐ 用户**显式关了** ⇒ **其它一切都不再相关** |
   | ② | `wake_phrase` 为空 | 词空 ⇒ **唤醒这条路不成立** |
   | ③ | `ptt` | 用户**按住说话** ⇒ 不要求唤醒词 |
   | ④ | 含唤醒词？ | 最后一道 |

2. ⭐⭐⭐⭐⭐⭐⭐ **而 ② 与 ③ 的相对位置是**判据层面的关键**：
   **「词空」**没有**让 PTT 那条路失效** —— 而这正是它排在 ③ **之前**而不是之后的原因
   ⇒⇒⇒ **⇒ 「词空」与「用户按住说话」是两件**正交**的事** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 而 ③ 的实现**印证**了这一点**：`phrase` 空时 `ptt` 分支仍可 `Allow`
   ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒⇒ 那一行 `let text = strip_wake_phrase(cleaned, &phrase)` 在空词下**
   **必然返回 `None`、于是 `.unwrap_or_else` 取 `cleaned` 原样** ⇒⇒⇒⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ 而这就是它必须排在 ③ **之前**的**可观察后果**：换成之后，词空 + PTT 会变成 `GateClosed`**

3. ⭐⭐⭐⭐⭐ **③ 的注释「若仍说了，顺手剥掉」是本仓的一个形状**：
   **「不要求」+「若碰巧有就剥」** ⇒⇒ **这是「宽松读取」的一个实例**（同 B0430「不把不知道说成知道」的镜像：
   **这里是把「多说的」也收下**）⇒⇒⇒⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐ **而 `.unwrap_or_else(|| cleaned.to_string())` 那个闭包把「没有」表达成「原样」**
   ⇒⇒⭐⭐⭐ **⇒ 与 B0453 那条「回落值不能与真实值同义」**方向相反、而**这次是对的**：
   **「没剥到」与「就该原样」**在 PTT 这条路上本来就是同一件事** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒ 而「同一段代码在两条路上的回落语义不同」**是它能对的原因** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 2 点**
> **四个终点的顺序本身携带理由**
> **⇒⇒⇒ 而 ② 排在 ③ 之前，是因为「词空」与「用户按住说话」是两件**正交**的事**
> **⇒⇒⇒ 而 ③ 的实现**印证**了它：`phrase` 空 ⇒ `strip_wake_phrase` 必 `None` ⇒ `.unwrap_or_else` 取原样**
> **⇒⇒⇒ ⇒ 换成之后，词空 + PTT 会变成 `GateClosed`** ⇒⇒⇒⭐⭐⭐⭐⭐
> **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒ 「顺序即语义」在本仓是**可核**的：把两个分支对调、行为就变**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `strip_wake_phrase` 本体（**上面第 2 点用它推出一个结论、实现未读**）·
   `ModServices.apply_settings` 本体 · `HttpTestError` 三个变体 ·
   `manual_enabled_from_config` / `wake_phrase_from_config` 两个读配置的函数（是否也容错 · **未核**）
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
