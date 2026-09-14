# Live2D-Ai 项目说明（AGENTS.md）

> 本文件是项目级通用说明，供任何 CLI agent（pi / Claude Code / Hermes 等）读取。
> 状态：2026-09-10 重写——确立 **Rust 核心 + Flutter 前端** 双主导分层，
> 前端/接口层自 `rust-ratio` 门禁中**显式豁免**（原版只描述 Rust 单主线）。

## 项目背景

- 文档索引见 `docs/README.md`；核心契约与目录约定见 `docs/architecture/core-contracts.md`
  与 `docs/architecture/directory.md`。
- 定位：**通用人形皮套 AI 接入一体化平台**——AI + Live2D 人形皮套的通用接入与应用层，
  **不绑定任何单一模型**（模型由用户合法导入，`assets/models/` 不捆绑二进制），
  **不做复杂上层**（实现保持最小）。验证「文本 → LLM（纯对话，无工具）→ TTS → 驱动口型
  → Live2D 皮套渲染 + 前端 UI」闭环。
- **当前版本 `0.2.0-rc.1`（Mod 纪元第一基线：external-input 直播刚需 + 社区许可，2026-09-14）**：
  主链皮肤（LLM→TTS→口型→Live2D、壳/舞台背景）**冻结未改**。本版把**外部事件注入**
  做成刚需：`POST /api/v1/external/chat` 契约文档化（text/token/Origin/loopback/忙碌/
  启停门禁/模板前缀 + curl），**B 站抓取不在主仓，在 Win sidecar**
  （`docs/examples/bilibili-sidecar/`：blivedm → 清洗 → POST）；`external-input` Mod 加强
  （静态 `settings_spec` v2：`listen_port`/`token`/`text_template`/`prefix`；启停唯一真源 =
  manifest `enabled`，停用 → `403 mod_disabled`；**缺省启用**）；**`local-llm` 废除启动**
  （移出 `AVAILABLE_MOD_FACTORIES`，4→3；crate 暂留仓库并标 DEPRECATED，**禁止挂回**）；
  `docs/architecture/mod-community-license.md` 写明注册/分发/许可边界。
  发布说明 `docs/releases/v0.2.0-rc.1.md`；契约 `docs/external-input.md`。
- **上一版 `0.1.0-rc.5`（壳全局背景 + 与舞台同步，2026-09-14）**：
  主链 LLM→TTS→口型→Live2D **一行未改**。本版只动前端显示层：`DisplayPrefs` 增加
  `shellImage` 与 `syncShellStageBg`（默认 `true`＝壳与舞台**共用同一张图**，
  `effectiveShellImage` 是一份真相）；壳根铺一层**固定 0.15 透明度**的全局背景
  （`ui/shell_backdrop.dart`，**不做** opacity 滑条），聊天面板在有背景时留一点透
  （`kShellSurfaceAlpha = 0.86`）但保持可读，「外观与互动」新增「壳背景」块
  （选图 / 清图 + 与舞台同步）。背景**只住本机 `DisplayPrefs` / `localStorage`**：
  不写 `live2d-ai.toml`、不做分区独立背景 / 轮播 / 背景 Mod / 在线拉图 / 动态加载。
  发布说明（含 **Win 选图 → 可见 → 清图** 肉眼 checklist）：
  `docs/releases/v0.1.0-rc.5.md`。
- **上一版 `0.1.0-rc.4`（Mod 产品链路 + 主链人设收敛，2026-09-13）**：
  主链 LLM→TTS→口型→Live2D 未变；本版把 Mod 从「能编进 binary 的骨架」做成
  **产品链路**——`mods.json` 启停/配置**原子写回**、`GET /api/v1/mods` 带
  `settings_spec`+`config`（`secret` 字段脱敏）供 Flutter 渲染表单、新 Mod 模板 crate +
  `descriptor.api_version` 门禁、`ModServices.apply_settings` **一等化**（删掉旧的
  `__apply_settings` 事件走私）；**酒馆角色卡抽成第一条标准 Mod**
  `live2d-ai-mod-persona`（V1/V2 JSON + PNG `chara` → 合成 `system_prompt`），
  主链 `[persona]` **只剩** `system_prompt` + `max_history_pairs`（**升级需手改一次
  旧 toml**，见发布说明 §4.3）；Win 舞台背景图三处根因修复。发布说明：
  `docs/releases/v0.1.0-rc.4.md`。上一版 rc.3 是结构质量（正文兜底 / 拆大文件 /
  双壳休眠 / CI 对齐）：`docs/releases/v0.1.0-rc.3.md`；rc.2（第二基线：动作层删到底 +
  模型库闭环 + `.env` 密钥真源）：`docs/releases/v0.1.0-rc.2.md`。
  **LLM 工具层与动作系统已整体拆除**——**不要**再以「LLM 调用工具」「动作系统」为前提
  写代码或文档。
- `Live2D-Ai-pc/`（Python）已归档（tag `py-legacy`）；`Live2D-Ai-Android/` 已归档
  （`android-archive` 分支，见 `ANDROID_ARCHIVE_POINTER.md`）。
  **2026-09-11 起这两个归档的远端 ref 已删除，只在维护者本地保留**——公开历史重新起算
  （`main` 成为单个根提交），见 `docs/releases/v0.1.0-rc.1.md`「历史重置」。
- 增强能力通过 **Mod 边界**隔离：`live2d-ai-mod-system` trait 注册中心，
  3 个 Mod（external-input / pet-desktop / persona）为 workspace crate；
  **缺省只启用 `external-input`**（直播弹幕/礼物经 sidecar 注入，见
  `cli_entry::default_mods_manifest`），pet-desktop / persona 缺省停用。
  **`local-llm` 已于 `0.2.0-rc.1` 废除启动**（移出注册表；crate 暂留仓库，**禁止挂回**）。
  **Mod 契约 / 加新 Mod 勾选表 / 正式版 Rust-C 规则**见
  `docs/architecture/mod-product-chain.md`（与旧 `plugin-sdk.md` 冲突时以它为准）；
  **许可与分发边界**见 `docs/architecture/mod-community-license.md`。
  **director Mod 已于 `0.1.0-rc.2` 删除**（它是动作序列的唯一驱动方，而动作在产品路径上
  不存在；归档在分支 `archive/action-layer-p6`）——静态注册的工厂数由
  `main.rs` 的 `mod_count_is_three` 断言守住，**不要再挂回去**。
- **TTS 不是 Mod**（2026-09-11 用户裁决）：语音合成是**核心链路**
  （LLM → TTS → 口型），端点唯一权威来源是 `live2d-ai.toml` 的 `[tts]` 段。
  见 `docs/architecture/tts-is-core.md`。

## 分层与技术栈（**双主导**）

本项目的「主导」是**两层**，不是一个语言：

