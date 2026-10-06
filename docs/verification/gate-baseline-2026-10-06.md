# 冻结基线独立门禁复核（T4）

> 复核人：`gate-verifier`（独立于本轮所有改动作者）。
> 复核对象：**`main` @ `36937df8cf82a6ae79d59d8143e349d2d6320048`**（= 任务书里的"冻结基线"，
> 3 个提交叠在 `08338f3` 上：`65c42f3` → `b0b0365` → `36937df`；HEAD 提交时间 2026-10-06 20:31:04 +0800）。
> 工作树：`/home/skystar/Live2D-Ai`（唯一工作树，分支 `main`）。
> 复核时间：2026-10-06 20:32–20:41 +0800。**起始与结束时 `git status --porcelain` 均为空**（见 §4.3）。
> 本报告的一切数字都来自本次在本树实跑的命令原始输出，**没有**照抄交接文档。

## 0. 结论（先给要判的那一条）

| | |
|---|---|
| 8 条指定门禁中 | **7 条绿、1 条红** |
| 与交接 §3 一致 | 1315/0 · doc 3 · fmt clean · rust-ratio 96.1016% · code-stats 四条 PASS · flutter analyze 0 · flutter test 1583 —— **逐条对得上** |
| 与交接 §3 **不一致** | `cargo clippy --workspace --all-targets -- -D warnings`：交接写 **0 warning**，本树实测 **exit 101 / 2 个 error**，可复现（跑了 3 次，其中 2 次全量形式）。 |

**一句话**：冻结基线的门禁不是"全绿"——**clippy 在 `--all-targets` 下是红的**，红点在
`crates/live2d-ai-desktop/src/web_api/tests_mod.rs`（`65c42f3` 新增的守卫注释，正文没动到逻辑）。
除此之外，其余七条门禁的数字与交接 §3 **完全吻合**。本报告如实记为 **BLOCKED（clippy 门禁未通过）**，不粉饰成"应该没问题"。 **该 BLOCKED 已由 §7 关闭**——Lead 修 `cc68f06` 后，2026-10-06 复检
`cargo clippy --workspace --all-targets -- -D warnings` = **exit 0**。

## 0.1 验证基准声明（树上有别的写者 —— 先记状态，再声明依据）

1. **代码基线 = 提交 `36937df`**。复核全程多次读 HEAD，始终是
   `36937df8cf82a6ae79d59d8143e349d2d6320048`（**HEAD 未被移动**）。
2. **起跑前快照（20:32:17 之前）**：`git status --porcelain` **零行**、`git diff --stat` **空**
   ⇒ 我的每一条命令都跑在 **36937df 的干净树**上（这条在 §4.4 有原始输出）。
3. 跑动期间出现的其它写者（全部落在 docs/ 与 scripts/ 面，逐条见 §4.4 最终快照）：
   - **docs-closeout**：`AGENTS.md` / `CHANGELOG.md` / `README.md` / `README.zh-CN.md` / `docs/**`，
     以及 **`crates/live2d-ai-desktop/src/main.rs`（mtime 20:36:05，纯 `///` 文档注释）**；
   - **probe-robustness**：`scripts/browser_probe.mjs` + 重建 `shell/flutter/build/web` +
     新增 `docs/verification/evidence-2026-10-06-e9/`。
4. **对「测试对象」的影响判定（我回源码复核过，不转述他人说法）**：
   - `git diff --stat -- 'crates/**'` 只有 `crates/live2d-ai-desktop/src/main.rs | 6 +++++-`，
     且 `git diff` 逐行确认 **全部是 `///` 注释、0 行代码**；
   - `git diff --stat -- 'shell/flutter/lib/**'` → **空**（flutter 测试对象未被任何人动过）；
   - `xtask` 未修改；`web_api/tests_mod.rs` / `web_api/mod.rs` 未修改（= 我的 clippy 红点与
     Rust 守卫所在文件，全程只有我自己「改坏→复原」的瞬时改动）。
