# BATCH-0212 · ⭐ **18 个「缺失」逐条分类 ⇒ 零个是真漂移** ⇒ F-0209-01 确定为**孤例**

Phase 1 · 域覆盖 · AGENTS.md 符号 × 代码（机械差集 + **逐条分类**）

## 跑的命令（全部只读）
```
python3 - <<'PY'   # 抽 AGENTS.md 的带点标识符，逐个在代码全文里找
PY
for s in shell_wallpaper.dart wallpaper_api.dart descriptor.api_version ModServices.settings AppShellState._settingsTick; do
  grep -n "$s" AGENTS.md; done
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**F-0209-01 收缩为「孤例」**
### ① 机械差集：63 个带点标识符里 **18 个在代码里找不到**
### ② 逐条分类 ⇒ **三类，零个是真漂移**
| 类 | 数量 | 例 |
|---|---|---|
| **(a) 我的正则把「文件名」切开了** | 6 | `client.rs` / `plan.rs` / `prompt.rs` / `normalize.rs` / `gpu.rs` / `mod_registry.rs` ⇒ **文件都在**（`crates/live2d-ai-runtime/src/performance/client.rs` 等） |
| **(b) 不在本仓代码里** | 2 | `CONTRIBUTING.md` / `nightly.yml`（**别的文件**）· `Element.updateChild`（**Flutter 框架 API**） |
| **(c) ⭐ **AGENTS 自己说的是「已拆除 / 历史上补过」** | 10 | `HostChannels.trigger_action`（「**删除**」·rc.2）· `persona_card.dart`/`persona_import.dart`（「Flutter **删**」·rc.4）· `stage_css.rs`（「随之**删除**」·rc.5）· `shell_wallpaper.dart`/`wallpaper_api.dart`（:49-50「**一并拆除**」——被拆掉的只是 wallpaper Mod 的**接线**）· `descriptor.api_version`/`ModServices.settings`（:692-693 在**变更历史**里以**过去时**叙述「模板 + 门禁」「**补** `ModServices.settings`」）· `AppShellState._settingsTick`（:480 在**修复记录**里叙述「F-0005-2 重建放大链」） |
⇒ ⇒ **零个是真漂移。**

### ③ ⇒ **F-0209-01 收缩为「孤例」**
「壳画哪一张图」这一个机制在 AGENTS.md 里有三处描述、**三种都已失效**（字段 / 默认值 / getter 名），
而**其余机制零漂移** ⇒ ⇒ **不是「文档不随代码走」的一类问题，是「一个机制被替换后忘了改文档」的一处。**
⇒ **这实际上是对本仓文档质量的正面结论**：变更历史里**大量「已删除」都留了记录**（类 (c) 有 10 条！），
**没有把删掉的东西从文档里抹掉** ⇒ 与我在 B0039–B0042 / B0170 核到的「记录推翻过的方案」**同族**。

### ④ ⭐ 而方法上又得到一条（比 B0147 的「脚本先自检」更进一步）
> **脚本给出差集之后，必须逐条分类，不能直接计数。**
> 本批：脚本报「18 个缺失」⇒ 若直接计数就会记成「**18 处文档漂移**」；
> 而**逐条判性质**之后，真漂移是 **0**，且**顺带确认了 10 条「已删除的记录」被正确保留**。
⇒ ⇒ 与 B0147「**脚本输出与既有认知冲突时先怀疑脚本**」同族；
**再加一条**：**输出与认知一致时，也要问「这些差集项是同一种东西吗」**。

## 未核实项
1. AGENTS.md 里**叙述性正文**（非变更历史）的其余机制未逐个核（我只做了符号级差集，
   **符号存在 ≠ 描述正确** ⇒ 例如「默认值」「行为约束」这类**无符号名**的描述差集查不到）
2. `mod-wallpaper/src/strategy.rs`(655) 未读；`mod-template` 未读（Mod crates 逐文件 10/61）
3. `design_tokens_test.dart` 余 ~32 条断言体 · `tokens.dart` 其余 ~600 行未读
4. `dev_tools_section.dart` 余面未读（1874 行）
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
