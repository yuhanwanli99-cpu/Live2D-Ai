# BATCH-0115 · **证伪「第二条 marker 注入面」**；并分清两种 Mod 内容的**正确形态差异**

Phase 1 · 域覆盖 · Mod 根第 12 批

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-persona/src/lib.rs` — 1004（定点 285-306：`compose_system_prompt`；308-311 `CardSource`）

## 跑过的命令（全部只读）
```
grep -rn "fn compose_system_prompt" -A 26 mod-persona/src/*.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；**一个未核实的假设被证伪**
### 假设：「角色卡文本不经 `sanitize_memory_line` ⇒ 它是 F-0114-01 的**第二条**注入面」—— **证伪**
`compose_system_prompt`（:285-306）确实把 `card.name` / `description` / `personality` /
`scenario` / `system_prompt` **原样** `.clone()` 拼接（**不经过**记忆那套清洗）。
**但它不构成 F-0114-01 的第二个实例**，理由是**位置**：
- F-0114-01 的损坏要求伪造的 END 落在**记忆块 body 内部**（即在 `BEGIN` **之后**），
  因为 `strip_memory_block` 取的是「**BEGIN 之后**第一个 END」（`strategy.rs:370`）；
- 角色卡文本在合成提示词里位于**记忆块之前** ⇒ 它的伪造 END 在 `BEGIN` **之前**
  ⇒ `find(BEGIN)` 先命中真实 BEGIN，其后第一个 END 就是**真实的那个** ⇒ **剥离不受影响**。
⇒ **F-0114-01 的范围维持「记忆记录」一条**，不扩到 persona。

### 顺带把两种 Mod 内容的形态差异**分清了**（这是对的，不是不一致）
| Mod | 进入 system prompt 的形态 | 有无标记 | 是否恰当 |
|---|---|---|---|
| `persona` | 角色卡正文（原样） | **无** | **恰当** —— 角色卡**按设计就是指令**（它就是人设） |
| `memory` | 召回的对话记录 | **有** `BEGIN…END`（`strategy.rs:478`） | **恰当** —— 记忆是**数据**，需要与指令区分 |

⇒ 两者形态不同**不是遗漏，是按内容性质分别对待**：把「指令」标成数据会削弱它，
把「数据」混进指令会污染它。**这也解释了为什么 F-0114-01 只可能发生在记忆侧** ——
只有「数据」那条路径才有「必须被标记、因而需要 marker」的约束，
也才因此需要防「伪造 marker 越出标记范围」。

## 未核实项
1. `memory/src/{summary.rs,store.rs,summary_store.rs,commands.rs}` 本体未读（检索/打分、摘要触发、store 上限）
2. `persona/sessions.rs` 全文未读；`PersonaCard::parse_json` 未读
3. `wallpaper`(729，已封存) · `template` 未读
4. 各 Mod 的 `settings_spec` 与实际读取键是否一致未核
5. `DISCIPLINE_TEMPLATE`（:303 被拼进 system prompt）的内容与是否含 marker 未核

## 本批新增
**0 条**
