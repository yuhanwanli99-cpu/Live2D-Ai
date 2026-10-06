# 产物预算（artifact budget）：Flutter Web build/web 与 wasm dist

> 状态：**现行**（2026-10-06 团队轮 W2 / 共享板 task-8 落盘）。
> 本文件回答三件事：**预算是多少、谁在守、哪里还漏风**。
> 上游依据：[DECISION-artifact-budget-2026-10-06.md](../plans/DECISION-artifact-budget-2026-10-06.md)（E4 方案纸，只论证未实施）·
> [NEXT-ROUND-main-2026-10-06.md](../plans/NEXT-ROUND-main-2026-10-06.md)（E4 / E13）·
> [PLAN-debloat-and-closeout-2026-10-01.md](../plans/PLAN-debloat-and-closeout-2026-10-01.md) §D4。
> 本文所有数字都是**本文件写作时在同一棵树上跑出来的**（工作树 /home/skystar/Live2D-Ai，HEAD 64f412d），
> 不是推算；无法实测的都标「未实测」。

---

## 0. 一张表

| 产物 | 现状（字节和口径） | 预算 | 谁在守 |
| --- | --- | --- | --- |
| `shell/flutter/build/web` | 清减前 **51 964 234 B = 49.56 MiB** / 66 文件 → 清减后 **30 804 686 B = 29.38 MiB** / 54 文件 | **35 MiB**（`xtask/src/code_stats/mod.rs` 的 `FLUTTER_WEB_BUDGET_MIB`，L178） | ✅ **有机器守门**：`scripts/prune_web_artifacts.sh --check`（本地必跑 + CI `flutter-checks.yml` → `flutter-web-offline-artifacts`） |
| `crates/l2d-wasm-demo/dist` | **5 606 642 B = 5.35 MiB** / 3 文件（wasm 5 484 191 + js 118 993 + index.html 3 458） | `WASM_DIST_BUDGET_MIB`（同文件 L182；**本文写作时 = 4**。决策纸 E4-3 建议按实测重定为 4.2，由本轮 W1/task-7 执行 —— **以 xtask 落地值为准**） | ❌ **无机器守门**（见 §5.1） |

两个口径说明（照抄 xtask，别混）：

1. 体积 = **目录内常规文件字节数之和**（`xtask/src/code_stats/collect.rs` 的 `dir_size`，symlink 跳过），
   不是 `du` 的块占用。同一棵树 `du -sh` 显示 50M 而字节和是 49.56 MiB，差在块对齐。
2. 预算判据真源在 **xtask**；`scripts/prune_web_artifacts.sh` 里重复了一份 35 MiB，
   目的是让产物门禁**不依赖 xtask 二进制**（DECISION §2.2 第 3 点：独立探针）。
   两处不一致时脚本在 `--check` 里**判红**（「预算判据漂移」），不会静默各说各话。

---

## 1. 为什么会有「死重」

`flutter build web` 不是「只产出用得到的东西」：它把 Flutter SDK 引擎缓存
（`$HOME/flutter/bin/cache/flutter_web_sdk/canvaskit/`）里的**整套** CanvasKit 变体
原样拷进 `build/web/canvaskit/` —— 6 个 `.wasm` 变体 + 各自的 `.js` loader +
各自的 `.symbols` 调试符号。而一次真实会话只会加载其中**一个**变体。

本构建（`flutter_bootstrap.js` 的 buildConfig：`compileTarget=dart2js`、`renderer="canvaskit"`、
`useLocalCanvasKit=true`）在 Chrome 下的生效路径是
`canvaskit/chromium/canvaskit.{js,wasm}`；`skwasm*` / `wimp*` 只在 renderer 为 `skwasm` 时用，
`*.symbols` 是栈回溯用的调试 sidecar，**运行期一次都不会被 fetch**。

实测（两次真实浏览器会话的 Network 转储，见 DECISION §1.3 与
`docs/verification/evidence-2026-10-06/net.json`）：

