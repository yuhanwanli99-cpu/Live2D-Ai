# Live2D-Ai — rc.2「第二基线」改良长计划

> **状态**：2026-09-12 起草（WSL2 工作区）
> **上游输入**：① win 侧 `docs/plans/PLAN-CORE-ENHANCEMENT.md`（步骤 1 移交计划，未入库）；
> ② 后端自查评审；③ 前端源码评审（口径 = 可读性 / 工程优雅，UI 观感未验，`/app/` 503）
> **本文件是步骤 1 的唯一真源**；与 win 侧那份或旧节点 plan 冲突时，以本文件 + 现行 `main` 为准。
> **口径**：个人项目；可读性与工程优雅优先；弱化安全；**宁删勿加**；不做步骤 2（Mod 功能堆砌）。
> **工作区纪律**：核心开发**一律在 WSL2** `/home/skystar/Live2D-Ai`；win 侧
> `C:\Users\33784\.copilot\repos\Live2Dai` 只承担「Flutter Web 产物 / 用户点火测试」的联动，
> 不做第二套真相。

---

## 0. 目标与版本路线

**一句话**：把「文本 → LLM 纯对话 → TTS → 口型 → Live2D 渲染 + Flutter UI」做成**唯一清晰、
可阅读、可改动**的产品真相，并把挡住验收的工程债（半拆除、假广告、点不着火）一次清干净。

| 版本 | 定位 | 内容 |
|---|---|---|
| **0.1.0-rc.2**（第二基线） | **本计划的主交付（已冻结为「瘦版」）** | **M0 + M1 + §5.2 A（模型闭环 A1→A2）+ §5.4（`.env` + `PUT /api/v1/env`）+ §8.3.1（旧 JS 预览隔离）** |
| 0.1.0-rc.3 | 结构性减法 | **M2-B 的 God Object 大拆（`main.dart` ≤700）**、**M3 整包**（大文件止血）、egui feature-gate、双壳/双音频裁决落地 |
| 0.1.0（首个正式） | 稳定 | M4 门禁对齐（CI = 本地一套真相）+ M5 可读性收尾 |

> **rc.2 范围冻结（2026-09-12 审阅修订，见 §9.4；2026-09-13 补冻结 §5.4）**：只做上面加粗那五项。
> `main.dart` 拆分、`surface.rs` 大拆、egui feature-gate、M3 整包**都不挡 rc.2 的 tag**——
> 能做就做，做不完成 rc.3。**主路径可读**从 rc.2 的硬指标降为软目标（文案/假广告类仍属硬指标）。

**rc.2 的「第二基线」定义（DoD，可验收）**：

1. 「动作」在产品路径上**不存在**：无注入、无广告、无文档矛盾，且 core 动作子系统有**明文休眠裁决**；
2. `GET /` 302 → `/app/` → **Windows 浏览器能打开、能对话、能出声、口型动**，且**模型库（含导入的模型）激活即换皮**（一次闭环点火的记录进 release note）；
3. **本地门禁全绿**（含 `rust-ratio`）——**CI 对齐不在 rc.2 范围**，归 rc.3 / M4（避免自己卡自己）；
4. `docs/releases/v0.1.0-rc.2.md` 写清**删除了什么**；版本号在三处同步（`Cargo.toml` / capabilities / 前端 `pubspec.yaml`）。

---

## 1. 非目标

- 新 Mod、完善 director / pet-desktop / external-input 产品能力（步骤 2）
- 重新引入 LLM tools / 动作编排作为产品能力
- 大范围「优雅架构」重写、为拆而拆的 crate 爆炸
- 安全 hardening、企业级观测、多端（Android / 原生 Windows）作为主线
- 会话记忆后端、复杂 settings schema 扩张
- 新视觉、新动画、动作调试器

> 若任务看起来属于上列：**停下来**，标到 §10 backlog，不要混进本计划 PR。

---

## 2. 事实基线（实测，2026-09-12）

