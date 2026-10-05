# BATCH-0424 · ⭐⭐⭐⭐ **一个还没发生的错误码，被提前写进了提示**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— **`dev_mode` 的门控**（B0000 ⑭ 那个缺陷的镜像面）

## 跑的命令（全部只读）
```
grep -rn "devMode" lib/app/shell_admin.dart lib/app/app_shell.dart
grep -rn "devMode" lib/ --include=*.dart | grep -vE "app_shell.dart|shell_admin.dart"
grep -rn "widget.devMode|devMode ?" lib/settings/ --include=*.dart
sed -n '246,256p' lib/settings/sections/dev_tools_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**门控正确 + 一条加强版的 P31**（0 条新发现）
```dart
if (devMode)
  const Padding(
    padding: EdgeInsets.only(top: Space.s3),
    child: Text('**开发者提示**：导入只接受 assets/models/ 下**已存在的目录名**；'
                '删除激活中的模型会被服务端拒绝（**409 model_active**）。'),
  ),                                                    // :249-256
```
四个可核点：
1. ⭐⭐⭐⭐ **「关掉 `dev_mode` ⇒ 这一块整块不出现」**（不是 disabled、不是折叠）
   ⇒⇒ **同 B0326 的可空回调族 / B0394 的 `if (!librarySource)` 族**
   ⇒⇒⇒⭐⭐ **⇒ 而 B0341 核的 `_ListenButton` 在 unsupported 时是「**禁用并说明**」——
   这里**直接不出现**** ⇒⇒⇒⭐ **⇒⇒ 两种做法、两种理由**：
   **这一块只对开发者有意义 ⇒ 不给非开发者看一条看不懂的约束**
2. ⭐⭐⭐⭐⭐ **而这段提示本身是两条 P31**：
   - 「导入只接受 `assets/models/` 下**已存在的目录名**」⇒⇒ **说清了「你不需要上传，你需要先把它放好」**
     ⇒⇒⇒ **⇒ 这正是 B0372 核的「用户把模型放进目录」那句的**UI 侧说法****
   - 「删除激活中的模型会被服务端拒绝（**409 model_active**」）⇒⇒⇒⭐⭐
     **⇒ 提前给出「会被拒绝」+ 带码**，而 **B0252 核过服务端 409 确实会发生**
     ⇒⇒⇒⇒⇒ **⇒⇒ ⇒ 连还没发生的码都先给**
3. ⭐⭐ **而它是 `const Padding`** ⇒⇒ **一段没有交互的说明** ⇒⇒ **⇒ 而 B0244 的 P28「不假装可用」在这里的对应形态是「不假装这段提示是按钮」**
4. ⭐⭐⭐⭐ **而 B0000 ⑭ 那个「`dev_mode` 开关被自己所在的分区藏起来」的缺陷** ⇒⇒
   **⇒ 这一批的形状是它的**镜像**：**这一块的存在被 `dev_mode` **正确地**门控着**
   ⇒⇒⇒⭐⭐ **⇒⇒ 而「正确门控」与「错误隐藏」的区别在于：门控的那一块**本来就该只对开发者可见****

⇒ ⇒⭐⭐⭐⭐ **而本批最值钱的是第 2 点**
> **一个还没发生的错误码，被提前写进了提示** ⇒⇒⇒⭐⭐
> **⇒ 而 B0159 那条纪律是「错误发生后，拿界面上的码去日志里搜」**
> **⇒⇒⇒ ⇒ 这一处是它的**加强版：码在用户撞上它之前就给了**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `shell_admin.dart:18-24` 里 `_devMode` 的**变更通知**（`notifyListeners` 之后设置面板会不会重建 · **未读**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
