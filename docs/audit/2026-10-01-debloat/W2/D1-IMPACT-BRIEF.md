# D1 影响简报 · 休眠资产裁决前必读（2026-10-01）

> 立档：**2026-10-01** · 状态：**活** · 作者：leader（编排者）
> 用途：回答维护者两问 ——「**各 Mod 现在是什么状态**」与「**为什么会深入主链路**」。
> 证据来源：`crates/live2d-ai-desktop` 逐行 grep + `xtask code-stats` 实测 + task-10（只读清点）回报。
> 口径：行数 = `code-stats` 口径（物理行；prod 不含 `#[cfg(test)]` 块）。

## 1. 十六个 crate 的现状（workspace members 实测）

| # | crate | 性质 | 在 `AVAILABLE_MOD_FACTORIES`（5 个）里吗 | D1 处置 |
|---|---|---|---|---|
| 1 | `l2d` | 主链·渲染核心 | — | 不动 |
| 2 | `live2d-ai-core` | 主链·状态机（`action/`+`performance/` **休眠保留为契约**） | — | 不动（台账已记） |
| 3 | `live2d-ai-desktop` | 主链（`web_api/`+`supervisor/`）**＋ 休眠岛**（见 §2） | — | 只切休眠岛 |
| 4 | `live2d-ai-runtime` | 主链·LLM/TTS/会话/密钥 | — | 不动 |
| 5 | `l2d-wasm-demo` | 主链·渲染面（`IdleState` 在此） | — | 不动 |
| 6 | `live2d-ai-mod-system` | Mod 框架契约 | — | 不动 |
| 7 | `live2d-ai-mod-external-input` | **在册·缺省启用** | ✅ | 不动 |
| 8 | `live2d-ai-mod-persona` | **在册·缺省停用** | ✅ | 不动（D2 拆它的 `lib.rs` 1004 行） |
| 9 | `live2d-ai-mod-voice-input` | **在册·缺省停用**（ASR 后端未接） | ✅ | 不动 |
| 10 | `live2d-ai-mod-memory` | **在册·缺省停用** | ✅ | 不动 |
| 11 | `live2d-ai-mod-director` | **在册·缺省停用·零投递**（最小骨架） | ✅ | 不动 |
| 12 | `live2d-ai-mod-template` | 在册但**不注册**（复制起点 + 活文档） | ❌（刻意） | 不动 |
| 13 | `live2d-ai-mod-local-llm` | **DEPRECATED**（0.2.0-rc.1 废除启动） | ❌ | **删** |
| 14 | `live2d-ai-mod-wallpaper` | **ARCHIVED**（产品级加强波次封存） | ❌ | **删** |
| 15 | `live2d-ai-mod-pet-desktop` | **ARCHIVED**（同上） | ❌ | **删** |
| 16 | `xtask` | 工程工具（W0b 新增 `code-stats`） | — | 不动 |

**Mod 计数器实测**：`crates/live2d-ai-desktop/src/main.rs:473` 断言
`AVAILABLE_MOD_FACTORIES.len() == 5`，表内 = external-input / persona / voice-input / memory / director。
⇒ 13/14/15 三个 crate **早已不在表内**，删它们**不改变 5**，红线 6 安全。

## 2. `live2d-ai-desktop` 内部的「休眠岛」与三层耦合

### T1 · 叶子（只被岛内引用，删除零主链影响）

| 对象 | 文件 / 行数 | keeper 引用实测 |
|---|---|---|
| `src/app/`（egui 壳：窗口/设置面/交互） | 12 文件 / 3,856 | 仅 `backend/` 与自身 |
| `src/benchmark/` | 5 文件 / 1,900 | 仅 `main.rs` CLI 分支 |
| `src/model_smoke/` | 3 文件 / 688 | 仅 `main.rs` 与 `backend/` |
| `src/tray/` | 2 文件 / 539 | 仅 `backend/` 与 `app_event::send_app_event` |
| `src/repl.rs` | 1 文件 / 173 | 仅 `backend/chat.rs` |
| `src/adapter/` | 566 | **仅** `app/`(2) 与 `benchmark/`(3) |
| `src/platform/window.rs` | 129 | 仅 `app/`（`platform/mod.rs:31` 有 `pub use`） |
| **小计** | **31 文件 / ≈7,851 行** | |

