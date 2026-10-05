# BATCH-0013 · mods.json 原子写回（红线 R）+ web_api 收尾

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`（**web_api 结项批**）

## 读过的文件（来自 `git ls-files`）

1. `crates/live2d-ai-desktop/src/mod_registry.rs` — 1362（**读 190-509 生产段 + 688-712
   `parse_mod_config` + 938-987 manifest 往返测试**；测试段其余未读）
2. `crates/live2d-ai-desktop/src/web_api/cli_entry.rs` — 769（**读 604-643**
   `mods_manifest_for_web` / `mods_path_for_web` / `default_mods_manifest` 头注）
3. `crates/live2d-ai-desktop/src/web_api/tests_reload.rs` / `tests_p0c.rs` — 测试名已在前批枚举

`tests_dev_mode.rs`(328) / `tests_mod.rs`(266) / `tests_ws_client.rs`(257) **未读**——
它们是 web_api 最后的三个未审文件，顺延 BATCH-0014 与结项一起处理。

## 跑过的命令（全部只读）
```
grep -n "fn persist_manifest|fn start_all|fn load_manifest|fn manifest_path|struct ModManifest" mod_registry.rs
grep -n "fn parse_mod_config" -A 25 mod_registry.rs
grep -rn "未知 id|unknown_id|保留|preserve" mod_registry.rs
grep -n "read_to_string|from_str|manifest_path|fn new|with_manifest_path|ModRegistry::new" mod_registry.rs
grep -n "ModRegistry::new|with_manifest_path|default_mods_manifest|mods.json" cli_entry.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批核心：红线 R 明列项「`mods.json` 原子写回**不得丢未知 id**」**被违反**

`persist_manifest`（mod_registry.rs:248-285）用**内存注册表**重建整份文档：
```rust
let mut mods = serde_json::Map::new();
for (id, e) in &self.entries {                       // :253 —— 只遍历在册 factory 的 id
    mods.insert((*id).to_string(), serde_json::json!({ "enabled": e.enabled, "config": e.config }));
}
let doc = serde_json::json!({ "mods": mods });        // :259 —— 全新文档，无「原文件」参与
... plan_atomic_write(path, &text) → std::fs::rename  // :274-276 —— 整体覆盖
```
而**读侧从未把未知 id 收进内存**：`mods_manifest_for_web()`（cli_entry.rs:613-622）
把整份 `mods.json` 读成一个 `Value` 传给 `ModRegistry::new`，但 `new` 内部只对
**每个在册 factory 的 id** 调 `parse_mod_config(manifest, desc.id)`
（mod_registry.rs:190 → :688-695 的 `manifest.get("mods").and_then(|m| m.get(id))`）。
⇒ **未知 id 在 `new` 那一刻就被丢弃**，全仓没有任何地方保留它，
`persist_manifest` 自然写不回去。

**完整调用链**：
`cli_entry.rs:182 / :296` `ModRegistry::new(AVAILABLE_MOD_FACTORIES, &mods_manifest_for_web())`
→ `.with_manifest_path(mods_path_for_web())`（:290 / :297）
→ 用户点任意 Mod 的「启用/停用/重启/保存配置」
→ `mods_routes.rs:307-312` → `mod_registry.rs:422/432/441/463`
→ `persist_manifest()` → `rename` 覆盖 `mods.json` → **未知 id 及其 config 永久消失**。

**可达性不是假想，本仓有先例**：cli_entry.rs:640-642 就在我读的这一段里写着
「**同版废除 `local-llm` 启动**……crate 暂留仓库但不再注册、不再编译进 binary」。
⇒ rc.0 上配置过 `local-llm`（含 `api_key_env` / 模型名等真实用户数据）的用户，
升级到 rc.1+ 后**第一次点任意 Mod 开关**，该条目连同其 config 一次性丢失。

**测试覆盖（§12.1）**：`reload_config_persists_manifest`（mod_registry.rs:944-966）
是唯一覆盖 manifest 往返的测试，它的名字写着「M1：reload_config…也写回 config，
且**不丢其它 Mod**」（:942），但它的起点 manifest 是
`&serde_json::json!({})`（:951）——**空文档**。
⇒ 「不丢其它 Mod」实际只验证了「不丢**在册** Mod」，而「未知 id 保留」这条
真正要紧的语义**结构上无法被这条测试捕获**。另两条 manifest 测试（:829 / :844）
同样从已知 id 出发。**零覆盖。**

## 维度覆盖（本批）
- 红线 R（Mod 边界与秘密）：✘ 未知 id 丢失 → F-0013-01（P1）
- 维度 B：`persist_manifest` 是 **best-effort**（磁盘失败只 `tracing::error!` 并继续，
  mod_registry.rs:246-247 写明理由「内存状态已变更，HTTP 不该因为磁盘只读就谎报操作失败」）
  ——这个取舍**判定合理**（内存已改、谎报失败会让开关看起来没动），不记发现
- 维度 A：`settings_specs` 静态 schema 在 `new` 里就填好（:201-208，含 `spec.validate()` 过滤），
  未启用 Mod 也能拿到表单 ✔

## 未核实项
1. `mod_registry.rs` 的 510-687、713-938、988-1362 未读（含 `command` / 订阅 / Drop 路径）。
2. `tests_dev_mode.rs` / `tests_mod.rs` / `tests_ws_client.rs` 未读 ⇒ **web_api 尚未 100% 审完**
   （35/49 → 这三个读完才是 38/49；余下 11 个是 `models_routes/tests_*.rs` 与
   `settings_routes/tests.rs` 等大测试文件，Phase 2 维度 J 横扫时统一处理）。
3. 登记在案的 M/L 侧条目本批只判定了「未知 id 保留」这一条；S2/S3/S5 未复核。

## 本批新增
P0 0 · **P1 1** · P2 0 · P3 1