| 层 | 语言 / 位置 | 职责 | 门禁 |
| --- | --- | --- | --- |
| **核心层（Rust）** | Rust workspace `crates/*` | 渲染（`l2d`）、状态机（`live2d-ai-core`）、LLM/TTS 网络与配置（`live2d-ai-runtime`）、桌面壳与 Web API（`live2d-ai-desktop`）、wasm 渲染面（`l2d-wasm-demo`）、工程工具（`xtask`） | `cargo test/fmt/clippy` + `rust-ratio` ≥95% |
| **前端/接口层（Flutter Web）** | `shell/flutter/`（Dart），由 Rust 在 `/app/` 同源托管 | 用户界面：Live2D 舞台、聊天、设置、外观/口型控制 | `flutter analyze` + `flutter test`（**豁免 `rust-ratio`**） |

- **唯一实现主线 = 核心层 Rust workspace**（`crates/`）：`l2d` / `live2d-ai-core` /
  `live2d-ai-runtime` / `live2d-ai-desktop` / `l2d-wasm-demo` / `xtask` / `live2d-ai-mod-system`。
- **前端以 Flutter Web 为准**（`shell/flutter/`），**且只有一个入口**：
  `GET /` 与 `GET /index.html` **302 到 `/app/`**。
  旧的**原生 JS 前端**（`web_api/index.html` + `app.js` + `chat.js` + `style.css`
  + `index_html.rs`）已于 **2026-09-11 删除**——它曾在 `/` 提供一套**与 `/app`
  不同**的界面：同一个服务两个产品，打开哪个 URL 看到哪个，是明确的坑。
  删除后 `rust-ratio` 反而升到 ~98%（`js` 不再计入分母）。新功能一律做在 Flutter 侧。

### 前端层豁免的理由（显式记录，勿当成遗漏）

1. **Flutter Web 无法用 `dart:ffi` 渲染**，业界统一做法是「隔离的 Web 渲染面 +
   版本化消息桥」——本项目即「Flutter 壳 + 复用 Rust 的 `/render`（wasm）」，
   渲染核心仍是 Rust，Dart 只承担界面与调度。
2. `xtask` 的 `rust-ratio` 统计扩展名（`rs/wgsl/py/ts/js/c/cc/cpp/h/hpp`）**不含 `dart`**，
   因此 Dart 不计入该门禁——这是**刻意的豁免**，不是统计漏洞。
   `xtask` 报告会**单独打印 Dart 行数**以保证可见性（可见但不设 Rust 占比门槛）。
3. 前端层的质量门槛改由 Flutter 自己的工具链承担：
   `flutter analyze`（静态检查）+ `flutter test`（含音频调度等纯逻辑回归）。
4. 治理红线仍然适用：前端**不得**绕过 core 仲裁、不得直接持有密钥、
   不得引入设备端推理运行时（LLM/TTS 仍走统一 OpenAI 兼容端点）。

## 工作区与点火纪律（WSL2 ↔ Windows，2026-09-12 定）

核心开发**只在 WSL2**（`/home/skystar/Live2D-Ai`）；Windows 侧只承担**浏览器肉眼验收**。
两边不做第二套真相，也不互相复制产物。

| 角色 | 职责 |
| --- | --- |
| **WSL2**（唯一开发环境 + **唯一进程宿主**） | `cargo` 全部构建与测试；Flutter SDK 在 `~/flutter`；**服务进程一律在这里起**（`./scripts/ignite.sh`，默认端口 18080）；前端构建 `flutter build web --release --base-href /app/ --no-web-resources-cdn` |
| **Windows** | 只做两件事：开浏览器点 `http://127.0.0.1:18080/app/`；把看/听的结论写回。**不跑二进制、不编 Flutter** |
| **产物** | 单一真源 = WSL 的 `shell/flutter/build/web`。`LIVE2D_AI_FLUTTER_WEB_DIR` 一律写 **WSL 路径**；**不要**引入 Windows UNC 路径写法（`\\wsl.localhost\…`）——只有「哪天真的在 Windows 上跑二进制」才需要，那不在本计划内 |

- `scripts/ignite.sh` 会把 `LIVE2D_AI_FLUTTER_WEB_DIR` 锚定成仓库内绝对路径，
  所以「cwd 不对 → `/app/` 503」不该再出现；503 响应体现在会**列出实际找过的每个路径**。
- **点火体检**：服务跑起来后另开一个终端跑 `./scripts/ignite.sh --check`，断言
  `GET /` = 302 → `/app/`、`GET /app/` = 200、`index.html` 与 `main.dart.js`
  **不含 `gstatic.com/flutter-canvaskit`**（断网红线）。这三条只有真起过一次服务才验得到，
  单元测试覆盖不了托管层与产物内容。

详细分工与验收：`docs/plans/PLAN-rc2-second-baseline-2026-09-12.md` §3。

## 开发约定

### 门禁：本地必跑 vs CI 必跑（2026-09-13 rc.3 对齐）

**一张表，一套真相**：本地提交前跑的命令，必须能在 CI 清单里逐条找到；CI 里没有的
检查不进「提交前必跑」清单。对应 workflow 在 `.github/workflows/`。

| 检查 | 本地命令 | CI（workflow → job） |
| --- | --- | --- |
| Rust 测试（lib/bin/tests/examples） | `cargo test --workspace --all-targets` | `pr-checks.yml` / `nightly.yml` → `rust-*-test` |
| Rust doc 测试（rustdoc 示例） | `cargo test --doc --workspace` | 同上（`Test doc examples`） |
| 格式 | `cargo fmt --all -- --check` | 同上 |
| Clippy（全目标、0 warning） | `cargo clippy --workspace --all-targets -- -D warnings` | 同上 |
| Rust 占比 ≥95% | `cargo run -p xtask -- rust-ratio` | 同上（`Rust ratio (>= 95%)`） |
| MSRV 1.92 可编译 | 手动（可选） | `pr-checks.yml` → `msrv-check`（仅 `cargo check`） |
| 前端静态检查 + 测试 | `cd shell/flutter && flutter analyze && flutter test` | `flutter-checks.yml`（`paths: shell/flutter/**`） |
| 仓库根历史资产测试 | `python3 -m pytest tests/ -q` | `pr-checks.yml` → `root-py-tests` |
| 端到端核心链（需活端点） | `python3 scripts/verify_core_chain.py --timeout 300` | `nightly.yml` → `core-chain-verify`（**仅当**配置仓库变量 `LIVE2D_AI_VERIFY_BASE_URL`） |
| wasm 渲染面可编译 | `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` | **暂无**（本地手动；wasm 改动必须 rebuild + 肉眼） |

两条纪律：
1. 端到端探针**只在有活端点时跑**，缺配置就明确跳过——绝不伪造绿灯
   （「天天红」比没有这条更坏，与「自检说谎」是同一条教训）；
2. wasm 那条暂时只有本地门禁——**已知缺口**，写在这里而不是假装它被 CI 守着。

### 核心层（Rust）