| 事实 | 数值 / 位置 |
|---|---|
| rust-ratio | `rs 53682 / py 1689 = 55371` → **96.9497% PASS**（门槛 95）；dart 豁免 29035 |
| Rust 占比可删上限 | 删除 Rust 会**拉低**占比；按 95% 门槛，最多可删 ≈ **21,600** 行 |
| desktop 复杂度 | `live2d-ai-desktop` **31,853** 行（占 Rust 大半） |
| 超线文件（生产 ≤500 / 测试 ≤800，豁免 ≤1000） | `surface.rs` 1721、`tests_models_routes.rs` 1121、`ws.rs` 1073、`tests_ws.rs` 954、`runners_surface.rs` 878、`mod-local-llm/lib.rs` 806 |
| Flutter 前端 | `shell/flutter/lib` **70 文件 / 15,906 行**；最大 `main.dart` **1180 行** |
| **模型库 ↔ 舞台断耦** | 前端侧：`ActivateResult.modelUrl` 已解析（`api/models_api.dart:83/92/100`）但**全仓只有它自己引用**；`main.dart:1065` 构造 `Live2DStage` **不传 `model:`**；而 `Live2DBridge.sendSync(model:)`（`live2d_bridge.dart:140/151`）与渲染面 `sync.payload.model`（`l2d-wasm-demo/src/main.rs:300-307`）**都已实现** |
| **模型双轨（后端，**推翻**「只差前端一根线」）** | 静态 `/models` 读 **cwd `assets/models/`**（`wasm_assets.rs:191`）；import / registry 写 **XDG `~/.local/share/live2d-ai/`**（`models_routes/registry.rs:5`）→ 两边不是一个根。**即便前端补上 `sync.model`，imported 模型也不会热换**（iframe 仍拉 cwd）。见 §5.2 |
| **`find_model3_json` 只扫顶层** | `models_routes/util.rs:11-24`：只遍历 `dir` 第一层。而仓库布局是 `assets/models/bai/runtime/bai.model3.json`（**深一层**）→ 按 `id=bai` 导入找不到 model3、易回 400 |
| **`app/status.active_model_id` 写死** | `app_routes.rs:212` 硬编码 `"bai_001"`，从不读 registry；`dto.rs:59` 自述「v1 = bai_001；D3 实现可配置」 |
| **upload 也是假广告** | `model_upload_supported: true`，而 multipart ZIP 端点 `POST /api/v1/models` 明标 **D3.2 后置、本批不实现**（`models_routes/mod.rs:30-31`） |
| **密钥源现状（待改为 `.env` 单一源）** | Rust 只读**进程环境**（`settings.rs:287`、`supervisor.rs:312`、`chat.rs:84`）；`.env` 由 `scripts/ignite.sh:23-27` 以 `set -a; . ./.env` 导入，脚本注释明写「**Rust 侧只读进程环境，不读 .env 文件**」；`.env` 已在 `.gitignore:31`、权限 `600`、含多把真实密钥；`.env.example` 只有 `DEEPSEEK_API_KEY` / `ZHIPU_API_KEY` 两个键名 |
| 名实不符 · 半接线 | `chat_panel.dart:20` 注释仍写「聊天历史**不落盘**」，与 `chat_controller.dart:58`（落盘回调）、`main.dart:360`（按轮落盘）矛盾；「历史轮数」易被读成「模型记忆」；`main.dart:939` `forcedByLaunchFlag: false` 写死；capabilities 的 upload/script 布尔接了但**诊断页不展示**；多份 API client 重复 normalize；`shell/flutter/README.md` 仍是模板 |
| 组合根过重 | `main.dart` 名义装配根、实际 God Object：API / WS / 音频 / 聊天 / 设置 / 模型 / Mod / 诊断全捏在一起；手写 `ChangeNotifier`（无 Riverpod）使代价全集中于此 |
| 动作注入仍活着 | `cli_entry.rs:145-171`（HostChannels `trigger_action`）→ `supervisor.rs:254/563`（`RootEvent::Action`） |
| 动作广告仍活着 | `web_api/dto.rs:32/38/277-280`（`actions`/`action_sources`/`strength_levels`/`script_invoke_supported`），而 `/api/v1/commands*` 已删（`web_api/mod.rs:36-38`） |
| **面向 LLM 的谎言** | `live2d-ai.toml.example:69`「若需要配合动作，再额外调用 `live2d_perform_action`」——示例配置会直接注入 system prompt |
| director 引用点 | 仅 3 处：根 `Cargo.toml:15`、`desktop/Cargo.toml:48`、`main.rs:57`；`mods.json`/`toml.example`/`verify_core_chain.py`/`xtask` **零引用** |
| 前端动作残件 | 仅注释考古（`main.dart:324/439`、`turn_liveness.dart:11`、`appearance_section.dart:12`）；解析 `scriptInvokeSupported` 但**无人显示** |
| 文档矛盾 | `core-chain-baseline.md:63`「没有任何路径会向 core 发 `Event::Action`」与上表第 6 行冲突 |
| 设计文档过厚 | `docs/design/` 8 项并存：`web-ui-spec-v2/v3`、`polish-spec`、`redo-spec-st1-2`、`settings-wiring-v1`… |
| **旧 JS 预览易误判** | `docs/design/ui-preview-v2.html` + `docs/design/assets/preview-{main,modal,actpanel}.png` = **旧原生 JS 前端**的静态预览（v2 token 时代的 HTML mock），实测**零代码引用**，却和现行 Flutter `/app/` 摆在同层——本地 agent 与维护者都会误当现网（处理见 §8.3） |
| CI 与本地差距 | `pr-checks.yml` / `nightly.yml` **都**只跑 `cargo test --workspace`：无 `--all-targets`、无 `--doc`、无 `verify_core_chain.py`、无 flutter job |
| 空长期分支 | `origin/mainline/1-core-baseline` == `origin/main`（纯指针，违反「不要为步骤 1/2 开空 mainline 分支」） |
| `/app/` 503 根因 | WSL 侧产物**完整且新鲜**（`shell/flutter/build/web/index.html` + `main.dart.js` 2.89 MB）；win 侧**无 `build/`**；`flutter_app.rs:64-80` 的 `build_dir()` 只认 `LIVE2D_AI_FLUTTER_WEB_DIR` 与两个相对 cwd 候选 |

---

## 3. M0 — 点火纪律（WSL2 ↔ Windows 分工）【rc.2 前置】

