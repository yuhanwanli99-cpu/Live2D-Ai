# BATCH-0225 · ⭐ **`ModLogger` 已经是唯一日志入口** ⇒ F-0224-01 修法 (b) 的成本**比我说的小**

Phase 1 · 域覆盖 · `mod-system/src/services.rs`（`ModLogger` 本体 —— F-0224-01 建议 (b) 的改动面）

## 跑的命令（全部只读）
```
sed -n '130,152p' mod-system/src/services.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**修正我自己对修法成本的估计**
```rust
inner: std::sync::Arc<LogFn>,                 // Fn(Level, &str)
pub fn info(&self,  msg: &str) { (self.inner)(log::Level::Info,  msg); }
pub fn warn(&self,  msg: &str) { (self.inner)(log::Level::Warn,  msg); }
pub fn error(&self, msg: &str) { (self.inner)(log::Level::Error, msg); }
```
⇒ ⇒ ⭐ **`ModLogger` 已经是唯一日志入口**（三个方法都是一行转发到同一个闭包）
⇒ ⇒ **不需要「收敛入口」——它已经是了**
⇒ ⇒ 修法 (b) 只是**加一个「按字段记」的入口**：传 `&ModSettingField` + 值，
**由契约决定「secret 字段只记 key 与长度」**
⇒ ⇒ 而宿主那侧（`mod_registry.rs:402`，B0198 已核）**也已经是那一个闭包**
⇒ ⇒ **改一处即可覆盖 8 个 Mod** ⇒ **成本比我在 B0224 写的「更彻底」小得多**

### ⚠ 但修法的**形状**必须避开「启发式」陷阱
⇒ **不要**在 `inner` 处加「按字符串模式猜密钥」的过滤器 —— 那是**猜**，
而 B0164 刚教过「保守启发式 + 声明误判代价」的代价有多贵
⇒ ⇒ ⭐ **正确的形状是「类型」**（结构化字段 + 值 ⇒ 由**契约**决定记什么），**不是「文本模式」**
⇒ 这与 B0213 的 `.filter()` 教训**同源**：**在转换/传输之前按类型挡住，别在事后按文本猜。**

## 未核实项
1. `mod-system` 余 8 文件未读（`session.rs` 476 · `topics.rs` 164 · `settings.rs` 余段 · `factory.rs` 142 · `status.rs` · `registry.rs` · `descriptor.rs` · `error.rs`）
2. 加「按字段记」入口对**现有 8 个 Mod 的调用点**的改动量未估（B0201 核过 50 处 `logger.` 调用）
3. Mod crates 逐文件覆盖率（19/61）
4. `dev_tools_section.dart` 余面(1874) · `tokens.dart` 余 ~600 行 · `design_tokens_test.dart` 余 ~32 断言
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **前端 `.dart` 183 未读（最大面）**
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