- **门禁**（提交前必须）：`cargo test --workspace --all-targets` + `cargo test --doc --workspace`
  + `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`
  + `cargo run -p xtask -- rust-ratio`（门槛 95%）。
- **源码 ≤500 行**（豁免 ≤1000 需头注理由）；测试文件 ≤800 行。
- **密钥安全**：密钥不进 GET/日志/WS/导出；loopback-only；mutating 需 `application/json`。
- **密钥真源 = `.env`**（2026-09-12 rc.2 用户口径）：查找优先级 **`.env` 快照 >
  进程环境 > 无（不鉴权）**；`live2d-ai.toml` 只持 `api_key_env`（变量**名**）。
  - 读取**只能**走 `live2d_ai_runtime::secrets::lookup`——直接 `std::env::var`
    会绕过 `.env`，于是「界面上刚写了 key、链路还说没配置」。
  - 写入走 **`PUT /api/v1/env`**（`{"key","value"}`；空值 = 清除）：原子写 +
    `0600` + **就地改行**（保留注释，与 `merge_into_toml` 同一条教训）；
    `GET /api/v1/env` **只回键名 + 是否已设置，永不回值**；写操作日志不记 body。
  - **热重载**：写入后刷新快照 + `supervisor.reload()`；`.env` 也在
    `file_watcher` 的监视集里（外部手改同样即时生效——与「配置快照必须与磁盘
    一致」是同一条纪律）。
- **Mod 边界**：Mod 必须通过 `live2d-ai-mod-system` 接入，不得绕过 core 仲裁。

### 推理模型的「思考」（2026-09-13 定）

上游可能是**推理模型**（实测 `deepseek-flash`）：除正文外还发 `reasoning_content`。

- 解析层必须**单列**一类事件（`LlmEvent::ReasoningDelta`），**不得**并进正文；
- **思考不进句子装配器、不进 TTS**——否则模型会把内心独白念出来（回归：
  `llm::reasoning_tests::reasoning_never_reaches_the_sentence_assembler`）；
- WS 用**独立帧类型** `reasoning_delta`（不复用 `text_delta`：后者的语义是
  「与音频同拍的正文」，混用会把思考追加进气泡正文）；
- **思考与正文共用 `max_tokens`**：所以默认上限是 **4096** 而不是 512。上限过小的
  表现不是「回复短一点」，而是**正文被挤成半句 → 切不出完整句 → 一个字都不上屏**
  （用户看到「模型没有返回」）。改这个默认值前先重跑 `settings_tests` 里那段实测记录；
- 前端思考**不落盘**（比正文长 5–20 倍，写进 localStorage 会膨胀一个数量级）；
  刷新后旧气泡不再有思考，这是刻意取舍。

### 动作与表演的归属（休眠台账，2026-09-12 rc.2 定）

**动作在产品路径上不存在**。这不是「暂时没接」，是裁决（`docs/architecture/core-chain-baseline.md`
§3.1）——所以每一处残留都要能回答「谁休眠、为什么、谁能唤醒」：

| 对象 | 状态 | 谁能唤醒 |
| --- | --- | --- |
| `live2d-ai-core` 的 `action/` + `performance/` | **休眠保留**（类型 / reducer / capability gate 原样） | 只有先重新论证 `core-chain-baseline.md` §3.2 的三条理由 + `lib.rs` 第 4/6 条不变量之后 |
| `ModServices.action_tx`（仍属 Mod API 契约） | **休眠**：host 注入固定 sender，请求只留一行 debug 日志并返回 `false` | 同上；**不得**在 `mod_registry.rs` 里私自接回真通道 |
| `live2d-ai-mod-director`（动作序列的唯一驱动方） | **已删除** | 归档在 `archive/action-layer-p6` |
| 渲染面（`l2d-wasm-demo`）的编舞残件 | **已删除**（`action-state` 接收器 + `surface.rs` 编舞） | 归档在 `archive/action-layer-p6`；恢复必须 wasm 重建 + 肉眼验收 |
| **待机生命体征**（`IdleState` 呼吸/眨眼/微表情） | **必须保留**——与动作系统是两套机制，只共用 override 层 | 无（它一直在产品里，删动作时**绝不要**连带删它） |

为什么不能「顺手接回去」：一个 `live2d_perform_action` 工具 + 空 system prompt 会让模型
**只调工具、不说话**，产出「正常完成但一个字都没有」的回合（§3.1 当场复现过）。
护栏是两条断言：`main.rs::mod_count_is_three`（工厂数不得因动作 Mod 增加）与
`mod_registry::tests::action_request_is_dormant_not_delivered`（动作请求必须不被接受）。

### 原生第二壳的归属（休眠台账，2026-09-13 rc.3 定）

**主路径是 `--web`**：Rust 服务 + Flutter Web `/app/`（`./scripts/ignite.sh` 点火）。
桌面侧那份 **egui 原生壳**（`src/app/` + 窗口/托盘/桌宠模式）与 **`--chat` 终端壳**
能编译、能跑，但**不在产品主路径上，也不承担验收**。

rc.3 裁决（计划 §5，**选项 B**）：**本轮不 feature-gate**。理由：本项目口径是
「可读性优先」；gate 会把 Cargo feature 矩阵与 `cli` 的用法/测试断言一起搅动，
而收益（编译时间/二进制体积）抵不上理解成本。等价措施是把界线**钉死**——
本节 + `README.md` + `docs/architecture/core-chain-baseline.md` §3.6，
**不允许第三种含糊表述**。

| 对象 | 状态 | 位置 | 谁能唤醒 |
|---|---|---|---|
| **Web 主链**（`--web` + Flutter `/app/`） | **主线** | `web_api/` + `shell/flutter/` | 不适用（它就是主线） |
| egui 原生壳（窗口 / 设置面 / 托盘 / 桌宠穿透） | **休眠保留**：非主线、不验收 | `src/app/`、`src/tray.rs`、`src/backend.rs`、`src/platform.rs` | 单独立项 + 先论证「谁来维护第二个 UI 壳」 |
| `--chat` 终端壳 | **休眠保留**：非主线、不验收 | `src/repl.rs` + `cli` 的 chat 分支 | 同上 |
| `--window-smoke` / `--model-smoke` / `--benchmark` | **工具**（冒烟/基准），**不是产品入口** | `src/model_smoke.rs`、`src/benchmark.rs` | 无（工具用途不变；`--benchmark` 明令禁止成为生产默认） |

两条红线：
1. **默认文档入口只推销 `--web` / `scripts/ignite.sh`**；
2. **不得**再往 egui 设置面板加与 Flutter 重复的产品字段——那是「第二个产品」，
   属于步骤 2，**停**，记到 `PLAN-rc3-structure-quality-2026-09-13.md` §10 backlog。

### 前端层（Flutter）

- **门禁**（改动 `shell/flutter/**` 时必须）：
  `cd shell/flutter && flutter analyze && flutter test`。
