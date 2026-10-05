# BATCH-0159 · ⭐ 主链 `ErrorKind::code()`：**`code` 与 `stage` 同源构造 + 回归锁定**（结 B0158 缺口）

Phase 1 · 域覆盖 · `conversation/error_code.rs`（红线「错误码两侧同源」的最后一块）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/conversation/error_code.rs` — 249（定点 44-74：
   `code()` / `stage()` / `is_fatal()`）

## 跑的命令（全部只读）
```
grep -n "pub fn code" -A 30 conversation/error_code.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**红线「错误码两侧同源」从源码侧完整闭合**
### ① `code()` 与 `stage()` **在同一个 `match` 上构造** ⇒ 二者**不可能不一致**
```rust
pub fn code(&self) -> String {                                   // :44
    match self {
        ErrorKind::Llm(e)    => format!("llm_{}", e.code_suffix()),
        ErrorKind::Tts(e)    => format!("tts_{}", e.code_suffix()),
        ErrorKind::Decode(e) => format!("decode_{}", e.code_suffix()),
        ErrorKind::Backpressure { .. } => "tts_backpressure".to_string(),
    }
}
/// **与 [`Self::code`] 的前缀同源**：`code()` 总是以本值开头（**回归测试锁定**），
/// 这样日志里既能按 `stage` 粗筛、也能按 `code` 精确定位。
/// **背压归 `tts`** —— 它就是 TTS 队列的背压，**不是独立阶段**。      // :55-57
pub fn stage(&self) -> &'static str {                           // :58
    match self { Llm → "llm", Tts | Backpressure → "tts", Decode → "decode" }
}
```
⇒ **「`stage` 是 `code` 的前缀」不是纪律，是构造事实** —— 两者在**同一个枚举的同一个 match**
上产出 ⇒ 漏一处编译不过（枚举变体加新臂时两处都要写）⇒ **接缝无人管**这一类在这里**结构上不成立**。
⇒ 而 :55-57 明写这条性质**被回归测试锁定** ⇒ 它**不是「我核过」，是「有人钉住」**。

### ② 「背压」这个**概念上不归属任何阶段**的变体，处理得对
它同时出现在 `code()` 的 `tts_backpressure` 与 `stage()` 的 `tts`，**两处一致**，
理由写明：「它就是 TTS 队列的背压，**不是独立阶段**」
⇒ **把别扭的变体归到它真正所属的阶段，且在两个函数里同样归** ⇒ 又一处「一处归类、两处复用」。

### ③ `is_fatal()` 声明自己是**单一口径**
`:68-69` 「**口径唯一**：与 supervisor 的 `saw_fatal_kind` **完全一致**——`Llm` **不致命**
（已生成的语音继续播完，见 **D8 裁决**）」
⇒ 指名了它在 B0138 核过的那个 D8 决策 ⇒ **三处引用同一裁决**（`is_fatal` / `saw_fatal_kind` / 阶段 1 注释）。

### ④ 由此「错误码契约」的完整链条（**全部从源码核过**）
| 环节 | 机制 | 核验 |
|---|---|---|
| **产生（主链）** | `code()` 与 `stage()` **同源构造** + 回归锁定 | **本批** |
| **产生（表演层）** | 闭合枚举 19 条，`stage` 恒 `performance` | B0158 |
| **传递** | `AppEvent::Error` → WS `error` 帧，**两处码同源** | B0015 |
| **消费** | 前端**从码分流**（`needsFailureFallbackCode` 等），**不从显示文案猜** | B0036/B0037 |
⇒ **四个环节全部核过** ⇒ 这条红线**闭合**（第 14 条红线的最后一环）。

## 未核实项
1. `error_code.rs` 余 ~215 行未读（`code_suffix()` 各族的实现、`hint()` 全族、该文件自测）
   ⇒ **注**：`code_suffix()` 是「上游状态码进码」（如 `upstream_401`）的**实际拼装处**，本批未读
2. `plan.rs` 余 ~560 行未读（`message()` 逐一、`action_cue_payload`(851)、自测）
3. `client.rs` 余 ~540 行未读（`request` 主体、structured 降级、wire 回归测试体）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `mod.rs` 余 ~350 行 · `config.rs:51-140` 未读
5. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
6. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
