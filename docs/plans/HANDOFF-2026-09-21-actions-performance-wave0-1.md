# 交接：动作/表演链路修复（Wave 0 + Wave 1 已交付，Wave 2–5 未开始）

> 交接人：本轮编排者（orchestrator）。日期：2026-09-21。工作树：`/home/skystar/Live2D-Ai-l1`（分支 `mod/l1-product`）。
> **未做任何 git 提交**（全程禁止 `worktree/checkout/switch/stash/reset/commit/clean`）：所有改动都是工作树里的未提交 WIP。
> 本文件是「接手即用」的入口；规格真源仍以 `docs/plans/RESEARCH-actions-director-audit-2026-09-21.md` 与两份提示词文件为准。
>
> **⚠ 先读哪一份**：维护者另有一份 **round 级**交接 `docs/plans/HANDOFF-2026-09-21-actions-performance-round.md`
> （四件事 / 已冻结裁决 / 波次交付 / 未完成清单 / 有意不改项）。**本文件是编排者侧的明细补充**，只加它没有的东西：
> 环境与浏览器配方、门禁**原始数字**、逐块**文件清单与所有权审计**、A–E 裁决的落地账。两份冲突时**以维护者那份为准**。

---

## 0. 一句话状态

**Wave 0（W1/W2/W3/W6/W8 五块）与 Wave 1（W4/W5/W8b/W2b 四块）已全部交付、独立核验、门禁全绿、产物已重建并且服务已用新二进制重启。**
**Wave 2–5（W7 → W9 → W10 → W11）尚未开始**，四块是**同一条链路的递进改造，必须串行**。
当前 `git status --porcelain` = **189** 条（起点 181；含本轮新建文件、波内改动，以及**两份交接文档**：维护者那份 `-round.md` 与本文件 `-wave0-1.md`）。

---

## 1. 环境事实（下次开工前必读，都是实测）

| 事项 | 事实 |
|---|---|
| 分支 / 脏改动 | `mod/l1-product`，**187** 条未提交（含本文件） |
| **`flutter` 不在非交互 shell 的 PATH 上** | `which flutter` 为空；真身在 `/home/skystar/flutter/bin/flutter`（Flutter 3.47.3 / Dart 3.13.3）。跑 Flutter 命令前必须 `export PATH="$PATH:/home/skystar/flutter/bin"`（`scripts/ignite.sh` 自己会 export，裸 shell 不会） |
| **服务当前在跑** | `pid 64616`，`./target/debug/live2d-ai-desktop --web --http-port 18080`，cwd = 本工作树；`GET /` = 302、`GET /app/` = 200 |
| 重启服务 | `cd /home/skystar/Live2D-Ai-l1 && ./scripts/ignite.sh --port 18080`（它自己 pkill 旧进程；改 Rust 后必须先 `cargo build -p live2d-ai-desktop --bin live2d-ai-desktop`） |
| 体检 | `./scripts/ignite.sh --check --port 18080`（四项：/ →302 /app/、/app/→200、index.html 与 main.dart.js 都不含 `gstatic.com/flutter-canvaskit`） |
| **TTS 端点不在线** | `[tts] base_url = http://127.0.0.1:8080/v1`，`curl .../models` → **000**。起来了要补跑 §5.8 端到端（真实 PCM 帧 + `turn_state=completed`） |
| `live2d-ai.toml` / `.env` / `mods.json` | gitignored 运行时配置。`live2d-ai.toml` 已由 W2 改（`[action] body 0.80`），**热重载已验证生效**；基线备份在 `/tmp/live2d-ai.toml.baseline-20260921-205756`（md5 `6da460c676a18a2e9f6c61b793a71010`） |
| `dev_mode` | 我做过一次**内存期** `PATCH /api/v1/settings {"dev_mode":true}`（`persisted:false`，**没写盘**）。开发者面板需要它 |

### 1.1 无头浏览器配方（本轮新增能力，W7/W9/W10/W11 的浏览器核验都靠它）

本机**没有系统 Chrome**，但 Playwright 的 chromium 在 `~/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome`；
**默认 headless 拿不到图形适配器**（wgpu 报 `gl support not compiled in, webgpu found no adapters`），
**必须把自带的 SwiftShader Vulkan ICD 指给它**才拿得到 WebGPU（下面这条**单行**可直接跑，用后台 job 起）：

    CD=~/.cache/ms-playwright/chromium-1243/chrome-linux64; VK_ICD_FILENAMES=$CD/vk_swiftshader_icd.json LD_LIBRARY_PATH=$CD $CD/chrome --headless=new --remote-debugging-port=9222 --remote-allow-origins=* --no-sandbox --disable-dev-shm-usage --disable-gpu-sandbox --in-process-gpu --enable-unsafe-swiftshader --enable-unsafe-webgpu --enable-features=Vulkan,DefaultANGLEVulkan --use-angle=vulkan --use-gl=angle --user-data-dir=/tmp/chrome-w0c about:blank