- 前端产物**不入库**：`shell/flutter/build/`、`.dart_tool/` 已被嵌套 `.gitignore` 排除；
  `xtask` 亦按目录名排除 `build`（否则 `main.dart.js` 会打穿占比口径）。
- 改动前端后**必须重新构建**才生效：
  `flutter build web --release --base-href /app/ --no-web-resources-cdn`
  （静态文件由服务端按请求读盘，故前端改动只需重建、不必重启后端）。
  **`--no-web-resources-cdn` 不是可选项**：缺省构建把 CanvasKit 指向
  `https://www.gstatic.com/flutter-canvaskit/<engineRevision>/`，
  而 `build/web/canvaskit/` 那份本地副本不被引用——**断网即白屏**。
  本项目是本地优先的桌宠，不允许依赖 Google CDN（`scripts/ignite.sh` 会探测并告警）。
- **中文字体必须自托管**（`assets/fonts/`，Noto Sans SC 子集，OFL-1.1）。
  Flutter Web 的 CanvasKit **取不到设备字体**，`fontFamilyFallback` 也不会命中系统字体——
  缺字时引擎只会去 `fonts.gstatic.com` 下载，**断网即豆腐块**（有网时看不出来）。
  不要删掉 `ThemeData.fontFamily`，也不要改用「系统字体回落」；回归在
  `test/theme_test.dart`（含 CJK 本地化后字体不被覆盖回 Roboto 的守卫）。
- **界面文案里不得出现子集外的字符**（2026-09-11 定，真机点火抓到的缺陷）：
  真机上实测到 `GET https://fonts.gstatic.com/s/notosanssc/v37/…woff2 [200]`
  ——因为流式光标用了 `▍`(U+258D)、macOS 前缀用了 `⌘`(U+2318)，两者都不在子集里。
  **门禁**：`test/font_subset_test.dart`（扫 `lib/**/*.dart` 字符串字面量 vs
  `assets/fonts/*.ranges.txt`；后者由 `scripts/font_subset_ranges.py` 生成，头部记录
  字体字节数 + FNV-1a——**换字体必须重新生成，否则门禁按哈希不符判红**）。
  修法是二选一：**改成不依赖字形的实现**（流式光标现在画竖条、`⌘` 改成 ASCII `Cmd`），
  或把字符加进子集后重新生成覆盖表。
- **协议约定**：前端与渲染面之间只走**版本化消息**（`{version,type,payload}`，见
  `l2d-wasm-demo` 的 v1 协议）；新增字段必须向后兼容（缺省即默认值）。
- **压在舞台 iframe 之上的可交互控件必须套 `StagePointerInterceptor`**
  （`lib/live2d/`，2026-09-11 定）：Flutter Web 的两张画布都是 `pointer-events: none`，
  真正接指针的是 iframe 那个 DOM 元素、且它在画布之上——指针落在舞台区域会进
  iframe 自己的文档，**父页收不到**。漏套不报错，只是**「看得见、点不着、也滑不动」**
  （设置面板、断线横幅、缩放角标、错误重试全中过），回归在
  `test/stage_pointer_interceptor_test.dart`。
- **链路错误必须走 `AppEvent::Error` + `tracing` + WS `error` 帧**（2026-09-11 定）：
  一次 LLM/TTS/解码失败要同时出现在**后端日志**（`tracing::error!`，带
  `code=` 结构化字段）与**前端**（WS `error` 帧，`{code,stage,message,hint,epoch,fatal}`），
  且两处的 `code` 是**同一个字符串**（用户要拿界面上的码去日志里搜）。
  错误码由 `live2d-ai-runtime::conversation::ErrorKind::code()` 给出
  （`<stage>_<suffix>`，上游状态码进码：`llm_upstream_401`），`hint()` 给一句
  可执行处置。**禁止**新增只 `println!` 的错误路径（不进文件 sink = 排障时一片空白），
  **禁止**让前端从显示文案里猜错误类型（文案会改，码是契约）。
- **请求级日志在 `web_api/dispatch.rs`**（≥500 `error` / ≥400 `warn` /
  mutating 成功 `info` / 其余 `debug`）；**不记录请求体**（含用户提示词等内容）。
- **配置快照必须与磁盘一致**：`file_watcher` 的外部修改路径除了
  `supervisor.reload()` 还要 `StatusContext::refresh_from_disk`——否则
  `GET /api/v1/settings` 回旧值，且下一次界面「保存」会把旧快照整份写回、
  **静默覆盖用户手改的配置**。PATCH 路径与它共用同一个函数。
- **不得**在 Flutter 侧复制核心逻辑（状态机、动作仲裁、LLM/TTS 协议）——
  那些属于 Rust 核心层，前端只做展示与调度。

### 语音输出约定（2026-09-10 用户裁决）

- **一句一单元**：一个句子必须**完整合成后连续播放**，不做句中切分、不做听感上的
  「断断续续」。**延迟可接受，断句不可接受。**
- 因此：分句只按真实句读边界（`。！？…` 等）切分；不允许按字符位置硬切。
- 播放侧允许为「整句完整性」引入预缓冲；宁可晚开口，不可中途卡顿或重复。
- **默认出声**（2026-09-10 用户裁决，v0.4.3 起）：出厂即有声——**「不出声」必须显式
  要求**，不能是默认状态（否则用户会以为 TTS 坏了）。要安静有两条路：用户按 UI 静音，
  或服务端 `LIVE2D_AI_MUTE_AUDIO=1`。**跑测试 / 无人值守一律用后者**：它在音频源头
  生效（下发零 PCM + 真实 `volume`），绕不过任何客户端缓存或第三方客户端。
- **主音量是客户端输出设置**，与口型**正交**：改音量、按静音都不得影响口型驱动。
  实现随音频路径变过，但这条正交性不变。
  **2026-09-11 变更**：播放路径从 `AudioContext` 逐片调度改为 **`<audio>` 媒体元素
  + Blob(WAV)**（见下条），因此静音/音量不再走 `GainNode` 增益，而是直接落在
  媒体元素上（`element.muted` / `element.volume`）。**不要**再把它折成增益——那样会
  重新绕开浏览器的站点级静音权限路径（`hasAudioContext` 这个名字保留，语义变成
  「媒体元素后端可用」）。
- **音频播放走媒体元素，不走 Web Audio**（2026-09-11 用户裁决，起因是「浏览器禁用
  声音但依旧有声音传出」）：Web Audio 归 Chromium 的 **autoplay 策略**管，而浏览器
  「站点声音 = 阻止」作用在**媒体元素**上——用 `AudioContext` 逐片播放时站点级静音
  很可能拦不住。现在后端把 PCM 经 WS 下发（一句一组分片，带 `start`/`end`/
  `sentence_seq` 句子边界），前端按句封成 **WAV**（= 44 字节头 + 裸 PCM，零依赖）交给
  `<audio>` 播放，口型用 `audio.currentTime` 查服务端 `volume` 包络。
  **本机 TTS 实测出不了 mp3**（`mp3`/`wav` 均 HTTP 400，只有 `pcm` 200）——所以要
  「推文件」就用 WAV，别指望 mp3。
