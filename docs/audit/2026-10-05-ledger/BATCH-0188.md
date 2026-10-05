# BATCH-0188 · ⭐ 「GET / PATCH 乱序」：**两次响应带不同内容** ⇒ 真的能测出「谁生效了」（0 条新发现）

Phase 1 · 域覆盖 · `shell/flutter/test/settings_controller_test.dart`（并发不变式核验）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/test/settings_controller_test.dart` — 619（定点 559-599：`GET / PATCH 乱序` 组两条）

## 跑的命令（全部只读）
```
grep -rl "GET / PATCH 乱序" test/ ; grep -n "GET / PATCH 乱序" -A 40 <file>
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**0 条新发现**；⭐ 得到一条**新的测试设计要求**
```dart
if (request.method == 'GET') {
  gets++;
  // **第二次 GET 回一个不同的 model，用来分辨「哪一次的响应生效了」。**   // :565-566
  return jsonResponse(gets == 1 ? kGetJson : kGetJson.replaceAll('deepseek-flash', 'later-model'), 200);
}
…
await c.load();   expect(c.remote!.llm.model, 'deepseek-flash');            // :579-580
c.edit((d) => d.ttsVoice = 'nova');  expect(await c.save(), SaveOutcome.savedApplied);   // :582-584
await c.load();   expect(c.remote!.llm.model, 'later-model', reason: '**最后一次 load 才是真相**');  // :586-587
expect(c.dirty, isFalse, reason: '**load 会丢弃草稿**');                       // :588
```
⇒ **序列里 save 夹在两次 load 之间** ⇒ 第一次 GET 的响应**完全可能**落在 save 之后才生效
⇒ **两次响应带不同 `model`** ⇒ 断言**能分辨是哪一次生效了**

### ⭐ 由此得到一条**新的测试设计要求**（正面模式 **P20**）
> **一条「乱序 / 迟到响应」的不变式测试，必须让两次响应**可区分**（不同内容或不同 id）。**
> **否则两次响应同值 ⇒ 「谁生效了」不可观测 ⇒ 该测试对它要防的 bug 是「结构上无法失败」** ——
> **这正是模式 H（假绿）的变体，而且它最容易被写出来时顺手做错**（复用同一个 fixture 响应）。
⇒ 本文件**做对了**：第二次 GET 的 body 被 `replaceAll` 改过 ⇒ **两次**必然不同。
⇒ 这与 B0174「对照 = 线上真实发生的**次数**」**同族**（第三次应用）：
**B0174 数次数 · 本批 让内容可区分** ⇒ 合起来是「**并发/时序类不变式必须可观测**」。

### ② 顺带：第二条把**不变式与 UI 义务**绑在同一个测试名里
:591 「load 会**丢弃未保存草稿**（**调用方必须先问用户——见 `confirmLeave`**）」
⇒ 不只说「会丢」，还说「**谁负责兜住这次丢**」⇒ 与 AGENTS/B0040 核过的
`confirmLeave`（我在 B0040 核过 `confirmLeave` 三处共用一入口）**接上了**。

## 未核实项
1. `SettingsController.load/save/edit` 的**实现**未读（只读了测试）⇒ 该不变式的**实现侧**未核
   ⇒ **注**：`settings_controller.dart:120-322` / `:372-409` 是**早前批次留下的未核项**，本批未动
2. 新露出的 208 条自我设防名**只看了 11 条** ⇒ 其余未逐条核
3. `dev_tools_section.dart`(1874) / `tokens.dart`(928) / `live2d_stage.dart` 余 ~640 行未读
4. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
5. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
6. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
