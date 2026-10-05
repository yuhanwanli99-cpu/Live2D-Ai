# BATCH-0007 · 密钥端点 + 模型 registry 写回 + 路径工具

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`

## 读过的文件（来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/env_routes.rs` — 380（**生产段 1-182 全读**；:183 起是 `#[cfg(test)] mod tests`，未逐行读）
2. `crates/live2d-ai-desktop/src/web_api/models_routes/registry.rs` — 312（全读）
3. `crates/live2d-ai-desktop/src/web_api/models_routes/util.rs` — 134（全读）

合计 3 文件 / 826 行（生产段 628 行全读）。
**`dto.rs` 从本批移出**（未读即不计入），顺延到 BATCH-0008。

## 为验证调用链而读的清单外文件（只读片段）
- `secrets.rs:178-220`（BATCH-0002 已读；本批复核 `write_key` → `refresh_from(path)?` 的尾部）
- `mod.rs:306-308` 头注（`default_store` 的损坏降级：stderr 警告 + 空 registry）

## 跑过的命令（全部只读）
```
grep -rn "pub fn write_key" -A 25 crates/live2d-ai-runtime/src/secrets.rs
grep -n "default_store" crates/live2d-ai-desktop/src/web_api/mod.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 红线 R（秘密）：✔✔ `GET /api/v1/env` 逐字段确认**只回** `section` / `key` / `set`
  （env_routes.rs:48-64、:88-105），**无值字段**；`PUT` 的白名单取自
  `AppSettings::declared_key_envs()`（:127-128）——与 `GET` 同一份真源，不各抄段清单；
  日志只记 key（:150 `tracing::info!(key = %req.key, set, "env 写入成功（值不记录）")`）；
  错误信息不含 value（:146 注释 + :118-121/:129-143 的实际文案）。**本批红线全过。**
- 维度 A（真源唯一性）：✔ 模型 registry 路径只有一处定义（`model_root::registry_path`），
  相对路径存储 + 绝对路径每次派生（registry.rs:14-17），避免 WSL 迁移后失效
- 维度 G（路径安全）：✔✔ `normalize_relative_id`（registry.rs:40-63）是严格白名单
  `[a-zA-Z0-9_-]`，拒 `/` `\` `..` 前导 `.` NUL 与非 ASCII——比通用 `normalize_rel` 更严，正确
- 维度 D：`compute_dir_size` 显式栈遍历（util.rs:59-78），`symlink_metadata` 不展开外指，
  不会因符号链接成环 ✔
- 维度 E：`deny_unknown_fields` 与「只增不改」的兼容诉求**冲突** → F-0007-01
- 跨文件重复 → F-0007-02

## 新发现（对已有发现的派生影响，不另立条目）
**F-0002-01 的第二条受害者**：`handle_put` 的键名白名单读的是 `settings` 快照
（env_routes.rs:127，由 dispatch.rs:100 的 `status_ctx.settings_snapshot()` 喂入）。
在没有 file_watcher 的首次配置路径上（F-0002-01），用户若**手改** toml 新增一个
`api_key_env`，前端从 `GET /api/v1/env` 拿不到该键（快照旧），直接 `PUT` 也会被
`400 unknown_key` 拒（:129-143）。即「手改配置不生效」这条缺陷不只影响热重载，
还影响密钥写入的键名白名单。→ 记入 F-0002-01 的影响段（Phase 4 复核时一并回填）。

## 未核实项
1. `models_routes/mod.rs::default_store` 的损坏降级实现未逐行读（只读 mod.rs:306-308 头注）——
   F-0007-01 的「清空」结论依赖该头注，**标未核实**，留 BATCH-0008 确认。
2. env_routes.rs:183-380 的测试段未读。

## 本批新增
P0 0 · P1 0 · P2 1 · P3 1
