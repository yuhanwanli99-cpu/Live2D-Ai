# BATCH-0017 · 错误码契约面（第 6 条红线：后端 tracing 的 code 与 WS 帧的 code 必须是同一个字符串）

> **⚠ 本文件是 CONSOLIDATION-06（2026-09-28）补写的占位文件。**
> 审计过程中该批次的 `BATCH-*.md` 未被写出（工具调用只落了 FINDINGS/INDEX/STATE/NEXT 的更新）。
> 权威内容在 `FINDINGS.md` / `INDEX.md` / `STATE.md`；此处只保留可从磁盘核实的信息，**不臆造缺失内容**。

## 读过的文件
- read crates/live2d-ai-runtime/src/conversation/error_code.rs + crates/live2d-ai-runtime/src/error.rs + desktop/supervisor/handlers.rs

## 该批次结论
- 见 FINDINGS.md 的「BATCH-0017（错误码契约面）」与「BATCH-0017 —— 第 6 条红线的验证结论」两节；产出 F-0017-01（P2）