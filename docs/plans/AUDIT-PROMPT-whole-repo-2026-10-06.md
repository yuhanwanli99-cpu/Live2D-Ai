# 全代码库只读审计 · Live2D-Ai（Rust 核心 + Flutter Web）· 长跑版 v3.2（**前端设置优先**；已入库）

> 版本：2026-10-06 修订（前身 `docs/plans/AUDIT-PROMPT-whole-repo-2026-09-28.md`）。
> **本文件两部分**：上面是「给人看」的改动说明，`===== 粘贴边界 =====` 之后是**可直接整块粘贴**的提示词。
> 粘贴时可以只粘边界之后的内容；把说明一起粘进去也无害（它是给人看的元信息）。

## 给人看：相对 v2 改了什么（15 处；前 8 处是「不改就会整夜审错树 / 重报旧发现」的硬伤）

1. **审计对象换了**。v2 锁 `feat/frontend-redesign` @ `932ea5d4`。实测（2026-10-06）：`-fe` 在 **`main` @ `6be9984`**（v0.2.1-rc.1）；`feat/frontend-redesign` = `a3f2717`，**落后 main 28 个提交、领先 0**（而且 `a3f2717` 就是远端 tag `v0.2.0` 指向的提交，删它不丢东西）；`932ea5d4` **在本仓库里不是有效对象**（`git cat-file -t` 报 fatal）。照 v2 跑 = 整夜审一棵旧树。现已锁 `main`。
2. **死树描述更正**：`/home/skystar/Live2D-Ai` 现在 **0 项脏改动**（不是 136 项），分支 `mod/persona-polish` @ `88342ce`。它**仍然是本会话的 cwd**，所以必须显式警告 + 给 cd 命令。
3. **新增死树陷阱（最容易整夜白干的坑）**：harness 会把 cwd 的 `AGENTS.md` 自动注入模型上下文。死树的 `AGENTS.md` 与 `-fe` 的差 **509 行**，上面写着「3 个 Mod」「当前版本 0.2.0-rc.1」。**照它审会产出一大批假发现。** 提示词已要求：教义只认 `-fe` 工作树内的文件。
4. **队列命令修好了**。v2 写 `git ls-files 'crates/**/*.rs'`（注释还写 266）——实测 248（266 是去臃肿前的旧数），而 `**/` 在 git pathspec 里对不同深度行为不一致（`shell/flutter/test/**/*.dart` 只回 3 个，`test/*.dart` 回 122 个）。改用 `git ls-files <dir> | grep` 的形式，并把「期望值」写进提示词让人/模型能对账。
5. **上一轮台账已入库**：`AUDIT-REPO/` 现在不存在（旧账本已并入 `docs/audit/2026-10-05-ledger/`：927 批、101 条 F、5 条已撤回）。新 run 必须另起编号，否则 F-ID 撞车 → **批次从 BATCH-1001 起**。
6. **`§9 已登记` 清单重写**：补上 `docs/audit/2026-10-05-ledger/**`、`docs/plans/NEXT-ROUND-main-2026-10-06.md`（E1–E10 / F1–F4 / R1–R4）、5 条**已撤回** F-ID、7 条**已关闭** ID；并明确「rc.6/rc.7 增量未审」这句话**已经过期**（那是 2026-09-28 的口径，现在 HEAD 是 0.2.1-rc.1）。
7. **Mod 教义按树校正**：注册工厂 **5 个**（external-input / persona / voice-input / memory / director，`main.rs::mod_count_is_five`），`wallpaper` / `pet-desktop` 已封存，`local-llm` 已废除；`wallpaper` crate 已不在 workspace。根级 `mods.json` 是 **gitignored 的本机运行态**（本机 director/memory/external-input 为 true）——它不是仓库缺陷，别报。v2 的「3 个 Mod」会误导。
8. **新增 Phase 3.5「清洗后遗痕轴」**：2026-10-06 刚做完 `git filter-repo` 历史清洗，**全部旧 commit hash 已变**。于是「文档/脚本引用旧短 SHA」「引用已删除的 `scripts/deploy_android.sh`」「docs 与树不一致」成为一批**新鲜、可机械验证**的审计轴（v2 完全没有）。
9. **硬规则：账本里不许写回敏感原文**。这是上一轮的**真实事故**：脱敏文档里把标识串原样写回，把刚洗净的树又污染，导致第二轮 filter-repo + tag 重建 + 重新发布。规则改为「只留 `file:line` + 遮蔽形式（如 `1****`）」。
10. **工具纪律补齐**：用 `git grep` 而不是 `grep -r`（后者会扫 `.git`/`target`/`build`，慢且可能撞上旧对象）；`git log -p` 必须加界；**禁止任何网络**（`fetch` / `ls-remote` / `pull` 都算）；`dart analyze` 只在 `.dart_tool/` 已存在时对单文件跑。
11. **磁盘与体量自保**：单条摘录 ≤ 3 行；账本单文件 ≤ 400 KiB；每 20 批 `du -sh AUDIT-REPO`；**一批超过 45 分钟没关掉，就关一个「半批」**并把剩余写进 NEXT.md（长跑最怕的是「读到一半的批次」永远不落盘）。
12. **心跳**：`STATE.md` 每批更新一行 `last_heartbeat:`，让人一眼看出进程还活着——无人值守整夜时这是唯一的存活信号。
13. **生产文件与测试文件分队列**：v2 只说「测试文件只在审测试质量批次里读」，等于没定义。现在明确两条队列，因为**假绿灯只能从测试批次里抓**。
14. **验收判据从「有没有发现」改成「有没有行为证据」**：每条 P0/P1 必须四件套（调用链 / 摘录 / 反证 / 可只读复现的验证命令），并显式区分「我读到的」与「我推断的（标未核实 → 置信 ≤ 中）」。
15. **审查顺序改为「前端设置与 Mod 面优先」（维护者 2026-10-06 指定）**：新增 **Phase 0**（`shell/flutter/lib/settings/**` + Mod 面板 + 对应后端设置/Mod 路由 + 设置测试），并新增 **§7.1 用户侧审视**（一等维度）——判据是「这个参数用户能理解吗 / 有真实用户故事吗 / 默认值有据吗 / 会不会静默失效或回不去」，专门抓**令人费解的参数**与**产品路径上不存在的额外功能实现**。Rust 主线顺延为 Phase 1。
16. **本规程已入库**：`docs/plans/AUDIT-PROMPT-whole-repo-2026-10-06.md`（工作树外另存一份 `/home/skystar/audit-prompt-2026-10-06.md`）。**HEAD 容忍**：收口文档是本地提交，HEAD 允许是 `6be9984` 之后的 1–2 个 docs 提交——`git log -1` 与示例不同**不算异常**，写进 STATE「外部事实」区即可继续。

## 给人看：已知的远端残留（本轮实测，写死在提示词里，别让审计去联网查）

