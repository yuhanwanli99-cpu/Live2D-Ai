# BATCH-0002 · 状态真源 + 配置热重载 + supervisor 槽位 + Flutter 静态托管

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`

## 读过的文件（全部来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/app_routes.rs` — 544
2. `crates/live2d-ai-desktop/src/web_api/file_watcher.rs` — 174
3. `crates/live2d-ai-desktop/src/web_api/supervisor_slot.rs` — 234
4. `crates/live2d-ai-desktop/src/web_api/flutter_app.rs` — 384

合计 4 文件 / 1336 行。

## 为验证调用链而读的清单外文件（只读片段，不计入本批审计结论）
- `cli_entry.rs:110-179`（`supervisor_opt` 的两条 `None` 分支）
- `cli_entry.rs:320-385`（`_file_watcher` 装配条件、退出回收顺序）
- `dto.rs:240-258`（`llm_status_from` 的 `has_api_key` 口径，作为 performance 段的对照）
- `secrets.rs:178-220`（`write_key` → `write_key_to` 尾部 `refresh_from(path)?`，确认 UI 写密钥后快照确实刷新）
- `env_routes.rs`（grep 命中行：`handle_put` 调 `secrets::write_key` + `is_set`）

## 跑过的命令（全部只读）
```
git ls-files 'crates/live2d-ai-desktop/src/web_api' | xargs wc -l | sort -rn
grep -rn "FileWatcher|watch_config" crates --include=*.rs
grep -n "supervisor_opt" crates/live2d-ai-desktop/src/web_api/cli_entry.rs
grep -n "fn llm_status_from" -A 18 crates/live2d-ai-desktop/src/web_api/dto.rs
grep -rn "pub fn write_key" -A 25 crates/live2d-ai-runtime/src/secrets.rs
grep -n "refresh_from_disk|secrets::" crates/live2d-ai-desktop/src/web_api/env_routes.rs
grep -n "pub fn current_epoch" -A 6 crates/live2d-ai-desktop/src/supervisor.rs
du -sh shell/flutter/build/web ; ls -laS shell/flutter/build/web | head -6
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- A 状态与真源唯一性：✔ `StatusContext` 是 settings 运行时唯一真源；`refresh_from_disk`
  （app_routes.rs:192）与 PATCH 路径（dispatch.rs:150）**确实共用同一函数**，无第二条更新路径 ✔；
  但**装配侧**有个真源断裂：file_watcher 的生命周期挂在 `supervisor_opt` 上，而不是挂在
  「是否已有 supervisor」这个运行时事实上 → F-0002-01
- B 生命周期/副作用：✔ `FileWatcher::drop` 置 stop_flag + join（file_watcher.rs:167-173），
  线程 100ms 轮询退出，最长 join ≈ 100ms + debounce；`_file_watcher` 是真绑定不会被提前 drop ✔
- C 竞态：✘ **未覆盖**——`ensure_after_patch`（supervisor_slot.rs:108 `try_get` → :142 `set`）
  与 file_watcher 的 `supervisor.reload()`（file_watcher.rs:146）之间无互斥；动态装配出的
  supervisor 被装进槽位时，file_watcher 线程仍持有**旧的** `Arc<SupervisorHandle>` 副本
  （本批成立的前提是 supervisor_opt 非空，故 F-0002-01 未触发时也会发生）→ 记入 STATE 候选池 C-4
- D 热路径：`resolve_build_dir()` 每请求重算候选（含 `current_exe()` + ancestors stat）
  → F-0002-06；静态资产全 `no-cache` 且无 ETag → F-0002-05
- E 契约一致性：✘ `performance.has_api_key` 与 `llm/tts.has_api_key` 两套口径 → F-0002-03；
  `last_fallback` 缺省值谎报 → F-0002-04
- G 安全：✔ flutter_app 复用 `normalize_rel` + canonicalize 双检（:228-241），
  非 GET 405，`/` 非 GET 交回 dispatch 不造第二套语义（:145-152）
- 红线 R（秘密）：✔ `PUT /api/v1/env` 链完整——`handle_put` → `secrets::write_key` →
  `write_key_to` 尾部 `refresh_from(path)?`（secrets.rs:219）→ dispatch.rs:105
  `ensure_supervisor_after_patch`，快照确实刷新，无需「重启才生效」
- 红线 K（离线优先）：✔ 本批 4 文件无任何外链；产物 47MB 本地自持

## 未核实项
1. C-4（reload 与动态装配的竞态）只做了静态推导，**未构造并发时序**——需门禁验证。
2. `performance` 段在前端如何渲染（是否真的会显示「上次回退原因」）未读——F-0002-04 的
   用户可见性标 未核实，置信中。
3. F-0002-05 的 47MB 为本机 `du` 实测；实际浏览器重载是否真的全量重传需门禁/浏览器验证。

## 本批新增
P0 0 · P1 2 · P2 3 · P3 2
