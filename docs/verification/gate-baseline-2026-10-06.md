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


---

## 8. T5 终局独立复验（所有写者停止后的最终树）

> **本次验的是：提交 `43e465eb34ccc2fdd21a6749f78c20f723562077` + 工作树 0 改动。**
> （Lead 发话「所有写者已停止、工作树已冻结」后开跑；下文的起跑前快照即证据。）
> 与 §1–§7 的 T4 记录**并存不覆盖**：§1–§7 验的是 `36937df` / `cc68f06`，本节验 `43e465e`。全程
> `flock /tmp/l2d-heavy.lock` 串行，原始日志在 `/tmp/t5-gates/`。

### 8.1 起跑前快照（冻结声明）

```text
$ git rev-parse HEAD
43e465eb34ccc2fdd21a6749f78c20f723562077

$ git status --porcelain
（零行）
$ git status --porcelain | wc -l
0

$ git diff --stat
（空）

$ grep -n 'RATCHET_DART_800' xtask/src/code_stats/mod.rs
144:const RATCHET_DART_800: u64 = 1;

$ stat -c '%y %n' shell/flutter/build/web/main.dart.js
2026-10-06 20:36:38.677891492 +0800 shell/flutter/build/web/main.dart.js   # probe-robustness 重建产物（本轮）
```

### 8.2 门禁逐条（11 条，全部 exit 0）

| 门禁 | exit | 原始结果 |
|---|---:|---|
| `cargo fmt --all -- --check` | 0 | 日志 0 字节（无 diff） |
| `cargo test --workspace --all-targets` | 0 | **1315 passed / 0 failed**（22 个 test target 汇总） |
| `cargo test --doc --workspace` | 0 | **3 passed / 0 failed** |
| `cargo clippy --workspace --all-targets -- -D warnings`（强制重检，见 8.3） | 0 | **真重检 live2d-ai-desktop + xtask**，3.65s 完成，0 warning |
| `cargo run -p xtask -- rust-ratio` | 0 | **96.1024%（86964 / 90491）PASS**（门槛 95%） |
| `cargo run -p xtask -- code-stats --check` | 0 | 四条 PASS；**Dart 行 = `1 | ≤ 1 | 2 | PASS`**（棘轮已收紧，见 8.4） |
| `cd shell/flutter && flutter analyze` | 0 | `No issues found! (ran in 3.6s)` |
| `cd shell/flutter && flutter test` | 0 | **`00:38 +1583: All tests passed!`** |
| `python3 -m pytest tests/ -q` | 0 | `22 passed, 1 skipped in 0.11s` |
| `python3 scripts/check_public_secrets.py` | 0 | `repo secret-pattern scan: ok (2137 files scanned)` |

原始片段：

```text
$ grep -E '^test result:' test-all.log | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f}'
passed=1315 failed=0

$ tail -1 flutter-test.log
00:38 +1583: All tests passed!

$ grep -E '结论|Rust\(rs\)|门槛' rust-ratio.log
Rust(rs) 占比: 96.1024%（86964 / 90491 物理行）
门槛        : 95.0000%
结论        : PASS — 达到或高于门槛

$ code-stats 硬门禁四行
| crates/*/src .rs > 500 行的文件数（上限 44）        | 44 | ≤ 44 | 15 | PASS |
| Dart lib > 800 行的文件数（上限 1）                |  1 | ≤  1 |  2 | PASS |
| crates/*/src .rs > 1000 行的文件数（上限 0）       |  0 | ≤  0 |  0 | PASS |
| live2d-ai-desktop 顶层 [dependencies] 条数（上限 22）| 22 | ≤ 22 | 22 | PASS |
- 结论：PASS —— 判定范围内无超限；退出码 0
```

### 8.3 clippy 强制重检（**不依赖缓存**，Lead 指派的必查项 ③）

**做法**：先 `cargo clean -p live2d-ai-desktop`（清掉该包的产物），再跑全量 clippy；判据是日志里
**必须出现 `Checking live2d-ai-desktop`**。

```text
$ cargo clean -p live2d-ai-desktop
     Removed 5564 files, 9.9GiB total

$ cargo clippy --workspace --all-targets -- -D warnings
    Checking live2d-ai-desktop v0.2.1-rc.1 (/home/skystar/Live2D-Ai/crates/live2d-ai-desktop)
    Checking xtask v0.2.1-rc.1 (/home/skystar/Live2D-Ai/xtask)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.65s
EXIT=0
```

