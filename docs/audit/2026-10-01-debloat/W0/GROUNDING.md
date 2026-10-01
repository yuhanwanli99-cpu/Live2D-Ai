# GROUNDING · W-VERIFY-0（独立复核 rc.7 基线 + W0 波）

> 复核者：`verifier`（非实施者，只读源码；唯一写入处 = `docs/audit/2026-10-01-debloat/**`）
> 任务：`task-3`（W-VERIFY-0）· 工作树：`/home/skystar/Live2D-Ai-fe`（**未触碰** `/home/skystar/Live2D-Ai`）
> 本文所有结论**必须**能对回本文的命令原文与原始输出；转述不作为证据。
> 原始输出全文另存：`docs/audit/2026-10-01-debloat/W0/raw/`（同目录，未删改）

---

## 0. 起始态（原文）

```console
$ cd /home/skystar/Live2D-Ai-fe && pwd
/home/skystar/Live2D-Ai-fe
$ git rev-parse HEAD
dde6b85509aa61cd4ff16baafb19dfad18dd290b
$ git status --short
 M docs/README.md
?? AUDIT-REPO/
?? docs/DOC-MAP.md
?? docs/legacy/plans-archive-candidates-2026-10-01.txt
?? docs/plans/AUDIT-PROMPT-whole-repo-2026-09-28.md
?? docs/plans/HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md
?? docs/plans/IMPL-PROMPTS-debloat-round-2026-10-01.md
?? docs/plans/ORCHESTRATOR-PROMPT-debloat-round-2026-10-01.md
?? docs/plans/PLAN-debloat-and-closeout-2026-10-01.md
$ df -h . | tail -1
/dev/sdd        147G  104G   36G  75% /
```

`HEAD` = `dde6b855` = `v0.2.0-rc.7`（与 PLAN §1 / ORCHESTRATOR §2 一致）。

---

## Phase A · 后端无关的前端基线（`dde6b855` 未提交树，W0 不碰 flutter）

### A.1 环境前提
裸 `PATH` 下 **没有 flutter**：

```console
$ flutter analyze
bash: line 1: flutter: command not found
ANALYZE_EXIT=127
```

Flutter SDK 在 `$HOME/flutter`（`scripts/ignite.sh:21` 也是这么加的）。以下所有命令均带
`export PATH="$HOME/.cargo/bin:$HOME/flutter/bin:$PATH"`。

```console
$ flutter --version
Flutter 3.47.3 • channel stable • https://github.com/flutter/flutter.git
Framework • revision e8113bf456 (4 weeks ago) • 2026-09-04 13:20:08 -0700
Engine • hash 0e228ec8c8d2abc9fcf1d053e8a40665bb859ec7 (revision 06a2e2a110) (28 days ago) • 2026-09-03 16:07:13.000Z
Tools • Dart 3.13.3 • DevTools 2.60.0
```

### A.2 命令 + 原始输出

```console
$ cd shell/flutter && flutter analyze && flutter test
```
（全文 262,175 B：`raw/phaseA_flutter.txt`；关键行如下）

```text
Analyzing flutter...
No issues found! (ran in 3.0s)
ANALYZE_EXIT=0
...
00:28 +1378: All tests passed!
TEST_EXIT=0
```

### A.3 结论

| 项 | 声称 | 实测 | 判定 |
|---|---:|---:|---|
| `flutter analyze` | 0 issue | `No issues found!` / exit 0 | ✅ 一致 |
| `flutter test` | **1378** | **+1378 All tests passed!** / exit 0 | ✅ 一致 |

> 与 `docs/releases/v0.2.0-rc.7.md:22,128` 的自述（`+1378 All tests passed!`）一致。

---

## Phase B · W0 后端全量门禁（落在已提交树 `d1e29ba3` 上）

> **取证锚点（重要）**：Phase B 的窗口是 **2026-10-01 21:44:44 → 21:52**，
> 当时 `git rev-parse HEAD` = `d1e29ba3`，`git status --short` 只有 `?? AUDIT-REPO/` 与
> `?? docs/audit/2026-10-01-debloat/` ⇒ **工作树 = W0 提交树，无第三方改动**。
> W1-a 的 Dart 改动（`ws_client.dart` / `ws_liveness.dart`）出现在 **21:50:44 与 21:51:19**，
> 即**在 flutter 复跑结束（21:49:36）之后**，不影响本文任何 flutter 数字；
> 之后又有 `3a3b17d0`（docs-only）落盘，故 `code-stats` 的 docs 行数在 B6 与 §C.3 之间从
> 70,801 漂到 71,069（Rust/Dart 数字不变）。
> 复现本文数字请 `git checkout d1e29ba3`（或 `3a3b17d0`，仅 docs 差异）。

运行前提原文（`raw/phaseB_gates.txt` 头部）：

```console
# 时间: 2026-10-01T21:44:44+08:00
$ git rev-parse HEAD
d1e29ba32afc68a484de06cd9b94e9f1ed6ae453
$ git status --short
?? AUDIT-REPO/
?? docs/audit/2026-10-01-debloat/
$ df -h . | tail -1
/dev/sdd        147G  104G   36G  75% /
# 核查：无其它 cargo/rustc 进程
(none besides this script)
```

### B.1 逐条命令与原始输出

| # | 命令 | 原始结果 | 声称基线 | 判定 |
|---|---|---|---|---|
| B1 | `cargo test --workspace --all-targets` | **1,474 passed / 0 failed**，exit 0（26 个 target 的 `test result:` 行求和） | 1457 / 0 | ✅ **不降，且 +17** |
| B2 | `cargo test --doc --workspace` | `3 passed; 0 failed`（`live2d_ai_runtime` doc-tests 3 条），exit 0 | 3 | ✅ |
| B3 | `cargo fmt --all -- --check` | 无输出，`B3_EXIT=0` | clean | ✅ |
| B4 | `cargo clippy --workspace --all-targets -- -D warnings` | `Finished dev profile`，exit 0（`-D warnings` 下无 warning） | 0 | ✅ |
| B5 | `cargo run -p xtask -- rust-ratio` | **97.0850%**（98750 / 101715）`PASS` | **97.3263%** | ⚠ **不一致，已定位原因（见 B.3）** |
| B6 | `cargo run -p xtask -- code-stats` | Rust prod **68,867** / inline 16,326 / integ 13,542 · Dart 33,362 / 28,895 · docs 303/70,801 | — | ✅ 内部自洽（见 C.2） |
| B7 | `cargo run -p xtask -- code-stats --check` | `结论：PASS …退出码 0`（58/58 · 7/7 · 4/4 · 34/34） | — | ✅ 绿 |

B1 的求和原文（每 target 一行，共 26 行）：

```console
$ grep -oE "test result: (ok|FAILED)\. [0-9]+ passed; [0-9]+ failed" phaseB_cargo_test_full.txt \
  | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f}'
passed=1474 failed=0
$ grep -c "test result: FAILED" phaseB_cargo_test_full.txt
0
$ tail -1 phaseB_cargo_test_full.txt
CARGO_TEST_EXIT=0
```

**1,474 的对账**（这是「测试条数不降」的正面证据）：

```console
$ cargo test -p xtask    # W0b 新增的 code_stats 模块
test result: ok. 27 passed; 0 failed; ...
```
- `27` = 旧 `xtask` 10 条 + **W0b 新增 17 条**（`git show d1e29ba3` 的 commit body 也自报 10 旧 + 17 新）；
- **1,474 − 17 = 1,457** = rc.7 声称基线，**逐条对得上**；W0a 是 docs-only、W0b 只动 `xtask/**`+CI，
  没有任何业务测试被删/被改。

