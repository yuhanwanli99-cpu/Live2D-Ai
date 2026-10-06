# DECISION — E4：产物预算（`build/web` 49.55 MiB vs ≤35；wasm `dist` 5.35 MiB vs ≤4）

> 状态：**方案纸，未实施**。写面 `docs/plans/**`；本文件不改任何源码、不改任何产物。
> 上游：[HANDOFF-2026-10-06-e1-e5-debt-round.md](HANDOFF-2026-10-06-e1-e5-debt-round.md) §6 第 2 条（E4 **待裁决**）·
> [NEXT-ROUND-main-2026-10-05.md](NEXT-ROUND-main-2026-10-05.md) §N9「产物预算要么达成、要么**明文改预算 + 理由**」·
> [PLAN-debloat-and-closeout-2026-10-01.md](PLAN-debloat-and-closeout-2026-10-01.md) §D4 第 168 行
> **所有数字都来自实际命令输出**（见 §6）；凡未实测的一律标注「未实测」。

---

## 0. 结论（先看这一段）

| 问题 | 本次实测答案 |
|---|---|
| 这两条「超预算」现在会让谁变红？ | **谁都不会**。`gates.rs` 只判四条门禁（`src-rs-500 / dart-800 / src-rs-1000 / deps-desktop`），**没有 artifacts**；产物只有一行**报告文本**「超预算」。CI 里唯一跑 `code-stats` 的是 **Rust job**，它**不构建 Flutter** ⇒ 那两行在 CI 里是「缺 / 未构建」（§1.4） |
| `build/web` 有真减路吗？ | **有，而且余量很大**：只删「本构建渲染器**永不加载**」的引擎副本 + 无引用的 `*.symbols` ⇒ **−20.18 MiB → 29.38 MiB**（≤35 达标，还留 5.6 MiB）。**前提**：不能盲删 `chromium/` —— 实测 Chrome 加载的**正是**它（§1.3） |
| wasm `dist` 有真减路吗？ | **只靠剥调试名不够**：剥掉 `name` 段后 `dist` = **4.11 MiB，仍超 0.115 MiB**（120 501 B）；且 `dist` 里**没有**任何可剥的冗余文件（只有 3 个文件）。要真减到 ≤4 必须再上 `wasm-opt`/LTO 之类**对代码段**动手的编译器级手段，**本机没有这些工具、本文未实测收益** |
| 建议 | `build/web`：**做真减**（方案 1，脚本化 + CI 接线 + 守卫）；wasm `dist`：**二选一并写清理由** —— (a) 先做已实测的 `name` 段剥离，再把预算**明文**重定为 4.2 MiB；(b) 装 `wasm-opt` 真减到 ≤4（收益需实建实测）。**两条都不许只改数字**（§3.3） |
| 与既有调研的关系 | `docs/research/ui-design-flutter-architecture-2026-09.md:795` 当时写的是「**推断**当前 JS+CanvasKit 路径不请求它们，但**未实测网络请求验证**」。本轮**补上了这个实测**（§1.3）；同时该行把 `chromium/` 也列进「用不到的副本」，**这一点是错的**，照它删会直接白屏 |

---

## 1. 实测构成

### 1.1 `shell/flutter/build/web`

```
$ du -sh shell/flutter/build/web
50M     shell/flutter/build/web
# 但 xtask 的 dir_size 是「常规文件字节数之和」（collect.rs:252-274），不是 du 的块占用：
$ find shell/flutter/build/web -type f -printf '%s\n' | awk '{s+=$1} END{print s}'
51962935        # = 49.55 MiB（xtask 报告里显示的就是这个数）
$ find shell/flutter/build/web -type f | wc -l
66
```

分类（同一份字节口径）：

| 分类 | MiB | 文件数 |
|---|---:|---:|
| `canvaskit/*.wasm`（**6 个变体**） | **27.50** | 6 |
| `canvaskit/*.symbols`（调试符号 sidecar） | **8.18** | 6 |
| `assets/fonts`（NotoSansSC 子集 Bold+Regular 等） | 6.10 | 3 |
| `main.dart.js` | 3.14 | 1 |
| `font-fallback/`（E10 的离线字体镜像） | 2.72 | 24 |
| `assets/` 其余（NOTICES 1.24 + shaders + MaterialIcons 等） | 1.44 | 8 |
| `canvaskit/*.js`（6 个变体的 loader） | 0.42 | 6 |
| 其余（`index.html` / `flutter.js` / `flutter_bootstrap.js` / icons / `version.json` 等） | 0.07 | 12 |
| **合计** | **49.56** | **66** |

