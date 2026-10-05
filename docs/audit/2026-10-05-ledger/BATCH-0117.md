# BATCH-0117 · `memory` 的 store 上限：**钳位保证了有界**，代价是「上限 = 每轮 I/O 预算」

Phase 1 · 域覆盖 · Mod 根**最后一批**

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-memory/src/store.rs` — （定点 96-122：`append_capped` / `rewrite` 原子写 + :387 回归名）
2. `crates/live2d-ai-mod-memory/src/strategy.rs` — （定点 52-53：`MIN/MAX_MAX_RECORDS`）

## 跑过的命令（全部只读）
```
grep -n "fn append_capped" -A 26 mod-memory/src/store.rs
grep -rn "MIN_MAX_RECORDS\s*:|MAX_MAX_RECORDS\s*:|const MIN_MAX_RECORDS|const MAX_MAX_RECORDS" mod-memory/src/*.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；记一条**备忘级取舍**
### ① 上限由**配置钳位**保证 ⇒ 文件一定有界
`strategy.rs:52-53`：`MIN_MAX_RECORDS = 1` · `MAX_MAX_RECORDS = 10_000`
⇒ `append_capped:103` 的 `max_records == 0`（= 不限）分支**从配置路径不可达**（钳位下界是 1），
它只是**防御性**分支 ⇒ **「文件无限增长」这个失败模式被钳位挡在门外**（正面）。

### ② 代价：上限值同时也是**每轮 I/O 预算**（记此备忘）
`append_capped`（:96-117）的实现是 **append → load(整个文件) → rewrite(整个文件)**：
```rust
self.append(record)?;                 // :101
let loaded = self.load(0)?;           // :102  ← 读**整份**
if max_records == 0 || loaded.records.len() <= max_records { … }
let cut = loaded.records.len() - max_records;
self.rewrite(&loaded.records[cut..])?;   // :111 ← 再写**整份**（保留最新 max_records 条）
```
⇒ 每次追加的 I/O 与**当前记录数**成正比；在上限 10,000 × 约 200 字符（`MAX_MEMORY_LINE_CHARS`
`strategy.rs:43`）≈ **2 MB** 时，单轮约 **4 MB** 读+写。
⇒ **不记发现**的理由：文件**有界**（①）、上限**用户可调**、原子写是正确性所需
（`rewrite` 文档 :119-122：「`rename` 在同一文件系统内是原子的，因此崩溃/断电最多留下一个
无关的 `.jsonl.tmp`，**不会把正式文件截成半截**」），且有回归
`append_capped_truncates_file_to_cap_keeping_newest`（:387）钉住「保留最新」这个语义。
⇒ **记此备忘的目的**：让日后调整 `MAX_MAX_RECORDS` 的人知道 —— **那个数字同时是每轮 I/O 预算**，
不是纯粹的「能存多少条」。

### ③ `rewrite` 的原子性有回归可查
`:387` 的测试名即断言「**truncates file to cap, keeping newest**」⇒ 物理淘汰方向（新留旧删）被钉住。

## 未核实项（Mod 根**本批之后**的剩余面）
1. `memory/src/{summary.rs,summary_store.rs,commands.rs,assistant.rs 余段}` 未读
   （检索打分、摘要触发、命令面）
2. `persona/sessions.rs` 全文 · `PersonaCard::parse_json` 未读
3. `wallpaper`(729，**已封存**) · `template` 未读 —— **两者是 Mod 根仅剩的整文件空白**
4. 各 Mod 的 `settings_spec` 与实际读取键是否一致未核
5. `include_discipline` 默认值与前端是否暴露未核

## Mod 根小结（B0100–B0117，18 批）
- **产出 2 条 P2**：F-0111-01（argv 携带 token + 可带凭据 URL）、F-0114-01（伪造 marker 污染基线）
- **两条都发生在「Mod ↔ 宿主」的边界上**（进程边界 / 标记边界），**Mod 内部逻辑 0 缺陷**
- **2 次自我更正**：B0104「唯一带 HTTP 客户端的 Mod」（实为两个）、B0109/B0110 扫描口径（漏 API 族）
- **正面样本密集**：默认关 + 键名（4 个一致样本）· 私有汇合 + 策略闭包（6 个消费者）·
  更严的写法 / 建模优于判断（5 个样本）· 三层注入预算 · 空对象式「默认关」·
  跨层的「提示侧 ↔ 实现侧」对齐
