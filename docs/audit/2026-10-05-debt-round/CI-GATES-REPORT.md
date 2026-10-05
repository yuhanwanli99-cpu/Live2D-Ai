# CI 三条红线门禁报告（W1-B · task-2）

- **task**：task-2（共享任务板）· 状态：本报告落盘时为 `in_progress`，随报告提交后置 `completed`
- **工作树**：`/home/skystar/Live2D-Ai-fe`（分支 `chore/debt-round-2026-10-05`）· 未做任何 git 提交/破坏性 git
- **写作用域**：`.github/workflows/**`、`scripts/ignite.sh`、`scripts/check_public_secrets.py`、`docs/audit/2026-10-05-debt-round/**`
- **对应审计条目**：F-0048-01（P2）、F-0049-01（P1）、F-0050-01（P2）
- **本机环境**：Flutter 3.47.3 stable（`~/flutter`）、python3 + PyYAML 6.0.3、真二进制 `target/debug/live2d-ai-desktop`（已存在，只执行未重建）

---

## 0. 证据边界（先说清楚，免得被当成伪造绿灯）

1. **本机没有 CI runner**：所有 workflow **一次都没在真 runner 上跑过**。本报告里凡标「CI」的都是
   **YAML 结构校验（`yaml.safe_load` 全部 6 个文件通过）+ 本地等价命令实跑**，不是 GitHub Actions 的运行结果。
   没有 `actionlint` 也没有 `shellcheck`（本机未安装），YAML 只做了 Python 解析级校验。
2. **共享产物目录 `shell/flutter/build/web` 在 18:28:35 被一次未完成的构建清空**（Lead 已按「未归因的工作区竞争」记录）。
   我的全部构建与变红实验都在 **/tmp 副本**里做，不碰共享目录；共享目录我只**读**（`--check-dir` / HTTP）。
3. **真服务的两个实例是我自己起的**（18098 指向坏产物副本、18099 指向好产物副本），只为取红/绿证据；
   报告落盘后已关。**没有**动 18080（Lead/browser-qa 的验收实例）与任何 5 个已存在的服务。
4. `main.dart.js` 里恒有 `fonts.gstatic.com` 的运行期字体兜底（CanvasKit 引擎行为）——**不是**本门禁的判据，
   命中**不判红**；运行期零外部请求由 browser-qa 的真实浏览器 Network 断言负责（Lead 2026-10-05 已确认）。

---

## 1. 三条门禁总览

| # | 门禁名 | 判红条件（生效判据） | CI 位置（workflow → job → step） | 本地等价命令 | 变红自证 |
|---|---|---|---|---|---|
| G1 | **产物离线门禁（构建期）** | 产物文件缺失；或 `index.html`/`main.dart.js` 含 `gstatic.com/flutter-canvaskit`；或 `flutter_bootstrap.js` 生效的 `canvasKitBaseUrl` 指远端；或无本地 baseUrl 且无 `"useLocalCanvasKit":true` | `flutter-checks.yml` → `flutter-web-offline-artifacts` → *Offline artifact gate* | `./scripts/ignite.sh --check-dir shell/flutter/build/web` | §2.4 RED1–RED4（exit=1） |
| G2 | **真服务离线体检（托管期）** | 同上（走 HTTP 实吐字节）+ `GET /` 非 302→/app/ + `GET /app/` 非 200 | `nightly.yml` → `web-offline-serve-check` → *Ignition check* | `./scripts/ignite.sh --check --port <N>` | §2.4 HTTP-RED（exit=1） |
| G3 | **密钥不进仓库扫描** | 4 类模式（`sk-…` / `ghp_…` / `AKIA…` / `PRIVATE KEY`）在 tracked tree 任意受扫文件中命中 | `secret-scan.yml` → `public-secret-scan` → *Scan tracked tree…* | `python3 scripts/check_public_secrets.py` | §4.3（exit=1，含 Dart/docs 新扫描面） |

三个门禁都先过 **YAML 解析**与**本地等价实跑**；G1/G2 的探针清单是**唯一真源**（`scripts/ignite.sh` 的
`CDN_PROBE_SPEC`，`--check` 与 `--check-dir` 共用同一份判定函数 `cdn_judge`）。

---

