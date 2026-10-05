# 独立复核报告 · public 仓库历史清洗（2026-10-06）

- **复核者**：ci-gates（本轮角色 = 独立验证者，非清洗执行者）
- **工作树**：`/home/skystar/Live2D-Ai-fe`（`git worktree`，common dir = `/home/skystar/Live2D-Ai/.git`）
- **约束遵守**：全程只读 + 只写本文件；**没有** add / commit / push / checkout / reset / stash / rebase；克隆与实验全部在 `/tmp`；**没有**跑 flutter / cargo。
- **清洗前备份**：`/home/skystar/backup-2026-10-06/pre-purge-all-refs.bundle`（132,448,444 B）+ `refs-before.txt`（104 refs）

> 本报告**不含**那 5 位设备锁屏口令的值（下称「**needle A**」）。needle A 仅在我方脚本内以变量形式从备份的
> `scripts/deploy_android.sh` 里提取（全文件	extbf{唯一}一个 5 位数字串），落盘于 `/tmp/pv-pin.txt`（0600），
> 复核结束即删。第二个 needle 写作 `192.168.0.x`（下称「**needle B**」）。

---

## 0. 结论摘要

| # | 结论 | 判定 |
|---|---|---|
| 1 | **本地重写后的历史本体是干净的**：以 7518 个 blob 为全集逐一证明，除「删文件」与「两个字面量替换」外**零差异**（0 个无法解释的差异、0 个新增 blob） | ✅ 已证 |
| 2 | **GitHub 远端仍可取到清洗前的对象**：`refs/pull/1/head` 仍指向清洗前提交，且**按旧 SHA 直接 fetch 也能成功**（2/2）。needle A 因此**此刻仍可从公网下载** | ❌ **未清除** |
| 3 | 提交消息里有 **30 条**记录了旧 commit 短 SHA，重写后被同步更新（第四类差异；性质无害，但**不在**声明的三类之内） | ⚠️ 需知悉 |
| 4 | **当前 HEAD 里仍有两处 needle B 字面量**（文档描述脱敏操作时写回了原名），其中一处**已经在远端 main 上** | ⚠️ 需处置 |
| 5 | 结构：提交数 574 → 573，恰好剪掉 1 个「只改该脚本」的提交；main 188 → 188；树哈希 `main` 新旧同为 `c5f82d3d…`（与 Lead 自查一致） | ✅ 已证 |
| 6 | 反向全 blob 凭据扫描：只有 `sk-` 一类命中 63 处 / 8 个唯一值，**全部是测试夹具或占位符**；`ghp_` / `github_pat_` / `AKIA` / `PRIVATE KEY` **零命中** | ✅ 判定见 §5 |

**一句话**：**历史清洗本身执行正确且有完备证明，但它还没"清干净"——清洗前的对象仍挂在 GitHub 上（PR ref + 按 SHA 直取），5 位口令仍可公开下载。**

---

## 1. 方法与快照时间线（含一次**观察窗口被移动**的如实记录）

| 时刻 | 事件 | 来源 |
|---|---|---|
| 21:45 | 备份 bundle + refs-before.txt 生成 | 文件 mtime |
| ~21:47 | 我克隆 OLD（bundle）与 NEW（工作树镜像）快照 | 本报告 §2–§5 的基准 |
| ~21:52 | 我第一次 `ls-remote`：远端 main = `2d492447…`（**清洗前**） | 当时输出 |
| 21:53:10 | Lead 提交 `11ded6a`（release v0.2.1-rc.1） | `git reflog` |
| **21:54:08** | **Lead 推送到 GitHub**（reflog: `refs/remotes/origin/main: update by push`） | `git reflog` |
| 21:55:27 | Lead 提交 `15271e1`（清洗记录，尚未推送） | `git reflog` |
| 21:59+ | 我发现时间线移动，**重跑**远端复核（本报告 §6 用的是重跑结果） | 本报告 |

