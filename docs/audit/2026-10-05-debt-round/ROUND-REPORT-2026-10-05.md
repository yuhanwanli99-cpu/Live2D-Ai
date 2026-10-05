# 0.2.x 债轮 · 轮次报告（2026-10-05）

> 编排：Lead ｜ 唯一工作树：`/home/skystar/Live2D-Ai-fe` ｜ 分支：`chore/debt-round-2026-10-05`（→ `main` `--ff-only`）
> 纪律：worker 不做破坏性 git、不提交；**共享产物（`shell/flutter/build/web`、`target/**`）唯一写者 = Lead**；
> 每条结论要「命令 + 原始输出」；跑不了就写 blocked，**不许伪造绿灯**。

## 0. 一句话

四条工作流（CI 红线门禁 / Rust 依赖与密钥接缝 / Flutter 真 bug 与假绿灯 / **首次真实浏览器验收**）+ 仓库治理全部收口；
**没有升版本号、没有发 release**（维护者肉眼 13 项未做，版本号留到下一轮）；已知缺口写进
`docs/plans/NEXT-ROUND-main-2026-10-05.md`。

## 1. 团队与波次（共享任务板）

| 任务 | owner | 结果 |
| --- | --- | --- |
| task-1 W1-A 真实浏览器验收取数 | browser-qa | **completed**：38 项判定 + 55 份证据 + 可重跑探针 |
| task-2 W1-B CI 三条红线门禁 | ci-gates | **completed**：3 条门禁 + 单一真源探针 + 变红自证矩阵 |
| task-3 W1-C Rust 债 | rust-debt | **completed**（含追加轮）：D4 25→22 + F-0062-01 + 端点级清除语义 |
| task-4 W1-D Flutter 债 | flutter-debt | **completed**：F-0007-2 + F-0034-01 + D6 假绿灯 |
| Lead | lead | 环境与证据链、仓库治理、冻结重建、独立复核、提交与合流、本报告 |

## 2. 交付（逐条）

### 2.1 CI 红线门禁（F-0048-01 / F-0049-01 / F-0050-01）
详见 `CI-GATES-REPORT.md`。要点：
- 新增 **`scripts/lib/cdn_probe.sh`**（唯一真源）；`ignite.sh` 与 `ignition-precheck.sh`（**同一缺陷的第二份副本**）都 source 它。
- 判据改为**生效路径**：`flutter_bootstrap.js` 的 `useLocalCanvasKit` / 生效 `canvasKitBaseUrl`；**文件缺失判红**、**缺库 exit 2**。
- `ignite.sh --check` 现在扫 **4 个产物文件**；新增 `--check-dir`（不需要服务）。
- CI：`flutter-web-offline-artifacts`（PR）、`web-offline-serve-check`（nightly 真二进制 + 60s 轮询）、**新增 `secret-scan.yml`**。
- `check_public_secrets.py`：修掉与事实相反的 docstring（Gitleaks 不存在）、扫描面 **292 → 937 文件**（含 `shell/`、`docs/`）。
- **对账本的纠正**：无标志坏构建里 `main.dart.js` **会**命中 1 ⇒ 账本「旧探针很可能仍 0 命中」的预测**不成立**（结构缺口仍成立）。

### 2.2 Rust 债
详见 `RUST-DEBT-REPORT.md`。
- **D4**：desktop 依赖 **25 → 22**（`futures-util` → `Response::chunk`；`serde_with` → crate 内 14 行 `double_option`；`tokio-util` → 改由 runtime 转发 `CancellationToken`）。**棘轮已在同一 commit 收紧到 22**（Lead 执行）。
  **口径**：`tokio-util` 只去 **direct edge**，仍在依赖树里（经 runtime）——不得读成「依赖已完全消失」。