## 2. F-0050-01 · `ignite.sh --check` 探针文件选错

### 2.1 回源码核实的现状

`scripts/ignite.sh`（改前 :89/:94）只探两个文件、按裸串判红：

```bash
for f in index.html main.dart.js; do                                   # :89
  ... curl 取 code，非 200 判红 ...
  elif curl ... "$base/app/$f" | grep -q 'gstatic\.com/flutter-canvaskit'; then   # :94
```

对**当前正确构建**逐文件实测（门禁一模一样的模式）：

| 文件 | 命中 | 是否被旧门禁探 |
|---|---|---|
| `index.html` | 0 | ✅ 探 |
| `main.dart.js` | 0 | ✅ 探 |
| `flutter_bootstrap.js` | 1 | ❌ **不探（活配置在此）** |
| `flutter.js` | 1 | ❌ 不探 |

### 2.2 A/B 实测（同一份源码，只差 `--no-web-resources-cdn`）

两份构建都是我本机在 /tmp 副本里跑的（源码 = 当前树）：

| 文件 | 带 `--no-web-resources-cdn`（/tmp/fl-good） | 不带（/tmp/fl-bad） |
|---|---|---|
| `index.html` | canvaskit 串 0 | 0 |
| `main.dart.js` | **0** | **1** |
| `flutter_bootstrap.js` | 串 **1**，`"useLocalCanvasKit":true` **1** | 串 **1**，`"useLocalCanvasKit":true` **0** |
| `flutter.js` | 串 1，`useLocalCanvasKit` 接线 1 | 串 1，接线 1 |

→ **两个可下结论的事实**：
1. `flutter_bootstrap.js` 里那个 gstatic 串是 `!useLocalCanvasKit ? "https://www.gstatic.com/…"` 的**死分支**，
   **正确构建里同样命中 1** ⇒ 按裸串判红 = **天天红**。
2. `"useLocalCanvasKit":true` 这个**标志**是带/不带 `--no-web-resources-cdn` 的**单调判别量**
   （带=有、不带=无）⇒ 这才是「生效路径」判据。

### 2.3 修法与自保

```
CDN_PROBE_SPEC="index.html:static main.dart.js:static flutter_bootstrap.js:config flutter.js:loader"
```

- **static**（index.html / main.dart.js）：出现 `gstatic\.com/flutter-canvaskit` ⇒ 红（裸串在这里是**真引用**）。
- **config**（flutter_bootstrap.js）：按**生效路径**判 —— ①配置形式的 `canvasKitBaseUrl` 指远端 ⇒ 红；
  ②配置形式给出本地值（如 `"canvaskit/"`）⇒ 绿；③两者都没有时要求 `"useLocalCanvasKit":true`，缺 ⇒ 红。
  （loader 里的三元死分支不匹配「配置形式」的正则，因此不会被误判。）
- **loader**（flutter.js）：决策由 config 喂入，本身恒含死分支 ⇒ 只自保「文件存在」+「仍含 `useLocalCanvasKit`
  接线」；上游若改了 loader 形状，config 那句断言就失去意义，**此时判红**要求人回来重新推导。
- **自保**：清单为空 ⇒ 红；任一探针文件 HTTP 非 200 / 文件不存在 ⇒ **红**（不静默跳过，这正是 F-0050-01 的教训）；
  `--check-dir` 无参数 ⇒ `exit 2`（不会掉进「预检+启动」分支）；产物目录不存在 ⇒ 红。
- **顺带修掉的隐患**：旧实现用 `curl … | grep -q`，而脚本有 `set -o pipefail` —— grep 提前命中会让 curl 收到
  SIGPIPE(141)，pipefail 把**命中**判成失败（假绿）；新实现用 `$(curl …)` + here-string，无此坑。

### 2.4 变红/变绿矩阵（原始输出摘录）

