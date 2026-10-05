# BATCH-0018 · TTS 侧：PCM 解码 / WAV 分支 / 音频模块结构

Phase 1 · 域覆盖 → `crates/live2d-ai-runtime/**`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/tts.rs` — 130（**全读**）
2. `crates/live2d-ai-runtime/src/audio/decoder.rs` — 193（读 1-120）
3. `crates/live2d-ai-runtime/src/conversation/worker.rs` — 344（读 176-344，**补齐 B0015 未读段**）
4. `crates/live2d-ai-runtime/src/audio/rms.rs` — 242（结构 + 导出面）
5. `crates/live2d-ai-runtime/src/audio/queue.rs` — 174（读 1-30）
6. `crates/live2d-ai-runtime/src/audio/mod.rs` — 113（读 re-export 面）
7. `crates/live2d-ai-runtime/src/lib.rs` — 216（读 48-93，**crate 根 doctext 与 re-export**）

## 跑过的命令（全部只读）
```
git ls-files 'crates/live2d-ai-runtime/src/tts.rs' 'crates/live2d-ai-runtime/src/audio/' | xargs wc -l
grep -rn "fn rms|fn compute_rms|rms_s16le" crates/ --include=*.rs
grep -n "^pub fn |^pub struct |^pub enum " audio/rms.rs
grep -n "pub mod|pub use" audio/mod.rs
grep -rn "rms::|RmsWindow|SampleQueue" crates/ --include=*.rs
grep -rln "cpal" crates/ --include=*.rs
grep -rn "SampleQueue|VecDeque|Vec<f32>" crates/live2d-ai-desktop/src/audio/*.rs
grep -rn "^cpal|cpal =" crates/live2d-ai-desktop/Cargo.toml Cargo.toml
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖
- 维度 C：取消纪律在 worker 的两处 select（:163 / :264）与 `tts_worker` 的 select（:65）都在
- 维度 D：解码热路径**无逐样本分配**（容量精确，见下方「证伪」）；`take_leading_chunk`
  每块一次 `drain().collect()` 分配 ≈ 19KB × 5 块/秒 ≈ 100KB/s，**量级可忽略，不报**
- 维度 B：worker 退出路径三条，全部走 `WorkerExit`，无线程泄漏（engine.rs:534 统一 join）
- 维度 G：TTS 请求体只含 `input`/`model`/`voice`/`response_format`，**不回显任何密钥** ✔
- 架构：F-0018-01

## 本批主动推翻的假设
1. 「PCM 解码热路径会因逐样本 `Vec` 扩展而反复 realloc」——**证伪**：`decoder.rs:51`
   `Vec::with_capacity(total / 2)` 容量精确，一次分配；循环内只有 `push`。
2. 「`take_leading_chunk` 的 `drain().collect()` 是性能悬崖」——**证伪（量级）**：
   4800 样本/块 × 4 字节 = 19.2KB，每 200ms 一次 ⇒ ≈ 96KB/s 分配 churn，
   对常驻桌宠进程完全不可观测。**不记发现**（若将来块大小或声道数上调 10 倍再评）。
3. 「PCM 与 WAV 两条路径的切块循环条件不同（`>=` vs `>`）会导致某句缺 `end`」——**证伪**：
   两条路径都无条件在末尾发一次 `final_chunk: true`（:217 / :327），
   整句整数块时只是「末块形态」不同（空尾块 vs 满末块），`end` 帧都到。

## 未核实项
1. `audio/decoder.rs:121-193` 的测试段未读。
2. `audio/{resample,spec,wav}.rs`（280+95+228 = 603 行）未读——重采样与 WAV 解析，
   顺延 BATCH-0019。
3. `audio/rms.rs` / `queue.rs` 是否有 `#[cfg(test)]` 测试段未读（不影响 F-0018-01 的结论：
   本条的论据是**生产零调用**，与它们自己有没有测试无关）。
4. `crates/live2d-ai-desktop/src/audio/`（cpal 输出路径）未读——它与 F-0018-01 的结论
   （「cpal 用自己的 ring 缓冲」）相关但未逐行核实。

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0