5. **因此本报告数字的口径**：§2 的 cargo / clippy / rust-ratio / code-stats / flutter 结果按
   「**代码语义 == 36937df**」解释，依据即第 4 条（期间受影响的只有注释与文档）。
   - 更严格的一条时间线：`cargo test --workspace --all-targets`（20:32:18）、
     `clippy`（20:32:30）、`flutter analyze+test`（20:32:32–20:33:17）都跑在 main.rs 被改**之前**
     （其 mtime = 20:36:05）⇒ 那几条数字是**逐字节的 36937df 本身**；
   - 只有 20:36 之后的 `--keep-going` clippy 与两次 Rust 守卫复跑落在注释改动之后，
     而它们与 main.rs 无关（守卫只读 `mod.rs`；clippy 红点在 `tests_mod.rs`），结论不变。
6. **失效条件**：若后续出现 `crates/**` 的**非注释**改动或 `shell/flutter/lib/**` 改动，
   §2 对应数字当场作废、必须重跑——本报告不覆盖那之后的状态。

## 1. 环境与纪律

```text
$ rustc --version; cargo --version; cargo clippy --version
rustc 1.98.1 (48a229cea 2026-09-01)
cargo 1.98.1 (797e8a9bc 2026-08-05)
clippy 0.1.98 (48a229ceae 2026-09-01)

$ cat rust-toolchain.toml   ->  (none)          # 无工具链钉版，走默认 stable = 1.98.1
$ rustup toolchain list     ->  stable(active,default) / 1.87.0 / 1.92.0 / 1.98.0
```

- 全部 `cargo` / `flutter` 命令一律 `flock /tmp/l2d-heavy.lock` **串行**执行，无并发重型命令。
- `flutter` 每个 shell 都 `export PATH="$HOME/flutter/bin:$PATH"`。
- 原始日志留在 `/tmp/t4-gates/`（会话临时文件，路径见 §6）。

## 2. 门禁逐条原始结果

| 门禁（任务书原文命令） | exit | 关键原始输出 |
|---|---:|---|
| `cargo test --workspace --all-targets` | 0 | `passed=1315 failed=0`（22 个 test target 汇总） |
| `cargo test --doc --workspace` | 0 | `test result: ok. 3 passed; 0 failed` |
| `cargo fmt --all -- --check` | 0 | （无输出 = clean） |
| `cargo clippy --workspace --all-targets -- -D warnings` | **101** | **2 × `error: doc list item without indentation` @ tests_mod.rs:276-277** |
| `cargo run -p xtask -- rust-ratio` | 0 | `Rust(rs) 占比: 96.1016%（86946 / 90473 物理行）… 结论: PASS` |
| `cargo run -p xtask -- code-stats --check` | 0 | 四条 `PASS`（见下） |
| `cd shell/flutter && flutter analyze` | 0 | `No issues found! (ran in 3.3s)` |
| `cd shell/flutter && flutter test` | 0 | `00:37 +1583: All tests passed!` |

### 2.1 cargo test --workspace --all-targets

22 条 `test result: ok.` 全部 0 failed，聚合：

```text
$ grep -E '^test result:' test-all.log | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f}'
passed=1315 failed=0
```

（逐条 `test result` 行在 `/tmp/t4-gates/test-all.log`；最大的单个 target 为 `511 passed`。）

### 2.2 cargo test --doc --workspace

```text
   Doc-tests live2d_ai_runtime
running 3 tests
test crates/live2d-ai-runtime/src/conversation/mod.rs - conversation (line 46) - compile ... ok
test crates/live2d-ai-runtime/src/lib.rs - (line 23) - compile ... ok
test crates/live2d-ai-runtime/src/dialogue/mod.rs - dialogue (line 21) ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 2.3 cargo fmt --all -- --check

exit 0，日志 0 字节（`/tmp/t4-gates/fmt.log` 为空 = 无 diff 输出）。

### 2.4 cargo clippy --workspace --all-targets -- -D warnings —— **红**

首次跑（20:32:30）与复跑（20:35 左右）**输出完全一致**，均为 exit 101：

```text
    Checking live2d-ai-desktop v0.2.1-rc.1 (/home/skystar/Live2D-Ai/crates/live2d-ai-desktop)
error: doc list item without indentation
   --> crates/live2d-ai-desktop/src/web_api/tests_mod.rs:276:5
    |