- **句子边界必须由引擎给出**（2026-09-11 定）：WS 音频帧的 `start`/`end` 标的是
  **句**的首/末片（来自引擎 `EngineEvent::AudioChunk` 的 `first_chunk`/`final_chunk`），
  **不得**在发射侧用记账/推断去猜（旧实现按 epoch 推 `start`，实测产出
  「0 个 start、13 个 end」，前端会把一句切成十几段播）。**空末块也要发边界帧**
  （`audio: ""` + `end: true`）——句子样本数是 `audio_chunk_samples` 整数倍时末块
  0 样本，漏掉它就等于让那一句永远没有句尾闸门。
- **「连通性自检」只测连通，不合成**（2026-09-13，起因是实测假失败）：
  `POST /api/v1/settings/test/tts` 打的是 `GET {tts.base_url}/models`（毫秒级），
  **不再真合成一次**。三个理由都实测过：本机合成一次 2.4s 而自检默认只等 3s；
  链路正在合成时再点自检会稳定报 `timeout`（**而同时语音完全正常**）；
  HTTP 循环是单线程的，同步探针会把整个 API 占住它等待的全长。
  **自检与产品链路抢资源的代价是「自检说谎」**——那比没有自检更坏。
  失败分类：401/403 → `auth_failed`；404 → **算通过**（上游回话即可达，`/models`
  并非所有语音实现都提供）+ 一句 `note` 说明；其余 4xx/5xx → `protocol_error`。
  密钥读取一律走 `secrets::lookup`（这里也曾直接读进程环境 → 非 `ignite.sh` 启动时
  出现同一类假失败）。
- **分句器把换行也算句读**，因此会产生**纯空白句**：这类句子**不发 TTS 请求**
  （上游会回 400 "input 为空"，而 TTS 错误是 fatal，会把整轮判失败），走静音句
  路径即可（2026-09-11 修）。

## 变更历史

- **2026-09-14（v0.2.0-rc.1，Mod 纪元第一基线：external-input 直播刚需 + 社区许可）**：
  版本线 `0.1.0-rc.5` → **`0.2.0-rc.1`**（不是补丁增量：外部事件成为一等刚需）。
  **主链皮肤冻结**——LLM/TTS/口型/Live2D、壳/舞台背景一行未改。① **外部事件半成品做实**：
  `POST /api/v1/external/chat` 契约重写（端点表 / token 优先级 env→Mod config→不鉴权 /
  Origin+loopback / 忙碌 `ok:false` / 启停门禁 / 模板前缀 / 6 条 curl / 错误表），
  并**写明「B 站抓取不在主仓，在 Win sidecar」**。② **`external-input` Mod 加强**：
  工厂静态 `settings_spec` v2（去掉与 manifest 重复的 `enabled`；新增 `text_template`/`prefix`），
  纯函数 `render_injected_text`/`render_from_config`/`token_from_config`；handler 加
  `403 mod_disabled` 门禁并复用 Mod 纯函数；**修掉 Authorization Bearer 头未读取的缺陷**；
  补 6 条 handler 回归（门禁三态 / Bearer / config token / 模板膨胀）与 10 条 Mod 规格/纯函数测试。③ **Win py 示例**：
  `docs/examples/bilibili-sidecar/`（blivedm，只处理 `DANMU_MSG`+`SEND_GIFT`，
  其它 cmd 只 log；清洗后 POST；README 写房间号/SESSDATA/token 注意点，注明
  `SEND_GIFT_V2` 灰度需后续兼容）。④ **废除 `local-llm` 启动**：移出 `AVAILABLE_MOD_FACTORIES`
  （4→3）与 desktop 依赖、移出缺省 manifest，crate 暂留并标 DEPRECATED；缺省 manifest 改为
  启用 `external-input`。persona **不回退**（仍只在 Mod 侧）。⑤ **`mod-community-license.md`**：
  注册面开放 / 分发面 AGPL 兼容 / 闭源走私用或商业许可，**无「闭源可进默认包」承诺**。
  发布说明：`docs/releases/v0.2.0-rc.1.md`。
- **2026-09-14（v0.1.0-rc.5，壳全局背景 + 与舞台同步）**：主链一行未改，只动
  前端显示层。① **偏好字段**：`DisplayPrefs` 增加 `shellImage`（壳自己那张，
  只在同步关时用）与 `syncShellStageBg`（默认 `true`）；`effectiveShellImage`
  getter 是唯一判据（同步开 → 舞台那张）。② **渲染**：新增 `ui/shell_backdrop.dart`
  （`ShellBackdrop` + 纯函数 `decodeDataUrlBytes`，坏 dataURL 永不抛），
  `AppShell` 最外层铺主题底色 + **固定 0.15** 的背景图；有背景时脚手架底透明、
  聊天面板面透明度 `0.86`（保持可读），无背景时观感与改动前一致。
  ③ **UI**：「外观与互动」新增「壳背景」块（选图 / 清图 + 「与舞台同步」）；
  同步开时该行的选 / 清图改的就是 `stageImage`——**一份真相，不是两张**。
  ④ **版本三处同步** `0.1.0-rc.5`（`Cargo.toml` / `pubspec.yaml` / README 首屏）。
  ⑤ **rc.5 修复：舞台背景图不显示（2026-09-14，实测抓到）**——现象是标题栏
  （Flutter 壳）能看到背景图、**Live2D 舞台仍纯黑**。根因：渲染面把
  `canvas.style.background-image` 写成 `url(...) center/cover no-repeat`，
  而 `background-image` 是**长手**属性、只接受 `<image>#`，位置/尺寸/重复让**整条
  声明被 CSS 解析器丢弃**，且 `style.setProperty` 不报错——于是 backgroundImage 恒空。
  修法：只写 `url(...)` / `none`，位置/尺寸/重复分写三个长手属性；换算抽成
  **原生可测**的 `crates/l2d-wasm-demo/src/stage_css.rs`（旧实现埋在 wasm-only 的
  `web::surface::input` 里，原生 `cargo test` 编译不到 = **没有回归**，与 `mouth.rs` 同款教训）。
  （当时据「clear 为 `TRANSPARENT`、`alpha_mode` 优先 `PreMultiplied`」判断
  「不是合成问题」——**该判断被紧接着的 ⑥ 推翻**：caps 里根本没有 PreMultiplied。）
  渲染面产物已 `trunk build` 重建（`dist/` 被 gitignore，改 wasm 必须重建）。
  ⑥ **rc.5 修复二（已证伪并回滚）：黑幕盖住舞台**——CSS 值合法后，加载瞬间舞台能
  闪到背景、随后被**黑幕铺满**。根因在 wgpu 29.0.4 源码里：`webgpu` 后端的
  `get_capabilities` **只报** `alpha_modes: vec![Opaque]`，`wgpu-core` 又以
  `UnsupportedAlphaMode` 拒绝 caps 之外的 `alpha_mode` ⇒ WebGPU 路径**无法请求
  `PreMultiplied`**，canvas 必然按 `Opaque` 合成，`Clear(TRANSPARENT)` 变成不透明黑。
  曾据此改成 **WebGL2 优先** 想换透明 canvas，但**用户机 HUD 实测仍是
  `GPU: webgpu/WebGPU`**（`request_gl` 未生效）→ 该改动已**回滚**（`gpu.rs` 恢复
  WebGPU 优先），并判定「靠 canvas 透明露 CSS 背景」**结构性不可达**。
  ⑦ **rc.5 修复三（最终方案）：舞台背景画进 framebuffer**——背景预通道
  `LoadOp::Clear(<当前 stageColor>)` + 有图时全屏三角形按 `cover` 贴纹理
  （`web/surface/background.rs`），模型通道改用新增的
  `l2d::ModelRendererCore::render_to_view_submit_with_load(..., LoadOp::Load)` 只叠
  Live2D、**不再 clear**；叠放语义不变（图盖纯色底、模型在最上、清图回纯色），
  与 canvas 是否透明**完全无关**，WebGPU 保持默认。图像解码走浏览器
  （base64 → Blob → `createImageBitmap` → OffscreenCanvas 2D → `write_texture`），
  PNG/JPEG/WebP 全支持且**不引入 Rust 图像依赖**；坏图退回纯色底。
  canvas 的 CSS `background-color` / `background-image` 写入**已删除**、`stage_css.rs`
  随之删除（不透明 canvas 上永远看不见，留着只会误导）。纯逻辑在原生可测的
  `l2d-wasm-demo/src/stage_bg.rs`（`base64_decode_data_url` / `cover_uv` /
  `clear_color`）。HUD 新增 `bg: solid|image` 字段作为「图有没有上屏」的直接信号。
  门禁：`flutter analyze` 无问题 + `flutter test` **833** 通过（本轮修复未改 Flutter）；
  cargo **830** 通过 / 0 失败（rc.4 822 + `stage_bg` 8；§3.1/§3.2 的 3 条临时回归已随路线
  废弃删除）/ doc **3** / fmt clean / clippy **0 warning** / rust-ratio **97.1135% PASS** /
  `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` ok。
  发布说明（含 **Win 选图 → 可见 → 清图** 肉眼 checklist）：
  `docs/releases/v0.1.0-rc.5.md`。**范围真源缺口**：任务书点名的
  `docs/plans/PLAN-rc5-shell-bg-sync-2026-09-14.md` **不存在**（已确认不在任何分支 /
  stash / 工作区），本轮以任务书逐条列出的必做 / 禁止为范围，未列出的都没做。
