# BATCH-0573 落盘（无装饰）· docs/verification/ 21 个文件、4626 行

## 清单（wc -l；合计 4626 行）
rust-bakeoff-mocari.md 891（最大）
node-c-c7-llvmpipe-benchmark-2026-08-27.md 571
node-c-c5-prepare-upload-audit.md 416
node-c-manual-acceptance-checklist-draft.md 363
node-c-manual-rc-checklist-2026-08-28.md 266
flutter-shell-manual-checklist.md 244
acceptance-metrics-report.md 241 / -part2.md 199
verification-win.md 235 / node-c-c5-static-frame-trace-2026-08-27.md 227
node-c-location-recheck-2026-08-27.md 207
audio-playback-noise-2026-09-10.md 150
rust-bakeoff-ayagami.md 144 / ignition-core-loop.md 88
rust-bakeoff-decision.md 66 / smoke-checklist.md 56
desktop-platform-smoke-2026-08-26.md 32 / pc-preview-smoke.md 24
assets/ 4 张 png
（注：wc 对 png 报的是字节换行数，不是行数）

## 与 B0572 一致：这一块有五处代码活引用，分布在两个 crate
crates/l2d/src/lib.rs:14                     决策记录见 docs/verification/rust-bakeoff-decision.md
crates/l2d/src/renderer/mod.rs:16           接线来自已验证的 bakeoff 工程（docs/...）
crates/l2d/examples/static_frame_trace.rs:27  docs/verification/node-c-c5-prepa...
crates/live2d-ai-desktop/Cargo.toml:12       见 docs/verification/rust-bakeoff-ayagami.md
crates/live2d-ai-desktop/src/benchmark/mod.rs:26  DISPLAY=:0，详见 docs/ver...

## rust-bakeoff-decision.md 前 6 行（66 行的文件）
标题：Rust Bakeoff 选型决策：Ayagami vs Mocari（RFC D4）
日期：2026-08-26
结论：选 Ayagami 作为 runtime/render 底座，以固定 rev
  640ae4b10bad8def1adcacdada4f8241b484c169 引入 crates/l2d；
  Mocari 不引入，保留为未引入参考与动画补充候选。

## 三个可核点
1. 决策带一个具体的 40 位 rev（与 B0561 核的 soullink 锁 0.1.0-beta.1 同一手法）
   => 依赖钉死 + 决策写进仓库，两处都这么做。
2. 三张图里 mocari-bai.png 正是没被选中的那个的渲染结果
   => 留着图 = 保留反例证据，不是垃圾文件（与 B0571 的归档/活引用区分一致）。
3. 文件名带日期的多是 2026-08，而 audio-playback-noise 是 09-10
   => 这一块跨越了两次架构切换（08-31 Rust 主线重写、09-11 删 renderer/）
   => 判据：看文件名日期就知道一份验收记录属于哪一代架构。

## 未核
20 份 md 的正文 · render_model.rs 其余部分 · arbiter/decision/ledger/presets/
plan/staging* 本体 · 其余 8 个 mod crate 本体 · mod-system 的 tests/ 四行 · shared/ 其余 8 个 json