### B.2 gstatic 断网红线（`raw/phaseB_gstatic.txt`）

```console
$ grep -c "gstatic.com/flutter-canvaskit" shell/flutter/build/web/index.html shell/flutter/build/web/main.dart.js
shell/flutter/build/web/index.html:0
shell/flutter/build/web/main.dart.js:0
$ grep -rl "gstatic.com/flutter-canvaskit" shell/flutter/build/web/ | wc -l
2                      # flutter.js / flutter_bootstrap.js 各 1 处
$ grep -o '.\{120\}gstatic\.com/flutter-canvaskit.\{60\}' shell/flutter/build/web/flutter.js
…function _(n,e){return n.canvasKitBaseUrl?n.canvasKitBaseUrl:e.engineRevision&&!e.useLocalCanvasKit?W("https://www.gstatic.com/flutter-canvaskit",e.engineRevision):"canvaskit"}…
$ grep -o '"useLocalCanvasKit":[a-z]*' shell/flutter/build/web/flutter_bootstrap.js
"useLocalCanvasKit":true
```

| 口径 | 结果 | 判定 |
|---|---|---|
| `index.html` + `main.dart.js`（**= `ignite.sh --check` 实际检查的两个文件**） | **0 / 0** | ✅ 与声称的「0/0」一致 |
| 整个 `build/web/`（含库文件 `flutter.js`/`flutter_bootstrap.js`） | **2 个文件各 1 处** | ⚠ **不等于 0** |
| 那 2 处的性质 | 库内三元回落分支 `e.engineRevision && !e.useLocalCanvasKit ? <CDN> : "canvaskit"`，同文件 `buildConfig` 里 `"useLocalCanvasKit":true` ⇒ **死分支** | 断网红线**未破** |
| 本地 canvaskit 副本 | `canvaskit.wasm` 7,284,602 B + `canvaskit.js` 86,987 B 在位 | ✅ |
| 产物新鲜度 | `main.dart.js`（2026-09-28 22:30）**晚于**所有 `lib/**/*.dart`（`find -newer` 无输出） | ✅ |

> **口径结论**：「gstatic 0/0」只在「`index.html` + `main.dart.js`」这个定义下成立（也是 `scripts/ignite.sh:89-101`
> 的定义）；**不能读成「`build/web` 零命中」**。若下一轮有人写 `grep -r` 全目录并期望 0，会得到 2（假红）。

### B.3 `rust-ratio` 声称 97.3263% vs 实测 97.0850% —— **原因已定位：是复核者自己的证据脚本**

```console
$ wc -l docs/audit/2026-10-01-debloat/W0/raw/*.py
   46 …/check_links.py
   85 …/compare_rules.py
  172 …/reimpl_code_stats.py
  303 total
$ find . -name '*.py'（rust-ratio 统计范围内，排除 target/build/node_modules）| wc -l
16        # 其中 3 个是我的证据脚本
$ find . … -name '*.py' | grep -v docs/audit/2026-10-01-debloat | xargs wc -l | tail -1
  2661 total      # 不含我的脚本
```

`rust-ratio` 的统计扩展名含 **`py`**，而它**不排除 `docs/audit/**`**：

| 输入 | 值 | 占比 |
|---|---:|---:|
| `rs` | 98,750（含 W0b 新增的 ~1,835 行 xtask） | — |
| `py` 含复核脚本 | 2,964 ~ 2,965 | **97.0850%（实测）** |
| `py` 不含复核脚本 | ≈2,661 | **≈97.376%** |
| 声称基线 97.3263% 反推的 `py` | ≈2,712 | — |

⇒ **对账**：`rs` 因 W0b **上升**（应使占比高于 rc.7），而 `py` 因**我自己的 3 个 `.py` 证据脚本 +303 行**上升（使占比下降），
净效果 = 97.0850% < 97.3263%。**基线声称没有错**；不一致完全由「复核者在 `docs/audit/**` 里写 `.py`」解释。
（我**不会**把脚本改名成 `.txt` 去把数字做绿——那是「改文件让门禁变绿」，正是红线。）
**建议**：`rust-ratio`（与 `code-stats` 的 docs 口径）排除 `docs/audit/**`，或在报告里同时给「含/不含审计产物」两个数。

### B.4 `./scripts/ignite.sh --check` —— **未跑（前提不成立），不伪造绿灯**

```console
$ ss -ltn | grep 18080
(18080 无监听)
$ curl -s -m 3 -o /dev/null -w "%{http_code}\n" http://127.0.0.1:18080/
http_code=000            # 连接失败
$ ./scripts/ignite.sh --check
==> 已导出 .env 到进程环境（Rust 侧也直接读 .env）：OPENCODE_API_KEY DASHSCOPE_API_KEY ALIYUN_API_KEY DEEPSEEK_API_KEY
==> 点火体检：http://127.0.0.1:18080（--check 不自启服务；请先在另一终端跑 ./scripts/ignite.sh）
    [FAIL] GET / 期望 302 → /app/，实得 无响应 Location=无
    [FAIL] GET /app/ 期望 200，实得 000000
    [FAIL] GET /app/index.html 期望 200，实得 000000
    [FAIL] GET /app/main.dart.js 期望 200，实得 000000
==> 体检失败：见上面的 [FAIL]
IGNITE_CHECK_EXIT=1
```

**判定：未跑成 —— 前提缺失（18080 无监听进程）**。`--check` 按设计只体检**已在跑**的服务
（`scripts/ignite.sh:64` 明写「不自启服务」），所以这 4 个 `[FAIL]` 是**前提不成立的表现，不是门禁红**。
**不得**把这 4 行当「点火体检失败」写进结论。声称的「四项 ok」在**没有起服务**的情况下**无法复核**。
（谁起了服务告诉我，我 30 秒内重跑并把四项原文贴回来。我**没有**自行起服务——那会引入与本任务无关的进程与端口占用。）

## Phase C · W0 波 diff 复核

> **W0a 已提交**（`b9eff54e` + `4285af8b`，tag `checkpoint/pre-doc-archive` = `dde6b855`）；
> **W0b（`xtask/src/code_stats/**`）在飞，未提交**。以下 C.1 是**落在 commit 上**的版本
> （核验树用 `git archive <commit> | tar -x -C /tmp/…` 解出，**不动工作树**）；
> C.2 是**工作树在飞态**的复核（提交后会用 commit 树重跑并替换数字）。

### C.0 提交范围

```console
$ git show --name-only --format="" b9eff54e | wc -l          # 127
$ git show --name-only --format="" b9eff54e | sed 's/.*\.//' | sort | uniq -c
    127 md
$ git show --name-only --format="" b9eff54e | grep -vE '\.md$'   # → 无输出
$ git show --name-status --format="" b9eff54e | awk '{print $1}' | sort | uniq -c
     33 M
     94 R*        # R094×2 R095×4 R096×4 R097×7 R098×26 R099×51 = 94
$ git show --name-status --format="" b9eff54e | grep '^R' | awk '{print $2" -> "$3}' \
  | grep -vcE '^docs/plans/[^/]+\.md -> docs/legacy/plans/[^/]+\.md$'
0                # 94 个 rename 全部是同名「docs/plans/<x>.md → docs/legacy/plans/<x>.md」
$ git show --name-status --format="" 4285af8b
M	AGENTS.md
M	CHANGELOG.md
A	docs/DOC-MAP.md
M	docs/README.md
A	docs/legacy/plans-archive-candidates-2026-10-01.txt
A	docs/plans/AUDIT-PROMPT-whole-repo-2026-09-28.md
A	docs/plans/HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md
A	docs/plans/IMPL-PROMPTS-debloat-round-2026-10-01.md
A	docs/plans/ORCHESTRATOR-PROMPT-debloat-round-2026-10-01.md
A	docs/plans/PLAN-debloat-and-closeout-2026-10-01.md
M	docs/releases/v0.3.0.md
```

