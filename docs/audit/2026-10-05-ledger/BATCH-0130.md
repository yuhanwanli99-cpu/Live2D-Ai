# BATCH-0130 · `llm.rs`：请求体构造 —— **三处「硬形态」都被钉住**，F-0020-01 的定级措辞可收紧

Phase 1 · 域覆盖 · `live2d-ai-runtime`（LLM 请求构造面）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/llm.rs` — 530（定点 125-164：`ChatRequestBody` / `is_zero_u32` /
   `chat_request_body`；:19-22 头注；:400-421 `wire_request_has_no_tools_key`）

## 跑的命令（全部只读）
```
grep -n "fn build_request|fn chat_body|json!(|max_tokens|tools|tool_choice|functions" llm.rs
sed -n '125,164p' llm.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；两处正面 + **对 F-0020-01 措辞的收紧依据**
### ① 「请求体不含 tools」被以**最难的形式**钉住（正面，模式 H 的典范）
```rust
/// **本任务最重要的断言（2026-09-11 用户裁决）**：请求体**不含** `tools` 键。        // :400
/// 不是「tools 为空数组」，而是断言**键根本不存在**：空数组仍会在 wire 上宣告          // :404-406
for max_tokens in [0u32, 512] {                                                      // :413-414
    assert!(json.get("tools").is_none(), "LLM 请求体不得含 tools 键（max_tokens={max_tokens}）…");  // :417-418
    assert!(json.get("tool_choice").is_none(), "got: {json}");                      // :421
}
```
⇒ 断言**键不存在**（而非值为空）· **覆盖两条序列化分支**（`max_tokens` 0 与 512）
· **顺带锁住别名**（`tool_choice`）⇒ rc.1 的「工具层整体拆除」**不可静默回潮**。

### ② `max_tokens: 0` 的编码方式考虑过互操作风险（正面）
```rust
/// 输出 token 上限。**0 = 不限制**（字段整体省略，沿用服务端默认）。                 // :126
#[serde(skip_serializing_if = "is_zero_u32")]                                          // :131
max_tokens: u32,
```
⇒ 「不限制」实现为**整个字段省略**，而不是发 `max_tokens: 0`
（后者在部分服务端会被当作「什么都别生成」）⇒ **跨服务端互操作的风险被想到了**。

### ③ 又一处**跨层对齐**，且是三腿中的一腿
`:128-130`：
> 「这是『控制回复长度』的**机制**——**不靠提示词求模型守规矩**，而是在协议层给出硬上限。
> 分句由我们按 `。！？` 主动做（见 `dialogue/sentence.rs`），**两者配合**才是完整的长度控制。」

⇒ 与 B0116 记的 `DISCIPLINE_TEMPLATE`（提示侧要求「每句话以真实句读结尾」）
和装配器侧「只按真实句读切、不许硬切」**同属一条跨层设计**，而这里是**第三条腿**（协议层硬上限）。
⇒ **三腿都被显式写明并互相引用**。

### ⭐ 对 F-0020-01 措辞的**收紧依据**（不改定级，只补上下文）
F-0020-01（P1）的指控是「`MAX_TOKENS = 1_024` 使超长回复被确定性截断」。
本批核到：**该机制本身的设计是对的**（字段省略语义、跨服务端互操作、与分句器的配合都考虑到了）
⇒ **缺陷是「一个常量的值不对」，不是「机制不对」** ⇒ 修法是**改一个常量**，
且**不应改机制**（例如不宜把 0 改成「发 0」或加别的编码）。
⇒ 这与 CONSOLIDATION-12 §③ 的判据一致：**这一片老代码的缺陷是「局部的具体疏漏」**。

## 未核实项
1. `llm.rs` 其余 ~360 行未读（SSE 解析 `:165+` / `LlmConfig` / 错误映射 / 其余测试）
2. `client.rs`(652) 只在 B0013 定点过 `MAX_TOKENS`；其余（重试/超时/`join_endpoint`）未审
3. `conversation/{engine,mod,worker}.rs` 未审；`sse.rs`(361) 未审
4. `plan.rs` 其余 ~700 行未读
5. `secrets.rs` 10 条测试断言体未读；B0120「是否别处 chmod 过 toml」未核