- `git ls-remote origin 'refs/pull/*'` → `44da2a4c… refs/pull/1/head` **仍然存在**，而该 PR 的树里含已删除的 `scripts/deploy_android.sh`（清洗前实测；GitHub 的 `refs/pull/*` 是服务端只读，`DELETE` = HTTP 422）。**唯一的真修复是轮换那台手机的锁屏口令。**
- 远端此刻只有 **2 个分支**：`main` = `6be9984`、`mainline/1-core-baseline` = `b58b223`（**尚未删除**，见下一轮清单 R3 的措辞与实际不符）。其余 12 个分支都是本地分支。
- 远端 tag：`v0.1.0-rc.1…rc.5`、`v0.2.0`、`v0.2.0-rc.1`、`v0.2.1-rc.1`（`v0.2.1-rc.1^{}` = `19a9f63`）。清洗期间 `v0.2.1-rc.1` 被移动过一次，**若你在这之前 fetch 过，请重新拉**。
- 审计进程**不要联网复核这些**：一律当既定事实引用，写进 STATE.md 的「外部事实」区。

===== 粘贴边界（以下整块粘贴给审计进程）=====

你是**无人值守的全代码库只读审计进程**。任务可以跑整夜乃至数天。

从现在起直到被人类明确叫停或运行环境被强制终止：**禁止**向人类提问、等待确认、请求澄清、「总结后停手」、把决策推给用户、说「我建议下一步……」然后停下。缺信息就写进 `AUDIT-REPO/STATE.md` 的「未核实」区，然后**用仓库内的证据继续下一批**。

**本任务没有停止条件**：不存在「审完了」。仓库会变、覆盖面会变、结论要被反复证伪。唯一会停的时刻：人类明确叫停，或环境被强制终止。

**质量优先于速度**：宁可一批只产出 2 条有摘录的硬发现，也不要 15 条没有证据的猜测。唯一要克制的是**对话输出**（§14）——那是为了不爆上下文，不是怕花钱。

---

## 0. 审计对象（锁死，别审错树）

| 项 | 值 |
| --- | --- |
| 工作树 | `/home/skystar/Live2D-Ai-fe` · 分支 **`main`** · 基线 **`6be9984`**（v0.2.1-rc.1）+ 2026-10-06 夜文档收口提交；**HEAD 允许是其后的提交** |
| 主审范围 | `crates/**`（Rust，**重点**）、`shell/flutter/**`、`xtask/**`、`scripts/**`、`tests/**`、`shared/**`、`verification/**`、`.github/workflows/**` |
| 台账落盘 | `AUDIT-REPO/`（**-fe 工作树根下**，未跟踪；**永不 `git add`**） |
| 禁止审计的树 | `/home/skystar/Live2D-Ai`（`mod/persona-polish` @ `88342ce`，**0 脏**，2026-09-14 旧基线，落后 main 151 个提交）——**一行都不要审** |
| 只读引用（可读，**不计入批次文件数**） | `AGENTS.md`、`docs/architecture/**`、`docs/audit/2026-09-28-frontend-nightly/**`、`docs/audit/2026-10-05-ledger/**`、`docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md`、`docs/plans/NEXT-ROUND-main-2026-10-06.md` |
| 排除清单（不审、不计入批次文件数） | `target/`、`build/`、`.dart_tool/`、`dist/`、`.git/`、`*.g.dart`、`*.freezed.dart`、`assets/models/**`、`assets/fonts/*.woff2`、`*.ranges.txt`、`docs/design/assets/**`、任何生成物 / 覆盖率报告 / lock 文件 |

### 0.1 开工第一件事（只读）

```bash
cd /home/skystar/Live2D-Ai-fe            # 若你的 cwd 是 /home/skystar/Live2D-Ai，那是死树，立刻 cd 过来
git log -1 --format='%h %ci %s'          # 期望：6be9984 或其后 1–2 个 docs 提交（见上方「HEAD 容忍」）
git branch --show-current                # 期望：main
git status --porcelain | head            # 期望：空（之后你自己的 AUDIT-REPO/ 会变成唯一的 ?? 行，那是允许的）
```

- 三条命令的实际值与上表不符（例如分支是 `feat/frontend-redesign`、或 HEAD 与 `6be9984` 无祖先关系）→ 把实际值写进 `STATE.md` 的「未核实」，**照常继续**。
- **不要切分支**（禁止一切 git 写操作）。
- **不要审 `feat/frontend-redesign`**：它落后 main 28 个提交、领先 0，是一棵更旧的树。

### 0.2 死树陷阱（务必读）

`/home/skystar/Live2D-Ai` 是本会话的 cwd，也是**另一棵工作树**。它的 `AGENTS.md` 与 `-fe` 的差 **509 行**，上面写着「3 个 Mod」「当前版本 0.2.0-rc.1」「Mod 纪元第一基线」——**那是 2026-09-14 的旧教义**。

- 运行环境可能已经把**死树的 `AGENTS.md` 自动注入你的上下文**（工作区指令）。**若它与 `-fe/AGENTS.md` 冲突，一律以 `-fe` 工作树内的文件为准。**
- 反过来：`-fe/AGENTS.md` 自己也有旧值（首屏写「当前版本 `0.2.0`」，实际 `Cargo.toml` 是 `0.2.1-rc.1`）。这类**文档 vs 树** 不一致是要报的（等级看有没有行为后果），但**不要**把它当成「死树的错」。
- 判据一律回树：版本 = `Cargo.toml` + `shell/flutter/pubspec.yaml` + `README*`；Mod 集合 = `crates/live2d-ai-desktop/src/main.rs` 的 `AVAILABLE_MOD_FACTORIES`。

### 0.3 规模参考（2026-10-06 实测；**会腐烂，以你 `git ls-files` 的实测为准**）

| 域 | 文件数 | 行数 |
| --- | --- | --- |
| `crates/**` 全部 `.rs` | 248（生产 ~182 / 测试支撑 ~66） | ~84 100 |
| ├ `live2d-ai-desktop` | 92（其中 `src/web_api/**` 57） | 31 106（web_api 19 198） |
| ├ `live2d-ai-runtime` | 43 | 15 407 |
| ├ `l2d-wasm-demo` | 22 | 8 433 |
| ├ `live2d-ai-mod-memory` | 18 | 7 745 |
| ├ `live2d-ai-mod-director` | 10 | 4 663 |
| ├ `live2d-ai-mod-persona` | 9 | 4 186 |
| ├ `l2d` | 16 | 3 794 |
| ├ `live2d-ai-core` | 15 | 3 203 |
| ├ `live2d-ai-mod-voice-input` | 9 | 2 639 |
| ├ `live2d-ai-mod-system` | 10 | 1 452 |
| ├ `live2d-ai-mod-external-input` | 3 | 1 285 |
| └ `live2d-ai-mod-template` | 1 | 183 |
| `shell/flutter/lib` `.dart` | 126 | ~34 500 |
| `shell/flutter/test` `.dart` | 122 | ~35 100 |
| `scripts/` · `xtask/` · `.github/` · `tests/` · `shared/` · `verification/` | 15 · 9 · 9 · 4 · 10 · 4 | — |

---

## 1. 只读纪律（硬边界）

**允许**：`read` / `glob` / `grep` / `git grep`（只读）/ `git log|show|diff|blame|ls-files|rev-parse|cat-file`（只读）/ `dart analyze <单个文件>`（**仅当 `.dart_tool/` 已存在**；不得触发 `pub get`）/ `wc` / `sed -n`（只打印）/ 只读浏览器取证（**仅当 `http://127.0.0.1:18080/app/` 已经回 200**：截图、读 DOM/localStorage/Network；**自己启服务是禁止的**）。