MCP 浏览器工具随后能连上；注意三点：
1. `list_pages` 只有文本，**pageId 是数字**（`navigate_page` 要 `{pageId: <number>, url}`，传字符串会报参数错）；
2. Flutter 是 CanvasKit：想按**文字**点控件，先启用语义树 —— `document.querySelector('flt-semantics-placeholder').click()`，
   之后按 `aria-label` / `textContent` 找 `flt-semantics`（**分区导航项只有 aria-label，没有 textContent**）；
3. **读渲染面 HUD 是 DOM**（同源 iframe）：`document.querySelector('iframe').contentDocument.querySelector('#status').textContent`。
   HUD 关键字段：`preset: face=<id> gesture=<id> face_intensity=<x> src=<s> <剩余ms> | scale h../b../e.. | msg=N/M [最后一种消息]`。
   HUD 的毫秒是**剩余**毫秒（可用来判断某条 preset 是否被重新起算/续期）。

**抓「前端到底给渲染面发了什么帧」的桩**（我验 W4 撤销时用过，很有效；传输实现见 `shell/flutter/lib/live2d/live2d_host_web.dart:78`）：

    const w = document.querySelector('iframe').contentWindow;
    const orig = w.postMessage.bind(w);
    w.postMessage = (m, o, t) => { try { window.__frames.push(JSON.parse(m)); } catch(e){} return orig(m, o, t); };

---

## 2. 波次图（哪些做了、哪些没做）

| 波次 | 块 | 状态 |
|---|---|---|
| W0 | W1 文档对齐 / W2 幅值重标定 / W3 调试面板 / W6 令牌走 secrets / W8 导演口径 | **已交付 + 我独立核验** |
| W1 | W4 撤销语义 / W5 Rust 卫生 / W8b 文档漂移三处 / W2b 标定口径+T3 | **已交付 + 我独立核验** |
| **W2** | **W7 幅度下发通道整修** | **未开始**（前置已就绪：W2/W4 都收口了） |
| W3 | W9 单一驱动者 | 未开始（等 W7） |
| W4 | W10 动作协议字段化 | 未开始（等 W9） |
| W5 | W11 导演 AI 侧 | 未开始（等 W10） |

---

## 3. 门禁与产物证据（原始数字，来自我串行的收口跑）

### 3.1 Wave 0

    cargo test --workspace --all-targets  -> EXIT=0；TOTAL passed=1394 failed=0
    cargo test --doc --workspace          -> EXIT=0（runtime 3 passed / 0 failed）
    cargo fmt --all -- --check            -> EXIT=0
    cargo clippy --workspace --all-targets -- -D warnings -> EXIT=0（0 warning）
    cargo run -p xtask -- rust-ratio      -> 97.1829%（90661/93289）PASS
    flutter analyze                       -> No issues found!（2.7s）
    flutter test                          -> +1036: All tests passed!

### 3.2 Wave 1

    cargo test --workspace --all-targets  -> EXIT=0；passed=1394 failed=0（另有一次串行干净重跑同值）
    cargo test --doc --workspace          -> EXIT=0（3 passed）
    cargo fmt --all -- --check            -> EXIT=0
    cargo clippy --workspace --all-targets -- -D warnings -> EXIT=0（0 warning）
    cargo run -p xtask -- rust-ratio      -> 97.1830%（90663/93291）PASS
    cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo -> EXIT=0
                                             （4 条既有 dead_code 警告：PRESET_IDS / frame_params / base_params / active_in）
    flutter analyze                       -> No issues found!（3.4s），exit 0
    flutter test                          -> +1041: All tests passed!（W0 是 1036，+5 = W4 新增）
    flutter build web --release --base-href /app/ --no-web-resources-cdn -> exit 0
    trunk build（crates/l2d-wasm-demo）    -> EXIT=0
    ./scripts/ignite.sh --check --port 18080 -> 四项全 [ok]（CHECK_EXIT=0）

### 3.3 产物 mtime（红线三项 + wasm §5.12）

| 产物 | mtime | 对比源 |
|---|---|---|
| `shell/flutter/build/web/main.dart.js` | 2026-09-21 **22:20:39** | 最后被改 `.dart` = `test/action_cue_test.dart` **22:17:06**；`-newer` 计数 **0**；`gstatic` 计数 **0** |
| `crates/l2d-wasm-demo/dist/index.html` | **22:19:00** | 最后被改 wasm `.rs` = `preset/scales.rs` **22:18:19**；`-newer` 计数 **0** |
| `target/debug/live2d-ai-desktop` | **22:21:46** | 服务 22:21 起用新二进制（pid 64616） |

