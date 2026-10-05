# BATCH-0227 · ⭐ `error_banner.dart`（**仅 63 行**）：**三条来自调研的硬要求** + 三处码→动作映射 + 一次三合一收敛

Phase 1 · 域覆盖 · 前端 `.dart`（36/218）—— `lib/ui/error_banner.dart`(63)

## 跑的命令（全部只读）
```
wc -l lib/ui/error_banner.dart ; sed -n '1,18p' lib/ui/error_banner.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **63 行里含三条独立的正面模式**
### ① ⭐ 设计决策**引用了外部调研**并给了可查索引（:5-8）
> 「**禁止把失败渲染成「只有降低不透明度」或「红色小字」** —— 这是同类项目**最一致的缺陷**之一
> （**Nexus / OLV-Web**）【**调研 survey §8 缺陷 17**】」
> 「所以这里用 `dangerSurface` 底 + `dangerBorder` 描边 + 图标 + **可点动作**。」

⇒ ⇒ 理由**不是「好看」**，而是「**三个具名项目都栽在这里，survey 有条目可查**」
⇒ ⇒ ⇒ **P1 的最强形态**：**后来者可以核对这条主张**（去翻 survey §8 缺陷 17）

### ② ⭐ 而且它给的是一条**关于规则的要求**（:9-12）
> 「**失败要有「下一步」**：`429 busy` 给「**打断并重发**」、`503 no_supervisor` 给「**去 LLM 设置**」、
> `network_error` 给「**重试**」。**只报告不给出路的错误提示会让用户停在原地。**」

⇒ ⇒ **三处码 → 动作的具体映射，逐条点名** ⇒ ⇒ 这就是红线「`hint()` 给一句可执行处置」
（B0161 核过后端侧）·「前端**按码分流**」（B0136 核过分支）**在 UI 侧的实现** —— **现在本体也读了**
⇒ ⇒ 而「**只报告不给出路**」被写成了**一条禁令** ⇒ ⇒ **禁令的对象是「设计」本身**，不是某次实现

### ③ 一次**三合一收敛**，且写了 before/after（:14-17）
> 「同一件事过去有**三套写法**（这里、`field_row` 的 `'⚠ $error'`、`dev_tools_section` 的同一句字面文本）。
> 现在**外形只有 `InlineNotice` 一处**，本组件只负责把 `ErrorAction` 翻译过去、并固定成 danger 档。
> 这样『**错误长什么样』以后只改一个文件**。」

⇒ ⇒ **P2（私有汇合）+ 理由**（改一处即可）
⇒ ⇒ 且**记下了收敛前的样子**（三套写法各在哪）⇒ ⇒ 与「记录推翻过的方案」**同族**
⇒ ⇒ ⭐ 这是本仓**第四处**「把同一件事收敛」的记录（另三处：令牌台账 B0202 ·
`settings_spec` 引用常量 B0215 · `key`→`has_api_key` 布尔 B0123）

### ④ ⭐ 而「文件长度 ≠ 信息密度」的第一例
**63 行**里含**三条独立的正面模式** + **三处码→动作映射** + **一条禁令**
⇒ ⇒ 对照 `dev_tools_section.dart`(1874)：**长文件不等于决策多，短文件不等于决策少**
⇒ ⇒ **读法上**：先看头注往往比先看代码**收益高**（B0226 已验一次，本批再验一次）

## 未核实项
1. `error_banner.dart` 余 45 行未读（`ErrorAction` 的定义与 `InlineNotice` 的对接）
2. `message_bubble.dart`(577) 未读（**纯文本消息不进 `Text.rich`** 那条测试的实现侧未读）
3. `memory_panel.dart`(694) · `persona_panel.dart`(608) · `director_observer_section.dart`(668) ·
   `live2d_stage.dart`(666) · `chat_panel.dart` 余 540 行未读
4. 前端 `.dart` 仍 182 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