```
$ grep -o -E '/app/canvaskit/[A-Za-z/._-]+' docs/verification/evidence-2026-10-06/net.json | sort | uniq -c
      4 /app/canvaskit/chromium/canvaskit.js
      4 /app/canvaskit/chromium/canvaskit.wasm
$ grep -c -E 'skwasm|wimp|\.symbols' docs/verification/evidence-2026-10-06/net.json
0
```

---

## 2. 清减清单（删什么 / 留什么）

**删**（脚本 `scripts/prune_web_artifacts.sh`，渲染器 = `canvaskit` 时）：

| 类别 | 文件 | 字节 |
| --- | --- | ---: |
| `*.symbols` | canvaskit.js.symbols / chromium/canvaskit.js.symbols / webparagraph/canvaskit.js.symbols / skwasm.js.symbols / skwasm_heavy.js.symbols / wimp.js.symbols | 8 581 625 |
| `skwasm*` | skwasm.js / skwasm.js.symbols / skwasm.wasm / skwasm_heavy.js / skwasm_heavy.js.symbols / skwasm_heavy.wasm | 8 809 277 |
| `wimp*` | wimp.js / wimp.js.symbols / wimp.wasm | 5 453 834 |
| **合计** | **12 文件** | **21 159 548 B = 20.18 MiB** |

**必须保留**（`--check` 逐条断言存在且非空，被删就判红）：

- `canvaskit/canvaskit.{js,wasm}` —— **非 Chromium 浏览器**（没有 break-iterator / WebCodecs 图像解码）
  时的回落，删了那些浏览器当场不可用；
- `canvaskit/chromium/canvaskit.{js,wasm}` —— **本机 Chrome 实际加载的就是这份**（红线）；
- `canvaskit/webparagraph/**` —— 本轮**刻意保留**（B 档）：上游一旦默认打开 `preferWebParagraph`
  就会 404，为 3.6 MiB 承担这个未来风险不值得（DECISION §2.1）；
- `index.html` / `main.dart.js` / `flutter.js` / `flutter_bootstrap.js`
  （CDN 探针清单与加载决策的真源，见 `scripts/lib/cdn_probe.sh`）；
- `assets/**`（含 `NOTICES`，许可义务）与 `font-fallback/**`（E10 离线字体镜像）。

**渲染器判定**：脚本从 `build/web/flutter_bootstrap.js` 的 buildConfig 读 `renderer`；
读不到就**判红**（不猜）。`renderer="skwasm"` 时只清 `*.symbols`、**不删任何 wasm 变体**
（skwasm 构建的回退边界本轮未实测），并在 stderr 打 warn。可用 `PRUNE_WEB_RENDERER` 覆盖。

---

## 3. 怎么用（本地 ↔ CI 逐条对齐）

```bash
# 1) 构建（红线：--no-web-resources-cdn 不是可选项）
cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn && cd ../..
# 2) 清减（构建后步骤；手删产物 = 下次构建悄悄反弹）
./scripts/prune_web_artifacts.sh
# 3) 门禁：死重 0 + 红线文件齐全 + ≤ 35 MiB + 与 xtask 预算一致
./scripts/prune_web_artifacts.sh --check
# 4) 离线产物红线（与上面同源判据，见 scripts/ignite.sh --check-dir）
./scripts/ignite.sh --check-dir shell/flutter/build/web
```

CI 对位：`.github/workflows/flutter-checks.yml` → job `flutter-web-offline-artifacts`：
`flutter build web … --no-web-resources-cdn` → `prune_web_artifacts.sh` →
`prune_web_artifacts.sh --check` → `ignite.sh --check-dir`。
**CI 里没有的检查不进「本地必跑」清单**，反之亦然 —— 所以本地第 2/3 步与 CI 的两个
step 是同一对命令（`scripts/prune_web_artifacts.sh` 已加进该 workflow 的 `paths:` 触发面）。

> AGENTS.md 的「本地必跑 vs CI 必跑」表需要新增一行（本地 `prune_web_artifacts.sh --check`
> ↔ `flutter-checks.yml` → `Artifact budget gate`）。**该文件不在 W2 写面**，
> 由 Lead / 文档 owner 在收口时补。

---