**问题**：用户看不到活 UI（`/app/` 503）。根源不是前端代码，是**三方约定没写下来**：
谁编、谁跑、谁验，以及 cwd / 产物目录如何解析。

### 3.1 明文约定（写进 `AGENTS.md` + README）

| 角色 | 职责 |
|---|---|
| **WSL2**（唯一开发环境 + **唯一进程宿主**） | 后端 `cargo` 全部构建与测试；**服务进程一律在 WSL 起**（`./scripts/ignite.sh`，端口 18080）；Flutter SDK 在 `~/flutter`（`ignite.sh:20` 已 export PATH）；`flutter build web --release --base-href /app/ --no-web-resources-cdn` |
| **Windows** | 只做两件事：开浏览器点 `http://127.0.0.1:18080/app/`；把观感反馈写回。**不跑二进制、不编 Flutter** |
| **产物** | **单一真源 = WSL 的 `shell/flutter/build/web`**；`LIVE2D_AI_FLUTTER_WEB_DIR` 一律写 **WSL 路径**（`/home/skystar/Live2D-Ai/shell/flutter/build/web`）。**不要**引入 Windows UNC 路径写法——只有「哪天真的在 Win 上跑二进制」才需要，那不在本计划内 |

### 3.2 工作项

1. `flutter_app.rs` 的 `build_dir()`：候选列表补「相对可执行文件」与「相对 workspace 根（向上找 `Cargo.toml`）」，
   并让 503 响应体**列出实际找过的路径**（现在是通用指引，排障时一片空白）。
2. `scripts/ignite.sh`：显式 `export LIVE2D_AI_FLUTTER_WEB_DIR="$PWD/shell/flutter/build/web"`，
   使 cwd 误判从根上不可能；保留「产物缺失 → `--build` 自动构建」。
3. 加一条点火探针（`scripts/ignite.sh --check` 或 `xtask`）：`GET /` = 302、`GET /app/` = 200、
   `index.html` 与 `main.dart.js` **不含 `gstatic.com/flutter-canvaskit`**。
4. 前端版本号与后端对齐：`shell/flutter/pubspec.yaml:4` 现为 `1.0.0+1`，与 `0.1.0-rc.2` 漂移——
   至少在 release note 记明两者关系，最好同步。

### 3.3 验收

- [ ] WSL2 `./scripts/ignite.sh` 后，Windows 浏览器打开 `/app/` **200**，能发一条消息并听到声音、口型动
- [ ] 断网（或断 gstatic）下 `/app/` 仍可加载（`--no-web-resources-cdn` 生效）
- [ ] 这次点火的时间、端口、浏览器、截图结论写进 `docs/releases/v0.1.0-rc.2.md`「点火记录」

---

## 4. M1 — 动作层一次裁决（删到底）【rc.2 核心】

### 4.1 决策（冻结）

**产品冻结无动作；core 动作子系统保留但明文休眠。** 不做「仅 Mod 实验保留完整动作协议」的中间态（那是步骤 2）。

### 4.2 工作项（agent checklist，按此顺序）

0. **先归档再删**：开 `archive/action-layer-p6` 分支（或 tag）冻结现状——
   与 `archive/action-trigger-p5` / `py-legacy` / `android-archive` 的先例一致。
   **删除类 PR 合并前必须有归档点**，否则这是仓库第一次不可回滚的大删除。
1. **摘 director**（最便宜，3 处）：根 `Cargo.toml:15` members、`desktop/Cargo.toml:48`、`main.rs:57` FACTORY。
   顺带加一条**防回归断言**：注册表工厂数 = 3（防止再挂回去）。
2. **capabilities 去广告**：`dto.rs` 删 `actions` / `action_sources` / `strength_levels` /
   `script_invoke_supported` / **`model_upload_supported`**（multipart ZIP 端点明标
   `models_routes/mod.rs:30-31` **D3.2 后置、本批不实现**，广告不得为 supported）；
   **同步改** `dto.rs` 的 `app_info_carries_version_and_stable_fields`
   （它现在断言 6 动作 / 3 source）。前端已先行删净（`diagnostics_api.dart:21`），只需
   同步删掉仍被 parse 但无人显示的 `scriptInvokeSupported`。
3. **断产品路径上的 Action 注入**：`cli_entry.rs:145-171`、`mod_registry.rs:17-49/219-224`、
   `supervisor.rs:159-165/240-255/550-566`。目标：`ignite` / `--web` 路径**没有任何**实现会发 `RootEvent::Action`。
4. **文档裁决记录**（一次改完，不留新矛盾）：
   - `AGENTS.md`：把「刻意保留但休眠」的 action/performance 与 `ModServices.action_tx` 归属一次说清
     （含「谁休眠、为什么、谁能唤醒」）；
   - `core-chain-baseline.md:52-63`：删掉「没有任何路径会向 core 发 `Event::Action`」或改成删路径后的真话；
   - **修 `live2d-ai.toml.example:69`**：删掉教 LLM 调 `live2d_perform_action` 的那一行——这是当前唯一
     会主动误导模型的动作残留（示例配置直接进 system prompt）。
5. **WASM 编舞：单独 PR + 人工验收**（见 §4.3 的假验收警告）。
6. **core 内 action/performance 的归属写死**：`cfg` 隔离或 `doc(hidden)` + 豁免台账登记；
   **禁止**在本里程碑「修好导演再接回来」。

