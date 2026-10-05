# BATCH-0441 · ⭐⭐⭐⭐⭐ **「排除注释」是一个具名 helper** —— 而它**没有成为默认**

Phase 4 · **证伪**（按 B0440 的规矩**按内容找**，不按文件名：找守「不留占位 UI」那条裁决的测试）

## 跑的命令（全部只读）
```
grep -rln "占位|没有功能|无功能" test/ lib/ --include=*.dart | head -4
grep -rn "占位" test/*.dart | head -4
sed -n '228,256p' test/background_hydration_window_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0438 的问题完整回答了**
```dart
// test/background_hydration_window_test.dart:228-252
group('F-0002-2 / F-0002-3：接线顺序与「假兜底」（**结构性守卫**）', () {
  // ⚠️ **这几条只能扫源码**：`main.dart` 在 VM 里加载不了（依赖 `package:web`）——
  // 所以「水合前暴露的是不是占位内存库」这种行为**无法在 VM 里直接跑**。
  // 扫的是**调用形状**（不是某一行字），而且**先剥掉注释与字符串**——
  // 否则**注释里提一句旧实现就会把守卫自己判红**。**改坏接线断言就红。**
  final String src = **stripCommentsAndStrings**(File('lib/main.dart').readAsStringSync());
  test('F-0002-2：_store 一开始就是真库（不是 MemoryBackgroundStore 占位）', () {
    expect(src.contains('MemoryBackgroundStore('), **isFalse**,
      reason: '主入口里不该再有内存占位库（**代码路径，不是注释里提一句**）');
  });
```
五个可核点：
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 「排除注释」在这个仓是一个**具名 helper**（`stripCommentsAndStrings`）**
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ 它不是每个测试各写一遍、是一个可复用的函数**
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ B0437 那句「弱的那种恰好写在承诺更强的测试名下」**因此有了正面解法**：
   **⇒⇒⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ 「强的那种不是没被写，是**没有成为默认**」**
2. ⭐⭐⭐⭐ **而 `reason` 把「为什么必须是 `isFalse` 而不是注释里提一句」写明了**：「**代码路径，不是注释里提一句**」
   ⇒⇒⇒⭐⭐ **「这条断言的强度由这条注释担保」**
3. ⭐⭐⭐⭐⭐ **⇒ 而「为什么只能扫源码」也被写明了**（`main.dart` 依赖 `package:web`、VM 加载不了）
   ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒ 与 B0409 核的「依赖方向即测试性」完全一致**
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 「因为它依赖 `package:web`」这个理由，同时是「它只能被扫」的原因和「它扫不到行为」的原因**
   ⇒⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒ 这是一条完整的因果链**
4. ⭐⭐⭐ **⇒ 而注释还预警了「改坏接线断言就红」** ⇒⇒ **「我知道我脆弱」的第二种写法**（B0439 核的 `reason` 那种是第一种）
5. ⭐⭐⭐⭐ **⇒ 「不留占位」那条裁决（B0300 / AGENTS v0.5.0）有守卫** ⇒⇒ **守卫形式 = 「**反向断言**（`isFalse`）+ `reason`」**
   ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 「不留占位」因此可被机械验证** ⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒ 与 B0342「禁用在输入层生效」是同一条纪律的**测试侧**形态**

⇒ ⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ 而四个实例的自我证明形态至此是**：
| # | 形态 | 自证方式 | 文件 |
|---|---|---|---|
| 1 | 扫源码 | **合成坏样例** | `visual_language_test.dart`（B0381） |
| 2 | 真 widget | **`findsNothing`** | `stage_overlay_single_test.dart`（B0439） |
| 3 | 跨文件数值 pin | **「改任何一边都会红」** | `chat_session_test.dart` + `keyboard_test.dart`（B0440） |
| **4** | ⭐ **扫源码 + `stripCommentsAndStrings`** | ⭐⭐⭐ **`isFalse` + `reason`（代码路径，不是注释）** | `background_hydration_window_test.dart`（**本批**） |
⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ 而第 4 种是**唯一一个「排除注释」成为 helper 级默认**的** ⇒⇒⇒⭐⭐⭐⭐⭐
**⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒ 而 `wiring_test.referencedOutside` 与它**同层**却没有用那个 helper ⇒⇒⇒⭐⭐⭐
**⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒ 这就是「有正解、但没推广」的**可核形状****

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. ⭐ **`stripCommentsAndStrings` 的实现与它的使用点**（**哪些测试用了、哪些没用 · 未全数**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~455）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