## 4. 本轮的实测证据（原始片段）

### 4.1 清减（`scripts/prune_web_artifacts.sh`，同一棵树）

```
==> 产物清减：/home/skystar/Live2D-Ai/shell/flutter/build/web（渲染器 = canvaskit）
    清减前：51964234 B = 49.56 MiB / 66 个文件
    [-] canvaskit/canvaskit.js.symbols (1356926 B)
    [-] canvaskit/chromium/canvaskit.js.symbols (1235870 B)
    [-] canvaskit/skwasm.js (63537 B)
    [-] canvaskit/skwasm.js.symbols (1540006 B)
    [-] canvaskit/skwasm.wasm (3593715 B)
    [-] canvaskit/skwasm_heavy.js (63650 B)
    [-] canvaskit/skwasm_heavy.js.symbols (1672152 B)
    [-] canvaskit/skwasm_heavy.wasm (5216217 B)
    [-] canvaskit/webparagraph/canvaskit.js.symbols (963641 B)
    [-] canvaskit/wimp.js (59127 B)
    [-] canvaskit/wimp.js.symbols (1813030 B)
    [-] canvaskit/wimp.wasm (3581677 B)
==> 清减完成：删除 12 项 / 21159548 B = 20.18 MiB
    清减后：30804686 B = 29.38 MiB / 54 个文件
```

### 4.2 `--check`（清减后）

```
==> 产物清减检查：…/shell/flutter/build/web（渲染器 = canvaskit，预算 35 MiB）
    [ok]   死重清单 0 项（*.symbols / skwasm* / wimp*）
    [ok]   红线文件齐全（9 项，含 chromium/ 与 base canvaskit）
    [ok]   目录体积 30804686 B = 29.38 MiB ≤ 35 MiB（54 个文件）
    [ok]   预算与 xtask FLUTTER_WEB_BUDGET_MIB 一致（35 MiB）
==> 通过   （exit 0）
```

### 4.3 反向自证（三条，各自红→绿）

| # | 破坏方式 | `--check` 输出 | 复原后 |
| --- | --- | --- | --- |
| 1 | 从引擎缓存放回 `canvaskit/wimp.js.symbols`（1 813 030 B） | `[FAIL] 死重残留（应被清减）：canvaskit/wimp.js.symbols` → **exit 1** | 删除 → 通过（exit 0） |
| 2 | 加一个非死重名的大文件 `canvaskit/zz-budget-probe.bin`（6 MiB，总量 35.38 MiB） | `[FAIL] 目录体积 37096142 B = 35.38 MiB > 35 MiB 预算` → **exit 1** | 删除 → 通过（exit 0） |
| 3 | 移走红线文件 `canvaskit/chromium/canvaskit.wasm` | `[FAIL] 红线文件缺失/空：canvaskit/chromium/canvaskit.wasm` → **exit 1** | 放回 → 通过（exit 0） |

三条覆盖了三个独立分支：**死重残留 / 超预算 / 红线被删**。自证用的三个探针文件都已清理，
最终目录回到 54 文件 / 30 804 686 B。

### 4.4 产物离线体检（`ignite.sh --check-dir`）

```
==> 产物离线体检：shell/flutter/build/web（--check-dir；不需要服务）
    [ok]   …/index.html 不依赖 Google CDN
    [ok]   …/main.dart.js 不依赖 Google CDN
    [ok]   …/flutter_bootstrap.js "useLocalCanvasKit":true（不落 Google CDN 分支）
    [ok]   …/flutter.js 仍读 useLocalCanvasKit（决策由 flutter_bootstrap.js 的配置给出）
    （CDN 探针共扫 4 个产物文件）
==> 产物体检通过（未发现 Google CDN CanvasKit 依赖）
```

### 4.5 清减后**真服务 + CDP** 验证（连跑两次，同一 HEAD / 同一探针哈希）

服务：`./scripts/ignite.sh`（18080）；浏览器：Playwright 自带 Chrome/153.0.8010.12 headless +
swiftshader WebGPU，CDP 9222；驱动走项目既有探针 `scripts/browser_probe.mjs net`
（sha256 `302dc9cf…`），`PROBE_OUT` 指向 `/tmp`（不写仓库）。