276 | ///   逐文件记 info 只会把日志淹掉，刻意不记）与 dispatch 之后那一处
    |     ^^
    |
    = help: if this is supposed to be its own paragraph, add a blank line
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#doc_lazy_continuation
    = note: `-D clippy::doc-lazy-continuation` implied by `-D warnings`
    = help: to override `-D warnings` add `#[allow(clippy::doc_lazy_continuation)]`
help: indent this line
    |
276 | ///     逐文件记 info 只会把日志淹掉，刻意不记）与 dispatch 之后那一处
    |       ++

error: doc list item without indentation
   --> crates/live2d-ai-desktop/src/web_api/tests_mod.rs:277:5
    |
277 | ///   （它已由 dispatch::log_request_outcome 记过）。
    |     ^^
    = help: indent this line
    |
277 | ///     （它已由 dispatch::log_request_outcome 记过）。
    |       ++

error: could not compile `live2d-ai-desktop` (bin "live2d-ai-desktop" test) due to 2 previous errors
```

复跑证据：`/tmp/t4-gates/clippy.log`（exit 101）与 `/tmp/t4-gates/clippy-rerun.log`（exit 101，逐字相同）。

### 2.5 cargo run -p xtask -- rust-ratio

```text
rs              259        86946
py               19         3492
js                1           35
合计              279        90473
Rust(rs) 占比: 96.1016%（86946 / 90473 物理行）
门槛        : 95.0000%
结论        : PASS — 达到或高于门槛
```

### 2.6 cargo run -p xtask -- code-stats --check

```text
| 门禁 | 当前值 | 上限（棘轮） | PLAN 目标 | 判定 |
| `crates/*/src` `.rs` > 500 行的文件数（上限 44） | 44 | ≤ 44 | 15 | PASS |
| Dart `lib` > 800 行的文件数（上限 2） | 1 | ≤ 2 | 2 | PASS |
| `crates/*/src` `.rs` > 1000 行的文件数（上限 0） | 0 | ≤ 0 | 0 | PASS |
| `live2d-ai-desktop` 顶层 `[dependencies]` 条数（上限 22） | 22 | ≤ 22 | 22 | PASS |
- 判定范围（影响退出码）：全部（lines / over-1000 / deps）
- 结论：PASS —— 判定范围内无超限；退出码 0
```

（同报告披露的两条超预算项未变：`shell/flutter/build/web` 49.6 MiB > 35 MiB；wasm `dist` 5.3 MiB > 4 MiB。
这是 E4 待裁决项，不是 `--check` 的判据。）

### 2.7 flutter analyze

```text
Analyzing flutter...
No issues found! (ran in 3.3s)
```

### 2.8 flutter test

```text
00:37 +1583: All tests passed!
```

## 3. 与交接 §3 对账表

| 门禁 | 交接 §3 声称 | 本次在 36937df 实测 | 判定 |
|---|---|---|---|
| `cargo test --workspace --all-targets` | 1315 / 0 | **1315 passed / 0 failed** | ✅ 一致 |
| `cargo test --doc` | 3 | **3 passed / 0 failed** | ✅ 一致 |
| `fmt` | clean | **exit 0，无输出** | ✅ 一致 |
| clippy `-D warnings` | **0 warning** | **exit 101，2 error** | ❌ **不一致** |
| `rust-ratio`（≥95%） | 96.1016% PASS | **96.1016% PASS** | ✅ 一致（小数位相同） |
| `code-stats --check` | 44/44 · 0/0 · 1/2 · 22/22 | **44/44 · 0/0 · 1/2 · 22/22 四条 PASS** | ✅ 一致 |
| `flutter analyze` | 0 issue | **No issues found!** | ✅ 一致 |
| `flutter test` | 1583 | **+1583 All tests passed!** | ✅ 一致 |

另：交接 §1/§9 说"期望 `git status --porcelain` = 40 项（未提交）"——**已经过时**：改动已被
`65c42f3`/`b0b0365`/`36937df` 三个提交收走，本树工作区**干净**（§4.3）。这不影响门禁数字。

## 4. 差异定位与"破坏即红"自证

### 4.1 clippy 红的定位（只报，不修）

- **红点**：`crates/live2d-ai-desktop/src/web_api/tests_mod.rs` **第 276、277 行**（`65c42f3` 新增的
  `F-0001-01` 守卫注释）。第 275 行以 `///   + 3 处…` 起了一个 markdown 列表项，276/277 是其续行，
  但缩进只有 3 空格、与列表标记 `+` 的内容列不齐 ⇒ clippy 判 `doc_lazy_continuation`。
  **纯注释缩进问题，不涉及任何生产代码语义。**
