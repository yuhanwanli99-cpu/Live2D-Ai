# GROUNDING · W-VERIFY-2A（独立复核 W2-A/D1 第一段：删除 3 个已归档 Mod crate）

> 复核者：`verifier`（非实施者，**只读仓库**；唯一写入处 = `docs/audit/2026-10-01-debloat/**`）
> 复核对象：`42ca9a37 refactor(debloat)!: W2-A/D1 第一段 —— 删除 3 个已归档 Mod crate`
> 还原点：`checkpoint/pre-d1-dormant` = `d140604f`
> **工作树纪律**：工作树当时有 `dart-fixer-a`（W1-b）与 `rust-debloat`（W2-B）在飞 ⇒
> 本复核**全部在归档副本里跑**：`/tmp/w2a_before`（`d140604f`）· `/tmp/w2a_after`（`42ca9a37`），
> 共用 `CARGO_TARGET_DIR=/tmp/w2_verify_target`（省一次全依赖编译，且**不碰**仓库的 `target/`）。
> 本轮**未改任何源码/已有文档、未 commit、未碰 `/home/skystar/Live2D-Ai`**。

---

## 0. 命令与前提（原文）

```console
$ git -C /home/skystar/Live2D-Ai-fe archive d140604f | tar -x -C /tmp/w2a_before
$ git -C /home/skystar/Live2D-Ai-fe archive 42ca9a37  | tar -x -C /tmp/w2a_after
$ export CARGO_TARGET_DIR=/tmp/w2_verify_target
# 时间: 2026-10-01T23:14:41+08:00；df -h / → 147G 107G 33G 77%
```
脚本（可原样重跑）：`raw/run_w2a_backend.sh`　原始输出：`raw/w2a_backend.txt`（12 KB）

## 1. 提交范围（`git show --stat` / `--name-status`）

```console
$ git show --name-status --format="" 42ca9a37
M	AGENTS.md
M	Cargo.lock
M	Cargo.toml
D	crates/live2d-ai-mod-local-llm/Cargo.toml
D	crates/live2d-ai-mod-local-llm/src/lib.rs
D	crates/live2d-ai-mod-pet-desktop/Cargo.toml
D	crates/live2d-ai-mod-pet-desktop/src/lib.rs
D	crates/live2d-ai-mod-pet-desktop/tests/pet_desktop_state.rs
D	crates/live2d-ai-mod-wallpaper/Cargo.toml
D	crates/live2d-ai-mod-wallpaper/src/lib.rs
D	crates/live2d-ai-mod-wallpaper/src/strategy.rs
M	docs/architecture/ARCHIVED-mods.md
M	scripts/ignition-precheck.sh
```

| 判据 | 结果 |
|---|---|
| 删除 = **恰好 8 个文件** | ✅ 3× `Cargo.toml` + 4× `src/*.rs` + `tests/pet_desktop_state.rs`（逐条见上） |
| 修改 = 5 个文件 | ✅ `Cargo.toml` / `Cargo.lock` / `AGENTS.md` / `docs/architecture/ARCHIVED-mods.md` / `scripts/ignition-precheck.sh` |
| **无夹带**（尤其 `crates/live2d-ai-desktop/**`） | ✅ `git diff --name-only 42ca9a37^ 42ca9a37 -- crates/live2d-ai-desktop shell/flutter \| wc -l` → **0** |
| 总账 | `13 files changed, 157 insertions(+), 3090 deletions(-)`；删除行 3090 = 8 个被删文件总行（830+16+729+655+16+362+415+16 = **3,039**）+ 文档/脚本净改动（3090−3039 = 51）✓ 自洽 |

## 2. 归档副本上的后端全量门禁（`42ca9a37`）

| # | 命令 | 原始结果 | 期望 | 判定 |
|---|---|---|---|---|
| A | `cargo test --workspace --all-targets` | **1410 passed / 0 failed**（22 个 target，`test result: FAILED` 出现 0 次） | 1410/0 | ✅ |
| B | `cargo test --doc --workspace` | `3 passed; 0 failed`（runtime 三条） | 3 | ✅ |
| C | `cargo fmt --all -- --check` | 无输出，`FMT_EXIT=0` | clean | ✅ |
| D | `cargo clippy --workspace --all-targets -- -D warnings` | `Finished dev profile … in 28.14s`，exit 0（0 warning） | 0 | ✅ |
| E | `cargo run -p xtask -- code-stats` | **Rust 生产 67,310** · 逐 crate 表 **13 行**（12 crate + xtask）· `>500` **55** · `>1000` 4 · `deps=34` | 67,310 / 13 / 55 | ✅ **逐项一致** |
| F | `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` | `Finished … in 28.31s`，exit 0（**7 warnings**，非 error） | ok | ✅（见 §7 注 2） |

