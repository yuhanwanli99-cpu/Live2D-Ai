# BATCH-0193 · ⭐ `:108` **不是按钮，是「保存 ⇒ 回读」** ⇒ 主路径**比我上一批说的更现实**

Phase 1 · 域覆盖 · `shell_settings.dart:99-112`（F-0189-01 主路径的现实性）

## 跑的命令（全部只读）
```
sed -n '100,116p' lib/app/shell_settings.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**F-0189-01 的主路径现实性上调**（不是下调）
```dart
} on ApiException catch (e) {                     // :100  先 PATCH 覆盖
  if (!mounted) return;
  _modelOverrideMessage = '本模型覆盖保存失败：${e.message}';
  …
}
if (!mounted) return;
await _settings.load();                           // :108  ← **不是按钮：这是「保存 ⇒ 回读」的尾巴**
if (!mounted) return;
```
⇒ `:108` 是**「保存本模型覆盖」之后的整份回读**，**不是**用户点的刷新按钮
⇒ ⭐ 而这让**重叠更容易发生**，因为触发不需要「连点两次」：
| 时间线 | 事件 |
|---|---|
| t0 | 打开设置面板 ⇒ `:18` 的 **init `load()`（GET#1）** 发出 |
| t1 | 用户**很快**改了「本模型覆盖」并保存 ⇒ PATCH |
| t2 | `:108` 的 **`load()`（GET#2）** 发出 |
| t3 | **GET#2 先回**（更新）· **GET#1 后回**（旧值）⇒ **覆盖 + 草稿被清空** |
⇒ 「**打开设置后立刻改东西**」是**正常操作**，不需要任何异常行为
⇒ 而且 `:108` 的模式是「**每次保存覆盖都触发一次整份回读**」⇒ **编辑越多、GET 越多、重叠机会越多**

### ⭐ 而同一处的 `mounted` 守卫说明**异步纪律存在、只是漏了 staleness**
`:99` `:107` `:109` 三处 `if (!mounted) return;` ⇒ **他们想到并处理了「await 之后组件已卸载」**
⇒ ⇒ **缺的不是异步意识，而是「两次异步的结果谁作数」这一个判据**
⇒ 这让修复的定位更清楚：**不需要引入取消机制**（他们已有的心智模型里没有这个），
**只需要一个自增序号**（与 B0040 的 `epoch` 闸同形，仓里已有先例）。

## 未核实项
1. `shell_settings.dart:18` 那处 init `load()` 的确切位置与时序（**未读上下文**）⇒ 上表 t0 一步是**推断**
2. `_guard()` 实现未读（超时/重试/去重）
3. `settings_controller.dart:120-222` / `:296-409` 其余未读；`:327` 那处置 `_draft` 未读
4. 新露出的 208 条自我设防名只看了 11 条；`dev_tools_section.dart`(1874) / `tokens.dart`(928) 未读
5. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