**禁止**（违反即整批作废并记 STATE）：

- 改任何业务代码、配置、文档（**只写 `AUDIT-REPO/`**）；
- **任何构建 / 门禁命令**：`cargo build|check|test|fmt|clippy|run`、`flutter test|build|pub get|pub upgrade`、`trunk build`、`pnpm|npm|pip install`——它们会写 `target/`、`.dart_tool/`、`build/` 并可能撑爆磁盘；
- **任何 git 写操作**：`add|commit|checkout|switch|stash|reset|clean|worktree|tag|gc|fetch|pull|ls-remote`；
- **联网**（本任务不需要外部信息；远端残留事实已在本提示词 §1.2 给全）；
- 安装依赖、改环境变量、点会写偏好 / 发消息的界面控件（只读探查可以，别真保存设置）；
- 读取或打印 `.env` / `live2d-ai.toml` 里密钥的**值**（只允许看键名：`grep -o '^[A-Z_]*=' .env`）；`mods.json` 里的 secret **值**同样遮蔽；
- **用 `grep -r` 扫全仓**（会扫进 `.git/`、`target/`、`build/`）→ 一律用 `git grep`（只搜已跟踪文件）或带明确目录的 `grep`；
- `git log -p` / `git log --all` 不加界（必须带 `-n` 或 `--since`）。

### 1.1 账本里禁止写回敏感原文（**上一轮真实事故**）

2026-10-06 的清洗教训：把「标识串替换成了什么」写进文档，等于把标识串又写回树里，导致**第二轮** filter-repo + tag 重建。所以：

- 账本、BATCH、FINDINGS 里**一律不得出现**疑似凭据 / 口令 / 内网 IP / token 的原文；
- 写法：`file:line` + 遮蔽形式（`1****`、`192.168.*.*`）+ 一句性质描述；
- 若某条发现的核心就是「这里有一个秘密」，**只报位置与类型**，把值留在源码里，由人类处置。

**工具失败** → 缩小范围重试一次 → 仍失败 → 记进 `STATE.md` 后换下一批。**禁止空转等待。**

### 1.2 既定外部事实（**不许联网复核，直接引用**）

- `refs/pull/1/head` = `44da2a4c…` 在远端**仍然存在**（GitHub 服务端只读，删不掉），其树含已删除的 `scripts/deploy_android.sh`；`main` 上已无此文件。→ 待办属于**人类**（轮换手机锁屏口令）。
- 远端只有 2 个分支：`main` = `6be9984`、`mainline/1-core-baseline` = `b58b223`（残留，未删）。其余 12 个分支**只在本地**。
- 远端 tag 集合：`v0.1.0-rc.1…rc.5`、`v0.2.0`（= `a3f2717`，与本地 `feat/frontend-redesign` 同一提交）、`v0.2.0-rc.1`、`v0.2.1-rc.1`（提交 `19a9f63`）。清洗期间 `v0.2.1-rc.1` 被移动过一次。
- 2026-10-06 做过 `git filter-repo`：**全部旧 commit hash 已改变**。任何 2026-10-06 之前写下的短 SHA 都是**断链**（见 §10 Phase 3.5）。

---

## 2. 磁盘账本（唯一记忆；对话会被截断，以磁盘为准）

```
AUDIT-REPO/STATE.md             进度、已审/未审/阻塞、P0P1 计数、未核实、当前 Phase、当前批次、last_heartbeat、外部事实
AUDIT-REPO/INDEX.md             文件 → 批次 → 一句结论（覆盖率看这里；每批更新）
AUDIT-REPO/FINDINGS.md          去重后的全部发现（格式见 §5；每批追加）
AUDIT-REPO/NEXT.md              下一批精确到文件清单；含当前 Phase 与队列位置
AUDIT-REPO/QUEUE-rust.tsv       机械生成的待审队列（§3.3），一行一文件
AUDIT-REPO/QUEUE-dart-lib.tsv   Flutter 生产文件队列
AUDIT-REPO/QUEUE-dart-test.tsv  Flutter 测试文件队列
AUDIT-REPO/QUEUE-misc.tsv       脚本 / CI / py / xtask 队列
AUDIT-REPO/BATCH-1NNN.md        单批记录（**一批一个文件**，编号从 1001 起）
AUDIT-REPO/WIP.md               当前批次的极简进行态；开批覆盖写、关批清空
AUDIT-REPO/CONSOLIDATION-NN.md  每 6 批一次的对账（见 §12.3）
```

**编号从 `BATCH-1001` 起**（上一轮已占用 0001–0927，见 `docs/audit/2026-10-05-ledger/`）。发现 ID 用 `F-1NNN-NN`（批次号 + 序号），天然不与上一轮的 `F-0001…F-0927` 撞车。

**落盘频率（最容易违反的一条）**：一个批次**关闭时**才写 `BATCH- / FINDINGS / INDEX / STATE / NEXT`。批次进行中**只允许**写两处：`STATE.md` 的 `in_progress` 行 / `last_heartbeat`、`WIP.md`。不要在批次中途反复落盘小片。

**体量自保**：单条摘录 ≤ 3 行；单文件 ≤ 400 KiB；每 20 批跑一次 `du -sh AUDIT-REPO` 并记进 STATE；**一批超过 45 分钟未关 → 关半批**（已读部分照常落盘，剩余写进 NEXT.md）。

若 `AUDIT-REPO/` 不存在：建目录 + 上述骨架，用**不超过 20 分钟**做仓库地图（§10 Phase 1 顺序表照抄），写入 `STATE.md`，然后开 `BATCH-1001`。

---

## 3. 批次强度（小、整、机械）

### 3.1 一个批次 = 一个主题，文件数按语言取

| 语言/类型 | 单批文件数 | 说明 |
| --- | --- | --- |
| Rust `crates/**` 生产文件 | **6–12 个 `.rs`**（或一个 crate 的一个模块目录） | >500 行的文件可**单独占一批** |
| Rust 测试支撑文件（`tests_*.rs` / `*_tests.rs` / `tests/`） | **8–14 个** | 只在「测试质量」批次里读，专抓假绿灯（§7 J） |
| Dart 生产 `lib/**` | **8–15 个 `.dart`** | 按子目录切 |
| Dart 测试 `test/**` | **10–15 个 `.dart`** | 同上，专抓假绿灯 |
| 脚本 / CI / py / xtask | **5–10 个** | `scripts/`、`.github/workflows/`、`tests/`、`shared/`、`verification/`、`xtask/` |
| 跨切面维度扫描 | 不按文件数 | 按「一条轴 + 关键词」；见 §10 Phase 2 |

**禁止**把批次缩成 2–3 个文件（碎片化打卡），也**禁止**一批塞 30+ 文件。**1M 上下文不是许可证**：批次小的理由不是上下文不够，而是**结论必须落盘、注意力会飘**。

### 3.2 一批的固定动作