**1410 的对账**：`d140604f` 树（before）运行数 = **1474**（我在 §3 的 `--list` 里逐条数出），
减去 3 个 crate 的 64 条 = **1410** ✅ 与实测**逐位相同**。

## 3. 集合差（**自己算的多重集差**，不采信「64 条」口径）

脚本：`raw/list_diff2.py`（自写；先按 `(binary_stem, test_name)` 建多重集，再按**目录位置**
把集成测试文件归属到 crate）　原始输出：`raw/w2a_list_diff.txt`

```console
$ cargo test --workspace --all-targets -- --list        # 两棵树各跑一次
$ python3 raw/list_diff2.py raw/w2a_before_list_raw.txt raw/w2a_after_list_raw.txt /tmp/w2a_before
== 总体 ==
before 用例数 = 1474   after 用例数 = 1410   （二进制目标：before 26 / after 22）
removed=64  added=0  kept=1410

== 退出集合按 binary/crate 归类 ==
  live2d_ai_mod_local_llm   crate=live2d-ai-mod-local-llm     退出= 10  (unittests src/lib.rs)
  live2d_ai_mod_wallpaper   crate=live2d-ai-mod-wallpaper     退出= 37  (unittests src/lib.rs)
  pet_desktop_state         crate=live2d-ai-mod-pet-desktop   退出= 17  (integration tests/pet_desktop_state.rs)
  合计退出 = 64

== 新增集合（应为空）==  （无）
== 主链集合差（不属于被删 crate 的 binary）==  主链 removed = 0   ✅ 0
== 退出用例按 crate 计数 ==  local-llm 10 · pet-desktop 17 · wallpaper 37
== 守卫测试（after 树）==  'web_api::cli_entry::tests::default_mods_manifest_enables_external_input_only' → [('live2d_ai_desktop', …)]
```

| 判据 | 结果 |
|---|---|
| 退出集合 = 三个被删 crate 的测试集合 | ✅ **恰好 64 = 10 + 37 + 17**（逐条 64 行见 `raw/w2a_list_diff.txt`） |
| **新增集合** | ✅ **0 条**（删的同时没有别处冒出新测试） |
| **主链集合差 = 0** | ✅ 属于 `l2d` / `live2d-ai-core` / `live2d-ai-desktop` / `live2d-ai-runtime` / `l2d-wasm-demo` / `mod-system` / `mod-external-input` / `mod-persona` / `mod-template` / `mod-voice-input` / `mod-memory` / `mod-director` / `xtask` 的用例**一条没少**（`kept=1410`） |
| 守卫测试仍在且绿 | ✅ `--list` 在 after 树命中；全量运行里 `test web_api::cli_entry::tests::default_mods_manifest_enables_external_input_only ... ok` |
| 二进制目标数 | 26 → 22（−4 = 3 个 lib + 1 个集成测试）✓ 与「删 3 crate」一致 |
| **台账 64 条全名 vs 实际** | ✅ **逐名完全一致**：`ARCHIVED-mods.md` 里列出的 64 个名字（wallpaper 37 + pet 17 + local-llm 10）与实测退出集合 **0 漏列 / 0 多列**（我按集合差机械对照） |

> 说明：pet-desktop 的 `src/lib.rs` 内联测试 **0 条**（`0 tests, 0 benchmarks`），17 条**全部**来自
> `tests/pet_desktop_state.rs` —— 与台账 §2.4 的自述一致 ✅。

## 4. 反例尝试（逐条）

| # | 反例目标 | 做法/原文 | 结果 |
|---|---|---|---|
| 4.1 | `mod_count` 断言是否仍守 5 | `diff <(git show d140604f:crates/live2d-ai-desktop/src/main.rs) <(git show 42ca9a37:…)` | ✅ **整文件逐字相同（0 差异）**；`main.rs:471 fn mod_count_is_five` 断言 `AVAILABLE_MOD_FACTORIES.len() == 5` 原样 |
| 4.2 | `cli_entry.rs` 两条 absence 断言是否保留 | 同上 `diff` 整个 `web_api/cli_entry.rs` | ✅ **逐字相同**；两条断言原文：`m["mods"].get("local-llm").is_none()`（"local-llm 已废除启动（0.2.0-rc.1），不得再进缺省 manifest"）与 `m["mods"].get("wallpaper").is_none()`（"wallpaper 缺省停用（mode 缺省 off），不得进缺省 manifest"） |
| 4.3 | 是否还有对已删 crate 的**代码**引用 | `grep -rn "live2d_ai_mod_\(local_llm\|wallpaper\|pet_desktop\)" --include=*.rs crates/ xtask/`（在 after 树） | ✅ **0 命中**（与你独立跑的结果一致） |
| 4.4 | 连字符形式是否还有依赖 | `grep -rn "live2d-ai-mod-\(local-llm\|wallpaper\|pet-desktop\)" --include=*.toml --include=*.rs .` | ⚠ **3 命中，但全部在 `crates/live2d-ai-desktop/Cargo.toml` 的注释里**（`:50/:53/:59`），且措辞已**过时**（写「**已封存**（ARCHIVED）」而 crate 已被删除）——见 F-2A-1 |
| 4.5 | 三个目录是否真没了 | `ls -d crates/live2d-ai-mod-{local-llm,wallpaper,pet-desktop}` | ✅ 三者 `No such file or directory`；`crates/` 由 15 目录 → **12**（+ xtask = 13 crate） |