**如实说明**：我第一次的远端观察（"远端未清洗"）在几分钟后被 Lead 的推送推翻；本报告采用的是 **21:59 之后的重跑结果**。
如果我没有回头核对 reflog，就会把一个**已经过期 2 分钟**的结论写进报告——这正是"独立复核"要防的伪结论。

命令与输出（快照基准）：

```console
$ git clone -q --mirror /home/skystar/backup-2026-10-06/pre-purge-all-refs.bundle /tmp/pv-old.git
$ git clone -q --mirror /home/skystar/Live2D-Ai-fe /tmp/pv-new.git
$ git -C /tmp/pv-old.git bundle verify /home/skystar/backup-2026-10-06/pre-purge-all-refs.bundle
...
The bundle records a complete history.
The bundle uses this hash algorithm: sha1
$ # ref 数
old refs: 104  new refs: 102  refs-before.txt: 104
```

---

## 2. 逐 ref 对比

### 2.1 ref 名称对齐

```console
$ awk '{print $2}' refs-before.txt | sort > /tmp/names.before ; git -C /tmp/pv-new.git show-ref | awk '{print $2}' | sort > /tmp/names.new
$ diff /tmp/names.before /tmp/names.new
10a11
> refs/heads/mainline/1-core-baseline
11a13
> refs/heads/pr-1
13,16d14
< refs/remotes/origin/HEAD
< refs/remotes/origin/main
< refs/remotes/origin/mainline/1-core-baseline
< refs/remotes/origin/pr-1
```

⇒ 4 个 `refs/remotes/origin/*`（含 `HEAD` 符号引用）在重写后的本地仓库里消失；同时多出 2 个本地分支
`refs/heads/mainline/1-core-baseline`、`refs/heads/pr-1`（内容对应旧的 `origin/mainline/1-core-baseline`、`origin/pr-1`）。
**这不是内容损伤，是 ref 命名空间变化**，但属于"声明之外的变化"，记录在此。

### 2.2 逐 ref 树哈希

```console
common refs: 100 | identical tree: 4 | different tree: 96
identical: ['refs/heads/android-archive', 'refs/heads/main', 'refs/tags/checkpoint/gaps-round-2026-10-06', 'refs/tags/pre-msg-fix-2026-10-06']
old heads/main  = d03347b1a9d4452a9c83426e3f082733f4250421 tree=c5f82d3d0e8ed9e2b8289e42c2358f4f902cd47b
new heads/main  = e497edc7ec412cb51ecbc26df12f02a07dbb8764 tree=c5f82d3d0e8ed9e2b8289e42c2358f4f902cd47b   ← 与 Lead 自查一致
```

（`refs/remotes/origin/main` 是**另一个**提交 `2d492447`/tree `9ed75c6a…`，它在重写后的仓库里没有同名对应物：
remote-tracking ref 被删了，**不能**拿它当"旧 main"与 `refs/heads/main` 比对——我第一次就踩了这个坑，已修正。）

### 2.3 每个差异 ref 的路径级 diff —— 只允许三类

对 100 个同名 ref + 2 个改名 ref 逐对跑 `git diff --raw --no-renames`：

```console
ref pairs compared: 102 | refs with tree changes: 98 | refs with identical tree: 4
raw diff entries: 303                   ← 这一行原稿打错（脚本把 diff 的 mode 列当成 status 打印）
                                           分类逻辑用的是正确的 old/new blob 列，结果见下面三项
ADDED paths: (none)
DELETED paths: {'scripts/deploy_android.sh': 97}
unique modified blob pairs: 9 | modified path instances: 206
modified paths: {'README.md': 4, 'docs/legacy/plans/plan-verify-e2e.md': 1, 'docs/plan-verify-e2e.md': 1,
                 'docs/plans/plan-verify-e2e.md': 1, 'scripts/adb_ui_tap.py': 2}
modified pairs containing needle A in OLD blob: 0 | containing 192.168.0.x: 9
FOURTH-CLASS VIOLATIONS: 0
```

⇒ 树差异 = **只有**：① 删掉 `scripts/deploy_android.sh`（97 个 ref 差异里出现，无例外）；
② 9 个 blob 的 `192.168.0.x → 192.168.0.x`；**无任何新增路径**；**0 个第四类**。

