# HANDOFF — 2026-10-06 夜（第二轮团队：**0.2 工程债清算**）

> 上游：`main` @ `834a4c6`（本轮 **3 个提交**：`2790a2c` / `83c89e1` / `834a4c6`）· 唯一工作树
> `/home/skystar/Live2D-Ai`（linked worktree `Live2D-Ai-fe` 已删除）。
> 范围真源：[NEXT-ROUND-main-2026-10-06.md](NEXT-ROUND-main-2026-10-06.md) +
> 两份决策纸（E2 / E4）。上一份交接：
> [HANDOFF-2026-10-06-team-round.md](HANDOFF-2026-10-06-team-round.md)。

## 0. 一句话

维护者裁决「**继续清理工程债务，清完就是 0.2 时代的任务**」，并同时明确 **E8 文档减量本轮不动**、
**R8 清洗前备份全部删除**、**E2 / E4 按决策纸的推荐方案执行** ⇒ 三个提交把
**E2 + E12**、**棘轮归零 + dist 预算重定 + 报告口径**、**E4 + E13 + CI 接线**做完。
**这不是「工程债已全部清空」**：§4 仍有 6 条（4 条 blocked / 裁决不做 + 2 条新残项）。

## 1. 一分钟上手

```bash
cd /home/skystar/Live2D-Ai          # 唯一工作树（-fe 已删除，不要再 cd 它）
git log --oneline -5                 # 期望：834a4c6 · 83c89e1 · 2790a2c · 64f412d · 43e465e
git status --porcelain               # 期望：空（若只多出 docs/verification/**，那是 V1 终局复验在写）
```

门禁（改动前后都跑；全绿才算数）：

```bash
cargo test --workspace --all-targets && cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings   # ← 必须带 --all-targets
cargo run -p xtask -- rust-ratio && cargo run -p xtask -- code-stats --check
cd shell/flutter && export PATH="$HOME/flutter/bin:$PATH" && flutter analyze && flutter test
# 前端产物：先构建，再跑预算门禁（清减 + 四条判据）
flutter build web --release --base-href /app/ --no-web-resources-cdn && cd ../..
./scripts/prune_web_artifacts.sh --check && ./scripts/ignite.sh --check-dir shell/flutter/build/web
```

- 重型命令一律 `flock /tmp/l2d-heavy.lock <cmd>`；驱动浏览器 `flock /tmp/l2d-browser.lock <cmd>`。
- `flutter` **不在默认 PATH**（SDK 在 `~/flutter/bin`）——每个新 shell 都要 export。
- **改完前端必须重建**，否则离线的产物类判据（含探针 `8b` 新鲜度）会红，而那不是判据的错。

## 2. 本轮三个提交

| 提交 | 内容 |
| --- | --- |
| `2790a2c` | **E2**：`display_prefs.dart` 1169 → 583 行（5 个同库 part）+ **E12**：24 键落盘守卫 |
| `83c89e1` | **棘轮归零**（`RATCHET_DART_800` 1 → 0）+ **dist 预算重定**（4 → 4.2，新 `Mib` 单位类型）+ 报告口径 |
| `834a4c6` | **E4**：`scripts/prune_web_artifacts.sh`（−20.18 MiB）+ **E13**：预算变真门禁 + CI 接线 |

## 3. 各轨实测数字（只在标明的范围/提交上成立，别外推）

### 3.1 E2 + E12（`2790a2c`）

- `display_prefs.dart` **1169 → 583 行**；新增 **5 个同库 part**：
  playlist 225 / codec 225 / limits 149 / derived 91 / copy 71（**全部 <800**）
  ⇒ Dart `lib >800` 计数 **1 → 0**。
- **留在类里**：24 字段 / const 构造 / static const / `==` / `hashCode` / `toString`；
  静态 API 面只做 **19 行转发**，**调用点零改动**。
- 搬迁用**逐字非循环自证**：剥掉 75 处 `DisplayPrefs.` 限定符后，原始 **25 段**逐字出现。
- 守卫升级：`display_prefs_test.dart:431` 与 `no_backdrop_filter_test.dart:543` 改走
  `readLibrarySource()`（**红-绿自证**）；顺带修 `setting_wiring_test` 的过滤条件——原判据把
  5 个新 part 当**外部读者**，死字段能骗过它。
- **E12（此前零覆盖）**：新增 `shell/flutter/test/display_prefs_persist_keys_test.dart`，**30 条断言**：
  `toJson` 键集合 == 手写 24 键全集 == const 构造字段集合；24 键逐个**非默认值往返** +
  `==`/`hashCode` 对称；未知键忽略；旧键迁移。`flutter test` **1583 → 1613**。

### 3.2 棘轮与预算（`83c89e1`）

