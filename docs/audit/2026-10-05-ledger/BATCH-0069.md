# BATCH-0069 · 模式 G 前端侧 —— ⚠ **撤回 F-0030-01（P2）**

Phase 1 · 域覆盖 · 前端 `app/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/app/shell_prefs.dart` — 500（**定点 100-154**：`_applyPrefs` 的三通道下发 + `_updatePrefs`）
2. `shell/flutter/lib/live2d/action_scales_sync.dart` — （定点 15-42 头注 + :53 + :136：**防抖层**）
3. `shell/flutter/test/action_scales_preview_test.dart` — （**枚举全部 11 条用例名** + 精读 :142/:162/:186 三条）

## 跑过的命令（全部只读）
```
sed -n '100,154p' shell_prefs.dart
grep -n "active" -A 12 shell/flutter/lib/live2d/action_scales_sync.dart
git ls-files 'shell/flutter/test' | grep -i "action_scale|stage"
grep -n "test('|test(\"" shell/flutter/test/action_scales_preview_test.dart
grep -n "debounce|Debounce|_pending|Timer|kDebounce" shell/flutter/lib/live2d/action_scales_sync.dart
sed -n '/^### F-0030-01/,/^---$/p' AUDIT-REPO/FINDINGS.md     # 重读自己的原文
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**撤回 F-0030-01（P2）**（本审计最重的一次自我证伪）
三条子主张的下场：
| 我 B0030 的主张 | HEAD 实况 | 结论 |
|---|---|---|
| 「滑杆每像素一次」的症状仍在 | `action_scales_sync.dart:53/136` 有 **150ms 防抖** ⇒ 连续拖动 ≤7 帧/秒；回归 `preview_test.dart:142`「防抖窗口内连拖多次：只发最后一帧」 | **假**（我漏了防抖层） |
| 第三条通道「无去重」是遗漏 | `:186` `test('force 绕过去重：iframe 重建后必须重发')` —— 该行为**被命名并钉住**；另有 `:162` 证明非 force 路径会去重 | **不是遗漏** |
| 「文档用途与实际用途不符」 | `shell_prefs.dart:129-131` 与 `action_scales_sync.dart:33-35` **两处**都写明「改主题 / 调音量等任意 `DisplayPrefs` 变更都会走」并给出理由 | **HEAD 已不成立** |
⇒ **F-0030-01 撤回**；**模式 I 回到 1 个样本**（F-0028-01 解码缓存容量）。

## 模式 L 执行的补充规则（本批教训）
**追链路必须核「发送前还有没有一层」**。任何「每 X 一次」的性能主张，
都须先确认 X 与发送之间**没有防抖 / 节流 / 合帧**。
知识资产会随时间失效，**必须每轮重新核，不能靠「我上轮核过」** ——
B0023（记得 33ms 二次节流 ⇒ 没写错）与本批（忘了 150ms 防抖 ⇒ 写错了）互为镜像。

## 未核实项
1. `app_shell.dart`（F-0005-2 在其中，B0032 已定点核过两处）其余部分未读
2. `display_prefs.dart:970` 的 **id-only `sameAs`**（F-0034-01 潜在陷阱）未核 ——
   **本批的教训要求重新核，不靠旧结论**
3. `stage_bg_resend_dedupe_test.dart` / `asset_guard_stage_attach_resend_test.dart` 未读
4. `settings_controller.dart`(409) / `appearance_*.dart` 未读
5. 其余 Mod 面板（`memory_panel` 694 等）未读

## 本批新增
**0 条新发现** ｜ **撤回 1 条既有 P2**（F-0030-01）｜ 模式 I 样本 2 → 1