- **F-0062-01**：宿主 `reload_config` 改**按键合并 + secret 保留**（键名取**静态 `settings_spec`**，停用 Mod 也受保护）；**声明 secret 的键显式空串 = 删键**（唯一删除语法），非 secret 空串照存（与 `PUT /api/v1/env` 的「空=清除」刻意区分）。
- 测试 **+9**（498 → **507**），含**遍历 `AVAILABLE_MOD_FACTORIES`** 的「全部在册 Mod 都保住 secret」与端点级「显式清除 / 非 secret 空串照存」两条；两次定点突变各自可红。

### 2.3 Flutter 债
详见 `FLUTTER-DEBT-REPORT.md`。
- **F-0007-2**：实为**两层**——① busy 分支没接 `onSend`；② `_send` 读的是**已 `clear()` 的输入框**。修法：`interruptAndResend`（先 await stop 再 await resend）+ `lastUserText()` + `send(echoUser:false)`；**真泵**回归 15 条。
- **F-0034-01 / F-0074-01**：`DisplayPrefs.sameAs`（id-only）→ `!=`（含逐图样式），`==`/`hashCode` 恢复对称；新增 11 条（含真泵 `AppearanceSection`）。
- **D6**：豁免表 → **路径前缀 + 处数上限**（前缀零命中 = 死豁免判红）；已删 crate 的硬编码夹具 → **读 `main.rs` 唯一真源的 `registeredModIds()`** 真断言；`_stripComments` ×2 合并并**修掉「不认识字符串」这个假绿灯方向的洞**；反复制门禁扩成 5 定义唯一性表。
- 全量 **1564 passed**（基线 1524，**+40**）；临时红证文件已删。

### 2.4 真实浏览器验收（本轮最大的方法论突破）
详见 `docs/verification/v0.2.1-browser-acceptance-2026-10-05.md`。
- **推翻历史结论**「本机无 Linux Chrome、无法从 WSL 驱动」：用 `~/.cache/ms-playwright/chromium-1243`（Chrome for Testing 153）+ CDP 9222 + DSH browser MCP；渲染面要 `--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader`。
- 38 项：**pass 29 / fail 3 / blocked 4 / manual-only 2**（冻结后 Lead 复跑 fail 6，其中 4 条判为探针伪影）。
- **字体兜底实测红**（已知缺口 #1 首次有复现）：产品文案走查 20 步 **0 次 gstatic**；注入 `𠮷`/`🀄`/`𝄞` → **真的出网** notosansjp / notocoloremoji / notomusic；**阳性对照 +1**（证明探测不瞎）。
- **F-0050-01 的正面回答**：`useLocalCanvasKit:true` + 40 条请求**全同源** ⇒ 主链**不会断网白屏**。
- **决定性命中**：无头截图**拍不到 WebGPU canvas**（`stageColor` 强改 `#ff0000` 后逐像素差异 = 0）⇒ 舞台像素级 = manual-only。

### 2.5 仓库治理
详见 `REPO-HYGIENE.md`。worktree 3 → 2、分支 12 → 11、死树 136 脏项 → **0**、回收 **8.7 G**、唯一未覆盖的遗留分支补 bundle。

## 3. 门禁（冻结树，**Lead 亲自复跑**）

| 门禁 | 结果 |
| --- | --- |
| `cargo test --workspace --all-targets` | **1311 passed / 0 failed** |
| `cargo test --doc --workspace` | 3 passed |
| `cargo fmt --all -- --check` | clean（exit 0） |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0 warning**（exit 0） |
| `cargo run -p xtask -- rust-ratio` | **96.1382% PASS**（86410/89881；dart 69,425 行豁免） |
| `cargo run -p xtask -- code-stats --check` | **PASS**（>500 48/48 · >1000 4/4 · deps **22/22**） |
| `cd shell/flutter && flutter analyze` | **No issues found** |
| `cd shell/flutter && flutter test` | **1564 passed / 0 failed** |
| `./scripts/ignite.sh --check` | **四项 ok**（现扫 4 个产物文件，含 `flutter_bootstrap.js`） |

