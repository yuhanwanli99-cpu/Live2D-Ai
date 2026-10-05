# BATCH-0228 · ⭐ 三个面板头注：**每一个都先说职责与边界**，其中一个**点名了不存在的 API**

Phase 1 · 域覆盖 · 前端 `.dart`（38/218）—— `message_bubble.dart`(577) · `memory_panel.dart`(694) · `persona_panel.dart`(608)

## 跑的命令（全部只读）
```
for f in <三个文件>; do wc -l $f; sed -n '1,12p' $f; done
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**P22 第五例**（规则要写明「它不适用于谁、为什么」）
### ① `message_bubble.dart`：**import 边界的理由是「可测性」**
> 「只依赖纯模型 [ChatMessage] 与**纯 Dart** 的 [parseChatMarkdown] —— **不** import `chat_controller.dart`，
> **那样会把 `package:web` 拖进来，让整个气泡不可测**。」

⇒ ⇒ 不是「我们选择不 import」，而是「**import 它会让整个气泡不可测**」⇒ ⇒ **可测性作为stated reason**
⇒ 且带**日期 + 编号**的改动记录（2026-09-11 P2-3），第 1 条**指向规则所在处**
（`chat/chat_markdown.dart` 头注）而**不复述** ⇒ ⇒ 又一处「**指路而非复制**」（B0217/B0220 同族）

### ② `memory_panel.dart`：**有独立的「# 边界」段**，并**声明文件归属**
> 「面板**自己不做网络**：所有动作走 [ModPanelContext.onCommand]；数值来自 `GET …/state`；
> 列表来自 `command("list")`（**最新在前**），编辑/删除**按 `id` 定位**」
> 「**本文件由 memory 轨道独占，其他轨道不要改**。」

⇒ ⇒ **数据源逐条枚举**（含**排序**「最新在前」与**身份**「按 `id` 定位」）
⇒ ⇒ **归属声明**（与 B0217 模板的「不要做什么」+ B0220 的职责分半**同族**）

### ③ ⭐ `persona_panel.dart`：**点名了一个「不存在」的 API**，从而说明设计是**被逼的**而非**选的**
> 「**面板没有「保存配置」的通道**，所以走一次性命令：`ModPanelContext` 只给了 `onCommand`
> （POST …/command），**没有** POST /mods/persona/config。所以导入**不是**往配置表单里塞文本，
> 而是：`import_card` 命令在 **Mod 侧解析 → 规范化落盘 → 立刻生效**。」

⇒ ⇒ ⭐ **写下一个不存在的 API**，读者才知道这个设计**没有别的选择**
⇒ ⇒ 并给出**后果与三步效果**（Mod 侧解析 → 规范化落盘 → 立刻生效）⇒ 与我在 B0113 核的 persona 侧**接得上**
⇒ 且它把**跨 Mod 关系**写进了头注：与 memory Mod 的 **last-writer-wins**（= B0221「形状各异、语义统一」的 UI 侧处理）

### ④ ⭐ 而本批的元观察：**5 个前端文件的头注，无一例外先说职责与边界**（命中率 5/5）
`chat_panel`(B0226) · `error_banner`(B0227) · `message_bubble` · `memory_panel` · `persona_panel`
⇒ ⇒ **P22 的第五例**，且是**成规模**的 ⇒ ⇒ 「**先头注**」这条读法在本根上**连续五批回本**

## 未核实项
1. 三个文件**本体未读**（1779 行）：`message_bubble` 的富文本/流式光标/失败态/复制 · `memory_panel` 的列表与编辑删除 · `persona_panel` 的导入链 UI
2. `chat_panel.dart` 余 540 行 · `error_banner.dart` 余 45 行未读
3. `director_observer_section.dart`(668) · `live2d_stage.dart`(666) 未读
4. 前端 `.dart` 仍 180 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
