# BATCH-0074 · `DisplayPrefs.==` 的测试覆盖（测试层选择）+ 第三次自我更正

Phase 1 · 域覆盖 · 前端

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/display_prefs.dart` — 1157（**定点 988-1015**：`hashCode` 第二组 + `toString`）
2. `shell/flutter/test/background_store_test.dart` — （定点 57-58：唯一碰到 `BackgroundImage ==` 的断言）
3. `shell/flutter/lib/design/background_item.dart` — （B0073 已核）

## 跑过的命令（全部只读）
```
grep -rn "backgrounds" shell/flutter/test/*.dart | grep -E "==|!=|sameAs|expect"
grep -rn "opacity" shell/flutter/test/background_logic_test.dart shell/flutter/test/background_copy_test.dart
grep -rn "prefs ==|prefs !=|== prefs|equals(prefs)|isNot(equals" shell/flutter/test/*.dart
grep -rn "sameAs" shell/flutter/test/*.dart
sed -n '988,1015p' display_prefs.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0074-01（P2）** + 两处自我更正
1. **F-0074-01（P2）**：全应用的变更检测总闸 `DisplayPrefs.==`（`shell_prefs.dart:146`
   `if (next == widget.prefs) return;`，**所有**偏好变更都过这道闸）**零测试覆盖**；
   它委托的 `sameAs` **也零测试**；唯一相邻的 `BackgroundImage ==` 断言
   （`background_store_test.dart:58`）两侧都用**默认样式** ⇒ `==` 丢掉三个样式字段时它照样绿
   ⇒ **对��轴结构上无法失败**。模式 H 教科书实例。
2. **更正一**：我 B0070 那条备忘「`hashCode` 完全不含 `backgrounds`」**是错的** —— `:1003` 有
   `...backgrounds`（我当时只读到 :994）
3. **更正二**：我以为「`==` 与 `hashCode` 两份手写清单已漂」——**没有**，两者覆盖同样 24 个字段
   ⇒ 该风险从「已漂」降为「靠人守」

## ⭐ 模式 L 执行规则 3（本批提炼）
**「没看到」≠「没有」**：对「一份字段清单 / 一组分支 / 一个函数」的**完整性**断言，
必须读到它的**结尾标记**（`}` / `];` / 文件末）。本审计三处同类错误全在这一区域
（B0070 两次 + 本批一次），与「必须核发送前还有没有一层」同族。

## 未核实项
1. `background_logic_test.dart` / `background_copy_test.dart` 未读（只 grep 了 opacity 的判等）
2. `background_hydration.dart:140-255` 未读
3. `settings_controller.dart`(409) / `app_shell.dart` 余段未读
4. `appearance_background.dart`(1551) 未读（只核过 :1264）
5. 其余 Mod 面板（4 个 / 2284 行）未读
6. `DisplayPrefs.copyWith` 家族（B0006 核过 `copyWith(id:)`；其余 20+ 字段的 copyWith 未核）

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0 ｜ 另：2 处自我更正 + 模式 L 执行规则 3
