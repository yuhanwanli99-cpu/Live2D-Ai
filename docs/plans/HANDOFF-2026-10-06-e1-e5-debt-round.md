# HANDOFF — 2026-10-06 夜（E1 结构拆分 + E5 台账 P1 清空）

> 上游基线：`main` @ `08338f3`（v0.2.1-rc.1；`origin/main` = `6be9984`，本地领先 2 个 docs 提交）。
> **本轮全部改动未提交**（维护者口径：目前只本地修改，提交/推送等 token）。工作区 **40 项**，全部门禁已验证。
> 范围真源：[`NEXT-ROUND-main-2026-10-06.md`](NEXT-ROUND-main-2026-10-06.md)（E1–E10 / F1–F4 / R1–R8）+ [`../audit/2026-10-05-ledger/README.md`](../audit/2026-10-05-ledger/README.md)。

## 0. 一句话

把「工作树」收成一条（linked worktree `Live2D-Ai-fe` 已删），把 **E1（守卫静默漏扫 + `main.dart` 拆分）** 做完，
把**台账 6 条未关 P1 全部清空**（`F-0002-02` / `F-0006-03` / `F-0013-01` / `F-0644-01` / `F-0002-01` / `F-0001-01`），
每条都有「破坏实现 → 断言变红」的**双向自证**；顺带复核关闭 **E6**（反复制门禁已成立）与 **E10**（字体镜像 PASS）。

## 1. 一分钟上手

```bash
cd /home/skystar/Live2D-Ai          # 唯一工作树（-fe 已删除，不要再 cd 它）
git status --porcelain              # 期望 40 项（本轮改动，尚未提交）
git log -1 --format='%h %s'          # 期望 08338f3 docs(audit): A/B 双跑支持 …
```

门禁（改动前后都跑；全绿才算数）：

```bash
cargo test --workspace --all-targets && cargo test --doc --workspace
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio && cargo run -p xtask -- code-stats --check
cd shell/flutter && export PATH="$HOME/flutter/bin:$PATH" && flutter analyze && flutter test
```

- `flutter` **不在默认 PATH**（SDK 在 `~/flutter/bin`）——每个新 shell 都要 export。
- `target/` 31 G 已迁入本树，增量编译很快；**不要删它**。
- 前端产物 `shell/flutter/build/web` 也一并迁入了（`./scripts/ignite.sh` 直接可用）。

## 2. 本轮交付（三块）

### 2.1 工作树单一化（原 R1，方向与原文相反）

- 删 linked worktree `Live2D-Ai-fe`，**唯一工作树 = `/home/skystar/Live2D-Ai`（`main`）**；
- 31 G `target/`、`shell/flutter/build`、`.dart_tool` 迁入（免一次全量编译）；
- 运行态以 `-fe` 为准并入：`.env`(0600) / `mods.json` / `sessions/` / `crates/l2d-wasm-demo/dist/`
  （旧值备份在 `/home/skystar/audit-ref-2026-10-06/dead-tree-runtime-backup/`）；
- A/B 审计台账按维护者指示**不再运行、仅作参考**，整份保运到树外 `/home/skystar/audit-ref-2026-10-06/`；
- 旧分支 `mod/persona-polish` @ `88342ce` 退役（**未删分支**，0 领先，可 `git branch -d`）。

### 2.2 E1：守卫不得静默漏扫 + `main.dart` 拆分

- **E1-a 20 处读取点升级**：`main.dart` 有 4 个 `part`，而 test/ 里 20 处源码扫描守卫**只读库文件本身**
  （读到的是一半、断言却照样绿）⇒ 全部改走 `test/support/dart_library.dart` 的 `readLibrarySource()`；
  新增门禁 `test/dart_library_guard_test.dart`（3 条判据：part 库不得被字面量直读 / 不得有「按路径读源码」的
  辅助函数 / 不得「变量路径 + part 库字面量」；目录递归遍历是唯一豁免；**红-绿双向自证**已做）。
- **E1-b `main.dart` 1417 → 657 行**：新增 5 个 part（`shell_cue_voice_wiring` / `shell_background_scale_wiring` /
  `shell_section_wiring` / `shell_app_root` / `shell_lifecycle_wiring`），**逐字搬迁**（未重写一行业务代码）；
  part 里的 extension **不能调 `setState`**（`@protected`）⇒ 新增**唯一**重建桥 `_rebuild()`（14 处改桥）；
  `initState` / `didUpdateWidget` / `dispose` 只留 `super.*` + 一次委托，本体进 part。
  Dart `lib >800` **2 → 1**（只剩 `display_prefs.dart` = E2）。

### 2.3 E5：台账 6 条 P1 → 0

