# BATCH-0119 · ⭐ **换根 + 换手法**：`settings.rs` 写盘路径 —— 老代码「查内部」的第一批，**四处全对**

Phase 1 · 域覆盖 · **换根到 `live2d-ai-runtime`（老代码区）**，按 CONSOLIDATION-12 §③ 的判据
**老代码 ⇒ 查内部**（缺日志 / 析取恒真 / unwrap / 整份重建 / 边界值）。
`settings.rs`(826) 是该区**最大且内部未审**的文件，且 AGENTS.md 变更历史记着一条**真实事故**：
「⑤ **保存设置会抹掉配置文件全部注释**（`toml::to_string` 重生成文档）—— 当天真的发生一次，
改用 `toml_edit` **就地改值**（`merge_into_toml`）」⇒ 本批去核：**那次修复是否覆盖了所有写盘点**。

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/settings.rs` — 826（定点 571-578 `load_from_path` ·
   700-755 `to_toml_string` / `merge_into_toml` / `to_toml_string_merging` · 758-765 `sync_item`）
2. `crates/live2d-ai-runtime/src/` 目录规模 — 17 文件 / 9924 行（本批只读 `settings.rs`）

## 跑过的命令（全部只读）
```
git ls-files 'crates/live2d-ai-runtime/src' | grep -v tests | xargs wc -l | sort -rn
grep -n "toml_edit|to_string_pretty|toml::to_string|DocumentMut|merge_into_toml|fs::write|fn save" settings.rs
sed -n '700,712p' settings.rs
grep -rn "\.to_toml_string(|::to_toml_string(" crates/ --include=*.rs        # 穷举调用点
sed -n '744,765p' settings.rs
grep -n "fn load_from_path" -A 14 settings.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；**四处核验全部通过**（正面样本密集）
### ① 写盘点已被**收进一处**，且理由点名了系统性风险（正面）
```rust
/// 合并进**磁盘上现有**的 TOML；读不到就当作「没有现有文件」。
///
/// 这是写盘点**该用的入口**——它把「读现有 → 合并」这一步收在一处，
/// 避免三个写盘点各写一遍（**漏一个就又是一次注释清空**）。          // :747-750
pub fn to_toml_string_merging(&self, path: &std::path::Path) -> String {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    self.merge_into_toml(&existing).unwrap_or_else(|_| self.to_toml_string())   // :751-755
}
```
⇒ **「漏一个就又是一次注释清空」** 被写成头注 ⇒ 这是一次**针对系统性风险**的结构性收口，
不是「顺手改了一处」。

### ② 那个「重新生成整份文档」的序列化器**仍在**，但**只作兜底/中间表示**（核过全部 3 个生产调用点）
穷举 `to_toml_string` 的调用点：
| 行 | 用途 | 是否写盘 |
|---|---|---|
| `:734` | 现有文本**为空**时（没什么可保留）⇒ 重生成 | 合理 |
| `:740` | 把自身解析成 `DocumentMut` 作**中间表示** | **否**（不落盘） |
| `:754` | **merge 失败**的兜底 | ⚠ 唯一可能写盘的一条 ⇒ 见 ③ |
（另 5 处在 `settings_tests.rs` / `patch_tests.rs`。）

### ③ 兜底分支**不可达**，因而不会毁掉用户文件（关键核验）
`:754` 的兜底会在 `merge_into_toml` 失败时**整份重生成并覆盖**，其唯一危险输入是
**语法坏掉的用户 TOML**。而 `:571-578`：
```rust
pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, SettingsError> {
    let text = std::fs::read_to_string(path).map_err(|source| SettingsError::Io { … })?;  // 读失败 ⇒ Err
    Self::from_toml_str(&text)                                                              // 解析失败 ⇒ Err（**不回退缺省**）
}
```
⇒ **从磁盘出发的任何写路径都必然先成功解析过该文件** ⇒ `merge_into_toml` 的解析错误分支
在这些流程里**到不了** ⇒ 兜底不会覆盖一个语法坏掉的用户配置。
⇒ **不记发现**（可达性不成立）；但**记一条备忘**：这条安全性**依赖 `load_from_path` 的严格性** ——
若将来有人给加载路径加「解析失败回落缺省」（那本身是个常见优化），
`:754` 就会**立刻变成静默覆盖用户配置的路径**。**这是本条最脆的一环，已写进 STATE。**

### ④ `sync_item` 的实现理由**对 `toml_edit` 语义的理解是准确的**（正面）
```rust
/// `Item::Value` 分支刻意**改值而不是换键**——`toml_edit` 里「键自身的前缀注释」挂在 key 上，
/// **换键会把它们一起换掉**；就地改值只动值，键与它的注释都留着。     // :760-762
```
⇒ 这是**知道库 internals 才会写出的注释**，而不是「用 toml_edit 就完事」。

## 未核实项
1. **三个写盘点是否真的都走 `to_toml_string_merging`** —— 本批只核了该函数自身与
   `to_toml_string` 的调用点，**未逐个核调用方**（desktop 侧 `merge_into_toml` 的注入点、
   `PUT /api/v1/env` 的 `.env` 写入、Mod 的 `apply_settings` 写盘路径）⇒ **「漏一个就又是一次注释清空」
   这句头注的另一半尚未验证**，**这是下批第一件事**
2. `settings/patch.rs`(780) 内部未审（B0010 只读了 `PatchBody` 结构）
3. `performance/plan.rs`(857) / `llm.rs`(530) / `conversation/{engine,mod}.rs` 未审
4. `secrets.rs`(416) / `sse.rs`(361) / `audio/{resample,rms}.rs` 未审
5. `dialogue/{clean,sentence}.rs` 已审（早前批次）
