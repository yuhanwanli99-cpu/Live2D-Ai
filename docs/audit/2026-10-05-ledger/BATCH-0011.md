# BATCH-0011 · web_api 测试面收口（模式 B 复查）+ 自检端点

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`（收尾中）

## 读过的文件（来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/tests_security.rs` — 398（**读 230-398**；前 229 行是
   Origin/CT 断言，BATCH-0001 已 grep 过全部 30+ 条测试名）
2. `crates/live2d-ai-desktop/src/web_api/tests_security_e2e.rs` — 254（**全读**）
3. `crates/live2d-ai-desktop/src/web_api/tests_ws.rs` — 971（**读 700-796 + 枚举全部 28 条测试名**；
   未逐行全读——本批目的是模式 B 定向复查，不是测试全审）
4. `crates/live2d-ai-desktop/src/web_api/settings_routes/test_endpoints.rs` — 406（**读 60-359**）

## 跑过的命令（全部只读）
```
ls -la /tmp/cfg.toml /tmp/l1-session-route*.toml        # → 不存在（本机未跑过 cargo test）
grep -rn '"/tmp/cfg.toml"|/tmp/cfg\.toml' crates/ --include=*.rs
grep -n "^fn |^#\[test\]" -A 1 crates/live2d-ai-desktop/src/web_api/tests_ws.rs
grep -c "#\[test\]" crates/live2d-ai-desktop/src/web_api/tests_ws.rs   # → 28
grep -n "    fn |^fn " crates/live2d-ai-desktop/src/web_api/tests_ws.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 模式 B 复查收口结论（本批最重要的产出）
用「枚举全部测试名 + 抽读可疑条目」的方法复查 3 个测试文件（合计 1623 行 / 约 75 条测试）：

| 文件 | 结论 |
|---|---|
| `tests_security.rs` | **30+ 条全部为真测试**。`is_truthy_env_value` 5 正 6 负、`is_mutating_route` 覆盖 11 个 RouteId 变体、`extract_origin_and_ct` 三个场景各有独立输入。**无析取式恒真断言。** |
| `tests_security_e2e.rs` | 13 条全部走真实 `dispatch_with_security`，断言状态码 **且** 断言 body 里的 code 字面量。**无脱节式测试。** |
| `tests_ws.rs` | 28 条测试名**全部承诺可观察行为**（`repeated_subscribe_unsubscribe_does_not_leak` / `fill_to_limit_unsubscribe_refill` / `broadcast_does_not_artificially_drop_active_subscribers` / `real_ws_40_iterations_with_http_concurrent` …）。抽读 3 条（:726 / :747 / :779）**均为真测试**，且 :779 连「drop 掉 rx 制造 try_send 失败」这种不happy-path 都驱动了。**无模式 B。** |

⇒ **模式 B 不是本仓的普遍现象，而是集中在两处**：`model_root.rs`（F-0001-02）与
`supervisor_slot.rs`（F-0002-02）。这个校准很重要——它把「假绿灯」从「系统性文化问题」
降级为「两处具体的、可定点修的缺陷」。§12.3 的模式 B 结论据此**修订**。

## 发现对既有发现的**反证**（本批最有价值的一步）
`tests_ws.rs:776-777` 的测试头注写：「subscriber_count 在 broadcast 期间不会被错误地"清零"
——仅在 unsubscribe 显式调用 / **慢订阅者 try_send 失败时**回收。」
⇒ 我在 BATCH-0003 提的 **F-0003-04**（「背压被当成死亡」）所描述的行为，
是**被测试固化的既定策略**，不是实现与意图不符。已在 FINDINGS.md 显式回填该反证，
并把它的实质价值降为 P3 备忘（保留「策略有代价」的事实陈述，撤掉「实现与意图不符」的指控）。
**这是本轮第一次由后续批次证伪先前发现，按 §5 纪律记录在案。**

## 维度覆盖（本批）
- 维度 C（三态）：✔ 自检端点口径与 AGENTS.md 完全对齐：
  TTS 只探 `GET /models` **不合成**（test_endpoints.rs:228）、404 **算通过**附 note
  （:278-282，理由写明「只实现 /audio/speech 的也合法」）、401/403 → `auth_failed`、
  其余 4xx/5xx → `protocol_error`、超时文案明说「**这不等于不能出声**」（:253-255）
- 维度 G：✔ `resolve_api_key` 走 `secrets::lookup`（:151-153），注释记录了「原先直接
  `std::env::var` 绕过 .env」的历史缺陷；上游响应体截断 ≤512 字符（:275 / :327）防爆体积
- 维度 E：⚠ LLM 自检的 404 → `model_not_found`（**失败**）与 TTS 的 404 → 成功**刻意不同**，
  且 :264-270 给出了充分理由（`/chat/completions` 是 OpenAI 兼容服务必须实现的端点）。
  **判定为合理取舍，不记发现**，但两套口径共用同一套 `TestOutcome` 词表，
  后来者「统一」它会砸掉 TTS 那边来之不易的语义——已在 BATCH 里记一句提醒
- 维度 J：✘ 两条 e2e 测试真写固定路径 → F-0011-01；✘ 一条自述重复的测试 → F-0011-02

## 未核实项
1. `tests_ws.rs` 的 28 条测试**未逐行全读**（读了 100 行 + 全部测试名）。
   模式 B 复查的结论基于「命名承诺 + 抽读」，**不是全量断言审阅**——如实标注。
2. `tests_security.rs:1-229` 未读（但 BATCH-0001 的 grep 已见过全部断言行，形态一致）。
3. `test_endpoints.rs:1-59 / 360-406` 未读。
4. F-0006-03 的未核实项（`mod_registry.rs` 的 enable/restart 是否可能 panic）**本批未收口**，
   顺延 BATCH-0012。

## 本批新增
P0 0 · P1 0 · P2 0 · P3 2 ｜ 另：**证伪 F-0003-04 的一部分**（已回填 FINDINGS.md）
｜ **修订 CONSOLIDATION-01 的模式 B 结论**（见上）
