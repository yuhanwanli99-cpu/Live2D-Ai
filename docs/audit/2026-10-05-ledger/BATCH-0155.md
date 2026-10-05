# BATCH-0155 · `config.rs` + **`ApiSecret` 的 `Debug` 核实**（结掉 B0153 的未核实项）

Phase 1 · 域覆盖 · `live2d-ai-runtime`（收尾）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/config.rs` — 140（**全读** 1-50）
2. `crates/live2d-ai-runtime/src/secret.rs` — （定点 11-41：**`ApiSecret` 全类型**）

## 跑的命令（全部只读）
```
wc -l config.rs ; grep -n "pub enum Tri|match self|Tri::" config.rs      # 零命中（Tri 在 patch.rs，不是漏搜）
sed -n '1,50p' config.rs
grep -n "struct ApiSecret" -B 6 -A 24 secret.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**B0153 留的未核实项 4 结清**，且是**最强的一档脱敏**
### ① `ApiSecret`：**`Debug` 不参与 derive** ⇒ 「被 derive 泄露」**被编译器挡住**
```rust
#[derive(Clone, Default, PartialEq, Eq)]      // :16 ← **刻意不含 Debug**
pub struct ApiSecret(String);

impl fmt::Debug for ApiSecret {               // :36
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // **绝不输出 self.0：长度、前后缀都不泄露。**      // :38
        f.write_str("ApiSecret(REDACTED)")                  // :39 ← **常量**
    }
}
```
三个可核的性质：
1. **derive 列表不含 `Debug`** ⇒ 日后有人手滑把 `Debug` 加进 derive ⇒ **编译错误**（impl 冲突），
   **不是静默泄露** ⇒ **约束跟着类型走，不靠人记得**（正面模式 P2 的最强实例）；
2. 输出是**常量** ⇒ **连长度与前后缀都不泄露**（注释逐字说明了这一点）
   ⇒ 比常见的弱实现（打印 `sk-…abcd` 或带长度的掩码）**更严**；
3. 头注 :14-15 直接写出它支撑的上位性质：「因此**把含密钥的配置结构体整体 `Debug` 打印也是安全的**」
   ⇒ 而 `config.rs:4` 与 :10 正是**对 `LlmConfig` 派生了 `Debug`** —— 依赖的就是这条。
⇒ **记一条正面模式 P10**：**「不许 derive 出来」比「记得别打印」强** ——
把禁止项**编码进类型定义**（derive 列表 + 手动 impl），让违反者**编译不过**。

### ② `config.rs` 里的「默认值只由一处决定」= P2 又是同一个形状
- `:19-23` `max_tokens`：「**0 = 不限制**，请求体里省略该字段」+「已由
  `settings::LlmSettings::effective_max_tokens` **解析过默认值**，所以这里拿到的是**最终生效值**」
- `:33-34` `new()` 构造时 `max_tokens: 0` + 注释「测试/便捷构造**默认不限制**；
  **生产路径由 settings 解析注入**」
⇒ 「什么值生效」这个决定**汇合在 `settings` 一处**；便捷构造的 `0` 被**显式标记为非生产路径**。
⇒ 与 B0122 核的 `apply_patch`（三态 + 合并后校验）、B0152 核的 `join_endpoint` 同属 **P2**。
⇒ ⭐ 而 B0129 记的 **F-0020-01（P1，`MAX_TOKENS = 1_024`）就在这个值的**生产注入点**上 ——
本批确认了**机制与注入点都对，只有那个常量的值不对**（与 B0130 的结论一致）。

## 未核实项
1. `config.rs:51-140` 未读（`TtsConfig` / `AudioSpec` / 其余测试）
2. `sse.rs` 余段（约 200 行：消费侧与 `[DONE]` 判定）未读
3. `plan.rs` 余 ~700 行未读（分段算法 / `PlanError` 全族 / `json_schema_strict` 余段 / `action_cue_payload`）
4. `llm.rs` 余 ~360 行未读（`ChatChunk` 解析、错误映射）
5. `performance/client.rs` 余 ~540 行未读（`request` 主体、structured 降级、wire 回归测试体）
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