```
########## GREEN: --check-dir /tmp/fl-good/build/web ##########
    [ok]   /tmp/fl-good/build/web/index.html 不依赖 Google CDN
    [ok]   /tmp/fl-good/build/web/main.dart.js 不依赖 Google CDN
    [ok]   /tmp/fl-good/build/web/flutter_bootstrap.js "useLocalCanvasKit":true（不落 Google CDN 分支）
    [ok]   /tmp/fl-good/build/web/flutter.js 仍读 useLocalCanvasKit（决策由 flutter_bootstrap.js 的配置给出）
    （CDN 探针共扫 4 个产物文件）
==> 产物体检通过（未发现 Google CDN CanvasKit 依赖）        exit=0

########## RED 1: 无 --no-web-resources-cdn 的真实构建 ##########
    [FAIL] /tmp/fl-bad/build/web/main.dart.js 仍引用 Google CDN 的 CanvasKit —— 断网会白屏
    [FAIL] /tmp/fl-bad/build/web/flutter_bootstrap.js 既无本地 canvasKitBaseUrl、也未见 "useLocalCanvasKit":true
           —— 构建很可能漏了 --no-web-resources-cdn（落 Google CDN 分支，断网会白屏）  exit=1

########## RED 2: 生效 canvasKitBaseUrl → 远端（在好产物副本上注入）##########
    [FAIL] /tmp/gate-remote/flutter_bootstrap.js 生效的 canvasKitBaseUrl 指向远端地址 —— 断网会白屏
           实际取值：canvasKitBaseUrl":"https://www.gstatic.com/flutter-canvaskit/abc123"        exit=1

########## RED 3: 探针文件缺失（在好产物副本上删 flutter.js）##########
    [FAIL] 产物缺失：/tmp/gate-missing/flutter.js —— 探针文件扫不到即判红（不静默跳过）           exit=1

########## RED 4: index.html 里出现 canvaskit CDN 串（好产物副本注入）##########
    [FAIL] /tmp/gate-static/index.html 仍引用 Google CDN 的 CanvasKit —— 断网会白屏              exit=1

########## GREEN 2: 显式本地 canvasKitBaseUrl="canvaskit/" ##########
    [ok]   /tmp/gate-local/flutter_bootstrap.js 生效的 canvasKitBaseUrl":"canvaskit/"（本地相对路径）  exit=0

########## HTTP GREEN: --check --port 18099（好产物副本 + 真二进制）##########
    [ok] GET / → 302 Location: /app/ ; [ok] GET /app/ → 200 ; 四个产物文件全 ok ; 共扫 4 个文件   exit=0

########## HTTP RED: --check --port 18098（坏产物副本 + 真二进制）##########
    [FAIL] /app/main.dart.js 仍引用 Google CDN 的 CanvasKit —— 断网会白屏
    [FAIL] /app/flutter_bootstrap.js 既无本地 canvasKitBaseUrl、也未见 "useLocalCanvasKit":true      exit=1
```

另有**一次非计划的红**（共享产物被清空期间，18:29）：

```
    [FAIL] GET /app/main.dart.js 期望 200，实得 404 —— 探针文件扫不到即判红（不静默跳过）
    [FAIL] GET /app/flutter_bootstrap.js 期望 200，实得 404 —— 探针文件扫不到即判红（不静默跳过）
==> 体检失败：见上面的 [FAIL]                                        exit=1
```

### 2.5 对审计账本的两处**修正**（以实测为准）

1. 账本 F-0050-01 的影响段猜测「有人漏掉 `--no-web-resources-cdn` 重新构建后，被探的两个文件**很可能**仍是 0 命中
   ⇒ 体检报不依赖 CDN」。**本次 A/B 证伪了这个具体猜测**：Flutter 3.47.3 下 `main.dart.js` 在无标志构建里**命中 1**
   （带标志时 0），所以**旧探针恰好也能抓住这一次坏构建**。结构缺口是真的（探针集与「活决策文件」不交、
   行为随上游版本漂移），但「旧门禁一定是假绿」这一条**不成立**，如实记下。
2. 账本建议①「把 `flutter_bootstrap.js` 加进 :89 的清单」若**照字面**实现（换文件、仍按裸串判红），
   会得到**永远变红**的门禁 —— 实测：`/tmp/fl-good/.../flutter_bootstrap.js` 与 `flutter.js` 的裸串命中**各 1**。
   必须按**标志/生效路径**判（本实现即②）。这条也在 `ignite.sh` 注释里写死了，防止后人把串加回红模式。

### 2.6 第二处同形副本：**已修**（Lead 授权，见 `8.1）

