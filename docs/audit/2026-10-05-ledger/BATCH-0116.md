# BATCH-0116 · `DISCIPLINE_TEMPLATE`：**无 marker**（结 B0115 的点）+ 一处**跨层设计**的发现

Phase 1 · 域覆盖 · Mod 根第 13 批（收尾）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-persona/src/lib.rs` — 1004（定点 175-187：`DISCIPLINE_TEMPLATE` + `PersonaCard`）

## 跑过的命令（全部只读）
```
grep -rn "DISCIPLINE_TEMPLATE" -A 12 mod-persona/src/lib.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新缺陷**；结掉 B0115 的点 + 记一处**跨层设计**
### ① `DISCIPLINE_TEMPLATE` 不含任何 marker ⇒ B0115 的未核实点**关闭**
```rust
const DISCIPLINE_TEMPLATE: &str = "【对话纪律】\n\
1. 每次回复只写 1-5 句，句子要短，像即时通讯。\n\
2. 每句话以真实句读结尾（。！？），不要用逗号把多层意思串成长句。\n\
3. 不要写 Markdown 标题/列表/加粗，不要替用户说话。";                    // :175-178
```
⇒ 与 F-0114-01 相关的第三条内容源（宿主侧模板）**干净**；
⇒ 「合成提示词的三个来源」（角色卡原文 / 记忆标记块 / 纪律模板）中，**只有记忆那条需要防 marker 伪造**，
与 F-0114-01 的范围判断一致。

### ⭐ 顺带记一处**跨层设计**（正面，且它是**承重**的）
`DISCIPLINE_TEMPLATE` 第 2 条：「**每句话以真实句读结尾（。！？）**，不要用逗号把多层意思串成长句」
⇒ 这条提示是 AGENTS.md「**一句一单元**」红线（语音输出约定）在**提示词侧**的落点。
而我在 B0005/B0006 核过的**装配器侧**是：`dialogue/sentence.rs` **只按真实句读边界切分**、
**不允许按字符位置硬切**（那条禁令写在 `settings/patch.rs:20-23` 附近的注释里，我 B0006 记过）。
⇒ **两层必须同时成立**才有良好的听感：模型若**不**遵守这条提示而用逗号长句，
装配器**按设计不能硬切** ⇒ 用户会听到**无法断开的长句**（而这不是装配器的 bug）。
⇒ 记此正面样本：**红线的「提示侧」与「实现侧」被显式对齐**，且实现侧的禁令（「不许硬切」）
反过来**依赖**提示侧的质量。

## 未核实项
1. `memory/src/{summary.rs,store.rs,summary_store.rs,commands.rs}` 本体未读（检索/打分、摘要触发、store 上限）
2. `persona/sessions.rs` 全文未读；`PersonaCard::parse_json`(:210-241) 未读
3. `wallpaper`(729，**已封存**) · `template` 未读
4. 各 Mod 的 `settings_spec` 与实际读取键是否一致未核
5. `include_discipline` 的默认值与前端是否暴露该开关未核

## 本批新增
**0 条**