- 已确认该注释**就在 HEAD 里**（不是脏改动）：
  `git status --porcelain` 为空，且 `git show HEAD:crates/live2d-ai-desktop/src/web_api/tests_mod.rs`
  的第 276/277 行即上述内容。
- **为什么可能被漏掉**（定位用，不是辩解）：该诊断只在**测试目标**里出现 ——
  `cargo clippy -p live2d-ai-desktop -- -D warnings`（不带 `--all-targets`）**exit 0 / Finished**；
  而任务书与 AGENTS.md 规定的命令带 `--all-targets`（会编 `cfg(test)`），红。
- 全量补扫：`cargo clippy --workspace --all-targets --keep-going -- -D warnings` 输出里**只有
  `live2d-ai-desktop` 这一个 crate 的 error**（日志中 `Checking` 行仅 1 条 = 其余 crate 复用既有
  clippy 缓存，未强制重建）。**边界如实标注**：本次**没有**做 `cargo clean` 后的全新全量 clippy，
  所以"其余 crate 也干净"这一句依赖的是缓存命中，不是本次逐 crate 重检。
- 供 Lead 参考的修法（**未执行**，写面限制）：把 276/277 两行多缩进 2 空格（clippy `help` 已给出），
  或对这两行所在项加 `#[allow(clippy::doc_lazy_continuation)]`。**修完必须重跑带 `--all-targets` 的全量 clippy。**

### 4.2 破坏即红 #1 —— Dart 新门禁 `test/dart_library_guard_test.dart`

被守卫的实现：`test/setting_wiring_test.dart` 里"外观分区源码 = 库 + parts 并集"的唯一读取点。

**改坏**（`git diff` 原文）：

```diff
 String appearanceSectionSource() =>
-    readLibrarySource('lib/settings/sections/appearance_section.dart');
+    File('lib/settings/sections/appearance_section.dart').readAsStringSync();
```

**跑该条 → 红**（`flutter test test/dart_library_guard_test.dart`，exit **1**）：

```text
00:00 +1: 没有任何 test 用 File(...) 直读声明了 part 的库
00:00 +1 -1: 没有任何 test 用 File(...) 直读声明了 part 的库 [E]
  Expected: empty
    Actual: [
              'test/setting_wiring_test.dart -> lib/settings/sections/appearance_section.dart'
            ]
  这些守卫只读库文件、漏掉它的 part，扫描面是一半而断言看起来还绿。改用 support/dart_library.dart 的 readLibrarySource(...)。
  当前命中：test/setting_wiring_test.dart -> lib/settings/sections/appearance_section.dart
  package:flutter_test/src/widget_tester.dart 473:18  expect
  test/dart_library_guard_test.dart 83:5              main.<fn>
00:00 +2 -1: Some tests failed.
Failing tests:
  /home/skystar/Live2D-Ai/shell/flutter/test/dart_library_guard_test.dart: 没有任何 test 用 File(...) 直读声明了 part 的库
```

**复原**：`git checkout -- shell/flutter/test/setting_wiring_test.dart`（本任务唯一允许的写仓库命令），
随后 `git status --porcelain` 为空。

**再跑 → 绿**（exit **0**）：

```text
00:00 +1: 没有任何 test 用 File(...) 直读声明了 part 的库
00:00 +2: 没有「按路径读源码」的辅助函数 / 变量路径 + part 库字面量
00:00 +3: All tests passed!
```

### 4.3 破坏即红 #2 —— Rust 新门禁 `tests_mod::pre_dispatch_responses_go_through_respond_and_log`

**改坏**（`crates/live2d-ai-desktop/src/web_api/mod.rs`，把一条前置路由改回直发，即被守卫禁止的形状）：

