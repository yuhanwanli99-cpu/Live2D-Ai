# BATCH-0200 · ⭐ **director 已经就是正确写法** ⇒ F-0199-01 的修法被「定死」，且我上一批的修法 ② 被降权

Phase 1 · 域覆盖 · `mod-director/src/lib.rs:706-724`（B0199 留：director 是否也整块记 payload）

## 跑的命令（全部只读）
```
grep -n "payload," -B 4 -A 2 mod-director/src/lib.rs
sed -n '706,724p' mod-director/src/lib.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**F-0199-01 的修法收紧**（观察，不是缺陷）
```dart
let len = payload.chars().count();                    // :711  ← ⭐ **先算长度**
match self.ledger.record_prompt(payload,              // :712  payload 传给**分析器**、不是日志
                                 self.config.emotion_lexicon, &self.config.presets) {
  PromptOutcome::Silent => self.services.logger.info(&format!(
      "director 收到空正文（**len={len}**），本轮静默：无决策、零副作用"));   // :719-720
```
⇒ **director 不记 payload，只记 `len={len}`** ⇒ **那正是我建议的修法 ①**

### ① ⇒ **这不是一个设计问题，是「让 `external-input` 做 director 已经做的事」**
**同仓 · 同 Mod 模式 · 两个 Mod · 相反的选择 —— 而正确的那个已经在树里。**
⇒ ⭐ 这是**我能给出的最强形式的修法建议**：**先例就在相邻文件里**，不需要引入新约定、不需要改 API。

### ② ⚠ 并据此**降权我自己的修法 ②**
我 B0199 建议「在 `mod_registry.rs:402` 的 `ModLogger` 层统一收口（只记 `topic`+`len`）」
⇒ **director 证明 Mod 自己就能选对** ⇒ 在宿主层强制 = **过度设计**
⇒ ⇒ **正确修法就是那一行**；② 降为「**可选：把这条约定写进 `ModServices` 的文档**」。

### ③ 顺带一条关于 **API 形状**的观察（**P2 私有汇合**的思路）
`ModLogger` 的签名**不区分「记一个标量」与「记一段内容」**
⇒ 而 director 的 `len=` 表明**这条约定是可学的、且已被遵守过一次**
⇒ ⇒ **不需要改 API，只需要让第二个 Mod 也照做。**
（⇒ 这也再次印证 **P2 的边界**：**该汇合的汇合，不该汇合的别硬汇** ——
B0152 的 `join_endpoint` 是「有理由的重复」，而这里是「**重复的是调用方选择、不是实现**」，
**两件事**。）

## 未核实项
1. 其余 Mod（`persona` / `memory` / `voice-input` / `wallpaper` / `template` / `pet-desktop`）的日志内容未查
2. `dev_tools_section.dart` 其余实现未读（1874 行）；`tokens.dart`(928) 未读
3. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 的采集点/轮转未核
4. 新露出的 208 条自我设防名只看了 11 条
5. Mod crates 逐文件覆盖率（9/61）
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
