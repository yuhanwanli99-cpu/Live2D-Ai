# BATCH-0083 · 三处拦截的「选留下 ⇒ 中止」分支 —— **0 条新发现**（结掉 B0082 的诚实边界）

Phase 1 · 域覆盖 · 前端 `app/`

## 读过的文件（来自 `git `ls-files` + `wc -l`）
1. `shell/flutter/lib/app/app_shell.dart` — （定点 443-458 / 547-553 / 660-675：
   `closeSettings` / `_closeSheetGuarded` / `_selectGuarded` + `_hasBackground` 注释）

## 跑过的命令（全部只读）
```
sed -n '443,458p' app_shell.dart ; sed -n '547,553p' app_shell.dart ; sed -n '660,675p' app_shell.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；三处**全部正确中止**
| 拦截点 | 「留下」时的行为 | `mounted` 守卫用在哪 | 备注 |
|---|---|---|---|
| `closeSettings`（:447-458） | `if (!leave \|\| !mounted) return;` ⇒ **不关** | `mounted`（State），await 后**再查一次** | `await` 可能活得比 widget 久 ⇒ 查两次 |
| `_closeSheetGuarded`（:547-553） | `if (ask != null && !await ask()) return;` ⇒ **不 pop** | **`sheetContext.mounted`**（不是 State） | 因为 `Navigator.of(sheetContext).pop()` 需要**那个** context 活着 ⇒ **精确区分** |
| `_selectGuarded`（:666-675） | `if (await ask()) _select(next);` ⇒ **只有离开才切** | —（`unawaited` 包异步，因为 handler 是 `void`） | **快路径**：`ask == null` 或**同分区**时直接切，**不问** ⇒ 不做无谓弹窗 |

⇒ 三处的结构**不同**（这是对的：`closeSettings` 需 `setState`、sheet 需 `Navigator.pop`、
分区切换是纯通知），但**语义完全一致**：留下 ⇒ 不动。
⇒ **B0082 留的诚实边界关闭。**

## 顺带核到一个正面样本（同一文件 :660-665）
```dart
/// 设置页的说明还写着「聊天面板会跟着透」，实际面板已经退回不透明——
/// **界面在骗人**。那份条件连同它造成的断层一起删掉了。
bool get _hasBackground => widget.prefs.hasBackgroundAt(_currentBackground);
```
⇒ 不只修行为，还**删掉会骗人的说明文字**，并把「界面在骗人」写成理由
⇒ 与本审计反复见到的诚实纪律同源（F-0074-01 的参数化测试建议、
B0081 的两条「不谎报已生效」用例，都是同一族）。

## 未核实项
1. `app_shell.dart` 其余部分未读（全文未读，仅 4 处定点）
2. `background_hydration.dart:140-255` 未读
3. `settings_controller.dart:120-322` / `:372-409` 未读
4. `shell_admin.dart` 余段 / `main.dart` 其余 ~1000 行未读
5. `appearance_background.dart`(1551) 未读；其余 Mod 面板（4 个 / 2284 行）未读

## 本批新增
**0 条**