## 5. `Cargo.lock`（−27 行 / +0 行）

```console
$ git diff --numstat 42ca9a37^ 42ca9a37 -- Cargo.lock
0	27	Cargo.lock
$ git diff 42ca9a37^ 42ca9a37 -- Cargo.lock | grep -c '^+[^+]'
0
```
✅ 27 行删除**恰好是 3 个 `[[package]]` 块**（`live2d-ai-mod-local-llm` / `live2d-ai-mod-pet-desktop` /
`live2d-ai-mod-wallpaper`，各 9 行：块名 + version + dependencies 三行 + 花括号），**0 新增**、
没有牵动任何其它 package 的版本或依赖边。

根 `Cargo.toml`：`4 -` / `0 +` = 3 行 members + 1 行注释（wallpaper 的注释行），逐行见 diff。

## 6. 红线

| 红线 | 核验 | 结论 |
|---|---|---|
| `shell/flutter/**` 零改动 | `git diff --name-only 42ca9a37^ 42ca9a37 -- shell/flutter \| wc -l` → 0 | ✅ |
| WS 帧 / `ws_status` | 该 commit 未碰 `crates/live2d-ai-desktop/**`（含 `web_api/**`）⇒ 结构性未触碰 | ✅ |
| `mod_count` 断言 | 见 §4.1（整文件逐字相同） | ✅ **未改** |
| `clean_for_tts` / 密钥 / A1–A8 / `IdleState` | `live2d-ai-runtime` / `l2d` / `l2d-wasm-demo` / `live2d-ai-core` 均不在改动清单里 | ✅ |
| 测试条数 | 1474 → **1410**（−64，**全部**来自被删的 3 个 crate，逐条已列）；**主链 0 减少** | ✅ 红线 8 修订版（休眠资产显式冻结 + 主链集合差 = 0）成立 |

## 7. 与「验收数字」的对账（两处口径修正）

1. **`code-stats` 的 Δ 与 task-11 的预期不符，但实测值可逐项解释**：
   提案里写「Rust 生产 −2,020、文件 −8」，实测是——

   | 桶 | before（`d140604f`） | after（`42ca9a37`） | Δ | 解释 |
   |---|---:|---:|---:|---|
   | Rust 生产 | 68,867 | **67,310** | **−1,557** | = 三个 crate 的 `prod` 之和 **551 + 644 + 362** ✅ |
   | Rust 内联测试 | 16,326 | 15,307 | −1,019 | = **279 + 740 + 0** ✅ |
   | Rust 集成/示例 | 13,542 | 13,127 | −415 | = pet-desktop 的 `tests/pet_desktop_state.rs` **415** ✅ |
   | 生产文件数 | 243 | 239 | **−4** | 4 个 `src/*.rs`（集成测试文件进「集成」桶） |
   | 集成文件数 | 31 | 30 | −1 | 同上 |
   | crate 数 | 16 | **13** | −3 | ✅ 与你的期望一致 |
   | `>500` 文件 | 58 | **55** | −3 | 被删的 6 个 `.rs` 里只有 3 个 >500（830/729/655）|

   ⇒ **−2,020 / −8 这两个预期数不成立于 `code-stats` 的口径**（−8 是把 3 个 `Cargo.toml` 也算进去了；
   −2,020 与任何桶都不等）。**实测的 −1,557 才是正确口径**（且恰好等于工具自己 before 表里那三行 `prod` 的和）。
   结论：**验收数字本身要更正，删除行为无问题**（F-2A-2）。

   > **before 数字的来源与有效性**：上表 before 行取自 W0b 复核时在 `d1e29ba3` 上跑的 `code-stats`
   > （prod 68,867 / inline 16,326 / integ 13,542 / 生产文件 243 / 集成文件 31 / `>500`=58 / crate 16）。
   > 我先确认了 **`d1e29ba3 → d140604f` 之间 Rust 侧零改动**
   > （`git diff --name-only d1e29ba3 d140604f -- crates xtask Cargo.toml Cargo.lock` → 0 行，W1 全是 Dart），
   > 所以这些数字对 `d140604f` 同样成立，可以直接与 after 相减。