> ⚠ `crates/l2d-wasm-demo/dist/` 是 gitignored 构建产物：任何改到 `crates/l2d-wasm-demo/**/*.rs` 的波次都必须
> `cd crates/l2d-wasm-demo && trunk build`，并复核 `dist/` mtime 晚于最后一个被改的 `.rs`（ORCHESTRATOR §4 / §5.12）。
> 这条纪律是本轮**我踩出来的**：W0 时 wasm 产物陈旧，而原红线只写了 `.dart`。

### 3.4 日志（**在 /tmp，可能被清**；关键数字已抄到上面）

`/tmp/w0-gates.log`、`/tmp/w0-flutter.log`、`/tmp/w0-wasm.log`、`/tmp/w1-gates.log`、`/tmp/w1-flutter.log`、
`/tmp/w1-cargotest-clean.log`、`/tmp/w1-wasm.log`、`/tmp/baseline-deadzone.txt`（开工前独立复算，
md5 `1fc4e555cfaea059bf47b9f30513fda4`）、`/tmp/verify_w2.py`、`/tmp/verify_t3.py`、`/tmp/verify_w1.py`、
`/tmp/audit_ownership.py`、`/tmp/deadzone_recompute.py`、`/tmp/l1-blocks/W*.txt`（IMPL-PROMPTS 11 个块的**逐字**提取）、
`/tmp/l1-blocks/_PREAMBLE.txt`。

---

## 4. 逐块状态与文件（W0 + W1 已交付）

| 块 | 已改文件（本轮） | 关键结论 / 测试 |
|---|---|---|
| **W1** | `AGENTS.md`、`docs/README.md`、`../legacy/plans/PRODUCT-L1-GOALS-2026-09-15.md`、`docs/architecture/action-packs-v0.md`(§10) | 新增「动作与表演的**现行状态**（2026-09 实测）」五条事实+真源；`:112` / `:27` / `:137` / `:450` 的假断言改成带现状更正；GOALS `:6` / `:43` 加指针。**我另修了 R2 泄漏**（见 §7.2） |
| **W2** | `preset/{mod,scales}.rs`、`preset/tests/{packs,assets,mod}.rs`、`assets/actions/presets.json`、`runtime/src/settings.rs`、`settings/{view,patch,patch_tests}.rs`、`live2d-ai.toml(.example)`、`web_api/settings_routes/tests.rs`、Dart `api/settings_models.dart`、`settings/sections/appearance_section.dart`、`test/{action_scales_preview,settings_api}_test.dart` | 见 §5。新增 9 条 Rust 回归 + 1 条 Dart |
| **W3** | `settings/sections/dev_tools_section.dart`、`settings/preset_labels.dart`、`test/{developer_section,preset_labels}_test.dart` | `_overlayFace` 默认 **false**；`_liveExpressionId` 到点即 none；删掉两处硬编码 id 列表（`kDebug*`）；ttl 从**舞台回执 PresetStatus.ttl** 派生（`design_tokens_lint` 禁止 `lib/settings/**` 写死 `Duration(milliseconds:)`）。+4 测试 |
| **W6** | `mod-external-input/{Cargo.toml,src/lib.rs}`、`web_api/{external_routes,voice_routes,cli_entry}.rs`、`voice_routes_tests{,_token}.rs` | 三处令牌改走 `secrets::lookup`（`.env` 快照 > 进程环境）；优先级链**顺序未变**；`mod-external-input -> runtime` 依赖（与 mod-memory/director 同款，**无环**）；E6 选 **(a)** 统一强口径。+3 注入式测试 |
| **W8** | `docs/architecture/{director-rfc,director-mod-v0,performance-layer-v0}.md`、`assets/actions/preset_labels.json` | 文首「现行状态与已过时红线」三行标注被推翻；清掉「遗留并行实现/不再是主路由/辅助用户」（**0 命中**）；Q1 三处一致标**未定** |
| **W4** | `lib/main.dart`、`lib/live2d/live2d_stage.dart`(+62)、`docs/architecture/performance-layer-v0.md`(§2.2)、`test/action_cue_test.dart`(88→192) | 新增纯逻辑 `DirectorPresetGate`（`live2d_stage.dart:92-115`）：**先 seq 去重、再 none 归零**；`main.dart:484-515` 的 `_applyDirectorPreset` 落 `applyPreset('none', source:'director')`。+5 测试（4→9） |
| **W5** | `runtime/src/lib.rs`、`performance/{client,mod,tests}.rs`、`mod-director/staging_http.rs`、`mod-memory/summary_http.rs`、`l2d-wasm-demo/src/{main.rs,preset/table.rs,web/surface/input.rs,preset/scales.rs}`、`runtime/src/settings.rs` | `join_endpoint` **4->3 份**（同 crate 重复已删）；`:497` 注释改实情；`input.rs` 注释改实情；`preset.rs` -> `preset/`；**第二轮**修 `scales.rs` T3 注释把 nod 与 shake 并档的数值错误 |
| **W8b** | `docs/architecture/{core-chain-baseline,directory,mod-product-chain}.md` | 「零投递/不驱动动作/delivered:false」全部改带「当时（…）」限定 + 现状更正；**第二轮**收掉同族假断言「动作在产品路径上不存在」（严格 grep **0 命中**） |
| **W2b** | `docs/architecture/action-packs-v0.md`(新增 §11，184→317 行)、`assets/actions/presets.json`(**仅 _doc**) | §11 = 公式/上限/出厂/量程[0.2,2.2]/三条独立走满口径/T3 原样+23 组展开/死区两种口径/身头比/morph 例外 |