**读法**：`Checking live2d-ai-desktop` 出现 = 该 crate（含 `cfg(test)` 的 `--all-targets`）**真的被重新检查过**，
不是 §7 那种 0.20s 短路；`Checking xtask` 同理（xtask 源码也变过）。两条都 0 warning ⇒
**修复在非纯缓存路径下成立**。（`cargo clean -p` 只动 `target/`，不碰工作树、不碰 `.git`。）

### 8.4 棘轮收紧的核对（Lead 指派的必查项 ②）

`f5210f3` 把 `RATCHET_DART_800` 由 2 收到 1（`grep` 实测 `= 1`）。code-stats 输出：

```text
| Dart `lib` > 800 行的文件数（上限 1） | 1 | ≤ 1 | 2 | PASS |
```

显示的是 **上限 1**（不是 2）⇒ **我跑的就是最终树**，Lead 提的「若仍显示 2 说明跑错树」的告警未触发。
当前唯一超 800 行的 Dart lib 文件 = `display_prefs.dart`（= 待裁决的 E2）。

### 8.5 前端产物新鲜度（必查项 ④）

`shell/flutter/build/web/main.dart.js` mtime = **2026-10-06 20:36:38**（probe-robustness 本轮重建）。
本次 T5 **没有重建产物**（只跑了 `flutter analyze` / `flutter test`，两者都不写 `build/web`），
故产物新鲜度沿用其 mtime 判定，未另行核对内容。

### 8.6 与 T4（`36937df`）的差异——逐条

| 项 | T4 @36937df | T5 @43e465e | 说明 |
|---|---|---|---|
| cargo test | 1315 / 0 | **1315 / 0** | 相同（本轮新增的是文档/探针，无新 Rust 测试） |
| doc | 3 | **3** | 相同 |
| clippy | **红（2 error）** | **绿（exit 0，强制重检）** | 由 `cc68f06` 修掉 |
| rust-ratio | 96.1016%（86946/90473） | **96.1024%（86964/90491）** | 分母/分子小幅上涨（探针脚本 + 文档轮次带来的 `.rs`/注释），仍 PASS |
| code-stats Dart 棘轮 | 1/2 | **1/1** | `f5210f3` 同 commit 收紧常量（欠账已补） |
| flutter test | 1583 | **1583** | 相同 |
| pytest / 密钥扫描 | 未跑 | **22 passed/1 skipped · 2137 files ok** | T5 补齐 |

### 8.7 冻结性的一个如实修正（跑动期间树又被写了）

起跑前 0 改动**成立**；但在我的运行窗口内，Lead 又落了两个**非代码**改动：

```text
2026-10-06 21:22:58  AGENTS.md                                        （+50 行，变更历史本轮段落）
2026-10-06 21:23:20  docs/plans/HANDOFF-2026-10-06-team-round.md      （新建）
（我的窗口：21:22:10 – 21:23:22）
```

**影响判定（我自己核的，不转述）**：

- `git status --porcelain -- 'crates/**' 'shell/**' 'xtask/**' 'scripts/**' 'assets/**'` = **空**
  ⇒ **测试对象一个字节都没变**；
- 有没有测试**读 AGENTS.md 内容**？我全仓 grep（`shell/flutter/test` / `crates` / `tests`）：
  命中 **12 处，全部是注释/文档注释里的文字引用**（如「AGENTS.md 行数纪律」），
  **没有任何测试把它当输入文件读**；`docs/plans` 的 1 处命中同样是注释 ⇒ **无竞态**。
- 故 §8.2 的数字仍按「**代码语义 == 43e465e**」成立；这条差异只影响"整树字节冻结"的说法，不影响门禁结论。

### 8.8 结论

**11/11 门禁全绿（exit 0）**，其中 clippy 是在 `cargo clean -p live2d-ai-desktop` 之后、
日志可见 `Checking live2d-ai-desktop` 的**真重检**下通过。§0/§5 的 BLOCKED 在最终树上确认关闭。
未做：真浏览器探针（`scripts/browser_probe.mjs`，属 T2 面且需浏览器锁，T5 清单未要求）、
`ignite.sh --check` / wasm check（同上，未在此清单内）。

---

## 9. V1 第二轮终局独立复验（E2 / E12 / E4 / E13 最终树）