### 4.3 假验收警告（**必须按此改，否则会漏掉真回归**）

`surface.rs` / `main.rs` 的编舞代码位于 `#[cfg(target_arch = "wasm32")] mod web`，
**`cargo test --workspace --all-targets` 结构性编不到它**。因此：

- 该改动的验收 = **wasm 重建 + 肉眼确认皮套仍渲染、口型仍动、待机呼吸/眨眼仍在**
  （`core-chain-baseline.md:66-89` 已给出同样结论）；
- **绝不要连带删掉** `IdleState` / `idle_enabled`——待机生命体征与动作动画只是共用 override 层；
- 删除前先读 `core-chain-baseline.md §3.3` 的三条「为什么不删」理由；若要推翻，**在 PR 里写明理由**。

### 4.4 验收

- [ ] `cargo test --workspace --all-targets` + `cargo test --doc --workspace` 绿
- [ ] `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` 干净
- [ ] `cargo run -p xtask -- rust-ratio` ≥ 95%（删除 Rust 会拉低占比，需复核）
- [ ] `python3 scripts/verify_core_chain.py` 全跳通过
- [ ] `rg -n "script_invoke_supported|action_sources|model_upload_supported|live2d_perform_action" crates/ live2d-ai.toml.example` **无输出**
- [ ] `rg -n "Event::Action" crates/live2d-ai-desktop/src crates/live2d-ai-mod-*` 只剩测试
- [ ] `flutter analyze` + `flutter test` 绿；`/app/` 点火回归通过（§3.3）
- [ ] 默认 `ignite`/`--web` 文档与代码一致：「无产品动作」

### 4.5 非目标

- 不实现新的动作播放器；不为 director 补情感映射；不动 core 的动作**类型**语义

---

## 5. M2 — 产品面可用与名实相符（模型闭环 / 密钥源 / 瘦身）

### 5.1 决策

**默认产品路径** = WSL2 `ignite` → Windows 浏览器 `/app/` → HTTP + WS（Flutter Web）。
原生 egui 设置、`--chat` REPL、`--pet-mode`：**降级为 feature gate 或文档标明「非主线 / 实验」**，
并登记**休眠台账**（谁休眠、为什么、谁能唤醒）——不留第三种含糊态。

### 5.2 工作项 A（**P0，rc.2 必做**）：模型闭环 = 后端统一模型根 + 前端接线

**结论更正（2026-09-12，后端源码核对后）**：先前判断「只差前端一根线」**不成立**。
前端那根线确实是缺口（`main.dart:1065` 不传 `model:`），但**接上也换不了皮**——因为后端是
**模型双轨**：静态 `/models` 读 cwd `assets/models/`（`wasm_assets.rs:191`），
而 import / registry 写 XDG `~/.local/share/live2d-ai/`（`models_routes/registry.rs:5`）。
iframe 永远拉 cwd，imported 模型不在那里。**所以顺序是：先后端统一模型根，前端才有真源可接。**

**A1 · 后端（先做）**

1. **统一模型根（已裁决，不再二选一）**：**唯一真源 = 仓库 `assets/models/`**。
   理由：仓库已有 Bai、`/render` 默认就按 cwd 路径取、`--web` 是唯一产品面。
   - 静态 `/models` 继续读 `assets/models/`（`wasm_assets.rs:191`）；
   - `import` 也**写这里**（`assets/models/<id>/`），registry 记录同一根下的 id；
   - XDG `~/.local/share/live2d-ai/models/` **降级为兼容读**（读到即提示迁移），
     或在 rc.2 直接删掉这条路径并在 release note 说明——**不得再出现两个根**。
2. **`find_model3_json` 递归一层**：`models_routes/util.rs:11-24` 现在只扫顶层，
   而仓库布局是 `assets/models/bai/runtime/bai.model3.json`（深一层）→ 按 `id=bai` 导入必然找不到。
   递归深度限一层、仍拒绝符号链接外指（沿用现有安全约定）。
3. **`app/status.active_model_id` 读真实 registry**：`app_routes.rs:212` 写死 `"bai_001"`，
   与 activate 的结果无关——这是「激活了但状态栏没变」的根源。
4. **热换契约（已冻结，不再二选一）**：
   - `activate` = **只改 registry + 返回可 `GET` 的 `model_url`**（`/models/<id>/....model3.json`），
     **后端不推送、不通知渲染面**；
   - **换模由前端 `sendSync(model: url)` 发起**，以渲染面 **`stage-ack` / `loaded` 回执**为准；
   - **`requires_restart` 在热切可行时改为 `false`**（渲染面本就支持热换）；若某模型确需重启，
     由后端逐案返回 `true` 并说明理由——**不允许恒为 `true`**；
   - WS 主动推送模型变更 **留 rc.3**，rc.2 不做。

**A2 · 前端（A1 之后）**

1. 激活成功后把 `modelUrl` 送进舞台：`Live2DStage(model: ...)` + `sendSync(model: url)`；
   **先收渲染面回执（`stage-ack` / `loaded`）再提示「已切换」**，不得在收条前谎报已生效
   （沿用 `live2d_stage.dart:72-74` 已有的 R2 规矩）。