### 4.1 文件所有权审计

`/tmp/audit_ownership.py` 逐个 mtime 归因：**本轮 35+ 个改动文件每个只映射一个 owner，无 UNOWNED、无并行双写**。
**两处「任务文字 vs 归属清单不一致」（按任务文字执行，已记账）**：

- **W5** 的块内任务文字明确要求「两个 Mod 必须在各自注释里写明与 `runtime::join_endpoint` 同语义」，
  但【文件归属】没列 `mod-director/src/staging_http.rs` 与 `mod-memory/src/summary_http.rs` —— W5 按任务文字改了这两个（+ `performance/tests.rs` 测试改名）。
- **W2** 的改动里 `runtime/src/settings/{view,patch,patch_tests}.rs` 与 `preset/tests/mod.rs` 也不在其清单，属**门禁必需**。

---

## 5. W2 标定（数值真源在哪、怎么核）

**权威数值真源 = `crates/l2d-wasm-demo/src/preset/scales.rs` 顶部注释**；
副本 = `docs/architecture/action-packs-v0.md` §11 与 `assets/actions/presets.json` 的 `_doc`（三处应一致）。

- 公式：`最终值 = clamp_to_channel(表值 × 峰值系数 × intensity × 通道倍率)`；上限 **头 30 / 身 10 / 五官 4（未变）**。
- 出厂倍率 **head 0.75 / body 0.80 / expression 1.0**；`MAX_ACTION_SCALE` **2.5 -> 2.2**；量程 `[0.2, 2.2]`。
- 峰值系数：Single = 1.0；**shake（Oscillate cycles=2）= 0.9285**（回归真算，非写 1.0）。
- 三条验收口径：出厂组合 <= 上限x0.60；scale 走满 <= 上限x0.95；intensity 走满 <= 上限x0.95（**方案 (i)**，`MAX_INTENSITY` 保持 3.0）。
- **两个旋钮同时拉满会钳位**：独立复算 = **23 组**（smile 5 + surprised 6 + nod 2 + shake 2 + look_* 4 + tilt_* 4）；**morph 包 unhappy 不钳位**。
- 主轴死区起点（`上限x0.95/(表值x峰值)`）：**nod/look/tilt 头 2.375、shake 头 2.558、身 2.436/2.623、surprised 眼 3.016 —— 全部 > 2.2 ⇒ 滑条全行程有效**。
  （另一口径「钳死点 `上限/(表值x峰值)`」= 2.500/2.564/2.693/2.762；RESEARCH §2.1 表头已钉明两者相差 5% 是刻意的。）
- 身/头比：六条手势包 **表内 0.3250 / 出厂后 0.3467**（`tilt_*` 表内旧 0.25 -> 已修）。
- **代价（需用户肉眼确认）**：默认观感明显变小 —— look 头 16.50°->**9.00°**、look 身 9.80°->**3.12°**、nod 头 15.00°->**9.00°**、
  nod 身 9.80°->**3.12°**、shake 头 15.32°->**8.36°**、shake 身 9.75°->**2.90°**、tilt 头 12.00°->**9.00°**、tilt 身 5.60°->**3.12°**。
- 复核方式：`python3 /tmp/verify_w2.py`（从源码实时解析，**8 项全过**；改动前自测是 5 项 FAIL，不是盖章器）。
- 活体读数：HUD `scale h0.75/b0.80/e1.00`。