6 个 `.wasm` 变体逐个（**这是本决策的核心表**）：

```
6.95 MiB  canvaskit/canvaskit.wasm              base（非 Chromium 浏览器的回落）
5.18 MiB  canvaskit/chromium/canvaskit.wasm     本机 Chrome 实际加载的就是它（§1.3）
4.97 MiB  canvaskit/skwasm_heavy.wasm           只在 renderer == "skwasm" 时用
3.56 MiB  canvaskit/webparagraph/canvaskit.wasm 只在 preferWebParagraph && hasTextCluster 时用
3.43 MiB  canvaskit/skwasm.wasm                 同上（skwasm）
3.42 MiB  canvaskit/wimp.wasm                   同上（skwasm 单线程）
```

### 1.2 `crates/l2d-wasm-demo/dist`

```
$ find crates/l2d-wasm-demo/dist -type f -printf '%s %p\n' | sort -rn
5484191  .../l2d-wasm-demo-dd042ea77650b295_bg.wasm
 118993  .../l2d-wasm-demo-dd042ea77650b295.js
   3458  .../index.html
# 合计 5 606 642 B = 5.35 MiB（xtask 预算判据算的也是这个全目录）
```

wasm 内部段（自己解析 LE 段表得到；**全文件 5 484 191 B**）：

```
type 1 896    import 37 090    function 8 120    table 13    memory 5    global 27
export 879    element 6 244    code 3 602 048   data 536 024
custom/name              1 291 560 B   ← 23.6% 的文件是这个调试名表
custom/producers               126 B
custom/target_features         151 B
```

- `Trunk.toml` 已 `release = true`（不是 debug 产物；段表里也**没有** `.debug_*` 段）。
- 仓库根 `Cargo.toml` **没有任何 `[profile]` 段**（实测 `grep -n profile Cargo.toml` 无输出）⇒ 走默认 release。
- `.js` 胶水 **未压缩**：2 569 行、4 空格缩进（`function __wbg_get_imports() {` 起手）。

### 1.3 **谁真正被加载**（这一节是本轮把「推断」变成「实测」的地方）

**加载决策的真源**（`build/web/flutter_bootstrap.js`，压缩后 1 行，关键分支原文）：

```js
let r = s.hasChromiumBreakIterators && s.hasImageCodecs;
if (!r && e.canvasKitVariant == "chromium") throw "…unsupported in this browser";
let i = r && e.canvasKitVariant !== "full",
    o = i && e.preferWebParagraph && s.hasTextCluster,
    c = t;
o ? c = m(c,"webparagraph") : i && (c = m(c,"chromium"));
let l = "canvaskit.wasm";
o ? l = "webparagraph/canvaskit.wasm" : i && (l = "chromium/canvaskit.wasm");
```

本构建的配置（同一文件末尾的 build 配置）：`"builds":[{"compileTarget":"dart2js","renderer":"canvaskit","mainJsPath":"main.dart.js"},{}],"useLocalCanvasKit":true`
⇒ 渲染器固定 `canvaskit`（**不是** skwasm），且 Chrome 满足 `r` ⇒ 生效路径 = `canvaskit/chromium/canvaskit.{js,wasm}`。

**真实网络取证**（2026-10-05 / 2026-10-06 两次浏览器验收的 Network 转储）：

```
$ grep -o -E '/app/canvaskit/[A-Za-z/._-]+' docs/verification/evidence-2026-10-06/net.json | sort | uniq -c
      4 /app/canvaskit/chromium/canvaskit.js
      4 /app/canvaskit/chromium/canvaskit.wasm
$ grep -c -E 'skwasm|wimp|\.symbols' docs/verification/evidence-2026-10-06/net.json
0
（2026-10-05 那份同形：chromium 变体 4+4，skwasm/wimp/symbols 0）
```

⇒ 三条结论：

