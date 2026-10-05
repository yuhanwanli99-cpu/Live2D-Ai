# BATCH-0412 · ⭐⭐⭐⭐ **反证条款被用上了，结果是「反证不成立」** —— 而 `；` 那一半**更锋利**

Phase 4 · **证伪**（去**代码**里读判据，不读文档 —— 那正是 B0410/B0411 刚犯的错）

## 跑的命令（全部只读）
```
grep -rn "。|?\|!|…" crates/live2d-ai-runtime/src/dialogue/sentence.rs | head -8
ls crates/live2d-ai-runtime/src/dialogue/ ; grep -rn "is_sentence_end|SENTENCE_END|fn is_end" …/dialogue/*.rs
grep -nE "b'。'|b'\.'" …/sentence.rs          # ⇒ 零命中（不是字节字面量）
sed -n '20,32p' crates/live2d-ai-runtime/src/dialogue/sentence.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0411-01 不撤回、反而加强**（0 条新发现）
```rust
/// 句终止符集合：中英文句读 + 换行（**协议固定，不含分号/冒号等弱标点**）      // :22
fn is_terminator(c: char) -> bool {
  matches!(c, '.' | '?' | '!' | '。' | '！' | '？' | '…' | '\n')              // :24
}
/// **弱标点**（自然停顿点）：顿号/逗号/**分号**/冒号 + 半角对应。            // :26-27
/// **只在安全阀触发时**用作切点
fn is_soft_break(c: char) -> bool {
  matches!(c, '，' | ',' | '、' | '；' | ';' | '：' | ':')                     // :30-31
}
```
四个可核点：
1. ⭐⭐⭐ **反证不成立** —— `dialogue/mod.rs:11-12` **不是过时注释**；
   `sentence.rs:23-24` 的**代码**逐字与它一致（**八个字符**）
   ⇒⇒ **我在 F-0411-01 差异表的 Rust 侧那半**完全正确**
2. ⭐⭐⭐⭐⭐ **而 `；` 那一半比我说得更锋利**：
   ⇒⇒ **`；` 被显式归入 `is_soft_break`（弱标点）** ⇒⇒
   ⇒⇒⇒ **且 `is_terminator` 的文档逐字写着「**不含分号/冒号等弱标点**」**
   ⇒⇒⇒⭐ **⇒ 于是 Dart 侧把 `；` 放进 `kSentenceEnders` 意味着：
   读屏会在一个「**协议明确规定不是句界**」的位置立刻播报**
   ⇒⇒⇒ **⇒ 这不是「多了一个无害的字符」，是「与一条显式的协议约定相反」**
3. ⭐⭐⭐ **而 Rust 侧给的理由是「**协议固定**」** ⇒⇒ **⇒ 集合是契约的一部分、不是实现细节**
   ⇒⇒⇒⭐ **⇒ 所以 F-0411-01 建议里那条「让 Dart 侧引用 runtime 的同一集合」有了更强依据：它本来就是契约**
4. ⭐⭐⭐ **而 `is_soft_break` 只在「**安全阀触发时**」用作切点** ⇒⇒
   ⇒⇒ **「句界」与「停顿点」是两个概念**，而 **Dart 侧只有「句界」一个概念**
   ⇒⇒⇒⭐ **⇒ 这解释了为什么 `；` 会被误收进去：缺了「停顿点」那一档**

⇒ ⇒⭐⭐⭐⭐ **而本批最值钱的是这一点**：
> **我第一次在一批之内**用自己写的反证条款反过来**加强**了发现。**
> ⇒⇒ **⇒ 「写出反证条款」与「下一批真的用它」是两件事** ——
> **⇒ 而只有后者发生，反证条款才是条款而不是装饰** ⇒⇒ **同 B0319 那条「不可核标签也会出错」的对称面**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义 ·
   `is_soft_break` 的**安全阀触发条件**（谁在什么时候触发它 · **未读**）
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
