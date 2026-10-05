# BATCH-0199 · ⭐ **F-0199-01（P2）**：Mod 把 `TextDelta` 的 payload **整块**写进日志

Phase 1 · 域覆盖 · Mod crates 逐个查日志内容（B0198 留的点）

## 跑的命令（全部只读）
```
grep -rn "logger\.|log::" crates/live2d-ai-mod-*/src/*.rs
sed -n '244,250p' mod-external-input/src/lib.rs
grep -rn "logger\.(info|warn|error)(&format!" -A 3 mod-director/src/lib.rs | grep -oE "\{[a-z_.]+\}"
sed -n '228,248p' mod-external-input/src/lib.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0199-01（P2）** —— **本审计第一条「内容面」发现**（此前全是密钥面）
### ① 四跳链路，**每跳都核到行**
| # | 位置 | 发生了什么 |
|---|---|---|
| 1 | `external-input/src/lib.rs:247` | **`TextDelta` 的 payload（= 助手正文块）整块写进 Mod 日志**（而它只订阅 `TurnStarted`/`TextDelta`，:234-235） |
| 2 | `mod_registry.rs:402` | `ModLogger::new(\|_lvl,msg\| tracing::info!("{}",msg))` ⇒ **透传进宿主日志文件**（B0198） |
| 3 | `log_routes.rs:86` | `read_tail_lines(...)` ⇒ **逐字转发**给前端（B0198） |
| 4 | `dev_tools_section.dart:1139-1165` | 诊断面板「**日志**」区**显示**（B0197） |

### ② 而本条的核心是：**同一仓库内两种口径**
- **宿主侧刻意不记内容**：`dispatch.rs:250/252/254` 只有 `%method, path, route, status`
  ⇒ 合 AGENTS 明文「**不记录请求体**（含用户提示词等内容）**」**（B0198 已核 69 处）
- **Mod 侧整块记内容** ⇒ 且宿主**透传** ⇒ **Mod 绕过了一条宿主已立的规矩**

### ③ ⚠ 而**我先猜错了，已改正**
我第一反应是「payload 是**用户弹幕文本**」⇒ ❌ **自己推翻**：
该 Mod 只订阅 `TurnStarted`/`TextDelta` ⇒ payload 是**助手正文块**；
**弹幕走 `POST /api/v1/external/chat` 端点，不作为 Mod 事件 payload**
⇒ ⇒ **用户输入不经过这条路** ⇒ 这是把它定 P2 而非 P1 的理由之一。
**⇒ 这已是本审计第 N 次「自己的第一反应被源码推翻」**（B0104「唯一」、B0186 grep 零命中、B0196 猜名…）

### ④ 另一条自我修正：**泄漏面只有「显示」，没有「剪贴板」**
我先以为「日志会经复制快照出去」⇒ ❌ **不成立**：`_snapshotLines()` **只含标量**（B0197 已核），
**日志不在快照里** ⇒ ⇒ **泄漏面只有诊断面板的显示区**。

### ⑤ 定 P2 的理由
可核（4 跳全有 quote）· 违反的是**本仓自己写的规矩**（不是外部标准）· 后果限于**本机显示**
**不是 P1**：无远程面 · 无密钥 · **无用户输入** · 不跨用户。
**修法**：① `:247` 改记 `payload.chars().count()`（事件**类型与规模**才是排障所需）
② 更根本：`mod_registry.rs:402` 的 `ModLogger` 提供**只记 `topic`+`len`** 的通道，
或**明确文档化「Mod 日志原样进宿主日志」这条代价** ⇒ **取舍要被写下来**
③ 若保留正文 ⇒ 至少考虑给日志文件与 `.env` 同样的 `0600`（`secrets.rs` 已有 chmod 先于 rename 的先例，B0120）

## 未核实项
1. ⭐ **其余 Mod 的日志内容未逐个查** —— `director` 的 `format!` 参数含 **`{payload}`**
   （另有 `{note}`/`{e}`/`{len}`/`{epoch}`/`{cleared}`/`{pitch}`/`{speed}`）
   ⇒ **同一形态可能在 director 侧也存在** ⇒ **B0200 第一件事**
2. `dev_tools_section.dart` 其余实现未读（1874 行）；`tokens.dart`(928) 未读
3. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 的采集点/轮转未核
4. 新露出的 208 条自我设防名只看了 11 条
5. Mod crates 逐文件覆盖率（8/61 → 本批 +1）
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