- `RATCHET_DART_800` **1 → 0**（与拆分同一次改动，依棘轮纪律）。
- `WASM_DIST_BUDGET_MIB` **4 → 4.2**，并从 `u64` 改成新的 **`Mib` 单位类型**（允许小数、按字节精确）。
- 重定理由（**不是为了让门禁变绿**）：PLAN 的「4」是**未实测的估计值**——实测 dist **5.35 MiB**、
  剥 `name` 段后 **≈4.11 MiB** 仍超 4，而本机**无 wasm-opt / wasm-strip** ⇒ 真减**未实测、不估数**、
  本轮不改 wasm 构建。故重定为「**当前接受值**」并写明**重新评审条件**（`wasm-opt` 可用时回到 ≤4 MiB）。
- 报告口径：`report.rs` 表头「PLAN 预算」→「**预算**」；表后加说明区分 PLAN 目标与实测重定的接受值，
  并写明「超预算」是**如实打印**、`--strict-plan` **不会**因此变绿。

### 3.3 E4 + E13（`834a4c6`）

- 新增 `scripts/prune_web_artifacts.sh`（**234 行**）：删 6×`*.symbols` +
  `skwasm*`/`skwasm_heavy*`/`wimp*` 共 **12 项**。
- 体积：`build/web` **51 964 234 B（49.56 MiB / 66 文件）→ 30 804 686 B（29.38 MiB / 54 文件）**。
- `--check` **四条判据**：死重 0 / 红线文件齐全 / ≤35 MiB / 与 `xtask` 预算不漂移。
- **`chromium/` 必须保留**：真实 Network 实测 `chromium/canvaskit.wasm` **200 ×4**、死重 **0 请求**
  ——既有调研说「`chromium/` 是无用副本」**是错的**，照删会白屏。
- CI 接线：`.github/workflows/flutter-checks.yml` 的 `flutter-web-offline-artifacts` job =
  构建 → prune → `--check` → `ignite --check-dir`。
- 真验证：`ignite --check-dir` **四条 ok**；起服务 + CDP 连跑两次，`browser_probe net` 两次
  **5 项 fail 0**、独立断言 **ASSERT_FAIL=0**、未预期 **404 = 0**（唯一 404 是设计内的
  `/actions/field_map.json`）、控制台**真错误 []**。
- **反向自证 3 条**：放回 `*.symbols` / 加 6 MiB 文件 / 移走 `chromium/canvaskit.wasm` 各自判红。
- 新增 [docs/architecture/artifact-budget.md](../architecture/artifact-budget.md)。

### 3.4 R8 已执行（Lead）

删除 **6 项**清洗前历史副本（删前**逐项 `stat` 核对绝对路径**，实测释放 **465 391 616 B ≈ 443.8 MiB**）：

| 文件 | 字节 |
| --- | ---: |
| `Live2D-Ai-LEGACY-FULL-HISTORY.bundle` | 114 930 748 |
| `Live2D-Ai-PY-LEGACY.bundle` | 100 919 311 |
| `Live2D-Ai-baseline-28de52cf.bundle` | 114 731 556 |
| `Live2D-Ai-baseline-incremental.bundle` | 13 211 521 |
| `backup-2026-10-05/archive-full-history-2026-09-11.bundle` | 114 771 476 |
| `redesign-backup-2026-09-27.tar.gz` | 6 799 153 |

**未删**：`Live2D-Ai-ANDROID-ARCHIVE.bundle`（748 KB，R8 清单未含）、`backup-2026-10-05/` 其余三件
（快照/补丁/untracked tar）、`backup-2026-10-06/refs-before.txt`、
`/home/skystar/backups/dsh-data-backup-20260821.tar.gz`（128 MB，实测内容是 `.dsh/` **DSH 自身数据**、
非仓库历史 ⇒ **不属 R8 范围**，需维护者另行决定）。

### 3.5 E10 复核 + R7 收尾

- **E10**：`scripts/font_fallback_mirror.sh --check` **PASS**（21 文件 / 2 815 292 B，清单 == 磁盘 == 引擎表全集）。
- **R7**：余 **8 条**分支各自都有**独有提交**（`git rev-list --left-right --count main...<b>` 右值 **1…374**）
  ⇒ **全部保留**，本轮不再删。

## 4. 仍未关清单（**6 条**；不要读成「工程债已清空」）

| # | 事项 | 状态 |
| --- | --- | --- |
| 1 | **E7 CI 真 runner 首跑** | **仍 blocked**（无 token / 无 runner） |
| 2 | **推送 `origin`** | **仍 blocked**：`git push origin main` 回 `No anonymous write access`；写作时现测 `git rev-list --count origin/main..main` = **14** |
| 3 | **E8 文档减量** | **维护者裁决本轮不动**：不含 `docs/audit/**` **68 122 行**，PLAN ≤45 000 仍超 **23 122 行** |
| 4 | **dist 无机器守门** | 全仓 workflow **无 `trunk build`** ⇒ dist 体积涨了不红（budget 只在 `xtask` 报告里） |
| 5 | **新残项：守卫整文件豁免** | `test/dart_library_guard_test.dart` 判据③ 对含 `listSync(recursive:true)` 的**整文件豁免**；本轮实测它**掩盖了** `no_backdrop_filter_test` 的直读点（prefs-split **如实上报并仍升级了该处**）⇒ 建议登记为独立债项 |
| 6 | **nightly 未清减** | `web-offline-serve-check` 仍构建**未清减**产物（不判体积） |