2. `requires_restart: true` 时：**先试热切**（渲染面本就支持），确需重启才提示重启，
   文案与真实行为一致——现在最容易误导人的正是这一格。按 A1.4 的冻结契约，
   **后端恒返回 `true` 视为契约违规**，前端一律以 `stage-ack` / `loaded` 回执为准。
3. **导入入口（rc.2 必须接上）**：接上 `POST /api/v1/models/import`（`models_api.dart:140`，
   现无任何 UI 调用）的按钮——因为 DoD 的端到端要跑「导入 → 激活 → 换皮」。
   退路：若导入 UI 确实延期，则把 DoD 的 e2e 降为「仓库 Bai 激活即换皮」，
   并**同步删掉** `dev_tools_section.dart:185` 那句「然后在这里导入」。**不许两者都不做**。
4. 端到端验收：Windows 浏览器里 `模型库 → 导入 → 激活 → 皮套真的换了`（**不重启后端**），
   记录进 `docs/releases/v0.1.0-rc.2.md` 的点火段。

### 5.3 工作项 B（P1/P2）：瘦身与名实相符

> **范围标记（§0 冻结）**：本节的**文案 / 假广告 / 旧预览 / 真相修正类**属 **rc.2 硬指标**；
> **`main.dart` 大拆、egui feature-gate、结构性重排**属 **rc.3 软目标**（能做就做，不挡 rc.2 tag）。

1. **入口图**：`AGENTS.md` 或 `core-chain-baseline.md` 画一张只含主路径的图
   （`cli_entry` / `chat_routes` / `ws` / Flutter `lib/`），新人只读 5 个文件能口述主链。
2. **前端对齐（吸收前端评审）**：
   - 设置面板默认只突出 **LLM / TTS / 模型库 / 对话** 四条主路径；Mod / 诊断 / 开发模式**折叠或进「高级」**；
     顺带拆文件：**主路径的「模型库」现在住在 `settings/sections/dev_tools_section.dart` 里**
     （该文件同时装 Mod/诊断/模型库/开发模式四个分区）——名实不符，按主路径优先重排；
   - 删掉误导性的能力展示（capabilities 谎言、默认 persona 里的动作话术）；
   - `main.dart`（1180 行）**拆**：`admin`（Mod/诊断）、`settings wiring`、`模型库接线` 挪出组合根，
     组合根只留装配（目标 ≤700）；
   - 注释考古收敛：全库「2026-09-11 裁决 / 钉子 N」式墓志铭 → 「不变量三行 + 链接 docs」，
     保留 `app_shell` 那种真正解释**为什么**的头注；
   - **「会话记录 ≠ 模型记忆」**：`POST /api/v1/chat` 只发 `{text}`，多会话落盘 ≠ 模型有记忆——
     UI 文案写清；同时修掉 `chat_panel.dart:20` 过时的「历史不落盘」注释；
   - 诊断页**展示** capabilities 的 upload/script 布尔（或直接删字段）；`main.dart:939` 的
     `forcedByLaunchFlag` **接线或删掉**（写死 `false` 是半接线）；
   - 多份 API client 的重复 normalize 去重；重写 `shell/flutter/README.md`（现在还是模板一句）。
3. **后端**：
   - `settings_ui.rs`（601 行，自带 ≤1000 豁免）：**先只做文档标注非主线**；只有实测编译/体积收益明显才 gate
     （会动 Cargo feature 矩阵与 `cli/tests.rs` 的 `--pet-mode`/`--web` 断言）；
   - supervisor 内 cpal / native dry-run 分支：能 feature-gate 则 gate；注释只留不变量；
   - `dist/` 下过期 Python 产物树（rust-ratio 已排除，但会误导阅读）清理或指针化。
4. **注释考古迁出源码**：D11/D13/节点编号史诗 → `docs/plans/` 历史文件。

### 5.4 工作项 C：`.env` = 唯一密钥源（契约，2026-09-12 用户口径）

> **口径（冻结）**：LLM / TTS 的 **端点与 API key 一律以 `.env` 为存储与读取来源**；
> 前端**可以写入 `.env`**，写入后**热重载**（不重启后端）。

**为什么是它**：现在「改了设置却 401」这类问题的根因是**密钥只存在于后端进程环境**——
`ignite.sh` 用 `set -a; . ./.env` 导入，Rust 侧从不读 `.env`
（`scripts/ignite.sh:23-27` 的注释把这条写死了）。前端能改模型名，却改不了 key。

1. **契约**：`resolve_with` 的查找源从「进程环境」改为「**`.env` 快照**」；
   优先级写死并文档化：**`.env` > 进程环境 > 无（不鉴权）**；`live2d-ai.toml` 继续只持
   `api_key_env`（环境变量**名**，现状不变），键名映射仍是唯一入口。
2. **Rust 读 `.env`**：最小实现（`KEY=VALUE` + `#` 注释 + 可选引号，约 50 行）或引入 `dotenvy`——
   按「宁删勿加」倾向前者；**不要**用 `std::env::set_var` 回灌环境
   （Rust 2024 下是 `unsafe`，多线程下不可靠）→ 走**快照**读取。