1. **`chromium/` 是必需品**（本机唯一被加载的 CanvasKit 变体）——`docs/research/ui-design-flutter-architecture-2026-09.md` 的 490/496/795 行把它与 `skwasm`/`wimp`/`symbols` 并列为「用不到的副本」，**那里错了**；
2. **`skwasm*` / `wimp*` / `webparagraph/` / `*.symbols` 在真实会话里 0 请求**（这是 795 行明确缺的那条实测）；
3. `base canvaskit/canvaskit.wasm` 是 `r == false`（浏览器没有 Chromium break-iterator 或 WebCodecs 图像解码）时的回落，**不能删**（本机没测到它，但它服务的是别的浏览器）。

### 1.4 预算判据在哪、是不是门禁

```
# 判据常量（唯一定义点）
xtask/src/code_stats/mod.rs:161-168
  const FLUTTER_WEB_DIST: &str = "shell/flutter/build/web";
  const FLUTTER_WEB_BUDGET_MIB: u64 = 35;     // PLAN §D4：47M → ≤35M
  const WASM_DIST: &str = "crates/l2d-wasm-demo/dist";
  const WASM_DIST_BUDGET_MIB: u64 = 4;        // PLAN §D4：5.4M → ≤4M

# 度量口径：目录内常规文件字节和（symlink 跳过），目录不存在 → None（"缺"）
xtask/src/code_stats/collect.rs:252-274  fn dir_size()
xtask/src/code_stats/collect.rs:132-141  把两个目录 push 进 stats.artifacts

# 判定：**只是报告文本**，不进退出码
xtask/src/code_stats/report.rs:210-244  write_artifacts() → "在预算内" / "超预算"
xtask/src/code_stats/gates.rs:127-172   evaluate() 构造的 GateResult **只有四条**，没有 artifacts
xtask/src/code_stats/gates.rs:176-185   exit_code() 只看 results 里 selected && !pass
```

**CI 侧**：

```
.github/workflows/pr-checks.yml:78-83        code-stats --check --only lines / over-1000 / deps（Rust job）
.github/workflows/flutter-checks.yml:78       flutter-web-offline-artifacts（构建 + ignite.sh --check-dir）
$ grep -n "code-stats" .github/workflows/flutter-checks.yml
（无输出）
$ grep -n "flutter" .github/workflows/pr-checks.yml
9: # 前端门禁在单独的 flutter-checks.yml —— 纯 Rust/文档 PR 不必等 Flutter 工具链
```

⇒ **CI 的 Rust job 不构建 Flutter** ⇒ `shell/flutter/build/web` 不存在 ⇒ 报告里是「缺 / 未构建」；
wasm `dist` 同理（Rust job 不跑 `trunk build`）。
**即：这两条预算目前只有维护者本机跑 `code-stats` 时才会被看到，没有任何机器守着它。**

---

## 2. 方案 1：真减

### 2.1 `build/web` —— 三档删除清单（全部实测）

| 档 | 删什么 | 删掉 | 剩余 | 保留文件 |
|---|---|---:|---:|---:|
| **A** | 全部 `*.symbols`（6）+ `skwasm.js/.wasm/.symbols` + `skwasm_heavy.*` + `wimp.*` | **20.18 MiB** | **29.38 MiB**（达标） | 54 |
| B | A + `canvaskit/webparagraph/`（3 文件） | 23.81 MiB | 25.75 MiB（达标） | 52 |
| C | B + base `canvaskit/canvaskit.wasm` | 30.76 MiB | 18.80 MiB（**不建议**：砍掉非 Chromium 浏览器回落） | 51 |

**A 档逐文件清单（字节级）**：

```
1356926  canvaskit/canvaskit.js.symbols
1235870  canvaskit/chromium/canvaskit.js.symbols
 963641  canvaskit/webparagraph/canvaskit.js.symbols
1540006  canvaskit/skwasm.js.symbols
1813030  canvaskit/wimp.js.symbols
1672152  canvaskit/skwasm_heavy.js.symbols
3593715  canvaskit/skwasm.wasm          63537  canvaskit/skwasm.js
5216217  canvaskit/skwasm_heavy.wasm    63650  canvaskit/skwasm_heavy.js
3581677  canvaskit/wimp.wasm            59127  canvaskit/wimp.js
```

