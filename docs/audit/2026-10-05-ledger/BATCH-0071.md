# BATCH-0071 · backgroundFingerprint 残留风险核验（结 B0070 唯一残留）

Phase 1 · 域覆盖 · 前端

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/design/background_item.dart` — （定点 466-495：图案名表 / 模数 / `backgroundFingerprint` / `backgroundIdOf`）

## 跑过的命令（全部只读）
```
grep -n "backgroundFingerprint" -A 3 -B 12 shell/flutter/lib/design/background_item.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；B0070 的唯一残留风险**归零**
- 指纹是 **djb2 mod 2^31-1 的完整 31 位值**（:483 `return h`），id 用 8 位十六进制表示
  ⇒ **无截断** ⇒ 「不同图因碰撞判等」不成立（n=100 时概率 ≈1.2e-6，且后果良性：
  被当作重复项提示，**不丢数据/不损坏/不影响渲染**）
- 三处微决策**都写了理由且正确**：模数取奇素数（33 与之互质 ⇒ 哈希不退化）·
  id 带 `bg` 前缀（日志/IndexedDB 一眼可辨）· 整个 dataURL 含 MIME 头参与（明确承认是取舍）
- 与 B0070 记的「`hashCode` 与 `==` 非对称」**不矛盾**：djb2 是非密码学哈希，
  用途是**本地身份标识**，不承担安全职责

## 未核实项
1. `app_shell.dart` 余段未读
2. `settings_controller.dart`(409) 未读
3. `appearance_background.dart`(1551) / `appearance_section.dart`(709) 未读
4. `stage_bg_resend_dedupe_test.dart` / `asset_guard_stage_attach_resend_test.dart` 未读
5. 其余 Mod 面板（`memory_panel` 694 / `voice_input_panel` 610 / `persona_panel` 608 / `director_panel` 372）未读
6. `background_hydration.dart`（`copyWith(id:)` 那条「逐图样式被静默清空」的路径）未读

## 本批新增
**0 条**
