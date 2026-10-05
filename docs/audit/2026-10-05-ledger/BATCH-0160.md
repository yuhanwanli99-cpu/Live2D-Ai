# BATCH-0160 · ⭐ `code_suffix()`：**上游状态码以 `as_u16()` 进码 ⇒ 码里不可能有上游文本**

Phase 1 · 域覆盖 · `runtime/src/error.rs`（错误码契约的**最后一处未读代码**）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/error.rs` — 216（定点 78-121：`code_suffix` / `upstream_status` / `status()`）

## 跑的命令（全部只读）
```
grep -rn "code_suffix" crates/ --include=*.rs
sed -n '78,121p' error.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**错误码红线的安全性质由类型保证**
### ① 上游状态码进码走的是**类型化整数**，不是字符串
```rust
Error::Status { status, .. } => format!("upstream_{}", status.as_u16()),   // :87
```
⇒ 码的形态**恒为** `<stage>_<已知后缀>` 或 `<stage>_upstream_<数字>`
⇒ **码里不可能出现上游响应文本** ⇒ 而码要流经**三处**：WS `error` 帧 · 日志 · 前端分支
⇒ **三处都不受上游文本影响** ⇒ **这是结构性保证，不是「做了清洗」**（与 P10 同形：
**把禁止项编码进类型**）。

### ② 契约性质**连同它的根因**一起写明
:80-84 「取值是**契约**：前端 / 日志 / 测试按它分支，**改动等同协议变更**。
上游状态码直接进码（`upstream_401`），因为『401 缺密钥』与『429 限流』**必须一眼分开**
—— 那正是 **2026-09-11『后端出错无具体错误代码』的根因**。」
⇒ **为什么**可以从代码本身恢复 ⇒ 改动前的人会先看到它。

### ③ ⭐ `upstream_status()` 单独存在，且**写明了「不要做什么」**
:98-101 「调用方常需『只按状态码分流』（401 → 查密钥、429 → 退避），
**不该去解析 `Display` 字符串**。」
⇒ **正面模式 P3 的又一实例**（更严的写法 + 点名更松的那个）：提供**类型化访问器**，
免得调用方去 `Display` 上做字符串解析。⇒ 且 P2（私有汇合）的同形。

### ④ 顺带核到响应体片段的**两条边界**（`status()` :110-119）
- **按字符截断**（`body.chars().take(MAX_SNIPPET_CHARS)`）⇒ 理由逐字写明「**避免劈开 UTF-8**」
  ⇒ 截断**不会**产出非法 UTF-8（否则序列化 panic 或日志乱码）；
- 超限时**补一个 `…`** ⇒ 截断是**可见的**，不会被误当成完整响应体。
⇒ 错误体片段因此**有界 + utf-8 安全 + 截断可见** ⇒ 与 B0121 的 `MAX_WAV_BYTES`、
B0127 的 `chars().take()` 属同一族「**按字符计界**」纪律。

## 未核实项
1. `error.rs` 余 ~95 行未读（`Error` 枚举定义、`Display` 实现、`hint()` 全族、自测）
2. `plan.rs` 余 ~560 行未读（`message()` 逐一、`action_cue_payload`(851)、自测）
3. `client.rs` 余 ~540 行未读（`request` 主体、structured 降级、wire 回归测试体）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `mod.rs` 余 ~350 行 · `config.rs:51-140` 未读
5. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
6. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