3. **前端写入（新 mutating 端点，已冻结）**：
   - **`PUT /api/v1/env`**（body：`{"key":"DEEPSEEK_API_KEY","value":"..."}`，`application/json`）——
     **不**并入 `PATCH /api/v1/settings`（与 settings 解耦；2026-09-13 用户裁决）；
   - 写盘走既有原子写语义（`plan_atomic_write`：tmp → `fdatasync` → rename），**文件权限 `0600`**；
   - 加入 `is_mutating_route`（`security.rs:310`）→ 自动获得 Origin + Content-Type 强制校验；
   - **永不回显**：`GET` 只回「键名 + 是否已设置」布尔，**不得**回值；写操作日志**不记录 body**。
4. **热重载**：写入成功后刷新密钥快照 + 触发 supervisor 重载；`.env` 纳入 `file_watcher`
   的监视集（与 AGENTS「配置快照必须与磁盘一致」同一条纪律——外部手改 `.env` 也要跟，
   否则下一次界面保存会把手改内容覆盖掉，`StatusContext::refresh_from_disk` 是同一类修法）。
5. **安全红线（沿用 AGENTS，不放松）**：密钥不进 GET / 日志 / WS / 导出；loopback-only；
   `.env` 已在 `.gitignore:31`（保持）；`.env.example` 只保留**键名**占位。
6. **迁移**：`scripts/ignite.sh:23-27` 的「Rust 侧不读 `.env` 文件」注释必须改掉；
   `set -a; . ./.env` 可保留（向后兼容），但**文档要写清 `.env` 是唯一真源**。

**验收**：

- [ ] 前端填入 key → 保存 → **立刻能聊**（不重启后端、不重跑 `ignite.sh`）
- [ ] 手改 `.env` 后 `GET /api/v1/settings` 跟随磁盘（不被旧快照覆盖）
- [ ] `rg` 明文 key 在 `GET` 响应 / `server.log` / WS 帧里**搜不到**
- [ ] 非 loopback 来源或缺失 `Content-Type` 的写入被拒（现有 mutating 门禁生效）
- [ ] `.env` 仍为 `0600`、仍在 `.gitignore` 内

### 5.5 验收

- [ ] **模型闭环（§5.2 A1+A2）**：模型根唯一；`find_model3_json` 能认 `bai/runtime/` 布局；
      `app/status.active_model_id` 反映真实激活；**导入 → 激活 → 皮套真的换了**（不重启后端）；
      空态文案与真实入口一致
- [ ] 新人只读 `README.md` + `core-chain-baseline.md` + `cli_entry` + `ws` + Flutter 入口，能口述主链
- [ ] 默认 release 二进制以 `--web` 为推荐入口；native UI 不挡编译（或明确 feature）
- [ ] 界面上**不再出现**任何已拆除能力的字样（人工点一遍 + `rg` 双向验证）
- [ ] `rg -n "不落盘" shell/flutter/lib` 无过时注释；UI 明示「会话记录 ≠ 模型记忆」
- [ ] `forcedByLaunchFlag` 不再是写死 `false`（接线或删除）
- [ ] `flutter analyze` / `flutter test` 绿；`main.dart` ≤700 为 **rc.3 软目标**（不挡 rc.2 tag）

---

## 6. M3 — 大文件止血

### 6.1 原则

- **先删死肢，再考虑拆文件**；拆分按行为边界，禁止「为过门禁平均切文件」
- 三条可数硬指标：**生产 ≤500**（豁免 ≤1000 且**数量不增**）、**测试 ≤800**、**豁免头注总数 ≤ 执行前**
  （当前 `crates/` 内「豁免」字样实测 ≥12 处 / 9 个文件，可作为基线；只减不增）
- **大砍测试前先跑 `rust-ratio`**：删 `.rs`（含测试）会同时压分子与分母，**合并前先量一次**——
  按 95% 门槛只剩 ≈2.16 万行余量，别为了过文件行数门槛把占比打穿（§2、§6.3）

### 6.2 优先文件

| 文件 | 现状 | 方向 |
|---|---|---|
| `l2d-wasm-demo/src/web/surface.rs` | 1721 | **先更正预期**：删编舞只减约 186 行（31 处命中 + 约 150 行测试），删完仍约 1520 行 > 1000——必须自己立项拆「渲染 / 输入 / 待机」，且这是 **wasm 门控、原生测试零覆盖**的最高风险一刀 |
| `web_api/ws.rs` | 1073 | 音频帧 vs 连接生命周期 |
| `.../tests_models_routes.rs` | 1121 | **已超测试 800 线**：按场景切开或删冗余套件 |
| `supervisor.rs` / `mod_registry.rs` / `turn.rs` | — | 删注释与死分支优先于再抽象 |
| `mod_registry.rs` | 745 | Mod 注册 + HostChannels 映射：M1 摘 director / 断 Action 注入后应显著下降 |
| `web_api/cli_entry.rs` | 608 | 装配「史诗」：只留主路径装配，其余进函数或挪 Mod 边界 |

### 6.3 验收

- [ ] 生产源码无「无头注且 >1000 行」；测试无 >800 行（或登记新豁免并说明）
- [ ] 豁免头注数量 ≤ 执行本计划前
- [ ] `rust-ratio` ≥ 95%（拆文件不改占比；删测试会拉低，需复核）

---