**必须保留**（否则当场坏）：`canvaskit/canvaskit.{js,wasm}`（非 Chromium 回落）、`canvaskit/chromium/canvaskit.{js,wasm}`（**本机实际加载**）、
`main.dart.js`、`flutter.js` / `flutter_bootstrap.js`（CDN 探针与加载决策的真源）、
`index.html` / `manifest.json` / `version.json` / `icons/`、
`assets/` 全部（`NOTICES` 1.24 MiB 是许可义务，**不许删**）、`font-fallback/` 全部（E10 红线）。

**建议档 = A**：B 档多省的 3.6 MiB 来自 `webparagraph/`，而它的启用条件是 `preferWebParagraph` —— 该开关目前**在 Flutter 工具链里搜不到**
（`docs/research/ui-design-oss-adoption-2026-09.md:180` 已实测「无官方开关，属未启用路径」），删了当前无害、但一旦上游默认打开就是 404。
A 档已经回到预算内且余量 5.6 MiB，**没必要为 3.6 MiB 承担这个未来风险**。

### 2.2 关键坑：**`flutter build web` 会把它们全拷回来**

实测：Flutter 的引擎缓存里是**同一份完整集合**（13 个文件 + 2 个子目录，逐个同名同大小）：

```
$ ls -la $HOME/flutter/bin/cache/flutter_web_sdk/canvaskit/
canvaskit.js  canvaskit.js.symbols  canvaskit.wasm  chromium/  skwasm.js  skwasm.js.symbols
skwasm.wasm   skwasm_heavy.js  skwasm_heavy.js.symbols  skwasm_heavy.wasm  webparagraph/
wimp.js       wimp.js.symbols   wimp.wasm
```

⇒ **手删产物 = 下一次构建就复原**，数字会**悄悄反弹**。真减必须做成**构建后步骤**：

1. 新脚本（建议 `scripts/prune_web_artifacts.sh`）：删 A 档清单；
   **外加 `--check` 模式** —— 断言这些路径**不存在**（不存在则 exit 0，存在则红并打印清单）。
   没有 `--check` 的话，这条「已减」在两次构建之后就会变成一句没人验的声明。
2. 接线：
   - `.github/workflows/flutter-checks.yml` 的 `flutter-web-offline-artifacts` job：`flutter build web` 之后、`ignite.sh --check-dir` 之前插入 prune + `--check`；
   - `AGENTS.md` 前端层的构建命令串（现在只有 `flutter build web …` 一条）补上 prune；
   - 本机维护者流程同样要跑（否则 `code-stats` 的数字又回到 49.56）。
3. 可选（更硬）：把 A 档清单做成 `code-stats` 之外的**独立探针**，避免「改了 xtask 就改了判据」的疑虑。

### 2.3 wasm `dist` —— 真减到 ≤4 MiB 的路径（**部分未实测**）

**已实测有效的部分**：`name` 段（含段头的 `end-start` 是 **1 291 560 B**）是纯调试名表，**去掉后模块仍然合法**：

```
$ node …（逐段解析 → 丢弃 custom/name,producers,target_features → 重写 → 复验）
walk ends exactly at EOF: true | total 5484191
stripped size: 4192354 = 3.9981 MiB | dropped: 1291837 B
stripped walk re-parses to EOF: true
WebAssembly.validate(original): true
WebAssembly.validate(stripped): true
WebAssembly.compile(stripped): OK
```

**但账算不过来**（预算算**整个 dist 目录**，不只是 wasm）：

```
dist 合计            5 606 642 B  (5.35 MiB)
- name 段            -1 291 837 B
= 剥后 dist          4 314 805 B  (4.11 MiB)
预算                 4 194 304 B  (4.00 MiB)
⇒ 仍超 120 501 B（0.115 MiB）
```

**剩下这 120 KB 从哪来（三条路，按可信度排）**：

| 路 | 依据 | 状态 |
|---|---|---|
| (a) 压缩 `.js` 胶水（118 993 B，未压缩） | 实测 2 569 行 4 空格缩进；**只去掉行首空白与空行**后 97 006 B（**上限性质的探针，不是真 minifier 输出**） | 单靠它**到不了**：要填 120 501 B 的坑，胶水必须被砍掉超过 100% —— 不可能。可作补充，不能作主路 |
| (b) `wasm-opt -Oz`（binaryen） | `wasm-opt/wasm-strip/wasm-objdump` **本机全部 absent**（`command -v` 实测）；代码段 3 602 048 B 是主要目标 | **收益未实测**（需装工具 + 真跑）。按常理这是最可能一次到位的路，但本文不给估算数字 |
| (c) Cargo profile：`strip / lto = true / opt-level = "z" / codegen-units = 1` | 根 `Cargo.toml` **当前没有 `[profile]` 段**（实测）⇒ 改它会影响**整个 workspace**（编译时间 + 全部 crate 的产物），成本面很大 | **未实测**；且 Cargo 的 `[profile.release.package.*]` 覆盖**是否允许 `lto/strip` 需实施时实测确认**（本文不作为既定事实） |

