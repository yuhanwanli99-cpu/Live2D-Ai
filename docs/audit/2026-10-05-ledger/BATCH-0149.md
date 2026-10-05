# BATCH-0149 · **「名字钉住的规则」确实被断言了**，且测试**自己隔离前提**

Phase 1 · 域覆盖 · `web_api` 测试面（补 B0148 留的「名字 ↔ 断言」这一步）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-desktop/src/web_api/voice_routes_tests_gate.rs` — 209（定点 198-208：
   `token_check_precedes_gate` **断言体**）

## 跑的命令（全部只读）
```
grep -n "fn token_check_precedes_gate" -A 24 voice_routes_tests_gate.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；两处核验通过，第二处是本批真正的收获
### ① 被按名钉住的顺序规则，**断言体真的在测它**
```rust
if std::env::var(TOKEN_ENV_VAR).is_ok() { return; }                    // :199-201
let ctx = ctx_voice(serde_json::json!({"token": "cfg-secret"}));        // :202 在 **Mod config** 里配 token
let resp = call_auth(&ctx, r#"{"text":"你好"}"#, None).unwrap();      // :203 **不带** Authorization
assert_eq!(resp.status_code(), StatusCode(401),
           "token 已配置且缺失 → 401，**先于总闸判定**");               // :204-208
```
⇒ 若**顺序反了**（先判门），返回会是 403/400 ⇒ **本断言会红**
⇒ 所以 B0148 记的「顺序在代码里看不出来、这里说出来了」**成立** —— 名字与断言**一致**。

### ② ⭐ 而 `:199-201` 是**P7 用在测试自己的前提上**（本批真正的收获）
```rust
if std::env::var(TOKEN_ENV_VAR).is_ok() { return; }
```
⇒ 若**跑测试的机器**进程环境里已有 `VOICE_INPUT_TOKEN`，按红线 R 的优先级
（**`.env`/环境 > Mod config**，B0104/B0124 已核）它会**压过**测试在 config 里配的 token
⇒ 测试的前提（「token 来自 config」）**不成立**，它会去测**另一件事**。
⇒ 处置：**显式跳过**，而不是照样断言一个不成立的前提。
⇒ **两条要点**：
1. 跳过的**判据**正好是那条优先级规则 ⇒ **与被测规则同源**；
2. 用 `return` 做**运行期**跳过（比 `#[ignore]` 更精确：`#[ignore]` 只能静态标记，
   而这里的干扰条件是**环境相关**的）。
⇒ 可提炼：**正面模式 P8 —— 测试要显式隔离自己的前提，且在前提不成立时跳过而不是硬断言。**

## 未核实项
1. 余 4 个测试文件未读（`voice_routes_tests_say.rs` 148 · `models_routes/tests_models_{core,handlers,common}.rs` 730）
2. `voice_routes_tests.rs`(793) 的 helper（`call` / `call_auth` / `dummy_ctx` / `blackhole_endpoint`）
   **断言体**未读 ⇒ 「用例名 ↔ 断言」这一步**只核了 1/9**
3. `live2d-ai-runtime` 余面（`client.rs` 652 / `config.rs` / `sse.rs` 余段）· `plan.rs` 余 700 行未读
4. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
