# BATCH-0027 · 跨审计去重（对照 ORCHESTRATOR-PROMPT closeout 登记表）

Phase 1 · 域覆盖 → **跨审计层**（不审新代码，只做去重与交叉验证）

## 读过的文件（全部来自 `git ls-files` / 任务指定的只读引用）
1. `docs/plans/ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md` — 250（**全读**）
2. `shell/flutter/lib/settings/sections/llm_section.dart` — （读 155-189）
3. `shell/flutter/lib/app/app_shell.dart` — （读 262-309，验证 F-0005-2 的修复）

## 跑过的命令（全部只读）
```
grep -o "F-00[0-9][0-9]-[0-9]" closeout-2026-09-28.md | sort -u      # 45 个编号
grep -o "^### F-[0-9]\{4\}-[0-9]\{2\}" AUDIT-REPO/FINDINGS.md      # 我的 50 条
grep -rn "contains('ok')|contains(\"ok\")|contains('ms')|contains(\"ms\")" shell/flutter/lib/
sed -n '158,189p' llm_section.dart ; sed -n '1,59p' / '60,189p' closeout-2026-09-28.md
git log --oneline --format='%h %ci %s' -- no_backdrop_filter_test.dart
git show --stat --format='%h %s' d0e22f12 | grep -i test
```
未跑任何 cargo / flutter / trunk / pnpm。`git log` / `git show --stat` 为只读 git 用法。

## 本批产出（**0 条新缺陷**，但关闭了本轮最大的漏报风险）
1. **两套编号命名空间独立**（`F-0001-1` vs `F-0001-01`），语义比对必须按内容而非编号
2. **重叠仅 1 条**：他们的 `F-0017-1` ≡ 我的 `F-0021-01`（同一事实的两个方向）
3. **他们的 45 条 100% 落在 Dart**（`crates/` 从未被任何一轮审计覆盖）——我这一轮补的正是那另一半
4. **独立交叉验证：两个互不知情的审计都得出 P0 = 0**（他们 58 源+34 测试文件；我 ~70 个 Rust 文件）
5. **三条已登记项的 HEAD 现状**：F-0005-2 已修（机制在位）· F-0012-1 **仍存在**（P1）·
   rc.6 背景域未抽查

## 未核实项
1. 他们的其余 42 条**未逐条验证 HEAD 现状**（只抽查了 3 条）——它们落点在 Dart 背景域，
   我尚未审的 `main.dart` / `shell_prefs.dart` / `shell_backdrop.dart` / `background_hydration.dart`
   等文件里，**可能在修、也可能没修**，需逐条核（留后续前端批次）
2. ~~原始账本可能已丢失~~ —— **本条自我更正**：我先按 `ls -d AUDIT AUDIT-B` 判「均不存在」
   并担心证据不可回溯，**随后复查发现它们已按 closeout §2.1 的要求入库到
   `docs/audit/2026-09-28-frontend-nightly/`**（长跑 21 批在根目录，短跑 4 批在 `short-run/`），
   **证据链完整**。那份 README 还自带冻结口径 `45 条 · P0 0 / P1 12 / P2 19 / P3 14`，
   并**公开了自己过去的计数错误**（「P1=12 而非 10」：因 `F-0006-1`/`F-0013-1` 的标题写成
   `P1（**升级 F-000x-y**）`，朴素匹配 `· P1 ·` 会漏）。**记此更正是因为它本身就是模式 D 的反例：
   一份账本把「我以前数错了、为什么错、怎么复算」写进文档，比装作没发生过更有价值。**
3. closeout §1 提到「24 个 worktree / 1 个 stash」；我未核查（属只读 git 范畴，但与本审计无关）

## 本批新增
P0 0 · P1 0 · P2 0 · P3 0（**纯去重批**）


---

## BATCH-0027 补：两轮审计的口径对照（校准用）

| | 团队前端审计 | 本审计 |
|---|---|---|
| 范围 | **Dart 117 文件**（58 源 + 34 测试，另含 `test/` 计数口径） | **Rust 约 70 文件**（web_api 49/49 结项 + runtime 22 + mod_registry 部分）+ 前端 5 批片段 |
| 批次 | 21 批 + 2 份 CONSOLIDATION | 27 批 + 3 份 CONSOLIDATION |
| 冻结计数 | **45 条 · P0 0 / P1 12 / P2 19 / P3 14** | **50 条 · P0 0 / P1 7 / P2 23 / P3 20** |
| 重叠 | — | **1 条**（`F-0017-1` ≡ 我的 `F-0021-01`） |
| 共同结论 | P0 = 0 | P0 = 0 |

⇒ 两轮**互不知情**、树不同、方法不同的审计，在**重叠仅 1 条**的前提下得出**同一个 P0 = 0**，
且各自的 P1 数量级相当（12 vs 7）。这是对「该仓库真实风险分布」的有力约束：
**高危面不在「已知债务清单」上，而在从未被任何一轮审计覆盖的那一半树上**——
他们漏了 `crates/`，我（前 26 批）漏了前端 212 个文件。
