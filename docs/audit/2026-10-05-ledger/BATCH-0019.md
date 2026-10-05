# BATCH-0019 · 密钥真源 + 表演层 + WAV 解析

Phase 1 · 域覆盖 → `crates/live2d-ai-runtime/**`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/secrets.rs` — 416（读 1-176 + B0010 已读的 178-220）
2. `crates/live2d-ai-runtime/src/performance/mod.rs` — 478（读 60-110 / 300-419）
3. `crates/live2d-ai-runtime/src/audio/wav.rs` — 228（读 1-110）

## 跑过的命令（全部只读）
```
git ls-files 'crates/live2d-ai-runtime/src/tts.rs' 'crates/live2d-ai-runtime/src/audio/' | xargs wc -l
grep -rn "std::env::var|env::var(" crates/ --include=*.rs | grep -v "test"      # → 15 处非测试命中
grep -n "enum FallbackReason" -A 40 performance/mod.rs
grep -n "self.fallback(|fn fallback" performance/mod.rs                        # → 4 调用点
grep -rn "fallbacks()" crates/ --include=*.rs                                  # → 唯一断言在 tests.rs:648（=0）
grep -rn "InvalidPlan|performance_plan_invalid" performance/tests.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖
- 维度 A：密钥快照是唯一真源，`SNAPSHOT` 单例 + `lookup` 单入口；无 `set_var` 全局污染 ✔
- 维度 G：`known_keys()` 只回键名、`is_set()` 只回布尔（:157-165）；`GET /api/v1/env` 的实现侧已在 B0007 逐字段确认
- 维度 D：`snapshot()` 每次 `lookup` 克隆整张 `BTreeMap`（:132）。频率：每次 status/self-check/
  supervisor 装配一次，map 仅 2–5 项 ⇒ 量级可忽略，**不记发现**
- 维度 C：表演层回退有 4 条路径，F-0019-01
- 架构/正确性：WAV 解析 6 条不变量全过

## 本批主动推翻的假设
1. 「`snapshot()` 每次克隆整张表是性能问题」——**证伪（量级）**：map 仅 2–5 项，
   `lookup` 的调用频率是「每次 status/自检/装配」，不是热路径。
2. 「`snapshot()` 在读取失败时会用空表覆盖好数据」——**证伪**：:136 `let _ = refresh_from_disk();`
   忽略错误后重新 `read()`，而 `refresh_from`（:114-127）只在 `NotFound` 时置空表、
   其它错误直接 `return Err` **保持旧快照**；且此时 SNAPSHOT 仍为 `None`，`unwrap_or_default()`
   只是让 `lookup` 落到第二优先级 `std::env::var`——正是契约写的降级路径。
3. 「RIFF 块长度声明为 0xFFFFFFFF 会 panic 或死循环」——**证伪**：:53 取交集把 body 夹到实际长度，
   :73 的 `pos = body_start + size + (size & 1)` 在 64 位 usize 下不溢出，
   下一次 `pos + 8 <= bytes.len()` 判假即退出。

## 未核实项
1. `secrets.rs:177-416` 中 `write_key_to` 的后半段（回读校验细节）只在 B0010 读过 178-220，
   `known_keys`/`is_valid_key` 之后的辅助函数未逐行读。
2. `performance/plan.rs`(857) 与 `performance/client.rs`(652) 未读——plan 校验器本体
   （`parse_plan` / `is_noop` / warnings）顺延 BATCH-0020。
3. `audio/resample.rs`(280) / `audio/spec.rs`(95) 未读。
4. `audio/wav.rs:111-228` 的测试段未读。
5. F-0019-01 的用户可见性（诊断面板实际显示）依赖 B0009 已确认的 DTO 投影，
   **未跑真实服务验证**（本任务禁跑门禁）。

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0 ｜ 另：**红线 R 核心纪律全仓验证通过**（15 处 env 读取逐一归类）