## 5. 本轮踩到的坑（4 条）

1. **预算要能表达小数，才配得上「实测值」**：`WASM_DIST_BUDGET_MIB` 原是 `u64`，**表达不了 4.2**——
   要么写成 4（继续假绿）要么写成 5（凭空放宽）。改成 `Mib` 单位类型（允许小数、按字节精确）
   才能既如实、又不放宽到没有意义。
2. **Dart 守卫的整文件豁免会掩盖直读点**：`dart_library_guard_test.dart` 判据③ 对含
   `listSync(recursive: true)` 的**整文件豁免**，于是 `no_backdrop_filter_test` 里那处直读
   **骗过了门禁**（本轮是人工复核抓到的）。**豁免必须精确到行/调用点，不能整文件放行。**
3. **清减脚本必须保住 `chromium/`**：既有调研结论「`chromium/` 是 canvaskit 的无用副本」是**错的**；
   真实 Network 抓到 `chromium/canvaskit.wasm` **200 ×4**，照删会**白屏**。删任何产物前先抓一次真实请求。
4. **跑动期间「写者 / 验证者」必须先声明边界**：验证者（V1）与写者同时动同一棵树 ⇒ 复核结论必须
   先写清**被测 SHA + 写面**，否则数字对不上时无法判断是回归还是别人刚写进去的。**冻结基线 = 一个 SHA + 一份声明。**

### 5.1 V1 补记：一条**静默失效**（已修）+ 一条**边界登记**（不是 bug）

> 来源：V1 第二轮终局独立复验报告
> [docs/verification/gate-baseline-2026-10-06.md](../verification/gate-baseline-2026-10-06.md) **§9.4**
> （「我自己额外抓到的两个问题」）。**V2 复验**见同一份文件的 **§9.8**（落盘后以其实际标题为准）。

**① 发现 A —— 真问题、已修（`5e8193e`）**：`83c89e1` 把 `FLUTTER_WEB_BUDGET_MIB` 从 `u64`
改成新的 `Mib(35.0)` 类型后，`scripts/prune_web_artifacts.sh` **第 4 条判据「预算判据漂移」**
的 `sed` 仍按**旧的 `u64` 形状**匹配 ⇒ **零命中** ⇒ 走 `[warn]` ⇒ **永不 FAIL**
（两处当时恰好都是 35，**没有任何东西在守它**）。

- 修法：解析**同时认两种形状**；**解析不到直接判 FAIL**（不再是 `warn`）；比对改 **awk 数值比较**。
- **红绿自证**：未篡改 → **exit 0**；把 `xtask` 常量临时改成 `Mib(40.0)` →
  **`[FAIL] 预算判据漂移` / exit 1**；复原 → **exit 0**。

**② 发现 B —— 边界登记（不是 bug）**：死重扫描面是 **`build/web/canvaskit/**`**
（`kill_list()` 的两个 `find` 都以 `$DIR/canvaskit` 为根）⇒ 放在 `build/web` **根目录**的
`*.symbols` **抓不到**（V1 实测 exit 0）。Flutter 引擎只会把 `*.symbols` 拷进 `canvaskit/`，
脚本头注也写了清单来源 ⇒ **语义正确**，仅**任务书措辞**有歧义。

**教训**：跨类型重构（`u64` → `Mib`）时，**任何按旧形状解析的下游都要一起改**——否则
「匹配不到」会被写成 `warn` 而不是 `FAIL`，门禁就变成**装饰**。「解析不到」永远该是**红**，不是提示。

## 6. 文档指针

- 变更历史（本轮）：[AGENTS.md](../../AGENTS.md) §变更历史 → `2026-10-06 夜（第二轮团队 / 0.2 工程债清算…）`；
  门禁表新增「前端产物预算门禁」一行（本地 `prune_web_artifacts.sh --check` ↔ CI `flutter-web-offline-artifacts`）。
- 产物预算契约：[docs/architecture/artifact-budget.md](../architecture/artifact-budget.md)
- 决策纸（本轮执行的依据）：[DECISION-display-prefs-2026-10-06.md](DECISION-display-prefs-2026-10-06.md)（E2）、
  [DECISION-artifact-budget-2026-10-06.md](DECISION-artifact-budget-2026-10-06.md)（E4）
- 下一轮清单：[NEXT-ROUND-main-2026-10-06.md](NEXT-ROUND-main-2026-10-06.md)
- **V1 终局复验**：独立复核，结果落盘在
  [docs/verification/gate-baseline-2026-10-06.md](../verification/gate-baseline-2026-10-06.md) **§9**。
  **本文与 AGENTS.md 都不替它下结论**——要数字就去看那份文件。
- 上一份交接：[HANDOFF-2026-10-06-team-round.md](HANDOFF-2026-10-06-team-round.md)（T1–T5 团队轮）

## 7. 变更历史

- 2026-10-06 夜：初版（第二轮团队 / 0.2 工程债清算收尾落盘）。