### T2 · 桥（岛内枢纽，删掉它才让 T1 干净）

| 对象 | 行数 | 说明 |
|---|---|---|
| `src/backend/` | 849 | `chat.rs:15-21` 引 `app::{ChatBridge,ShellApp,run_shell}`、`tray::spawn_pet_tray`、`repl::{ReplCommand,parse_line}`、`model_smoke::ModelSmokeError`、`user_event::*`、`app_event::send_app_event` |

### T3 · **主链触点**（这就是「为什么会深入主链路」）

休眠岛的 `PetUserEvent` 被**主链根事件类型**引用，位置逐条实测：

| 主链文件（**不是**休眠岛） | 行 | 现状 | 移出岛时必须做的事 |
|---|---|---|---|
| `src/app_event.rs` | **:21** | `use crate::user_event::PetUserEvent;` | 删该 use |
| `src/app_event.rs` | **:27** | `AppEvent::Tray(PetUserEvent)` 变体 | **删变体**（或保留空壳） |
| `src/app_event.rs` | **:251** | `pub fn send_app_event(&EventLoopProxy<AppEvent>, ...)`（`winit` 类型） | 删函数（同时摘掉 `winit` 依赖的最后一处） |
| `src/web_api/ws/events.rs` | **:203** | `\| AppEvent::Tray(_) => {…}` **WS 投影匹配臂** | 删该臂 |
| `src/supervisor.rs` | **:859** | 测试 `non_conversation_events_yield_none` 用 `AppEvent::Tray(PetUserEvent::TrayExit)` 当「非 Conversation 事件」样本 | **换样本**（改用 `RootAudit` / `ShutdownReady`，语义等价） |
| `src/main.rs` | :41-51 · :98 · :102 · :243-259 | `mod app/benchmark/model_smoke` 声明、`mod repl`、`mod tray`、`benchmark::*` 分发 | 删声明与 CLI 分支 |
| `src/platform/mod.rs` | :31 | `pub use window::{WindowBackendKind, bottom_right_target}` | 收窄可见性（否则 clippy `dead_code` 红） |
| `src/platform/capabilities.rs` | 539 行 | **主链 Info 分支仍在用**（`main.rs:302-319`） | **保留**，只裁尾巴 |

**判据要害**：`AppEvent::Tray(_)` 在 WS 投影里本来就投影为 **`None`（不发帧）**
（`web_api/ws/events.rs:203` 所在的分组即「不投影」臂）。
⇒ **删这个臂不改变任何 WS 帧**，§7 红线 1（WS 帧只增不改）**不破**；
但它确实动了**主链的投影 match 与事件枚举**——这正是「深入主链路」的全部含义：
**不是主链行为被改，而是主链的事件类型/投影与休眠壳被同一个 `enum` 绑在一起。**

### 连带集（只删点名 5 个对象一定编译不过）

`backend/`（849）· `adapter/`（566）· `platform/window.rs`（129）· `app_event::send_app_event` ·
`user_event.rs`（191）——由引用图实测得出，非推断。

## 3. 量化影响（三个方案）