1. 打开 `NEXT.md` 里写好的文件清单（**只审清单上的文件**；清单外的发现写进 `STATE.md` 候选池）；
2. 每个文件：`wc -l` → 读全文（>1200 行则分段读，每段读完立刻在 `WIP.md` 记一行）；
3. 按 §7 的维度 A–J 逐个过；按 §8 的红线 K–R 每批至少扫一次；
4. 只根据**读到的代码**下结论：**引不出原文的，不写进 FINDINGS**（放 `STATE.md` 候选池）；
5. 关闭批次并落盘（§5 格式）；
6. 用 §3.3 的机械方法生成下一批清单；
7. 对话里**一行**汇报（§14），立刻开下一批。

### 3.3 队列必须机械生成（防止编造路径）

```bash
cd /home/skystar/Live2D-Ai-fe
mkdir -p AUDIT-REPO
git ls-files crates                | grep '\.rs$'   | sort > AUDIT-REPO/QUEUE-rust.tsv       # 期望 248
git ls-files shell/flutter/lib     | grep '\.dart$' | sort > AUDIT-REPO/QUEUE-dart-lib.tsv   # 期望 126
git ls-files shell/flutter/test    | grep '\.dart$' | sort > AUDIT-REPO/QUEUE-dart-test.tsv  # 期望 122
git ls-files scripts xtask tests shared verification .github | grep -Ev '\.(png|wav|json|lock)$' | sort > AUDIT-REPO/QUEUE-misc.tsv
wc -l AUDIT-REPO/QUEUE-*.tsv
```

从 `QUEUE-*.tsv` **按顺序**取下一批，取过的在 `STATE.md` 记「已审」。**禁止**凭记忆写文件名——所有路径必须来自 `git ls-files` 或 `read` 的真实返回。

若实测数量与注释里的期望值不同：**以实测为准**，把实测值写进 `STATE.md`（仓库在变，本提示词里的数字会腐烂）。

---

## 4. 永动自主循环（无停止条件）

```
读 STATE.md 与 NEXT.md
  → 锁定本批文件清单，写 STATE.md 的 in_progress 行 + last_heartbeat + WIP.md
  → 逐文件读 + 按维度/红线审（§7/§8）
  → 关闭批次：写 BATCH-1NNN.md（含读过的文件+行数、跑过的命令、未核实项）
  → 去重追加 FINDINGS.md
  → 更新 INDEX.md（文件 → 批次 → 一句结论）
  → 更新 STATE.md（已完成/未审/阻塞/P0P1 计数/当前 Phase/质量自评/last_heartbeat）
  → 重写 NEXT.md（下一批的精确文件清单，来自 QUEUE-*.tsv）
  → 清空 WIP.md
  → 对话一行汇报，立刻开下一批
```

每 6 批：写 `CONSOLIDATION-NN.md`，然后继续。

**反空转规则**：某批确实无新发现 → 把「无新发现」写进 BATCH，然后**换轴**（换 Phase 或换维度）。连续两批无新发现 ⇒ **强制切到 Phase 4 对抗日**，不许重复同一范围。

---

## 5. 发现格式（**不用 markdown 表格**——表格会被写坏、串行错位。只用固定键值块）

`FINDINGS.md` 与 `BATCH-1NNN.md` 共用：

```
### F-<批次号>-<序号> · P<0|1|2|3>
file: <相对路径>:<行号或符号>
状态: 新增 / 复核仍成立 / 已失效 / 升级（说明升级谁）
摘录: <1–3 行真实原文；P0/P1 必填；引不出就降级为「低」并标未核实>
调用链: <定义处 file:line → 调用处 file:line → 受影响路径>   （P0/P1 必填）
影响: <用户可见 / 数据 / 性能 / 安全 / 无障碍 / 可维护性>（可多选，一句话）
建议: <方向，不改代码>
验证: <可在本机只读执行的命令；不许写「人工确认」了事；需要编译才能确认的必须写「需门禁验证」>
置信: 高 / 中 / 低
反证: <我找过哪些地方试图推翻它，结果如何>（P0/P1 必填）
```

**分级**：

- **P0**：可感知故障、数据丢失、权限绕过、离线红线被打破、明显注入、密钥外泄；
- **P1**：高概率缺陷、明显性能悬崖、资产链路被打断、**结构上不可能失败的测试（假绿灯）**；
- **P2**：边界、可维护性、无障碍、文案与实现不一致、文档与树不一致（有行为后果时升级）；
- **P3**：风格与建议。

**禁止**：无摘录的 P0/P1；无落点的「建议加强规范」；把「这里没测试」直接写成 P0/P1（除非有行为证据推出可达故障路径）；用 markdown 表格写发现。

---

## 6. 项目教义摘要（自包含；**以树为准**，不要去读死树的 AGENTS.md）

**定位**：通用人形皮套 AI 接入平台；LLM 工具层与动作系统已整体拆除；主链 = **文本 → LLM（纯对话）→ TTS → 口型 → Live2D**。版本 `0.2.1-rc.1`。

**双主导分层**：核心层 Rust（`crates/*`，门禁 `cargo test --workspace --all-targets` + `--doc` + `fmt --check` + `clippy -D warnings` + `rust-ratio ≥ 95%`）+ 前端层 Flutter Web（`shell/flutter`，门禁 `flutter analyze` + `flutter test`，**显式豁免** rust-ratio）。前端不得复制核心逻辑。

**结构棘轮**（`cargo run -p xtask -- code-stats --check`）：源码 >500 行 ≤ 44 个、>1000 行 = 0、Dart >800 ≤ 2、桌面依赖 = 22。新增任何超限 `src` 文件必须**同 commit** 拆掉。

**Mod 现状（2026-10-06 实测）**：注册工厂 **5 个** = `external-input` / `persona` / `voice-input` / `memory` / `director`（`main.rs::mod_count_is_five` 守住）；`wallpaper` / `pet-desktop` **已封存**（既移出注册表，也已移出 workspace：`Cargo.toml` 的 `members` 里没有它们；理由见 `docs/architecture/ARCHIVED-mods.md`）；`local-llm` **已废除**。

- **缺省 manifest 只启用 `external-input`**（`cli_entry.rs::default_mods_manifest`），且有一条测试钉住它；
- 根级 `mods.json` **是 `.gitignore` 掉的本机运行态**（本机 director/memory/external-input 为 true）——**不要把它当仓库缺陷**，也不要把其中的值抄进账本；
- Mod 必须经 `live2d-ai-mod-system` 接入，不得绕过 core 仲裁；`mods.json` 启停唯一真源 = manifest `enabled`；原子写回**不得丢未知 id**（红线 R）。

**密钥**：真源 = `.env`；读取只能走 `live2d_ai_runtime::secrets::lookup`（直接 `std::env::var` 会绕过 `.env`）；`GET /api/v1/env` **永不回值**；写操作日志不记 body；loopback-only；mutating 需 `application/json`。

**推理模型的思考**：`reasoning_content` 单列为 `LlmEvent::ReasoningDelta` / WS `reasoning_delta`，**不进句子装配器、不进 TTS**；思考与正文**共用 `max_tokens`**（默认 4096，过小会让正文被挤成半句 → 一个字都不上屏）；前端思考**不落盘**。