探针自身 5 项 **fail 0**（两次一致），其中：

```
[PASS] 1a 加载 /app/ 后零外部源请求 — 共 41 条请求，外部源 0 条；hosts=127.0.0.1:18080
[PASS] 1b CanvasKit 实际从同源 canvaskit/ 加载 — 200 …/app/canvaskit/chromium/canvaskit.wasm |
       200 …/app/canvaskit/chromium/canvaskit.js | 200 …/app/canvaskit/chromium/canvaskit.js |
       200 …/app/canvaskit/chromium/canvaskit.wasm
[PASS] 1c 模型资产走本机静态路由且 200 — 5 条
[PASS] 2 控制台：… 设计内 404 1 条（…/actions/field_map.json，可选覆盖表，回落内建默认）；
       未预期 4xx/5xx 0 条；WebGL CONTEXT_LOST_WEBGL 噪声 2 条（无头 swiftshader）；其余真错误 0 条：[]
```

对原始事件（`net.json` / `console.json`）的独立断言，两次都是 **ASSERT_FAIL=0**：

```
[ok] canvaskit/chromium/canvaskit.wasm 200 :: 200 …/app/canvaskit/chromium/canvaskit.wasm（×2）
[ok] canvaskit/chromium/canvaskit.js   200 :: 200 …/app/canvaskit/chromium/canvaskit.js（×2）
[ok] 死重（skwasm*/wimp*/*.symbols）0 请求 :: 0 条
[ok] 无未预期 404（设计内可选覆盖表除外） :: 未预期 404 = 0 条；设计内可选覆盖表 = 1 条（/actions/field_map.json）
[ok] 无未预期 3xx/4xx/5xx（同上口径）
[ok] 控制台无真错误 :: []
全部响应 41 条；/app/ 响应 28 条
```

托管层字节核对（`curl` 实收 == 磁盘）：

```
GET /app/canvaskit/chromium/canvaskit.wasm -> http=200 size=5428806   （磁盘 5428806 B）
GET /app/canvaskit/chromium/canvaskit.js   -> http=200 size=86226     （磁盘 86226 B）
磁盘死重残留数 = 0
```

两条**如实说明**（都不判红，但别误读）：

- `/actions/field_map.json` 的 404 是**设计内**的：渲染面 `main.rs` 头注写明该覆盖表不存在就
  回落内建默认表。清减前后都存在，与本次删文件无关。
- CDP 记录里有一条 `net::ERR_ABORTED` 的 failure 事件（**没有 URL**，41 条请求
  都有对应响应；探针判定 `真错误 0`）。属无头环境噪声，未展开追因。
- WebGPU canvas 在无头 swiftshader 下会偶发 `CONTEXT_LOST_WEBGL`（探针已按环境噪声分类）。
  本轮**没有**试图拿它当产品判据。

收工：服务与 chrome 已停（`pgrep -af 'live2d-ai-desktop'` → 空；`remote-debugging-port=9222` → 空；
18080/9222 端口空闲）。

---

## 5. 已知缺口（**没做到的部分，写在这里而不是假装它被守着**）

### 5.1 `crates/l2d-wasm-demo/dist` 没有任何机器守门

实测依据（本文件写作时同树）：

```
$ grep -rn 'code-stats' .github/workflows/
pr-checks.yml:79  cargo run -p xtask -- code-stats --check --only lines
pr-checks.yml:81  cargo run -p xtask -- code-stats --check --only over-1000
pr-checks.yml:83  cargo run -p xtask -- code-stats --check --only deps
$ grep -rn 'trunk' .github/workflows/
（无输出）
```

- `code-stats` 的三条 `--only` **不含** `over-500`（lines 组）以外的 artifacts/预算组，
  且它跑在 **Rust job** —— 那个 job 不构建 Flutter、也不 `trunk build` wasm；
- 全仓 workflow 里**没有** trunk build ⇒ `dist` 在 CI 里根本不存在，预算只有维护者本机
  跑 `cargo run -p xtask -- code-stats` 时才会被看到（报告行「在预算内 / 超预算」，不进退出码）；