---

## 6. 独立核验进度（ORCHESTRATOR §5 逐条）

| # | 项 | 状态 |
|---|---|---|
| 1 | 死区复算 + 两条旋钮断言 + 两旋钮同拉组合表 | **已做**（`/tmp/verify_w2.py` 8/8；`/tmp/verify_t3.py` 23 组与 W2 逐行一致） |
| 2 | 身/头比（主轴配对）在 [0.30,0.50] | **已做**（0.3250 / 0.3467） |
| 3 | 默认值五处一致 | **已做**（0.75/0.80/1.0 五处两两相等；`MAX_SCALE` 三处 = 2.2） |
| 4 | 临时覆盖不再被冲掉 | **未做**（属 W7 后） |
| 5 | 撤销真的发生 | **已做**（浏览器帧级证据，见 §6.1） |
| 6 | 调试面板不续期 | **已做**（浏览器实测：点表情 -> 3.2s 后 UI「无（已到点）」；到点后点手势 HUD 仍 face=-） |
| 7 | 卫生 grep | **已做**（join_endpoint 4->3；`std::env::var(` 非测试只剩 secrets + XDG/DISPLAY/HOME + LIVE2D_AI_*；三令牌名不再出现） |
| 8 | 端到端不回归 | **部分**：LLM 路径 + text_delta + text_fallback + error(tts_transport) + `settings/test/llm` 的 `ok:true` 都验了；**真实 PCM 帧与 turn_state=completed 因 TTS 下线未取证** |
| 9 | 只切不改字（W11） | 未开始 |
| 10 | 字段化+叠加+hold（W10） | 未开始 |
| 11 | 时间轴统一 stage-clock（W10） | 未开始 |
| 12 | 渲染面产物 mtime | **已做**（dist 22:19:00 > 最后 wasm .rs 22:18:19） |

### 6.1 §5.5 的原始证据（W4 撤销，浏览器实测）

| 轮 | 措辞 | `GET /api/v1/mods/director/state` -> `state.latest` | 前端发往渲染面的帧（我给 iframe 传输打了桩） |
|---|---|---|---|
| 1 | 今天真开心 | `seq 2, emotion happy, intent chat, preset_id "smile"` | `{"type":"preset","payload":{"id":"smile","source":"director","intensity":1.2}}` |
| 2 | 嗯，随便聊聊吧 | `seq 3, emotion neutral, intent chat, preset_id null` | `{"type":"preset","payload":{"id":"none","source":"director"}}` <- **显式撤销** |

HUD 随后：`preset: face=- gesture=- | scale h0.75/b0.80/e1.00 | msg=7/7 [preset]`。
**旧代码在 null/none 时直接 return，第二帧根本不会存在。**

---

## 7. 未完成工作（**接手的重点**）

### 7.1 Wave 2 = **W7 幅度下发通道整修**（前置已就绪：W2 与 W4 都已收口）

W7 的**块原文**在 `docs/plans/IMPL-PROMPTS-actions-performance-round.md`（`===== 复制给 Worker W7 =====`，现约 :331 起）；
**除原文外，必须把维护者已下达的 4 条裁决写进【编排补充】**（这 4 条不在块里）：

| 编号 | 内容 | 精确落点（我已 grep 确认现状） |
|---|---|---|
| **A** | 「叠加基础表情」开关说明文案写实：打开后**每次点手势都会把当前基础表情重新起算（2.6s 重新计时）** | `settings/sections/dev_tools_section.dart` 的文案段（块内点名的 :1596-1601 附近）。**裁决：维持 W3 现状（选项 i）—— 不改 `_applyGesture`、保留旧 Face->Gesture 回归语义**。实测证据：叠加开着时点手势，HUD 剩余毫秒 1274 -> **2388**（被重新起算） |
| **B** | `kExpressionPresetIds` **不删**：两个消费点**优先用标签表、取不到表才回落常量** | `live2d/live2d_stage.dart:436`（ttl 显示）与 `settings/mods/director_panel.dart:104`（通道标签）；定义点 `settings/preset_labels.dart:32`（isExpressionPreset）。报告里贴两处改后 diff |
| **C** | `director_panel.dart` 的「**遗留回退旁路，不是主路径**」文案（docs 侧已清，界面还在） | :21、:51、:55（kDirectorLegacyNotice） |
| **D** | 三处陈旧倍率/量程注释 | `live2d/live2d_stage.dart:156`（原 :93，W4 插入代码后**行号已移位**）「出厂默认（0.75 / 1.4 / 1.0）」、`app/shell_prefs.dart:31`「（0.75 / 1.4 / 1.0）」、`live2d/live2d_bridge.dart:205`「渲染面钳 [0.2, 2.5]」 |

