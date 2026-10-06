# 下一轮清单（**在 main 上开工**）· 2026-10-06 补齐轮交接

> **接手先读**：[`HANDOFF-2026-10-06-team-round-2.md`](HANDOFF-2026-10-06-team-round-2.md)
> （2026-10-06 夜**第二轮团队 / 0.2 工程债清算**：E2+E12 / 棘轮归零 + dist 预算重定 / E4+E13+CI；
> 含一分钟上手、三个提交、各轨实测数字、**仍未关清单 6 条**、本轮 4 个坑，以及 **V1 终局复验的指针**）。
> 上一份（团队轮：冻结基线独立复核（**clippy 真红**）+ 文档漂移收口 + E9 探针 + E2/E4 决策论证）：
> [`HANDOFF-2026-10-06-team-round.md`](HANDOFF-2026-10-06-team-round.md)。
> 上一份（工作树单一化 + E1 拆分 + 台账 6 条 P1 全清）：
> [`HANDOFF-2026-10-06-e1-e5-debt-round.md`](HANDOFF-2026-10-06-e1-e5-debt-round.md)。
>
> 本轮报告：[`../audit/2026-10-06-gaps-round/ROUND-REPORT.md`](../audit/2026-10-06-gaps-round/ROUND-REPORT.md)。
> 本文只写**没做完的**，每条给「现状 / 可执行规格 / 验收判据」。纪律不变：回源码复核（行号会腐烂）、
> **不许伪造绿灯**、测试只增不减、共享单写者文件串行。

## P0 · 只有维护者能做

| # | 事项 | 现状 | 规格 / 判据 |
| --- | --- | --- | --- |
| ~~M1~~ | ~~轮换手机锁屏口令~~ | **已关闭（2026-10-06 夜）**：维护者已轮换那台手机的锁屏口令 ⇒ 远端残留 blob 里的口令**已失效**（残留本身仍在，见 §4 与 [`../audit/2026-10-06-purge/PURGE-RECORD.md`](../audit/2026-10-06-purge/PURGE-RECORD.md)） | 无。要彻底过期旧对象只剩 GitHub Support（非必需） |
| ~~M2~~ | ~~维护者肉眼/听音清单~~ | **已关闭（2026-10-06）** | 维护者确认肉眼/听音验收通过，并指示删掉勾选表 ⇒ `docs/verification/v0.2.0-checklist.md` 已退役删除 |
| ~~M3~~ | ~~舞台 WebGPU canvas 像素与拖动手柄~~ | **已关闭（2026-10-06）**（随 M2 一并由维护者肉眼确认） | 探针里那 4 项 `manual-only` **仍如实标注，没算成 pass** |

## P1 · 工程债（可立即开工）

