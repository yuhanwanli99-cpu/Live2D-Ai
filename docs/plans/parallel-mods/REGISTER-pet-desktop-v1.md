# REGISTER — `pet-desktop`（Wave 2 轨 E → Wave 3 轨 D「软闭环」）

> 本文件是交给主 agent 的**待办清单 + 验证步骤**，不是注册动作本身。
> 分支 `mod/w3-pet` @ 基线 `118bd435`（`0.2.0-rc.3` + Wave 3 基座）。
> 范围真源：[`PARALLEL-WAVE3-2026-09-14.md`](PARALLEL-WAVE3-2026-09-14.md) §3 轨 D
>（Wave 2 起点：[`PARALLEL-WAVE2-2026-09-14.md`](PARALLEL-WAVE2-2026-09-14.md) §3E）。
> 设计与契约：[`../../architecture/pet-desktop-mod-v0.md`](../../architecture/pet-desktop-mod-v0.md)。

## 0. 一句话

**FACTORIES 一行都不用改**——`pet-desktop` 早已在 `AVAILABLE_MOD_FACTORIES` 里，
**缺省停用不变**（`cli_entry::default_mods_manifest` 仍只启用 `external-input`）。
Wave 3 本轨只做一件事：把这个**已经可读**的状态面接到 Flutter `/app/` 上，
让「启用 → 展开 → 看到运行态 → 改配置 → 字段跟着变」成为**可跑的证据**。

## 1. Wave 3 增量（2026-09-14，软闭环 = 产品标签「仅状态面」）

### 1.1 选定：软闭环，**不设窗口**

硬开桌宠窗口要唤醒**休眠原生壳**（`docs/architecture/core-chain-baseline.md` §3.6
台账 + `AGENTS.md`「原生第二壳的归属」），Wave 3 裁决不走那条路。
所以闭环终点是**状态面**：`window.opened` 依旧恒 `false`
（`reason = native_shell_dormant`），界面上把它如实写成
**「窗口未开（原生壳休眠），此面仅状态」**。

### 1.2 改了什么

| 层 | 文件 | 改动 |
| --- | --- | --- |
| Rust | `crates/live2d-ai-mod-pet-desktop/src/lib.rs` | **契约一字未改**；头注加 Wave 3「软闭环」一节 + 消费方指引 |
| Rust | `crates/live2d-ai-mod-pet-desktop/tests/pet_desktop_state.rs` | 新增 2 条守卫：`state_json_key_set_is_pinned`（顶层/`window` 字段集是前端渲染契约）、`window_reason_string_is_stable_and_ascii`（前后端共享的字面量） |
| Flutter | `shell/flutter/lib/api/mods_api.dart` | 新增 `ModStateResult` + `ModsApi.state(id)` → `GET /api/v1/mods/{id}/state`（`200` / `404 not_found` / `503 state_unavailable` 三分） |
| Flutter | `shell/flutter/lib/settings/sections/dev_tools_section.dart` | `ModsSection` 新增 `onLoadState`；展开卡片新增「运行态（只读）」块（五个字段 + 刷新 + 两种失败码分流 + **保存成功后自动重取**）；纯函数 `orderedModStateKeys` / `modStateLabel` / `formatModStateValue` / `formatWindowState` |
| Flutter 测试 | `shell/flutter/test/pet_desktop_state_test.dart`（新建） | **9 条**：端点/解析、503/404 分流、休眠文案、字段顺序、展开显示、**保存→重取→字段跟着变**、503 上屏 |
| Flutter 测试 | `shell/flutter/test/mods_section_test.dart` | 既有展开类用例注入 `_stubState`（确定性；真机路径由新文件覆盖） |
| 文档 | `docs/architecture/pet-desktop-mod-v0.md` | §1/§4/§5 加 Wave 3 软闭环与 Flutter 消费面；§7 把「Flutter 未消费」改成已闭环 |
| 文档 | 本文件 | — |

### 1.3 闭环证据（可跑）

```bash
export CARGO_TARGET_DIR=/home/skystar/Live2D-Ai-integrate/target
cargo test -p live2d-ai-mod-pet-desktop        # 17 passed（Wave 2 的 15 + 新增 2）
export PATH="$HOME/flutter/bin:$PATH"
cd shell/flutter && flutter test               # 全绿，含 pet_desktop_state_test.dart 的 9 条
```

**「配置热更新可测」的闭环断言**（不需要活服务，注入 fake）：
`pet_desktop_state_test.dart` 的「保存配置 → 重取 state → 字段跟着变」用例——
第一次展开读到 `always_on_top=true/click_through=false/opacity=0.95`，
保存成功后 fake 第二次返回新值，界面断言变成 `关 / 开 / 0.3`，
而 `window` 依旧「窗口未开（原生壳休眠），此面仅状态」。

