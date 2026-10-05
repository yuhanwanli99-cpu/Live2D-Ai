# BATCH-0186 · ⭐ **红线 M 三层闭合**：平台拆分 · 5 个使用点 · 枚举+行为+stub 的测试

Phase 1 · 域覆盖 · `shell/flutter/lib/live2d/live2d_stage.dart` + `stage_pointer_interceptor*` + 回归

## 跑的命令（全部只读）
```
grep -c "StagePointerInterceptor" lib/live2d/live2d_stage.dart
grep -nE "GestureDetector|InkWell|TextButton|IconButton|onTap|onPan" lib/live2d/live2d_stage.dart
ls test/stage_pointer_interceptor_test.dart
grep -rln "StagePointerInterceptor" lib/ test/
grep -oE "(testWidgets|test|group)\(\s*'[^']+'" test/stage_pointer_interceptor_test.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ① ⚠ **先记两条自己的 grep 失误**（都是规则 3 变体 7）
1. 第一次 `grep -oE "test\('[^']+'"` **零命中** ⇒ 误以为「文件里没有测试」——
   实际它用的是 **`testWidgets(`** 与 **`group(`** ⇒ **声明形式不同而已**
   （**今天第二次**栽在 grep 模式与实际声明形式不符）
2. `live2d_stage.dart` 里 `StagePointerInterceptor` **0 命中**、可交互控件 **0 命中**
   ⇒ **不是「漏套」，而是那个文件根本没有控件**（见 ②）

## ② ⭐ 三层核验
### 第 1 层 · **平台拆分**（正面模式 **P10 的形状**）
`stage_pointer_interceptor.dart`（API + 条件导出）· `_web.dart`（真实 DOM 垫层）· `_stub.dart`（无 DOM 时）
⇒ 组件**按平台条件实现** ⇒ `flutter test` 里是 stub ⇒ 测试**不需要 DOM 就能渲染**。

### 第 2 层 · **使用点 = 5 处**，而舞台组件自身**不需要**
`app_shell.dart` · `nav_host.dart` · `session_sheet.dart` · `stage_host.dart` · `confirm_discard_dialog.dart`
⇒ 而 `live2d_stage.dart` **零控件** ⇒ 它是**纯展示** ⇒ 「谁需要垫层」这个问题在**结构上就是清楚的**。

### 第 3 层 · **测试枚举完整 + 有行为 + 验 stub**
```
testWidgets  非 web 上垫层**完全透明**（不改尺寸、不挡内容）      ← ⭐ stub 侧也验证
testWidgets  断线横幅的「点此重试」有垫层                        ← 事故清单 ①
testWidgets  舞台右下角浮标有垫层                                ← 事故清单 ②「缩放角标」
testWidgets  loading 覆盖层有垫层                                ← ⭐ **不在事故清单里**（主动加的）
testWidgets  error 覆盖层的「重试」有垫层（**并且真的点得着**）    ← 事故清单 ③ **且带行为断言**
testWidgets  expanded 内联侧板 / medium 底部浮层 / compact 整页设置 三种宿主都有垫层   ← 事故清单 ④ **三种形态全测**
```
⇒ AGENTS.md 记的 4 个中招控件（**设置面板 · 断线横幅 · 缩放角标 · 错误重试**）**全部有对应测试**；
另**多测了 `loading` 覆盖层**与**设置面板的三种宿主形态**（事故原文是「点设置能唤醒，但**点不动、不能上下滑**」）。
⇒ ⭐ **三个层次各自到位**：
1. **枚举完整**（每个控件都有一条）
2. **不止结构、还有行为**（「**并且真的点得着**」⇒ `testWidgets` **真点一次**）
3. **降级实现也被验证**（stub 是「**不改尺寸、不挡内容**」，**不是「什么都不做」**）
   ⇒ 这是 **P10「建模优于判断」** 在**测试侧**的对应物。

## 未核实项
1. `stage_pointer_interceptor_web.dart` 与 `_stub.dart` **本体未读**（只核了文件存在与使用点）
2. `live2d_stage.dart` 其余 ~640 行未读（本批只做了红线 M 的定点核验）
3. 5 个使用点里是否**各有一处**垫层（`grep -rln` 只知「这 5 个文件含它」，未逐处核数量与位置）
4. `test/*.dart` 的**声明形式不止 `test(`** ⇒ **本审计早前对 Dart 测试的枚举可能系统性漏项**（**待全量复核**）
5. 未审 `.dart` 仍 187 个（`dev_tools_section.dart` 1874 · `tokens.dart` 928 · `message_bubble.dart` 577 …）
6. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
