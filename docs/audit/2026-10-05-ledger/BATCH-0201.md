# BATCH-0201 · ⭐ **F-0199-01 是「孤例」**：另 6 个 Mod、50 处日志，**零处携带内容**

Phase 1 · 域覆盖 · Mod crates 逐个查日志内容（B0199 留的范围问题）

## 跑的命令（全部只读）
```
for m in persona memory voice-input wallpaper pet-desktop template mod-system; do grep -c "logger\." …; done
grep -rn "logger\." -A 3 mod-{persona,memory,voice-input,wallpaper,pet-desktop}/src/*.rs | grep -oE "\{(payload|text|…)\}|\{\}"
grep -rn "logger\." -A 4 <同上> | grep -oE "\b(text|payload|prompt|content|message)\b *[),}]"
sed -n '92,108p' mod-voice-input/src/commands.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**F-0199-01 的范围定为「孤例」**
| Mod | 日志处数 |
|---|---|
| `persona` | 12 |
| `memory` | 27 |
| `voice-input` | 5 |
| `wallpaper` | 3 |
| `pet-desktop` | 1 |
| `template` | 2 |
| `mod-system` | 0 |
⇒ **另 6 个 Mod 共 50 处**。

### ① 逐处查参数：**零处携带内容**
- `memory` 的具名参数全集：`{covers}` `{e}` `{id}` `{pct}` `{reason}` `{removed}` `{role}` `{session}` `{total}` `{turn}`
  ⇒ **全是非内容**（覆盖数 / 错误 / id / 百分比 / 原因 / 条数 / 角色 / 会话 / 总量 / 轮次）
- 另 45 处是 `{}`（静态文案或**已算好的值**）
⇒ ⇒ **`payload` / `text` / `prompt` / `content` 零命中**

### ② ⚠ 两处**疑似**命中**已逐一定位、均非内容**（**不凭形状下结论**）
- `voice-input/commands.rs:105` `logger.warn(&message)` ⇒ 该 `message` 是**固定状态串**：
  `:95 format!("已注入主链（backend={backend}, locale={locale}）")` / `:101` 忙碌文案
  ⇒ **不含转写文本**，且**只在被拒时**记录（`:104 if !accepted`）⇒ **刻意**
- `persona/sessions.rs:270` `path.display()` ⇒ 是**文件路径**（本机日志的常规内容）

### ③ ⇒ **本条是「一个文件里的一个站点」的孤例**
⇒ 修法的**一行**性质更明确；发现的**精确度更高**（不是「一类问题」，是「一处」）。

## 未核实项
1. `mod-director` 的**其余**日志站点未逐处查（B0200 核了 `{payload}` 那一处 ⇒ 它不记 payload）
2. `dev_tools_section.dart` 其余实现未读（1874 行）；`tokens.dart`(928) 未读
3. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 的采集点/轮转未核
4. 新露出的 208 条自我设防名只看了 11 条
5. Mod crates 逐文件覆盖率（9/61）
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
