# BATCH-0010 · settings 写盘 + 假绿灯定向复查（模式 B）

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`

## 读过的文件（来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/settings_routes/mod.rs` — 359（**读 1-260**）
2. `crates/live2d-ai-desktop/src/web_api/settings_routes/tests.rs` — 685（**读 400-599**）
3. `crates/live2d-ai-desktop/src/web_api/settings_routes/test_endpoints.rs` — 406（**未读**，
   本批只 grep 了它的错误码字面量；顺延 BATCH-0011）

本批实际细读 200 + 200 = 400 行 / 1450 行清单。**这是一次「窄而深」的批次，不是全读**——
理由见下「模式 B 复查的采样说明」。

## 模式 B 复查的采样说明（诚实交代）
假绿灯复查本应逐个测试读断言。685 行的测试文件全读会挤掉本批的上下文预算，
而本批的核心问题只需要**确认错误码耦合是否有测试兜住**。因此采取：
先 grep 定位可疑耦合点（`url_invalid` / `invalid_env_name`），再定点读该测试的断言体。
**结论：本文件读到的 200 行内未发现模式 B 的两种写法。**
这不是「已全面排查」，是「已按可疑点定向排查」，未读部分已登记。

## 跑过的命令（全部只读）
```
grep -rn "base_url 非法|api_key_env 非法" crates/ --include=*.rs
grep -rn "url_invalid|invalid_env_name" crates/ --include=*.rs
```

## 维度覆盖（本批）
- 维度 A：✔✔ `to_toml_string_merging`（mod.rs:203）保住注释与排版——AGENTS.md 记的
  2026-09-10「切一次 dev_mode 抹掉 46 行注释」缺陷**未回退**；写盘走
  `plan_atomic_write` + `rename`，rename 失败清理 tmp（:217-226）不��垃圾
- 维度 G：✔ 全文件**无** `std::env::var`，`lookup` 全程由 dispatch 注入（= `secrets::lookup`）
- 维度 E：✘ 错误码由消息子串派生 → F-0010-01（**有测试兜住，故只记 P3**）
- 维度 J（测试质量）：✔✔ `e2e_max_tokens_tri_state_through_http_patch`（tests.rs:492-549）
  是三态语义的端到端真测试，且 :531-534 明确写了「断言**引用常量**而不是写死数字」的理由
  ——这恰恰是任务书 §7-J 点名的 `expect(常量,常量)` 反面，团队**主动**规避了它。
  `e2e_action_scales_patch_clamps_via_http`（:552-576）同理，断言钳位而非写死策略值。

## 关键复核（差点报成 P2 的一条）
初判：「`apply_patch` 的错误码靠 `msg.contains(...)` 从**中文错误消息**里抠出来，
跨 crate 文案一改，前端按 `url_invalid` / `invalid_env_name` 分支的文案就静默退化成
`invalid_payload`」——按 §12.1 去找覆盖该路径的测试：
`settings_routes/tests.rs:420-434` `url_invalid_patch_returns_400` **真的**断言
`err.error.code == "url_invalid"`，且它走的是真实的 `apply_and_write` 路径。
⇒ 只要有人改 `live2d-ai-runtime/src/settings/patch.rs:715` 的文案，这条测试立刻变红。
**降级为 P3（可维护性债），不报 P2。**

## 未核实项
1. `settings_routes/mod.rs:261-359`（`handle_patch` 尾部）未读。
2. `settings_routes/tests.rs:1-399 / 600-685` 未读——模式 B 复查**未完成**，
   留 BATCH-0011 补（那里还有 web_api 其余 tests_*.rs）。
3. `test_endpoints.rs` 全文未读。

## 本批新增
P0 0 · P1 0 · P2 0 · P3 1 ｜ 另：一条「差点报成 P2」被测试兜住而降级（记录在案）
