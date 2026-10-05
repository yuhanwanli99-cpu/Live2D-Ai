# BATCH-0020 · 表演层校验器本体（红线 P 最后一环）

Phase 1 · 域覆盖 → `crates/live2d-ai-runtime/**`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/plan.rs` — 857（读 580-794 + 结构图 + 常量）
2. `crates/live2d-ai-runtime/src/performance/client.rs` — 652（读 128-150 / 251-295 + 常量）

## 跑过的命令（全部只读）
```
grep -n "^pub fn |^fn |^pub struct |^pub enum |^impl " performance/plan.rs
grep -n "json!|post(|bearer_auth|api_key|body(" performance/client.rs
grep -rn "MAX_TOKENS|MAX_SEGMENTS|MAX_SEGMENT_CHARS|MAX_CUES|MIN_TTL_MS|MAX_TTL_MS|MIN_INTENSITY|MAX_INTENSITY|AXIS_MIN|AXIS_MAX" … | grep const
grep -n "fn to_wire_cue" -A 32 performance/plan.rs
grep -n "sentenceSeq|sentence_seq|'at'|presetId|class ActionCue" -A 3 shell/flutter/lib/api/ws_frame.dart
grep -rn "\.at\b" shell/flutter/lib/ | grep -i "cue|action"
sed -n '818,850p' shell/flutter/lib/main.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖
- 维度 G：请求体无密钥（key 只进 header）；`temperature: 0` 保证可复现
- 维度 D：`MAX_CUES=16` / `MAX_SEGMENTS=64` / `MAX_SEGMENT_CHARS=4000` 三道上限都存在
  ⇒ **无无界增长风险**；但它们与 `MAX_TOKENS` 不自洽 → F-0020-01
- 维度 E：`to_wire_cue` 的既有 5 键（`sentence_seq`/`preset_id`/`intensity`/`ttl_ms`/`priority`）
  逐字保留，v1 新键全为 `Option`（`plan.rs:174-187`），符合红线 Q「只增不改」
- 维度 J：`performance/tests.rs`(798) 本批**未读**（测试面留给 Phase 2 维度 J 横扫）

## 未核实项
1. `performance/plan.rs:1-96 / 96-580 / 794-857` 未读（`CueField`/`CueAnchor`/`FieldCue`/
   `PlanWarning`/`PlanError`/`parse_legacy`/`json_schema_strict` 全文）
2. `performance/tests.rs`(798) 未读。
3. F-0020-01 的**字符阈值**依赖分词器（0.7–1.4 token/中文字），本批按区间表述；
   若要精确阈值需门禁实测（本任务禁跑）——**结论不依赖精确阈值**（两个常量的量级差 4 倍已足够）。
4. F-0020-02 的「渲染面是否按 `at` 判断」未核实（`stage.applyPreset` 实现未读）。

## 本批新增
P0 0 · **P1 1** · P2 0 · P3 1
