# BATCH-0161 · ⭐ `hint_for`：一条提示词**专门反驳一个具体的误判**（0 条新发现）

Phase 1 · 域覆盖 · `conversation/error_code.rs`（`hint()` 全族）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/conversation/error_code.rs` — 249（定点 85-102 `hint()` · 105-143 `hint_for`）

## 跑的命令（全部只读）
```
grep -rn "fn hint" -A 24 conversation/error_code.rs
sed -n '112,143p' conversation/error_code.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**提示词被当作「反误诊装置」用**
### ① 状态码分支**穷举**，且连兜底都给出可执行动作
| 状态 | 提示（逐字要点） |
|---|---|
| **401 / 403** | 「确认 `[{stage}] api_key_env` 指向的环境变量在『**后端进程**』的环境里**已设置且非空**」+ 追加 `non_auth_note` |
| **404** | 「确认 `base_url` 与 model 名在服务端存在（**含尾部 `/v1` 的写法**）」⇒ 点名一个常错细节 |
| **429** | 「降低并发或稍后重试」 |
| **5xx** | 「服务端故障，重试或查看上游日志」 |
| 其它 | 「核对 `[{stage}]` 配置，并看上文字段里的**上游响应体**」⇒ **兜底也可执行，且指向片段** |
⇒ **`Status` 臂内零个 `None`**；其余 `Error` 变体（`Http` / `InvalidBaseUrl` / `Sse` / `Json` …）也各有专属文案。

### ② ⭐ 而 `non_auth_note` 是个**专门反驳某个误判**的参数（:107-109）
> 「`non_auth_note` **只在 401/403 时追加**（**它要直接反驳「是不是我改提示词改坏的」这个误判**）。」

⇒ 这与 AGENTS.md 记的 2026-09-11 事故**逐字对应**：
用户报「**改提示词就崩**」+「后端出错无具体错误代码」，
而复核结论是「改提示词本身不失败（10 组取值全 200），当前那条失败是**进程环境缺 `DEEPSEEK_API_KEY` 的 401**」。
⇒ 所以那句 `non_auth_note` 不是一个泛泛的提示，**它是一条被写进代码的「反误诊」**：
把用户从**错误假设**（提示词改坏了）拉回**正确排查**（后端进程环境里的凭据）。
⇒ 并且它**按段参数化**：`llm` 传「提示词内容与鉴权无关」· `tts` 传「检查 `[tts]` base_url / voice」
⇒ **同一个 401 在不同段给出不同的反误诊** ⇒ **一个 `hint_for` + 段参数**（正面模式 **P2**）。

### ③ 这条提示**真的到得了用户眼前**（与前几批的核验接上）
- WS `error` 帧携带 `{code, stage, message, hint, epoch, fatal}`（B0015 已核）
- 前端错误横幅「**下一步**」**按码分流**（B0036/B0037 已核）
⇒ 「码 → 可执行处置」这条链**闭合**：码可搜（上一批核的 `as_u16()` 有界），
且**带一句能立刻动手的话** ⇒ 这不是装饰性字段。

## 未核实项
1. `error_code.rs` 余 ~145 行未读（`Error` 枚举在此文件的定义部分、`Display`、自测）
2. `error.rs` 余 ~95 行未读（`Error` 枚举定义、`Display`、`hint` 之外的 `self_test` 段）
3. `plan.rs` 余 ~560 行未读（`message()` 逐一、`action_cue_payload`(851)、自测）
4. `client.rs` 余 ~540 行未读（`request` 主体、structured 降级、wire 回归测试体）
5. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `mod.rs` 余 ~350 行 · `config.rs:51-140` 未读
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
