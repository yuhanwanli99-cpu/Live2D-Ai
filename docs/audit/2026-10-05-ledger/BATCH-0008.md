# BATCH-0008 · 模型库 default_store 装配 + handler 路径层

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`

## 读过的文件（来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/models_routes/mod.rs` — 344（全读）
2. `crates/live2d-ai-desktop/src/web_api/models_routes/handlers.rs` — 427（**读 1-329**；
   :330-427 是 `handle_delete` 尾部与错误响应 helper，未读）

合计 2 文件 / 771 行（读了 673 行）。
`dto.rs`（models_routes 与 web_api 各一份）与 `settings_routes` 顺延 BATCH-0009。

## 跑过的命令
无新增（全部为 read）。未跑任何 cargo / flutter / trunk / pnpm 命令。

## 回填：BATCH-0007 未核实项 1 —— **已确认**
`default_store()`（models_routes/mod.rs:250-253）→ `ModelStore::from_disk`（:149-183）：
- `NotFound` → 静默用空 registry（:157-159，正常首启路径）
- 其它 `Err`（含 JSON 解析失败）→ **备份**到 `<path>.corrupt-<unix_secs>`（`fs::copy`，:167）
  + `eprintln!` 告警（:168-173）+ **继续用空 registry**（:175-178）；
  若连备份都失败，**原文件保留**（:174-178）
- 注释自陈：`后续 persist 仍会覆盖 registry_path（损坏副本保留在 .corrupt-* 里）`（:163-164）

⇒ **F-0007-01 的链条闭合**：原文件内容不丢（`.corrupt-*` 里有副本），但**运行期模型列表为空、
`active_id` 丢失**，且下一次任意写操作会用空 registry 覆盖 `registry_path`。
F-0007-01 的置信度由「中」升为「高」（结构 + 降级行为均已逐行确认）。
同时新发现该告警只走 `eprintln!` → F-0008-02。

## 维度覆盖（本批）
- 维度 G（路径安全）：✔✔ `resolve_safe_path`（handlers.rs:46-73）是本仓写法的上界——
  `normalize_relative_id` 严格白名单 → join → **双方各自 canonicalize** → `starts_with` 前缀校验；
  且对「尚不存在」的路径有「父目录规范化 + 拼末段」的退化分支（:52-64），不会因
  `canonicalize` 失败就放行。`match_model_route`（mod.rs:93）另有一层 `..`/`\`/前导 `/` 拒绝，
  与 `extract_id_from_*` 的裸 `strip_prefix`（:291-305）配合正确（后者只在 `match_model_route`
  返回 `Some` 之后才被调用）
- 维度 A（状态真源）：✔✔ `active_model_id`（mod.rs:216-240）是**纯决策内核 + 穷举测试**的写法：
  回落顺序 registry.active_id → 内置模型（且**只在文件真存在时**）→ 空串，
  四条 `#[test]` 覆盖四种组合（:319-343）。**这是模式 B 的正面对照**
- 维度 C（竞态）：✔✔ 写锁纪律显式且有注释：`RwLock` 不支持递归，故 import/activate/display
  都是「写锁内改内存 → 释放锁 → persist」（handlers.rs:201-221 / :245-273 / :316-329），
  且 `persist()` 内部重新 `read()` 取快照（mod.rs:185-194）⇒ 并发写不会丢更新
- 维度 B（回滚）：✔ 三个写路径都在 `persist` 失败时回滚内存态（handlers.rs:216-219 / :268-271）
- 维度 D：✘ `handle_list` → `entry_to_list_item` → `compute_dir_size` 每请求全递归 stat → F-0008-01

## 未核实项
1. `handlers.rs:330-427`（`handle_delete` + 错误 helper）未读。
2. `models_routes/dto.rs` / `web_api/dto.rs` 未读（维度 E 的逐字段对齐未做）。

## 本批新增
P0 0 · P1 0 · P2 1 · P3 1 ｜ 另回填 F-0007-01 置信度 中 → 高