> 每对修改 blob 都是逐字节验证的：`new == old.replace(needleA, <REDACTED…>).replace("192.168.0.x","192.168.0.x")`，
> 不是肉眼看 diff。needle A 在"被修改的 blob"里 0 命中 ⇒ 它只存在于被删除的那个文件里（与 §5 的全量扫描一致）。

---

## 3. 全历史 needle 扫描（清洗后的 NEW）

```console
$ git -C /tmp/pv-new.git rev-list --all            # 575 行（当前工作树）/ 573（快照时）
commits scanned (rev-list --all): 573
  git grep -I -F '<needle A>'  over all 573 commits -> hits: 0
  git log -S '<needle A>'                            -> commits: 0
  git log --grep '<needle A>'                        -> hits: 0
  git grep -I -F '192.168.0.x' over all 573 commits -> hits: 0
  git log -S '192.168.0.x'                          -> commits: 0
  git log --all -- scripts/deploy_android.sh          -> ''            （空）
  rev-list --all --objects | grep -c deploy_android   -> 0
$ # 替换确实落地
  git grep -I -F '<REDACTED-DEVICE-PIN-2026-10-06>'  -> 57 lines
  git grep -I -F '192.168.0.x'                       -> 1800 lines
```

**当前工作树（比快照多 2 个提交，main=`15271e1`，`--all` 575 个提交）重跑**：

```console
PIN hits over all commits: rc=1 lines=0          # rc=1 = 无命中
IP  hits over all commits: rc=0 lines=3
    15271e1… -> docs/audit/2026-10-06-purge/PURGE-RECORD.md:#   192.168.0.x      ==> 192.168.0.x
    15271e1… -> docs/releases/v0.2.1-rc.1.md:   （5 位数字口令 / \`192.168.0.x\`）替换为脱敏值。
    11ded6a… -> docs/releases/v0.2.1-rc.1.md:   （5 位数字口令 / \`192.168.0.x\`）替换为脱敏值。
deploy_android objects in current history: 0
```

⇒ needle A：**全历史 0 命中**（含消息）。needle B：重写后的**历史 blob** 里 0 命中，但**当前树两处文档**把它写了回来，
其中 `docs/releases/v0.2.1-rc.1.md` **已经在远端 main（`11ded6a`）上**。见 §7.2。

---

## 4. 结构与"消失的提交"

```console
OLD rev-list --all --count over original refs: 574
NEW rev-list --all --count over original refs: 573
OLD main commits: 188 | NEW main commits: 188
refs whose commit count changed: 72          （全部 -1）
  refs/heads/archive/action-trigger-p5 357 -> 356
  refs/heads/dev/integrity            301 -> 300
  ...
distinct (ct subject) lines present in OLD refs but absent in NEW: 3
  x72 1786941440 chore: Android 部署脚本支持 PHONE 环境变量覆盖设备串号      ← 真·被剪掉的提交
  x23 1789302141 docs: 记录 rc.2 已推送（main=6ab4a074 / tag v0.1.0-rc.2）+ …
  x2  1789108788 docs(handoff): 交接文档对齐提交后的状态（HEAD 9716f7b3，工作区干净）
$ git show --stat --format= --name-only 0158772cfcbe71111ddf1bc08d14786c846e4166
scripts/deploy_android.sh
$ git log --all --format=%H --grep='Android 部署脚本支持 PHONE' | wc -l   # 新历史
0
```

⇒ 574 → 573 的差额 = **恰好 1 个提交**：`0158772c`「chore: Android 部署脚本支持 PHONE 环境变量覆盖设备串号」，
它**只**动 `scripts/deploy_android.sh` ⇒ 文件被整份移除后该提交变空，被 `git filter-repo` 默认剪掉
（72 个 ref 的 `-1` 就是它）。这与"预期恰好是那个只删该文件的提交"一致。
另外 2 条"缺失行"不是被剪，而是**消息被改写**（见 §7.1）。

---