| # | 事项 | 现状 / 规格 | 判据 |
| --- | --- | --- | --- |
| ~~**E1**~~ | ~~`main.dart` 1411 行拆分~~ | **已完成（提交 `b0b0365`）**：20 处源码扫描守卫改走 `test/support/dart_library.dart` 的 `readLibrarySource()`；`main.dart` **1417 → 657 行**（5 个 part）；新增门禁 `test/dart_library_guard_test.dart`（破坏即红自证见 T4 报告 §4.2） | — |
| ~~**E2**~~ | ~~`display_prefs.dart` 1169 行~~ | **已完成（提交 `2790a2c`，按决策纸 B2 执行）**：1169 → **583 行**；新增 5 个**同库 part**（playlist 225 / codec 225 / limits 149 / derived 91 / copy 71，全部 <800）⇒ Dart `lib >800` **1 → 0**。24 字段 / const 构造 / static const / `==` / `hashCode` / `toString` **留在类里**；静态 API 面 **19 行转发**、**调用点零改动**；**逐字非循环自证**（剥 75 处 `DisplayPrefs.` 后原始 25 段逐字出现）；守卫 `display_prefs_test.dart:431` / `no_backdrop_filter_test.dart:543` 改走 `readLibrarySource()`（红-绿自证）+ 修 `setting_wiring_test` 过滤条件 | — （**E12** 同提交落地） |
| **E3** | `>500` 棘轮顶格 | Rust `>500` 现为 **44/44**；**Dart `>800` 棘轮已两次同 commit 收紧：`2 → 1`（`f5210f3`）→ `1 → 0`（`83c89e1`，实测 0）** | **新增任何 >500 行的 src 文件必须同 commit 拆掉**；**任何让计数下降的改动必须同 commit 收紧常量**，否则 `code-stats --check` 直接红（见 HANDOFF 坑 2） |
| ~~**E4**~~ | ~~产物预算~~ | **已完成（提交 `834a4c6`，按决策纸执行）**：新增 `scripts/prune_web_artifacts.sh`（**234 行**）删 6×`*.symbols` + `skwasm*`/`skwasm_heavy*`/`wimp*` 共 12 项 ⇒ `build/web` **51 964 234 B（49.56 MiB / 66 文件）→ 30 804 686 B（29.38 MiB / 54 文件）**；`--check` 四判据（死重 0 / 红线文件齐全 / ≤35 MiB / 与 `xtask` 预算不漂移）；**`chromium/` 必须保留**（真实 Network：`chromium/canvaskit.wasm` **200 ×4**、死重 0 请求——既有「无用副本」调研**是错的**）；CI 已接线 `flutter-web-offline-artifacts`（构建 → prune → `--check` → `ignite --check-dir`）。**dist 侧**：`WASM_DIST_BUDGET_MIB` **4 → 4.2** + 新 `Mib` 单位类型（`83c89e1`；实测 5.35 MiB、剥 `name` 段 ≈4.11 MiB 仍超 4、本机无 wasm-opt ⇒ **未实测真减、不估数**；重新评审条件 = wasm-opt 可用时回到 ≤4 MiB） | 真验证：`ignite --check-dir` **四条 ok**；`browser_probe net` 两次 **5 项 fail 0**、**ASSERT_FAIL=0**、未预期 **404 = 0**（唯一 404 = 设计内 `/actions/field_map.json`）；**反向自证 3 条**（放回 `*.symbols` / 加 6 MiB 文件 / 移走 `chromium/canvaskit.wasm`）各自判红。契约见 [`docs/architecture/artifact-budget.md`](../architecture/artifact-budget.md) |
| **E5** | ~~台账仍未关闭的 6 条 P1~~ | **已完成（2026-10-06 夜）**：六条全关（细节与红-绿自证见 [`AGENTS.md`](../../AGENTS.md) 变更历史「2026-10-06 夜（第二轮 / 第三轮）」与台账 README 的「关闭状态」）。**仍未关闭的 P1 = 0 条** | — |
| **E6** | ~~D6 假绿灯续（`F-0184-01`）~~ | **已核实关闭（2026-10-06 夜）**：`stripCommentsAndStrings` 全仓**只剩一个定义点**（`test/support/source_scan.dart`），`test/source_scan_test.dart` 的反复制门禁（含零命中判红）**20 条全过**。NEXT-ROUND 原文的「8 份副本」是 W3-D3 合并前的旧状态 | — |
| **E7** | CI 在真 runner 上首跑 | **仍 blocked（2026-10-06 夜 · 第二轮收尾）**：本机无 runner、也无 token；nightly 的 `cargo build` 是否缺 `libasound2-dev` 仍未验证 | 首次 nightly 原始日志；红则修 |
| ~~**E8**~~ | ~~文档减量账~~ | **已完成（2026-10-06，维护者指示「E8 分析修改和文档维护」）**：两条 PLAN §5 判据**双双达标** —— ① docs（**不含 `docs/audit/**`**）**70,762 → 39,273 行**（预算 45,000，余量 **5,727**）；② `docs/plans` 顶层 **33 → 21**（判据 ≤25）。做法 = **移出工作树**（不是搬运、不是压缩）：**139 文件 / 31,509 行**（`docs/legacy/**` 108 + `docs/design/legacy/**` 9 + 2026-08 节点 C / 原生壳验证 10 + 已收口计划 12），逐份登记进 [`docs/REMOVED-docs-index-2026-10-06.md`](../REMOVED-docs-index-2026-10-06.md)（含 `git show 42c5825:<路径>` 取回命令）；**活文档改链、历史文档刻意保留死链**（改历史引用 = 篡改记录）。**新增机器门禁**：`xtask code_stats` 新门禁组 `docs`（`DOCS_BUDGET_LINES`，判**不含 audit**）→ `code-stats --check` 第五条 PASS，并**单独接进 `pr-checks.yml`**（`--only docs`；此前三条 `--only` 都不含 docs ⇒ 不接就等于没有门禁）。`DOC-MAP` §2/§3/§4 按新口径重写。**红-绿自证**：常量改 30,000 → `FAIL docs-lines … exit 1`；复原 → `PASS exit 0` | 复算：`cargo run -p xtask -- code-stats --check`（docs 行 = 39,273 ≤ 45,000 PASS）、`ls docs/plans/*.md | wc -l` = 21、`cargo test -p xtask` 28 passed（新增 `docs_budget_gate_excludes_audit_and_is_two_way`，含「audit 不得计入」的判别力断言） |
| ~~**E9**~~ | ~~探针稳健性~~ | **已完成（T2，提交 `7d53def`）**：① `audio-c` 改**三路取证**（DOM 2000→500 ms + 页内自增 id + 页内媒体记录器 + CDP `Media` 域）⇒ 真实链路两轮全 pass、0 blocked；② 四主题阈值**各自标定**（`themebase` 16 次实测：ON = 基线 + 40%×(红信号−基线)、OFF = 基线 + 12、`maxShare` = share + 0.02）。`all` 同一 HEAD / 同一脚本哈希连跑两遍：各 **40 项 pass 36 · manual-only 4 · fail 0 · blocked 0**、判定 0 处不同 | 证据 [`docs/verification/evidence-2026-10-06-e9/`](../verification/evidence-2026-10-06-e9/README.md)；`8b` 新鲜度重建后真绿；`ignite.sh --check-dir` 四条 ok |
| **E10** | Flutter 升级漂移 | **已复核（2026-10-06 夜 · 第二轮团队）**：`scripts/font_fallback_mirror.sh --check` 当前 **PASS**（清单 == 磁盘 == 引擎表全集，21 文件 / 2 815 292 B，逐文件 sha256 相符） | 升级 Flutter 的那一轮**必须**跑它；**红 = 该重跑生成脚本**的信号，不是 bug——本行保留为常驻提醒 |
| **E11** | **clippy 口径假绿（新，已修）** | **`36937df` 上 `cargo clippy --workspace --all-targets -- -D warnings` 真红 2 条** `doc_lazy_continuation`（`crates/live2d-ai-desktop/src/web_api/tests_mod.rs:276/277`）；**不带 `--all-targets` 不报** ⇒ 上一轮交接的「clippy 0 warning」是**假绿**。修复 = `cc68f06`（「+ 」→「另有 」，纯注释），T4 复检 **EXIT=0** | 常驻纪律：门禁命令**必须**带 `--all-targets`；**改注释也算改代码**（见 HANDOFF 坑 1） |
| ~~**E12**~~ | ~~E2 的 24 键落盘全集零测试覆盖~~ | **已完成（提交 `2790a2c`）**：新增 `shell/flutter/test/display_prefs_persist_keys_test.dart` **30 条断言** —— `toJson` 键集合 == 手写 24 键全集 == const 构造字段集合；24 键逐个**非默认值往返** + `==`/`hashCode` 对称；未知键忽略；旧键迁移。`flutter test` **1583 → 1613** | — |
| ~~**E13**~~ | ~~产物预算无门禁~~ | **已完成（提交 `834a4c6`）**：`build/web` 侧由 `scripts/prune_web_artifacts.sh --check`（死重 0 / 红线文件齐全 / ≤35 MiB / 与 `xtask` 预算不漂移）**变成真门禁**并接进 CI（`flutter-checks.yml` → `flutter-web-offline-artifacts`）；`dist` 侧仍**无机器守门** ⇒ 见 **E14** | — |
| ~~**E14**~~ | ~~dist 无机器守门~~ | **维护者裁决（2026-10-06）：不做**——dist **仅披露**：`WASM_DIST_BUDGET_MIB`（4.2 MiB）由 `xtask code-stats` 如实打印「超预算」，**不**纳入 CI 构建、**不**设门禁。这是**已裁决关闭**，不是「没做完」：`trunk build` 要拉 wasm 工具链，代价与本机无 `wasm-opt` 的现状不成比例 | 无（重新开启条件：真要在 CI 里构建 wasm 渲染面时一并接 `--check`） |
| ~~**E15**~~ | ~~`dart_library_guard_test` 整文件豁免~~ | **已完成（2026-10-06）**：整文件 `continue` 改成**逐调用点白名单** `varReadExemptions`（各带必填 reason），并加**两条自证判据**：① 白名单条目必须**恰好命中一个**真实调用点（失准 / 零命中 / 没写理由 ⇒ 红）；② 该文件必须仍有机械锚点（目录遍历，或一个**不声明 part** 的 `lib/*.dart` 字面量），锚点消失 ⇒ 红。当前 4 条豁免全部对上真实调用点 | **红-绿三向自证**：删掉一条豁免 → 该调用点被点名判红；插入一条假豁免 → 「命中 0 次」判红；把锚点函数改成恒假 → 四条豁免全部判红；复原 → `flutter test test/dart_library_guard_test.dart` **3 passed / exit 0** |
| ~~**E16**~~ | ~~nightly 未清减产物~~ | **已完成（2026-10-06）**：`nightly.yml` 的 `web-offline-serve-check` 在构建后、起服务前补两步 —— `./scripts/prune_web_artifacts.sh` → `./scripts/prune_web_artifacts.sh --check`，与 `flutter-checks.yml` 的 PR 路径**同一份产物形态 + 同一份四条判据**（此前 nightly 绿 ≠ 用户拿到的产物绿） | 本机复跑：prune **exit 0**、`--check` **exit 0**（死重 0 / 红线 9 项齐全 / 29.38 MiB ≤ 35 / 预算不漂移）；`nightly.yml` **YAML 可解析**。**真 runner 首跑仍受 E7 阻塞**——本条不谎报 CI 已验证 |
| **E17** | **prune 预算漂移判据静默失效（V1 抓到，已修）** | `83c89e1` 引入 `Mib(35.0)` 之后，`scripts/prune_web_artifacts.sh` 第 4 条判据「预算判据漂移」的 `sed` 仍按旧 `u64` 形状匹配 ⇒ **零命中 ⇒ `[warn]` ⇒ 永不 FAIL**（两处恰好都是 35，但没有任何东西在守它）。**已修（提交 `5e8193e`）**：解析**同时认两种形状** / 解析不到**直接判 FAIL**（不再是 `warn`）/ 比对改 **awk 数值比较** | **红绿自证**：未篡改 **exit 0** · 把 `xtask` 常量临时改 `Mib(40.0)` → **`[FAIL] 预算判据漂移` exit 1** · 复原 → **exit 0**。**备注（边界登记，不是独立待办）**：死重扫描面只覆盖 `build/web/canvaskit/**`（`kill_list()` 的两个 `find` 均以 `$DIR/canvaskit` 为根）⇒ `build/web` **根目录**的 `*.symbols` 抓不到（V1 实测 exit 0）；引擎只把它拷进 `canvaskit/` ⇒ **语义正确**，仅任务书措辞有歧义。**V2 复验指针**：`docs/verification/gate-baseline-2026-10-06.md` §9.8 |

