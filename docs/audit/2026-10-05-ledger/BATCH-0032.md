# BATCH-0032 · 验证「约定与回归」+ 为团队 F-0012-1 补机制

Phase 1 · 域覆盖 → 前端层

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/test/app_shell_background_test.dart` — 313（读 88-145 + 枚举全部 9 条用例）
2. `shell/flutter/lib/ui/field_row.dart` — 634（读 590-634 末段）
3. `shell/flutter/lib/state/live_region.dart` — （定点 26-39）
4. `shell/flutter/lib/main.dart` — （定点 555 / 1019 / 1171-1177 / 1321）

## 跑过的命令（全部只读）
```
wc -l app_shell_background_test.dart
grep -n "test(|testWidgets(|group(" app_shell_background_test.dart
grep -n "BackgroundRuntimeScope|backgroundIndex|hydrating" app_shell_background_test.dart
sed -n '88,145p' / '175,195p' app_shell_background_test.dart
grep -n "_live\b|LiveRegionThrottle|_syncLiveRegion" main.dart
grep -n "class LiveRegionThrottle" -A 18 state/live_region.dart
grep -n "result" -A 4 ui/field_row.dart ; sed -n '594,634p' ui/field_row.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 J：**模式 B 第 6 次复查 = 未命中**（`app_shell_background_test.dart` 的 scope 探针是真身份断言）
- 维度 F（无障碍）：读屏节流真在用，1500 ms 间隔 + 可注入时钟 + dispose ✔
- 维度 B：`LiveRegion` 在 `main.dart:1019` dispose ✔
- 跨审计核验：为团队 F-0012-1（P1）补上「假成功 ⇒ 无任何输出」的复合机制

## 未核实项
1. `main.dart` 仍有约 1000 行未读（:295-460 / 750-1150 / 1182-1344）
2. `tts_section.dart:177-179` 只按 grep 确认与 llm_section 同构，**未逐行读**
3. `field_row.dart` 只读了 590-634（`FieldActionRow` 一段）；同文件其余部分（`FieldShell` 的
   `error` 参数如何渲染）未读
4. **未验证**：自检成功后服务端确实返回了可显示的文案（`TestOutcome` 的序列化形状我在
   BATCH-0011 从 Rust 侧核过：`ok/latency_ms/model_echo/note`；但「前端把哪一段拼成
   `testResult` 字符串」这一环未读）

## 本批新增
**0 条新缺陷**（净产出：关闭 B0031 的未核实项 + 模式 B 第 6 次复查未命中 +
为团队一条未修 P1 补上复合机制与精确落点）
