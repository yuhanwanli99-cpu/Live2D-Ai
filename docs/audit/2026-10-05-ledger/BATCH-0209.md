# BATCH-0209 · ⭐ **F-0209-01（P3）**：AGENTS.md 落后代码 13 天，且落在**用户可见的默认值**上

Phase 1 · 域覆盖 · Mod crates 逐文件（`wallpaper`）+ 交叉核 `DisplayPrefs`

## 跑的命令（全部只读）
```
wc -l crates/live2d-ai-mod-wallpaper/src/*.rs
grep -rn "sync|同步|默认" mod-wallpaper/src/lib.rs
grep -n "default|…follow_stage" mod-wallpaper/src/lib.rs
grep -rn "syncShellStageBg" shell/flutter/lib/settings/display_prefs.dart
sed -n '634,665p' display_prefs.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0209-01（P3）**（新）；`wallpaper` 侧 **0 条新发现**
### ① ⭐ 漂移：代码已在 **2026-09-27 换掉** `syncShellStageBg`，AGENTS.md 仍写它是现行机制
- 代码 `display_prefs.dart:152`：「**为什么换掉 `syncShellStageBg`**（2026-09-27 **修一个真缺陷**）」
- 新机制是 **`backgroundSource` 枚举**（`backgroundSourceStageImage` / `backgroundSourceLibrary`），
  旧键**只作迁移输入**（`:654` `legacySync = _readBool(json['syncShellStageBg'], false)`）
- **新默认是「背景库」**（`:642-643`「没有舞台图 → 迁到『背景库』，**也就是新默认**」，
  而「绝大多数用户」落在这一支）
- 而 **AGENTS.md §0.1.0-rc.5 仍写**：「`syncShellStageBg`（**默认 `true`**）……`effectiveShellImage` 是一份真相」
⇒ ⇒ **照 AGENTS.md 理解的人会以为「壳默认跟随舞台」**；实际是「**壳默认用背景库**」
⇒ **时间可核**：AGENTS.md 变更历史**最后一条是 2026-09-14** ⇒ **没有任何更新条目能覆盖 09-27**
⇒ ⇒ **漂移确认**（不是「另有新章节」）

### ② ⚠ 而这是**实现很好、文档落后**（故定 P3 而非 P2）
迁移**写得很好**，两处：
- `:645-646` 把**两种做法的差别**写明：写成「读旧键」而**不是**「改旧键默认值」，
  理由是「改默认值会让『**显式设过 true**』和『**从没设过**』分不开，**而那正是本缺陷的成因**」
  ⇒ **把原缺陷的机制写在了代码里**
- `:661-665` `effectiveBackground`「**判据只有这一处**……判据散到两处就会出现
  『**设置说用背景库、画的不是背景库**』这类不一致」⇒ **正面模式 P2** 又一例
⇒ **代码里每一处都写明了**（:152 记替换与日期 · :636-658 记迁移规则与理由）⇒ **漂移只在 AGENTS.md**
⇒ ⇒ **不是 P2**（那是「代码里缺解释」）；**是 P3**（读者是 agent 与新维护者 ⇒ 影响是**理解偏差**，
不是行为错误）

### ③ 顺带：`wallpaper` 侧 0 条新发现，且它的口径与前端**对齐且写明**
- `mod-wallpaper/src/lib.rs:212`：「壁纸策略：壳跟随舞台（**等价 `DisplayPrefs.syncShellStageBg = true`**）」
  ⇒ ⭐ **Mod 在决策点直接点名等价的前端字段** ⇒ **P13 同族**（同一条规则、两层实现、一处说明）
- `:15`「本 Mod 只回答『**何时换壁纸 / 要不要和舞台同步**』，**不画图**」⇒ **决策与渲染分离**
- `:110` `default: Some("off")` ⇒ **Mod 缺省不接管** ⇒ 与前端的新默认**不冲突**（各管各的）
- ⇒ **本条的根因不在 Mod**，在 AGENTS.md

## 未核实项
1. `mod-wallpaper/src/strategy.rs`(655) 未读（本批只读 `lib.rs` 的口径与默认）
2. `mod-template` 未读（B0110 起挂着）
3. Mod crates 逐文件覆盖率（10/61）
4. `design_tokens_test.dart` 余 ~32 条断言体 · `tokens.dart` 其余 ~600 行未读
5. `dev_tools_section.dart` 余面未读（1874 行）
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
