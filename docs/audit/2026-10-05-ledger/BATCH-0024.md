# BATCH-0024 · 舞台保活（红线 N）+ 断点布局测试面

Phase 1 · 域覆盖 → 前端层（`shell/flutter/**`）

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/live2d/live2d_stage.dart` — 666（读 60-189）
2. `shell/flutter/lib/app/app_shell.dart` — 998（读 680-739；接线片段已在 B0023 读过）
3. `shell/flutter/lib/app/page_cross_fade.dart` — （读 100-150）
4. `shell/flutter/test/app_shell_layout_test.dart` — （枚举全部用例名 + 读 179-420 结构）
5. `shell/flutter/test/settings_panel_keepalive_test.dart` — （读 1-55）
6. `shell/flutter/test/compact_settings_page_test.dart` / `stage_bg_resend_dedupe_test.dart` — （定点读）

## 跑过的命令（全部只读）
```
sed -n '100,150p' shell/flutter/lib/app/page_cross_fade.dart
grep -n "StageHost|widget.stage" shell/flutter/lib/app/app_shell.dart
sed -n '680,739p' shell/flutter/lib/app/app_shell.dart
grep -rln "setSurfaceSize|sizeClass|compact" shell/flutter/test/
grep -rn "stageKey|GlobalKey<Live2DStageState>|currentState|重建|保活" shell/flutter/test/
grep -n "test(|testWidgets(|group(" shell/flutter/test/app_shell_layout_test.dart
sed -n '20,55p' shell/flutter/test/settings_panel_keepalive_test.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 J：红线 N 核心不变量零测试 → F-0024-01；并**命名了新模式 H**
- 维度 B/C：舞台保活的两个机制都读过——`PageCrossFade` 用 `Offstage` 保留隐藏页的
  Element/RenderObject（page_cross_fade.dart:139-141 注释明写「不 paint，但 Element/RenderObject 都在」，
  且给出两条偏离参考实现的理由：保活 + 避免切换帧卡顿）；`Flex` 三断点同形状（app_shell.dart:702-711）
- 维度 D：`ActionCueStore.replace`（live2d_stage.dart:63-69）用 map 语义天然吃掉同 seq 重复帧，
  `take` 后即移除 ⇒ 「每条 cue 恰好应用一次」（:71-85）✔

## 未核实项
1. `live2d_stage.dart:190-666` 未读（`_attach` / retry / setMouth / sync 的实现）
2. `render_events.dart`(700) 本批未读（排在下一批）
3. **模式 B 在 Dart 测试文件的首轮复查仍未开始**——本批只读了 3 个测试文件的局部。
   `app_shell_layout_test.dart` 的 20 个用例名已枚举，**但断言体未逐行读**。
4. 红线 K/L（离线优先 / 字体子集）两批都未查
5. `settings_panel_keepalive_test.dart` 只读了 1-55（替身定义），其断言体未读

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0 ｜ 另：**红线 N 验证通过**（实现正确）＋ **模式 H 命名**