### 1.4 两条实现决定（主 agent 需要知道）

1. **`shell/flutter/lib/app/shell_settings.dart` 刻意未改**。宿主接线（把
   `ModsApi.state` 传进 `ModsSection`）落在那个文件，而它不属本轨所有权
   （Wave 3 文件边界；B 轨同样要动它做壁纸列表操作）。等价措施：
   `_ModConfigTileState._loader` 在 `onLoadState == null` 时**自己造一个同源
   `ModsApi()`**（base 解析与 `main.dart` 里那个完全一致：`API_BASE` →
   `Uri.base.origin`），所以真机展开卡片就能看到运行态，**零额外接线**；
   测试注入 fake 则完全隔离网络。若主 agent 收束时愿意显式接线，
   只需在 `shell_settings.dart` 的 `ModsSection(...)` 里加一行
   `onLoadState: (String id) => _modsApi.state(id)`（可选，不是缺口）。
2. **状态面在「有 spec 的 Mod」展开卡片里**（`_ModConfigTile`）。没有 `settings_spec`
   的旧 Mod 仍是「一行 + 开关」，不做展开；`pet-desktop` 有 spec（Wave 2 起静态），
   所以它一定进这条路。

## 2. Wave 2 已做完的（worker 侧，集成方不必重做）

| 项 | 位置 | 说明 |
| --- | --- | --- |
| 静态 `settings_spec()` | `crates/live2d-ai-mod-pet-desktop/src/lib.rs` | `pet_desktop_settings_spec()`，与 `start` 注册**同一份**；单测钉住不分叉 |
| `state_json()` | 同文件 | `always_on_top` / `click_through` / `opacity` / `voice_active` / `window`（恒 `opened:false, reason:"native_shell_dormant"`） |
| 配置读取 + 钳位 | 同文件纯函数 | config JSON 优先；缺省 `true`/`false`/`0.95`；`opacity` 钳 `0.1..=1.0`，非有限值回落 |
| 既有语义回归 | 同文件 | `on_event`（`VoiceStarted`/`VoiceEnded` 翻转 + 其它主题无副作用）/ `shutdown` 复位 |
| 范围声明 | crate 头注 | 「窗口未开 + 为什么 + 与 egui 原生壳 / `--web` 主路径的边界」，指向 `core-chain-baseline.md` §3.6 |
| 回归测试 | `crates/live2d-ai-mod-pet-desktop/tests/pet_desktop_state.rs` | Wave 2 的 **15 条**（Wave 3 加 2 条 → **17**） |
| 架构文档 | `docs/architecture/pet-desktop-mod-v0.md` | 范围 / 配置契约 / 状态面契约 / 边界 / curl 闭环 / 非目标 |

**未碰**（红线）：`crates/live2d-ai-desktop/src/main.rs`（FACTORIES / `mod_count_*` /
id 断言）、`cli_entry::default_mods_manifest`、根 `Cargo.toml` 的 `version`、
`pubspec.yaml`、`AGENTS.md`、`docs/README.md`、`docs/releases/**`、基座五个文件
（`topics.rs` / `factory.rs` / `mod_registry.rs` / `mods_routes.rs` / `supervisor.rs`）、
B 轨的 Flutter 文件（`display_prefs.dart` / `wallpaper_api.dart` / `shell_wallpaper.dart` /
`shell_prefs.dart` / `appearance_section.dart` / `shell_settings.dart`）、
`lib/main.dart`（无新 `part`/import）、其它轨的 crate/文档。
**未引入** `winit`/`egui`/`wgpu` 或第二套渲染引擎。

## 3. 集成 PR 待办（**无 FACTORIES 项**）

- [ ] **不改** `AVAILABLE_MOD_FACTORIES`（`pet-desktop` 已在表内）；
- [ ] **不改** `mod_count_*` / `mod_factory_ids_match_expected`；
- [ ] **不改** `cli_entry::default_mods_manifest`（pet-desktop 仍缺省停用）；
- [ ] `docs/architecture/mod-product-chain.md` §5 表的 `pet-desktop` 行（**主 agent 独占**）：
      更新为「配置/事件态可测面 + Flutter「Mod 管理」可展开看运行态；**窗口未开**（休眠原生壳）」，
      并链 `docs/architecture/pet-desktop-mod-v0.md`；
- [ ] `AGENTS.md` 的 Mod 段（**主 agent 独占**）：把 `pet-desktop` 从「骨架」更新为
      「配置/事件态可测面（窗口未开，Flutter 可读运行态，见 `pet-desktop-mod-v0.md`）」；
      **不要**动版本号与 Mod 计数（本轨不新增 Mod）；