## 7. M4 — 门禁对齐（CI = 本地一套真相）

### 7.1 工作项

1. **`.github/workflows/pr-checks.yml` / `nightly.yml`**（现状两者都只有 `cargo test --workspace`）：
   - 补 `--all-targets`（否则集成测试与 wasm target 压根不跑——这正是「CI 比本地瘦」的实质）；
   - 补 `cargo test --doc --workspace`；
   - `scripts/verify_core_chain.py`：真放进 workflow（PR 或 nightly 二选一，**由本计划指定：nightly**，PR 只跑快速集）；
   - 补 `cargo run -p xtask -- rust-ratio`（本地硬门槛，CI 目前完全没有）；
   - Flutter job：`paths` 过滤，改 `shell/flutter/**` 才跑 `flutter analyze` + `flutter test`。
2. **一句真相**：`AGENTS.md` / `CONTRIBUTING` 写「本地必跑」vs「CI 必跑」同一张表（含上面每条命令）。
3. **探针（可选）**：故意破坏一条核心链断言，确认 CI/nightly 真会红。
4. 删除空指针分支 `origin/mainline/1-core-baseline`（== `main`，违反对长期空分支的纪律）。

### 7.2 验收

- [ ] 文档中的门禁命令与 workflow **逐条对得上**（无「两套真相」）
- [ ] 故意破坏核心链 → CI/nightly 红（探针记录进 PR 描述）

---

## 8. M5 — 可读性收尾

1. 根目录作废文档（Python 时代 `HANDOVER.md` / `REFACTOR_PLAN.md` / `REFACTOR_SUMMARY.md` 等）：删或迁 `docs/legacy/` 只留指针。
2. `PROGRESS.md` 等保持归档声明，避免被当成现行进度。
3. **设计文档收束**：`docs/design/` 8 项并存（v2 / v3 / polish / redo / wiring）→ 收敛为
   **一份现行规格 + 历史存档**，避免「实现已跑偏、文档还在拖决策」。

   **3.1 ⚑ 旧 JS 预览隔离（rc.2 内提前执行，不等 M5）**

   对象：`docs/design/ui-preview-v2.html`、`docs/design/assets/preview-main.png`、
   `preview-modal.png`、`preview-actpanel.png`。

   性质：**旧原生 JS 前端**的静态设计预览（`--brand: #40C5F1` 那套 v2 token 的 HTML mock）。
   原生 JS 前端已于 2026-09-11 整体删除，现行产品面是 Flutter `/app/`——
   两者不是同一个产品，摆在同一层就会误导（维护者与本地 agent 都会误判）。

   **处理（二选一，本计划取 A）**：

   - **A（默认，保可逆）**：迁到 `docs/design/legacy/`，并新建
     `docs/design/legacy/README.md`，头注明确写：**「旧 JS 前端预览，勿当现网」**——
     再加一行「现行产品面 = Flutter `/app/`，规格看 `docs/design/` 现行那份」。
     顺带把文件名带上前缀以便一眼可辨：`ui-preview-v2-oldjs.html`、
     `preview-{main,modal,actpanel}-oldjs.png`。
   - **B（更彻底）**：直接删。删除前确保有 commit 记录可回溯（不额外开归档分支）。

   **验收**：

   - [ ] `find docs/design -maxdepth 1 -name 'ui-preview*' -o -maxdepth 1 -name 'preview-*'` 无输出
   - [ ] `docs/design/legacy/README.md` 存在，且含「勿当现网」字样
   - [ ] 全仓 `rg -n "ui-preview-v2|preview-actpanel"` 无引用（除 legacy README 自身）
4. **日志与错误口径：沿用，不新增**。既有契约已经够用：
   `ErrorKind::code()`（`<stage>_<suffix>`）+ WS `error` 帧 `{code,stage,message,hint,epoch,fatal}` + 日志 `code=`。
   - 写进 README 的 `RUST_LOG` 示例（如 `live2d_ai=info,tower=warn`）；
   - supervisor「节点完成」类 info 降 debug，错误保留 `code` + 一句可行动处置；
   - **不引入** `turn_id` 这类跨层新字段（与 §1「宁删勿加」冲突）；确需时**必须替换**现有字段。

---

## 9. rc.2 发布清单（DoD）

### 9.1 版本与产物

- [ ] 根 `Cargo.toml` `version = "0.1.0-rc.2"`；`env!("CARGO_PKG_VERSION")` 经 capabilities 正确暴露
- [ ] `shell/flutter/pubspec.yaml` 版本关系记明（对齐或显式解耦）
- [ ] `docs/releases/v0.1.0-rc.2.md`：写清**删除了什么**、主路径如何更短、点火记录、门禁数字
- [ ] 旧 JS 预览已隔离到 `docs/design/legacy/` 并标注「勿当现网」（§8.3.1）
- [ ] `CHANGELOG.md` 一条

### 9.2 门禁（全部本地实测，数字进 release note）

- [ ] `cargo test --workspace --all-targets` / `--doc`
- [ ] `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo run -p xtask -- rust-ratio`（≥95%）
- [ ] `python3 scripts/verify_core_chain.py`
- [ ] `cd shell/flutter && flutter analyze && flutter test`
- [ ] M0 §3.3 点火闭环（Windows 浏览器肉眼）+ **模型库激活即换皮**（§5.2）

