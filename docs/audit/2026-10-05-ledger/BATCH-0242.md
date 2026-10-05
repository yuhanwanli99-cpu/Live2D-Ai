# BATCH-0242 · `ErrorAction`：**上界越界意味着「你这里错了」而不是「丢掉」**（0 条新发现）

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `lib/ui/error_banner.dart` 余段（`ErrorAction`）

## 跑的命令（全部只读）
```
sed -n '19,48p' lib/ui/error_banner.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ 一个新的「上界」家族成员
```dart
class ErrorAction {                                    // :23-27
  const ErrorAction({required this.label, required this.onPressed});
  final String label;  final VoidCallback onPressed;
}
…
/// 建议动作（**0–2 个**）。**超过 2 个说明这条错误该拆。** */      // :44-45
final List<ErrorAction> actions;
```
四个可核点：
1. **一个动作 = 标签 + 回调**（极小的值类型）⇒ ⇒ 造一个动作**成本极低**
   ⇒ ⇒ 与 B0227 的「**失败要有「下一步」**」**一致**：既然便宜，就没有不提供的理由
2. ⭐ **数量有上界 0–2，而越界的含义是「**你这里错了**」**：「超过 2 个说明**这条错误该拆**」
   ⇒ ⇒ **这不是 UI 裁剪，是一条诊断信号**
3. ⭐ 它写在**字段的文档上** ⇒ ⇒ **每个调用点的读者都会看到**
4. `onDismiss` 可空且**空态有定义**：「`null` 时**不显示**关闭按钮」⇒ ⇒ 缺席是定义、不是意外
   ⇒ 且整个组件是 `StatelessWidget` + `const` 构造 ⇒ ⇒ **极易测**

### ⭐ 由此把「上界」分成两族（本审计此前没分开过）
| 族 | 越界意味着 | 例子 |
|---|---|---|
| **截断族** | **丢掉多出来的** | `MAX_SEGMENTS` / `MAX_SPEAK_CHARS`（B0127）· `MAX_RECORDS`（B0117）· `kObserverBufferCapacity`（B0240） |
| **信号族**（本批） | **「你这里错了」** | `actions` 0–2（:44-45）⇒ 「**该拆**」 |
⇒ ⇒ **信号族用注释、截断族用代码** —— 因为前者是**给人读的**（设计自检），后者是**给机器拦的**（资源边界）
⇒ ⇒ 而 B0117 的「登记表长度与预期一致」、B0207 的 `fieldCount` 对账又属**第三族**：
**把「新增一个成员」变成必然失败**（P24）⇒ ⇒ **三族各有其写法，混用就会失效。**

## 未核实项
1. `chat_panel.dart`(540) · `persona_panel.dart`(~560) · `memory_panel.dart`(~590) ·
   `message_bubble.dart`(~490) · `live2d_stage.dart`(~620) · `director_observer_section.dart`(~600) 未读
2. `error_banner.dart` 余 ~15 行（`InlineNotice` 对接与 danger 档固定）未读
3. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
4. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
5. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
6. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