> **本次验的是：提交 `834a4c67883c42f34c1b2a7b7e31178e8f87c5d8` + 起跑前工作树 0 改动。**
> 与 §1–§8 并存不覆盖（§1–§7 = T4 @`36937df`/`cc68f06`；§8 = T5 @`43e465e`）。
> 全程 `flock /tmp/l2d-heavy.lock` 串行；原始日志在 `/tmp/v1-gates/`；**未起浏览器**。

### 9.1 起跑前快照（冻结声明）

```text
$ git log --oneline -3
834a4c6 feat(scripts): E4 前端产物真减（−20.18 MiB）+ E13 预算变真门禁 + CI 接线
83c89e1 fix(xtask): Dart 棘轮 1 → 0 + wasm dist 预算按实测重定为 4.2 MiB + 报告口径不再自称 PLAN
2790a2c refactor(flutter): E2 display_prefs 1169 → 583 行（5 个同库 part）+ E12 24 键落盘守卫

$ git rev-parse HEAD         -> 834a4c67883c42f34c1b2a7b7e31178e8f87c5d8
$ git status --porcelain     -> 零行（wc -l = 0）
$ git diff --stat            -> 空
$ grep RATCHET_DART_800 xtask/src/code_stats/mod.rs -> 146:const RATCHET_DART_800: u64 = 0;
```

### 9.2 全套门禁（11 条，全部 exit 0）

| 门禁 | exit | 原始结果 |
|---|---:|---|
| `cargo fmt --all -- --check` | 0 | 日志 0 字节 |
| `cargo test --workspace --all-targets` | 0 | **1315 passed / 0 failed** |
| `cargo test --doc --workspace` | 0 | **3 passed** |
| `cargo clippy --workspace --all-targets -- -D warnings`（**强制重检**） | 0 | `Checking live2d-ai-desktop` + `Checking xtask`，3.66s，0 warning |
| `cargo run -p xtask -- rust-ratio` | 0 | **96.1064%（87057 / 90584）PASS** |
| `cargo run -p xtask -- code-stats --check` | 0 | 四条 PASS（见 9.3） |
| `cd shell/flutter && flutter analyze` | 0 | `No issues found! (ran in 2.2s)` |
| `cd shell/flutter && flutter test` | 0 | **`00:41 +1613: All tests passed!`** |
| `python3 -m pytest tests/ -q` | 0 | `22 passed, 1 skipped in 0.10s` |
| `python3 scripts/check_public_secrets.py` | 0 | `repo secret-pattern scan: ok (2146 files scanned)` |

**强制重检证据**（先清包再 clippy，不是缓存短路）：

```text
$ cargo clean -p live2d-ai-desktop -p xtask
     Removed 3033 files, 1.4GiB total
$ cargo clippy --workspace --all-targets -- -D warnings
    Checking live2d-ai-desktop v0.2.1-rc.1 (/home/skystar/Live2D-Ai/crates/live2d-ai-desktop)
    Checking xtask v0.2.1-rc.1 (/home/skystar/Live2D-Ai/xtask)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.66s
EXIT=0
```

### 9.3 本轮六条具体判据（逐条独立核）

**① Dart 棘轮 == 0，code-stats 行显示 `0 ≤ 0`** ✅

```text
$ grep -n 'RATCHET_DART_800' xtask/src/code_stats/mod.rs
146:const RATCHET_DART_800: u64 = 0;
| Dart `lib` > 800 行的文件数（上限 0） | 0 | ≤ 0 | 2 | PASS |
（章节：#### 3.3 Dart `lib` > 800 行（0 个，PLAN 目标 ≤2））
```

**② build/web ≤35 MiB 且 `prune_web_artifacts.sh --check` exit 0** ✅

```text
| `shell/flutter/build/web` | 29.4 MiB（30804686 B） | ≤ 35 MiB | 在预算内 |
$ ./scripts/prune_web_artifacts.sh --check
    [ok]   死重清单 0 项（*.symbols / skwasm* / wimp*）
    [ok]   红线文件齐全（9 项，含 chromium/ 与 base canvaskit）
    [ok]   目录体积 30804686 B = 29.38 MiB ≤ 35 MiB（54 个文件）
==> 通过
EXIT=0
```

**③ 反向自证（我自己的红绿）：注入 1 MiB `*.symbols` → 红 → 删 → 绿** ✅

