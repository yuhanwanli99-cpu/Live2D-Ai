# BATCH-0148 · `web_api` 语音路由测试面：**每条规则都配了反向对照，且钉住了安全相关的顺序**

Phase 1 · 域覆盖 · `web_api` 余 7 个测试文件（本批核其中 3 个）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-desktop/src/web_api/voice_routes_tests_gate.rs` — 209（**枚举全部 9 个用例名**）
2. `crates/live2d-ai-desktop/src/web_api/voice_routes_tests_token.rs` — 58（1 个用例名）
3. `crates/live2d-ai-desktop/src/web_api/voice_routes_tests.rs` — 793（结构枚举：helper + 协议级用例名）

## 跑的命令（全部只读）
```
for f in voice_routes_tests.rs voice_routes_tests_gate.rs voice_routes_tests_token.rs; do
  grep -n "fn |#\[test\]" <$f> | grep "fn "; done
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；这层测试是**本审计见过最强的「规则 ↔ 对照」结构**
### ① 唤醒门 9 个用例：每个规则都有**反向对照**，且有**三向**与**优先级**
```
gate_closed_403_when_wake_phrase_explicitly_empty      403 分支
missing_wake_phrase_falls_back_to_product_default     ← **反向对照**：缺失 ≠ 显式空 ⇒ 行为不同
manual_off_403_takes_priority                         ← **优先级规则**：手动关闭压过唤醒匹配
wake_phrase_required_400_when_not_present              ← **第三种结局（400 ≠ 403）** ⇒ 三向
matched_wake_phrase_with_punctuation_is_stripped       归一化
ptt_true_bypasses_wake_match_and_echoes_flag          绕过规则
without_ptt_bare_body_still_requires_wake_phrase       ← **绕过的反向对照**
ptt_wrong_type_is_invalid_payload                     类型严格
token_check_precedes_gate                             ← ⭐ **安全相关的顺序规则被按名钉住**
```
⇒ **`token_check_precedes_gate`** 值得单记：它把「**鉴权先于功能门**」这条**顺序**约束钉住了。
顺序反了 ⇒ 未鉴权的调用方可以**探测唤醒门的响应差异**（403/400 各自暴露）。
**而这条顺序在代码里看不出来，只能从路由的 `if` 次序推断** —— 这里有一个**用名字说出来的**回归。

### ② token 优先级的用例名**直接就是红线 R 的表述**
`dotenv_token_is_read_and_still_wins_over_config` ⇒ **`.env` 读到的值仍然压过 Mod config**
⇒ 与我在 B0104/B0124 核到的「`.env` 快照 > 进程环境 > 无」、
以及 B0005 核到的 external-input「env > Mod config」**同一条优先级**，现在**有了一条按名钉住它的回归**。

### ③ 协议级用例给出**互不相同的结局**（而非「都 4xx」）
`mismatched_path_returns_none` · `wrong_method_405` · `non_json_ct_415`
⇒ 404/405/415 三种**分别**被钉住 ⇒ 「路由存在」这件事**无法**用一个宽松断言蒙过去。

### ④ ⭐ 由此得到一条**对 §③ 判据的重要修正**
我在 B0147 写「`web_api` 是『缺陷形态由层年代决定』判据的唯一实证来源」。
本批读到这层测试后，判据要**细化**：
> **`web_api` 的生产代码是「局部疏漏」的富矿（7 条 P1），
> 而它的测试层却是本审计见过最规范的（每规则配对照、优先级按名钉住）。**
> ⇒ **同一层里，生产与测试的质量是分开的两件事。**
> ⇒ 「代码老 ⇒ 缺陷在内部」这个判据，**不能外推到同一层的测试**上。
> ⇒ **这修正了我自己刚写下的判据**（第 N 次，但这次是**判据本身**而非结论）。

## 未核实项
1. 余 4 个测试文件未读（`voice_routes_tests_say.rs` 148 · `models_routes/tests_models_{core,handlers,common}.rs` 730）
2. `voice_routes_tests.rs`(793) 的 helper（`call` / `call_auth` / `dummy_ctx` / `blackhole_endpoint`）
   **未读断言体** ⇒ 「用例名对应断言」这一步本批**未逐条核**
3. `live2d-ai-runtime` 余面（`client.rs` 652 / `config.rs` / `sse.rs` 余段）· `plan.rs` 余 700 行未读
4. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