- **2026-09-13（v0.1.0-rc.4，Mod 产品链路 + 主链人设收敛）**：主链
  LLM→TTS→口型→Live2D 未变。① **Mod 产品链路（M0–M4）**：新增
  `docs/architecture/mod-product-chain.md`（契约 + 加新 Mod 勾选表 + 正式版 Rust/C 规则）；
  `mods.json` 启停/配置**原子写回**（`plan_atomic_write`）；`crates/live2d-ai-mod-template`
  模板 + `descriptor.api_version` 门禁（不兼容 → Failed 不崩）；`ModServices.apply_settings`
  **一等化**并**删除** `__apply_settings` 事件走私；补 `ModServices.settings`（脱敏读取）
  + `config_path`。② **schema 发现（M2）**：`ModFactory::settings_spec`（静态，未启用也拿得到）
  → `GET /api/v1/mods` 带 `config` + `settings_spec`（secret 脱敏）+ `GET …/config`；
  Flutter `ModsSection` 按 kind 渲 Bool/String/Number/Select + 保存，secret 留空不提交。
  ③ **角色卡标准 Mod（M5）**：`live2d-ai-mod-persona`（Rust）解析 SillyTavern V1/V2 JSON +
  PNG `chara` → 合成 `system_prompt` 经一等 `apply_settings` 写回，禁用时按
  `persona-mod-base.txt` 基线**还原**；主链 `[persona]` 只留 `system_prompt` +
  `max_history_pairs`（**破坏性：老 toml 卡字段会解析失败，升级需手改一次**），Flutter 删
  `persona_card.dart`/`persona_import.dart` 与导入 UI；Mod 数 3 → 4（`mod_count_is_four`）。
  ④ **Win stage-bg（M6）**：`Live2DStage` 挂桥/重建时补发 `stage-bg`；`saveDisplayPrefs`
  返回 bool（写失败给老实文案）；`pickImageDataUrl` 三态（读失败不再冒充取消）。
  **Win 肉眼验收待做**。门禁：cargo **822** 通过 / clippy 0 warning / rust-ratio
  **97.0776% PASS**；flutter analyze 无问题 + **819** 测试通过；`ignite.sh --check` 四项全 ok；
  API 端到端验证角色卡写回/还原。发布说明：`docs/releases/v0.1.0-rc.4.md`。
- **2026-09-13（v0.1.0-rc.3，结构质量）**：主链一行未改，收的是「挡住正式 0.1.0」的两类东西。
  ① **正文兜底（N0）**：上屏闸门是「该句语音已合成完毕」（同拍契约），代价是 TTS 故障 /
  半句切不出 / LLM 中途断流时**整轮一个字都不上屏**。新增 `EngineEvent::TextFallback` →
  `ConversationUiEvent::TextFallback` → WS **新帧 `text_fallback`**（覆盖式整段正文，
  不是残余；只在 `Failed` 发、在 `Terminal` 之前、健康轮永不发），前端气泡加
  「未收尾」说明行且**落盘**；契约写进 `core-chain-baseline.md` §2.1。
  **真机又抓到一个时序缺陷并修掉**：`run_one_turn` Stage A 的 biased `select!` 选中
  `gen_fut` 臂后不再回头 poll `event_rx`，引擎收尾时**同步发出**的那批事件被
  `drain_residual_events` 静默丢掉（`TextFallback` 首当其冲）→ 现在生成返回后先把
  通道排空；回归已实测「去掉修复即红」。
  ② **结构减法（N1）**：`main.dart` 1288→**543**（admin/settings/模型库/偏好回调外移到
  `app/shell_*.dart`，`package:web` 收进 `app/browser_io.dart`）；
  `surface.rs` 1167→**24**（render/input/idle/gpu，`IdleState` 逐字保留）；
  `ws.rs` 1073→**263**（audio/broadcaster 子模块）；`tests_models_routes.rs`
  1249→**4 个场景文件**（48 条测试一条不少）。
  ③ **双壳裁决（N2）**：egui 原生壳 / `--chat` **非主线**，休眠台账进 AGENTS
  （**选项 B：不 feature-gate**，理由与红线写在台账里）。
  ④ **半接线清干净（N3）**：`forcedByLaunchFlag` 不再写死 `false`（改由「有效
  dev_mode vs 落盘设置」推出）；修掉 `chat_panel.dart`「历史不落盘」的过时注释；
  UI 明示**会话记录 ≠ 模型记忆**；设置/诊断文案反向扫描无已删能力残留。
  ⑤ **门禁对齐（N4）**：`pr-checks.yml` / `nightly.yml` 补 `--all-targets` /
  `--doc` / `rust-ratio` / clippy `--all-targets`；新增
  `flutter-checks.yml`（`paths: shell/flutter/**`）——CI 之前**完全不管前端**；
  nightly 新增端到端探针 job（**只在配置 `LIVE2D_AI_VERIFY_BASE_URL` 时跑**，缺配置明确
  跳过，不伪造绿灯）；AGENTS 出一张「本地必跑 vs CI 必跑」表；`CONTRIBUTING.md` 整份
  重写（旧版还在教已归档的 Python 双端）。
  ⑥ **可读性收尾（N5，部分）**：仓库根 8 份 Python 时代旧计划 → `docs/legacy/`；
  `docs/design/` 4 份旧 JS 规格 → `docs/design/legacy/`（现行只剩
  `web-ui-spec-v3.md`）。
  门禁：cargo **801** 通过 / clippy 0 warning / rust-ratio **97.0054% PASS**；
  flutter analyze 无问题 + **822** 测试通过；`ignite.sh --check` 四项全 ok；
  `verify_core_chain.py --timeout 300` **18 跳全过**。
  发布说明：`docs/releases/v0.1.0-rc.3.md`。