- `xtask/src/code_stats/gates.rs` 的 `evaluate()` 只有四条门禁，**没有 artifacts**。

⇒ 结论：**`dist` 的 4 MiB（或重定后的 4.2）目前是一句没人验的声明。**
要真守它，需要「CI 里真的构建 wasm + 让判据进退出码 + 目录缺失也判红」三件一起做
（DECISION §3.3 / E4-4）。本轮**不做**。

### 5.2 wasm 真减未实测

`dist` 里只有 3 个文件，个个在用；决策纸实测「只剥 `name` 段」后是 4.11 MiB，**仍超 4 MiB 预算
0.115 MiB**，剩下的缺口只能靠编译器级手段（`wasm-opt` / LTO / `opt-level="z"`）。
本机工具链实测：

```
wasm-opt: absent    wasm-strip: absent    wasm-pack: absent    trunk: /home/skystar/.cargo/bin/trunk
```

⇒ **收益未实测，本文不给估算数字**；本轮对 `dist` 只做「预算明文重定 + 写清理由」
（决策纸 E4-3 的 (b) 路，执行在 W1/task-7）。

### 5.3 清减门禁的覆盖面

- 只在 `flutter-checks.yml` 的 `flutter-web-offline-artifacts` job 跑；触发面 =
  `shell/flutter/**`、`crates/*/src/**`、`scripts/prune_web_artifacts.sh`、该 workflow 自身。
  **纯 Rust/文档 PR 不跑**（与既有分工一致，不是新缺口）。
- `nightly.yml` 的 `web-offline-serve-check` 也会 `flutter build web`（**未清减**），
  但它只用产物做服务层离线体检、不判体积 ⇒ 与预算无关；若将来在 nightly 加体积判据，
  必须在同一 job 里先接 prune。
- 本机手跑 `flutter build web` 之后**必须重跑 prune**，否则 `code-stats` 的 artifacts 行会
  回到「超预算」。这正是把清减做成**脚本 + --check**而不是手删的原因。
- 35 MiB 是**当前 Flutter stable** 产物的实测上界，清减后余量 5.6 MiB。CI 用
  `subosito/flutter-action@v2` 的 `channel: stable`（**未钉版本**）⇒ 上游升级引擎会改变
  CanvasKit 体积，可能把这条门禁判红。判红时先分清「引擎变大」还是「死重回潮」：
  前者按 §6 第 1 条两处同改预算并写明依据，后者重跑 prune。

### 5.4 本文件未做的事

- 没有改 `xtask/**`（预算常量与报告口径属 W1/task-7 写面）。
- 没有删 `canvaskit/webparagraph/`（B 档，DECISION §2.1 明确保留）。
- 没有在 CI 里构建 wasm（§5.1）。
- 没有新增/修改任何前端源码（`shell/flutter/lib` 一行未动；build/web 是产物目录，已按
  `.gitignore` 排除，不入库）。

---

## 6. 改动须知（下次动预算 / 动清减清单时）

1. **改预算 = 至少同时改两处**：`xtask/src/code_stats/mod.rs` 的常量（+注释写明理由与实测依据）
   与 `scripts/prune_web_artifacts.sh` 的 `BUDGET_MIB`（否则 `--check` 报「预算判据漂移」判红）。
   **不许只改数字**（DECISION §3.2 / §3.3）。
2. **改清减清单 = 同时改三处**：脚本的 `kill_list`、`KEEP_LIST`、以及本文件的 §2 表
   （清单、字节数、依据）；并做一次「放回 → --check 红 → 删 → 绿」的反向自证。
3. **改渲染器相关**（`--wasm` 构建）：先补 skwasm 分支的清单与实测，再放开
   `renderer != canvaskit` 的告警路径；当前它只清符号、不删变体。
4. 任何让「死重」重新出现的构建（`flutter build web`、清 `build/` 后重建）之后，
   **同一轮里**要重跑 prune + `--check`，否则数字与判据会静默脱节。