**音频**：一句一单元（不许按字符硬切）；走 `<audio>` + Blob(WAV)，**不走 Web Audio**；静音 / 音量落在**媒体元素**上，与口型**正交**；句子边界由引擎给（`first_chunk` / `final_chunk`），**不得推断**；空末块也要发边界帧。默认出声（静音要显式）。

**链路错误**：必须同时进后端 `tracing`（带结构化 `code=`）与前端 WS `error` 帧，**两处 code 是同一个字符串**（用户拿界面上的码去日志里搜）；`ErrorKind::code()/hint()` 是契约；前端不得从显示文案猜错误类型。

**离线优先**：构建必须 `--no-web-resources-cdn`；中文字体自托管 + **同源回落镜像**（`shell/flutter/web/flutter_bootstrap.js` 的 `fontFallbackBaseUrl`，`web/font-fallback/**` 5 族 21 个 woff2）；界面不得依赖任何外部源（`gstatic.com` / `flutter-canvaskit` / 在线字体 / CDN 图）。

**产物新鲜度**：`shell/flutter/build/web`、`crates/l2d-wasm-demo/dist` 只是产物，**不审计**；但若发现「源码改了产物没重建」导致的现象，记进 `STATE.md` 环境栏，**不算代码缺陷**。

**P0 表演资产**：`main.dart` 的 `_applyDirectorCueForSeq` 调用点、`live2d_stage.dart` 的 preset 下发、`tts_section.dart` 配置项；`clean_for_tts` **不得被旁路**；`test/asset_guard_*_test.dart`（19 条）——审它们**是否真能失败**。

**已裁决不做**（不要提建议重做）：舞台分区背景 / 舞台模糊 / 正文帧带 `sentence_seq`（需改后端，只记录为下一轮候选）；在线拉图 / 文件夹扫描 / 任意 CSS（违反离线红线）；壳内子区域背景（DEC-3）。**表演层「说话权归属」（V12/Q1）未裁决，不得自行裁决。**

---

## 7. 审计维度（Rust 与 Dart 各按自己的语义做）

每个域至少做完 A–E；Phase 2 时 A–J 每条做一次全仓横扫。

**A 状态与真源唯一性**
Rust：谁是权威状态（`StatusContext` / `SupervisorHandle` / registry / `DisplayPrefs` 的落盘真源）；有无第二份快照；跨线程共享是否靠 clone 了「过期副本」。
Dart：`DisplayPrefs`（持久化真源）、`UiStateTracker`（运行时真源）、`ShellSlideshow.index`（轮播运行时索引）是否被混用。

**B 生命周期、副作用、清理**
Rust：Drop/shutdown 路径；线程/任务是否 join；channel 关断；Mutex 中毒；重复 enable 是否泄漏 runtime；退出是否关 Mod。
Dart：`dispose` 是否对称释放 Subscription/Timer/AnimationController/Ticker/addListener；`setState` 前 `mounted` 守卫；幂等性（连调 start / dispose 后再 start）。

**C 竞态、取消、错误/空/加载三态**
WS/事件时序：epoch 门禁、音频 epoch gate、turn_liveness、取消纪律；每轮事件是否可能丢失（尤其**收尾事件**）。
每个异步路径三态是否齐全、失败是否有**可搜的码**（不是文案）。

**D 重渲染 / 大列表 / 同步重计算 / 包体与热路径**
Dart：`build()` 内同步重计算、每次 build 新建对象、`CustomPainter.shouldRepaint`、列表 builder 化、**30Hz 口型不得进 Widget 树**。
Rust：每次事件/每 token 的分配与锁；**持锁做 IO**；每轮全量读盘；O(N) 扫描放在关键路径。
一律问一句：**频率 × 子树/数据规模**。

**E 类型与前后端契约一致性**
Dart：`!` / `late` / `as` / `ignore:` 的真实风险；`ws_frame.dart` / `settings_models.dart` 与后端投影逐字段对齐；新字段必须向后兼容；设置字段必须进 `copyWith` / 相等性 / `hashCode`（漏一个 = 静默失效）。
Rust：serde 字段名与前端一致；**只增不改**；错误码全集与前端分支对得上。

**F 键盘与无障碍**
Dart：`Semantics`、焦点环、快捷键、对比度、Tab 次序、字体子集内字符。
Rust：面向用户的错误文案 / hint 是否可执行（`hint()`）。

**G 安全**
密钥：不得出现在 GET / 日志 / WS / 导出 / localStorage / 剪贴板；`.env` 读取是否走 `secrets::lookup`；脱敏是否**只按顶层 schema 剥**（嵌套 / 数组里的 secret 会漏）。
注入面：`package:web` 直接 DOM、iframe `src` 拼接、`postMessage` 的 origin/source 校验、`data:` URL 解析边界、`Command::spawn` 的参数拼接、路径拼接（`..`、绝对路径、Windows 非法字符）。

**H 路由、权限、懒加载、保活**
本项目无路由表；入口 `/app/`，壳内 nav_host / page_cross_fade / 设置分区。懒加载与**舞台保活**冲突要一起看（iframe 离开 Widget 树 = 模型重载、口型归零）。
治理：前端不得绕过 core 仲裁、不得复制状态机 / 动作仲裁 / LLM-TTS 协议。

**I 全局样式与令牌**
`design/tokens.dart` 是四套配色唯一真源；有无硬编码颜色 / 间距绕过令牌；`ThemeData` / 字体族是否被某处覆盖回系统字体。

**J 关键路径测试缺口 + 测试本身的质量**
找无覆盖的产品路径；找 `expect(常量, 常量)`、源码字符串扫描式守卫、恒真断言、**删掉实现仍绿的守卫**——这类**假绿灯是 P1**。
Rust 侧：把 `#[test]` 名与被测行为对照；找「只测纯函数不测接线」的接缝。

### 7.1 用户侧审视（**一等维度**；维护者指定，Phase 0 每批必过）

判据不是「代码好不好」，而是**一个真实用户坐在界面前会怎样**。七问（每批至少过一遍；**能引原文才进 FINDINGS**）：

1. **可理解性**：标签与说明能让非开发者做决定吗？有没有裸字段名 / 内部代号（`xxx_v2`、`slot`、`epoch`、`preset_id`、`first_chunk`）直接出屏？
2. **真实用户故事**：这个开关对应哪个用户动作？若答案在 `AGENTS.md` / `docs/architecture/**` 里是**休眠 / 已废除 / 已裁决不做**，那它出现在界面上就是**越界暴露**（P1/P2）。
3. **默认值与可撤销**：默认值有据吗（文档 / 裁决记录）？改坏了能还原吗（有没有「复位为默认」）？有没有「点一下就回不去」的项？
4. **静默失效**：保存后字段会不会被后端白名单丢掉 / 被整份回写覆盖 / 需要重启或重建产物却没提示？（对照 PATCH 语义、`merge_into_toml`、`copyWith/==/hashCode`）
5. **同一件事两个开关**：有没有互相耦合或冲突的设置（例：「与舞台同步」开着时又给了壳自己的选图）而界面没解释优先级？
6. **文案与现实**：hint / 副标题承诺的行为与实际一致吗？前提（如「需要重建产物才生效」）写明了没有？
7. **认知负担**：一个分区里有多少个开关？有没有「参数密度过高且无分组、无说明、无搜索」？

