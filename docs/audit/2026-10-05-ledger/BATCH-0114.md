# BATCH-0114 · ⭐ **F-0114-01（P2）：伪造 marker ⇒ 记忆块污染基线并逐轮累积**

Phase 1 · 域覆盖 · Mod 根第 11 批

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-memory/src/strategy.rs` — （定点 35-37 常量 / 357-377 剥离 / 383-390 行清洗 / 478 拼装）

## 跑过的命令（全部只读）
```
grep -rn "fn budget_injection|fn injection_block|fn build_injection|注入块|【|<<<|\[记忆" mod-memory/src/*.rs
grep -rn "MARKER|marker" mod-memory/src/lib.rs
grep -rn "MEMORY_MARKER_BEGIN\s*:|MEMORY_MARKER_END\s*:" -A 3 mod-memory/src/*.rs
grep -rn "MEMORY_MARKER_BEGIN" mod-memory/src/strategy.rs
grep -n "fn strip_memory_block|fn strip\b" -A 22 mod-memory/src/strategy.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0114-01（P2）**（B0113 留的点，答案是「有问题」）
四步全部核到行：
1. **拼装** `base + BEGIN\n{body}\nEND`（`strategy.rs:478`）；
2. **记录里的 marker 不会被中和** —— `sanitize_memory_line` 的 `split_whitespace().join(" ")`
   （:384）折叠空白，而 marker **本身只含单空格** ⇒ 逐字保留；
3. **剥离提前收口** —— `strip_memory_block` 取「BEGIN 之后**第一个** END」（:370）⇒
   伪造 END 使删除只覆盖 `BEGIN..伪造END`，**真实 END 与其后残片留在基线**；
   随后 `find(BEGIN)` 已无命中 ⇒ 循环退出 ⇒ **残片永久留存**；
4. **逐轮累积** ⇒ 提示词里两个 END + 上一轮记忆残片常驻基线 ⇒ 每轮都变差；
   关闭记忆 Mod 也剥不干净（`lib.rs:491/744` 同按 marker 剥）。
**定 P2**：触发需用户/角色卡文本**逐字含该 marker**（低概率、无远程路径）；后果是**持久**状态损坏。
**不是 P1**：无安全影响、无外泄、需本机内容命中。
**与 B0109 同源**：`:364-366` 已考虑「有 BEGIN 无 END 的半截块」并正确选择「剥到末尾」，
但**没考虑「多一个 END」** ⇒ 两半都对、接缝无人管。
**修法（两条都是「让约束跟着组件」）**：① `sanitize_memory_line` 中和 marker（一行）⇒ 记录不可能携带它；
② `strip_memory_block` 改为「取最后一个 END」或 BEGIN/END 计数配平；③ 补回归：
   内容含 END marker 的记录 ⇒ 幂等且**基线不含任何记录文本**。

## 未核实项
1. 检索与打分（词元重叠 / `top_k`）未读
2. `memory/src/{summary.rs,store.rs,summary_store.rs,commands.rs}` 其余部分未读
3. `persona/sessions.rs` 全文 · `wallpaper`(729) · `template` 未读
4. 角色卡文本是否也会经过 `sanitize_memory_line`（若**不经过**，则 persona 侧是另一条 marker 注入面）—— **未核**

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0