| 判据 | 结果 |
|---|---|
| `git show --name-only <W0a>` 无 `.rs/.dart/.toml/.yaml` | ✅ **两个 commit 合计 0 个**（127 `.md` + 1 `.txt`） |
| 94 rename 完整性 | ✅ 94 条 rename，目标全在 `docs/legacy/plans/`，**无一条漏名/改名** |
| 提交树计数 | `docs/plans/*.md` = **24**；`docs/legacy/plans/*.md` = **94**；banner 命中 **94/94** |
| `AGENTS.md` 单一化 | ✅ 提交树内只有 **1 份** `AGENTS.md`（858 行）；首屏第 7 行显式点明 `-Ai` 那份是**过时历史副本**；新旧 doctrine 以 `mod_count_is_three → mod_count_is_five` 的方式保留为历史陈述 |
| `CHANGELOG.md` 停用头注 | ✅ 顶部 8 行声明「自 `v0.1.0-rc.4` 停更 + 版本真源 = releases + 代码三处」 |
| `docs/releases/v0.3.0.md` 标历史草案 | ✅ 首行「**历史草案，未发布**」+ 作废说明 |

**33 个 M 文件的改动性质（机械核）**：53 行删 / 53 行增，**1:1 对齐**；逐行把两侧的
「路径串」归一化后比较，**全部是同一行的路径改写**（`docs/plans/<名>` / 裸名
→ 各文件自身深度正确的 `../legacy/plans/<名>` / `../../legacy/plans/<名>` / `plans/<名>`），
**没有一行正文语义被改**（我逐条读过 5 处初判「非路径差异」，全部是我正则未覆盖裸文件名的假警报）。

**范围提示（已在 lead 的 commit body 认领）**：`.md` 里除 `docs/**`、`AGENTS.md`、`CHANGELOG.md`
之外，`b9eff54e` 还改了 `CREDITS.md`、`SOURCES.md`、`crates/l2d/README.md`、
`crates/live2d-ai-desktop/README.md`。它们**越出 task-1 的字面文件归属**，但「全仓改链」必然波及
（DOC-MAP §4.1）。本复核按「改链优先」判**不构成红线违规**。

### C.1 W0a · 归档/改链

#### 命令与原始输出

```console
# 核验器（verifier 自写，非实施者产物）
$ bash docs/audit/2026-10-01-debloat/W0/raw/check_archive_refs.sh
checked_names=94
A_names_with_docsplans_ref=3   <- 验收判据（须 0）
B_names_with_bare_ref_live=33  <- 信息性（裸名，多为我脚本对 `…/legacy/plans/<名>` 改写结果的重复命中）
C_names_with_bare_ref_legacy=63 <- 信息性（历史层互引）
PASS_A=no
```

**归档候选清单 94 条逐条对照**（`git status` 的 `RM` 计数 = 94，`docs/legacy/plans/` 实到 94 份，
候选清单里 0 条缺失、0 条重复、0 条与保留清单（21 份）交集）：

```console
$ ls docs/legacy/plans/*.md | wc -l        # 94
$ ls docs/plans/*.md | wc -l               # 归档前 118（= 113 跟踪 + 5 未跟踪）
```

**头部行抽查（task-1 要求「每份归档文档顶部加一行历史标注」）**：

```console
$ for f in docs/legacy/plans/*.md; do head -3 "$f" | grep -q "历史（2026-10-01 归档" || echo "NO-HEADER: $f"; done
moved_files=94 without_header=0
$ head -5 docs/legacy/plans/HANDOFF-2026-09-10.md
> 历史（2026-10-01 归档，勿当现网）。
                                    ← 空行
# 交接说明（2026-09-10）：核心链路闭环 + 音频「很吵」修复
```

**`docs/README.md` 断链（逐条输出，脚本自写）**：

```console
$ python3 docs/audit/2026-10-01-debloat/W0/raw/check_links.py docs/README.md
…（逐条 OK，见 raw/inflight_links.txt）…
[docs/README.md] links=105 missing=0 external=0 anchor_only=0
TOTAL_MISSING=0
```

归档前同一条命令的基线：`links=102 missing=0`（`raw/pre_archive_links.txt`）。

**归档件内部相对链接**（移动一层目录会打断相对链接，必须逐一核）：

```console
$ python3 docs/audit/2026-10-01-debloat/W0/raw/check_links.py docs/legacy/plans/*.md
… 94 文件 / 82 条链接 / 唯一 1 条 MISSING …
TOTAL_MISSING=1
```
唯一 MISSING 经查是**本脚本的假阳性**：`docs/legacy/plans/node-d-py-asset-migration.md:62`
里的**正则字面量** `[\（(]([^\（）()]*)[）)]` 被 `[…](…)` 规则误当成 markdown 链接
（该内容归档前即存在，非 W0 引入的断链）。

#### A 形态 3 条残留 —— **判定：不构成违规（3 条都不该改）**

| # | 位置 | 命中内容 | 判定 |
|---|---|---|---|
| 1 | `docs/legacy/plans/ORCHESTRATOR-PROMPT-settings-and-key.md:16` | `/home/skystar/Live2D-Ai-l1/docs/plans/IMPL-PROMPTS-key-single-source-and-settings-trim.md` | **-l1 外仓绝对路径**的历史自注，改了就变假话 |
| 2 | `docs/audit/2026-09-28-rc7/REPO-HYGIENE.md:35` | 引述 `-Ai-product` worktree 的 `docs/plans/PRODUCT-L1-GOALS-2026-09-15.md` | **已封口审计账本**（task-1 明令禁改） |
| 3 | `docs/legacy/plans/STAGE2-WAVE2-plan-2026-09-22.md:111` | `/home/skystar/Live2D-Ai-l1/docs/plans/STAGE2-WAVE2-plan-2026-09-22.md` | 同上，**-l1 外仓绝对路径** |

⇒ **判据应按语义读作「本仓 `docs/plans/<归档名>` 相对引用 = 0」**：按此口径 **PASS（0/94）**。
字面 `grep` 非 0 的三条全部是「外仓绝对路径 / 封口账本引述」。**这条口径必须写进结论，
否则下一轮会有人拿字面 grep 判红，进而去改封口账本。**

#### 全仓 `.md` 断链「零回归」独立核（**不采信转述**：自己把基线重放了一遍）

基线树用 `git archive HEAD` 解到 `/tmp/prew0`（**只读，不动 worktree、不加 worktree**），
用同一个自写核验器跑全仓 `.md`，与工作树现态逐条 `comm` 对照：

```console
$ git archive HEAD | tar -x -C /tmp/prew0
$ cd /tmp/prew0 && python3 …/check_links.py $(find . -name '*.md') | grep MISSING … | sort > /tmp/base_v2.txt
$ cd /home/skystar/Live2D-Ai-fe && python3 …/check_links.py $(find . -name '*.md' …) | grep MISSING … | sort > /tmp/now_viol.txt
$ comm -13 /tmp/base_v2.txt /tmp/now_viol.txt   # NEW
（无）
$ comm -23 /tmp/base_v2.txt /tmp/now_viol.txt   # FIXED
docs/plans/post-launch-quality-roadmap.md
```