```text
# 注入（放在 脚本实际扫描的 canvaskit/ 下）
$ head -c 1048576 /dev/zero > shell/flutter/build/web/canvaskit/v1-proof.symbols
$ ./scripts/prune_web_artifacts.sh --check
    [FAIL] 死重残留（应被清减）：
           canvaskit/v1-proof.symbols (1048576 B)
           [ok]   目录体积 31853262 B = 30.38 MiB ≤ 35 MiB（55 个文件）
==> 失败：见上面的 [FAIL]
EXIT=1
# 删除（先 readlink -f 核对绝对路径，再 rm -f 该路径）
$ rm -f -- /home/skystar/Live2D-Ai/shell/flutter/build/web/canvaskit/v1-proof.symbols
$ ./scripts/prune_web_artifacts.sh --check
    [ok]   死重清单 0 项（*.symbols / skwasm* / wimp*）
==> 通过
EXIT=0
```

**④ `display_prefs` 行数与 `flutter test` 计数** ✅

```text
583 shell/flutter/lib/settings/display_prefs.dart          （库，<800）
225 display_prefs_codec.dart / 225 display_prefs_playlist.dart / 149 display_prefs_limits.dart
 91 display_prefs_derived.dart /  71 display_prefs_copy.dart（5 个 part 全部 <800）
合计 1344 行 —— `flutter test` = +1613 ≥ 1583 + 30 ✅（E12 新文件 30 条断言）
```

**⑤ E12 守卫反向自证：删 toJson 一个键 → 红 → `git checkout --` 复原 → 绿** ✅

改坏（`shell/flutter/lib/settings/display_prefs_codec.dart`）：

```diff
     'tier': tier,
-    'edgeStrength': edgeStrength,
   };
```

红（`flutter test test/display_prefs_persist_keys_test.dart`，**exit 1 / +28 -2**）：

```text
  Expected: true
    Actual: <false>
  键 edgeStrength 没有出现在 toJson() 里——这个字段改了不会落盘
  test/display_prefs_persist_keys_test.dart 208:9
00:00 +28 -2: Some tests failed.
Failing tests:
  …: 落盘 24 键全集（E12） toJson 的键集合 == 手写全集（少一个 / 多一个 / 改名都红）
  …: 落盘 24 键全集（E12） 键 edgeStrength 落到 JSON 且能原样读回（其余 23 键不变）
```

复原：`git checkout -- shell/flutter/lib/settings/display_prefs_codec.dart`（本任务唯一一次写仓库文件），
即刻 `git diff --stat -- <该文件>` = **空**。
绿（重跑，**exit 0 / +30**）：`00:00 +30: All tests passed!`

**⑥ CI 接线：YAML 可解析 + 步骤顺序 = 构建 → prune → --check → ignite --check-dir** ✅

```text
$ python3 -c "import yaml; d=yaml.safe_load(open('.github/workflows/flutter-checks.yml')); print(list(d['jobs']))"
['flutter-analyze-test', 'flutter-web-offline-artifacts']

job: flutter-web-offline-artifacts 的步骤顺序（yaml 解析后逐条打印）：
0 Checkout code
1 Setup Flutter (stable) + cache
2 Resolve dependencies                 -> flutter pub get
3 Build Flutter web (offline: no web resources CDN) -> flutter build web --release --base-href /app/ --no-web-resources-cdn
4 Prune dead engine variants (symbols / skwasm / wimp) -> ./scripts/prune_web_artifacts.sh
5 Artifact budget gate (build/web <= 35 MiB, red lines intact) -> ./scripts/prune_web_artifacts.sh --check
6 Offline artifact gate (no Google CDN CanvasKit) -> ./scripts/ignite.sh --check-dir shell/flutter/build/web
```

顺序与 AGENTS.md / 脚本头注声明的「构建 → 清减 → --check → 离线体检验最终树」**一致**。

### 9.4 我自己额外抓到的两个问题（不在任务清单里，但属"守卫静默失效"类）

**发现 A（真问题）：`prune_web_artifacts.sh` 的"预算判据漂移"检查现在是 `[warn]`、永远不会 FAIL。**

第 4 条判据本意是「本脚本的 35 MiB 与 `xtask` 的 `FLUTTER_WEB_BUDGET_MIB` 不一致就判红」，
但 `83c89e1` 把该常量从 `u64` 改成了新的 `Mib` 单位类型，而脚本的正则仍是旧的 `u64` 形状：