## 5. 反向检查：全 blob 流式扫描（不是只看 needle）

### 5.1 全 blob 全集证明（7518 个 blob 逐一）

```console
OLD blobs: 7518  total: 280.2 MB      NEW blobs: 7510  total: 280.2 MB
=== PASS A: every OLD blob must survive unchanged OR transform exactly ===
OLD blobs containing needle A: 8 | containing 192.168.0.x: 42 | both: 7
survived unchanged in NEW: 7475
transformed (needle-bearing) and present in NEW: 37
needle-bearing but transformed blob MISSING in NEW: 6
    71846bb70ae9 scripts/deploy_android.sh 5509 bytes
    d7afd615d001 scripts/deploy_android.sh 4989 bytes
    69f2b9b26430 scripts/deploy_android.sh 4990 bytes
    1576f86adfab scripts/deploy_android.sh 4980 bytes
    718bd0bc973f scripts/deploy_android.sh 3708 bytes
    4a5e4b23f532 scripts/deploy_android.sh 3704 bytes
all missing are the deleted path: True
UNEXPLAINED missing old blobs (4th class): 0 []
=== PASS A2: any NEW blob not explained by OLD? ===
NEW blobs not derivable from OLD: 0 []
```

⇒ 这是本报告最强的一条：**历史里每一个 blob 要么逐字节不变、要么恰好等于替换后的结果**；唯一"消失"的 6 个 blob
全部是那个被删文件的 6 个版本；**没有新内容被引入**。

### 5.2 凭据模式逐条判定（8 个唯一值，全量）

```console
grep -E 'sk-[A-Za-z0-9_-]{20,}'  → OLD 63 命中 / NEW 63 命中（同一批）
ghp_ / github_pat_ / AKIA / BEGIN … PRIVATE KEY → OLD 0 / NEW 0
NEW blobs containing needle A: 0  occurrences: 0        （OLD: 8 blobs / 15 处）
```

| 唯一值（**刻意打码**：前缀 + 长度） | 次数 | 所在文件 | 我的判定 |
|---|---|---|---|
| `sk-LIV…`（33，含 `LIVE2D_AI`+`INJECTED`） | 31 | `crates/live2d-ai-desktop/src/web_api/{app_routes,dto}.rs`、`docs/**/node-d-api-contract-*.md` | **测试夹具**（自带 INJECTED 标记） |
| `sk-use…`（29，含 `USER-CUSTOM`） | 12 | `Live2D-Ai-Android/**/LLMProviderManagerTest.kt`（归档 Android 测试） | **测试夹具** |
| `sk-use…`（26，含 `USER-CUSTOM`） | 6 | 同上 | **测试夹具** |
| `sk-tes…`（26，含 `TEST`+`DO-NOT-LEAK`） | 4 | `crates/live2d-ai-runtime/tests/unified_client.rs` | **测试夹具**（名字就是"别泄漏"） |
| `sk-you…`（29，含 `YOUR`） | 3 | `.env.example` | **占位符** |
| `sk-ds-…`（27） | 3 | `tests/test_pc_zero_config.py` | **测试夹具** |
| `sk-liv…`（24，形态 `AA-AAAA-AAAAA-AAAAAA-###`） | 2 | `crates/live2d-ai-runtime/src/secret.rs`（`const SECRET: &str = "…"`） | **测试常量**（结构是词-词-词-词-数字，非真实密钥形态） |
| `sk-fak…`（34，含 `FAKE`） | 2 | `tests/test_pc_preview_build.py` | **假值** |

⇒ 结论：**没有真凭据**。真实密钥（DeepSeek 等）是 `sk-` + 32 位无分隔随机串；上表 8 个值全部带
TEST/FAKE/EXAMPLE/YOUR/DO-NOT-LEAK/USER-CUSTOM 语义或明显人造结构。**唯一真凭据是 needle A（设备锁屏口令）**，
它在重写后的历史里 0 命中（但在远端仍可取，见 §6）。

---

## 6. 远端残留（⚠️ 本报告最重要的一节）

### 6.1 重写后的 ref 确实已经推上去了

