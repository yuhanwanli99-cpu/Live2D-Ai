# BATCH-0473 · ✅ **`manual_enabled` 回落到 `true`** —— 而那是 **B0453 判据的**反例**、**这次是对的**

Phase 4 · 证伪（兑现 B0472 留的：`clean_transcript` 本体 + 两个读配置的函数是否容错）

## 跑的命令（全部只读）
```
grep -n "fn clean_transcript" -A 14 crates/live2d-ai-mod-voice-input/src/lib.rs | head -18
grep -n "fn manual_enabled_from_config" -A 6 crates/live2d-ai-mod-voice-input/src/gate.rs | head -8
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**一个闩、一个不可见字符清单、一个有论证价值的默认值**
```rust
// lib.rs:256-266
pub fn clean_transcript(raw: &str) -> Option<String> {
    let mut pending_space = false;
    for ch in raw.chars() {
        if ch.is_whitespace() {
            // 前导空白落不下（out 为空），尾随空白在循环结束后自然丢
            pending_space = !out.is_empty();
            continue;
        }
        if matches!(ch, '\u{200B}' | '\u{FEFF}' | '\u{200C}' | '\u{200D}') || ch.is_control() {
            continue;
        }
```
```rust
// gate.rs:103-108
pub fn manual_enabled_from_config(config: &Value) -> bool {
    config.get("manual_enabled").and_then(Value::as_bool).unwrap_or(**true**)
}
```

### 四个可核点
1. ⭐⭐⭐⭐⭐ **而 `pending_space` 是一个闩**（延后写空格）⇒⇒
   **⇒⇒ 两个边界用**同一个机制**处理**：「前导空白落不下（`out` 为空）· 尾随空白在循环结束后自然丢」⇒⇒⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 而它的形状是「**边界不是两处特判、而是一个状态**」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐
2. ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而那四种零宽 / 方向控制字符被显式点名**
   （`\u{200B}` 零宽空格 · `\u{FEFF}` BOM · `\u{200C}` ZWNJ · `\u{200D}` ZWJ）
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 关键：它们 `is_whitespace()` **为 `false`** ⇒⇒⇒ 上面那个分支**抓不到它们** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   **⇒⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒ 「所以这两行不能合成一行」**
   ⇒⇒ **判据**：「**`is_whitespace()` 抓不到零宽**」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ ⇒ 这与 B0000 ⑫ / B0142 那类「不可见字符」族同源** ⇒⇒⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `unwrap_or(true)` = 「读不到 ⇒ 闸门默认开」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 这是 B0453 那条判据的一个**反例**、而**这次是对的**：**
   | | 值域里有没有「本身有意义的值」 | 判 |
   |---|---|---|
   | B0453 `max_tokens` | ✅ **有**（`0` = 无限） | ⇒ **不能回落到 `0`** ⇒ 用 `containsKey` |
   | **本处** `manual_enabled` | ❌ **没有**（`true`/`false` 都不是「无意义值」） | ⇒ **回落到 `true` 是对的** |
   ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒⇒⇒⇒⇒ ⇒⇒ 「回落值不能与真实值同义」**在有意义的值存在时才成为问题****
4. ⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「读不到 ⇒ 默认允许」是一个**需要论证**的默认值**
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 它论证的方向是「用户装了 Mod 就是想用、而闸门关着会让他以为坏了」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒⭐⭐⭐⭐⭐ **⇒ 而它的对立面（回落到 `false`，更保守）**也是可辩护的** ⇒⇒⭐⭐⭐⭐⭐
   ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒⇒⇒ ⇒ 而这个默认值**没有注释论证** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐**
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ ⇒ 「可辩护的两种默认值里选了其中一个、而不写为什么」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

⇒ ⇒ **0 findings**（记为**观察**：按判据，「没有论证」不足以记缺陷 —— 同 B0429「规则说不出理由」的**不同**情形：

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. ⭐ **`wake_phrase_from_config` 的缺省词是什么、以及它是否也 `unwrap_or(true)` 式地兜底**（**本批只核了 `manual_enabled` 那一处**）·
   `lower_eq` 本体 · `is_separator` 本体 · `clean_transcript` 的尾部（`:270` 之后 `Option` 怎么变）
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