| ID | 修法要点 | 新回归 |
|---|---|---|
| `F-0002-02` | `supervisor_slot` 两条假绿灯 → 真生命周期（set/try_get/take）+ 真毒化（`#[cfg(test)] inner_arc()`，先断言 `is_poisoned()`） | 2 条 |
| `F-0006-03` | `mods_routes` 5 处 `.expect("poisoned")` → 单一定义 `lock_registry()`（`into_inner()`），与 external/voice 同口径 | 毒化后列表仍回 200 |
| `F-0013-01` / `F-0644-01` | `persist_manifest` 以**磁盘当前内容**为基底、只覆写在册 id（新增 `raw_manifest` 字段）+ 启动 `warn!` 点名未知 id | 未知 id / 其 config / 顶层键都不被抹掉 |
| `F-0002-01` | `FileWatcher` 收惰性 `SupervisorSource`（现取槽位）并**无条件安装**：首跑窗口改 `.env` 至少立刻进快照 | `on_config_changed_without_supervisor_still_refreshes_snapshots` |
| `F-0001-01` | 四条 API 前置路由 + WS 前门（共 6 处）统一走 `web_api::mod::respond_and_log`（**复用** dispatch 的同一份分级判据；`log_request_outcome` 收成 `pub(super)` + `&str` 路由标签） | `tests_mod::pre_dispatch_responses_go_through_respond_and_log`（调用形状 7/4，零命中判红） |

另：本批把 `mods_routes.rs` 顶到 **1009 行**（`>1000` 门禁 FAIL）⇒ 按既有配方拆成
`mods_routes_tests.rs`(360) + `mods_routes_tests_support.rs`(158)，生产文件回到 508 行（`>500` 计数不变）。

## 3. 门禁数字（本轮终值）

| 门禁 | 结果 |
|---|---|
| `cargo test --workspace --all-targets` | **1315 / 0**（基线 1311 + 新回归 4） |
| `cargo test --doc` / `fmt` / clippy `-D warnings` | 3 / clean / **0 warning** |
| `rust-ratio`（≥95%） | **96.1016% PASS** |
| `code-stats --check` | **四条 PASS**：`>500` 44/44 · `>1000` **0/0** · Dart `>800` **1/2** · deps 22/22 |
| `flutter analyze` / `flutter test` | **0 issue / 1583 passed** |

## 4. 未提交改动（40 项）

- **Rust（11 改 + 2 新）**：`mod_registry.rs`(结构体加字段) · `mod_registry/{registry,tests_lifecycle}.rs` ·
  `web_api/{mod,dispatch,tests_mod,mods_routes,supervisor_slot,file_watcher,cli_entry}.rs` ·
  新 `web_api/mods_routes_tests.rs` · `web_api/mods_routes_tests_support.rs`
- **Dart（19 改 + 6 新）**：`lib/main.dart` · 新 `lib/app/shell_*.dart`(5) · `test/*`(17 改) · 新 `test/dart_library_guard_test.dart`
- **文档（3）**：`AGENTS.md` · `docs/plans/NEXT-ROUND-main-2026-10-06.md` · `docs/audit/2026-10-05-ledger/README.md`

## 5. 下一轮建议（按优先级）

1. **E9 探针稳健性**（要真浏览器，本机可跑）：① `audio-c` 2 次里 1 次 blocked（媒体元素已销毁 / 还没建）→
   加宽采样窗口或改用 `Media` 域事件；② 像素阈值只对**默认黑主题**成立 → 四主题各自给阈值并写清来源。
   取证环境：`~/.cache/ms-playwright/chromium-1243`（Chrome for Testing 153）+ CDP；渲染面需
   `--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader`；探针 `scripts/browser_probe.mjs`。
2. **E8 文档减量账**：审计台账入库后口径要重算（用「不含 `docs/audit/**`」那一行判定；**归档不减总量**）。
3. **E2 `display_prefs.dart`（1169 行 / 单类 944 行）**：**待维护者裁决**（见 §6）。
4. **E4 产物预算**：**待裁决**（见 §6）。
5. **R2 / R6 / R7 / R8 仓库收尾**：5 条 0 领先分支可删（`feat/frontend-redesign` / `chore/debt-round-2026-10-05` /
   `mainline/1-core-baseline` / `pr-1` / `mod/persona-polish`）；`archive/action-layer-p6` 被 10+ 处引用但本地与远端都没有；
   清洗前 bundle（~600 MB）删留待定。
6. **E7 CI 首跑**（需 GitHub runner；nightly 是否缺 `libasound2-dev` 未验证）。
7. **功能线**：`F1` 正文帧带 `sentence_seq`（朗读高亮，全项目唯一「不做就永远做不出来」的一条，要改后端 WS 契约、只增不改）；
   `F2` 会话记忆后端 / 动作选型（真源 `PLAN-actions-voice-memory-2026-09-15.md`）；`F3` 舞台图解码状态帧 / capabilities 发现；
   `F4` 离线 CJK 字体回落（五族约 11.9 MiB，加镜像或做成可选构建步骤）。