```console
$ git ls-remote https://github.com/yuhanwanli99-cpu/Live2D-Ai.git      # 21:59 重跑
11ded6a1374ec67d6daba50ec3329717bff6fec8  HEAD
11ded6a1374ec67d6daba50ec3329717bff6fec8  refs/heads/main
b58b2231ed0943a776bf12c374715fb342063f84  refs/heads/mainline/1-core-baseline
44da2a4c6935658965fe11e74c9836579511cddc  refs/pull/1/head          ← 清洗前！
8dab84a9ad14… / aa964dba…                 refs/tags/v0.1.0-rc.2(+ peeled)   ← 已重写
bf8f16d0b83b… / b36924c5…                 refs/tags/v0.2.0-rc.1(+ peeled)   ← 已重写
$ # 与本地重写后 ref 对比
refs/heads/main                     local=15271e1ab5a8 remote=11ded6a1374e DIFFER（本地领先 1 个提交，未推送）
refs/heads/mainline/1-core-baseline local=b58b2231ed09 remote=b58b2231ed09 MATCH
refs/tags/v0.1.0-rc.2               local=f2ec0e145043 remote=f2ec0e145043 MATCH
refs/tags/v0.2.0-rc.1               local=bf8f16d0b83b remote=bf8f16d0b83b MATCH
remote total refs: 19 | refs/pull/*: 1
```

⇒ main / 分支 / 被推的 tag **都是重写后的哈希**（清洗确实发布了）；远端 main 比本地少 1 个提交（`15271e1` 未推）。

### 6.2 但清洗前的对象**仍可从公网下载**（两条独立路径，均已实测）

**路径 ①：`refs/pull/1/head` 仍指向清洗前提交**

```console
$ git -C /tmp/pv-probe fetch --no-tags <URL> refs/pull/1/head
 * branch            refs/pull/1/head -> FETCH_HEAD          （rc=0）
$ git -C /tmp/pv-probe rev-parse FETCH_HEAD
44da2a4c6935658965fe11e74c9836579511cddc                     ← 清洗前
$ git -C /tmp/pv-probe ls-tree -r FETCH_HEAD -- scripts/deploy_android.sh
100644 blob 71846bb70ae9c050f2a4d04bcdcbd8af1534fff5	scripts/deploy_android.sh
$ git -C /tmp/pv-probe show FETCH_HEAD:scripts/deploy_android.sh | wc -c
4489          # needle A 在这个 4489 字节的文件里出现 2 次
```

**路径 ②：按旧 SHA 直接 fetch（GitHub 仍服务已不被引用的对象）**

```console
$ git fetch --no-tags <URL> 2d4924473ecbb19eb4555fbe9a4e102b5074126d     # 旧 main，已不在任何 ref 上
 * branch            2d4924473…  -> FETCH_HEAD              （rc=0）
$ git fetch --no-tags <URL> 6ab4a07441eee52386790db1380e19606a851d56     # 旧 tag v0.1.0-rc.2 的目标
 * branch            6ab4a0744…  -> FETCH_HEAD              （rc=0）
```

这两条覆盖了**所有**远端 ref 的对象：`refs/heads/main`、`mainline/1-core-baseline`、`refs/pull/1/head` 与 6 个 tag
（含 peeled）在清洗前的树里**全部**含 `scripts/deploy_android.sh`：

```console
  HEAD                                2d4924473ecb tree=9ed75c6a9c0c  deploy_android.sh=PRESENT
  refs/heads/main                     2d4924473ecb tree=9ed75c6a9c0c  deploy_android.sh=PRESENT
  refs/heads/mainline/1-core-baseline 65e62115c126 tree=4b59aca1e4f7  deploy_android.sh=PRESENT
  refs/pull/1/head                    44da2a4c6935 tree=4b59aca1e4f7  deploy_android.sh=PRESENT
  refs/tags/v0.1.0-rc.1 … v0.2.0-rc.1（6 个 tag + 6 个 peeled）        …… 全部 PRESENT
```