## P2 · 功能（本轮未碰）

| # | 事项 | 说明 |
| --- | --- | --- |
| **F1** | **R1 正文帧带 `sentence_seq`（朗读高亮）** | 全项目**唯一「不做就永远做不出来」**的一条：音频帧已有 `sentence_seq`，正文帧没有，纯前端无法映射 |
| **F2** | 会话记忆后端 / 动作-语音选型 | 真源 `docs/plans/PLAN-actions-voice-memory-2026-09-15.md`；实施前先改 `AGENTS.md` 的 director 台账措辞 |
| **F3** | R3/R4（舞台图解码状态帧、capabilities 能力发现） | 排障成本相关，可选 |
| **F4** | 离线 CJK 字体回落 | 现在只镜像 5 族（emoji/符号/数学/音乐）；CJK 五族（jp/hk/sc/tc/kr）**约 11.9 MiB** 未镜像 ⇒ 离线时子集外 CJK 是豆腐块 | 若要：要么加镜像（体积代价），要么把「引擎回落表驱动生成」做成可选构建步骤 |

## P3 · 仓库收尾

| # | 事项 | 现状 |
| --- | --- | --- |
| **R1** | ~~移除死树 `-Ai` worktree~~ | **已完成（2026-10-06 夜，方向与原文相反）**：维护者裁「`-Ai` 为主树」⇒ 改为**删除 linked worktree `Live2D-Ai-fe`**、把 `main` 切回 `/home/skystar/Live2D-Ai`（31 G `target/` 与前端产物已迁入，运行态以 `-fe` 为准并入）；A/B 审计台账保运到树外 `/home/skystar/audit-ref-2026-10-06/`（按指示不再运行）。当前 `git worktree list` 只剩 `/home/skystar/Live2D-Ai ... [main]` |
| **R2** | `dev/integrity` / `dev/node-p1-p2` 裁决 | bundle 已覆盖，可降级为档案 |
| **R3** | ~~是否推 `origin`（N18）~~ | **已完成（2026-10-06）**：维护者授权 ⇒ 历史清洗后 force-push `main`、重写后的远端 tag；新 tag `v0.2.1-rc.1` 与 GitHub pre-release 已建。**措辞更正（2026-10-06 夜 · T1）**：原文「删除残留分支 `mainline/1-core-baseline`」与事实不符——**远端** `refs/heads/mainline/1-core-baseline` = `b58b223` **仍然存在**（本机无 token，删不掉）；**本地**那份已于 2026-10-06 夜由 Lead 用 `git branch -d` 删除（连同另外 4 条 0 领先分支，见 R7）。要删远端分支需 GitHub token |
| **R4** | Gitleaks（N19） | 本轮仍不补（多一个第三方 action = 多一条供应链面）；要补单独一轮 |