| 项 | HEAD 基线 | W0a 工作树 | 判定 |
|---|---:|---:|---|
| 全仓 `.md` 断链条数 | 10 | 9 | — |
| 其中脚本假阳性（正则字面量 `[\（(]([^\（）()]*)[）)]`） | 1 | 1 | 非真断链 |
| **真断链** | **9** | **8** | **NEW = 0**（零回归）✅ |
| 顺带修好的 | — | `docs/legacy/PROGRESS.md:9` | 合法：`post-launch-quality-roadmap.md` 在归档清单 `:86`，改链后指向 `plans/…` 正确 |

原始输出：`raw/all_md_links_diff.txt`　（与 docs-scribe 自报的「基线 9 → 现 8、NEW=[]」**独立复算一致**。）

#### 【落在 commit 上】三个树的断链对照 —— ⚠ **发现：归档 commit 自身不自洽（收尾 commit 才干净）**

核验树用 `git archive <rev> | tar -x -C /tmp/<dir>` 解出（**不动工作树**）：
`/tmp/prew0` = `dde6b855`（基线）· `/tmp/w0a_arch` = `b9eff54e` · `/tmp/w0a_chore` = `4285af8b`。

```console
$ for d in /tmp/prew0 /tmp/w0a_arch /tmp/w0a_chore; do
>   python3 …/check_links.py $(cd $d && find . -name '*.md') … | grep MISSING …
> done
HEAD(dde6b855)   全仓断链 10 条（含 1 条脚本假阳性）    docs/README.md: links=102 missing= 0
b9eff54e         全仓断链 32 条                        docs/README.md: links= 94 missing=18   ← ⚠
4285af8b         全仓断链  9 条                        docs/README.md: links=105 missing= 0
$ comm -13 /tmp/base_v2.txt /tmp/arch_viol.txt      # b9eff54e 相对基线 NEW
…23 条（docs/plans/PRODUCT-GRADE-CLOSEOUT.md、plans/HANDOFF-2026-09-11.md、plans/PLAN-V1.md …）
$ comm -13 /tmp/base_v2.txt /tmp/commit_viol.txt    # 4285af8b 相对基线 NEW
（无输出 = 0）
```

| 树 | 全仓断链 | 其中真断链 | NEW vs 基线 | `docs/README.md` |
|---|---:|---:|---:|---|
| `dde6b855`（基线） | 10 | 9 | — | 102 / **0 broken** |
| `b9eff54e`（归档 commit） | **32** | 31 | **+23** | 94 / **18 broken** ⚠ |
| `4285af8b`（W0a 收尾 commit = 分支 tip） | 9 | 8 | **0** | 105 / **0 broken** |

⇒ **判据在最终树（tip）上成立**（`docs/README.md` 105 条链接 0 broken；全仓 NEW = 0；还顺带修好 1 条
基线老断链）。**但归档 commit `b9eff54e` 单独看是红的**：它的 18 条 `docs/README.md` 断链与 23 条全仓
新增断链，是在**下一个 commit** `4285af8b` 才补齐的。

- **影响面**：不污染最终交付；但 **tag/回滚/`git bisect` 若停在 `b9eff54e`，会落到「索引 18 条断链」的中间态**。
  lead 打的 `checkpoint/pre-doc-archive` 是 `dde6b855`（归档**前**），所以还原点本身是干净的——
  风险只在「有人 `git checkout b9eff54e` 或按 commit 逐点 bisect」时。
- **建议**：把 `checkpoint/pre-doc-archive` 之外再补一个**归档完成点** tag（打 `4285af8b`，不要打 `b9eff54e`）；
  并在阶段报告里写明 `b9eff54e` 是中间态。（本轮纪律禁 rebase，故不改历史，只标注。）

原始输出：`raw/three_trees_links.txt`（含三树逐条断链清单）× `raw/commit_4285af8b_readme_links.txt`（105 条逐条 OK）。

#### 【落在 commit 上】DOC-MAP §3 表的独立复算

用 DOC-MAP 自己写的复算命令在 `4285af8b` 树上跑（`raw/docmap_recount_4285af8b.txt`、工作树态 `raw/docmap_recount_worktree.txt`）：

| 单元 | DOC-MAP §3 自述 | 我实测 | 判定 |
|---|---:|---:|---|
| `docs/plans` 顶层 `.md` | 24（41 含 `parallel-mods/` 17） | 24（41） | ✅ |
| `docs/legacy/plans` | 94 | 94 | ✅ |
| `docs/architecture` / `research` / `verification` / `releases` / `design` / `legal` / `examples` / `development` / `screenshots` | 32/9,532 · 23/8,937 · 18/4,420 · 14/2,375 · 8/3,302 · 3/45 · 2/465 · 2/124 · 0 | 逐项**完全一致** | ✅ |
| `docs/` 顶层 `*.md` 行数 | **2,270** | **2,316** | ❌ **+46 行（+2.0%），唯一复算不上的单元** |
| `docs` 合计（含未跟踪） | 303 / 70,444 | 303 / 70,739 | ✅ 文件数一致；行数差 = 本复核文件 `GROUNDING.md` 从 84 → 333 行的增长（303 里含我个人目录 1 份） |

> ⚠ **可复算性提示（给 D5）**：DOC-MAP 与 `code-stats` 的 `docs` 口径**把 `docs/audit/**` 算进去了**，
> 而 `docs/audit/2026-10-01-debloat/**` 是**复核者/编排者的过程产物**，会随每波增长
> （我这份 GROUNDING 一晚上就长了 249 行）。⇒ 「docs 69,733 → ≤45,000」这条线会被审计产物稀释；
> 建议在 `code-stats` 的 docs 口径里**单列/剔除 `docs/audit/**`**，或在报告里同时给「含/不含 audit」两个数。

#### `docs/plans` 归档前后计数

| 时点 | `docs/plans/*.md`（顶层） | `docs/legacy/plans/*.md` | 备注 |
|---|---:|---:|---|
| 归档前 | **118** | 0（目录不存在） | 113 跟踪 + 5 未跟踪（本轮 4 份新文档 + 1 份 09-28） |
| 归档后 | **24** | **94** | 118 − 94 = 24 ⇒ `docs/plans` 判据「≤25」达标 |

> ✅ **已收口**：`DOC-MAP.md` §3 在提交版（`4285af8b`）里已改成**可复算表**（附复算命令 +
> 「tracked 296 + 未跟踪 7 = 303」的口径说明），并明确写出「归档前基线：顶层 118（跟踪 113 + 未跟踪 5）」。
> 我按它自己的命令独立复算：除 `docs/` 顶层行数（2,270 vs 实测 2,316）外**逐格一致**（见上一小节）；
> 该单元不达 ±0 但差异仅 2.0%，成因是**测得时点早于 `docs/README.md`/`DOC-MAP.md` 的最终版本**。
> ⚠ 另有一处**同一 commit 内的时点错位**：§3 第 5 条仍写「…**仍未提交** —— 由 leader 统一 commit」，
> 而它所在的 `4285af8b` **正是**那个 commit（`AUDIT-REPO/` 除外仍是未跟踪，这句只有它成立）。
> 属文档自述与所在提交的时点不一致（P4 级），不影响归档结论。

### C.2 W0b · `code-stats` 可复算性 —— 🔴 **发现 1 个 P1 级缺陷（超出 ±2%，且现有单测无判别力）**

#### 用**真代码**复现（不改仓库任何文件：`#[path]` 把仓库那份 `measure.rs` 挂进 /tmp 的 rustc 单文件程序）