**为什么"我们改不了"**：
- `refs/pull/*` 是 GitHub **服务端只读**的命名空间，`git push` 写不进去（既不能指向新对象，也不能删除）；
  重写历史后它依旧指向旧提交。
- 旧对象仍可**按 SHA 拉取**：重写只让它们"不可达"，**不等于删除**；GitHub 侧何时 gc、是否保留，是平台行为。
- 真要清干净只有三条路（均需 Lead 决策）：① 找 GitHub Support 对仓库执行 gc / 清除不可达对象；
  ② **删库重建**（代价：issue/PR/star/URL 全丢，且 fork 仍留旧对象）；③ 把仓库设为私有（不等于删除，但公开面消失）。
  —— 本轮我**没有**做任何推送或远端写操作。

---

## 7. 声明之外的差异（第四类）清单

### 7.1 提交消息里的"自引用短 SHA"被同步更新（30 条；无害，但属第四类）

```console
records only in OLD: 31 | records only in NEW: 30      （按 (author-time, subject, body) 多重集比较）
fields that differ across 28 matched records: {'body': 28}
--- sample: at=1785833256 subject='feat: Live2D model-switching system (Android + PC)…'
    -Second half of the SPOQ-v7-verification epic (first half committed as 6f87069…)
    +Second half of the SPOQ-v7-verification epic (first half committed as fe39817…)
--- sample（subject 改写，2 条）
    OLD bfdd1420 1789302141 docs: 记录 rc.2 已推送（main=6ab4a074 / tag v0.1.0-rc.2）+ …
    NEW e7230d18 1789302141 docs: 记录 rc.2 已推送（main=aa964dba / tag v0.1.0-rc.2）+ …
=== 这 30 条差异是不是只改了 hash？ ===
removed hex tokens that exist as OLD objects: 49 | not found: []
added   hex tokens that exist as NEW objects: 49 | not found: []
pairs whose messages differ in anything OTHER than hex tokens: 0
```

⇒ 把两边的十六进制 token 全部替换成占位符后，**消息逐字节相同**；49 个被移除的 token **都是 OLD 里真实存在的对象**，
49 个新增 token **都是 NEW 里真实存在的对象**。所以这 30 条差异 = **旧短 SHA → 其重写对应物的替换**，
不是内容编辑。它**不在**声明的三类里（声明只管文件内容：删脚本 / needle A / needle B），因此如实列为第四类，
但性质是重写的必然副产品，**不构成内容损伤**。

### 7.2 当前 HEAD 里 needle B 仍有两处（其中一处已在远端）

```console
$ git grep -I -F -l -e 192.168.0.x HEAD
HEAD:docs/audit/2026-10-06-purge/PURGE-RECORD.md
HEAD:docs/releases/v0.2.1-rc.1.md
$ git grep -I -F -l -e 192.168.0.x 11ded6a1374ec67d6daba50ec3329717bff6fec8   # 远端 main
11ded6a…:docs/releases/v0.2.1-rc.1.md
$ git grep -I -F -l -e <needle A> HEAD  → （空）   # needle A 当前树 0 命中
```

⇒ 替换只在**历史 blob** 上生效；**当前树**里描述这次脱敏的两份文档又把 needle B 写回来了。
若"当前 HEAD 不再出现 needle B"也是目标，需要单独改这两份文档（本报告自身也在 §6 里引用了该字面量，
**应视为新的第 3 处**——建议 Lead 决定是否把本报告也脱敏或 gitignore）。

### 7.3 ref 命名空间变化
见 §2.1（4 个 `refs/remotes/origin/*` 消失、2 个本地分支出现）。内容无损伤。

---

## 8. 本地残留（未发布，但影响"本机是否还留着旧对象"）

- **两个 worktree 共用一个对象库**：`/home/skystar/Live2D-Ai-fe/.git` 是文本文件 →
  `gitdir: /home/skystar/Live2D-Ai/.git/worktrees/Live2D-Ai-fe`；common dir = `/home/skystar/Live2D-Ai/.git`。
