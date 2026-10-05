# BATCH-0131 · `sse.rs`：增量解码器 —— **难形态逐条钉住**（模式 H 第 10 次复查，零命中）

Phase 1 · 域覆盖 · `live2d-ai-runtime`（**不可信上游文本的解析面**）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/sse.rs` — 361（定点 28-77：`SseDecoder` 全部字段与
   `push` / `next_event` / `finish`；**枚举全部方法与 8 条测试名**）

## 跑的命令（全部只读）
```
grep -n "pub fn |fn parse|data: |\[DONE\]" sse.rs
sed -n '28,77p' sse.rs
grep -n "    fn " sse.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；这是本审计**最扎实的一处解析器**
### ① 实现：把「难形态」当作**设计输入**，不是事后补丁
| 难形态 | 处理 | 行 |
|---|---|---|
| 字节**任意切分**（TCP 分片不对齐事件） | `push` 只追加、`next_event` 只在**空行边界**返回完整事件 | :51-64 |
| 尾部**没有空行**就关连接（宽容） | `finish()` 把残余缓冲当最后一行，data 非空则兜底派发 | :66-77 |
| 末尾**悬挂的 CR** | 显式 `pop()`，注释「流末尾悬挂的 CR 是行终止符，**不属于行内容**」 | :73-75 |
| **BOM** | `bom_checked` 状态位（只做一次） | :41-42 |
| 多行 `data:` | 以 `\n` 连接（不是直接拼接） | :39 |
### ② 测试：**8 条全部按「线上的字节形态」命名**，而不是「test_sse_1」
```
decode_byte_by_byte                ← 逐字节喂（最苛刻的切分）
basic_lf_events_and_comments
crlf_and_cr_terminators_mix_freely            ← **混用** CRLF / CR / LF
crlf_split_across_pushes_is_not_two_lines      ← **CRLF 被切在两次 push 之间**
multiline_data_joins_with_newline
field_value_strips_single_leading_space_only   ← 只脱**一个**前导空格（规范细节）
event_field_is_captured_and_reset_between_events
empty_events_are_skipped_without_reseting_pending_state   ← 拼写笔误（reseting）**顺带**记此
```
⇒ 这套命名方式本身就是可复核的证据：**测试名就是它在线上要抵御的那种字节形态**。
⇒ **模式 H 第 10 次复查零命中**；与 B0095（`guess_format` 钉住「6 字节但魔数合法」）
同属本审计**最硬的两个解析器回归**。

### 顺带记一条**笔误**（P3 以下，仅备忘，不记发现）
测试名 `empty_events_are_skipped_without_reseting_pending_state` 里
`reseting` 应为 `resetting`（单 t）⇒ 纯拼写，不影响行为；
**记此是因为它是唯一一处**测试名本身不准确的**地方**，与本仓其它命名质量不一致。

## 未核实项
1. `sse.rs` 其余 ~280 行未读（`SseEvent` 消费侧 / 与 `llm.rs` 的对接 / `[DONE]` 判定）
2. `llm.rs` 其余 ~360 行未读（SSE 解析之上层、错误映射）
3. `client.rs`(652) 除 `MAX_TOKENS` 外未审（重试/超时/`join_endpoint`）
4. `conversation/{engine,mod,worker}.rs` 未审；`plan.rs` 其余 ~700 行未读
5. `secrets.rs` 10 条测试断言体未读；B0120「是否别处 chmod 过 toml」未核
