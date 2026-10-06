# 交接：E8 文档减量 + E15/E16 收口（2026-10-06 夜 · 第四轮）

> **接手先读本文件**。上一轮（0.2 工程债清算：E2/E12/E4/E13/R8）见
> [`HANDOFF-2026-10-06-team-round-2.md`](HANDOFF-2026-10-06-team-round-2.md)。
> **本轮状态**：单提交 `7918974` · 工作树干净 · 领先 `origin/main` **17** 个提交 · **未推送（无 token）**。
> **主链（LLM/TTS/口型/Live2D）与 Flutter `lib/` 本轮一行未改**。改动只有三类：文档、Dart 守卫、CI。

## 1. 一分钟上手

- 唯一工作树 `/home/skystar/Live2D-Ai`，分支 `main`；点火 `./scripts/ignite.sh`（18080），体检 `./scripts/ignite.sh --check`。
- **提交前必跑**（与本轮新增门禁一起）：
  ```bash
  cargo test --workspace --all-targets && cargo test --doc --workspace
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings   # 必须带 --all-targets
  cargo run -p xtask -- rust-ratio
  cargo run -p xtask -- code-stats --check                # 五条：lines/over-1000/deps/docs(+dart 在 lines 组)
  cd shell/flutter && flutter analyze && flutter test && cd ../..
  python3 -m pytest tests/ -q && python3 scripts/check_public_secrets.py
  ./scripts/prune_web_artifacts.sh --check                # 需先 flutter build web
  ```
- 本轮基线见 §2；**仍未关**见 §5；**有意为之的边界**见 §4（别当 bug 修）。

## 2. 本轮交付（三件，全部有红-绿自证）

### ① E8 · 文档减量到位 + 变成机器门禁

**两条 PLAN §5 判据双双达标**：

| 判据 | 之前 | 现在 | 结果 |
|---|---:|---:|---|
| docs 行数（**不含 `docs/audit/**`**） | 70,762 | **39,273**（`7918974` 时） | ≤45,000 **PASS**（余量 5,727） |
| `docs/plans` 顶层份数 | 33 | **21** | ≤25 **PASS** |

- **做法 = 移出工作树**（不是搬运、不是压缩）：**139 文件 / 31,509 行** —— `docs/legacy/**` 108、
  `docs/design/legacy/**` 9、2026-08「节点 C」/原生壳验证 10、**已收口计划 12**。
- **逐份可逐字取回**：[已移出工作树的文档索引](../REMOVED-docs-index-2026-10-06.md)
  （`git show 42c5825:<路径>`；`42c5825` = 移出前最后一个提交）。
- **引用刻意不对称**：活文档已改链；**历史文档保留死链**（改历史引用 = 篡改记录）。
- **机器门禁**：`xtask code_stats` 新增门禁组 **`docs`**（`DOCS_BUDGET_LINES = PLAN_DOCS_LINES = 45_000`，
  判**不含 audit**）；`collect.rs` 一次扫描出「含 / 不含 audit」两数（报告 §1 两行都打印）；
  **单独接进 `pr-checks.yml`**（`--only docs`）——原来三条 `--only lines/over-1000/deps` 都不含 docs，
  不接这条 = 门禁只存在于本地。
- **红-绿自证**：常量临时改 `30_000` → `FAIL docs-lines：当前 39257 ＞ 上限 30000（当时值）` / **exit 1**；复原 → **PASS / exit 0**。
  单测 `docs_budget_gate_excludes_audit_and_is_two_way`（含「audit 不得计入」判别力断言），`cargo test -p xtask` **27 → 28 passed**。
- **没有做（如实）**：没有伪造「压缩/合并」——不存在把 25k 行历史压成 2k 行的诚实做法；
  研究类 `docs/research/**` 未动（结论仍被引用）；`docs/audit/**` 未动（只读过程产物）。

### ② E15 · Dart 守卫的「整文件豁免」收窄为逐调用点

`shell/flutter/test/dart_library_guard_test.dart` 判据③ 原来是
`if (src.contains('listSync(recursive: true)')) continue;` —— 只要文件里**任何一处**目录遍历，
**全文所有**变量路径读取点一起免检（实测就是这么让 `no_backdrop_filter_test` 的直读点溜过去的）。

- 现在是**逐调用点白名单** `varReadExemptions`（4 条，各带**必填 reason**）+ **两条自证判据**：
  ① 白名单必须**恰好命中一个**真实调用点（失准 / 零命中 / 没写理由 ⇒ 红）；
  ② 该文件必须仍有**机械锚点**（目录遍历，或一个**不声明 part** 的 `lib/*.dart` 字面量），锚点消失 ⇒ 红。
- **红-绿三向自证**：删一条豁免 → 该调用点被点名判红；插一条假豁免 → 「命中 0 次」判红；
  锚点函数改成恒假 → 四条豁免全部判红；复原 → `flutter test test/dart_library_guard_test.dart` **3 passed / exit 0**。

### ③ E16 · nightly 与 PR 用同一份产物形态

