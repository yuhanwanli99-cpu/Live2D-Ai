# BATCH-0064 · 会话分桶实现面（补上挂了 4 批的空）

Phase 1 · 域覆盖 · Mod 根（第 5 批，同一根）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-desktop/src/session_scope.rs` — 675（**定点 111-130 / 148-166 / 196-266**：
   `compose_slots` / `as_mod_prompts` / `prompt_for` / baseline 四入口 / `set_owned`）
2. `crates/live2d-ai-mod-system/src/session.rs` — 476（B0061 已核 `sanitize_session_id` / `owner_merge_rank`）

## 跑过的命令（全部只读）
```
grep -n "fn as_mod_prompts" -A 30 session_scope.rs
sed -n '196,265p' session_scope.rs
grep -n "fn compose_slots" -A 18 session_scope.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；「按会话分桶不串味」契约**闭环**
- 隔离是**类型级**的：`compose_slots(&Slots)` 只接受某会话的槽引用，**拿不到 `Inner`/id**
  ⇒ 结构上不可能读到别的会话（`prompt_for` 的 `get(&id)?` 无跨会话回落）
- 五处入口**各自**调 `sanitize_session_id`；`prompt_for` / `baseline_for` 被写成
  「**逐字同构**」；:168-172 显式禁止「另起一套归一化或容量判定」——
  **那正是『第二套会话表』的开端**
- 合成顺序单一真源（`owner_merge_rank`，宿主与测试替身**必须**共用，否则组合结果变两份真相）
- ⭐ **对照 F-0062-01**：同批代码里两种姿态——会话表按「多写者会出事」完整设计，
  config 往返按「只有一条路」设计而漏了 ⇒ **模式 L 补充判据**：
  审计多写者系统时还要问「**同一个值会不会经两条路往返**」

## 未核实项
1. `session_scope.rs` 的容量上界（`MAX_SESSION_PROMPT_CHARS` / `MAX_SESSION_BASELINE_CHARS` /
   会话条数上限）与溢出时的行为未读
2. `session_scope.rs:1-110` 与 `:266-675` 未读（含 `compose_session_prompt` 的实现与测试）
3. `mod-system/src/{topics,settings,factory,error,status,descriptor,registry}.rs` 未读
4. `persona`(1004) / `director`(902) 实现本体未读；`template` crate 未读
5. `external-input` / `voice-input` 的 HTTP 端点门禁回归是否覆盖「token 被抹」这一态 —— 未查

## 本批新增
**0 条**（净产出：会话隔离闭环核验 + 模式 L 的补充判据）