- 旧脚本 blob `71846bb7…` **仍在该共享对象库里**（`cat-file -e` 通过），但**在 `-fe` 的 refs 下不可达**
  （`rev-list --all --objects | grep 71846bb7` = **0**）。
- 它的锚点是**另一个 worktree 的索引**：`git -C /home/skystar/Live2D-Ai ls-files --stage -- scripts/deploy_android.sh`
  → `100644 71846bb70ae9… 0	scripts/deploy_android.sh`；且该文件**在该 worktree 的工作区里真实存在**。
- 全盘"独立 token"扫描（排除 `.git`/`target`/`build`）：

  ```console
  [Live2D-Ai-fe（会推送的那个 worktree）] standalone-token hits: 0 files
  [Live2D-Ai（另一个 worktree）]            standalone-token hits: 2 files
       scripts/deploy_android.sh                      count=2   ← 真·残留（工作区未跟踪文件）
       .venv/…/dashscope/…/qwen.tiktoken              count=1   ← 分词表里的数字串巧合
  ```

  （先前 `grep -F` 报"25 个文件含口令"，全部落在 `target/**` 的 cargo fingerprint JSON 里；取样证明是
  **长数字串里的巧合子串**（如 `674768008219…00`），不是凭据。**这条我自我更正**：用 word-boundary 正则后 `-fe` 里是 0。）

⇒ 建议（非我执行）：确认 `/home/skystar/Live2D-Ai` 这个 worktree 的去留；若保留，至少清掉其索引/工作区的该文件，
并在清理后对共享对象库跑一次 `git gc --prune=now`（**只能在没人正在构建/提交时做**，且会动到 Lead 的仓库，我不动）。

---

## 9. 未能验证（如实列出 + 原因）

1. **GitHub 侧最终保留策略**：旧对象"何时 gc、是否永久保留"是平台行为，本地无法证明。
   我能证明的是**此刻**它们仍可被 fetch（§6.2 两条路径，均已实测 rc=0）。
2. **fork / 镜像 / 缓存**：GitHub 上的 fork 是否仍持有旧对象、第三方缓存（如 GH Archive、搜索引擎快照）是否留档，
   **未能验证**（无 API token，且属平台外部）。
3. **本机其它副本**：我只检查了备份 bundle、两个 worktree 与共享对象库；**没有**穷举整机（如 `~/backup-*` 之外的
   介质、其它克隆）。备份 bundle 按定义**包含** needle A（这是备份的意义）。
4. **真 runner / CI**：与本任务无关，未涉及。
5. 我在 §1 记录的"第一次远端观察"当时**未**加时间戳证据（事后由 reflog 还原），现在报告已用重跑结果覆盖。

---

## 10. 复现入口（全部只读）

| 目的 | 命令（在原工作树之外执行） |
|---|---|
| 克隆清洗前/后 | `git clone -q --mirror <bundle> /tmp/pv-old.git` ; `git clone -q --mirror /home/skystar/Live2D-Ai-fe /tmp/pv-new.git` |
| 逐 ref 树对比 + 路径级 diff | `/tmp/pv_analyze2.py`（本报告 §2 数字来源；把 `OLD/NEW` 换成本地镜像即可复跑） |
| 全 blob 证明 + 凭据扫描 | `/tmp/pv_blobs.py`（§5；7518 blob 流式扫描约 17 秒） |
| 消息/标签对比 | `/tmp/pv_meta.py`、`/tmp/pv_meta2.py`、`/tmp/pv_final.py`（§4/§7） |
| 远端残留 | `git ls-remote <URL>` ; `git fetch --no-tags <URL> refs/pull/1/head` ; `git fetch --no-tags <URL> 2d4924473…` |
| 当前态复核 | `git -C /home/skystar/Live2D-Ai-fe grep -I -F -c -e <needleA> $(git rev-list --all)` 等（§3、§7.2） |

（`/tmp/pv-*.py` 与 `/tmp/pv-pin.txt` 是复核过程的临时物：**needle A 的临时文件在报告落盘后删除**；
脚本内不再含该值，重跑时自行从备份提取。）

---

*报告完。*