```console
$ rustc --edition 2021 -O main.rs -o cs_check && ./cs_check
[复现案例] physical_lines = 5
[复现案例] inline_test_lines 实得 = 3   （measure.rs 文档口径：无花括号条目应吃到文件末尾 = 4）
[真实文件] settings.rs physical=826 inline实得=9 （34-37 行两条无花括号 cfg(test) mod；文档口径 = 793）
[真实文件] performance/mod.rs physical=478 inline实得=8
```
源码：`raw/repro_inline_mask.rs`　原始输出：`raw/repro_inline_mask.txt`

复现案例文本：`"pub fn a() {}\n#[cfg(test)]\nmod tests_x;\nuse serde::{Deserialize, Serialize};\npub fn b() {}\n"`

#### 根因（静态复核，`xtask/src/code_stats/measure.rs`）

- 文档口径（同文件 `:26-27`）：「若该条目**没有花括号**（`#[cfg(test)] mod x;` / `use ...;`），
  则算到**文件末尾**」。
- 实现 `block_end`（`:56-71`）在无花括号时**并不返回 EOF**，而是继续扫描，直到遇到第一行
  「含 `{` 且 `depth<=0`」——`use serde::{Deserialize, Serialize};` 这类**一行内自带平衡花括号**
  的语句会立刻满足条件 ⇒ 掩码提前收尾。
- 后果：`#[cfg(test)] mod xxx_tests;`（把测试外提到独立文件）之后的**生产代码没被掩掉**，
  内联测试被低估、`rust_prod` 被高估。

#### 与 PLAN §2 基线的对照（我的独立 Python 重实现，两条规则跑同一棵树）

```console
$ python3 docs/audit/2026-10-01-debloat/W0/raw/compare_rules.py
== Rust src（不含 xtask）==
规则A 现实现   : prod=69801 inline=13395
规则B 文档口径 : prod=67188 inline=16008
PLAN §2        : prod=67381 inline=16006
→ 规则A vs PLAN: prod +2420 (+3.59%)   ← 超 ±2%
→ 规则B vs PLAN: prod -193 (-0.29%)    ← 达标
受影响文件数=7  合计低估内联=2605 行
   + 784  total=  826 A=    9 B=  793  crates/live2d-ai-runtime/src/settings.rs
   + 435  total=  592 A=   70 B=  505  crates/live2d-ai-desktop/src/web_api/mod.rs
   + 429  total=  478 A=    8 B=  437  crates/live2d-ai-runtime/src/performance/mod.rs
   + 360  total=  500 A=  107 B=  467  crates/live2d-ai-desktop/src/cli/mod.rs
   + 260  total=  344 A=   40 B=  300  crates/live2d-ai-desktop/src/web_api/models_routes/mod.rs
   + 208  total=  226 A=    4 B=  212  crates/live2d-ai-desktop/src/backend/mod.rs
   + 129  total=  194 A=    7 B=  136  crates/live2d-ai-desktop/src/audio/mod.rs
```
原始输出：`raw/compare_rules.txt`

#### 现有单测为何没拦住（判别力不足，非「无测试」）

`xtask/src/code_stats/tests.rs:112 inline_cfg_test_without_braces_runs_to_end_of_file` 的 fixture 是
`"pub fn a() {}\n#[cfg(test)]\nmod tests_x;\n"` —— 无花括号条目**恰好是文件最后一行**，
`len-1` 与「吃到文件末尾」**同解** ⇒ 该断言**恒真**。
把它换成带后续生产代码的输入即变红（实得 3 / 期望 4）。

#### ✅ 复验：W0b 已按此修好，我在**已提交树**上重跑确认

`d1e29ba3` 的 `measure.rs` 已实现文档口径（无花括号条目在遇到 `;` 之前没有 `{` ⇒ 返回 `lines.len()-1`），
并且**新测试的 fixture 就是我的复现字符串**（`tests.rs`：`…mod tests_x;\nuse serde::{Deserialize, Serialize};\npub fn b() {}\n`，断言 `inline_test_lines == 4`），
注释里直接点名「旧 fixture 恒真断言（假绿灯）」。

修复后 `cargo run -p xtask -- code-stats` 的实测（`raw/phaseB_gates.txt`）：

| 项 | PLAN §2（同分母，剔 xtask） | 修复后实测 | 偏差 | 判定 |
|---|---:|---:|---:|---|
| Rust 生产 | 67,381 | **67,188**（68,867 − xtask 1,679） | **−0.29%** | ✅ **回到 ±2% 内** |
| Rust 内联测试 | 16,006 | 16,008（16,326 − xtask 318） | +0.01% | ✅ |
| Rust 集成/示例 | 13,129 | 13,099（13,542 − xtask 443） | −0.23% | ✅ |
| `settings.rs` 内联测试 | 应为 793 | 793（表 §3.2 原文） | 0 | ✅ 我的独立复现 `9 → 793` 被实现坐实 |

> 我修复前的独立 Python 重实现预测「文档口径 = 67,188」，与修复后工具输出 **逐字相同** ⇒ 交叉验证闭合。

#### W0b 其余数字（**已对齐**）

| 项 | PLAN §2 | 我的独立复算 / 工具实测 | 偏差 | 判定 |
|---|---:|---:|---:|---|
| `xtask` prod（修复前基线 `main.rs`） | 425 | **424**（physical 604 / inline 180） | −0.24% | ✅ |
| `xtask` prod（W0b 之后） | — | **1,679**（+1,255 = 工具自身） | — | ⚠ 见 §C.3 风险 |
| Dart `lib` 行数 | 33,476 | 33,362 | −0.34% | ✅ |
| Dart `test` 行数 | 28,996 | 28,895 | −0.35% | ✅ |
| docs 行数（**含本复核目录**） | 69,733 | 70,801 | +1.53% | ⚠ 口径含 `docs/audit/**`，见 §C.3 |
| docs 份数 | 298 | 303（跟踪 296 + 未跟踪 7） | +5 | ⚠ 已由 DOC-MAP 复算表更正 |
| `crates/*/src` `.rs >500` | 58 | 58 | 0 | ✅（总行数口径） |
| `crates/*/src` `.rs >1000` | 4 | 4（1362/1086/1062/1004） | 0 | ✅ |
| 集成/示例行数（不含 xtask） | 13,129 | 13,099 | −0.23% | ✅ |
| **Rust 生产（不含内联 test）** | **67,806（含 xtask 425）** | **68,867（含 xtask 1,679）** | 含 xtask 时 +1.56%，剔 xtask 后 **−0.29%** | ✅（口径差已解释） |
| `code-stats` 报告内部自洽 | — | §2 逐 crate 三桶求和 = 68,867 / 16,326 / 13,542 **与 §1 完全相等（差 0）** | 0 | ✅ |

### C.3 W0b · commit 范围 / CI / 阈值 / 红绿双向（**全量复核**）

#### C.3.1 `git show --name-status d1e29ba3` —— 范围零越界

```console
$ git show --name-status --format="" d1e29ba3
M	.github/workflows/pr-checks.yml      (+14 / −0)
A	xtask/src/code_stats/{cli,collect,gates,measure,mod,report,tests}.rs   (共 1,813 行新增)
M	xtask/src/main.rs                    (+42 / −19)
$ git show --name-only --format="" d1e29ba3 | sed 's/.*\.//' | sort | uniq -c
      8 rs
      1 yml
$ git show --name-only --format="" d1e29ba3 | grep -vE '^(xtask/|\.github/workflows/)'
（无越界文件）      ⇒ 未碰 crates/**、shell/flutter/**、Cargo.toml、任何业务源码
$ git show --numstat --format="" d1e29ba3 | awk '{a+=$1; d+=$2} END {print "added="a" deleted="d}'
added=1869 deleted=19
```