| | A · 只删 3 个已归档 Mod | B · 再移出原生壳（含连带） | C · feature-gate 壳 |
|---|---|---|---|
| Rust 生产行数 | −2,020 | −≈8,900 | −0（仅默认不编译） |
| 文件 | −8 | −≈31~37 | 0 |
| 测试条数（当前 1,474） | **−64** → 1,410 | **−78**（点名）/ **≈−142**（含连带）→ ≈1,332 | 0（保活门禁） |
| 主链文件改动 | **0** | `app_event.rs` · `web_api/ws/events.rs` · `supervisor.rs` 测试 · `main.rs` · `platform/mod.rs` | `Cargo.toml` + `#[cfg]` 散布 |
| `desktop` 依赖 | 34（不变） | **34 → 25**（真删 9 条：`egui` `egui-winit` `egui-wgpu` `raw-window-handle` `pollster` `ksni` `wgpu` `winit` `url`），再砍 3 条外围 → **22**（D4 目标） | **34（不降）**：`code-stats` 的 dep 计数把 `optional = true` 也算 |
| 红线 8（测试不降） | 需放行 −64 | 需放行 −142 | 不冲突 |
| 红线 1（WS 帧） | 不碰 | 不破（`Tray` 本来不投影） | 不碰 |

**顺带两条零成本收益（与方案无关都可做）**：
1. `url` 依赖**今天已经是死依赖**（全 crate `url::` / `use url` = 0 命中）→ D4 直接删。
2. `code_stats` / `rust-ratio` 按**文件系统**统计 ⇒ `[workspace] exclude` **不会**让任何 D0 度量下降；
   想让「休眠行数归零」为真，只有**物理移出仓库**（删或换仓）。

## 4. 红线 8 例外口径（维护者已授权 leader 定义）

**准予的例外范围（窄口径，必须同时满足）**：
1. 仅适用于**随休眠资产一起退出构建**的测试——即该测试所测对象已被物理移出；
2. 必须**逐条列全名 + 文件 + 退出原因**，写进台账（`docs/architecture/ARCHIVED-mods.md` /
   新增 `ARCHIVED-native-shell.md`），并写**恢复条件**（回到 tag `checkpoint/pre-d1-dormant` 的步骤）；
3. 必须有**机械证据**证明「退出的是被移出资产的测试、主链测试一条没少」：
   `cargo test --workspace --all-targets -- --list` 前后做集合差，
   `退出集合 ⊆ 被移出目录`，且 `主链集合（web_api/supervisor/runtime/core/l2d/l2d-wasm-demo/mod-system/5 在册 Mod）差 = 0`；
4. 若某条测试断言的是**主链行为**（哪怕住在岛内文件），必须**搬移或指出等价断言**，否则**不得退出**；
5. 主链测试条数**只增不减**；总条数下降必须在提交信息与发布说明里**明写数字与原因**；
6. 不得为凑数保留「永不执行」的空壳测试（那是假绿灯，D6 要清的正是这个）。

**用行话说**：红线 8 的立法意图是「**别偷偷丢掉对活行为的覆盖**」，不是「行数/条数神圣不可动」。
被移出的 `egui` 壳/终端壳/benchmark 是 AGENTS 明文的**非主线、不验收**资产，其测试随资产退出属**显式冻结**，
不是覆盖损失——前提是 §4.1–4.6 全做到。

## 4bis. 维护者裁决与红线修订（2026-10-01，已定）

维护者口径：**「你可以修改红线；本次是大修改，清理工程债务、降低复杂度」**。
据此由 leader 执行如下**窄口径**红线修订（不是放开全部红线）：

### 修订 1 · §7 红线 8（测试条数不降）→ 改为「**主链测试只增不减 + 休眠资产测试显式冻结**」

- **主链测试条数只增不减**：`web_api/` · `supervisor/` · `live2d-ai-runtime` · `live2d-ai-core` ·
  `l2d` · `l2d-wasm-demo` · `live2d-ai-mod-system` · 5 个在册 Mod —— 这些的测试集合差 **必须 = 0（只增）**。
- **休眠资产测试**允许随资产**物理移出**而退出构建，但必须全部满足 §4 的 1–6 条
  （逐条全名 + 台账 + 恢复 ref + 机械集合差证据 + 主链行为断言必须搬移 + 不许留空壳测试）。
