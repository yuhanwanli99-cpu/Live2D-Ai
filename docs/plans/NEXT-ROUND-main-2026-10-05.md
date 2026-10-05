# 下一轮清单（**在 main 上开工**）· 2026-10-05 债轮交接

> 本轮（`chore/debt-round-2026-10-05` → `main`）已把 CI 红线门禁、Rust 依赖与 Mod 密钥接缝、
> Flutter 真 bug 与假绿灯、仓库治理收口，并**首次跑通真实浏览器验收**。
> 本文只写**没做完的**，每条给「现状 / 为什么没做 / 可执行规格 / 验收判据」。
> 纪律不变：先回源码重读（行号会腐烂）、**不许伪造绿灯**、源码 ≤500 行、测试只增不减。

## P0 · 需要维护者裁决或肉眼

| # | 事项 | 现状 | 规格 / 判据 |
| --- | --- | --- | --- |
| **N1** | **运行期字体兜底（已知缺口 #1）** | 本轮有了**实测复现**：产品文案走查 20 步 0 次出网；注入 `𠮷`(U+20BB7)/`🀄`(U+1F004)/`𝄞`(U+1D11E) → **真的出网** `fonts.gstatic.com`（notosansjp / notocoloremoji / notomusic）。`main.dart.js` 里那条 `fonts.gstatic.com/s/` 是 **CanvasKit 引擎内置**，靠构建消除不掉 | 三条路**选一**：①自托管更大的兜底字体子集（要动字体二进制，体积代价）；②引擎侧关网络回退（要动 `lib/**` 或构建配置，需确认 Flutter 3.47 有无公开开关）；③**明确接受**「仅用户输入的子集外字符会出网」并写进 README/规格（当前实际行为）。**判据**：任选其一后，`browser_probe.mjs fonts` 的注入断言与该裁决一致（要么 0 出网，要么明确标注接受） |
| **N2** | **维护者肉眼 13 项「只能人工」+ 7 项「需真服务/Windows」** | `docs/verification/v0.2.0-checklist.md` 全未勾（本轮自动化覆盖了可自动化的那部分，见验收报告） | 在 `http://127.0.0.1:18080/app/` 上按清单落勾；重点：**舞台像素级**（无头拍不到 WebGPU canvas）、真实音频、读屏器 |
| **N3** | **背景系统像素级验收** | 本轮探针用**带 clip 的截图**判据被伪影击穿（见下 P1-N4）；Lead 用**整页截图**已证「红色背景 redness 22.97 > 20」⇒ 产品通过 | 真机逐档看 `cover/contain/stretch/tile` + 轮播 + 刷新保留 |

## P1 · 工程债（可立即开工）

| # | 事项 | 现状 / 规格 | 判据 |
| --- | --- | --- | --- |
| **N4** | **`scripts/browser_probe.mjs` 的截图判据修复** | `Page.captureScreenshot` **带 `clip`** 时可能返回全白（5.8 KB vs 有内容 52 KB），不带 clip 的整页截图正常；`waitForPaint()` 用带 clip 的截图做判据 ⇒ 被伪影击穿 | ①判据改**整页截图 + 进程内裁剪**；②`waitForPaint` 返回 `null` 时该场景判 **blocked（环境）**，**不得降级成产品 fail**；③背景注入改用 **IndexedDB 真字节**或走真实「添加图片」UI（当前注入 `dataUrl` 会被 v0.2.0 的水合迁移掉，读回已是 `len:0`）。验收：同一场景连跑两次，`5a/5b/5d/offline` 不再出现「全白截图」型 fail |
| **N5** | **D2 结构拆分**（唯一还挂着的结构硬指标） | `src` 生产 `.rs >1000` 仍 **4**（`mod_registry`(已拆成目录但仍计) / `chat_routes` 1086 / `external_routes` **1202** / `persona/lib.rs` 1004）；Dart `>800` 仍 **7**（`dev_tools_section` / `appearance_background` / `main` / `display_prefs` / `app_shell` / `settings_models` / `tokens`）。`chat_controller.dart` 本轮 650 → **684** | 判据改为 **`src` 生产 `.rs >1000 = 0`**。**配方**：`code-stats` 按**文件总行数含内联测试**计 ⇒ 拆 Rust 必须**同时**用 `#[cfg(test)] #[path]` 把内联测试移出（先例 `web_api/voice_routes.rs`）；Dart 优先 `part`/`part of`（零可见性改动，先例 `main.dart` 4 个 part）；令牌搬迁必须同 commit 改 `design_tokens_lint_test.dart` 的**前缀**豁免表 |
| **N6** | **D6 续：剩余假绿灯** | 本轮清了 3 类（豁免表 / 已删 crate 夹具 / 词法器重复）。剩余：`_stripComments` 类**同名不同义**的其它消费点（若有）、以及任何「硬编码夹具 + 对象已删」的残留 | 每条要有「**现在它能红**」的自证 |
| **N7** | **CI 门禁在真 runner 上首跑** | 本机无 runner；nightly 的 `cargo build` 是否缺 `libasound2-dev` **未验证**（已写「探测到缺才装」的条件步） | 首次 nightly 的原始日志；红则修 |
| **N8** | **`AUDIT-REPO/`（942 文件 / 6 MB，未入库）** | 只存在于 `-fe` 工作区，**未跟踪、无备份**；被本轮引用（F-0048/0049/0050/0062/0034 等） | 二选一：并入 `docs/audit/` 或归档到 `/home/skystar/backup-*`；同时清点其中**未闭环的 P1**（本轮只修了与前端/门禁相关的几条） |