⇒ **红线结构性未触碰**（同 §C.4）；且**零新增第三方依赖**（commit body 自报「纯 std」，`xtask/Cargo.toml` 不在改动清单里）。

#### C.3.2 CI 三条 `run:` 原文（`git show d1e29ba3:.github/workflows/pr-checks.yml`）

```yaml
      - name: Code size gate (crates/*/src .rs > 500 / Dart lib > 800)      # :77
        run: cargo run -p xtask -- code-stats --check --only lines          # :78
      - name: Code size gate (crates/*/src .rs > 1000；PLAN 目标 0、棘轮 4)   # :79
        run: cargo run -p xtask -- code-stats --check --only over-1000      # :80
      - name: Cargo dependency budget (live2d-ai-desktop ≤ 34；PLAN 目标 22)  # :81
        run: cargo run -p xtask -- code-stats --check --only deps           # :82
```
三条本地原样跑：`--only lines` / `--only over-1000` / `--only deps` 各自 `exit=0`（见 B7 与 `raw/w0b_check_extra_reds.txt` 的绿向）。

#### C.3.3 阈值「棘轮 vs PLAN 目标」—— **我的独立判定：不构成「为了让数字好看而放宽」**

```console
$ grep -n "const RATCHET_\|const PLAN_" xtask/src/code_stats/mod.rs
61:const RATCHET_SRC_RS_500: u64 = 58;      # = PLAN §2.4 现状 58
62:const RATCHET_SRC_RS_1000: u64 = 4;      # = PLAN §2.4 现状 4
63:const RATCHET_DART_800: u64 = 7;         # = PLAN §2.4 现状 7
64:const RATCHET_DESKTOP_DEPS: u64 = 34;    # = PLAN §2.5 现状 34（D4 目标 ≤22）
67:const PLAN_SRC_RS_500: u64 = 15;         # PLAN §5 目标
68:const PLAN_SRC_RS_1000: u64 = 0;
69:const PLAN_DART_800: u64 = 2;
70:const PLAN_DESKTOP_DEPS: u64 = 22;
```

**同意 lead 的裁决**，理由三条（都可复算）：
1. **棘轮 = 立档现状，不是「比现状更松」**（58/4/7/34 与 PLAN §2.4/§2.5 的现状数字逐字相同）——没有放宽任何一条已发布的门槛；
2. **PLAN 目标没有被丢掉**：`--check --strict-plan` 保留 15/0/2/22 口径，实测 **exit=1**（`dart-800` 7>2、`src-rs-1000` 4>0 并列出 4 个文件、`deps-desktop` 34>22）——目标口径**仍然红着**，没有被"包装成绿"；
3. **CI 判据是「不因存量必红」**：棘轮的存在意义是拦「新增超限」；把 CI 设成当前必红等于天天红（与「自检说谎」同类教训）。

⚠ **但必须写进台账的过程风险（我不同意「就不用管了」这一层）**：棘轮是**绝对上限**，因此
**D2/D4 每减少一个超限文件，必须在同一个 commit 把 `RATCHET_*` 收紧到新值**，
否则「降到 0 之后再涨回 1 也拦不住」。⇒ 请把这条写进 D2/D4 的验收判据。

#### C.3.4 `--check` 红绿双向（**我自己造，不写仓库任何文件**）

planner 的原判据是「临时造一个 >1000 行文件 → 必须红」。我先在 **`/tmp` 合成 fixture 树**上用**仓库编译出的真二进制**
（`target/debug/xtask`，cwd = fixture）跑，结果暴露了判据与实现的一个**语义差**：