W7 的【文件归属】（块内）：`lib/main.dart`、`lib/live2d/live2d_stage.dart`、`lib/app/shell_prefs.dart`、
`lib/live2d/action_scales_sync.dart`、`lib/app/shell_settings.dart`、`lib/settings/sections/dev_tools_section.dart`(仅 :1596-1601 文案段)、
新建 `test/action_scales_wiring_test.dart`（**加上面 A/B/C/D 涉及的三处**）。
W7 验收（ORCHESTRATOR §5.4）：临时覆盖 -> 改主题 -> 值仍在；「恢复产品设置」一定生效；**要出 HUD `scale_diag` 行原始读数**（用 §1.1 的浏览器配方）。

### 7.2 我在 W0 收口时自行做的两处修正（**请复核，可否决**）

1. **`AGENTS.md` 的 R2 泄漏**：W1 的块内第 6 条要求「原文照抄」的那段裁决文本含「导演/表演属**场景能力**、不属底座能力」——
   而这正是维护者已否定、并明令「**不得引用**」的 R2 推论。我把该段改成「确认口径（酒馆+皮套壳子 / 各功能由现有 mod 矩阵承担 /
   **导演是一个 AI、属产品本体** / 不做架构搬迁 / 输入=用户输入）」+ 一行「该推论已被否定，不得再作为口径或架构建议引用」（现 `AGENTS.md:300-312`）。
2. **`scales.rs` T3 注释的数值错误**（nod 与 shake 并档写 73.5）-> 由 W5 第二轮拆成 `shake 73.5` / `nod 79.2`、
   死区拆成 `2.375（Single：nod/look/tilt）/ 2.558（shake）`。理由见 §5 与 W2b 的逐字引用。

### 7.3 仍未处理的文档漂移（**我逐个判断过，未授权任何 worker 动**，等你裁决）

`grep -rn '动作在产品路径上不存在' docs` 仍有 6 处（**都是「动作」而非「director」的同类假断言**；W8/W8b 归属内的已清）：

- `docs/architecture/director-rfc.md:17 / :153 / :551`（W8 的块，已收口；W8 只标了三条红线）
- `docs/architecture/director-mod-v0.md:468`（同上）
- `docs/architecture/wallpaper-mod-v0.md:297`（**无任何块认领**）
- `docs/releases/v0.1.0-rc.2.md:27`（**历史发布说明，建议不改**）
- `docs/plans/*`（IMPL-PROMPTS 自己那句是引用问题描述，属正常）。

**我的建议**：Wave 2 顺手把前 4 处（不含 releases）就地限定为「当时（…）」；releases 保持历史原样。

### 7.4 Wave 3–5（串行，不许并行抢文件）

- **W9 单一驱动者**：退役「拉 `latest.preset_id`」那条通道，统一到 `action_cue`。
  ⚠ `main.dart` / `live2d_stage.dart` 里的 `DirectorPresetGate`（W4 刚加的）**正是 W9 要退役的东西**，
  W9 落地时它会随之删除/迁移 —— 这是**预期**，不是缺陷（W4 已在报告里说明）。
- **W10 动作协议字段化**（独占 `crates/l2d-wasm-demo/src/{main.rs,preset/*,web/surface/*}`、
  `lib/live2d/{live2d_bridge,live2d_stage}.dart`、两份架构文档）：三字段 + 同类相加 + hold +
  事件回执（`preset-applied/replaced/expired/cleared`，`dropped` = 皮套缺参数）+ `stage-clock`。
  **必须 `trunk build`**（它改 wasm 源码）。验收 = ORCHESTRATOR §5.9/§5.10/§5.11。
- **W11 导演 AI 侧**：输出 schema / 手册与 schema 同源 / 只切不改字（`segments` 拼接**逐字等于**主模型原文）/ 两条件唤醒 / 会话 baseline。
  ⚠ 它的【文件归属】名义上含 `desktop/src/supervisor.rs` 与 `supervisor/*` —— **那是红线点名的基座独占文件**：
  一旦 W11 必须动 `topics.rs` / `mod_registry.rs` / `supervisor.rs` / `desktop/src/main.rs`，**停下问维护者**；
  优先复用已有的 `POST /api/v1/mods/director/command` 通道。

---

## 8. 未取证 / 已知缺口（如实）