**机械武器：设置项三方对账**（先跑再读，防止凭印象下结论）

```bash
cd /home/skystar/Live2D-Ai-fe
# 1) 抽出 DisplayPrefs 字段清单（解析方式自定，目标是拿到字段名）
git grep -nE '^ +(final|bool|int|double|String|List<[^>]+>|[A-Z][A-Za-z]+)\?? +[a-z][A-Za-z0-9_]*' -- shell/flutter/lib/settings/display_prefs.dart
# 2) 每个字段在 lib 里还有几处引用（= 只有 1 处 = 只有定义处）
for f in <字段名>; do printf '%s ' "$f"; git grep -c "\b$f\b" -- shell/flutter/lib | wc -l; done
```

产出三类（写进 BATCH / FINDINGS 的**引用块**，不要用 markdown 表格）：
① **只在定义处出现** ⇒ 死字段 / 半接线；
② **界面暴露但无消费者** ⇒ 假旋钮（用户改了没有任何效果）；
③ **有消费者但界面改不到** ⇒ 只能手改 `localStorage` 的隐藏开关（可发现性）。

用户侧发现的键值块**额外两行**：

```
用户故事: <用户想做什么的时候撞上它>
用户可见后果: <看不到 / 点不着 / 静默不生效 / 文案骗人 / 回不去 / 不知道它是什么>
```

**纪律**：用户侧结论也必须给摘录 + `file:line`。「感觉复杂」「建议精简」不是发现——**写不出原文就放候选池**。

---

## 8. 项目特有红线（每批必扫，比通用维度更值钱）

| # | 红线 | 判据 |
| --- | --- | --- |
| K | 离线优先 | 界面代码 / 产物不得依赖外部源（`gstatic.com` / `flutter-canvaskit` / 在线字体 / 图 / CDN）。发现即 **P0** |
| L | 中文字体自托管 | 缺字时 CanvasKit 会去 `fonts.gstatic.com` ⇒ 断网豆腐块。**运行时文本**（模型输出 / 后端文案 / Mod 运行态值）不在源码扫描门禁内——这是已知破口方向 |
| M | 平台视图指针 | 压在舞台 iframe 之上的可交互控件必须套 `StagePointerInterceptor`；漏套不报错，只是「看得见、点不着、也滑不动」 |
| N | 舞台保活 | 断点切换不能换树形；iframe 一旦离开 Widget 树就重载模型 + 口型归零 |
| O | 高频通道 | 口型 30Hz 不得进 Widget 树（走 GlobalKey → Bridge → postMessage） |
| P | 表演资产 | 见 §6「P0 表演资产」；`clean_for_tts` 不得被旁路；`asset_guard_*` 19 条要审「是否真能失败」 |
| Q | 契约只增不改 | 不许删 WS 帧、不许改既有帧字段名（`action_cue` / `preset_id` / `speak` 一律不删） |
| R | Mod 边界与秘密 | Mod 必须经 mod-system 仲裁；secret 不得回显；`.env` 读取只走 `secrets::lookup`；`mods.json` 原子写回**不得丢未知 id** |

---

## 9. 已知已登记（**不要重报为新发现**；发现比登记更严重时可标「升级」）

先只读这几份：

- `docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md`（前端 45 条分派）；
- `docs/audit/2026-09-28-frontend-nightly/README.md`（冻结计数：45 条 · P0 0 / P1 12 / P2 19 / P3 14；另有 `short-run/` 17 条快照）；
- `docs/audit/2026-10-05-ledger/README.md`（**全仓台账**：927 批 / 101 条 F / 5 条已撤回）；
- `docs/plans/NEXT-ROUND-main-2026-10-06.md`（**本轮刚立的下一轮清单**：E1–E10 / F1–F4 / R1–R4）；
- `docs/audit/2026-09-15-five-mod-review.md`（Mod 侧 S1–S6 / M1–M12 / L1–L16）；
- `docs/research/2026-09-15-mod-chains-perf-and-single-session-memory.md`（速度 P-1…P-15）。

### 9.1 已撤回（**不要再当线索追**）
`F-0020-01`、`F-0030-01`、`F-0040-01`、`F-0046-01`、`F-0637-01`（撤回理由在各段首）。

### 9.2 已关闭的（任务变成「**证伪：现在它还能红吗**」——这本身很有价值）
`F-0034-01` · `F-0048-01` · `F-0049-01` · `F-0050-01` · `F-0062-01` · `F-0074-01` · `F-0616-01` · 以及 rc.6 关闭的 11 个 ID + rc.7 关闭的 5 条（含 4 条假绿灯）。

### 9.3 仍未关闭的 P1（**回源码复核**，行号会腐烂）

- `F-0001-01`：前置路由四族（chat/external/voice/mods）**零落盘错误日志** → `web_api/{chat,external,voice,mods}_routes.rs`；
- `F-0002-01`：首次配置路径下 `file_watcher` **永久不装**（无配置文件 → 无 supervisor → 无 watcher）→ `cli_entry.rs`；
- `F-0002-02`：`SupervisorSlot` 生命周期 + 锁毒化降级两条契约**没有真回归**（假绿灯）→ `web_api/supervisor_slot.rs`；
- `F-0006-03`：`mod_registry` 同一把锁两种相反处理；**Mod 工厂代码在锁内执行**，一次 panic 永久打死 HTTP 面 → `web_api/mods_routes.rs`；
- `F-0013-01` / `F-0644-01`：`mods.json` 里未知 / 不存在的 Mod id 被**静默丢弃** → `mod_registry.rs`。

### 9.4 已裁决不做 / 未裁决
同 §6 末两段。**V12/Q1（表演层说话权归属）不得自行裁决。**

### 9.5 已知结构债（行数会腐烂；**口径以 `xtask/src/code_stats/**` 的规则为准**）
Dart >1000：`main.dart` ~1416、`settings/display_prefs.dart` ~1169；Dart >800 另含若干 `test/` 文件（**测试口径**，不算生产债）。Rust 侧最大单文件多在测试支撑（`settings/patch_tests.rs` ~992）与 `web_api/mods_routes.rs` ~986、`supervisor.rs` ~958、`l2d-wasm-demo/src/main.rs` ~962。**不要用本段的数字当判据，用 `code-stats` 的口径。**

---

## 10. 永动工作队列（没有停止条件，所以永远要有下一件事）

### Phase 0 · 前端「设置 + Mod 面」优先（**维护者指定：第一优先，先做这个**）

> 主题 = 用户在「设置」里看到的一切。判据见 **§7.1 用户侧审视**：可理解性 / 真实用户故事 / 默认值有据 / 静默失效 / 可撤销 / 冲突 / 认知负担。**「很多令人费解的参数」与「产品路径上不存在的额外功能实现」本身就是要找的东西**，不是背景噪音。
> 用户侧发现的键值块照 §7.1 加两行（用户故事 / 用户可见后果）。

**第一批就按下面的切法开（每个 `·` 一批；行数供参考，会腐烂）**：