## P2 · 度量与文档

| # | 事项 | 现状 | 规格 |
| --- | --- | --- | --- |
| **N9** | D4 剩余 | 产物 `build/web` 47M（预算 ≤35M）、wasm `dist` 5.3M（≤4M）；`tokio-util` 仍是传递依赖 | 产物预算要么达成、要么**明文改预算 + 理由** |
| **N10** | D5 文档减量 | docs 总量 72,767 → 目标 ≤45,000（**必须用「不含 `docs/audit/**`」那一行**判定）；归档不减总量，只能真删/真合并 | 逐目录给减量账 |
| **N11** | 文档残留 | `docs/design/web-ui-spec-v3.md:2087` 仍写 `kNamedTimingFiles`（本轮已改成前缀+配额）；`docs/architecture/ARCHIVED-mods.md:195`、`pet-desktop-mod-v0.md`、`docs/plans/parallel-mods/*` 仍指旧文件名 `pet_desktop_state_test.dart` | 全仓 grep 旧标识符，逐处改或标历史 |
| **N12** | 产品语义待裁 | ① 无 code 时的「重试」分支同样走 `_send()` 读输入框（空即 no-op）——需先定「重发上一条 vs 发送框里新输入」；② `ModsSection` 的「运行态（只读）」通用块在**在册 5 个 Mod 上不可达**（各有专用面板），是否单独立项 | 先裁决再动手 |

## P3 · 功能（本轮未碰）

| # | 事项 | 说明 |
| --- | --- | --- |
| **N13** | **R1 正文帧带 `sentence_seq`（朗读高亮）** | 全项目**唯一「不做就永远做不出来」**的一条：音频帧已有 `sentence_seq`，正文帧没有，纯前端无法映射 |
| **N14** | 会话记忆后端 / 动作-语音选型 | 真源 `docs/plans/PLAN-actions-voice-memory-2026-09-15.md`；**实施前先改 AGENTS 的 director 台账措辞** |
| **N15** | R3/R4（舞台图解码状态帧、capabilities 能力发现） | 排障成本相关，可选 |

## P4 · 仓库收尾（本会话结束后可做）

| # | 事项 | 现状 |
| --- | --- | --- |
| **N16** | 移除死树 `-Ai` worktree | 已**冻结为 0 脏**且保档在 `/home/skystar/backup-2026-10-05/`；本会话因它是 Lead 的工作目录而**未删**。命令：`git -C -fe worktree remove --force /home/skystar/Live2D-Ai` + `git branch -d mod/persona-polish` |
| **N17** | `dev/integrity` / `dev/node-p1-p2` 裁决 | 与 `docs/releases/v0.3.0.md`（历史草案）的版本线矛盾相关；bundle 已覆盖，可降级为档案 |
| **N18** | 是否推 `origin` | 本地 `main` 领先 `origin/main` **124** 提交（远端停在 rc.1）；历史口径「不推远端」，**需维护者明确解除**才推 |
| **N19** | Gitleaks | 本轮**不补**（多一个第三方 action = 多一条供应链面）；docstring 已如实写「没有」。要补则单独一轮 |

## 附：本轮新增的可复用工具

| 工具 | 用途 |
| --- | --- |
| `scripts/browser_probe.mjs` | 零依赖 CDP 探针：`files` / `fonts` / `stage` / `settings` / `themes` / `bg` / `offline` / `render` / `all` / `summary` |
| `docs/verification/evidence-2026-10-05/png_stats.py` | 纯 stdlib PNG 统计（redness / 亮度分位 / 色度 / 主导色 / `--crop` / `--diff`） |
| `scripts/lib/cdn_probe.sh` | 离线红线探针的**唯一真源**（`ignite.sh` 与 `ignition-precheck.sh` 共用） |
| 启动渲染面的浏览器 | `~/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome --headless=new --no-sandbox --enable-unsafe-swiftshader --enable-unsafe-webgpu --use-webgpu-adapter=swiftshader --remote-debugging-port=9222` |