2. **`wasm` 那条约 7 个 warning**：`cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` **exit 0**，
   但有 `warning: … generated 7 warnings`（dead_code 类）。`l2d-wasm-demo` 不在本 commit 的改动清单里
   ⇒ 与 W2-A **无关**（判据是「仍 ok」，成立）。

## 8. 发现索引

| ID | 级别 | 一句话 | 位置 |
|---|---|---|---|
| F-2A-1 | P4 | `crates/live2d-ai-desktop/Cargo.toml:50/:53/:59` 三条**注释**仍写 `live2d-ai-mod-{pet-desktop,local-llm,wallpaper}` **「已封存（ARCHIVED）/已废除启动」**，而 crate 已被删除 ⇒ 措辞过时（读者会去找不存在的 crate）。该文件不在 task-11 的写范围内，故**不是违规**，只是漏收尾 | 代码注释 |
| F-2A-2 | P4 | task-11 的验收预期「Rust 生产 −2,020、文件 −8」与实测（**−1,557 / 生产文件 −4**）不符；实测值可逐项解释（§7.1）。**release note / 台账里的数字要按实测写** | 验收口径 |
| F-2A-3 | P4 | `ARCHIVED-mods.md:70` 写 wallpaper「**8 文件** / 1,400 行」，实测是 **3 文件** / 1,400 行（行数对、文件数错；另两个 crate 的 2/846、3/793 都对） | 台账 |

**没有发现 P1/P2/P3 级问题**：范围、集合差、红线、lock、断言全部通过。

## 9. 未核实栏（**不许空**）

1. **产物体积两行未验**：归档副本里 `shell/flutter/build/web` 与 `crates/l2d-wasm-demo/dist` 都不存在
   （gitignored，不进 `git archive`）⇒ `code-stats` §4 显示「缺 / 未构建」。真树上的体积未复核。
2. **`ignite.sh --check` 未跑**（无服务，同前几轮）：`/` 302、`/app/` 200、HTTP 字节不含 gstatic **未验**。
3. **真机 / 端到端未验**：没有起服务、没有浏览器 ⇒ 「删 3 个 crate 后产品行为不变」只有
   **编译 + 测试集合差**证据；`/api/v1/mods` 的实际响应（5 个 Mod 仍在）未实测。
4. **`ARCHIVED-mods.md` 的恢复步骤未实跑**：台账给了 `git checkout checkpoint/pre-d1-dormant -- <paths>` 类步骤
   与「恢复后 `cargo test -p …` 期望条数」，我**只核了名字集合**，**没有真的恢复一次**去验证步骤可执行。
5. **`AGENTS.md` / 台账的其余文案未逐字校**（只校了与删除相关的状态句与 64 条名字）。
6. **`xtask` 的 `--check` 棘轮是否随之收紧未验**：本 commit 把 `>500` 从 58 降到 55、`>1000` 仍 4、
   deps 仍 34 ⇒ 按我在 W0 复核里提的 F-V0-9，**棘轮常量是否在同一 commit 收紧**需要看 `xtask/src/code_stats/mod.rs`
   —— 本 commit **没有**改 `xtask/**`（不在改动清单），⇒ 棘轮仍是 58/7/4/34。**这不是缺陷**
   （W2-B 才动 deps），但请注意：**现在 CI 允许再退回 58 个 >500 文件而不红**，建议在 W2 波末统一收紧一次。
7. **W2-B 在飞**：`rust-debloat` 正在同一工作树做第二段，我未读、未碰它的改动，避免污染。

---

## 附：原始输出清单（`docs/audit/2026-10-01-debloat/W2/raw/`）

| 文件 | 内容 |
|---|---|
| `w2a_backend.txt` | A–H 全部门禁的原始输出（含 code-stats 全文） |
| `run_w2a_backend.sh` | 上面那条流水线的脚本（可原样重跑） |
| `w2a_after_test_raw.txt` | `cargo test --workspace --all-targets` 全文（1410/0 的求和来源） |
| `w2a_before_list_raw.txt` / `w2a_after_list_raw.txt` | 两棵树的 `--list` 全文（26 / 22 个目标块） |
| `w2a_list_diff.txt` | **自算的多重集差**（64 条逐条 + 主链差 0 + 守卫测试） |
| `list_diff.py` / `list_diff2.py` | 差分脚本（v2 修了 cargo `Running` 两种行格式） |
| `ARCHIVED-mods_42ca9a37.md` | 台账快照（离线查阅；64 条名字即来自此文件） |

**本复核未修改任何源码 / 配置 / 已有文档**；唯一写入 = `docs/audit/2026-10-01-debloat/**`。
**未 commit、未 push、未跑任何破坏性 git 命令；未触碰 `/home/skystar/Live2D-Ai`。**