```text
xtask/src/code_stats/mod.rs:180:  const FLUTTER_WEB_BUDGET_MIB: Mib = Mib(35.0);
scripts/prune_web_artifacts.sh:130:  sed -n 's/.*FLUTTER_WEB_BUDGET_MIB:[[:space:]]*u64[[:space:]]*=[[:space:]]*\([0-9][0-9]*\).*/\1/p'

$ 用脚本原样的 sed 跑一遍现在的 mod.rs  ->  无输出（零命中）
$ ./scripts/prune_web_artifacts.sh --check
    [warn] 读不到 …/xtask/src/code_stats/mod.rs 的 FLUTTER_WEB_BUDGET_MIB（不影响本门禁）
```

⇒ 今天两处**恰好都是 35**，但**没有任何东西在守它**；把 xtask 侧改成 40 或脚本侧改成 30，
`--check` 仍然 `==> 通过`。**这就是"守卫静默失效"**（与本轮 T4 抓的 clippy 假绿同类）。
**修法建议**（未执行，写面限制）：把 `xtask_budget_mib` 改成匹配 `Mib(\([0-9.]*\))`（或直接 grep 整行再解析小数），
并加一条自证；CI 里那一行注释仍写着「与 xtask 的 FLUTTER_WEB_BUDGET_MIB 漂移比对」——**名不副实**。

**发现 B（作用域边界，如实登记）：死重扫描面是 `build/web/canvaskit/**`，不是整个 `build/web/`。**

`kill_list()` 的两个 `find` 都以 `$DIR/canvaskit` 为根。实测：

```text
$ head -c 1024 /dev/zero > shell/flutter/build/web/v1-proof-root.symbols   # 放在 build/web 根
$ ./scripts/prune_web_artifacts.sh --check
    [ok]   死重清单 0 项（*.symbols / skwasm* / wimp*）                     # 没抓到
EXIT=0
（该探针文件随后已删除，最终 --check 仍 EXIT=0）
```

这**不是 bug**（Flutter 引擎只会把 `*.symbols` 拷进 `canvaskit/`，脚本头注也写明了清单来源），
但它解释了任务书「往 build/web 放一个 `*.symbols`」这句话的**歧义**：放在根目录**不会**变红，
必须放进 `canvaskit/` 才是判据射程。我两条都跑了，红绿以 `canvaskit/` 那条为准。

### 9.5 与 §8（T5 @43e465e）的逐条对账

| 项 | T5 §8 @43e465e | V1 §9 @834a4c6 | 变化原因 |
|---|---|---|---|
| cargo test | 1315 / 0 | **1315 / 0** | 无新 Rust 测试 |
| doc | 3 | **3** | 同 |
| clippy | 绿（强制重检） | **绿（强制重检，clean 两个包）** | 同（本轮 clean 面更大） |
| rust-ratio | 96.1024%（86964/90591 口径 90491） | **96.1064%（87057/90584）** | 新增/搬迁 Dart 不影响；Rust 侧行数与分母变化 |
| code-stats Dart 棘轮 | **1 / ≤1** | **0 / ≤0** | `83c89e1` 再收紧一格 |
| build/web | 49.6 MiB（超预算）→ §8 时未清减 | **29.4 MiB 在预算内** | `834a4c6` E4 真减 |
| flutter test | 1583 | **1613** | `2790a2c` E12 新增 30 条 |
| pytest | 22 passed / 1 skipped | **22 passed / 1 skipped** | 同 |
| 密钥扫描 | 2137 files ok | **2146 files ok** | 新增文件（脚本/文档/测试） |
| wasm dist 预算 | — | **5.3 MiB > 4.2 MiB「超预算」**（如实打印） | `83c89e1` 重定接受值；**未在本轮真减** |

### 9.6 冻结性的如实说明（同 §8.7 的情形再次出现）

起跑前 0 改动成立；运行窗口内/后 Lead 又落了**非代码**改动：

```text
2026-10-06 22:02:06  AGENTS.md                                  （+本轮变更历史）
2026-10-06 22:02:25  docs/plans/HANDOFF-2026-10-06-team-round-2.md（新建）
2026-10-06 22:03:01  docs/plans/NEXT-ROUND-main-2026-10-06.md    （改动）
（我的窗口：22:01:35 – 22:02:50，另加两次反向自证到 22:05 左右）
```

