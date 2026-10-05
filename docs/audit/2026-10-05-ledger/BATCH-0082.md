# BATCH-0082 · 「三处拦截」是否真的三处都接上（模式 H 变体）—— **0 条新发现**

Phase 1 · 域覆盖 · 前端 `app/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/app/shell_settings.dart` — 437（定点 168-207：`_clearModelOverride` 尾 + **`_confirmDiscard`**）
2. `shell/flutter/lib/app/app_shell.dart` — （定点 138 / 314 / 443-449 / 545 / 664：`confirmDiscard` 字段与**三处使用**）
3. `shell/flutter/lib/main.dart` — （定点 1301：注入点）

## 跑过的命令（全部只读）
```
grep -rn "confirmLeave" shell/flutter/lib/ shell/flutter/test/
sed -n '168,207p' shell_settings.dart
grep -rn "_confirmDiscard" shell/flutter/lib/ shell/flutter/test/
grep -rn "confirmDiscard" shell/flutter/lib/
```
未跑任何 cargo / flutter / trunk / pnpm 命令。**三形态 grep**（`name(` / tear-off / 回调字段）全部覆盖。

## 本批产出：**0 条新发现**；「三处拦截」**逐段核实为真**
### 完整链路（四段，每段都核到行）
```
SettingsController.confirmLeave(ask)          settings_controller.dart:335   ← 逻辑：dirty 闸 + 放弃即清草稿
        ↑ 被调
shell_settings._confirmDiscard()              shell_settings.dart:182-198   ← 弹窗 + 防重入
        ↑ 作为回调字段注入
main.dart:1301   confirmDiscard: _confirmDiscard                        ← 组合根
        ↓ 落到
AppShell.confirmDiscard (字段, :138/:314)      app_shell.dart               ← 可空
        ↓ 三处使用
app_shell.dart :449 · :545 · :664                                       ← **恰好三处**
```
⇒ `shell_settings.dart:177` 点名的三处（换分区 / 关设置 / 关浮层）**与代码里的三处使用**对得上。

### 三处可核的细节
1. **可空字段 + 有记录的降级**（`app_shell.dart:445`）：
   「`confirmDiscard` 为空（测试 / 未注入宿主）时**保持原来的同步行为**」
   ⇒ 缺注入**不崩**、也不静默变成「不拦」，而是**保持旧行为**并写明
2. **防重入有理由**（`shell_settings.dart:184-186`）：
   「防重入：Esc 连按 / 同时从两个入口进来时，**不叠第二个弹窗**」——用 `_confirmingLeave` 布尔挡，
   且用 `try/finally` 保证异常路径也复位
3. ⭐ **这「三处」曾经就是不三处**（`app_shell.dart:443-445`，P0 / 2026-09-20）：
   「**脏草稿要走同一套 `confirmDiscard`**（P0，2026-09-20）：过去这里直接……」
   ⇒ 「三处统一」是**修过的真实缺陷**，不是一开始就对的设计；当前接线**可证**它已修好

## 诚实的边界
- 我核的是「**三处都接上了统一入口**」；**未核**每处「用户选留下」时是否**正确取消导航**
  （那要看 :449/:545/:664 各自的 `if (!await ask()) return;` 之后有没有真的中止动作）——
  **未核实**，已记 STATE
- `app_shell.dart` 全文未读

## 未核实项
1. `app_shell.dart` :449/:545/:664 三处**各自的「留下 ⇒ 中止」分支**未读
2. `settings_controller.dart:120-322` / `:372-409` 未读
3. `background_hydration.dart:140-255` 未读
4. `shell_admin.dart` 余段 / `main.dart` 其余 ~1000 行未读
5. `appearance_background.dart`(1551) 未读；其余 Mod 面板（4 个 / 2284 行）未读

## 本批新增
**0 条**（模式 H 变体「入口存在 ≠ 三处都接上」**不成立**）