- **2026-09-12（v0.1.0-rc.2，第二基线）**：rc.1 之后的第二个基线。范围冻结为「瘦版」
  （M0 点火纪律 + M1 动作裁决 + 模型闭环 + `.env` 密钥源 + 旧预览隔离）；
  `main.dart` 大拆 / `surface.rs` 大拆 / egui feature-gate / CI 对齐**都推到 rc.3**。
  ① **动作层删到底**：director Mod 整体删除、`SupervisorHandle::trigger_action` 与
  supervisor 的 `action_rx` 分支删除（那是**唯一**能把 `RootEvent::Action` 送进 core
  reducer 的路径）、`HostChannels.trigger_action` 删除（`ModServices.action_tx` 保留为
  Mod API 契约，但注入固定休眠 sender）、渲染面 `action-state` 接收器 + 编舞表删除
  （`surface.rs` 1721 → 1169）。**待机生命体征一行未动。**归档：`archive/action-layer-p6`。
  ② **capabilities 去广告**：删 `actions`/`action_sources`/`strength_levels`/
  `model_upload_supported`/`script_invoke_supported`（后两个是假广告），`schema_version` → 2。
  ③ **模型库闭环**：新增 `web_api/model_root.rs` 定为**唯一模型根**
  （`<cwd>/assets/models`，静态服务与 registry 同源；XDG 那条删除）、
  `find_model3_json` 向下看一层（原来连自家的 `bai/runtime/` 都导入不了）、
  `app/status.active_model_id` 读真实 registry、activate 恒 `requires_restart=false`
  且 `model_url` 可 GET；前端接上 `sendSync(model:)` 并**等渲染面 `loaded` 回执**才说
  「已切换」，同时补上一直缺的**导入入口**。
  ④ **`.env` = 唯一密钥真源**：新增 `live2d-ai-runtime::secrets`（快照读取，不用
  `set_var`）+ `GET/PUT /api/v1/env`（**永不回值**、原子写、`0600`、就地改行、热重载）
  + `.env` 纳入 `file_watcher`；前端设置面板可直接填 key。
  ⑤ **点火纪律**：`ignite.sh` 锚定 `LIVE2D_AI_FLUTTER_WEB_DIR` + `--check` 体检；
  503 响应体列出找过的每个路径；WSL2 ↔ Windows 分工写进本文件与 README。
  旧 JS 前端预览隔离到 `docs/design/legacy/` 并标注「**勿当现网**」。
  ⑥ **补丁（2026-09-13，rc.2 内，实测抓到）**：上游是**推理模型**，思考与正文共用
  `max_tokens`——512 时正文被挤成半句、**一个字都不上屏**（用户报「模型没有返回」）。
  修法：解析 `reasoning_content` 单列 `LlmEvent::ReasoningDelta` → WS **新帧
  `reasoning_delta`** → 前端气泡的**「思考」折叠区**；默认上限 512 → **4096**；
  「只有思考没有正文」单独收口。见下方「推理模型的思考」小节。
  门禁：cargo **795** 通过 / clippy 0 warning / rust-ratio **96.9754% PASS**；
  flutter analyze 无问题 + **813** 测试通过（含无头浏览器验收补丁）；`ignite.sh --check` 四项全 ok；
  `verify_core_chain.py`（长思考提问）**18 跳全过**。
  发布说明（含点火记录与已知问题）：`docs/releases/v0.1.0-rc.2.md`；
  **接手入口**：`docs/plans/HANDOFF-2026-09-13-rc2-second-baseline.md`
  （一分钟上手 / 门禁数字 / 交付态实测 / 推到 rc.3 的事 / 本轮新踩的七个坑）。
- **2026-09-11（v0.1.0-rc.1，核心链路基线）**：用户验收通过 → **交接落盘 + 标注基线**，
  随后裁决「**旧版本代码可只存在本地，仓库可以洗一下**」。本版：
  ① **版本线由 `0.5.1` 重置为 `0.1.0-rc.1`**（本版不是 0.5.x 的增量，而是核心链路重新
  定义后的第一个对外 RC；按 0.5.2 递增会让一次拆除看起来像小修补）；
  ② **LLM 工具层 + 动作系统从前后端整体拆除**（LLM 只做对话，请求体不再有
  `tools`/`tool_choice`/`functions`，有回归断言三键不存在）；**刻意保留但休眠**的是
  `live2d-ai-core` 的 action/performance 子系统、director Mod、`ModServices.action_tx`
  （仍属 Mod API 契约，无自动驱动方）；**必须保留**的是**待机生命体征**
  （`IdleState` 呼吸/眨眼/微表情，与动作系统是两套机制）；
  ③ **音频改走 `<audio>` 媒体元素 + Blob(WAV)**，静音/音量落在 `element.muted`/`volume`
  （不再折成 `GainNode`——站点级静音只作用在媒体元素上）；
  ④ **公开历史重新起算**：`main` 变成**单个根提交**，旧历史里的 Python/Android 工程、
  60 MB 调试 APK 与 **Live2D 模型二进制**不再公开（与本项目「模型不捆绑分发」的立场
  原本自相矛盾；公开 `.git` 256 MB vs 产品树 19.7 MB）；旧代码只存在本地。
  发布说明：`docs/releases/v0.1.0-rc.1.md`；基线逐环：`docs/architecture/core-chain-baseline.md`；
  交接：`docs/plans/HANDOFF-2026-09-11-core-chain-baseline.md`。
  门禁：cargo **765** 通过、clippy 0 warning、rust-ratio **96.95%** PASS；
  flutter analyze 无问题、flutter test **778** 通过；`scripts/verify_core_chain.py` **17 跳全过**。
