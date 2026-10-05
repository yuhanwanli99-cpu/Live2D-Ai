# BATCH-0581 落盘（极简）· README 独有 22 份全名单

## 命令（只读）
comm -13 <(sort /tmp/a.txt) <(sort /tmp/r.txt) > /tmp/only_r.txt   -> 22
grep -oE "2026-09-[0-9]+" /tmp/only_r.txt | sort | uniq -c

## 22 份全名单
```
AUDIT-PROMPT-whole-repo-2026-09-28.md
HANDOFF-2026-09-10.md
HANDOFF-2026-09-11.md
HANDOFF-2026-09-21-actions-performance-round.md
HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md
IGNITION-CHECKLIST-stabilize.md
IMPL-PROMPTS-actions-performance-round.md
PLAN-V1.md
PLAN-V2-PC-LOCAL-TTS.md
PLAN-V3-SOULLINK-PERFORMANCE.md
PLAN-audio-wav-path-2026-09-11.md
PLAN-frontend-strengthening-2026-09-11.md
PLAN-ignition-core-loop-2026-09-10.md
Phase-0-notes.md
RUST-REWRITE-RFC.md
STABILIZE-PRECHECK-RESULT.md
future-roadmap-2026-09.md
integrity-takeover-fix-plan-2026-09-07.md
node-a-wiring-audit-brief.md
parallel-mods/PARALLEL-WAVE3-2026-09-14.md
parallel-mods/STABILIZE-CLOSEOUT.md
parallel-mods/WAVE3-CLOSEOUT-2026-09-14.md
```

## 按日期分组
| 日期 | 份数 |
|---|---|
| 2026-09-07 | 1 |
| 2026-09-10 | 2 |
| 2026-09-11 | 3 |
| 2026-09-14 | 2 |
| 2026-09-21 | 1 |
| 2026-09-28 | 2 |

余下 **11 份文件名里没有日期**：PLAN-V1 / PLAN-V2-PC-LOCAL-TTS /
PLAN-V3-SOULLINK-PERFORMANCE / Phase-0-notes / RUST-REWRITE-RFC /
STABILIZE-PRECHECK-RESULT / future-roadmap-2026-09 /
IGNITION-CHECKLIST-stabilize / node-a-wiring-audit-brief /
parallel-mods/STABILIZE-CLOSEOUT 等。

## 三个可核点
1. **22 份里 11 份不带日期**。B0578 那条「看文件名日期就知道属于哪一代」
   **只对一半文件成立**，另一半必须打开文件才能判年代。
2. **README 用 `parallel-mods/xxx` 二级目录**指计划；AGENTS 引的是根目录下的
   `PARALLEL-PROTOCOL` / `PARALLEL-WAVE2`（无子目录）。
   判据：**两处索引的「路径形状」都不同**（一个带子目录、一个不带），
   交叉比对时必须按 basename 比、不能按全路径比。
3. **README 独有 22 份里，最新的是 09-28 两份**（本轮审计那两份）；
   AGENTS 独有 6 份里，最新的也是 09-28（TRIAGE-0.2.0-audit-45）。
   再次印证 B0580：**两份索引同日期、不同子集**。

## 未核
11 份无日期文件属哪一代 · parallel-mods 子目录里还有多少份未进两份索引 ·
docs/audit/ 那 3 条 · director 本体其余文件