## 6. 需要维护者拍板的两件事（**未拍板前不要动**）

- **E2 / `DisplayPrefs`**：`display_prefs.dart` 1169 行里 `DisplayPrefs` 是**单个 944 行类**，而 Dart 的**类体不能跨 `part`**
  ⇒ 只能做**类分解**（改设计：拆字段对象 / 门面），会影响序列化与 60+ 使用点，**必须**逐字段行为等价论证。
  问：允许做类分解，还是只做低风险部分（例如只把同文件里的顶层函数 / `StagePlaylistAppendResult` 等搬出去）？
- **E4 / 产物预算**：`shell/flutter/build/web` **50 MiB**（预算 35）· wasm `dist` **5.4 MiB**（预算 4）。
  二选一：**真减**（例如按需裁剪 canvaskit 变体）或**明文改预算 + 理由**并写进 `xtask` 报告口径。
  ⚠ 不许为了变绿而悄悄改判据。

## 7. 本轮踩到的坑（给下一位，逐条都真的踩过）

1. **`>1000` 棘轮按「文件总行数（含内联测试）」计**：往 `src/` 加代码前先看它离 1000 多远；加过头要按配方把内联测试
   拆到 `#[path]` 兄弟文件，**每份必须 <500**——`>500` 上限也是顶格的 44/44，拆错反而多一个超限文件。
2. **`part` 里的 extension 不能调 `setState`**（`@protected`）：拆 Dart 前先确认目标方法 setState-free，否则要加**唯一**重建桥，
   并在头注写死「不要另开第二条」（同一个动作两条路 = 下一次漂移的种子）。
3. **`#[cfg(test)] mod tests` 必须在文件末尾**（clippy `items_after_test_module`，`-D warnings` 下直接红）。
4. **`assert_eq!(a, b, "x" "y")` 是错的**：Rust 不拼接相邻字面量，多字面量会被当成多个格式参数（`multiple unused formatting arguments`）。
   用**单条**消息 + 内联捕获 `{var}`。
5. **`if let` 的 scrutinee 里借了 `request` 就不能在分支里 move 它**：先 `let r = check(&request);` 再 `if let Err(x) = r`。
6. **读文件再做 `edit`**（fs 策略要求）；**脚本改文件没有版本护栏**，锚点必须唯一——我曾用「文件里第一个 `assert_eq!(`」当锚点，
   把 `tests_mod.rs` 截断（已 `git checkout` 恢复并重做）。改多文件时宁可多写几行断言。
7. **凡是「读源码做断言」的守卫，都要问：它读的是库还是库+parts**——`test/dart_library_guard_test.dart` 就是为这类静默失效设的；
   变量路径读取（`String readLib(String rel) => File(rel)...`）是它的已知盲区，已在文件头注写明。
8. **台账行号会腐烂**：每条 P1 都必须回源码复核（本轮 6 条全复核，其中两条**措辞被收窄**：F-0001-01 不是「这些端点零日志」，
   而是「**无响应**的失败零记录」；F-0002-01 不是「首跑无热重载」，而是「首跑窗口内 `.env` 改动要等下次启动」）。

## 8. 文档指针

- 变更历史（本轮三块）：[`AGENTS.md`](../../AGENTS.md) §变更历史 → `2026-10-06 夜` 三条（第三轮 / 第二轮 / 工作树+E1）
- 台账状态：[`../audit/2026-10-05-ledger/README.md`](../audit/2026-10-05-ledger/README.md)（**仍未关闭的 P1 = 0 条**）
- 下一轮清单：[`NEXT-ROUND-main-2026-10-06.md`](NEXT-ROUND-main-2026-10-06.md)（E1 / E5 / E6 / E10 已勾）
- 当前版本口径：[`../releases/v0.2.1-rc.1.md`](../releases/v0.2.1-rc.1.md)

## 9. 交接检查清单

- [ ] `cd /home/skystar/Live2D-Ai`（**不是** `-fe`，它已不存在）
- [ ] `git status --porcelain` = 40 项（若为空，说明有人提交了——`git log` 看新提交）
- [ ] 跑一遍 §1 门禁，确认「出发时是绿的」；数字应落在 §3
- [ ] 读 §6 两个待裁决项，**先拿到维护者口径**再动 E2 / E4
- [ ] 提交/推送需要 token；提交前 `cargo fmt` 后重跑全套
- [ ] 别删 `target/`；别把 `.env` / `mods.json` / `sessions/` 提交（`.gitignore` 已覆盖）
- [ ] 审计台账只在 `docs/audit/**`（树外 `audit-ref-2026-10-06/` 那份是参考副本，**不再运行**）

## 10. 变更历史

- 2026-10-06 夜：初版（会话收尾时落盘）。上一份交接见 [`HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md`](HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md)。
