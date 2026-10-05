# BATCH-0291 · ⭐ 复制按钮：实现与 B0261 的声明**一致**；而四行注释说的是「**项目明令不许**」

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `message_bubble.dart` 的复制实现（B0261 留的实现侧）

## 跑的命令（全部只读）
```
grep -nE "Clipboard|copy|复制" lib/ui/message_bubble.dart
grep -rn "class _BubbleActions" lib/ ; grep -rn "Clipboard|setData" lib/ui/*.dart
sed -n '545,562p' lib/ui/message_bubble.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **「复用令牌」的理由链 + 一条项目级禁令**
```dart
Future<void> _copy() async {
  await Clipboard.setData(ClipboardData(text: **widget.text**));      // :552  ⭐ 原文，非渲染 span
  if (!mounted) return;  setState(() => _copied = true);               // :553-554
  // 「停多久」走 `AppRhythms`（节奏），不是 `AppDurations`（过渡时长）——
  // 那 4 档的语义是**一次过渡有多快**。这里借 `interruptedHold`：
  // 它是「**瞬时状态保持多久**」的那一档，与「已复制」要的是同一个量级，
  // 而**再立一个同值令牌是本项目明确不要的**（**见该令牌的注释**）。      // :556-559
  await Future<void>.delayed(AppRhythms.interruptedHold);
  if (!mounted) return;  setState(() => _copied = false);              // :559-560
}
```
四个可核点：
1. ⭐ **复制的是 `widget.text`（原文）** ⇒ ⇒ **B0261 的声明在实现里成立**（同 B0229：显示与复制都取原文）
2. ⭐⭐ **两次 await，两次 `if (!mounted) return;`**（`:553` 与 `:559`）⇒ ⇒ **B0230 纪律第七、八处**
   ⇒ ⇒ 而**这次函数里有两次 await** ⇒ ⇒ **两次都判了** ⇒ ⇒ **教科书形态**
3. ⭐⭐⭐ 而 `:556-559` 解释了**为什么复用 `AppRhythms.interruptedHold` 而不是 `AppDurations`**：
   · 「那 4 档的语义是**一次过渡有多快**」（duration 的语义）
   · 「它是**瞬时状态保持多久**的那一档」（**这才是「已复制」要的量**）
   · ⭐ 「**再立一个同值令牌是本项目明确不要的**（**见该令牌的注释**）」
   ⇒ ⇒ **这不是「我没找到合适的」，是「项目明令不许」** ⇒ ⇒ 且**指向了那个令牌的注释**
   ⇒ ⇒ **又一次「指路而非复述」**（B0217 模板 / B0220 归属 / B0228 改动记录 / **本批**，第四次）
4. ⇒ ⇒ ⭐ **可提炼**：**令牌按「它度量什么」分类，而不是按「谁用它」分类** ——
   ⇒ **已有的档位才能被复用** ⇒ ⇒ **复用的前提是分类对**
   ⇒ ⇒ 而 `:556-557` 正是**把这个分类标准写出来**的地方（**那 4 档 = 过渡多快** vs **interruptedHold = 瞬时态留多久**）

## 未核实项
1. ⭐ `AppRhythms` / `AppDurations` 的**档位定义与其注释本体**未读
   （本批的结论依赖「那 4 档的语义是过渡时长」这句注释的**正确性** ⇒ **该核**）
2. `message_bubble.dart` 余约 460 行未读（角色标签 · 流式光标 · `onlyReasoning` 折叠区）
3. `live2d_stage.dart` 余约 600 · `shell_admin.dart` 余面 · `env_key_field.dart` 余约 110 未读
4. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
5. `main.rs` 余约 930 行未读
6. 真实动作计划的 token 长度（F-0020-01 永久敞口）
7. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
8. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
9. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
10. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
11. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
12. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
13. Mod crates 42 未读；`mod-system` 余 8 文件未读
14. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
15. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
16. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