`git status --porcelain -- 'crates/**' 'shell/**' 'xtask/**' 'scripts/**' 'assets/**' 'tests/**'` = **空**
⇒ 测试对象一字未变；§8.7 已实证**没有任何测试把 AGENTS.md 当输入文件读**（12 处命中全是注释），
故 §9 的数字按「**代码语义 == 834a4c6**」成立。**我自己的写（`display_prefs_codec.dart`）已 100% 复原**：
`git diff --stat -- shell/flutter/lib/settings/display_prefs_codec.dart` = 空。

### 9.7 结论与未做

**结论：11/11 门禁全绿；六条具体判据全部成立（①②④⑥ 直接核 + ③⑤ 我自己的红绿自证）；
另独立抓到两个"守卫静默失效/作用域"级问题（发现 A 需修，发现 B 属边界登记）。**

未做（如实标注，均不在 task-9 清单内）：

- 真浏览器验证 / `browser_probe.mjs` —— 任务明令不起浏览器（已由 artifact-trim 做过，属它写面）；
- `./scripts/ignite.sh --check-dir shell/flutter/build/web` —— 未跑（属产物托管面；本任务只要求核 CI 步骤顺序）；
- wasm `dist` 真减 / `wasm-opt` —— 本机 absent，本轮未做（`83c89e1` 已如实登记为"超预算"）；
- E7 CI 真 runner 首跑、`git push` —— 无 token / 无 runner，**仍 blocked**（不属本任务）。

### 9.8 V2 复核：prune 预算漂移判据修复（`5e8193e`）

> Lead 的修复 = 提交 `5e8193e`（`scripts/prune_web_artifacts.sh`）：`xtask_budget_mib` **同时认**
> `Mib(35.0)` 与旧 `u64 = 35` 两种形状；**解析不到判 FAIL**（不再 `[warn]`）；比对改 **awk 数值比较**
> （避免 `35.0` vs `35` 字符串比较假红）。**本节验的是：`5e8193e` + 起跑前 `scripts`/`xtask` 无改动**
> （当时 `git status --porcelain -- scripts xtask` = 空）。**只跑脚本，未起浏览器**；日志 `/tmp/v1-gates/`。

**修复后的解析器（源码，`scripts/prune_web_artifacts.sh:128-138`）**

```text
sed -n \
  -e 's/.*FLUTTER_WEB_BUDGET_MIB[^=]*=[[:space:]]*Mib([[:space:]]*\([0-9][0-9.]*\)[[:space:]]*).*/\1/p' \
  -e 's/.*FLUTTER_WEB_BUDGET_MIB[^=]*=[[:space:]]*\([0-9][0-9.]*\)[[:space:]]*;.*/\1/p' \
  "$XTASK_MOD" | head -1
调用方（:200-211）：XB 为空 ⇒ [FAIL] 解析不到…⇒ FAIL=1；否则 awk -v a -v b 'BEGIN{exit !(a==b)}' 数值比对。
```

#### ① 未篡改 → exit 0 ✅

```text
$ ./scripts/prune_web_artifacts.sh --check
    [ok]   死重清单 0 项（*.symbols / skwasm* / wimp*）
    [ok]   红线文件齐全（9 项，含 chromium/ 与 base canvaskit）
    [ok]   目录体积 30804686 B = 29.38 MiB ≤ 35 MiB（54 个文件）
    [ok]   预算与 xtask FLUTTER_WEB_BUDGET_MIB 一致（35.0 MiB）      <-- V1 时这里是 [warn]
==> 通过
EXIT=0
```

**这一行就是"修复生效"的直接证据**：V1 时同一位置是 `[warn] 读不到 …`，现在真的读到了 `35.0`；
且脚本的 `BUDGET_MIB=35`（整数）与 xtask 的 `35.0` **数值相等即通过** ⇒ **不存在 35.0 vs 35 的假红**。

#### ② 反向自证（我自己的红绿）：xtask 常量改 `Mib(40.0)` → 红 → 复原 → 绿 ✅

```diff
-const FLUTTER_WEB_BUDGET_MIB: Mib = Mib(35.0);
+const FLUTTER_WEB_BUDGET_MIB: Mib = Mib(40.0);
```

