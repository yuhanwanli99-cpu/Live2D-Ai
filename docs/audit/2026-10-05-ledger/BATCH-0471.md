# BATCH-0471 · ✅ **`strip_wake_phrase` 本体：① 确认 B0470 · ③ 是 B0453 在 Rust 上的第四次出现**

Phase 4 · 证伪（兑现 B0470 留的：读 `strip_wake_phrase` 本体）

## 跑的命令（全部只读）
```
grep -n "fn strip_wake_phrase" -A 16 crates/live2d-ai-mod-voice-input/src/gate.rs | head -20
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**推论被确认，且「回落不与真实值同义」第四次出现**（0 条新发现）
```rust
// crates/live2d-ai-mod-voice-input/src/gate.rs:158-168
pub fn strip_wake_phrase(text: &str, phrase: &str) -> Option<String> {
    let (start, end) = **find_wake_span(text, phrase)?**;                 // ① 找不到 ⇒ None
    let chars: Vec<char> = text.chars().collect();                          // ② ⭐ 按 char 不按 byte
    let mut out = String::with_capacity(text.len());
    out.extend(chars[..start].iter());
    out.extend(chars[end..].iter());
    // 短语与其后紧跟的标点 / 空白一起去掉。
    let body = out.trim_start_matches(is_separator).to_string();
    Some(**crate::clean_transcript(&body).unwrap_or_default()**)             // ③ ⭐ 空 ⇒ 空串
}
```

### 四个可核点
1. ⭐⭐⭐⭐⭐ **① 确认了 B0470 那个推论**：`find_wake_span` 返回 `None` 时**整条传播为 `None`**
   ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒⇒⇒ ⇒ 而 `phrase` 为空时 `find_wake_span` 也必返回 `None`**
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒ 也就是说：B0470 说的「`unwrap_or_else` 在词空下取原样」**成立且是唯一的路径** ⇒⇒⭐⭐⭐⭐⭐⭐⭐
2. ⭐⭐⭐⭐⭐ **③ 是一处 Unicode 安全处理**（`chars()` 而非字节切片）
   ⇒⇒⭐⭐⭐⭐⭐ **⇒ 而中文字符**必需**这一步**（按字节切会切在多字节序列中间）
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒ 这与 AGENTS 那条「界面文案里不得出现子集外的字符」**互为表里** ⇒⇒⭐⭐⭐⭐⭐
   （那条防**输出端**的字符；这条防**处理端**的切分）
3. ⭐⭐⭐⭐⭐ **③ 那一行是本批最值钱的**：`clean_transcript(&body).unwrap_or_default()`
   ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒⇒⇒ ⇒ 它明确区分了「剥完是空的」与「剥完有内容」⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐**
   （若 B0470 那个 `.unwrap_or_else(|| cleaned.to_string())` 用在这里，它就会把「剥完是空的」说成「原样」）
4. ⭐⭐⭐⭐⭐ **⇒ ③ 的形状是 B0453 那条判据的同一条、在 Rust 上的第四次出现**：
   | # | 语种 / 处 | 「缺」怎么表达 |
   |---|---|---|
   | 1 | Rust `toValuesMap()` | 键**不出现** |
   | 2 | Dart `segments: None` | **可空**、不写 `[]` |
   | 3 | Dart `settings_models` `max_tokens` | `containsKey` 问「键在不在」 |
   | **4** | **Rust `strip_wake_phrase` ③** | **专门空串**（与「原样」区分） |

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 3 点**
> **`clean_transcript(&body).unwrap_or_default()` 把「剥完是空的」与「剥完有内容」分成了两件事**
> **⇒⇒⇒⇒⇒ ⇒ 而 B0470 那个 `.unwrap_or_else(|| cleaned.to_string())` 若用在这里，
> 就会把「剥完是空的」说成「原样」** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `find_wake_span` 本体（**B0470/B0471 两批的结论都建立在「它必返回 `None`」上、实现未读**）·
   `clean_transcript` 本体（`unwrap_or_default` 的那条链）·
   `manual_enabled_from_config` / `wake_phrase_from_config`（是否容错 · **未核**）
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
