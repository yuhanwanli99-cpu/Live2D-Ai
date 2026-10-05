# BATCH-0108 · `persona` 的 PNG 卡解析：**手写二进制解析，逐处先量再取**

Phase 1 · 域覆盖 · Mod 根第 5 批

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-persona/src/lib.rs` — 1004（**结构穷举**（无 `head`）+ 定点 404-487：
   `extract_png_chara` / `read_text_chunk` / `read_itxt_chunk` / `latin1` / `read_u32` / `parse_chara_payload`）

## 跑过的命令（全部只读）
```
grep -n "^pub struct|^pub enum|^impl|^    pub fn |^    fn |^fn " mod-persona/src/lib.rs     # 无 head
sed -n '438,487p' mod-persona/src/lib.rs
sed -n '404,439p' mod-persona/src/lib.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；**我自己两个越界假设被逐行推翻**
`persona` 要解析**用户导入的角色卡 PNG**（不可信二进制），用的是**手写 chunk 遍历** ——
这类代码是越界 panic 的高发区，所以我先查它。

### 假设一：「`read_itxt_chunk` 的切片会越界」—— **证伪**
我逐行追了索引来源：`nul` 由 `position` 得（`< len`）；`:457` 的 `p + 2 > data.len()` 守住 `:460` 的
`data[p]`；`:462-463` 的 `lang_end` / `translated_end` **都由 NUL 位置推导** ⇒ 恒 `< len`，
对应切片至多为空（Rust 允许 `&v[len..]`）；找不到 NUL 时 `?` 传播 `None`。⇒ **无 panic 面**。

### 假设二：「`read_u32(bytes, at)` 无边界检查 ⇒ 越界」—— **证伪（但结论更值得记）**
`read_u32`（:472-474）确实**裸索引** `bytes[at..at+4]`。但它**只有一个调用者**
（`extract_png_chara:408`），而那个调用者的循环条件是 `while offset + 8 <= bytes.len()`（:407）
⇒ **`read_u32` 的 4 字节与随后的 `&bytes[offset+4..offset+8]` 都被这一条覆盖**。
⇒ **结构上的事实**：一个**私有、无检查的原语** + **恰好一个带守卫的调用者**。
⇒ **记此备忘（不是发现）**：这在本仓是**可接受**的写法（私有 + 单调用者 + 调用点自带守卫），
但它**对未来的改动是脆弱的** —— 若有人加第二个调用者而忘了守卫，就会变成真越界。
**建议（低成本）**：给 `read_u32` 加一个 `debug_assert!(at + 4 <= bytes.len())`
或直接返回 `Option<u32>`，让不变量**跟着函数走**而不是**跟着调用者走**。
（与 F-0092 的修法思路同源：**让约束跟着组件，而不是指望每个调用点都记得**。）

### 正面：`extract_png_chara` 的纪律写在**函数自己的头注**上
```rust
/// 全程只做「先量再取」的切片：块长度越界 → `Err`，不会 panic。      // :404
```
- `:412` `if data_end + 4 > bytes.len() { return Err(...) }` —— **先量后取**，
  且错误**带块长度与剩余字节数**（可定位）；
- 压缩 iTXt **显式拒绝并给可执行建议**（:426-429：「当前不支持；**请用 tEXt 卡或直接导入 JSON**」）
  ⇒ 而不是误解压或静默跳过；
- `IEND` 正常收尾（:433-435）；`offset = data_end + 4` 跳 CRC 后**由循环条件复检**。
- `parse_chara_payload`（:477-487）先试 base64（`STANDARD` 再 `STANDARD_NO_PAD`），
  失败时**只接受看起来像 JSON 的**（`trimmed.starts_with('{')`），否则**报明确错误**。

## 未核实项
1. `persona` 的**提示词写回路径**未读（`load_sessions` :573 / `load_card` :604 / `PersonaRuntime`
   的其余方法）—— **这是 B0061/B0062 留下的关键面**：它既用 `apply_settings` 写**全局**
   `persona.system_prompt`，又用 `ModSessionPrompts` 写**按会话**覆盖 ⇒
   **「A 会话的卡是否会经全局写回泄漏到 B 会话」仍未核**
2. `PersonaCard::parse_json`（:210-241）与其余 `PersonaConfig`/`PersonaRuntime` 方法未读
3. `mod-persona/src/tests_e2e.rs`(930) / `tests.rs`(849) 断言体未读
4. `memory/{summary.rs,store.rs,summary_store.rs}` 本体未读；`voice-input` / `wallpaper` / `template` 未读

## 本批新增
**0 条**（+ 2 个假设被自己推翻 + 1 条**备忘级建议**：让 `read_u32` 的不变量跟着函数走）