---

## 4 · 2026-10-06 夜（清洗收尾轮）：新增待办

### 4.1 已关闭 / 已处置（本轮）

- **M1 关闭**：维护者**已轮换那台手机的锁屏口令**。远端 `refs/pull/1/head`（`44da2a4c`）里那个 blob 中的口令**已失效** ⇒ 从「P0 泄密」降级为「技术残留、历史垃圾」（`refs/pull/*` 服务端只读，`DELETE` = 422）。
- **GitHub 凭据**：维护者已删除其 token；本地 `gh` 里那把凭据经 `gh auth status` 判定 **invalid**，本轮据维护者指示执行 `gh auth logout` 从 `~/.config/gh/hosts.yml` 移除（现为 `{}`，`oauth_token` 计数 0）。另查：`~/.git-credentials` 不存在、环境无 `GH_TOKEN`/`GITHUB_TOKEN`、remote URL 无水印式内嵌凭据。
- **清洗备份退役**：`backup-2026-10-06/pre-purge-all-refs.bundle`（**132 448 444 B**，`sha256 c499f8631fadddfc0d30be03c8e882e98e5bcfb2624e429ede0eead1af66a7be`）**已删除**；同目录仅保留 `refs-before.txt`。

### 4.2 新增待办

| # | 事项 | 判据 / 备注 |
| --- | --- | --- |
| **R5** | **本地提交待推**（2026-10-06 夜实测：`git push origin main` **失败**，remote 回 `No anonymous write access.` / `fatal: Authentication failed` ⇒ **无 token**；第二轮团队收尾**仍 blocked**，写作时现测 ahead = **14**） | `git rev-list --count origin/main..main` 回 0；新 token 就绪后 `git push origin main` |
| **R6** | **文档收口四项** ① `AGENTS.md` 首屏「当前版本 `0.2.0`」与树（`0.2.1-rc.1`）不一致；② `archive/action-layer-p6` 被 10+ 处引用（`AGENTS.md` / `CHANGELOG.md` / `README.md` / `README.zh-CN.md` / `docs/architecture/core-chain-baseline.md` / `crates/live2d-ai-desktop/src/main.rs`）但**本地与远端都没有该分支**（T1 已按「分支已不存在」改写写面内全部引用，并给出等价取回命令 `git show 98469df^:…` / `git show ef9f428^:…`；`CHANGELOG.md` 与 `crates/**` 不在 T1 写面，留给 Lead）；③ 本轮清单 R3 的措辞「已删除残留分支 `mainline/1-core-baseline`」**与实际不符**：远端此刻仍有 `refs/heads/mainline/1-core-baseline = b58b223`；④ 上一轮台账已入库，指向它的旧路径写法要收口 | 逐条 `git grep` + `git ls-remote` 复核 —— **已收口（2026-10-06 夜 · T1）**：① 版本口径改 `0.2.1-rc.1`（`Cargo.toml` `version` + tag 实测）、`0.2.0` 降为「上一版」；② 写面内 `archive/action-layer-p6` 引用全部改写（`.md` 侧，含 `docs/architecture/**` 与两份 README）+ 等价取回命令；③ 见 R3 行的措辞更正；④ 旧账本路径统一到 `docs/audit/2026-10-05-ledger/`（`docs/DOC-MAP.md` / `docs/README.md` / §4.3） |
| **R7** | ~~分支清理（13 条非 main）~~ | **已处置（2026-10-06 夜 · 第二轮收尾）**：0 领先的 5 条已用 `git branch -d` 删除 —— `feat/frontend-redesign`（28 落后／0 领先；它 = 远端 tag `v0.2.0` 的提交 `a3f2717`，**删了不丢东西**）· `chore/debt-round-2026-10-05`(24/0) · `mainline/1-core-baseline`(187/0，本地；远端 ref 仍在) · `pr-1`(188/0) · `mod/persona-polish`(151/0)；余 **8 条**本地分支**各自都有独有提交**（`git rev-list --left-right --count main...<b>` 右值 **1…374**）⇒ **全部保留、本轮不再删**；原清单（保留备用）：`archive/action-trigger-p5`(193/356) · `archive/full-history-2026-09-11`(193/374) · `android-archive`(193/1) · `dev/integrity`(193/300) · `dev/node-p1-p2`(193/243) · `backup-local-main-before-force`(193/215) · `feature/node-d-d3-d4`(193/215) · `refactor/elegance`(193/215)。判定命令：`git rev-list --left-right --count main...<b>` |
| ~~**R8**~~ | **已处置（2026-10-06 夜，Lead 执行）**：**删除 6 项**（删前**逐项 `stat` 核对绝对路径**，实测释放 **465 391 616 B ≈ 443.8 MiB**）——`backup-2026-10-05/archive-full-history-2026-09-11.bundle`（114 MB，**实测仍含清洗前链**：其 tip `5e455ad6` 在清洗后的库里 `git cat-file -t` 报 fatal，本地重写后同名分支 tip 为 `90e0fac9`）· `Live2D-Ai-LEGACY-FULL-HISTORY.bundle`(114 MB) · `Live2D-Ai-PY-LEGACY.bundle`(100 MB) · `Live2D-Ai-baseline-28de52cf.bundle`(114 MB) · `Live2D-Ai-baseline-incremental.bundle`(13 MB) · `redesign-backup-2026-09-27.tar.gz`(6.8 MB) · `backups/dsh-data-backup-20260821.tar.gz`(134 MB) | **未删**：`Live2D-Ai-ANDROID-ARCHIVE.bundle`（748 KB，R8 清单未含）· `backup-2026-10-05/` 其余三件（快照 / 补丁 / untracked tar）· `backup-2026-10-06/refs-before.txt` · `/home/skystar/backups/dsh-data-backup-20260821.tar.gz`（128 MB，实测内容是 `.dsh/` **DSH 自身数据**、非仓库历史 ⇒ **不属 R8 范围**，需维护者另行决定） |

### 4.3 审计安排（本轮已定，规程入库）

- 规程：[`AUDIT-PROMPT-whole-repo-2026-10-06.md`](AUDIT-PROMPT-whole-repo-2026-10-06.md)（v3.2；工作树外另存 `/home/skystar/audit-prompt-2026-10-06.md`）。锁 `main` @ `6be9984` 或其后 1–2 个 docs 提交。
- **第一优先 = 前端设置 + Mod 面**（维护者 2026-10-06 指定）：`Phase 0` 九批（`P0-1` `display_prefs` 字段真源 → 控制器/骨架 → 外观 → dev_tools → Mod 面板 → 主链设置 → 契约面 → 后端对照面 → 设置测试质量），配 **§7.1 用户侧审视**七问 + **设置项三方对账**（死字段 / 假旋钮 / 隐藏开关）。
- 台账 `AUDIT-REPO/`（未跟踪，永不 `git add`），批次从 **`BATCH-1001`** 起（上一轮已占用 0001–0927）。
- Rust 主线顺延 `Phase 1` 第 1 项 = `crates/live2d-ai-runtime/**`（上一轮台账恰在此停住）。