```console
# fixture: crates/demo/src/huge.rs = 1101 行（棘轮上限是绝对数 4）
$ cd /tmp/cs_fixture && /home/skystar/Live2D-Ai-fe/target/debug/xtask code-stats --check
| `crates/*/src` `.rs` > 1000 行的文件数（上限 4） | 1 | ≤ 4 | 0 | PASS |
结论：PASS —— 判定范围内无超限；退出码 0
RED_EXIT=0                     ← ⚠ 单个新文件**不会**触发棘轮（1 ≤ 4）
```

⇒ **「造 1 个 >1000 行文件必须红」这句话只在本仓成立**（本仓 `>1000` 恰好**等于**棘轮 4，第 5 个就红），
fixture 里必须造到 **5 个**才越过棘轮。修正后的双向（`raw/w0b_check_redgreen_fixed.txt`）：

```console
# [红] fixture 里 5 个 1101 行文件
  - `src-rs-1000`：当前 5 ＞ 上限 4（PLAN 目标 0）
    - `crates/demo/src/huge1.rs`：1101 行 … huge5.rs：1101 行
- 结论：FAIL —— 判定范围有超限；退出码 1        RED5_EXIT=1     ✅
# [绿] 删掉这 5 个文件
- 结论：PASS —— 判定范围内无超限；退出码 0      GREEN5_EXIT=0   ✅
```

四条门禁**逐条**红-绿（真仓 + `--max-*` 压阈值，**不写任何文件**；`raw/w0b_check_extra_reds.txt`）：

| 门禁 | 压阈值命令 | 结果 | 默认棘轮 |
|---|---|---|---|
| `lines`（>500） | `--check --only lines --max-src-rs-500 0` | `LINES_RED_EXIT=1` ✅ | `GREEN_EXIT=0` ✅ |
| `lines`（Dart>800） | `--check --only lines --max-dart-800 0` | `DART_RED_EXIT=1` ✅ | 同上 |
| `over-1000` | `--check --only over-1000 --max-src-rs-1000 0` | `OVER1000_RED_EXIT=1` ✅ | 同上 |
| `deps` | `--check --only deps --max-deps live2d-ai-desktop=10` | `DEPS_RED_EXIT=1` ✅ | 同上 |
| 反向（放宽到远超现状） | `--max-src-rs-500 999 --max-src-rs-1000 99 --max-dart-800 99 --max-deps …=999` | `RELAXED_GREEN_EXIT=0` ✅ | — |
| `--strict-plan`（PLAN 目标口径） | `--check --strict-plan` | **`STRICT_PLAN_EXIT=1`**（按设计现在是红）✅ | — |

> **对 planner 判据的诚实修正**：我**没有**在仓库里造 `crates/*/src/huge.rs`（那需要写业务源码，超出我的只读授权，
> 且会污染 xtask-gate 的范围）。我用的是「`/tmp` 合成树 + 真二进制」与「官方 `--max-*` 注入点」两种等价手段。
> 判据的**行为**已证（红绿双向 + 判别力），判据的**字面**（在真仓里造文件）未做——写进未核实栏。

### C.4 红线清单（W0 未触碰）—— 结构性判定 + 锚点哈希**逐个复核**

W0 的改动**按扩展名**就出不了业务红线：

- W0a（`b9eff54e` + `4285af8b` + `3a3b17d0`）只有 `.md`（+ 94 个 rename + 1 个 `.txt`）
  ⇒ WS 帧 / `clean_for_tts` / 错误码 / `.env` / A1–A8 / `IdleState` / `mod_count` 断言**全部不可能被触碰**。
- W0b（`d1e29ba3`）只改 `xtask/**` + `.github/workflows/pr-checks.yml`（8 `.rs` + 1 `.yml`）
  ⇒ 同样不进入业务 crate。

**锚点哈希逐个复核（不是「推定没碰」，而是「比对过哈希」）**：

```console
# A. W0 全部 4 个 commit 碰过的「代码类」文件（rs/dart/toml/lock/yml/yaml/sh）
$ git diff --name-only dde6b855..3a3b17d0 | grep -E "\.(rs|dart|toml|lock|yml|yaml|sh)$"
.github/workflows/pr-checks.yml
xtask/src/main.rs
xtask/src/code_stats/{cli,collect,gates,measure,mod,report,tests}.rs
        ⇒ 只有 W0b 的 9 个文件，全部落在 `xtask/**` + `.github/workflows/**`
# B. crates/** 与 shell/flutter/** 里被碰过的**全部**文件（含文档）
$ git diff --name-only dde6b855..3a3b17d0 -- crates shell/flutter
crates/l2d/README.md
crates/live2d-ai-desktop/README.md
        ⇒ 只有 2 个 crate 的 README.md（改链越界，已在 C.0 认领）；**无任何 .rs/.dart/.toml**
# C. 后续 commit 是否只动文档
$ git diff --name-only d1e29ba3..3a3b17d0
docs/DOC-MAP.md
```

`raw/preW0_redline_hashes.txt`（pre-W0 快照，20 个锚点文件）里的每一份，与最终 HEAD 逐一
`git hash-object` 比对结果见 `raw/redline_hash_compare.txt`：

| 红线 | 锚点 | 结论 |
|---|---|---|
| 1 · WS 帧只增不改 | `web_api/ws/events.rs`、`ws.rs`、`ws/audio.rs`、`app_event.rs` | 哈希**全同** ⇒ 帧名集合未动 |
| 2 · `clean_for_tts` 不旁路 | `runtime/src/dialogue/clean.rs` | 哈希**全同** |
| 3 · 错误码两侧同源 | `runtime/src/conversation/error_code.rs`、`web_api/dispatch.rs` | 哈希**全同** |
| 4 · 离线优先 | `scripts/ignite.sh`、`.github/workflows/flutter-checks.yml` | 哈希**全同**（`pr-checks.yml` 只被 W0b 追加 14 行 D0 门禁） |
| 5 · 密钥 `.env` 永不回值 | `runtime/src/secrets.rs`、`web_api/env_routes.rs` | 哈希**全同** |
| 6 · A1–A8 + `IdleState` | `l2d-wasm-demo/src/web/surface/idle.rs`、`runtime/src/performance/mod.rs` | 哈希**全同** |
| 7 · `mod_count` 断言 | `live2d-ai-desktop/src/main.rs`（`mod_count_is_five`，`mod_count_is_five` 见 `:471`） | 哈希**全同** |
| 8 · 测试条数不降 | — | 见 B.1：cargo **1474 ≥ 1457**；flutter **1378 = 1378**；`xtask` 27 = 10 + 17 新增 |

> 术语更正（**-fe 树 ≠ 系统提示里那份 AGENTS.md 的 doctrine**）：
> 本树 `crates/live2d-ai-desktop/src/main.rs:471` 的断言是 **`mod_count_is_five`（=5）**，
> 且 `crates/live2d-ai-mod-director` 存在并已注册（Wave 3 最小骨架，零投递）、
> `live2d-ai-runtime/src/performance/` 存在。两份 `AGENTS.md` 行数不同
> （`-Ai` 547 行 / `-fe` 851 行）⇒ **红线 7 必须按 -fe 的 `mod_count_is_five` 判**，
> 不能按 `mod_count_is_three` 判（后者是死树 doctrine，正是 W0a 要收口的对象）。
> 这条已导致 `task-11`（W2-p1）描述里写「当前值（3 还是 4？）」——**两个选项都错**，
> 我已单独告知 lead，lead 已修正该任务描述。

### C.5 复核结论总表（声称 vs 实测，逐条）

| # | 声称 | 实测（原文见括号内文件） | 判定 |
|---|---|---|---|
| 1 | flutter test **1378** | `+1378 All tests passed!`（Phase A `dde6b855`；Phase B 在 `d1e29ba3` 重跑仍 1378） | ✅ 一致 |
| 2 | flutter analyze 0 | `No issues found!`（两次独立运行） | ✅ |
| 3 | cargo test **1457/0** | **1474/0** = 1457 + W0b 新增 17（`xtask` 27 = 10+17） | ✅ 一致（已对账，不降） |
| 4 | doc **3** | `3 passed; 0 failed` | ✅ |
| 5 | fmt clean | `B3_EXIT=0` 无输出 | ✅ |
| 6 | clippy **0** | `-D warnings` 下 exit 0 | ✅ |
| 7 | rust-ratio **97.3263%** | **97.0850%** | ⚠ **不一致**，成因 = 复核者自己的 3 个 `.py`（+303 行）被计入 `py`；剔除后 ≈97.376% ⇒ 基线声称本身没错（B.3） |
| 8 | gstatic **0/0** | `index.html` 0 / `main.dart.js` 0；但 `build/web` 内 `flutter.js`/`flutter_bootstrap.js` 各 1（死分支） | ✅ 在 `ignite.sh` 口径下一致；⚠ 全目录口径**≠0** |
| 9 | `ignite.sh --check` 四项 ok | **未跑成**（18080 无监听，4 条 `[FAIL]` 是前提缺失） | ⛔ **未核实**（不伪造绿灯） |
| 10 | `code-stats` 能复算 PLAN §2（±2%） | 剔除 xtask：prod **−0.29%**、inline +0.01%、integ −0.23%、Dart −0.34%/−0.35%、>500=58、>1000=4、deps=34 | ✅ **修完 `block_end` 后全部达标** |
| 11 | `--check` 能红 | 逐条压阈值 4/4 红；fixture 5 个超限文件 → `FAIL exit 1`；`--strict-plan` → `exit 1` | ✅ |
| 12 | 94 个归档名 grep = 0 | 字面 **3 条**（外仓 `-l1` 绝对路径 ×2 + 09-28 封口账本 ×1，**都不该改**）；按「本仓相对引用」口径 = **0** | ✅ 口径修正后一致 |
| 13 | `docs/README.md` 无断链 | 105 链接 / **missing=0**（commit 树 `4285af8b`）；三树对照 NEW=0 | ✅ |
| 14 | W0 提交无 `.rs/.dart/.toml/.yaml` | W0a 4 个 commit 合计 **0 个**（`b9eff54e` 127 `.md`；`4285af8b` 10 项含 1 `.txt`；`3a3b17d0` 1 `.md`）；W0b 8 `.rs` + 1 `.yml` **全在 `xtask/`+CI** | ✅ |
| 15 | 归档前后 `docs/plans` | **118 → 24**；`docs/legacy/plans` = 94；banner 94/94 | ✅ |

### C.6 发现索引（本轮 verifier 产出）

| ID | 级别 | 一句话 | 状态 |
|---|---|---|---|
| F-V0-1 | **P1** | `code_stats::measure::block_end` 与自身文档口径不一致，`rust_prod` 高估 **+3.59%**（7 文件 / 2,605 行），且 `tests.rs:112` 是**恒真**断言（假绿灯） | ✅ 已修（`d1e29ba3`），我复算 prod 回到 **67,188 / −0.29%** |
| F-V0-2 | P3 | 归档 commit `b9eff54e` 单独不自洽（`docs/README.md` 18 条断链、全仓 +23 条），收尾 commit 才干净 | ✅ 已标注（`3a3b17d0` 写入 DOC-MAP「不要拿它当回滚点」+ tag `checkpoint/docs-archive-done` = `4285af8b`） |
| F-V0-3 | P3/P4 | 判据「94 归档名 grep = 0」按字面不可能为 0（3 条例外） | ✅ 口径已澄清并落 commit body |
| F-V0-4 | P4 | DOC-MAP §3 计数（`docs/plans 115`、docs 298、顶层 2,270）复算不上；§3-5 与所在提交时点矛盾 | ✅ 已修（`3a3b17d0`：实测表 + 双口径 + 时点说明） |
| F-V0-5 | P4 | `docs` 口径含 `docs/audit/**`（复核/审计过程产物），会把 D5 的 45,000 目标稀释 | ✅ lead 已采纳「含/不含 audit 双口径」；实测漂移证据：同一波内 docs 70,801 → **71,069**（我的 GROUNDING + DOC-MAP 改写，5 分钟内 +268 行） |
| F-V0-6 | P4 | `rust-ratio` 把复核者的 `.py` 证据脚本计入分母（-0.24pp 假落差） | ⚠ 仅报告，未改（改文件让它变绿 = 红线） |
| F-V0-7 | P4 | 工作树曾出现临时探针文件 `shell/flutter/test/zz_probe_tmp_test.dart`（`@TestOn('browser')`，单独跑 = `No tests ran`，不影响 1378） | ✅ 已被其作者删除（最终 `git status` 干净） |
| F-V0-8 | 风险 | D0 工具**自己**加了 **1,255 行生产码**（`xtask` prod 424 → 1,679，+1,869 行新增含测试），方向与「Rust 生产 −20%」相反 | ⚠ 请在 D0/D2 报告里点名（我已在 §C.3 记录） |
| F-V0-9 | 风险 | 棘轮是**绝对上限**：D2/D4 每减一个超限文件**必须在同一 commit 收紧 `RATCHET_*`**，否则「降到 0 再涨回 1」拦不住 | ⚠ 建议写进 D2/D4 验收判据 |

---

## 未核实栏（**不许空**）

1. **`./scripts/ignite.sh --check` 四项未核**：18080 无监听进程，`--check` 按设计不自启。
   需要的前提：**有人先跑 `./scripts/ignite.sh`**（会占端口 18080 并起真服务）。
   起好后我可 30 秒内重跑并贴四项原文。**我未自行起服务**（避免引入与本任务无关的进程/端口占用）。
   受影响结论：「/ 302 → /app/」「/app/ 200」「托管产物不含 gstatic」这三条**托管层**行为**未经真实服务验证**；
   其中「产物不含 gstatic」我只在**磁盘文件**上证了（B.2），**HTTP 响应的字节**没验。
2. **`--check` 红绿「字面判据」未做**：planner 写的是「在仓库里造一个 >1000 行文件」。
   我用「`/tmp` 合成树 + 真二进制」与「官方 `--max-*` 注入点」两种等价手段做了红绿双向与判别力，
   **没有**在 `crates/*/src/` 写任何文件（只读授权 + 避免污染 xtask-gate 范围）。
   若 lead 要求在真仓做，需要一个明确授权的临时文件 + 事后删除的书面记录。
3. **`wasm32` 门禁未跑**：`cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo`
   （AGENTS 已自认「CI 暂缺、仅本地手动」）。W0 未碰 wasm 源码，故未列为必跑；**未跑**。
4. **`python3 -m pytest tests/ -q`（仓库根历史资产）未跑**：不在 task-3 清单内，W0 未碰 `tests/**`。
5. **`flutter build web` 未重建**：产物仍是 2026-09-28 22:30 的那份（`main.dart.js` 晚于所有 `lib/**/*.dart`，
   新鲜度 OK）。W0 未碰 Dart，故未重建；**「重建后 gstatic 仍 0/0」未验**。
6. **肉眼项未做**：Win 浏览器 `http://127.0.0.1:18080/app/` 的 8 项目视/听验收（需真服务 + Windows 侧），
   与本次「后端无关基线 + 文档/工具门禁」无关，**未验**。
