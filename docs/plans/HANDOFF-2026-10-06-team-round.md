# HANDOFF — 2026-10-06 夜（团队轮：冻结基线独立复核 + 文档漂移收口 + E9 探针 + E2/E4 决策）

> 上游：`main` @ `43e465e`（本轮 8 个提交 = 冻结 3 + 团队轮 5）· 起草本文时工作区**空**（`git status --porcelain` 0 项）；
> **T5 终局复验正在改 `docs/verification/gate-baseline-2026-10-06.md`**——那份改动**属 T5**，不在本轮提交范围 ·
> 唯一工作树 `/home/skystar/Live2D-Ai`（linked worktree `Live2D-Ai-fe` 已删除）。
> 范围真源：[NEXT-ROUND-main-2026-10-06.md](NEXT-ROUND-main-2026-10-06.md)（E1–E13 / F1–F4 / R1–R8）+
> [../audit/2026-10-05-ledger/README.md](../audit/2026-10-05-ledger/README.md)。
> 上一份交接：[HANDOFF-2026-10-06-e1-e5-debt-round.md](HANDOFF-2026-10-06-e1-e5-debt-round.md)。

## 0. 一句话

维护者「开始派团队处理」后，Lead 先把上一轮 **40 项未提交改动冻成 3 个提交**，再派 4 名队友并行：
**T4 冻结基线独立复核**（抓到本轮最有价值的单点：**clippy 真红 ⇒ 基线并非全绿**）、
**T1 文档漂移收口**（含 Dart 棘轮同 commit 收紧）、**T2 E9 探针稳健性**、
**T3 E2/E4 决策论证**；另有 **T5 终局复验**（独立，见 §6 指针，本文不替它下结论）。

## 1. 一分钟上手

```bash
cd /home/skystar/Live2D-Ai          # 唯一工作树（-fe 已删除，不要再 cd 它）
git log --oneline -9                 # 期望：43e465e … 65c42f3（8 个提交 + 上一基线）
git status --porcelain               # 期望：空（若只多出 docs/verification/gate-baseline-*.md，那是 T5 的终局复验在写）
```

门禁（改动前后都跑；全绿才算数）：

```bash
cargo test --workspace --all-targets && cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings   # ← 必须带 --all-targets（见 §5 坑 1）
cargo run -p xtask -- rust-ratio && cargo run -p xtask -- code-stats --check
cd shell/flutter && export PATH="$HOME/flutter/bin:$PATH" && flutter analyze && flutter test
```

- 重型命令一律 `flock /tmp/l2d-heavy.lock <cmd>`；驱动浏览器 `flock /tmp/l2d-browser.lock <cmd>`。
- `flutter` **不在默认 PATH**（SDK 在 `~/flutter/bin`）——每个新 shell 都要 export。
- `target/` 31 G 已在本树，增量编译很快；**不要删它**；前端产物 `shell/flutter/build/web` 也在。

## 2. 本轮提交（8 个）

| 提交 | 内容 |
| --- | --- |
| `65c42f3` | 冻结：Rust 债（台账最后 6 条 P1 清空——前置路由补日志 / Mod 清单以磁盘为基底 / `file_watcher` 无条件安装） |
| `b0b0365` | 冻结：Dart 拆分（守卫改读「库 + parts 并集」；`main.dart` 1417 → 657 行，5 个 part） |
| `36937df` | 冻结：文档（E1/E5 轮交接与台账关闭状态 + 下一轮清单） |
| `cc68f06` | **fix(clippy)**：台账守卫的文档注释触发 `doc_lazy_continuation`（`--all-targets` 门禁真红） |
| `f5210f3` | **fix(xtask)**：Dart `>800` 棘轮 **2 → 1**（补 E1-b 欠的收紧）+ 留债栏/口径更正 |
| `37f94ec` | **docs**：文档漂移收口（版本口径 / 归档分支不存在 / director 现行口径 / E8 减量账） |
| `7d53def` | **test(probe)**：E9 探针稳健性——`audio-c` 三路取证 + 四主题各自像素阈值 + 证据入库 |
| `43e465e` | **docs**：E2/E4 决策方案纸 + 冻结基线独立门禁复核报告（含 clippy 真红发现） |

## 3. 各轨实测数字（**只在标明的 SHA 上成立，别外推**）

### 3.1 T4 冻结基线与独立复核（在 `36937df` 上自跑）

| 门禁 | 结果 |
| --- | --- |
| `cargo test --workspace --all-targets` | **1315 / 0** |
| `cargo test --doc` / `fmt` | 3 / clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | **红 2 条**（见 §3.1.1） |
| `rust-ratio`（≥95%） | **96.1016% PASS** |
| `code-stats --check` | **四条 PASS** |
| `flutter analyze` / `flutter test` | **0 issue / 1583** |

#### 3.1.1 本轮最有价值的单点：clippy 真红

