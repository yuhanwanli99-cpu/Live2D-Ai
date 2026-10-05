# BATCH-0153 · 表演层密钥：**两个独立机制**保证它不进日志

Phase 1 · 域覆盖 · `live2d-ai-runtime/performance/client.rs`（定点）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/client.rs` — 652（定点 166-176 类型与 `Debug`、:183-206 构造、:290 下发）

## 跑的命令（全部只读）
```
grep -n "bearer_auth|Authorization|header(" performance/client.rs
grep -n "api_key" performance/client.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；红线 R「密钥不进日志」的**第三处独立兑现**
### 机制一：类型是 `ApiSecret`，不是 `String`
`:166` `api_key: Option<ApiSecret>` · `:206` `api_key: Option<ApiSecret>` ·
`:290` `request = request.bearer_auth(key.expose_secret())`
⇒ **放进请求前必须显式 `expose_secret()`** ⇒ 「不小心 `format!("{key}")` 进日志」需要**主动**写一次 expose
⇒ 与 B0010 核过的 `LlmConfig::with_api_key`（F-0006-01 涉及处）同属 `ApiSecret` 这一套。

### 机制二：`Debug` 实现**只输出布尔**
`:172-177` 的 `Debug`：`.field("url", &self.url.as_str())` +
`.field("has_api_key", &self.api_key.is_some())`
⇒ **URL 与一个布尔**；**值永不出现**。而这个 `Debug` 是**错误类型**的实现
（`RequestError` 一类，会被日志/诊断消费）⇒ 头注 :10 声称的「只进 `Authorization: Bearer` 头，
**不进 URL / body / 日志**」在**两个独立机制**上成立。

⇒ ⭐ 至此红线 R「不进日志/状态面」已有**三处互不依赖**的兑现：
1. B0009：`GET /api/v1/env` **永不回值**（端点层）
2. B0105：Mod `state_json` 只回 `api_key_set` **布尔**（Mod 状态面）
3. 本批：`ApiSecret` **类型** + `Debug` **只输出布尔**（客户端错误面）
⇒ 三处**都不依赖另外两处** ⇒ 任何一处被改坏，另外两处仍然拦着。

## 未核实项
1. `client.rs` 余 ~540 行未读（`request` 主体、structured 降级路径、wire 回归测试体）
2. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）**断言体**未读
3. `mod.rs` 余 ~350 行（`PerformanceStats` 对外面、Budget 注入点）· `llm.rs` 余 ~360 行 ·
   `config.rs` · `sse.rs` 余段 · `plan.rs` 余 700 行未读
4. `ApiSecret` 自身的 `Debug`/`Display` 实现未读（B0010 只核过它在 LlmConfig 里的用法）
5. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