```text
$ ./scripts/prune_web_artifacts.sh --check
    [ok]   死重清单 0 项（*.symbols / skwasm* / wimp*）
    [ok]   红线文件齐全（9 项，含 chromium/ 与 base canvaskit）
    [ok]   目录体积 30804686 B = 29.38 MiB ≤ 35 MiB（54 个文件）
    [FAIL] 预算判据漂移：xtask=40.0 MiB vs 本脚本=35 MiB
           两处必须同值（改预算要同时改 xtask/src/code_stats/mod.rs 与本脚本并写明理由）
==> 失败：见上面的 [FAIL]
EXIT=1

$ git checkout -- xtask/src/code_stats/mod.rs
$ git diff --stat -- xtask/src/code_stats/mod.rs    -> 空
$ git status --porcelain -- scripts xtask           -> 零行
$ ./scripts/prune_web_artifacts.sh --check | tail -1
==> 通过
EXIT=0
```

#### ③ 解析失败路径：常量改成解析不到的形状 → **必须 FAIL，不得再 `[warn]`** ✅

改坏（模拟"改名 → 解析器跟不上了"这一真实漂移场景）：

```diff
-const FLUTTER_WEB_BUDGET_MIB: Mib = Mib(35.0);
+const FLUTTER_WEB_BUDGET_MIB: Mib = Mib(FLUTTER_WEB_BUDGET_DEFAULT);
```

```text
$ ./scripts/prune_web_artifacts.sh --check
    [ok]   死重清单 0 项（*.symbols / skwasm* / wimp*）
    [ok]   红线文件齐全（9 项，含 chromium/ 与 base canvaskit）
    [ok]   目录体积 30804686 B = 29.38 MiB ≤ 35 MiB（54 个文件）
    [FAIL] 解析不到 …/xtask/src/code_stats/mod.rs 的 FLUTTER_WEB_BUDGET_MIB ⇒ 漂移比对无法执行
           这不是「跳过」：解析不到常量 = 守卫已静默失效（改常量形状必须同步这里的解析）
==> 失败：见上面的 [FAIL]
EXIT=1

$ grep -c '\[warn\]' 该次输出  -> 0        # V1 的软失败形态【已消失】
$ grep -c '\[FAIL\]' 该次输出 -> 2

$ git checkout -- xtask/src/code_stats/mod.rs
$ git diff --stat -- xtask/src/code_stats/mod.rs    -> 空
$ ./scripts/prune_web_artifacts.sh --check | tail -1
==> 通过      （EXIT=0）
```

#### ④ 收尾：`scripts` / `xtask` 干净 ✅

```text
$ git status --porcelain -- scripts xtask
（零行）
```

#### 额外（我加的第三条探针）：旧 `u64` 形状兼容分支确实活着 ✅

修复声称"同时认旧形状"，但那是一条否则不会被任何现有用法走到的分支，所以我单独走了它：

```diff
-const FLUTTER_WEB_BUDGET_MIB: Mib = Mib(35.0);
+const FLUTTER_WEB_BUDGET_MIB: u64 = 35;
```

```text
$ ./scripts/prune_web_artifacts.sh --check | tail -3
    [ok]   目录体积 30804686 B = 29.38 MiB ≤ 35 MiB（54 个文件）
    [ok]   预算与 xtask FLUTTER_WEB_BUDGET_MIB 一致（35 MiB）
==> 通过
EXIT=0
（随后 git checkout -- 复原；scripts/xtask 仍零改动）
```

#### 关于 §9.4 发现 B 的定性（按 Lead 口径更正）

**发现 B 只是边界登记，不是 bug**：`kill_list()` 的两个 `find` 以 `$DIR/canvaskit` 为根是**刻意且语义正确**的
——Flutter 引擎只会把 `*.symbols` / `skwasm*` / `wimp*` 拷进 `canvaskit/`，脚本头注也把清单来源写明了；
放在 `build/web` 根目录的 `*.symbols` 抓不到**不影响任何真实产物的清减**。V1 记录它的价值仅在于**说明任务书
「往 build/web 放一个 `*.symbols`」这句措辞有歧义**（必须放进 `canvaskit/` 才是判据射程）。此处按 Lead 口径
更正定性，**不要求任何代码改动**。

#### 9.8 结论

**修复成立**：① 未篡改 exit 0；② `Mib(40.0)` → `[FAIL] 预算判据漂移` exit 1 → 复原 exit 0；
③ 解析不到 → `[FAIL] 解析不到…` exit 1 且 `[warn]` 计数 **0**（软失败已消失）→ 复原 exit 0；
④ 结束时 `git status --porcelain -- scripts xtask` **零行**；额外验了旧 `u64` 形状分支仍活。
V1 §9.4 发现 A（`[warn]` 永久软失败）**关闭**；发现 B 按上文更正为**边界登记、非 bug**。