- 红点：`crates/live2d-ai-desktop/src/web_api/tests_mod.rs:276/277` 的 `doc_lazy_continuation`
  ——第 275 行以「+」起头，被 markdown 当成**新列表项**，续行 3 空格不够。
- **只在不带 `--all-targets` 的 clippy 下不报** ⇒ 上一轮交接写的「clippy 0 warning」是
  **口径（+ 缓存）造成的假绿**。
- 修复 = `cc68f06`（「+ 」→「另有 」，**纯注释、语义零变化**）；T4 复检 **EXIT=0**。

#### 3.1.2 两条新守卫的「破坏即红」自证（T4 另做，收尾 `git status` 空）

- `test/dart_library_guard_test.dart`：改坏 → 红 → `git checkout --` 复原 → 绿。
- `tests_mod::pre_dispatch_responses_go_through_respond_and_log`：同上。

### 3.2 T1 文档（`37f94ec` + `f5210f3`）

- 版本口径 `0.2.0`/`rc.7` → 现行 **`0.2.1-rc.1`**（`Cargo.toml` `version` + tag + `pubspec.yaml` 实测）。
- `archive/action-layer-p6` 全部写面内引用按「**分支已不存在**」改写（`git branch --list` 空 /
  `git ls-remote --heads origin` 只回 `main`+`mainline/1-core-baseline` / 4 个历史 bundle 的 heads 也没有），
  并给出**等价取回**：`git show 98469df^:crates/live2d-ai-mod-director/src/lib.rs`（395 行）、
  `git show ef9f428^:crates/l2d-wasm-demo/src/web/surface.rs`（1721 行）。
- 台账历史引用加注「**刻意不改**（改台账 = 篡改证据）」；`docs/plans/AUDIT-PROMPT-whole-repo-2026-10-06.md`
  里指向已删除 `-fe` 树的路径整体改判。
- **Dart 棘轮 `RATCHET_DART_800` 2 → 1**：`cargo test -p xtask` **27 passed**；
  `code-stats --check` Dart 行 **1 ≤ 1 PASS**；`fmt --check` clean。
- **E8 文档减量账**：`docs/**/*.md` 全部 **1,279 份 / 143,270 行**；不含 `docs/audit/**` = **265 / 68,122**；
  `docs/audit/` 自身 = **1,014 / 75,148**。`xtask` 的 docs 行**不排除** audit ⇒「归档不减总量」成立；
  PLAN §5 的「docs ≤45,000」按不含 audit **仍未达标**（超 23,122 行）；`docs/plans` 现 46 份（`docs/legacy/plans` 94 份）。

### 3.3 T2 探针（`7d53def`）

| 项 | 结果 |
| --- | --- |
| `audio-c` 改三路取证 | DOM 2000→500 ms + 页内自增 id + 页内媒体记录器 + CDP `Media` 域 |
| 真实链路 `audio-a/b/c` | 两轮**全 pass、0 blocked** |
| 四主题像素阈值 | 各自标定（`themebase` **16 次**实测）：ON = 基线 + 40%×(红信号−基线)、OFF = 基线 + 12、`maxShare` = share + 0.02 |
| `all` 同一 HEAD / 同一脚本哈希连跑两遍 | 各 **40 项：pass 36 · manual-only 4 · fail 0 · blocked 0**；两次**判定 0 处不同** |
| `8b` 前端产物新鲜度 | 重建后**真绿**（0 个 dart 比 `main.dart.js` 新） |
| 离线产物门禁 | `ignite.sh --check-dir` **四条 ok** |
| 受控自证 | `mediaspy`（含阴性对照） |

证据：[docs/verification/evidence-2026-10-06-e9/](../verification/evidence-2026-10-06-e9/README.md)。

### 3.4 T3 决策（`43e465e`，两份方案纸，**只论证不实施**）

- **E2**（[DECISION-display-prefs-2026-10-06.md](DECISION-display-prefs-2026-10-06.md)）：推荐 **B2**
  （同库 `part` + extension 外搬，**可 1 → 0**；A 类分解暂不做）。关键事实：**24 键落盘全集零测试覆盖**。
- **E4**（[DECISION-artifact-budget-2026-10-06.md](DECISION-artifact-budget-2026-10-06.md)）：`build/web` 走**真减**
  （删 `*.symbols` + `skwasm*`/`wimp*`，**−20.18 MiB → 29.38 MiB**；`chromium/` 必须保留）；
  `dist` 只靠剥 `name` 段**仍超预算 0.115 MiB**；`wasm-opt` 本机 absent ⇒ **不估数**。

## 4. 待维护者裁决（四项，**未裁决前不要动**）

