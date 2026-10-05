# BATCH-0578 落盘（无装饰）· docs/README 确实是索引：B0577 的「0 次」是路径写法错

## 自纠（重要）
B0577 我跑的是 grep -c "docs/plans" docs/README.md -> 0，并据此说「README 不索引 plans」。
**这是错的。** README 里写的是**相对路径** plans/xxx.md，不是 docs/plans。
grep -cE "\]\(plans/" docs/README.md -> **28**
样例：](plans/HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md)
=> 这是 B0440 第 6 种形态（值在另一个目录/写法）的又一次自命中。
   **修正后的结论：docs/README.md 承担索引职责，且 plans 是它的第二大类。**

## 88 个条目按目标首目录分组
（grep -oE 提链接 + awk 取首段 + sort | uniq -c | sort -rn）

| 计数 | 首目录 |
|---|---|
| 29 | architecture |
| 28 | plans |
| 11 | releases |
| 10 | research |
| 5 | verification |
| 5 | ..（仓库根） |
| 3 | audit |
| 2 | legacy |

## 四个可核点
1. **两处索引并存**：AGENTS 逐名点 12 份 plans（8.6%），docs/README 覆盖 28 份（20%）。
   两者交集未核。
2. **README 的类目有实质**：architecture 29 > plans 28 > releases 11 > research 10。
3. **而 verification 只有 5 个条目**，而 B0573 数出 docs/verification/ 有 20 份 md
   => **⇒ 索引覆盖不完整**：5/20 = 25%。
   => 与 B0570 核的「三处里 AGENTS 点名 12 份」**同族**：
   **任何一份「索引」都不是全集，判据要看它承诺覆盖什么。**
4. 修正后重述 B0577 第 2 点：README **不是**「plans 的空白」，它 **28 条 plans 链接**；
   B0577 那句「README 承担索引职责」的方向是对的，**证据（0 次）**是错的。

## 未核
AGENTS 12 份与 README 28 份的交集 · verification 那 5 条是哪 5 份 ·
docs/audit/ 那 3 条 · director 本体其余文件
