# BATCH-0128 · ⭐ **收窄 F-0127-01（P2→P3）**，且**自己又犯了模式 Q 一次**

Phase 1 · 域覆盖 · `live2d-ai-runtime`（`plan.rs` 的 schema 面）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/plan.rs` — 857（定点 787-836 `json_schema_strict`；
   :22-23 头注的 `speak` 定位；:307/:318 `SpeakIgnored`；:350/:364-365；:391/:441/:458）

## 跑过的命令（全部只读）
```
sed -n '787,836p' plan.rs
grep -n "SpeakType|speak" plan.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**两件事，都关于我自己的记录**
### ① F-0127-01 的范围被我**下一批就收窄**（P2 → **P3**）
B0127 我写「`speak` 静默截断，而同函数内其它上限都报错 ⇒ 该文件唯一的例外」。本批核 `json_schema_strict`：
```rust
"segments": { "type": "array", "maxItems": MAX_SEGMENTS, "description": …"总字符不超过 {MAX_SEGMENT_CHARS}" },  // :795-800
"speak":    { "type": ["string","null"], "description": "**已弃用（V11 保留）：与 segments 同现时被忽略。**" },  // :803-806
"required": ["segments", "cues"],                                                                             // :793
```
⇒ **`speak` 是 v0 旧字段、已弃用**；schema 的 `required` 里**只有 `segments` 和 `cues`**，
所以**模型按 schema 产出时根本走不到那个截断**。
且与 `segments` 同现时是 **`SpeakIgnored` + warn**（:307/:318「segments 与 speak 同现：按 O1 忽略 speak」）——
**有提示，不静默**。
⇒ **真实范围**：只有**弃用兼容路径**（`speak` 存在而 `segments` 缺席，:458 `MissingSegments` 的另一侧）
才吃到那个静默截断。⇒ **P3**（原记述范围过宽）。
⇒ **顺带一个正面点**：schema 给 `segments` 写了 `maxItems`（机器可校验），
而**字符上限只写在 `description` 里**（`"总字符不超过 {MAX_SEGMENT_CHARS}"`，:799）
⇒ 也就是说**字符预算这条约束在 schema 层是「提示」而非「约束」** —— 这与 F-0127-01 同源，
但**对现代路径影响小**（因为 `MAX_SEGMENT_CHARS` 的硬校验在解析器里，:610-613 的
`segments.concat() == source` 不变式会把不合法的分段**整条拒掉**，B0013 已核）。

### ② ⚠ **我隔一批就又犯了模式 Q 一次**（必须诚实记）
CONSOLIDATION-12 §② 刚修掉「第 1 类缺陷：发现只有散文、没有 `### F-` 规范头」，
并立了规则：**改定级/存废必须同时改规范头**。
**B0127 写 F-0127-01 时，又只写在了批次散文里，没有规范头。**
⇒ 本批发现时，`grep "^### F-0127-01"` **零命中**；而我给 B0127 写的 INDEX/STATE 摘要里
已经写着「F-0127-01（P2）」—— **即：账本对外声称有这条 P2，账本内部却找不到它**。
⇒ 已补规范头，并把收窄后的 **P3** 与「模式 Q 复发」的说明**写在头下**。
⇒ **教训（比规则本身更重要）**：**立了规则不等于会执行**。
**模式 Q 需要一个机械检查**，否则「散文里的发现」与「规范头」会继续分叉：
> **每批收尾时必须跑一次「INDEX/STATE 里提到的每个 `F-xxxx-xx` 都能在 FINDINGS.md 里 grep 到
> `^### F-xxxx-xx`」**，否则本批的对外摘要就会引用一条不存在的条目。
⇒ 这条已写进模式 Q 的执行要求（见下）。

## 未核实项
1. `plan.rs` 其余 ~700 行未读（`PerformanceCue` / `PlanError` 全族 / 分段算法 :530-592 /
   `action_cue_payload`(851)）
2. `llm.rs`(530) 未审；`client.rs`(652) 只定点过 `MAX_TOKENS`
3. `conversation/{engine,mod,worker}.rs` 未审；`sse.rs`(361) / `audio/*` / `error_code.rs` 未审
4. `secrets.rs` 10 条测试的断言体未读；CRLF 与不配对引号两形态无测试（B0126）
5. B0120「是否别处 chmod 过 toml」仍未核
