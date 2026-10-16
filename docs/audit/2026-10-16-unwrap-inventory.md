# 生产代码 `unwrap` / `expect` 清点（2026-10-16）

> 状态：**活**。D7 只清点、**不改**（「不批量改」；有一条会在请求路径上 panic 的，
> 单独立片，见 §3）。本文件属 `docs/audit/`，**不计入** 45,000 行 docs 预算。

## 1. 口径与数（可复算）

范围：`crates/*/src/**/*.rs`（Rust 侧；Dart 没有 `unwrap`/`expect`，其近似物是 `!`，
不在本片口径内）。**生产** = 扣掉两类测试载体：

1. `#[cfg(test)] mod x;` 单独引用的模块文件（本仓 64 个这类文件，与 `code-stats` 报告
   §5「仅被 `#[cfg(test)] mod x;` 引用的 src/ 测试模块文件」同一口径）；
2. 名字即测试的载体（`tests*.rs` / `*_tests*.rs` / 目录 `tests/`、`core_tests/`、`test_support`）。

判据：逐文件扫 `\.unwrap()` / `\.expect(`，并**剥掉文件内联的 `#[cfg(test)]` 花括号块**。

| 量 | 值 |
|---|---:|
| `crates/*/src/**/*.rs` 总数 | 277 |
| 其中「只被 `#[cfg(test)] mod x;` 引用」的测试模块文件 | 64 |
| 生产候选文件 | 185 |
| **生产命中 `unwrap`/`expect`** | **47 处 / 29 个文件** |

**方法边界（如实标注）**：剥 `#[cfg(test)]` 用的是朴素花括号配平，
`#[cfg(test)]` 覆盖到**非连续**条目（宏展开、`include!`）时可能误判；
另外 47 里有 **1 处是文档注释**（`conversation/mod.rs:58` 的 `//! .unwrap();` 示例），
实际可执行命中 **46**。逐条明细见 §2 各类。

## 2. 按形状分类（47 处的分布）

| 类 | 处数 | 会不会在请求路径 panic | 代表 |
|---|---:|---|---|
| A. 常量 HTTP 头 `.expect("Content-Type header")` | 23 | **不会**：`Header::from_bytes` 对常量不失败 | `web_api/*/mod.rs`、`flutter_app.rs`、`wasm_assets.rs`、`ws.rs:146,150,154,209` |
| B. 本地不变量 `expect("刚刚保证…" / "just set" / "checked" / "non-none")` | 8 | 不会（同函数内已在上一行判定） | `mod_registry/persist.rs:40,53`、`events.rs:137`、`persona/card.rs:60`、`persona/sessions.rs:193`、`models_routes/handlers.rs:352`、`cli_entry.rs:157`、`runtime/audio/queue.rs:76` |
| C. 装配期 fail-fast（线程 / runtime / 常量表） | 7 | 不会「因用户输入」；启动期失败即快败 | `supervisor/spawn.rs:142,157`、`ws/connection.rs:116`、`runtime/audio/spec.rs:59`、`l2d-wasm-demo/src/main.rs:970`、`runtime/settings.rs:799`、`audio/ring.rs:450`（测试入口 `detached_pair_for_tests`） |
| D. 生成阶段不变量 `expect("生成阶段必须以 TurnReport 返回")` | 1 | 不会（同轮必然返回；见 `supervisor/turn.rs:351`） | `supervisor/turn.rs:351` |
| E. **共享锁毒化 `.lock().unwrap()` / `.expect("… mutex")`** | 7 | **会**：锁一旦毒化，之后的每一次都 panic | 见 §3 |
| —（误报）doc 注释里的示例 | 1 | 不适用 | `conversation/mod.rs:58` |

## 3. 会在请求路径上 panic 的候选（→ 单独一片 W1）

两条链**都在 HTTP 请求路径上**，且**幂等放大**：**一次 panic 之后，同一个锁的每一次
取用都 panic**（`std::sync::Mutex` 毒化语义）。也就是说故障不是「一次 500」，
而是「这条链从此全线 500 / 线程直接崩」。

### E1 `Broadcaster` 内部锁（WS 订阅 / 广播）

- 位置：`crates/live2d-ai-desktop/src/web_api/ws/broadcaster.rs:92`（`try_subscribe`）、
  `:108`（`unsubscribe`）、`:133`（`broadcast`）；
- 触发：任何一次在持锁临界区里 panic（临界区里有 `Vec::push` 等可能分配的操作），
  或**别处**已经 panic 而没有解包锁；
- 后果：`GET /api/v1/ws` 的连接注册、断线清理、**全部广播**（含 `action_cue`、
  `text_delta`、音频帧）此后每次都 panic。

### E2 Mod runtime 槽位锁（`runtime_state` / `command` / `enable` / `disable`）

- 位置：`crates/live2d-ai-desktop/src/mod_registry/registry.rs:148`（启动写入）、
  `:274`（`disable` take）、`:277`（放回），`crates/live2d-ai-desktop/src/mod_registry/events.rs:79`（worker `shutdown`）；
- 触发：某个 Mod 在自己的锁内 panic（Mod 是第三方边界，`on_event` 里 panic 就够）；
- 后果：`GET /api/v1/mods/{id}/state`（`runtime_state` 用的是 `try_lock`，**这里反而是安全的**）、
  `POST /api/v1/mods/{id}/{enable,disable,restart}` 会 panic —— 也就是**用户再也停不掉那个 Mod**。

**处置（本片不做）**：按计划书 D7「单独再开一片，不顺手改旁边的」——
名称为 **W1「共享锁毒化 → 请求路径 panic」**：把 `lock().unwrap()` 换成
`lock().unwrap_or_else(|e| e.into_inner())`（本仓已有先例：`events.rs` 的
`sink.lock().unwrap_or_else(|e| e.into_inner())`；`spawn_mod_op` 也用 `poisoned.into_inner()`），
并补「毒化后仍可用」的回归。**改它的前提是维护者点名**。

## 4. 明确没做（照计划书）

- **不批量改**任何一处；A–D 类保持原样；
- **不顺手改旁边的**：W1 只清点 + 立片，本次不动 `broadcaster.rs` / `registry.rs`（本片其它
  交付对 `registry.rs` 的改动属 S-003 / S-006 的授权面，与本条无关）；
- 不新增 lint 门禁、不改 `clippy` 配置。

## 5. 复算命令

```bash
# 候选文件（同 §1 口径）：
ls crates/*/src/**/*.rs crates/*/src/*.rs | grep -v "/tests/" | grep -v "tests_" | grep -v "_tests.rs$"
# 逐条：
grep -rn "\.unwrap()\|\.expect(" crates/*/src --include=*.rs
```