7. **`rust-ratio` 97.3263% 的「干净复现」未做**：我算出「剔除我的 `.py` 后 ≈97.376%」，
   但**没有**把脚本临时移走来实测那条命令（那会被读成「改文件让门禁变绿」）。
   要严格核，请在**不含复核目录的干净树**（或 CI）上跑一次 `rust-ratio`。
8. **`--strict-plan` 不作为 CI 门禁**：这是 lead 的裁决（棘轮先绿），我同意；但意味着
   **PLAN §5 目标在 CI 里目前没有强制力**，只有 `code-stats` 报告会显示目标列。是否可接受，请维护者确认。
9. **`AUDIT-REPO/`（942 文件 / 未跟踪）与 `docs/audit/2026-10-01-debloat/**` 的去留未决**：
   lead 说「wave 末统一收」；我这份目录目前**未跟踪**，且会被 `code-stats` 的 docs 口径算进去（见 F-V0-5）。

---

## 附：原始输出清单（`docs/audit/2026-10-01-debloat/W0/raw/`）

| 文件 | 内容 |
|---|---|
| `phaseA_flutter.txt` | Phase A `flutter analyze && flutter test` 全文（262,175 B） |
| `phaseB_gates.txt` | Phase B 七条门禁全文（B1–B7） |
| `phaseB_cargo_test_full.txt` | `cargo test --workspace --all-targets` 全文（1,474/0 的求和来源） |
| `phaseB_flutter.txt` | W0 提交树上重跑 `flutter analyze && flutter test`（1378） |
| `phaseB_gstatic.txt` | gstatic 三口径（index/main.dart.js、全目录、上下文、`useLocalCanvasKit`、产物新鲜度） |
| `phaseB_ignite_check.txt` | `ignite.sh --check` 原文 + 前提证据（无监听） |
| `w0b_check_redgreen.txt` / `_fixed.txt` / `w0b_check_extra_reds.txt` | `--check` 红绿双向（含「单个新文件不触棘轮」的语义差 + 修正版 + 四条门禁逐条） |
| `phaseC_final_codestats.txt` | 最终 HEAD 的 `code-stats` / `--check` |
| `compare_rules.py` / `compare_rules.txt` | 两条 `#[cfg(test)]` 边界规则的独立对比复算 |
| `reimpl_code_stats.py` / `reimpl_baseline.txt` | `code-stats` 口径的 Python 独立重实现 |
| `repro_inline_mask.rs` / `.txt` | **真代码**复现 F-V0-1（`#[path]` 挂仓库 `measure.rs`，不改仓库） |
| `check_links.py` / `check_archive_refs.sh` | 自写断链核验器 / 归档名残留核验器 |
| `pre_archive_links.txt` / `pre_archive_refs.txt` / `inflight_*.txt` | 归档前与在飞态基线 |
| `three_trees_links.txt` / `all_md_links_diff.txt` / `commit_*` | 三树（基线 / 中间态 / tip）断链对照（逐条） |
| `docmap_recount_4285af8b.txt` / `docmap_recount_worktree.txt` | DOC-MAP §3 的独立复算 |
| `preW0_redline_hashes.txt` / `preW0_ws_frames.txt` / `redline_hash_compare.txt` | 红线锚点哈希与 WS 帧名快照（pre-W0 → HEAD 比对） |
| `run_phaseB.sh` / `run_check_redgreen.sh` | 本次全部取证脚本（可原样重跑） |

**本复核未修改任何源码 / 配置 / 已有文档**；唯一写入 = `docs/audit/2026-10-01-debloat/**`（新建）。
**未 commit、未 push、未跑任何破坏性 git 命令；未触碰 `/home/skystar/Live2D-Ai`。**
