# REGISTER — external-input-v2（Wave 3 轨 F；给主 agent 的接线清单）

> 分支 `mod/w3-external`（worktree `/home/skystar/Live2D-Ai-w3-external`，基座 `118bd435`）。
> 闭环定义：**external-input + bilibili-sidecar 可观察**——接受 / 拒绝 / 忙碌计数进
> `state_json`（可测），sidecar 节流可开关且离线自检，`SEND_GIFT_V2` 上游查证结论落纸。

---

## 0. 本分支已经做了什么（主 agent 不必重做）

- **新文件** `crates/live2d-ai-mod-external-input/src/counters.rs`（181 行，含 4 条单测）：
  进程级 `AtomicU64` 计数（`accepts` / `rejects` / `busy` / `v2_ignored`），
  自由函数 `record_accept` / `record_reject` / `record_busy` /
  `record_v2_ignored` + `counters_snapshot`；头注钉死「每个计数对应哪条 handler 分支」。
- **`crates/live2d-ai-mod-external-input/src/lib.rs`**：`pub mod counters` + re-export；
  `ExternalInputRuntime::state_json` 现在返回计数 + `ready`（1 条新单测）。
- **`crates/live2d-ai-desktop/src/web_api/external_routes.rs`**：
  handler 抽出 `handle_external_chat_with(..., inject)` 注入点（生产入口
  `handle_external_chat` 不变）；401 / 403 `mod_disabled` 记 `reject`，
  `ok:true` 记 `accept`，`ok:false` 记 `busy`；body 新增可选字段
  `v2_ignored`（非负整数，覆盖写）。**新增 3 条 handler 回归**（3 成功 / 1 坏 token /
  1 忙 → 计数正确并经真实注册表读 `state_json`；v2_ignored 上浮；非整数 400）。
- **`docs/examples/bilibili-sidecar/bilibili_sidecar.py`**：`--min-interval-ms N`
  （缺省 1000，0 = 关）+ `Throttle` 纯逻辑；`--selftest`（16 项断言，离线可跑，
  依赖**懒加载**——不装 blivedm/aiohttp 也能验）；`build_body()` 把 `v2_ignored`
  随每次注入上报；`--dry-run`。
- **`docs/examples/bilibili-sidecar/README.md`** + `requirements.txt`：写清节流开关、
  自检命令、v2 结论；requirements 从 `blivedm>=2.3,<3`（不可满足）改为
  `blivedm @ git+https://github.com/xfgryujk/blivedm.git`。
- **`docs/external-input.md`**：§3 加 `v2_ignored` 字段与校验；新增 §5.1
  「可观察计数」（curl + 键语义表 + 三条边界）；§8 改成「上游已支持 v2 + 旧版兜底计数」；
  §11 索引补 counters / sidecar。

**没碰**：`main.rs` 的 `AVAILABLE_MOD_FACTORIES` / `mod_count_*` /
`mod_factory_ids_match_expected`、`cli_entry::default_mods_manifest`、
根 `Cargo.toml` 版本、`pubspec.yaml`、`AGENTS.md`、`docs/README.md`、
`docs/releases/**`、README*、`mod_registry.rs` / `mods_routes.rs` /
`supervisor.rs` / `live2d-ai-mod-system`。

---

## 1. 集成待办（勾选表）

- [ ] **FACTORIES：不用改**。`external-input` 已在 `AVAILABLE_MOD_FACTORIES`
      （6 个工厂，`mod_count_is_six`），`FACTORY` 是既有静态单例。
- [ ] **缺省 manifest：不用改**。`external-input` 缺省启用是既有口径
      （`default_mods_manifest` 只收录它）。
- [ ] **`docs/README.md` 索引**：若没有 `docs/external-input.md` 一行，收束时补上
      （本轮**不改**该文件，避免与其它轨争抢索引）。
- [ ] **`AGENTS.md` 同步**（独占清单，主 agent 改）：Mod 能力描述可补一句
      「external-input 有接受/拒绝/忙计数（`GET /api/v1/mods/external-input/state`），
      sidecar 有节流与自检」。
- [ ] **无需版本号 / 发布说明**（Wave 3 不发布）。

---

## 2. 验收证据（本轨）

- `cargo test -p live2d-ai-mod-external-input`：18 通过（含 4 条 counters + 1 条 state_json）。
- `cargo test -p live2d-ai-desktop external_routes`：24 通过（含 3 条新回归）。
- `python3 docs/examples/bilibili-sidecar/bilibili_sidecar.py --selftest`：
  输出 `[sidecar] selftest OK（16 项断言）`，退出码 0（无需依赖/网络）。
- 上游查证：blivedm **已支持** `SEND_GIFT_V2`（PR #86，2026-08-11，v1.1.7）——
  它批量展开 protobuf 后复用同一 `_on_gift` 回调；sidecar 不需要新回调。
  旧版兜底不再依赖 `_on_unknown_cmd`（blivedm 根本不调它），改为 `handle`
  覆盖 + `_is_unknown_gift_v2` 纯函数判定，selftest 4 条断言覆盖。

---

## 3. 未决 / 已知缺口

- 计数是**进程级、不持久化**：进程重启归零，且不含历史；定位是运行观察值，不是审计账本。
- `state_json` 仍走 `ModRegistry::runtime_state` 的 `try_lock`：Mod worker 持锁时
  `GET /state` 回 503（既有契约，与所有 Mod 一致）。
- `requirements.txt` 走 GitHub 安装，需要机器上有 `git`；若目标机器无网络，
  只能装到旧版 blivedm → 走 `v2_ignored` 兜底路径（不静默丢）。
- 本轨未起活服务做 curl 端到端（只有单测 + sidecar 自检）；活服务 curl 属收束轮全量门禁。
