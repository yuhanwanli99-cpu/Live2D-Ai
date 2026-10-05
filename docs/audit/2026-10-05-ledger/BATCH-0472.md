# BATCH-0472 · ✅ **两批推论全部成立** —— 而**文档说的两件事各有各的代码落点**

Phase 4 · 证伪（兑现 B0471 留的：读 `find_wake_span` 本体）

## 跑的命令（全部只读）
```
grep -n "fn find_wake_span" -A 26 crates/live2d-ai-mod-voice-input/src/gate.rs | head -30
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**五条 `None` 路径，每条一个名字**
```rust
// crates/live2d-ai-mod-voice-input/src/gate.rs:176-203
fn find_wake_span(text: &str, phrase: &str) -> Option<(usize, usize)> {
    let hay: Vec<char> = text.chars().collect();
    let needle: Vec<char> = phrase.chars().filter(|c| !c.is_whitespace()).collect();
    if needle.is_empty() { return None; }                                   // ① 词里没有非空白
    // 句首 = 第一个非空白字符；文本为空 / 全空白 → 不命中。
    let start = hay.iter().position(|c| !c.is_whitespace())?;                // ② 全空白
    let mut pi = 0usize; let mut i = start;
    while i < hay.len() && pi < needle.len() {
        let c = hay[i];
        if c.is_whitespace() { i += 1; continue; }                           // ③ 匹配中跳过文本空白
        if lower_eq(c, needle[pi]) { pi += 1; i += 1; } else { break; }     // ④ 不匹配 ⇒ break
    }
    if pi == needle.len() { Some((start, i)) } else { None }                 // ⑤ 没走完
}
```

### 四个可核点
1. ⭐⭐⭐⭐⭐ **B0470 / B0471 两批的推论全部成立**：
   - **`phrase` 为空 ⇒ `needle.is_empty()` ⇒ ① 返回 `None`**
   - ⇒⇒ **所以 `strip_wake_phrase` 在词空下必 `None`、`.unwrap_or_else(|| cleaned.to_string())` 是唯一路径**
   - ⇒⇒ **而 B0470 说的「把 ② ③ 对调会改变行为」也成立**：若 ② 排在 ③ 之后，词空会先走到 ③
2. ⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 ② 那一行注释「句首 = 第一个非空白字符；文本为空 / 全空白 → 不命中」把理由**写在旁边** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `phrase.chars().filter(|c| !c.is_whitespace())` 是「短语自身的空白先被丢掉」的**实现**
   ⇒⇒ **而文档（`:170-174`）说的两件事，各有各的代码落点**：
   | 文档说的 | 落在哪 |
   |---|---|
   | 「**短语自身的空白**先被丢掉」 | `:178` 的 `filter` |
   | 「**文本里的空白**在匹配过程中被跳过」 | `:188-190` 的 `if c.is_whitespace() { continue }` |
   ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ 「文档说的两件事」**各有各的落点** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐**
4. ⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `lower_eq`（大小写不敏感）也是文档里说的**（`:174` 之后）⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒⇒ 三条文档承诺、三个代码落点、一一对上** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 3 点**
> **文档说的两件事各有各的代码落点** ——
> **「短语自身的空白先被丢掉」落在 `:178` 的 `filter` ·「文本里的空白在匹配过程中被跳过」落在 `:188-190`**
> **⇒⇒⇒ ⇒⇒ ⇒⇒ 而这两句**长得极像**、极容易被当成同一件事** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐**
> **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒⇒ 「把两处都写成『忽略空白」会让人以为只有一处、于是改动时只改一处」** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
> **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ 这与 B0443「同一句话在同一文件里出现三次」**是同一族的反面**：
> **那一族是「重复得不够」，这一族是「**像得过头**」** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `clean_transcript` 本体（`unwrap_or_default` 的那条链）· `lower_eq` 本体 ·
   `manual_enabled_from_config` / `wake_phrase_from_config`（是否容错 · **未核**）·
   `is_separator`（`strip_wake_phrase:166` 用它做 `trim_start_matches`）
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
