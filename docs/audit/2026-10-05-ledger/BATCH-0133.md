# BATCH-0133 · 残余事件排空（rc.3 N0）：**修复与回归都在位，且回归复现的正是那个竞态**

Phase 1 · 域覆盖 · `live2d-ai-desktop/supervisor`（turn 编排主体）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-desktop/src/supervisor/turn.rs` — 682（定点 308-357：生成返回后的排空循环 + 停止分支）
2. `crates/live2d-ai-desktop/src/supervisor/tests_loop.rs` — （定点 758-790：`failed_turn_delivers_generated_text_via_text_fallback` 的文档 + 构造）

## 跑的命令（全部只读）
```
grep -rn "drain_residual|residual" crates/live2d-ai-runtime/src/conversation/*.rs      # 零命中
grep -rn "drain_residual_events|fn run_one_turn" crates/ --include=*.rs
sed -n '308,357p' supervisor/turn.rs
grep -rn "TextFallback" supervisor/tests_*.rs web_api/tests_*.rs
sed -n '758,790p' supervisor/tests_loop.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；AGENTS.md 的「去掉修复即红」**经核实为真**
### ① 修法在**正确的位置**、且**只有一条处理路径**
```rust
// ---- 生成返回后**必须先把通道里剩下的事件按正常路径处理掉**（2026-09-13 rc.3 N0 修，真机抓到的缺陷）----
for _ in 0..256 {                                        // :327 ← 有界
    let Ok(ev) = event_rx.try_recv() else { break };
    if ev_epoch(&ev) == root.epoch.get() {               // :329 ← 与 B0040 核过的 epoch 闸一致
        handle_engine_event(&ev, root, audio, …);       // :330 ← **与循环内同一个处理函数**
    }
}
```
⇒ 「顺序不变、语义不变：仍走同一个 `handle_engine_event`」（:326）
⇒ **没有第二条事件处理路径** ⇒ 不会与主循环漂移（**这正是模式 C/接缝缺陷的正解**：
把「处理」收进一处，让调用点只负责「什么时候处理」）。
⇒ `:353-354` **停止分支**改调 `drain_residual_events`（丢弃而非处理）⇒ **策略按场景分**：
正常结束 ⇒ 补处理；用户停止 ⇒ 丢弃（否则会继续上屏整轮正文）。**这个分叉是对的且未文档化于头注，
但由 `:322` 的「被后面的 `drain_residual_events` 静默丢掉」一句带出，读得出来。**

### ② ⭐ 回归**复现的正是那个竞态**（模式 H 最强形态，且**不变量被文档钉住**）
`tests_loop.rs:770` `failed_turn_delivers_generated_text_via_text_fallback`：
- 文档（758-768）把整条因果链写全：biased `select!` 选中 `gen_fut` 臂后**不回头 poll**
  ⇒ 事件滞留 ⇒ 被收尾的 `drain_residual_events` **静默丢掉** ⇒
  真机表现（2026-09-13，TTS 指着死端口）「WS 上只有 `error` + `turn_state`，**一个字都没有**」；
- 测试**构造的正是那个条件**：LLM mock 返回**真实正文**（`"第一句。第二"`）+
  TTS 指向 `http://127.0.0.1:9/v1`（**discard 端口**）⇒ **传输层**失败，
  且注释注明「**与上游 5xx 是不同的错误路径**」;
- 断言落在**可观察后果**上：那段正文必须经 `TextFallback` 到达 UI。
⇒ **去掉 `:327-344` 的排空循环 ⇒ 正文被丢 ⇒ 该测试变红** ⇒ AGENTS.md 的
「回归已实测『去掉修复即红』」**为真**，不是注释里的自我保证。
⇒ **这是本审计见到的第 3 个「真机抓到 → 结构性修法 → 复现竞态的回归」的完整闭环**
（前两个：WASM `stage_css` 的 background-image 长手属性、Mod `secrets` 的 chmod-before-rename）。

## 未核实项
1. `turn.rs` 其余 ~530 行未读（Stage A/B/C 全流程 / `TurnClose` 收口 / `emit_voice_ended_if_needed`）
2. `conversation/{engine,mod}.rs`（548/586）未审 —— **B0006/B0015/B0017 核过分句/致命分类/空句三处，
   但引擎主体与 `mod.rs` 的类型面未读**
3. `handlers.rs:351 drain_residual_events` 本体未读（**停止分支的丢弃语义**只从调用点推知）
4. `llm.rs` 余 360 行 / `client.rs` 余段 / `plan.rs` 余 700 行未读
5. `worker.rs` 余 200 行未读；`secrets.rs` 测试断言体未读；B0120「chmod toml」未核
