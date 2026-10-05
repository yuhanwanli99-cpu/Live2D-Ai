# BATCH-0080 · `settings_controller.dart` —— 职责澄清 + 红线 6 前端侧核验

Phase 1 · 域覆盖 · 前端 `settings/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/settings_controller.dart` — 409（**读 1-40 头注全段** + 定点 167-214：
   `SaveOutcome` 五结局 / `messageWith` / `isSuccess`）
2. `shell/flutter/lib/api/settings_models.dart` — （定点 35-78：`Tri` 三态族与序列化规则）

## 跑过的命令（全部只读）
```
sed -n '1,40p' settings_controller.dart
grep -n "class Tri|TriKeep|keep()|isDirty|save|patch(" settings_controller.dart
sed -n '198,214p' settings_controller.dart
grep -rn "class Tri|TriKeep|isDirty" shell/flutter/lib/api/settings_models.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**（四个正面核验）
1. **职责澄清**（解答 B0079 的疑问）：它是**服务端 settings 的草稿控制器**（LLM/TTS 走 PATCH），
   **不是**本地 `DisplayPrefs` 的落盘链 ⇒ B0079 的 `onPrefsChanged` 零命中**不是缺口，是职责不同**
2. **`Tri` 三态用 `sealed`**（`settings_models.dart:35/49/56/63`）⇒ **编译器穷尽三态**，
   新增变体必须补全所有 `switch`；且 `:72-78` 写明「`null` 与 `TriKeep` 都表示『不修改』→ **不写键**」
   ⇒ 与 Rust 侧 `PatchBody` 的 `Option<Option<T>>`（B0010 已核）+ `settings_routes/tests.rs:434`
   的端到端断言构成**同一契约的两端** ✔
3. ⭐ **红线 6 的前端半边成立**（`settings_controller.dart:207-212`）：
   ```dart
   /// 2026-09-11 修：`SnackBar` 过去只显示 [message]，于是任何失败都塌缩成一句「保存失败」——
   /// 用户的原话是「**后端出错无具体错误代码**」（PATCH 的 400/500 响应体里其实一直带着
   /// `code` + `message`，只是没人把它送上去）。
   String messageWith({String? error}) {
     if (this != SaveOutcome.failed) return message;
     final detail = error?.trim() ?? '';
     return detail.isEmpty ? message : '$message：$detail';
   }
   ```
   ⇒ 失败时**把后端带来的码与文案原样上屏**，不塌缩；且注释用**用户原话**作缺陷证据
4. **`SaveOutcome` 有 5 个结局**（`savedApplied` / `savedRestartRequired` / `savedQueued` /
   `savedNoSupervisor` / `failed`）+ `isSuccess` **只认 4 个绿**（`noChange` 与 `failed` 都不算）
   ⇒ 诚实区分「已保存需重启」「已保存排队中」「已保存但下次启动生效」，
   **不把非即时的成功说成即时**（与 B0030 核的 `shell_admin.dart:237-239` 同一纪律）

## 未核实项
1. `settings_controller.dart:40-167` 未读（三处拦截的实现本体 —— 头注承诺的
   「**三处拦截**」，**本批只核了头注，没核实现**；按模式 H 应问「三处拦截有没有测试」）
2. `settings_controller.dart:214-409` 未读
3. `shell_admin.dart` 余段未读；`main.dart` 其余 ~1000 行未读
4. `background_hydration.dart:140-255` 未读
5. `appearance_background.dart`(1551) 未读
6. 其余 Mod 面板（4 个 / 2284 行）未读

## 本批新增
**0 条**