- **2026-09-11（v0.5.1 追加）**：用户报「后端出错无具体错误代码 + 改动提示词就崩 +
  看后端日志什么都没有 + 前端无法知道错误信息」。查证**三条全对**：
  ① `dispatch.rs` 一行请求日志都没有；② 链路错误用 `println!`（不进 tracing 文件 sink）；
  ③ `AppEvent` 没有错误变体 → WS **从来不发** `error` 帧（前端那个 `WsErrorEvent`
  永远收不到实例）。修法：`ErrorKind::code()/stage()/is_fatal()/hint()` 错误码契约、
  `AppEvent::Error` + `error` 帧投影、请求级日志与 403 提到 `warn`、
  前端两条消费路径（`UiStateTracker` / `ChatController`）共用 `formatWsError`、
  保存失败 toast 带码、错误横幅「下一步」按码分流。**顺带查出真 bug**：
  手改 `live2d-ai.toml` 后 `GET /settings` 不跟随磁盘，下一次界面「保存」会把
  手改内容**覆盖**掉 → 新增 `StatusContext::refresh_from_disk`（回归
  `web_api/tests_reload.rs`）。复核：改提示词本身不失败（10 组取值全 200），
  当前那条失败是进程环境缺 `DEEPSEEK_API_KEY` 的 401。门禁 812 + 627 全绿。
- **2026-09-11（v0.5.1）**：v0.5.0 交付后接浏览器工具做**真机验收**，
  抓到**十二个 bug**（大部分逃过了当时全绿的 810+590 条测试）：
  ① 直接点「设置」不加载（面板写「读不到服务端设置」）；
  ② `showModalBottomSheet` 的 builder 只跑一次 → medium/compact 设置浮层**永远转圈**；
  ③ 路由把查询串当路径 → `GET /api/v1/logs?limit=200` 回 501，诊断日志从未成功；
  ④ `dev_mode` 的开关被自己所在的分区藏起来 → 界面上**永远打不开**；
  ⑤ **保存设置会抹掉配置文件全部注释**（`toml::to_string` 重生成文档）——
  当天真的发生一次，改用 `toml_edit` **就地改值**（`merge_into_toml`）；
  ⑥ UI 文案露出 Markdown `**`（15 处）→ 新增 `EmphasizedText` 真渲染 + 扫描规则；
  ⑦ 切主题后舞台不变色（`_applyPrefs` 读了尚未更新的 `widget.prefs`）；
  ⑧ 缩放读数永远是 `—`（没人读渲染面首帧的 `stage-ack`）；
  ⑨ 老 JS 前端占着 `/` 与 `/app` 是两套界面 → `/` 302 到 `/app/`，JS 前端整套删除；
  ⑩ 状态胶囊冒充「后端未连接」（`bridgeError → offline` 口径错 + 信号只置位不清除，
  普通刷新就能撞上）→ 删掉那条派生，渲染面失败只留在舞台自己的覆盖层上；
  ⑪ **压在舞台 iframe 上的控件全都点不着**（用户报「设置能唤醒，但点不动、不能上下滑」）
  → 新增 `StagePointerInterceptor` 指针垫层（透明 `<div>` 平台视图，零新依赖），
  设置面板 / 断线横幅 / 缩放角标 / 两处错误重试逐个接线（见上方前端层约定）；
  ⑫ **删掉左侧分区 rail**（用户裁决「把左边的这些设置一级选项去掉留给舞台」）——
  那一列 168 px 还给舞台，设置入口只剩 AppBar 的「设置」一处，分区切换在面板内的
  chip 行（顺带删掉「再点一次 rail 上同一分区收起」的切换语义）。
  浏览器工具到位后还重验了：本次加载**外部源请求 0**（断网红线通过），
  并端到端跑通圆形↑发送键（输入 → 点按钮 → 用户气泡 → LLM 回复）。
  教训：**测试全绿 ≠ 界面是对的**；异步时序、浮层构建时机、平台视图、
  精确路径匹配这四类问题只有真的在浏览器里点一遍才会露出来。
- **2026-09-11（v0.5.0）**：用户裁定「**只核心链路做丰满即可**」+「前端 ui 很 ai 化同质」。
  据此**先删后改**：动作手动触发移出成品（只留分支 `archive/action-trigger-p5`
  与 `docs/design/web-action-trigger-archive.md`）；清掉没有功能的占位 UI；
  **本地 TTS 从 Mod 提升为核心链路**（`docs/architecture/tts-is-core.md`，
  Mod 5 → 4）；新增**黑/白/蓝/灰四套配色**（主题住本地偏好，点一下立刻生效）；
  舞台改**纯色底**（协议新增 `sync.stageColor`，由渲染面写——iframe 内 canvas
  会盖住父页画的底色）+ 可选用户展台图；新增**组件外观统一层**
  `buildAppComponents`（elevation 全 0、圆角/字级统一）与「**发送 = 圆形上箭头**」
  「**尽量少用图片，用文字做按钮**」两条形状约定（`test/visual_language_test.dart`）。
  **删掉老 JS 前端**：`/` 302 → `/app/`，一个服务只有一个界面入口。
  另修三个「只有真浏览器才抓得到」的 bug（三个都逃过了当时的全部测试）：
  ① 直接点「设置」不触发加载 → 面板显示「读不到服务端设置」；
  ② `showModalBottomSheet` 的 builder 只跑一次 → medium/compact 的设置浮层
  **永远转圈**（内容不跟数据重建）；③ 路由把查询串当路径 →
  `GET /api/v1/logs?limit=200` 回 **501**，诊断面板的日志区从来没成功过。
- **2026-09-10（v0.4.4 / v0.4.5）**：确立前端**离线可用**的两条硬约束——
  构建必须带 `--no-web-resources-cdn`（否则 CanvasKit 走 Google CDN，断网白屏）；
  中文字体必须自托管（否则断网豆腐块）。两者都是「有网时看不出来」的缺陷。
- **2026-09-10（v0.4.3）**：语音输出约定新增「**默认出声**」与「主音量/静音与口型正交」；
  静音改为默认关闭的服务端开关 `LIVE2D_AI_MUTE_AUDIO=1`（跑测试/无人值守用）。
- **2026-09-10**：确立 **Rust 核心 + Flutter 前端** 双主导分层；前端/接口层
  自 `rust-ratio` 显式豁免并改由 Flutter 工具链门禁；原生 JS 前端降为遗留；
  新增「语音输出约定（一句一单元，不断句）」。
- **2026-08-31**：按 Rust 主线重写（移除 Android/Python 双端描述）。
- **2026-08-09**：重写——移除 pi 专属路径引用，保留项目级通用信息。
