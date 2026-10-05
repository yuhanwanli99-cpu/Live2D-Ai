# BATCH-0217 · ⭐ `mod-template`：**「不要做什么」带着理由与去处** —— 模板的最高价值不在代码形状

Phase 1 · 域覆盖 · `mod-template/src/lib.rs`(183) —— 复制起点 + **活文档**

## 跑的命令（全部只读）
```
wc -l mod-template/src/*.rs ; sed -n '1,22p' mod-template/src/lib.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**一处值得学的模板写法**
### ① 头注就是一份**合规清单**，而它**指向别处的执行机制**（P2 形状）
```rust
//! # 不要做什么
//! - 不接动作通道：`ModServices.action_tx` **自 rc.2 起休眠**，请求会被 host 丢弃并**返回 `false`**
//!   （见 `docs/architecture/core-chain-baseline.md` §3.3）；                        // :18-19
//! - 不直接读进程环境/密钥：配置写回走 **`ModServices.apply_settings`（一等 API）**。  // :20
```
⇒ ⭐ **它没有复述规则，而是指向「规则在哪被执行」**：
动作通道的**休眠台账**（B0009 起我核过的那张表）与 `apply_settings` 的**一等化**（AGENTS rc.4）
⇒ ⇒ **规则若在一处改动，指向它的读者会自动跟着对** ⇒ **模板因此不会过期**
⇒ 而这**正是 P25 说的**「尽量让各处都是引用」在**文档层**的对应物

### ② 而「不要做什么」是**复制者最容易做错**的那部分
- **能看见**的（类名、字段、函数骨架）读者照抄即可
- **看不见**的（有一条休眠通道看起来能用、进程环境里确实有密钥）**只能靠文档说**
⇒ ⇒ **模板里最高价值的内容不是代码形状，而是这张「别这么做 + 为什么 + 去哪查」的清单**

### ③ 另两条禁令也都带机制
- 「settings 是**纯数据 schema**（Bool/String/Number/Select）——**禁止**注入 HTML/JS」（:16-17）
  ⇒ 且**这条禁令在面板层有类型背书**（B0105 已核 `settings_spec` 按 kind 渲表单）⇒ **不是靠自觉**
- 「**模板 crate 自身不注册**进 `AVAILABLE_MOD_FACTORIES` —— 注册表里只放真实能力
  （见 `main.rs` 的 `mod_count_is_*` 断言）」（:5-6）⇒ **连「别注册自己」都写了，并指出守它的断言**
- 以及**使用的步骤清单**（:3-4）：根 `Cargo.toml` → desktop 依赖 → `AVAILABLE_MOD_FACTORIES`
  → **数量/id 断言** → 文档一行 ⇒ 五步，**每一步都有守它的机制**

## 未核实项
1. `mod-template/src/lib.rs` 其余 ~160 行未读（含它的单测）
2. `strategy.rs` 余 ~300 行未读（`to_prefs_patch` 全文 / `reconfigure` 细节 / 自测）
3. Mod crates 逐文件覆盖率（13/61）
4. `design_tokens_test.dart` 余 ~32 条断言体 · `tokens.dart` 其余 ~600 行未读
5. `dev_tools_section.dart` 余面未读（1874 行）
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