`nightly.yml` 的 `web-offline-serve-check` 从前把**未清减**的 `build/web` 喂给托管层测试
（nightly 绿 ≠ 用户拿到的产物绿）。现在构建后、起服务前补两步：
`./scripts/prune_web_artifacts.sh` → `--check` —— 与 `flutter-checks.yml` 的 PR 路径**同一份形态 + 同一份四条判据**。
本机复跑：prune **exit 0**、`--check` **exit 0**（死重 0 / 红线 9 项齐全 / 29.38 MiB ≤ 35 / 预算不漂移）；YAML 可解析。
**真 runner 首跑仍受 E7 阻塞**（见 §5）。

## 3. 本轮门禁实测（提交 `7918974`，逐条真跑）

cargo test --workspace --all-targets **1316 passed / 0 failed**（+1 新单测）· `--doc` **3**
· fmt clean · clippy `--all-targets -D warnings` **exit 0** · rust-ratio **96.1126% PASS**
· `code-stats --check` **五条 PASS**（>500 44/44 · Dart>800 0/0 · >1000 0/0 · deps 22/22 · **docs 39,273 ≤ 45,000**）
· flutter analyze **No issues found** · flutter test **1613** · pytest **22 passed / 1 skipped**
· 密钥扫描 **2011 文件 ok** · `prune --check` **exit 0** · 两条 workflow YAML 可解析。

## 4. 有意为之的边界（**不是 bug，别顺手改**）

1. **历史文档里的 `docs/legacy/…` 引用保留为死链** —— 这是「这份文档写于移出之前」的信号；取回见索引。
2. **两个行数口径差 6 行**：`find|cat|wc -l` 数换行符；xtask `physical_lines` 把无末尾换行的残行也算 1 行。
   **预算判据只认 xtask 口径**（`code-stats --only docs`）。详见 `docs/DOC-MAP.md` §3。
3. **`docs/audit/**`（1,014 份 / 75,154 行）不计入预算**、只读；它只增不减。
4. **E14 = 维护者裁决「不做」**：dist 仅披露（`WASM_DIST_BUDGET_MIB` 4.2 MiB 如实打印「超预算」），不设门禁。

## 5. 仍未关 / 需要维护者

| # | 事项 | 现状 | 下一步 |
|---|---|---|---|
| 1 | **推送** | `git push origin main` 仍 `No anonymous write access`，**17 个提交只在本地** | 提供 token（或改用别的推送方式） |
| 2 | **E7 CI 真 runner 首跑** | 本机无 runner / 无 token；**本轮新增的两条 CI 步骤（nightly 的 prune+check、pr-checks 的 `--only docs`）也未经真 runner 验证** | 首次 nightly 原始日志；红则修 |
| 3 | docs 预算增长纪律 | 39,273，余量 5,727 | 逼近 45,000 时按 `DOC-MAP` §4「移出四步」处理，**不是**调大常量 |

## 6. 本轮踩到的坑（下一轮别重犯）

1. **数字漂移循环**：写文档会改变「docs 行数」这个被引用的量 ⇒ 收尾必须**先定稿内容**，
   最后**一次性同值替换**，并**以 xtask 口径为准**（本文件里的 39,273 就是落盘提交的值）。
2. **口径差**：`find|cat|wc -l` 与 xtask `physical_lines` 差 6 行（见 §4.2）。引用数字前先想清楚用哪个口径。
3. **`flutter analyze` 不能只看 exit code**：本轮新写的 Dart 字符串拼接触发 6 条
   `prefer_interpolation_to_compose_strings`（info 级，退出码仍 0），必须看 `No issues found!`。
4. **删文档前必须确认全部 tracked**：本次 139 份全 tracked，故 `git show` 可逐字取回；
   未提交过的文件**不许删**（那是真丢）。
5. 工具侧：`run_code` 程序里反引号与 `\${` 需要转义，否则整段程序解析失败（本轮中招数次）。

## 7. 文档指针

- 生命周期与体量口径：[`docs/DOC-MAP.md`](../DOC-MAP.md) §2/§3/§4（**已按本轮新口径重写**）
- 取回清单：[`docs/REMOVED-docs-index-2026-10-06.md`](../REMOVED-docs-index-2026-10-06.md)
- 残项台账（本轮的 E8/E14/E15/E16 状态已就地更新）：[`NEXT-ROUND-main-2026-10-06.md`](NEXT-ROUND-main-2026-10-06.md)
- D5 达标记录：[`PLAN-debloat-and-closeout-2026-10-01.md`](PLAN-debloat-and-closeout-2026-10-01.md) §D5 + 量化目标表
- 变更历史：[`AGENTS.md`](../../AGENTS.md)「2026-10-06 夜（第四轮：…）」；门禁表新增 **docs 行数预算**行
- 上一轮交接：[`HANDOFF-2026-10-06-team-round-2.md`](HANDOFF-2026-10-06-team-round-2.md)；独立复核：[`gate-baseline-2026-10-06.md`](../verification/gate-baseline-2026-10-06.md)
