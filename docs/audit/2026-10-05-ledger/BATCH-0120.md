# BATCH-0120 · ⭐ **「三个写盘点收在一处」核实为真** + `.env` 侧同纪律（含一处**权限窗口**修复）

Phase 1 · 域覆盖 · `live2d-ai-runtime`（老代码区，查内部）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/settings.rs` — 826（B0119 已读 571-578 / 700-765；本批**穷举调用方**）
2. `crates/live2d-ai-runtime/src/secrets.rs` — 416（定点 50 / 167-175 / 211-214 / 226：`.env` 写盘）
3. `crates/live2d-ai-desktop/src/` 三处调用点（`app/settings_ui.rs:339,379` · `web_api/settings_routes/mod.rs:203` · `web_api/cli_entry.rs:225`）

## 跑过的命令（全部只读，穷举无 head）
```
grep -rn "to_toml_string_merging" crates/ --include=*.rs
grep -rn "fs::write|File::create" crates/live2d-ai-desktop/src/ crates/live2d-ai-runtime/src/ --include=*.rs | grep -v tests
grep -n "pub fn " secrets.rs
grep -n "注释|0o600|rename" secrets.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；B0119 留的开放点**关闭**
### ① 「三个写盘点收在一处」—— **为真，且恰好三个**
`to_toml_string_merging` 的**全部**生产调用点（穷举）：
| # | 调用点 | 是什么 |
|---|---|---|
| 1 | `web_api/settings_routes/mod.rs:203` | **HTTP 设置 PATCH/POST**（产品路径） |
| 2 | `web_api/cli_entry.rs:225` | **Mod `apply_settings` 写盘**（B0062 那条的宿主侧） |
| 3 | `app/settings_ui.rs:379` | egui 设置面（AGENTS.md「第二休眠台账」里的**休眠壳**） |
⇒ 与 `settings.rs:747-750` 头注声称的「三个写盘点」**逐一对应**。
另穷举 `live2d-ai-desktop` + `runtime` 内全部 `fs::write` / `File::create`：
命中项分别是 benchmark JSON、**log_routes 的测试**、models registry 自己的文件、
chat_routes/secrets 的**测试** ⇒ **没有任何一处直接写 `live2d-ai.toml`**。
写盘机制亦为原子：`settings_ui.rs:339`「tmp） + `fs::rename` 原子换名——**不**直接 `fs::write` 整个文件」。

### ② `.env` 侧是**同一条纪律**，且把同一个失败模式**点名写出**（正面）
`secrets.rs:167-175`：
```rust
/// 把 `KEY=VALUE` 写进 `.env`（**就地改那一行**，保留其它行与注释），并刷新快照。
…
/// 里面有注释、空行、别人写的变量。**整份重写会把这些抹掉——用户不会立刻发现，
/// 等发现时已经找不到原来的注释了**。
/// 写盘走 [`crate::settings::patch::plan_atomic_write`]（tmp → fdatasync → rename）
```
⇒ 与 toml 那次事故**同一个失败模式**，此处**显式记着**（含「用户不会立刻发现」的用户侧后果）。
`merge_env_line`(:226) 是**纯函数**实现就地改行 ⇒ 符合本仓「纯逻辑放可测处」的纪律。

### ⭐ ③ 顺带核到**本审计见过最细的一处安全细节**（正面）
`secrets.rs:211-214`：
```rust
// 先收紧 **tmp** 的权限再 rename：否则会有一段「文件已是新密钥、权限还是
// umask 默认（**通常 0644**）」的窗口。rename 之后收紧就晚了。
if let Err(e) = std::fs::rename(&tmp, path) {
```
⇒ **一个亚秒级窗口：新写入的 API 密钥以 0644 存在**（同机任何用户可读）。
**先 chmod tmp、再 rename** 把窗口关掉。
⇒ 这与 F-0111-01（token 进 argv，同机可读）是**同一类威胁（同机其他进程能读密钥）**，
但**这一处已经被想到了并修掉了**，而 argv 那处没有 ⇒
**说明「同机可读」这个威胁模型在本仓是认识的，只是没有覆盖到进程命令行那条通道。**
（这给 F-0111-01 的分量又加了一条佐证：**不是没人想到，是覆盖不全**。）

## 未核实项
1. `settings/patch.rs`(780) 内部未审（B0010 只读 `PatchBody` 结构）；`plan_atomic_write` 本体未读
2. `performance/plan.rs`(857) / `llm.rs`(530) / `conversation/{engine,mod,worker}.rs` 未审
3. `secrets.rs` 其余（`parse` / `lookup` / `refresh_from_disk` / `known_keys`）未审
4. `sse.rs`(361) / `audio/{resample,rms}.rs` / `error_code.rs` 部分未审
5. `app/settings_ui.rs`（**休眠壳**）其余部分未审 —— 按休眠台账**不接回**，但其**写盘点**已核

## 本批新增
**0 条**
