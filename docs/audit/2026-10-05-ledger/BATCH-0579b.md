# BATCH-0579b 落盘（极简）· showModalBottomSheet 在 lib/ 里零处真实调用

## 编号
`BATCH-0579.md` 已存在 ⇒ 本批落 `BATCH-0579b.md`。

## 命令（只读）
```
grep -rn "showModalBottomSheet" shell/flutter/lib/ | head -6
grep -rn "showModalBottomSheet(" shell/flutter/lib/ | head -5
```

## 结果：6 条命中**全部是注释**（行首 `///`）
```
live2d/stage_pointer_interceptor.dart:51-52  /// - `showModalBottomSheet(...)
app/app_shortcuts.dart:14                  /// 顺序是**模态优先**
app/app_shell.dart:11 / :45 / :53          /// - 设置是**叠加物或浮层**
                                            /// ✕ / Esc（`showModalBottomSheet` 另…
                                            /// `showModalBottomSheet` 自带的模态障…
app/nav_host.dart:6                        /// | 900–1279 medium | `showModalBottomSheet(is…
```
⇒ 加了左括号的 grep **仍只命中注释** ⇒ `lib/` 里没有一处真实调用。

## 三个可核点
1. **⇒⇒⇒⇒ B0578 第 ② 点的指控撤回**（本批最重要）。
   我说「`AppStore` 里的 `showModalBottomSheet` builder 只跑一次，
   『已修』与 Flutter API 的『不重跑』冲突」。
   **⇒⇒⇒⇒ 这个指控没有落点** —— 仓库里根本没有这个调用。
   ⇒⇒ ⇒⇒ 依 B0271（不存在的东西不能被指控）**撤回**。
   ⇒⇒ ⇒⇒ 只保留「**注释里仍以它为现行手段**」这一条。
2. **⇒⇒⇒⇒ 但那本身就是一条独立的漂移**：`app_shell.dart:11` 写
   「设置是**叠加物或浮层**」、`nav_host.dart:6` 断点表 medium 档写
   `showModalBottomSheet(is…` ⇒⇒ **文档说了、代码没做**
   ⇒⇒ 与「代码做了、文档过期」是**反向**的一种。严重度候选 P3（不立）。
3. **⇒⇒⇒⇒ 这是 B0440 第 8 形态（命中在注释里）的又一次**
   ⇒⇒ **判据强化**：凡要断言「某 API 在本仓被/没被用」，
   **先看命中那几行开头是不是 `//`**。

## 未核
设置面板的实际浮层手段 · `nav_host.dart:6` 断点表其余档位 ·
`app_shell.dart:11` 那句完整上下文