### 9.3 已裁决（2026-09-12 审阅修订）

1. **发布顺序**：**先本地提交 + tag**；等 **Windows 点火通过**（`/app/` 200 + 对话 + 出声 + 口型 + 激活即换皮）
   之后再推 `origin/main` + tag。**不许先推后发现 `/app` 还是 503**。
2. **rc.2 范围**：**瘦版**（见 §0 范围冻结）= M0 + M1 + §5.2 A（A1→A2）+ §5.4（`.env` + `PUT /api/v1/env`）+ §8.3.1。
   **M3 整包不纳入**；§5.3 里**文案 / 假广告 / 旧预览类**进 rc.2，
   **God Object 大拆（`main.dart` ≤700）、egui feature-gate、`surface.rs` 大拆**归 rc.3。
3. **`.env` 密钥源（§5.4）** —— ✅ **已裁决（2026-09-13）**：**进 rc.2**；写入端点冻 **`PUT /api/v1/env`**
   （不并入 `PATCH /api/v1/settings`）。理由：消灭「模型名能改、key 改不了 → 401」，与 M0 点火是同一件事的两半。

### 9.4 审阅修订记录（2026-09-12）

| # | 原稿问题 | 修订 |
|---|---|---|
| 1 | DoD #3「M4 起 CI=本地一套」与 §9.3 范围打架 | DoD #3 改为**只要求本地门禁全绿**；CI 对齐明确归 rc.3 / M4 |
| 2 | 模型热换契约二选一 | **冻结**：后端只回可 GET 的 `model_url`、前端 `sendSync`、以 `stage-ack` 为准、`requires_restart` 热切可行时改 `false`；WS 推送留 rc.3（§5.2 A1.4） |
| 3 | 模型根「二选一」 | **定死唯一真源 = 仓库 `assets/models/`**；import 写这里；XDG 降兼容读或删（§5.2 A1.1） |
| 4 | M0 混入 Win 侧 UNC | **写死「服务进程始终在 WSL 起」**；`LIVE2D_AI_FLUTTER_WEB_DIR` 用 WSL 路径；Win 只开浏览器（§3.1） |
| 5 | rc.2 塞太满 | **范围冻结为瘦版**；`main.dart` 拆分 / 大文件止血 / egui gate 标 rc.3 软目标，不挡 tag（§0、§9.3） |
| 6 | rust-ratio 与删除量的关系 | 已量化（≈2.16 万行余量）；**M3 若大砍测试，合并前先跑 `rust-ratio`**（§6.2） |
| 7 | **密钥源只在进程环境**（用户口径 2026-09-12） | 新增 **§5.4**：`.env` = 唯一密钥真源；Rust 读 `.env` 快照；前端 **`PUT /api/v1/env`**（mutating + 原子写 + `0600` + **永不回显**）+ 热重载；`ignite.sh` 相反注释要改。**2026-09-13：落 rc.2** |

---

## 10. 步骤 2 占位（仅记录，不执行）

- 方向：上层能力**只走 Mod** 堆砌
- 启动条件：M1–M3 完成，且 capabilities / 文档 / 主路径无半拆除矛盾
- 形式：届时再开短命 `mod/<name>`；**不要**现在建空的 `mainline/2-*` 长期分支

---

## 11. 移交检查清单（本地 agent 启动时）

```text
[ ] 读完本文件 §0–§3 + §9.4（范围与契约已在 2026-09-12 冻结）
[ ] 确认工作区 = WSL2 /home/skystar/Live2D-Ai（不是 win 侧）
[ ] git checkout main && git pull --ff-only
[ ] 确认当前里程碑：**M0 → M1 → §5.2 A1（后端统一模型根）→ A2（前端 sync）→ §5.4（`.env` / `PUT /api/v1/env`）→ §8.3.1**
[ ] 列出本 PR 要删的路径（先列删除清单再改代码）
[ ] 跑约定测试 / verify_core_chain / rust-ratio
[ ] PR 描述写清「删除了什么」「主路径如何更短」
[ ] 删除类 PR 先有归档点
[ ] rc.2 范围之外的东西**不要**顺手做（§0 冻结）
```

---

## 12. 参考路径速查

- 核心 reducer：`crates/live2d-ai-core/`
- LLM/TTS/会话：`crates/live2d-ai-runtime/`
- 壳 / API / supervisor：`crates/live2d-ai-desktop/`（`web_api/flutter_app.rs` = `/app` 托管）
- WASM：`crates/l2d-wasm-demo/`（`web/surface.rs` = 渲染 + 待机 + 动作残件）
- Mod 框架：`crates/live2d-ai-mod-system/` + `live2d-ai-mod-*`
- 前端：`shell/flutter/lib/`（`main.dart` 组合根、`live2d/` 舞台、`settings/sections/` 分区）
- 基线文档：`docs/architecture/core-chain-baseline.md`
- 点火：`scripts/ignite.sh`（默认端口 18080）；门禁脚本：`scripts/verify_core_chain.py`
- CI：`.github/workflows/`

---

*本计划是步骤 1 的唯一移交真源。上游 win 侧 `PLAN-CORE-ENHANCEMENT.md` 作为输入保留，不再单独维护。*