## 4. 独立复核（Lead，不采信自述）

| 复核对象 | 做法 | 结果 |
| --- | --- | --- |
| ci-gates 门禁 | 自己跑 `--check`（活服务）与 `--check-dir`（**删 main.dart.js 的 /tmp 副本**） | 绿 **exit 0** / 红 **exit 1**（「文件缺失判红，不静默跳过」） |
| ci-gates 单一真源 | `ls scripts/lib` + `grep source` 两个脚本 | 成立；`AGENTS.md` diff = **3 insertions**（只加表 3 行） |
| ci-gates 密钥扫描 | 自己跑 `check_public_secrets.py` | `ok (937 files scanned)` exit 0 |
| flutter-debt | 自己跑 `flutter analyze` + `flutter test` | **与自述逐字一致**（0 issue / 1564） |
| rust-debt | 自己跑 `-p live2d-ai-desktop -p live2d-ai-runtime --all-targets` | 全 ok |
| 棘轮 | 收紧后跑 `--only deps` | PASS，退出码 0 |
| QA 的 4 条 fail | 注入红色背景 → reload → 14s → **整页**截图 → 分区测量 | 右区 **redness 22.97**（>20）⇒ 背景确实上屏；4 条 fail 判为**探针伪影** |

## 5. 事故与口径更正（诚实栏）

1. **`build/web` 被一次未完成的 `flutter build web` 清空后中断（18:28:35）**：`main.dart.js`/`flutter_bootstrap.js` 等缺失、无 flutter 进程存活。核对后：Lead 与 ci-gates 都有不在场证据（Lead 当时只动了 `-product` worktree 与死树 `target/`；ci-gates 的构建全在 `/tmp` 且 `.dart_tool/flutter_build` 无 18:28 目录）⇒ 按「**未归因的工作区竞争**」记录。**防御**：共享产物唯一写者 = Lead；任何变红实验一律用 `/tmp` 副本。
2. **探针伪影几乎把 4 条产品通过误报成 fail**：`Page.captureScreenshot` **带 clip** 时可能返回全白图，而不带 clip 的整页截图同刻正常；`waitForPaint` 用带 clip 的截图做判据 ⇒ 被伪影击穿。已写进验收报告 §7.2 与下一轮清单。
3. **账本预测被实测推翻**（见 §2.1）——按项目纪律「以源码/实测为准」，不留旧说法。
4. **rust-debt 自查顶破门禁**：第一版把路由测试直接加进 `mods_routes.rs`（957 → 1012）顶破 `>1000`，作者自行外提测试并恢复 4/4；该自曝过程保留在报告里。
5. **本机二进制曾是 stale**：首轮验收被测后端 banner = `0.2.0-rc.7`（源码已是 0.2.0）；冻结后已重建为 `0.2.0`，终态数字以重建后为准。
6. **无头环境的两条硬限制**（不是产品缺陷）：WebGPU canvas 不进截图；`fonts.gstatic.com` 兜底由引擎内置，靠构建**消除不掉**。

## 6. 未做 / 留给下一轮

见 `docs/plans/NEXT-ROUND-main-2026-10-05.md`（字体兜底裁决、维护者肉眼 13+7 项、D2 结构拆分、探针伪影修复、产物预算、D5 文档减量、审计账本处置、R1 功能等）。

## 7. 复现命令

```bash
cd /home/skystar/Live2D-Ai-fe
git log --oneline -5
cargo test --workspace --all-targets && cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -q -p xtask -- rust-ratio && cargo run -q -p xtask -- code-stats --check
cd shell/flutter && flutter analyze && flutter test
cd ../.. && ./scripts/ignite.sh &                 # 另开终端起服务
./scripts/ignite.sh --check
PROBE_CDP=http://127.0.0.1:9222 node scripts/browser_probe.mjs all
```