| # | 事项 | 选项 / 关键事实 | 真源 |
| --- | --- | --- | --- |
| **E2** | `display_prefs.dart`（1169 行 / 单类） | (a) **B2 全做**（可 1 → 0）· (b) 只做低风险部分 · (c) 不拆 + 棘轮停在 1（**24 键落盘全集零测试覆盖**，三条路都要单独论证） | [DECISION-display-prefs-2026-10-06.md](DECISION-display-prefs-2026-10-06.md) |
| **E4** | 产物预算 | (a) **真减脚本化**（已验 −20.18 MiB → 29.38 MiB）· (b) **明文重定预算 + 理由**并写进 `xtask` 报告口径 | [DECISION-artifact-budget-2026-10-06.md](DECISION-artifact-budget-2026-10-06.md) |
| **R8** | ~470 MB 清洗前 bundle 删留 | 删前确认不再需要回滚（`backup-2026-10-05/archive-full-history-2026-09-11.bundle` 是当前唯一清洗前回滚路径） | [NEXT-ROUND-main-2026-10-06.md](NEXT-ROUND-main-2026-10-06.md) §4.2 R8 |
| **token** | 推送凭据 | `git push origin main` 现回 `No anonymous write access.` / `fatal: Authentication failed` ⇒ 本地提交**待推** | 同上 R5 |

## 5. 本轮踩到的坑（6 条，逐条真踩过）

1. **clippy 必须带 `--all-targets` 才算门禁**：`36937df` 上 `cargo clippy --workspace --all-targets -- -D warnings`
   红 2 条 `doc_lazy_continuation`（`tests_mod.rs:276/277`，第 275 行以「+」起头被当列表项，续行 3 空格不够），
   **不带 `--all-targets` 不报** ⇒ 「0 warning」是假绿。教训：**门禁命令一个字都不能省**；**改注释也算改代码**。
2. **棘轮必须同 commit 收紧**：E1-b 把 Dart `>800` 从 2 降到 1 却没把 `RATCHET_DART_800` 从 2 收紧，
   门禁就留下「涨回 2 仍绿」的空档（`f5210f3` 才补齐）。任何让计数下降的改动，必须**同一个提交**里收紧常量。
3. **写面外文件先拿授权**：T1 遇到三处写面外引用（`crates/live2d-ai-desktop/src/main.rs` 的注释、`CHANGELOG.md`、
   队友正在写的 `DECISION-*`）。前两处拿到 Lead 授权后**只改注释/措辞**；第三处**不动别人的交付物**，改为上报
   ——跨队友同一文件 = FS 版本冲突与覆盖风险。
4. **前端产物过期会让 `8b` 真红**：`8b` 的判据是 `main.dart.js` 比所有 `lib/**/*.dart` 新；改完前端**不重建**，
   探针就红，而那不是探针的错。T2 是**重建后**才拿到真绿（0 个 dart 比它新）。
5. **台账/历史文件不要「统一口径」**：`docs/audit/**` 里的旧引用是**证据原文**，改它 = 篡改证据；
   正确做法是在目录 README 加总说明（T1 已加）。
6. **多写者同树：复核类任务先钉住被测 SHA**：T4 复核期间 T1/T2 仍在写，它的报告因此先记「树上有别的写者」
   再声明依据；收尾还验了 `git status` 空。**冻结基线 = 一个 SHA + 一份声明**，不是「现在的树」。

## 6. 文档指针

- 变更历史（本轮）：[AGENTS.md](../../AGENTS.md) §变更历史 → `2026-10-06 夜（团队轮…）`
- 冻结基线独立复核报告（T4）：[docs/verification/gate-baseline-2026-10-06.md](../verification/gate-baseline-2026-10-06.md)
  —— 含逐条原始输出、差异定位、两条破坏即红自证、`cc68f06` 后的 clippy 复检。
- **T5 终局复验**：独立复核，结果落盘在**同一份** `docs/verification/gate-baseline-2026-10-06.md`（预期 §8 起，
  以其实际标题为准）。**本文与 AGENTS.md 都不替它下结论**——要数字就去看那份文件。
- E9 探针证据：[docs/verification/evidence-2026-10-06-e9/](../verification/evidence-2026-10-06-e9/README.md)
- E2/E4 决策纸：[DECISION-display-prefs-2026-10-06.md](DECISION-display-prefs-2026-10-06.md)、
  [DECISION-artifact-budget-2026-10-06.md](DECISION-artifact-budget-2026-10-06.md)
- 下一轮清单：[NEXT-ROUND-main-2026-10-06.md](NEXT-ROUND-main-2026-10-06.md)（E1–E13 / F1–F4 / R1–R8）
- 台账状态：[../audit/2026-10-05-ledger/README.md](../audit/2026-10-05-ledger/README.md)（**仍未关闭的 P1 = 0 条**）

## 7. 变更历史

- 2026-10-06 夜：初版（团队轮收尾落盘）。上一份交接见
  [HANDOFF-2026-10-06-e1-e5-debt-round.md](HANDOFF-2026-10-06-e1-e5-debt-round.md)。