1. **TTS（8080）不在线** -> ORCHESTRATOR §5.8 的「真实 PCM 帧 + `turn_state=completed`」**未取证**。
   现有证据：`POST /api/v1/chat {"text":"下午好"}` -> 200 accepted；WS `/ws/state` 帧序
   `subscribe_ack -> error(tts_transport, fatal) -> text_fallback(完整正文) -> turn_state(failed) -> text_delta -> heartbeat`；
   `POST /api/v1/settings/test/llm` -> `{"ok":true,"latency_ms":141,"model_echo":"deepseek-flash"}`。
   **TTS 一起来就补跑这一条**（其余 7 条已绿）。
2. **一次未能定名的 flaky**：全套 cargo 跑了 4 次 —— 3 次干净 **1394/0**；1 次（与我并发的 `flutter test` + `flutter build web` 同时进行）
   在累计 **740 passed / 1 failed** 后按 cargo 默认 fail-fast 中止，失败落在**那个 593 用例的 desktop `--bins` 测试二进制**内
   （含 5s stall 超时与 WS 生命周期计时用例）。**2 次定向复现失败**（7x CPU burners；与 flutter test 并发）都是 593/0 绿。
   **我拿不到失败用例名**（当时那条 totals 命令把输出过滤成只剩 `test result:` 行 —— 我的流程失误，记在这里免得下次再犯）。
   判为**环境敏感的既有 flaky，非本轮回归**。**下次收口时别再并发跑 cargo 与 flutter 门禁。**
   **已移交的定名任务（维护者那份交接 §4 指派，与 W7 并行做，不是我本轮做的）**：
   跑 **20 次 `cargo test --workspace --all-targets --no-fail-fast`**（覆盖并发窗口），用 `tee` 留**全量**输出；
   **20/20 绿 ⇒ 记为「未复现」并点名风险面**：`crates/live2d-ai-desktop/src/supervisor/tests_stall.rs`、`crates/live2d-ai-desktop/src/web_api/ws/connection.rs`；
   **一旦复现 ⇒ 立刻停下并上报**（别再猜）。
3. **W5 的两处取舍**（已记录、未改）：被删副本对 `base` 做 trim 而 runtime 那份靠 URL 解析器吃空白（病态空白 base 才有差异）；
   两个 Mod 未收敛到 runtime（会改掉它们各自的错误口径与空 base 特判）。
4. **未做**：§5.4（W7 后）、§5.9/§5.10/§5.11（W10/W11）、A/B/C/D 四条（全转 W7）、§7.3 的 4 处文档漂移。

---

## 9. 红线（全程有效）

- **禁止** `git worktree / checkout / switch / stash / reset / commit / clean`（会丢 187 条 WIP）。
- **不碰** `/home/skystar/Live2D-Ai`（另一个 worktree）。
- **不改** `docs/plans/RESEARCH-actions-director-audit-2026-09-21.md`（规格真源，只读；本轮只有维护者本人改过它的 §2.1 表头）。
- **禁止**不带 `--check` 的 `cargo fmt --all`、禁止仓库级 `dart format`。
- 改 `.dart` -> 必须 `flutter build web --release --base-href /app/ --no-web-resources-cdn`；
  改 `crates/l2d-wasm-demo/**` -> 必须 `trunk build`；两者都要复核产物 mtime 晚于最后一个源改动（ORCHESTRATOR §4 / §5.12）。
- 动 `topics.rs` / `mod_registry.rs` / `supervisor.rs` / `desktop/src/main.rs` -> **先停下问维护者**。
- **不采信子 worker 自述**：每条结论要原始命令输出；禁造假绿灯（TTS 不在线就如实标注，别造假 PCM）。
- 不把密钥明文 / `.env` 内容 / 请求体写进报告或日志（`ignite.sh` 只回显变量**名**）。

---

## 10. 如何接手（建议动作序列）

1. 读 `docs/plans/RESEARCH-actions-director-audit-2026-09-21.md` §8/§9/§10（规格）+
   `ORCHESTRATOR-PROMPT-actions-performance-round.md`（§4 门禁 / §5 核验 1–12 / §8 红线）。
2. 从 `IMPL-PROMPTS-actions-performance-round.md` **逐字**提取下一块的原文（块边界：`^===== 复制给 Worker (W\d+) =====$`，
   到下一个块头前的 `====` 之前）。⚠ **W10 之后与 W9/W11 的块尾没有空行**，别把最后一行归属说明漏掉 —— 我第一遍就漏过；
   `/tmp/l1-blocks/*.txt` 里是本轮已验证过的正确版本（可直接复用）。
3. 每块 prompt = **原文** + 【编排补充】（§7.1 的 A/B/C/D + 环境事实：flutter PATH、浏览器配方、服务状态、trunk build 纪律）。
   **不要转述块内任务**。
