# BATCH-0068 · NumberField 语义核实（结 B0067 未核实点）+ 静默拒绝

Phase 1 · 域覆盖 · 前端

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/ui/field_row.dart` — （定点 375-445：`NumberField` 全类）

## 跑过的命令（全部只读）
```
grep -rn "class NumberField" -A 6 shell/flutter/lib/ui/*.dart
grep -rn "min|max" field_row.dart | head -16
sed -n '385,460p' field_row.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**F-0068-01（P3）** + 结掉 B0067 的未核实点
1. **「不回调」核实为真**（field_row.dart:432-433 越界即 `return`，在 `widget.onChanged` 之前）
   ⇒ B0067 对「前端强制成立」的判读成立，注释也准确
2. **F-0068-01（P3）**：越界输入被**静默拒绝** —— `TextField` 照常显示用户输入，
   而 `_values` 仍是旧值 ⇒ **框里显示的不一定是将被保存的值**，且零反馈
   （helperText 的区间常驻，不构成「刚才被拒」的信号）
   ⇒ 与 F-0067-01 叠加出一条可达链：宿主零校验 ⇒ 直接 POST 可把越界值写上盘
   ⇒ 面板回显越界值且**不可见其为无效** ⇒ 再点「保存」会把越界值**原样再存一遍**

## 未核实项
1. `field_row` 是否有越界用例的测试 —— **未查**（F-0068-01 的反证 c）
2. `TextField` 内部 controller 是否在别处被回写（B0068 反证 a 只查了本文件）
3. `settings_controller.dart`(409) / `display_prefs.dart`(1157) 未读
4. `app/`（`shell_admin` / `shell_prefs` / `app_shell`）未读 —— **模式 G 前端侧**排 B0069
5. 其余 Mod 面板（`memory_panel` 694 等）未读

## 本批新增
P0 0 · P1 0 · P2 0 · **P3 1**