`scripts/ignition-precheck.sh:134` 原先逐字复制了同一份错误探针（且更弱：只 grep `main.dart.js`）。
本轮把判据抽成**单一真源** `scripts/lib/cdn_probe.sh`，两个脚本 `source` 同一份；两处都在缺文件时
`exit 2`（不静默跳过）。红绿自证与原始输出见 `8.1。

---

## 3. F-0048-01 · 红线 K 无构建期门禁

### 3.1 补洞前后（穷举 grep）

```
--- BEFORE (git HEAD，5 个 workflow 全量) ---
BEFORE hits=0        # flutter build / trunk build / ignite.sh / gstatic / check_public_secrets 全零命中
--- AFTER (working tree) ---
AFTER hits=13        # 见下表
```

### 3.2 新增的两个 job

**A. `flutter-checks.yml` → `flutter-web-offline-artifacts`（每个相关 PR）**

```yaml
- name: "Build Flutter web (offline: no web resources CDN)"
  working-directory: shell/flutter
  run: flutter build web --release --base-href /app/ --no-web-resources-cdn
- name: Offline artifact gate (no Google CDN CanvasKit)
  run: ./scripts/ignite.sh --check-dir shell/flutter/build/web
```

**B. `nightly.yml` → `web-offline-serve-check`（每日 + 手动）**：flutter build（带标志）→
安装 Rust + **按需**装 ALSA 头（先 `pkg-config --modversion alsa` 探测，缺了才 apt，刻意避开
`build-upload.yml` 那个无条件 apt 的 exit 100 形态）→ `cargo build -p live2d-ai-desktop` →
以**缺省配置**起服务（`LIVE2D_AI_FLUTTER_WEB_DIR` 显式指向产物、`LIVE2D_AI_MUTE_AUDIO=1`）→
60 秒轮询 `/app/`（起不来就 `exit 1` 并打印服务日志，**不伪造绿灯**）→ `./scripts/ignite.sh --check --port 18080`。

放 nightly 不放 PR 的理由：它要编 `live2d-ai-desktop`（含 cpal/ALSA），耗时与系统依赖不适合每个 PR；
**产物级**那一层已经在 PR 门禁里（A），两层互补：A 查文件、B 查服务实吐字节。

### 3.3 本地等价实跑（这是本报告能给的最强证据）

- A 的两步：`flutter build web --release --base-href /app/ --no-web-resources-cdn` 在 /tmp 副本跑通（exit 0）；
  随后 `./scripts/ignite.sh --check-dir` 对**好副本 exit 0 / 坏副本 exit 1**。
- B 的启动链：**模拟 CI 环境**（空 cwd、无 `live2d-ai.toml`、无 `.env`、无 `assets/models`）起真二进制：
  ```
  poll 1: /app/=200
  ==> 体检通过      exit=0            # ./scripts/ignite.sh --check --port 18097
  ```
- 对 **Lead 重建后的共享产物** 与 **Lead 的 18080 活服务**各跑一次，全绿（exit 0，见 §2.4 同形输出）。

### 3.4 本 job 未在真 runner 上验证的点（如实列）

1. **`ubuntu-latest` 是否自带 `libasound2-dev`**：未验证。我按 `pr-checks.yml`（同样编 desktop crate、
   同样无 apt 步骤）的先例写，并加了「缺了才装」的条件安装，把这一类风险压到最小；但仍**没有被真 runner 证明**。
2. **`cargo build -p live2d-ai-desktop` 在干净 runner 上的耗时**：本机是增量构建，无法代表 CI 冷启。
   nightly job 设 30 分钟超时；若不够需要 Lead 调。
3. 本机没有 runner，**这条 workflow 自身从未被 GitHub Actions 调度过**。

---

## 4. F-0049-01 · 红线 R 密钥扫描门禁整体缺失

### 4.1 改前的事实（回源码核实）

- `scripts/check_public_secrets.py:5` 自称「GitHub CI 额外会跑 Gitleaks 覆盖完整历史」——
  `grep -rni gitleaks .github/ scripts/` 的**唯一**命中就是这句话本身 ⇒ 承诺**不存在**。
- 脚本在 CI 里从未被调用（`grep -rn check_public_secrets .github/` 改前 = 0）。
- `INCLUDED_PREFIXES` 不含 `shell/`、`docs/`、`.github/` ⇒ Dart 与文档**不在扫描面**。

### 4.2 改了什么

| 项 | 改前 | 改后 |
|---|---|---|
| 自我说明 | 「CI 会跑 Gitleaks 覆盖完整历史」（假） | 删掉；写明**没有 Gitleaks**、本脚本是唯一门禁、只扫当前树（历史提交不在面内） |
| 扫描面 | `crates/ xtask/ shared/ tests/ scripts/` + 顶层文件 | 追加 `shell/`、`docs/`、`.github/` |
| CI 调用 | 0 | `.github/workflows/secret-scan.yml`（PR/push main/手动，**无 paths 过滤**） |
| 实测覆盖 | **292 个文件** | **937 个文件**（+645，含全部 Dart 与文档） |
| 豁免语义 | 行级、偏宽 | **行为不变**，但在 docstring 里写明「与豁免标记同一行的真密钥会被跳过」这个已知取舍（收紧属另一条改动，未擅自改） |

### 4.3 变红自证（原始输出）

**① 真仓库（planted 假密钥 → 红 → 还原 → 绿）**：往**我自己作用域内**的 `scripts/check_public_secrets.py`
临时追加一行 AWS 假键（实际写入 `AKIA` + 16 位大写串；**报告里此处按 4 字符断开写成 `AKIA-ABCDEFGHIJKLMNOP`，以免新门禁把本报告这份证据判红**）：

```
planted exit=1
potential secrets found:
scripts/check_public_secrets.py:130: AWS access key
sha_before=ffe4a2683a8f7dfac1d8edf33712448310d6480b2aaa1506e998f096aba62be6
sha_after =ffe4a2683a8f7dfac1d8edf33712448310d6480b2aaa1506e998f096aba62be6
RESTORED-OK (sha256 identical)
repo secret-pattern scan: ok (937 files scanned)          restored exit=0
```

**② 扩面自证（合成 git 仓库：`git init` + `git add`，不提交；两条假密钥分别落在 Dart 与 docs）**：

```
tracked: docs/leak.md scripts/check_public_secrets.py shell/flutter/lib/leak.dart
--- NEW scope (shell/ + docs/ + .github/) ---   new-scope exit=1
potential secrets found:
docs/leak.md:1: GitHub token
shell/flutter/lib/leak.dart:1: OpenAI-style key
--- OLD scope (crates/ xtask/ shared/ tests/ scripts/) ---
repo secret-pattern scan: ok (1 files scanned)     # ← 旧扫描面**完全看不到**这两条
```

> 为什么用合成仓库做扩面自证：脚本只枚举 `git ls-files` 的**已跟踪**文件，而“把假密钥写进共享工作树的
> Dart/doc 文件再还原”会踩别人正在编辑的作用域（task-3/task-4 正在改 `crates/**`、`shell/flutter/lib|test/**`）。
> 真仓库那半用我自己作用域内的 tracked 文件做，覆盖「真 `git ls-files` 路径 + 真退出码」。

### 4.4 已知边界（不假装有网）

- **历史提交不在扫描面**：脚本只看当前 tracked tree；「覆盖完整历史」这件事**没有任何门禁**（Gitleaks 依然不存在）。
- **行级豁免偏宽**：与 `INJECTED`/`sk-test` 等标记同行的真密钥会被跳过（设计取舍，已写进 docstring）。
- `.gitignore` 里的目录（`build/`、`target/`、`assets/models/`）天然不在 tracked tree，也就不在扫描面。

---

## 5. 未决 / 需要 Lead 决策

1. ~~第二处错误探针~~ —— **Lead 已授权并已修**（`2.6 / `8.1：抽成 `scripts/lib/cdn_probe.sh` 单一真源）。
2. ~~AGENTS.md「本地必跑 vs CI 必跑」表~~ —— **Lead 已授权并已补三行**（`8.2；`git diff --stat AGENTS.md` = 3 insertions，未动其它行）。
3. **nightly 的 `web-offline-serve-check` 在真 runner 上的可行性**（§3.4）：若 `cargo build -p live2d-ai-desktop`
   在干净 runner 上因 ALSA 失败，请把失败日志给我，我改条件安装或把它降级为「只跑产物级 + shim 宿主」。
4. **Gitleaks 补不补**：现在 docstring 已经说了「没有」。补 = 新 workflow + 维护成本；不补 = 历史提交继续无网。
5. `.env` 相关：本机 `live2d-ai.toml`/`.env` 存在，脚本会回显**变量名**（不回显值）；CI 里没有这些文件，
   上述输出不会出现。无需动作，仅记录。

---

## 6. 我**没有**做的事（及原因）

1. **没有跑任何 CI**：本机无 runner（§0.1）。
2. **没有碰共享产物目录与 target/**：遵守 Lead 18:2x 的所有权裁决；我的构建全在 /tmp 副本，二进制只执行不重建。
3. **没有改** `crates/**`、`shell/**`、`xtask/**`、`AGENTS.md`、`scripts/ignition-precheck.sh`（不在写作用域）。
4. **没有 git commit / checkout / stash / reset / worktree**，也没有做仓库级 `dart format`。
5. **没有修** `check_public_secrets.py` 的行级豁免宽度（属另一条改动，避免「一条改动多个理由」）。
6. **没有加 `fonts.gstatic.com` 判红**：那是引擎运行期兜底，按串判会天天红；交给浏览器的 Network 断言。
7. ~~没有把探针清单抽成跨脚本共享文件~~ —— **本轮 Lead 授权后已完成**（`8.1）；初版报告交付时它确实越界未动。

---

## 7. 改动文件清单（绝对路径）

| 文件 | 状态 | 说明 |
|---|---|---|
| `/home/skystar/Live2D-Ai-fe/scripts/ignite.sh` | 改 | `--check-dir` + usage；探针判据改为 `source scripts/lib/cdn_probe.sh`（缺库 `exit 2`） |
| `/home/skystar/Live2D-Ai-fe/scripts/lib/cdn_probe.sh` | **新增** | 离线红线判据**唯一真源**（清单 + `cdn_judge` + `cdn_scan_http` + `cdn_scan_dir`），两个脚本共用 |
| `/home/skystar/Live2D-Ai-fe/scripts/ignition-precheck.sh` | 改 | A 段改用共享判据（原为只 grep `main.dart.js` 的错误副本） |
| `/home/skystar/Live2D-Ai-fe/AGENTS.md` | 改（+3 行） | 「本地必跑 vs CI 必跑」表补三条（仅新增，未动其它行） |
| `/home/skystar/Live2D-Ai-fe/scripts/check_public_secrets.py` | 改 | docstring 纠偏；扫描面 +`shell/`、`docs/`、`.github/` |
| `/home/skystar/Live2D-Ai-fe/.github/workflows/flutter-checks.yml` | 改（+42） | 新增 job `flutter-web-offline-artifacts` |
| `/home/skystar/Live2D-Ai-fe/.github/workflows/nightly.yml` | 改（+86） | 新增 job `web-offline-serve-check` |
| `/home/skystar/Live2D-Ai-fe/.github/workflows/secret-scan.yml` | **新增** | 密钥扫描门禁（44 行） |
| `/home/skystar/Live2D-Ai-fe/docs/audit/2026-10-05-debt-round/CI-GATES-REPORT.md` | **新增** | 本报告 |

**行数**：`scripts/ignite.sh` 233 行、`scripts/lib/cdn_probe.sh` 145 行、`scripts/ignition-precheck.sh` 419 行、`scripts/check_public_secrets.py` 128 行、`secret-scan.yml` 44 行（shell/python/脚本不在 xtask 的源码行数门禁口径内，增量也都远低于 500 行）。

**校验（第二轮结束时的最终态）**：`bash -n` 三个 shell 文件 OK；`python3 -m py_compile` OK；`yaml.safe_load` 对 6 个 workflow 全通过（build-upload=1、flutter-checks=2、nightly=4、pr-checks=3、release-build=2、secret-scan=1）；`scripts/check_public_secrets.py` → `ok (937 files scanned)` exit 0；`ignite.sh --check-dir` 好副本 exit 0 / 坏副本 exit 1；`ignition-precheck.sh` A 段好实例 exit 0 / 坏实例 exit 1；`git diff --stat AGENTS.md` = 3 insertions。
