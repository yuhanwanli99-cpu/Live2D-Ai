# BATCH-0150 · `web_api` 余 4 个测试文件（30 用例）：**最后一组「正例 + 多向反例」**

Phase 1 · 域覆盖 · `web_api` 测试面**收尾**

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-desktop/src/web_api/models_routes/tests_models_core.rs` — 320（**枚举 14 个用例名**）
2. `crates/live2d-ai-desktop/src/web_api/models_routes/tests_models_handlers.rs` — 279（**枚举 12 个用例名**）
3. `crates/live2d-ai-desktop/src/web_api/voice_routes_tests_say.rs` — 148（**枚举 4 个用例名**）
4. `crates/live2d-ai-desktop/src/web_api/models_routes/tests_models_common.rs` — 131（**helper 模块**：tmp_path/tmp_dir/make_minimal_package/cleanup/fresh_store/sample_registry）

## 跑的命令（全部只读）
```
for f in <四个文件>; do grep -oE "fn [a-z0-9_]+" $f; done
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；30 个用例全部按行为命名，且成组配对照
### ① 路径守卫：**1 个正例 + 4 类反例**（这一组最关键）
```
normalize_relative_id_accepts_snake_case          ← **正例**：合法 id 真的被接受
normalize_relative_id_rejects_empty_and_dot
normalize_relative_id_rejects_parent_escape      ← `..`
normalize_relative_id_rejects_path_separators    ← `/` `\`
normalize_relative_id_rejects_special_chars
resolve_safe_path_accepts_legitimate_id         ← **正例**
resolve_safe_path_rejects_traversal              ← **反例**
registry_atomic_write_round_trip                 ← **round-trip**（写入再读回）
```
⇒ **正例证明它不是「一律拒绝」；四类反例证明它不是「只拦 `..`」** —— 两者缺一，这个守卫就可被绕过。
⇒ ⭐ 而**同一条纪律在两个独立位置各写了一遍**：
`models_routes` 的 `normalize_relative_id`（模型 id）与 B0092 核过的
`l2d/src/asset` 的 `normalize_resource_ref`（资源相对路径）—— **两套守卫、同一形态**。

### ② 「缺档」与「坏档」是**两个不同结局**
```
registry_load_missing_file_returns_default     缺 ⇒ 缺省
registry_load_corrupted_returns_error          坏 ⇒ 报错
```
⇒ 「一律回落缺省」与「一律报错」**都过不了**对方那条 ⇒ 不能用一个宽松断言蒙过。

### ③ handler 层：有**状态规则**与**四向**的更新语义
```
handle_activate_then_delete_active_rejected    ← 状态规则：不能删「正在用的」
handle_delete_non_active_succeeds_204          ← **对照**
handle_import_rejects_traversal_id / rejects_nonexistent_dir / rejects_dir_without_model3_json   ← 三种不同理由
handle_display_persists_model_and_stage / partial_only_stage_preserves_model /
  rejects_unknown_field / unknown_id_returns_404   ← **四向**更新语义
```

### ④ 语音「说」路径的 4 个用例
```
success_returns_200_with_cleaned_text       成功且**已清洗**（`clean_for_tts`，B0132/B0142 核过）
zero_width_chars_are_cleaned_before_say     **具体**的清洗场景
busy_returns_200_ok_false                  ⭐ **忙碌是 200 + ok:false**，不是错误码
empty_transcript_never_reaches_say          ⭐ **空转写绝不进 say 通道**（反例：防空 say）
```
⇒ `busy_returns_200_ok_false` 值得单记：它把「**忙碌**」这个状态**钉成一个具体的成功响应形状**
（200 + `ok:false`），而**不是**让实现自己选 —— 这与 AGENTS.md 里 external-input 的
「忙碌 `ok:false`」契约**同形**（B0005 核过 external 侧）。

⇒ **0 findings**。`web_api` 的 **8 个测试文件（2472 行）至此全部核过**。

## 未核实项
1. `web_api` **测试的断言体**只逐条核过 2 处（`token_check_precedes_gate`、`同文件其余靠命名判断）
   ⇒ 「名字 ↔ 断言」这一步**未全量核**（**如实标注**）
2. `live2d-ai-runtime` 余面（`client.rs` 652 / `config.rs` / `sse.rs` 余段）· `plan.rs` 余 700 行未读
3. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