**机制侧的一条提醒**：`name` 段的产生者是 `wasm-bindgen`/LLVM 链，剥离要落在**构建后**（`trunk` 有 `[[hooks]] stage = "post_build"` 可用；`trunk 0.21.14` 本机可用）。
本仓现有的构建入口是 `scripts/ignite.sh:190` 与 `scripts/run-web.sh:16` 的 `trunk build`（读 `Trunk.toml` 的 `release = true`）——
⇒ 只要 hook 挂在 `Trunk.toml` 上，两个入口**自动**都生效；挂脚本则两处都要记得调（**同一个动作两条路 = 下次漂移的种子**）。

### 2.4 红线与风险（真减方案必须过的门）

| # | 红线 / 风险 | 为什么安全 / 怎么验 |
|---|---|---|
| 1 | `--no-web-resources-cdn` 断网红线 | prune **不碰** `index.html` / `main.dart.js` / `flutter_bootstrap.js` / `flutter.js`；`cdn_probe.sh` 的清单正是这四个（`CDN_PROBE_SPEC`），逐文件 200 + 内容判定不变 ⇒ `ignite.sh --check` 与 `--check-dir` 仍绿 |
| 2 | CanvasKit 必须同源可用 | `chromium/` 与 base 都保留；`browser_probe.mjs` 判据 1b 只要求「存在一个 `/app/canvaskit/…` 的 200」（实测命中 chromium）⇒ 仍 pass |
| 3 | 加载器是否真的不会去 fetch 被删文件 | 已实测 0 请求（§1.3）；**验收仍要再测一次**：prune 后跑浏览器验收并把「`/app/` 请求里 404 数 = 0」作为硬判据（这一条比「数字变小」更重要） |
| 4 | 未来切 `--wasm`（renderer=skwasm）会缺文件 | 构建会把整套从引擎缓存拷回，prune 再按渲染器判断即可；**A 档清单要写成「只删当前构建的 `builds[].renderer` 不需要的变体」**，而不是硬编码清单（硬编码 = 下次切渲染器时的静默缺件） |
| 5 | 有人只删本地产物、构建流程没改 | `--check` 模式 + CI 接线（§2.2） |
| 6 | 字体 / 许可 / font-fallback 被连带删 | A 档清单**不含** `assets/`、`font-fallback/`、`NOTICES`；建议脚本用**白名单删除**（只删明确列出的路径），不用「保留白名单」的黑名单形状 |

### 2.5 方案 1 的验收判据

```
1) bash scripts/prune_web_artifacts.sh && bash scripts/prune_web_artifacts.sh --check   # 后者必须 exit 0
2) find shell/flutter/build/web -type f -printf '%s\n' | awk '{s+=$1} END{print s}'
   期望 ≤ 29.4 MiB（29 380 000 B 量级），且 ≤ 35 MiB 预算
3) cargo run -p xtask -- code-stats           # 第 4 节那一行必须从「超预算」变「在预算内」
4) ./scripts/ignite.sh --check                # 6/6（含 CDN 三类判定）
5) node scripts/browser_probe.mjs all         # pass 数与基线一致；且 /app/** 404 == 0（新增断言）
6) flutter analyze / flutter test             # 0 issue / 1583（prune 只动产物，不该影响前端测试）
```

---

## 3. 方案 2：明文改预算 + 理由

### 3.1 只读定位（**本文件不改**）

```
xtask/src/code_stats/mod.rs:163-164   /// Flutter Web 产物预算（PLAN §D4：47M → ≤35M）。
                                      const FLUTTER_WEB_BUDGET_MIB: u64 = 35;
xtask/src/code_stats/mod.rs:167-168   /// wasm 产物预算（PLAN §D4：5.4M → ≤4M）。
                                      const WASM_DIST_BUDGET_MIB: u64 = 4;
```

