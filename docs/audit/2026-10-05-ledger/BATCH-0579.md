# BATCH-0579 落盘（无装饰）· 交集 6 份 + verification 5 条名单

## 跑的命令（只读）
```
grep -oE "\]\(plans/[^)]+\)" docs/README.md | sed ... | sort > /tmp/r.txt   -> 28
git ls-files docs/plans/ | sed 's|.*/||' | sort > /tmp/p.txt            -> 139
grep -oE "\]\(verification/[^)]+\)" docs/README.md | ... > /tmp/v.txt    -> 5
for f in $(cat /tmp/p.txt); do grep -q "$f" AGENTS.md && echo "$f"; done > /tmp/a.txt
comm -12 <(sort /tmp/a.txt) /tmp/r.txt                                  -> 6
```

## 交集 6 份（AGENTS 12 份 ∩ README 28 份）
HANDOFF-2026-09-11-core-chain-baseline.md
HANDOFF-2026-09-13-rc2-second-baseline.md
IGNITION-CHECKLIST-product-grade.md
ORCHESTRATOR-PROMPT-actions-performance-round.md
PRODUCT-GRADE-CLOSEOUT.md
RESEARCH-actions-director-audit-2026-09-21.md

## AGENTS 有而 README 无的 6 份
PLAN-rc2-second-baseline-2026-09-12.md
PLAN-rc3-structure-quality-2026-09-13.md
TRIAGE-0.2.0-audit-45-2026-09-28.md
PARALLEL-PROTOCOL-2026-09-14.md
PARALLEL-WAVE2-2026-09-14.md
WAVE3-CLOSEOUT-2026-09-14.md

## README 的 verification 5 条（全部名单）
audio-playback-noise-2026-09-10.md
ignition-core-loop.md
rust-bakeoff-decision.md
smoke-checklist.md
verification-win.md
（docs/verification/ 实有 20 份 md => 覆盖 5/20 = 25%）

## 四个可核点
1. **交集 6 份**：占 AGENTS 12 份的一半、占 README 28 份的约 1/5。
   => 判据：两处索引**不是同一份清单**，各自挑的依据不同。
2. **AGENTS 独有的那 6 份里，有本审计任务书本身**
   （TRIAGE-0.2.0-audit-45-2026-09-28.md）。
   => 也就是说 **AGENTS 比 README 新**：README 停在 09-21，AGENTS 记到 09-28。
3. **README 独有的 22 份**（28 - 6）未逐个列出；它们主要是 HANDOFF-* 与当周件。
4. **verification 覆盖 5/20 = 25%**，与 plans 的 6/12 = 50% 相比更低。
   => 与 B0578 核的 plans 覆盖 28/139 = 20% 一起看：
   **三处「索引」各自覆盖率 20% / 50% / 25%，没有一处承诺全集。**

## 未核
README 独有的 22 份名单 · README 最后更新日期（判断它是否已停更）·
docs/audit/ 那 3 条 · director 本体其余文件
