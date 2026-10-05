# BATCH-0025 · Dart 假绿灯首轮横扫 + 红线 K/L

Phase 1 · 域覆盖 → 前端层

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/test/design_tokens_lint_test.dart` — 206（**全读**）
2. `shell/flutter/test/font_subset_test.dart` — 341（读 1-90 / 150-240 / 300-341）
3. `shell/flutter/lib/chat/chat_session.dart` — （片段 405-415）

## 跑过的命令（全部只读）
```
git ls-files 'shell/flutter/test' | xargs wc -l | sort -rn        # 28 894 行 / 102 文件
wc -l design_tokens_lint_test.dart font_subset_test.dart
sed -n '1,120p' / '121,206p' design_tokens_lint_test.dart
sed -n '1,90p' / '150,240p' / '300,341p' font_subset_test.dart
grep -rn "subset|codepoint|rune|toRange|replaceAll(RegExp" shell/flutter/lib/chat/ shell/flutter/lib/data/
grep -n "运行|模型|后端文案|破口|不在" font_subset_test.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 I（设计令牌）：`design_tokens_lint_test.dart` 5 条规则 + 豁免面审计，**能失败**
- 维度 J：**模式 B 在 Dart 侧首轮 = 未命中**（两个高危样本都自带反假通过措施）
- 红线 L：`font_subset_test.dart` 数据层（哈希）+ 词法器自测 + 校准位，**能失败**；
  但**运行态文本是结构性缺口** → F-0025-01（对已登记破口的补证+升级）
- 红线 K：`font_subset_test.dart:323` 的失败文案就是红线 K 的后果描述；
  **外链本身**（gstatic / CDN）本批未做全仓 grep → 顺延

## 未核实项
1. **红线 K 的全仓外链扫描未做**——应在 `lib/` 下 grep
   `gstatic|googleapis|cdn\.|https?://`（`font_subset_test.dart` 的失败文案提到 gstatic，
   但那是**运行时**行为；**源码里**有没有硬编码外链尚未查）。这是红线 K 的正题，**下一批必做**。
2. Dart 测试文件的假绿灯只复查了 2 / 102 个；`app_shell_layout_test.dart` 的 20 个用例
   **断言体未逐行读**。
3. `no_backdrop_filter_test.dart`（毛玻璃规则的真正归属地）未读。
4. F-0025-01 的 emoji 前提**未核**（`*.ranges.txt` 在排除清单内）。

## 本批新增
P0 0 · P1 0 · **P2 1**（对已登记破口的补证+升级）· P3 0
｜ 另：**模式 B 在 Dart 侧首轮未命中**，且结论进一步收窄（门禁面干净，假绿灯只在业务测试面）
