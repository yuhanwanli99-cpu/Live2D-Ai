# BATCH-0015 · 句子装配器 + 思考隔离 + LLM 事件解析（三条红线的实现地）

Phase 1 · 域覆盖 → 第 2 项 `crates/live2d-ai-runtime/**`（本目录首批）

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/dialogue/sentence.rs` — 232（全读）
2. `crates/live2d-ai-runtime/src/dialogue/mod.rs` — 50（全读）
3. `crates/live2d-ai-runtime/src/dialogue/orchestrator.rs` — 85（全读）
4. `crates/live2d-ai-runtime/src/dialogue/clean.rs` — 338（读 1-175）
5. `crates/live2d-ai-runtime/src/conversation/worker.rs` — 344（读 1-175）
6. `crates/live2d-ai-runtime/src/conversation/engine.rs` — 586（读 290-586）

合计 6 文件 / 1635 行（细读约 1400 行）。

## 跑过的命令（全部只读）
```
git ls-files 'crates/live2d-ai-runtime' | xargs wc -l | sort -rn
git ls-files 'crates/live2d-ai-runtime/src/dialogue/' | xargs wc -l
git ls-files '.../conversation*' '.../llm*' '.../sse*' | xargs wc -l | sort -rn
grep -rln "sentence|Sentence|reasoning_never_reaches" crates/live2d-ai-runtime/src/
grep -rn "reasoning_never_reaches_the_sentence_assembler" -A 40 crates/live2d-ai-runtime/src/
grep -rn "clean_for_tts" crates/ --include=*.rs                    # 29 命中
grep -n "TtsJob {" -A 6 conversation/engine.rs                     # 两个构造点
grep -c "tracing::" conversation/worker.rs conversation/engine.rs   # → 0 / 0
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 结论摘要
本批是**红线验证批**，五条红线的判定见 `FINDINGS.md` 的表格：思考隔离 ✔（双保险 + 能失败的测试）、
空句不进 TTS ✔（`worker.rs:105` 真实执行点）、`clean_for_tts` 不得旁路 ✔（两个 `TtsJob` 构造点 + D23 e2e）、
一句一单元 **有条件**满足（F-0015-02）、`sentence_seq` 双路径一致 ✔。
实质缺陷 1 条：F-0015-01（worker panic 消息被 `unwrap_or` 丢弃，且该路径 `tracing::` 为 0）。

## 未核实项
1. `dialogue/clean.rs:176-338` 未读（链接剥离 / 空格折叠 / 测试段）。
2. `conversation/worker.rs:176-344` 未读（增量解码循环 / WAV 分支 / `emit_sentence_voiced`）。
3. `conversation/engine.rs:1-290` 未读（**含 LLM 流主循环与取消 select**——维度 C 的重点，
   顺延 BATCH-0016）。
4. `llm.rs`（530）与 `sse.rs`（361）未读——`LlmEvent` 的产生侧与 SSE 解析是本主题的另一半，
   顺延 BATCH-0016。
5. F-0015-01 的「worker 会 panic」是**条件性**的（需要解码侧出错）；未构造。

## 本批新增
P0 0 · P1 0 · P2 1 · P3 1