```diff
@@ -549,7 +549,7 @@ pub fn run_request_loop(server: tiny_http::Server, ctx: ServerContext) {
             auth_header.as_deref(),
         ) {
-            respond_and_log(request, &method, &path, "external.chat", resp);
+            let _ = request.respond(resp);
             continue;
         }
```

**跑该条 → 红**（`cargo test -p live2d-ai-desktop --bin live2d-ai-desktop pre_dispatch_responses_go_through_respond_and_log`，exit **101**）：

```text
running 1 test
test web_api::tests_mod::pre_dispatch_responses_go_through_respond_and_log ... FAILED

---- web_api::tests_mod::pre_dispatch_responses_go_through_respond_and_log stdout ----
thread '…' panicked at crates/live2d-ai-desktop/src/web_api/tests_mod.rs:289:5:
respond_and_log 的调用形状变了：期望 7（1 定义 + 6 调用）；新增一条前置路由就把它一起改大 ——
直接 request.respond 会让响应绕过 dispatch 的分级日志（F-0001-01）。当前=6

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 510 filtered out; finished in 0.00s
error: test failed, to rerun pass `-p live2d-ai-desktop --bin live2d-ai-desktop`
```

**复原**：`git checkout -- crates/live2d-ai-desktop/src/web_api/mod.rs`；随后 `git status --porcelain` 为空。

**再跑 → 绿**（exit **0**）：

```text
running 1 test
test web_api::tests_mod::pre_dispatch_responses_go_through_respond_and_log ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 510 filtered out; finished in 0.00s
```

### 4.4 收尾：工作区必须为空（原始输出）

两条自证做完、复原之后：

```text
$ cd /home/skystar/Live2D-Ai && git status --porcelain
$ echo "[end]"
[end]
$ git show -s --format='%H' HEAD
36937df8cf82a6ae79d59d8143e349d2d6320048
```

`git status --porcelain` 零行 = **空**；HEAD 仍是复核对象 36937df。全部临时改坏均已复原。

**报告落盘时的最终快照**（此刻树上有**别的队友**的并发写入，如实贴出、逐条归因；**没有一条来自本任务**）：

```text
$ git status --porcelain
 M AGENTS.md                                  # docs-closeout 的文档收口（非本任务）
 M README.md                                  # 同上
 M README.zh-CN.md                            # 同上
?? docs/verification/evidence-2026-10-06-e9/   # probe-robustness 的 E9 取证（非本任务）
?? docs/verification/gate-baseline-2026-10-06.md  # 本任务的交付物（唯一由 T4 新增的文件）

$ git diff --stat -- shell/flutter/test/setting_wiring_test.dart crates/live2d-ai-desktop/src/web_api/mod.rs
（无输出 = T4 的两处临时改坏 100% 复原，与 HEAD 逐字节一致）
```

**读法**：任务要求「结束时 `git status` 必须为空」的**实质是「T4 的破坏即红不得留残留」**——这一条已由上面第二段
（两文件 diff 为空）证明。整树非空是因为**共享工作树**上另有队友在写（AGENTS.md / README / E9 取证目录），
与 T4 无关；T4 唯一新增的就是本报告。

## 5. 未做 / 边界 / blocked

- ~~**BLOCKED（唯一一条真红）**：clippy 门禁未通过（§2.4 / §3 / §4.1）~~ → **已于 §7 关闭**：
  Lead 在 `cc68f06` 修掉 `tests_mod.rs:275` 的列表项写法（语义零变化），复检
  `cargo clippy --workspace --all-targets -- -D warnings` = **exit 0**（详见 §7，含"命中缓存"的如实说明）。
  本报告当时**不修**它是写面纪律（T4 只写本文件），不是遗漏。
- **未做**（不在 T4 指定清单，也未声称）：`python3 -m pytest tests/ -q`、`python3 scripts/check_public_secrets.py`、
  `./scripts/ignite.sh --check`、`cargo check --target wasm32-unknown-unknown`、真浏览器探针 —— 这些属别条任务/别的
  队友写面，本次不并发抢浏览器锁与重命令锁。
- **边界**：`cargo clean` 后的全新全量 clippy 未跑（§4.1 已标注）；`flutter test` 是**全量**跑（+1583），不是只跑新门禁。
- **口径如实**：§2 的 1315 / 1583 等数字是"跑出来的"，不是"接文档抄的"；唯一不一致处已单列，没有被摊平进"全绿"。