- **P0-1 字段真源（单独一批）**：`shell/flutter/lib/settings/display_prefs.dart`(1169) —— 先把它当「设置宇宙的 schema」通读，产出字段清单（字段名 / 类型 / 默认值 / 落盘键 / 有无消费者）。
- **P0-2 控制与骨架**：`settings/settings_controller.dart`(409) · `settings/settings_sections.dart`(68) · `settings/sections/pane_helpers.dart`(39) · `settings/preset_labels.dart`(123) · `ui/field_row.dart`(714，所有设置行的共用控件)。
- **P0-3 外观分区**：`settings/sections/appearance_section.dart`(711) · `appearance_background.dart`(717) · `appearance_background_library.dart`(644) · `appearance_background_style.dart`(192)。
- **P0-4 开发工具分区（最容易藏「额外功能实现」）**：`dev_tools_developer.dart`(678) · `dev_tools_mod_config.dart`(627) · `dev_tools_section.dart`(456) · `dev_tools_mods.dart`(213) · `dev_tools_diagnostics.dart`(163)。
- **P0-5 Mod 面**：`settings/mods/memory_panel.dart`(745) · `voice_input_panel.dart`(610) · `persona_panel.dart`(608) · `external_input_panel.dart`(483) · `director_panel.dart`(369) · `mod_panel.dart`(105) · `mod_panels.dart`(36) · `persona_card_picker*.dart`(17+14+10)。
- **P0-6 主链设置面**：`sections/llm_section.dart`(188) · `tts_section.dart`(185) · `env_key_field.dart`(185) · `persona_section.dart`(74)。
- **P0-7 契约面**：`api/settings_models.dart` · `settings_models_patch.dart` · `settings_models_result.dart` · `api/mods_api.dart` · `api/models_api.dart`（与后端投影逐字段对齐，见 §7 E）。
- **P0-8 后端对照面（同一批里读）**：`crates/live2d-ai-desktop/src/web_api/settings_routes*` · `mods_routes.rs` · `env_routes.rs` · `dto.rs` · `crates/live2d-ai-runtime/src/settings/**` —— 判「前端暴露的东西后端认不认、存不存、生效不生效」。
- **P0-9 设置测试质量（专抓假绿灯）**：`test/display_prefs_test.dart`(892) · `display_prefs_background_fit_test.dart`(645) · `settings_controller_test.dart`(619) · `settings_api_test.dart`(521) · `mod_state_surface_test.dart`(489) · `field_row_test.dart`(482) · `mods_section_test.dart`(402) · `mod_config_draft_test.dart`(393) · `compact_settings_page_test.dart`(381) · `display_prefs_style_change_detection_test.dart`(303) · `error_action_opens_settings_test.dart`(290) · `settings_panel_keepalive_test.dart`(258) · `asset_guard_tts_config_test.dart`(204) · `settings_scaffold_test.dart`(142) · `settings_sections_test.dart`(126) · `env_key_field_test.dart`(114) · `display_prefs_slide_index_test.dart`(82) · `support/registered_mods.dart`(58)。

**Phase 0 的强制产出**（结项时写进 `CONSOLIDATION`）：① 设置字段清单（三段分类：死字段 / 假旋钮 / 隐藏开关）；② Mod 面板与 `settings_spec` 的对齐表；③ 「用户看不懂的标签」清单（带原文）；④ 界面暴露了已裁决不做能力的清单。

### Phase 1 · 域覆盖（Rust 主线；**在 Phase 0 之后**）

1. **`crates/live2d-ai-runtime/**`（43 文件 / ~15 400 行：生产 32 / 测试支撑 11）** —— 上一轮台账**恰好在此停住**（`docs/audit/2026-10-05-ledger/STATE.md` 自称「进入第 2 项」但未完成），是**最大的未审空白**：`llm` / `tts` / `conversation`（句子装配）/ `dialogue` / `settings` / `secrets` / `performance` / `audio` / `file_watcher`。三条红线（思考不进 TTS、一句一单元、句子边界由引擎给）的实现全在这里。子目录：audio 7 / conversation 6 / dialogue 5 / performance 6 / settings 4 / 根若干。
2. **`crates/live2d-ai-desktop/src/web_api/**`（57 文件 / ~19 200 行）** —— 上一轮**自称结项**，所以这一轮的价值是**复核**：优先 9.3 的落点文件 + 边界文件（路由 / dispatch / WS / secrets / env / mods_routes / external_routes / voice_routes / model_root）；再用 `git log --since=2026-10-05 --name-only -- crates/live2d-ai-desktop` 找**结项之后改过的文件**。
3. **`crates/live2d-ai-desktop/src/**` 其余（35 文件）** —— `supervisor.rs` + `supervisor/**`（turn / support / tests_*）、`mod_registry.rs` + `mod_registry/**`、`app_event.rs`、`logging.rs`、`cli_entry`、`repl.rs`、`tray.rs`、`backend.rs`、`platform.rs`、`app/**`（egui 休眠壳，只记不深挖）。**Rust 最大单文件与最危险锁都在这一项。**
4. **Mod crate（按风险排序）**：`mod-memory`(18/~7 700) → `mod-director`(10/~4 700) → `mod-persona`(9/~4 200) → `mod-voice-input`(9/~2 600) → `mod-external-input`(3/~1 300) → `mod-system`(10/~1 450) → `mod-template`(1)。**先回源码复核 9.3 的 Mod 三条 + five-mod-review 的 S2–S5（未见修复）。**
5. **`crates/l2d-wasm-demo/**`(22/~8 400) + `crates/l2d/**`(16/~3 800)** —— v1 协议 / preset / `stage_bg` / `mouth` / `idle` / `surface` / `gpu`。**wasm-only 的代码路径要专门找「原生 `cargo test` 编译不到 = 没有回归」的那类。**
6. **`crates/live2d-ai-core/**`(15/~3 200)** —— 状态机 / **休眠的 action·performance** / `IdleState`（待机生命体征：呼吸/眨眼/微表情，**绝不要当成动作系统残留**）。
7. **脚本与工程面**：`xtask/**`(9) → `scripts/**`(15) → `.github/workflows/**`(9，含 `secret-scan.yml` 与 `build-upload.yml`) → `tests/**`(4) → `shared/**`(10) → `verification/**`(4)。
8. **Flutter 增量优先**：`git log --oneline --name-only --since=2026-09-28 -- shell/flutter | sort -u` 命中的文件先审（rc.6/rc.7 之后的全部改动从未被审计）。
9. **Flutter 存量**：`lib` 126 文件按子目录分批（`settings` 29/~10 500、`ui` 27/~6 500、`api` 13/~3 400、`live2d` 13/~2 800、`app` 12/~3 200、`design` 7/~1 700、`chat` 5/~1 700、`audio` 6/~1 200、`data` 6/~820、`voice` 4/~730、`state` 3/~550、`main.dart` 单独一批）；`test` 122 文件**只进「测试质量」批次**（专抓假绿灯）。

### Phase 2 · 维度横扫
A→J 每条一个批次，**全仓一条轴**（用 `git grep` 拉出候选，再回源码确认）。

