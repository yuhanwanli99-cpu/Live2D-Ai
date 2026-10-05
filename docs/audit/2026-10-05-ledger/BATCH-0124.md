# BATCH-0124 · `.env` 快照与热重载：**接线在**，缺口只在 F-0002-01 说的「首次运行」

Phase 1 · 域覆盖 · `live2d-ai-runtime`（红线 R 的读取面 + F-0002-01 的第二半）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/secrets.rs` — 416（定点 110-148：`refresh_from_disk` / `refresh_from` /
   `snapshot` / `lookup` 契约）
2. `crates/live2d-ai-desktop/src/web_api/cli_entry.rs` — （定点 56-59 启动加载 · 334-345 watcher 反应）

## 跑过的命令（全部只读）
```
sed -n '110,148p' secrets.rs
grep -rn "env_file_path|refresh_from_disk|watch" desktop/src/web_api/cli_entry.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；**F-0002-01 的影响面被确认为两个文件**
### ① 快照刷新语义正确（正面）
```rust
let text = match std::fs::read_to_string(path) {
    Ok(t) => t,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),   // :118-119 缺文件 ⇒ 空表，不是错
    Err(e) => return Err(format!("读取 {} 失败: {e}", …)),                 // :120  真错 ⇒ 上报
};
let map = parse(&text);
if let Ok(mut g) = SNAPSHOT.write() { *g = Some(map); }                    // :123-125 整体替换
```
`snapshot()`（:128-141）先读快照、**没有才惰性 `refresh_from_disk()` 一次** ⇒ 首用自动落盘读。
`lookup` 的契约写在定义处（:146-148）：「**`.env` 快照 > 进程环境 > `None`**（空串一律视为未设置）」
+ 「这是全项目**唯一**该被用来读密钥的函数」。
⇒ **`.env` 快照 > 进程环境 > 无** 这条红线在实现层成立。

### ② ⭐ `.env` **确实在 watcher 的反应里** ⇒ F-0002-01 的影响面是**两个文件**
```rust
let _file_watcher = if let Some(sup) = &supervisor_opt {          // :334 ← 首次运行为 None
    …
    if status_for_watch.refresh_from_disk(&path_for_watch) { … }   // :342  toml 快照刷新
    match live2d_ai_runtime::secrets::refresh_from_disk() { … }   // :345 **.env 快照刷新**
}
```
⇒ 外部手改 `.env` ⇒ watcher 触发 ⇒ `secrets::refresh_from_disk()` ⇒ **新值对 `lookup` 可见**
⇒ **AGENTS.md 的宣称（「`.env` 也在监视集里，外部手改同样即时生效」）为真**。
⇒ 而 **F-0002-01（P1）的缺口正落在这条 `if let` 上**：首次运行 `supervisor_opt` 为 `None`
⇒ **整个 watcher 不存在** ⇒ **`live2d-ai.toml` 与 `.env` 在首次运行期间都得不到热重载**
⇒ **本批把该 P1 的影响面从「一个文件」确认为「两个文件」**（更广，且两条都是外部热修的关键入口）。
另 `:56` 启动时 `match secrets::refresh_from_disk()` 并把**路径写进错误消息** ⇒ 首次加载失败可定位。

### 顺带记一条**不可达的隐患**（备忘，不记发现）
`:123` `if let Ok(mut g) = SNAPSHOT.write()` —— **写锁结果被忽略**，若锁已中毒则
「刷新静默不生效、但仍返回 `Ok(n)`」。**中毒在本处不可达**（临界区只有一次 move，不 panic），
故不记发现；记此是因为本仓已因锁中毒付出过代价（F-0006-03：`mod_registry` 的
`lock().unwrap()` 让 HTTP 服务整体挂掉）。

## 未核实项
1. `parse`（:62）本体未读 —— `.env` 的引号/注释/行尾处理（**值得读**：它决定「用户手写的 .env 能不能被正确解析**）
2. `is_valid_key`（:97）/ `known_keys`（:163）/ `env_file_path`（:39）未读
3. `performance/plan.rs`(857) / `llm.rs`(530) / `conversation/{engine,mod,worker}.rs` 未审
4. `sse.rs`(361) / `audio/{resample,rms}.rs` / `error_code.rs` 未审
5. `patch.rs` 中段各段三态未逐段核；B0120 的「是否别处 chmod 过 toml」仍未核