## 7. 复检：cc68f06 之后 clippy 真绿（Lead 指派单条复验）

> Lead 已修：提交 `cc68f06` 把 `tests_mod.rs:275` 的 `+ 3 处…` 改成 `另有 3 处…`（消除 markdown 把 `+`
> 当新列表项导致的 lazy continuation；**语义零变化**，只动文档注释）。复核时 HEAD = `cc68f06`，
> 该文件工作树无改动（`git diff` 空），改动确实在提交里。

**命令与结果（原样）**

```text
$ cd /home/skystar/Live2D-Ai && flock -w 1200 /tmp/l2d-heavy.lock cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.20s
EXIT=0
```

**覆盖到哪些 crate —— 如实说：本次我的运行 `Checking` 行数 = 0，即 100% 命中缓存，没有重检任何 crate。**

- 日志全文只有上面那一行 `Finished … 0.20s`；`grep -c 'Checking'` = **0**，`grep -cE '^(warning|error)'` = **0**。
- 真正重检 `live2d-ai-desktop` + `xtask` 的是 **Lead 的增量运行**（Lead 报告 1.55s 完成，在 cc68f06 之后）。

**为什么"命中缓存"在这里仍等于"真绿"（推理链，不是猜测）**

1. cargo 的 check/clippy 指纹把**源码内容**计入；被测源（`tests_mod.rs`、`main.rs`、`xtask/src/code_stats/mod.rs`）
   在当前树上已变更，若对应的检查没做过，cargo 不会短路。
2. **未通过的编译不入缓存**——这也正是修复前 `cargo clippy` 每次都 exit 101 且**每次都重检 desktop** 的原因。
3. 于是：我的命令能以 0.20s 短路 ⇒ 上一次对 `live2d-ai-desktop`（`--all-targets`，含 `cfg(test)`）的
   clippy 检查**成功**，且指纹与当前源码一致 ⇒ **红色已消除**。

**边界（不夸大）**

- 本次**没有**做"强制重建后的逐 crate 重检"；要那种强度需 `cargo clean -p <crate>` 或等价强制手段逐个重检。
  Lead 明确只要这一条命令，故未做（需要的话可另派）。
- 其余 crate 的干净结论仍来自既有缓存；但它们自上次成功检查以来**源码未变**（§0.1 的 diff 证据），指纹有效。

**结论：clippy 门禁在 cc68f06 上 PASS（exit 0）。§0 与 §5 的 BLOCKED 项由此关闭。**

## 6. 原始日志（会话内留档）

| 文件 | 内容 |
|---|---|
| `/tmp/t4-gates/test-all.log` | cargo test 全量（104 KB） |
| `/tmp/t4-gates/test-doc.log` | doc tests |
| `/tmp/t4-gates/fmt.log` | fmt（空 = clean） |
| `/tmp/t4-gates/clippy.log` · `clippy-rerun.log` | **clippy 红 ×2（逐字相同）** |
| `/tmp/t4-gates/clippy-no-all-targets.log` | 诊断：不带 `--all-targets` 时 exit 0 |
| `/tmp/t4-gates/clippy-keepgoing.log` | `--keep-going` 全量：仅 desktop 一个 crate 报错 |
| `/tmp/t4-gates/clippy-after-fix.log` · `clippy-after-fix.exit` | **cc68f06 复检：exit 0**（0.20s、0 条 Checking = 全缓存命中，见 §7） |
| `/tmp/t4-gates/rust-ratio.log` · `code-stats.log` | xtask 两条 |
| `/tmp/t4-gates/flutter-analyze.log` · `flutter-test.log` | flutter 两条（+1583） |
| `/tmp/t4-gates/red-dart.log` · `green-dart.log` | Dart 守卫红/绿 |
| `/tmp/t4-gates/red-rust.log` · `green-rust.log` | Rust 守卫红/绿 |

> `/tmp` 是会话临时目录；若需长期留证，请 Lead 决定是否转存到 `docs/verification/evidence-2026-10-06/`（不在本任务写面内，未动）。