4. 派发前先做**文件所有权冲突检查**（把各块【文件归属】与上一波已改文件对一遍）；同波次零重叠才可并行。
5. 收口：串行跑整组门禁（**别并发**）、按需 `trunk build` / `flutter build web`、**重建 Rust 二进制并重启服务**、
   `ignite --check`，再按 ORCHESTRATOR §5 逐条自测（原始输出）。
6. 报告格式 = ORCHESTRATOR §7。

---

## 11. 本轮触碰过的文件清单（自 2026-09-21 20:57 起，供交接对账）

    AGENTS.md                                                     (W1 + 编排者 R2 修正)
    assets/actions/preset_labels.json                             (W8)
    assets/actions/presets.json                                   (W2 表值 / W2b _doc)
    crates/l2d-wasm-demo/src/main.rs                              (W5)
    crates/l2d-wasm-demo/src/preset/mod.rs                        (W2)
    crates/l2d-wasm-demo/src/preset/scales.rs                     (W2 + W5 第二轮注释)
    crates/l2d-wasm-demo/src/preset/table.rs                      (W2 / W5)
    crates/l2d-wasm-demo/src/preset/tests/{assets,mod,packs}.rs   (W2)
    crates/l2d-wasm-demo/src/web/surface/input.rs                 (W5)
    crates/live2d-ai-desktop/src/web_api/cli_entry.rs             (W6)
    crates/live2d-ai-desktop/src/web_api/external_routes.rs       (W6)
    crates/live2d-ai-desktop/src/web_api/settings_routes/tests.rs (W2)
    crates/live2d-ai-desktop/src/web_api/voice_routes.rs          (W6)
    crates/live2d-ai-desktop/src/web_api/voice_routes_tests.rs    (W6)
    crates/live2d-ai-desktop/src/web_api/voice_routes_tests_token.rs (W6 新建)
    crates/live2d-ai-mod-director/src/staging_http.rs             (W5，范围外扩见 §4.1)
    crates/live2d-ai-mod-external-input/Cargo.toml                (W6，范围外扩)
    crates/live2d-ai-mod-external-input/src/lib.rs                (W6)
    crates/live2d-ai-mod-memory/src/summary_http.rs               (W5，范围外扩见 §4.1)
    crates/live2d-ai-runtime/src/lib.rs                           (W5)
    crates/live2d-ai-runtime/src/performance/client.rs            (W5)
    crates/live2d-ai-runtime/src/performance/mod.rs               (W5)
    crates/live2d-ai-runtime/src/performance/tests.rs             (W5)
    crates/live2d-ai-runtime/src/settings.rs                      (W2 / W5 :497)
    crates/live2d-ai-runtime/src/settings/patch.rs                (W2，门禁必需)
    crates/live2d-ai-runtime/src/settings/patch_tests.rs          (W2，门禁必需)
    crates/live2d-ai-runtime/src/settings/view.rs                 (W2，门禁必需)
    docs/README.md                                                (W1)
    docs/architecture/action-packs-v0.md                          (W1 §10 / W2b §11)
    docs/architecture/core-chain-baseline.md                      (W8b)
    docs/architecture/director-mod-v0.md                          (W8)
    docs/architecture/director-rfc.md                             (W8)
    docs/architecture/directory.md                                (W8b)
    docs/architecture/mod-product-chain.md                        (W8b)
    docs/architecture/performance-layer-v0.md                    (W8 / W4)
    ../legacy/plans/PRODUCT-L1-GOALS-2026-09-15.md                     (W1)
    live2d-ai.toml                                                (W2，运行时配置，gitignored)
    live2d-ai.toml.example                                        (W2)
    shell/flutter/lib/api/settings_models.dart                    (W2)
    shell/flutter/lib/live2d/live2d_stage.dart                    (W4)
    shell/flutter/lib/main.dart                                   (W4)
    shell/flutter/lib/settings/preset_labels.dart                 (W3)
    shell/flutter/lib/settings/sections/appearance_section.dart    (W2)
    shell/flutter/lib/settings/sections/dev_tools_section.dart     (W3)
    shell/flutter/test/action_cue_test.dart                       (W4)
    shell/flutter/test/action_scales_preview_test.dart            (W2)
    shell/flutter/test/developer_section_test.dart                (W3)
    shell/flutter/test/preset_labels_test.dart                    (W3)
    shell/flutter/test/settings_api_test.dart                     (W2)

（另有本轮之外、本来就存在的 WIP 与三份 `docs/plans/*-actions-performance-round.md` / `RESEARCH-*` 新文件，未列。）

---

*交接文档完。规格真源与验收口径以 RESEARCH + 两份提示词文件为准；本文件只补「已交付 / 未完成 / 环境与配方」。*