- [ ] `docs/README.md`（**主 agent 独占**）架构索引可选加一行
      `[桌宠窗口 Mod v0](architecture/pet-desktop-mod-v0.md)`（不加也不算缺口）；
- [ ] 与 B 轨（壁纸）**无文件冲突**：本轨没碰 `shell_settings.dart` / `shell_prefs.dart` /
      `appearance_section.dart`（理由见 §1.4）；合并时不需要手工解冲突。

## 4. 验证步骤（集成后跑，全绿才算接上）

### 4.1 单元/集成（不需要活服务）

```bash
export CARGO_TARGET_DIR=/home/skystar/Live2D-Ai-integrate/target
cargo test -p live2d-ai-mod-pet-desktop          # 期望 17 passed / 0 failed
cargo fmt --all -- --check                        # clean
cargo clippy -p live2d-ai-mod-pet-desktop --all-targets -- -D warnings   # 0 warning
export PATH="$HOME/flutter/bin:$PATH"
cd shell/flutter && flutter analyze && flutter test   # analyze 无问题 + 全绿
```

### 4.2 全量门禁（与 AGENTS「一张表」一致，主 agent 收束时跑）

```bash
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio      # ≥95%
cd shell/flutter && flutter analyze && flutter test
```

> **共享 target 的坑（Wave 2 本轨实测踩到）**：`CARGO_TARGET_DIR` 被所有 worktree 共用，
> 而 cargo 的 dep-info 里源文件路径是**相对**的；当另一个 worktree 在「你改完源码
> 之后」编译过同名 crate，它的 rlib 与本 worktree 的 unit hash 相同 → cargo 会误判
> 「源码比产物旧 = fresh」，把**别人那份旧 rlib** 链接进你的测试，报出一堆
> 「`state_json` 不存在 / `DESCRIPTOR` 是 private」的假错误。
> 处置：`cargo clean -p <crate>` 后再跑。

### 4.3 真服务一步（照 `pet-desktop-mod-v0.md` §5 抄）

`./scripts/ignite.sh`（默认 18080）后：

```bash
BASE=http://127.0.0.1:18080
curl -s -X POST "$BASE/api/v1/mods/pet-desktop/enable" \
  -H "Content-Type: application/json" -H "Origin: $BASE" \
  -d '{"config":{"always_on_top":true,"click_through":false,"opacity":0.95}}'
curl -s "$BASE/api/v1/mods/pet-desktop/state"
curl -s -X POST "$BASE/api/v1/mods/pet-desktop/config" \
  -H "Content-Type: application/json" -H "Origin: $BASE" \
  -d '{"config":{"always_on_top":false,"click_through":true,"opacity":0.3}}'
curl -s "$BASE/api/v1/mods/pet-desktop/state"
```

判定：第二次 `/state` 的 `always_on_top=false` / `click_through=true` /
`opacity=0.3`，且 `window` 仍为
`{"opened":false,"reason":"native_shell_dormant"}`。
**界面版**：`/app/` → 设置 → Mod → 展开「桌宠窗口」→ 运行态块显示五个字段；
改「总在最前」→ 保存 → 运行态字段跟着变，窗口行始终是
「窗口未开（原生壳休眠），此面仅状态」。

> 注意：curl 是 mutating 路由，必须带 loopback `Origin` + `application/json`
> （既有安全口径）；`GET …/state` 只读，不校验 Origin。
> **`POST …/config` 是整份替换**（不是深合并）：界面保存表单要发整份配置——
> `_buildConfig()` 已从服务端已有 config 出发，只覆盖 spec 声明的字段。

## 5. 需要主 agent 注意的判断（已钉死，勿再升级）

1. **不真开窗口**：`window.opened` 恒 `false` 是裁决，不是未完成。
   唤醒原生壳的先决条件是「谁来维护第二个 UI 壳」的论证
   （`core-chain-baseline.md` §3.6 / `AGENTS.md` 休眠台账）。
2. **运行态的「不启用」表现是 503 不是 404**：界面按码分流
   （「运行态暂时读不到」vs「不在注册表」），不要合并成一句话。
3. **`shell_settings.dart` 的显式接线是可选项**（§1.4），默认自建同源
   `ModsApi()` 已让真机可用；若收束时手工加那一行，记得同步
   `dev_tools_section.dart` 的 `_loader` 头注，避免文档与实现不一致。

## 6. 验收证据（本轨实测，见汇报）

```text
cargo fmt --all -- --check                                             # clean
cargo clippy -p live2d-ai-mod-pet-desktop --all-targets -- -D warnings  # 0 warning
cargo test -p live2d-ai-mod-pet-desktop                                # 17 passed; 0 failed
cd shell/flutter && flutter analyze                                    # No issues found!
cd shell/flutter && flutter test                                       # 全绿（数字见汇报）
```
