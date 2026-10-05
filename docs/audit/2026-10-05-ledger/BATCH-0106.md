# BATCH-0106 · `DisabledStaging`：**「默认关」是结构性的，不是布尔判断**

Phase 1 · 域覆盖 · Mod 根第 3 批

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-director/src/staging.rs` — 150（**全读**：trait :31-45 · `DisabledStaging` :47-60 ·
   `StagingSetup` :62-87 · `assemble` :117-150）

## 跑过的命令（全部只读）
```
grep -n "^pub enum|^pub struct|^impl|^    pub fn |^    fn |^fn |^pub fn " staging.rs     # 不截断
sed -n '44,88p' staging.rs
sed -n '117,150p' staging.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**0 条新发现**；红线 R 的 staging 路径**端到端核实**
### ① 「默认关」用**空对象（null object）**实现，不是「先判断再决定发不发」
```rust
pub struct DisabledStaging;                                   // :48 —— **没有 HTTP 字段**
impl StagingClient for DisabledStaging {
    fn enabled(&self) -> bool { false }                        // :51
    fn kind(&self) -> &'static str { "disabled" }             // :54
    fn complete(&self, _system, _user, _timeout_ms) -> Option<String> { None }   // :57
}
```
⇒ 它**结构上不可能发请求**（对象里没有客户端）。
⇒ 且**两个「没接上」的构造器都注入它**：`disabled()`（:75）与 `degraded(reason)`（:83）
⇒ 头注 :72 的「与『默认关』**逐字一致**」因此是**类型保证**，不是纪律。

### ② `assemble`（:119-150）三道闸才可能出现请求能力
```rust
if !config.staging_enabled { return StagingSetup::disabled(); }              // :120-122
match OpenAiStagingClient::new(base_url, model, api_key_env, timeout) {
    Ok(client)  => { … Box::new(client) … }                                   // :129-147
    Err(reason) => StagingSetup::degraded(format!("{reason}（二路关闭，仅规则层）")),  // :148
}
```
⇒ **即使闸开了**、只要端点/模型非法（`new` 返回 `Err`），**注入的仍是 `DisabledStaging`**，
只是附带一句**可见**的降级原因 ⇒ 「开了闸但没接上」**也是安全的**。

### ③ 「没有密钥」的两种情形被**分开描述**，且**从不回值**
```rust
let note = if client.has_api_key() { None }
  else if config.staging_api_key_env.trim().is_empty() {
      Some("未配 staging_api_key_env：请求不带 Authorization（端点要求鉴权时会失败）")   // :134
  } else {
      Some(format!("staging_api_key_env={} 在 .env / 进程环境里查不到值：请求不带 Authorization",
                   config.staging_api_key_env.trim()))                        // :139-140
  };
```
⇒ **「没配变量名」与「配了但查不到值」被区分**（排障时这两者要查的地方完全不同），
且 note 里**只有变量名、绝无值** ⇒ 与宿主 `has_api_key: bool` 同一纪律。

### ⭐ 正面模式 P3 的**第 5 个样本，且是最强的一档**
前四个（`normalize_resource_ref` 拒绝 `..` · `guess_format` 先查长度 · `alpha_coverage` 返哨兵 ·
`FACIAL_PARAMS` 显式小词表）都是**运行时的更严分支**；
本例更进一步：**把「关」实现成一个没有该能力的对象**，于是「关」不需要分支来保证。
⇒ **可提炼（写进 P3）**：**当「关掉某能力」可以被建模为「不存在该能力」时，
优先建模而不是加判断** —— 前者由类型保证、后者由纪律保证。

## 未核实项
1. `staging.rs::build_user_prompt`（:100-117）未核 —— 它组装的 prompt 里**是否可能带用户正文**
   （若有，`:130-133` 那条「日志不写正文」只管日志，这个 prompt 是**要发出去的**，属正常）
2. `OpenAiStagingClient::with_api_key`（staging_http.rs:116）—— **显式注入密钥**的构造
   （注释说是「测试 / 未来 host 替身」）**是否只被测试调用**未核（若生产也用它，密钥来源就多了一条）
3. `director` 其余 6 个源文件（`arbiter` / `decision` / `ledger` / `plan` / `presets`）未读
4. 其余 7 个 Mod crate 实现本体未读；`tests_staging.rs`(683) 断言体未读

## 本批新增
**0 条**