改这两个常量**不会破坏任何测试**：`code_stats/tests.rs` 只断言 `stats.artifacts.len() == 2` 与「fixture 目录不存在时 `bytes.is_none()`」（199–230 行），**没有断言预算数值**。
（这一点是「改起来太容易」的风险本身。）

### 3.2 报告口径怎么写

现在报告表头是（`report.rs:213`）：`| 产物 | 大小 | PLAN 预算 | 状态 |`

若重定预算，**表头与列名必须一起改**，让「这不是 PLAN 原值」在报告里一眼可见，建议：

```
| 产物 | 大小 | 预算（2026-10-06 重定） | 原 PLAN 预算 | 状态 |
```

即：**保留原 PLAN 值作为第二列**（35 / 4），新增列写重定值，并在 `mod.rs` 常量注释里写「为什么重定、依据哪次实测」。
**不许**把 35 直接改成 50 再把注释删掉 —— 那就是「悄悄改判据」。

### 3.3 「不许悄悄改」的同步清单

改预算**至少**要同时动这些地方（少一处就是漂移）：

1. `xtask/src/code_stats/mod.rs` 两个常量 + 注释（写**理由 + 实测依据 + 日期**）；
2. `xtask/src/code_stats/report.rs` 表头（§3.2）；
3. `docs/plans/PLAN-debloat-and-closeout-2026-10-01.md:168`（D4 的原始目标）与 `NEXT-ROUND` 对应行 —— **原文不动**，加一条「2026-10-06 重定：…」的注记指针；
4. 本文件的方案 2 结果回填 + 交付报告「未做 / 需维护者过目」一节。

### 3.4 方案 2 的验收判据

```
1) git diff 只含上述四处，且 diff 里能读到「理由 + 实测依据」文字（不是纯数字改动）
2) cargo run -p xtask -- code-stats          # 报告里出现新列，状态「在预算内」
3) cargo test --workspace --all-targets      # 1315/0 不变（xtask 测试对预算数值无断言，实测确认）
4) 同一提交里 MUST 有一条「防反弹」说明：重定后的新预算凭什么是上界
```

---

## 4. 推荐（带证据）

| 产物 | 推荐 | 目标数字 | 依据 |
|---|---|---|---|
| `shell/flutter/build/web` | **方案 1 真减（A 档）+ 脚本化成构建后步骤 + `--check` + CI 接线** | 49.56 → **29.38 MiB**（≤35） | 实测 0 请求 + 引擎缓存同集合（§1.3 / §2.2），余量 5.6 MiB |
| `crates/l2d-wasm-demo/dist` | **先做名表剥离（已实测、零风险）；再二选一**：(a) 装 `wasm-opt` 真减（收益需实测）；(b) 把预算**明文**重定为 **4.2 MiB** | 5.35 → **4.11 MiB**；预算 4.0 → 4.2（若走 b） | 只靠剥离**不够**（仍超 0.115 MiB），这一点必须写进任何「已达成」的声明里 |
| 报告口径 | 若 dist 走 (b)，按 §3.2 加「重定」列 | — | **不许只改数字** |

**为什么 `build/web` 不建议改预算**：它有一条**零功能代价**的 20 MiB 真减路（删的全是「本构建永不加载」的引擎副本与调试符号），
把预算从 35 抬到 50 等于**主动放弃**一个已经拿到手的事实收益。而 `dist` 不同：目录里只有 3 个文件、每个都在用，
剩下那 0.115 MiB 只能靠编译器级手段换，成本明显更高 ⇒ 那里才值得考虑明文重定。

---

## 5. 裁决项 / 影响 / 判据

