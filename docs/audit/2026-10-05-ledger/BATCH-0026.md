# BATCH-0026 · 红线 K 正题（外链扫描）+ 门禁样本三/四 + 跨审计交叉引用

Phase 1 · 域覆盖 → 前端层

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/test/no_backdrop_filter_test.dart` — 272（读 240-272 + 1-49 钉子段）
2. `shell/flutter/test/visual_language_test.dart` — 385（定点：222 / 349）
3. `shell/flutter/test/semantics_test.dart` — （grep RepaintBoundary → 零命中）
4. `docs/plans/ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md` — （定点：106 / 124 / 151 / 200）

## 跑过的命令（全部只读）
```
wc -l no_backdrop_filter_test.dart visual_language_test.dart
grep -n "isNotEmpty|greaterThan|sources.length|零命中|空转|扫到" 两个门禁文件
sed -n '240,272p' no_backdrop_filter_test.dart ; grep -n "expect" semantics_test.dart | head -8
grep -rn "https\?://" shell/flutter/lib/            # 12 命中，逐条归类
grep -rno "https\?://[a-zA-Z0-9./-]*" shell/flutter/lib/ | sort -u
grep -rni "gstatic|jsdelivr|unpkg|cdnjs|cdn\." shell/flutter/lib/ shell/flutter/web/ shell/flutter/pubspec.yaml
grep -n "https\?://" shell/flutter/web/index.html
grep -rn "F-0005-1|假绿灯|恒真" docs/ --include=*.md
git log --oneline -3 --format='%h %ci %s' -- shell/flutter/test/no_backdrop_filter_test.dart
git show --stat --format='%h %s' d0e22f12 | grep -i test
```
`git log` / `git show --stat` 均为**只读** git 用法。未跑任何 cargo / flutter / trunk / pnpm。

## 维度覆盖（本批）
- **红线 K：✔ 验证通过**（源码级，12 处 URL 逐一归类，`web/index.html` 零外链）
- 维度 J：门禁样本三、四同样自带反假通过钉子；`visual_language_test.dart:349` 还测了**扫描器自身的判别力**
- 模式 B：新增**第五种写法**的记录（`contains('X')` 命中「不要用 X」的注释）——本仓已自行修复
- 跨审计交叉引用：团队自己的 4 条假绿灯修复全部落在 Dart，`crates/` 未被扫

## 未核实项
1. Dart 测试文件 102 个，本批累计只读了 **5 个**（design_tokens_lint / font_subset /
   no_backdrop_filter / visual_language 的定点 / semantics 的 grep）；最大三个
   （`display_prefs_test` 892 / `memory_panel_test` 852 / `action_scales_wiring_test` 737）未读
2. `docs/plans/ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md` 只读了 4 行（登记表），
   其余章节（F-0005-2「审计主发现」是什么、组 A–F 的分派）未读
3. 该文档里可能还有**别的已登记发现**与我的账本重叠——**这是当前最大的漏报风险**，
   下一批必须通读该文档的登记表并与 FINDINGS.md 逐条比对

## 本批新增
P0 0 · P1 0 · P2 0 · P3 0（**零发现批**）
｜ 净产出：**红线 K 验证通过** + 模式 B 第五种写法的记录 + **跨审计交叉引用**