- 总条数下降必须在 commit body 与 release note **明写数字与原因**。

### 修订 2 · §7 红线 1（WS 帧只增不改）→ 范围澄清

- **对外 WS 帧契约不变**：帧类型名与字段名（`text_delta` / `reasoning_delta` / `error` /
  `action_cue` / `preset_id` / `speak` / `stage-bg` …）一律不动，**不允许**新增/删除/改名任何帧字段。
- **内部事件枚举 `AppEvent`** 允许删除「**从来不投影成帧**」的休眠壳专用变体
  （`AppEvent::Tray(PetUserEvent)` 在 `web_api/ws/events.rs:203` 本就落在「不投影」臂）。
  该删除**不产生任何帧变化**，且必须在 commit body 逐条列出。

### 其它红线**不动**
`clean_for_tts` 不得旁路 · 上屏 == 送 TTS · 错误码两侧同源（前端不得从文案猜错误类型）·
离线优先（`--no-web-resources-cdn` + 自托管字体 + 零外部源）· 密钥 `.env` 真源 / 永不回值 ·
表演资产 A1–A8 语义与 `IdleState` 保留 · `mod_count` 断言不得删。

### 裁决结果
- **D1 走两段式**：`W2-A` 先（删 3 个已归档 Mod crate，**零主链触点**）→ `W2-B` 后（移出原生壳 + 连带集，
  含 `app_event.rs` / `web_api/ws/events.rs` / `supervisor.rs` 测试 / `main.rs` / `platform/mod.rs` 五处主链触点）。
  两个 commit，可各自回滚与归因。
- **W2-B 的主链触点获批**（按修订 2 的范围）。

## 5. leader 建议（已被上述裁决取代，保留作记录）

**两段式，两个 commit**（保持「重构与行为分开」「可独立回滚」）：

1. **W2-A（先做，零主链风险）**：删 3 个已归档 Mod crate（−3,039 行 / −64 测试 / 0 主链改动），
   更新 `ARCHIVED-mods.md` 台账 + `AGENTS.md` 状态句 + `scripts/ignition-precheck.sh:156` 文案。
2. **W2-B（再做，触及主链事件枚举）**：移出原生壳岛 + 连带集（T3 五处主链触点逐条改），
   新增 `ARCHIVED-native-shell.md` 台账；同一 commit 内**不碰** `web_api` 的业务逻辑，
   只删 `ws/events.rs` 的一个「不投影」臂。

理由：A 是纯减法、可先行落地且立刻改善 `code-stats` 与「休眠行数」；
B 触及主链事件枚举，单独成 commit 才能在出问题时**精确归因与回滚**。

## 6. 未核实栏（不许空）

1. 「岛整体移出后能编译」**未编译验证**（task-10 禁改；本简报的连带集是引用图推断，需 task-11 实做后 `cargo check` 才算数）。
2. `rust-ratio` 移出后 ≈96.76%（算出来的，未实测）；`--check --strict-plan` 现为红（`>500` 58>15 / Dart 7>2 / `>1000` 4>0 / deps 34>22），收紧时机挂在 D2/D4。
3. `cli/tests.rs` 具体哪 10–11 条随分支退出、`platform/` 9 条里哪几条退出：按测试名分类，需执行阶段跑一遍确认。
4. 删除 3 Mod crate 后 `cli_entry.rs:740/753` 两条「缺省 manifest 里没有 local-llm/wallpaper」的字符串断言**仍成立且必须保留**——已读码确认，未编译验证。
5. Flutter 侧 4 处残留引用（`test/pet_desktop_state_test.dart` · `test/admin_api_test.dart` · `test/mods_section_test.dart` · `lib/settings/sections/dev_tools_section.dart`）**删除后仍会全绿**（硬编码夹具，属假绿灯）：台账须标注，**不得**当作被删 Rust 测试的覆盖证据。