| ID | 裁决项 | 选项 | 影响 | 判据 |
|---|---|---|---|---|
| **E4-1** | `build/web` 走真减还是改预算 | (a) **真减 A 档**（推荐） (b) 改预算（35 → 50） | (a)：−20.18 MiB，需新脚本 + CI 接线 + 一条 404 断言；(b)：放弃事实收益，且 50 MiB 会成为新的「无理由上界」 | (a)：§2.5 六条全过；(b)：按 §3.3 四处同步 + 理由文本 |
| **E4-2** | A 档要不要连 `webparagraph/` 一起删（B 档） | (a) 只 A 档（推荐） (b) A+B 档 | (b) 多省 3.6 MiB，承担「上游默认打开 WebParagraph ⇒ 404」的未来风险 | (b)：清单里写明「此项依赖 `preferWebParagraph` 未启用」+ 一条探针 |
| **E4-3** | `dist` 走真减还是明文重定 | (a) 装 `wasm-opt` 真减 (b) **名表剥离 + 明文重定 4.2 MiB** | (a)：需引入工具链依赖（本机 absent）、收益未实测；(b)：4.11 是构建确定性结果，但预算上界被动抬高 0.2 MiB | (a)：实测 `dist ≤ 4.00 MiB` + `/render@@ 探针通过（**不许用估算值**）；(b)：§3.4 四条 |
| **E4-4** | 预算要不要进 `--check` 退出码 | (a) 维持「仅报告」（现状） (b) 变成第 5 条门禁 | (b) 会立刻红（49.56 > 35）逼着先做真减；但 **CI 不构建 Flutter** ⇒ 先解决口径，否则会退化成「缺 = 通过」的新假绿 | (b) 之前必须先让 CI 真的构建产物，并让「目录缺失」也算红（不是「未构建 = 跳过」） |

---

## 6. 复现命令与原始片段

```bash
cd /home/skystar/Live2D-Ai
du -sh shell/flutter/build/web                                    # 50M（块占用）
find shell/flutter/build/web -type f -printf '%s\n' | awk '{s+=$1} END{print s}'   # 51962935
find shell/flutter/build/web -type f | wc -l                      # 66
find shell/flutter/build/web -type f -printf '%s %p\n' | sort -rn | head -25
du -sh shell/flutter/build/web/canvaskit/*
grep -o -E '(canvaskit|skwasm|wimp|webparagraph)[A-Za-z_./-]*' shell/flutter/build/web/flutter_bootstrap.js | sort | uniq -c
grep -o -E '/app/canvaskit/[A-Za-z/._-]+' docs/verification/evidence-2026-10-06/net.json | sort | uniq -c
node -e '<逐段解析 wasm + 丢弃 custom 段 + WebAssembly.validate/compile 复验>'   # 见 §2.3
find crates/l2d-wasm-demo/dist -type f -printf '%s %p\n' | sort -rn
ls -la $HOME/flutter/bin/cache/flutter_web_sdk/canvaskit/          # 与产物同集合 ⇒ 重建会拷回
grep -n 'FLUTTER_WEB_BUDGET_MIB\|WASM_DIST_BUDGET_MIB' xtask/src/code_stats/mod.rs     # 164 / 168
grep -n 'gate(' xtask/src/code_stats/gates.rs                      # 四条，无 artifacts
grep -n 'code-stats' .github/workflows/*.yml                       # 只在 pr-checks.yml（Rust job）
trunk --version                                                    # trunk 0.21.14
command -v wasm-opt wasm-strip wasm-pack                           # 全部 absent（void 输出）
```

---

## 7. 本文件明确**没有**做的事 / 未实测项

- **没有删/改任何产物**：`shell/flutter/build/web` 与 `crates/l2d-wasm-demo/dist` 一个字节没动（只在 `/tmp` 里写过剥名表的复验副本）。
- **没有改** `xtask/src/code_stats` 的预算常量（§3.1 只是只读定位）。
- **未实测** `wasm-opt` / LTO / `opt-level="z"` 的收益（工具本机 absent）；本文对 dist 的真减路**不给估算数字**。
- **未实测** `Trunk.toml` 的 `[[hooks]]` post_build 是否能拿到 `wasm-bindgen` 产物之外的路径（机制可行性按 trunk 0.21.14 判断，**未跑通一次**）。
- **未实测** `strip = true` 是否真能去掉 wasm 的 `name` 段。**已实测的是**：该段去掉后模块仍 `validate` + `compile` 通过（§2.3）；「怎么让它在该构建里被去掉」需实施时验。
- **未跑** prune 脚本（它还不存在）：A 档删除清单是按字节统计**推算的执行效果**，实际执行后应回填真实数字。
- 数字口径：`51 962 935 B (49.55 MiB)` 与 `5 606 642 B (5.35 MiB)` 均为本次实测；与交接文档的「50 MiB / 5.4 MiB」（`du` 口径）一致，
  差异来自 `du` 的块占用 vs 字节和，已在 §1.1 说明。