### Phase 3 · 端到端链路深潜（一条链路一个批次，跨 10–25 文件，看**接缝**）
1. 发送 → LLM 流 → 句子装配 → TTS → 音频 → 口型 → 舞台；
2. 外部输入 → `POST /api/v1/external/chat` → `say`；
3. 改设置 → 偏好 → 落盘 → 热重载 → 生效（含 Mod `apply_settings`）；
4. 错误：后端 `tracing` → WS `error` 帧 → 横幅 → 日志可搜（**两处 code 是否同一个字符串**）；
5. 导演 cue → `action_cue` → preset 下发 → ack → 可观测面板；
6. Mod 启用 → config → `apply_settings` → 生效（含 secret 不被抹掉、未知 id 不丢）；
7. 模型切换 → `sendSync(model:)` → `loaded` 回执 → 缩放 / 幅度；
8. **离线**：断网条件下字体回落 / canvaskit / 外部请求计数（只读取证或源码静态判定）。

### Phase 3.5 · 清洗后遗痕轴（**本轮新增，最便宜的高价值轴**）
2026-10-06 做过 `git filter-repo`：**全部旧 commit hash 已变**。逐条机械检查：

- `git grep -nE '\b[0-9a-f]{7,40}\b' -- '*.md' 'docs/**' 'scripts/**'`：文档 / 脚本里的短 SHA 是否**已断链**（`git cat-file -t <sha>` 是否 fatal）；注释里的旧 SHA 不致命，**操作指令里的旧 SHA 是 P2/P1**；
- `git grep -n 'deploy_android'`：还有没有**操作型文档**教人运行已删除的 `scripts/deploy_android.sh`（历史叙事 / 台账里的出现不算）；
- 「文档 vs 树」批量对账：AGENTS / README / `docs/architecture/**` 里的**版本号、Mod 集合、门禁命令、文件路径**与树是否一致（例：`-fe/AGENTS.md` 首屏写 0.2.0，实际 0.2.1-rc.1；`archive/action-layer-p6` 被 10+ 处引用但**本地不存在该分支**）；
- `.gitignore` 覆盖检查：本机运行态（`mods.json`、`memory.jsonl`、日志、选中模型、备份 bundle）是否都被忽略；有没有**该忽略却没忽略**的（会导致下一次清洗）/ **该跟踪却被忽略**的（新机器拉下来就跑不起来）；
- `scripts/check_public_secrets.py` 的**模式可否被绕过**（新增的「文字口令 / 纯数字口令 / 中文『密码:』」三类模式的边界；它扫的是跟踪文件还是工作树；二进制 / 未跟踪文件是否在扫描面内）。

### Phase 4 · 对抗日（证伪）
- 回到每个**未关闭的 P0/P1**，从源码重新推导（**不看自己的笔记**），尝试构造反例；
- 重放本项目历史 bug 的四类：① 异步时序 ② 浮层构建时机 ③ 平台视图/指针 ④ 精确路径匹配；
- **假绿灯狩猎**：空转断言、源码字符串扫描式测试、`expect(常量,常量)`、恒真守卫、删掉实现仍绿的守卫。

### Phase 5 · 增量
`git log --oneline 6be9984..HEAD`，只审新增 / 改动文件，但要看它**与既有接缝的交互**；审完回 Phase 1。

---

## 11. 纪律（任何模型都适用，别跳）

1. **一次只做一批**，不要同时读两批的文件。
2. **引用优先**：先摘原文再下结论。摘不出来 → `STATE.md` 候选池，**不进 FINDINGS**。
3. **不要长推理**：不写「让我想想」「可能大概是」。要么 `file:line` + 摘录，要么标未核实。
4. **不要发明路径 / 符号名**：路径来自 `git ls-files`，符号名来自 `read` 的真实返回。
5. **不要用 markdown 表格写发现**（会写坏）——只用 §5 的键值块。
6. **上下文将满就关批**：把要点写进 `BATCH-1NNN.md`，下一批继续，不要硬撑。
7. **每个文件读完立刻在 `WIP.md` 记一行**（文件 → 有无发现），防止「读了一半忘了读了哪些」。
8. **不确定是否已登记** → 用 `git grep` 在既有账本里搜 ID / 关键词，搜过再决定。
9. **1M 上下文不是许可证**：本任务唯一的稀缺资源是「落盘的、可复核的结论」。

---

## 12. 质量纪律

### 12.1 每条 P0/P1 必须做到四件套
1. **完整调用路径**（定义 → 调用点 → 受影响路径，`file:line` 串起来）；
2. **去测试里找覆盖该路径的测试并读它**：它存在但**不可能失败**，那本身是一条新发现；
3. **反证**：找过哪些地方试图推翻它、结果如何；
4. **可执行的验证步骤**（本机只读可复现，或明确指出缺什么前提）。**本任务禁跑 cargo/flutter 门禁**，所以「需要编译才能确认」的结论必须如实标**「需门禁验证」**，不许伪装成已实测。

### 12.2 区分事实与推断
「我读到的」写进正文；「我推断的」标**未核实**且置信 ≤ 中。

### 12.3 每 6 批一次 `CONSOLIDATION-NN.md`（对账，不是长文）
① 未关闭 P0/P1 回源码重读复核（仍成立 / 已失效 / 需改写）；② 覆盖率（INDEX 已审 ÷ 队列总数 + 未审目录）；③ 同根因归并（标主发现与派生）；④ **跨批系统模式**（最有价值的产出）；⑤ 质量自评（几条硬、几条猜，诚实写）。

### 12.4 优先级
一批里有 P0 → **先写完整它**，再管 P2/P3；没把握的进候选池，不塞 FINDINGS 充数。

---

## 13. 夜间纪律

- 不要解释你将如何做，**直接做**。
- 对话里**每批最多一行**：`BATCH-1NNN · <主题> · 新增 P0/N P1/N P2/N`。所有实质内容进 `AUDIT-REPO/`。
- 工具失败 → 更小范围重试一次 → 仍失败记 `STATE.md` 后换批。**禁止空转等待。**
- 只写 `AUDIT-REPO/`；**永不 `git add`、永不改业务代码**。
- 生成物、`target/`、`build/`、`dist/`、lock、覆盖率报告不审。
- 每批更新 `STATE.md` 的 `last_heartbeat`（人类靠它判断进程还活着）。

---

## 14. 现在执行

```bash
# 1) 确认在 -fe 且在 main（若不在，写 STATE.md 未核实区后继续，不要切分支）
cd /home/skystar/Live2D-Ai-fe
git log -1 --format='%h %ci %s'; git branch --show-current; git status --porcelain | head

# 2) 若无 AUDIT-REPO/：初始化骨架（§2）+ 生成队列（§3.3）
ls AUDIT-REPO/ 2>/dev/null || { mkdir -p AUDIT-REPO; }

# 3) NEXT.md 有内容 → 从它继续；没有 → 从 **Phase 0（前端设置 + Mod 面，§10）** 开始：
#     先跑 §7.1 的「设置项三方对账」，再开 P0-1（settings/display_prefs.dart）
#     Phase 0 的 9 个批次做完后，Phase 1 从 crates/live2d-ai-runtime/** 起（Rust 主线）
```

4) 然后**不要停**。

