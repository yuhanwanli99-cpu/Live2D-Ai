# BATCH-0104 · 8 个 Mod crate 首审：**红线 R 的写侧与密钥侧都成立**

Phase 1 · 域覆盖 · ⭐ **换根：Mod 实现本体**（61 文件 / 24,665 行，此前 0 审）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-director/src/staging_http.rs` — （定点 94-120：生产构造与密钥解析）
2. `crates/live2d-ai-mod-memory/src/store.rs`(404 段命中) · `summary_store.rs`（仅 grep 定位）
3. 8 个 Mod crate 的 `fs` / `env` / 网络调用**全量 grep**

## 跑过的命令（全部只读）
```
grep -rn "std::env::var|env::var|std::fs::write|fs::write|File::create|read_to_string" crates/live2d-ai-mod-*/src/*.rs
grep -rn "reqwest|ureq|TcpStream|http::" crates/live2d-ai-mod-*/src/*.rs
grep -n "fn new" -A 18 crates/live2d-ai-mod-director/src/staging_http.rs
grep -n "api_key|key" crates/live2d-ai-mod-director/src/staging_http.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**0 条新发现**；红线 R 在 Mod 侧的两条通道都成立
### ① 写侧：**没有任何 Mod 直写宿主配置**
全量 grep 的 `fs::write` / `File::create` 命中**全部集中在 `memory` Mod 自己的 store**
（`store.rs` / `summary_store.rs`），写的是**它自己的 JSONL / 摘要 JSON**，
且是 `File::create(&tmp)` → rename 的**原子写**形态；`read_to_string` 的命中在 400+ 行处
**都是测试**（`unwrap()` 在测试上下文）。
⇒ **没有任何 Mod 触碰 `live2d-ai.toml` 或 `.env`** ⇒ 红线 R 的写侧成立。

### ② 密钥侧：唯一带**真实 HTTP 客户端**的 Mod（director）**只拿变量名**
```rust
pub fn new(base_url: &str, model: &str, api_key_env: &str, timeout_ms: u64) -> Result<Self, String> {
    let env_name = api_key_env.trim();
    let api_key = if env_name.is_empty() { None } else {
        live2d_ai_runtime::secrets::lookup(env_name)      // :110 ← **唯一受认可的查找**
    };
    Self::with_api_key(base_url, model, api_key, timeout_ms)
}
```
头注 :9 写明它读 `staging_api_key_env`、**不读** `live2d-ai.toml` 的 `[llm]`，
**「二路断了不影响主链」**（该路径「默认关」，B0040 已核它是零投递骨架）。
⇒ **Mod 手里只有一个变量名，值由宿主的同一个 `secrets::lookup` 解析**
⇒ 「密钥真源 = `.env`」与「Mod 不得持有密钥」**在实现层成立**。

### ⭐ 正面：**这是正面模式 P2 的第 5 个消费者**
Mod **复用**宿主的 `secrets::lookup`，而不是自己 `std::env::var`（那会绕过 `.env` 快照
⇒ 正是 AGENTS.md 记载过的那个坑：「界面上刚写了 key、链路还说没配置」）。
⇒ 「私有汇合 + 策略闭包」这个手法**不只用在宿主内部，也被 Mod 复用** —— 这是它最值得学的原因。

## 未核实项
1. **8 个 Mod crate 的实现本体基本未读**（本批只做了跨 crate 的**边界扫描** + director 一处定点）
   ⇒ `persona`(1004) / `director`(902) / `memory`(895) / `external-input`(847) / `voice-input` /
     `wallpaper`(729) / `template` / `local-llm`(830, 已废止) 的**主体逻辑**未审
2. `director` 的 staging **开关**实现（怎么保证「默认关」、谁能打开）未读
3. `memory` 的 `store.rs` / `summary_store.rs` 全文未读（只确认它写自己的数据）
4. 各 Mod 的 `settings_spec` 与实际读取的键**是否一致**（声明了但不读 / 读了但没声明）未核
5. 各 Mod 的**测试体**未读（BATCH-0104 之后应抽查 1–2 个）
