# REGISTER — `pet-desktop`（Wave 2 轨 E → 集成 PR 用）

> 本文件是交给主 agent 的**待办清单 + 验证步骤**，不是注册动作本身。
> 分支 `mod/pet-desktop-v1` @ 基线 `429609f2`（`0.2.0-rc.2` + Wave 2 基座）。
> 范围真源：[`PARALLEL-WAVE2-2026-09-14.md`](PARALLEL-WAVE2-2026-09-14.md) §3E。
> 设计与契约：[`../../architecture/pet-desktop-mod-v0.md`](../../architecture/pet-desktop-mod-v0.md)。

## 0. 一句话

**FACTORIES 一行都不用改**——`pet-desktop` 早已在 `AVAILABLE_MOD_FACTORIES` 里
（`main.rs:70`，`mod_count_is_five` 的 5 个之一），本轨只把它的**配置/事件态**
做成可测面（`state_json` + 静态 `settings_spec`），**缺省停用不变**
（`cli_entry::default_mods_manifest` 仍只启用 `external-input`）。

集成方要做的只有两件：**文档/AGENTS 同步** + **跑一遍验证步骤**。

## 1. 本轨已做完的（worker 侧，集成方不必重做）

| 项 | 位置 | 说明 |
| --- | --- | --- |
| 静态 `settings_spec()` | `crates/live2d-ai-mod-pet-desktop/src/lib.rs` | `pet_desktop_settings_spec()`，与 `start` 注册**同一份**；单测钉住不分叉 |
| `state_json()` | 同文件 | `always_on_top` / `click_through` / `opacity` / `voice_active` / `window`（恒 `opened:false, reason:"native_shell_dormant"`） |
| 配置读取 + 钳位 | 同文件纯函数 | config JSON 优先；缺省 `true`/`false`/`0.95`；`opacity` 钳 `0.1..=1.0`，非有限值回落 |
| 既有语义回归 | 同文件 | `on_event`（`VoiceStarted`/`VoiceEnded` 翻转 + 其它主题无副作用）/ `shutdown` 复位 |
| 范围声明 | crate 头注 | 「窗口未开 + 为什么 + 与 egui 原生壳 / `--web` 主路径的边界」，指向 `core-chain-baseline.md` §3.6 |
| 回归测试 | `crates/live2d-ai-mod-pet-desktop/tests/pet_desktop_state.rs` | **15 条**集成测试（含「换 config → `state_json` 立刻变」「`create` 携带 config」） |
| 架构文档 | `docs/architecture/pet-desktop-mod-v0.md` | 范围 / 配置契约 / 状态面契约 / 边界 / curl 闭环 / 非目标 |
| 本文件 | `docs/plans/parallel-mods/REGISTER-pet-desktop-v1.md` | — |

**未碰**（红线）：`main.rs`（FACTORIES / `mod_count_*` / id 断言）、
`cli_entry::default_mods_manifest`、根 `Cargo.toml` 的 `version`、`pubspec.yaml`、
`AGENTS.md`、`docs/releases/**`、基座五个文件
（`topics.rs` / `factory.rs` / `mod_registry.rs` / `mods_routes.rs` / `supervisor.rs`）、
`shell/flutter/**`、其它轨的 crate/文档。
`crates/live2d-ai-mod-pet-desktop/Cargo.toml` 也未改（依赖已够）。

## 2. 集成 PR 待办（**无 FACTORIES 项**）

- [ ] **不改** `AVAILABLE_MOD_FACTORIES`（`pet-desktop` 已在表内）；
- [ ] **不改** `mod_count_is_five` / `mod_factory_ids_match_expected`；
- [ ] **不改** `cli_entry::default_mods_manifest`（pet-desktop 仍缺省停用）；
- [ ] `docs/architecture/mod-product-chain.md` §5 表的 `pet-desktop` 行：
      「桌宠骨架」→ 一句 Wave 2 描述（如「桌宠配置/事件态可测面：静态
      `settings_spec` + `GET …/state`；**窗口未开**（休眠原生壳）」），
      并链 `docs/architecture/pet-desktop-mod-v0.md`；
- [ ] `AGENTS.md` 的 Mod 段：若该段有逐 Mod 一句话清单，把 `pet-desktop` 从
      「骨架」更新为「配置/事件态可测面（窗口未开，见 `pet-desktop-mod-v0.md`）」；
      **不要**动版本号与 Mod 计数（本轨不新增 Mod）；
- [ ] `docs/README.md` 架构索引可选加一行
      `[桌宠窗口 Mod v0](architecture/pet-desktop-mod-v0.md)`（与
      `wallpaper-mod-v0.md` 同款待遇；不加也不算缺口）；
- [ ] 与 C 轨（memory）无冲突：memory 只改 `mod_count_*` 的数字与 id 断言
      （5 → 6），本轨**不参与**那次改动——若两者一起合，`pet-desktop` 那几行
      保持原样即可。

## 3. 验证步骤（集成后跑，全绿才算接上）

### 3.1 单元/集成（不需要活服务）

```bash
export CARGO_TARGET_DIR=/home/skystar/Live2D-Ai-integrate/target
cargo test -p live2d-ai-mod-pet-desktop          # 期望 15 passed / 0 failed
cargo fmt --all -- --check                        # clean
cargo clippy -p live2d-ai-mod-pet-desktop --all-targets -- -D warnings   # 0 warning
```

### 3.2 全量门禁（与 AGENTS「一张表」一致）

```bash
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio      # ≥95%
```

> **共享 target 的坑（本轨实测踩到）**：`CARGO_TARGET_DIR` 被所有 worktree 共用，
> 而 cargo 的 dep-info 里源文件路径是**相对**的；当另一个 worktree 在「你改完源码
> 之后」编译过同名 crate，它的 rlib 与本 worktree 的 unit hash 相同 → cargo 会误判
> 「源码比产物旧 = fresh」，把**别人那份旧 rlib** 链接进你的测试，报出一堆
> 「`state_json` 不存在 / `DESCRIPTOR` 是 private」的假错误。
> 处置：`cargo clean -p <crate>` 后再跑（本轨即如此拿到绿）。
> 判定方法：`strings $CARGO_TARGET_DIR/debug/deps/lib<crate>-<hash>.rlib | grep -o '/home/skystar/Live2D-Ai[a-z0-9-]*' | sort -u`
> 出现别的 worktree 名就是被污染了。

### 3.3 真服务一步（照 `pet-desktop-mod-v0.md` §5 抄）

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
（完整期望 JSON 与 404/503 分支见架构文档 §5。）

> 注意：curl 是 mutating 路由，必须带 loopback `Origin` + `application/json`
> （既有安全口径）；`GET …/state` 只读，不校验 Origin。

## 4. 需要主 agent 注意的两个判断（已在本轨钉死，勿再升级）

1. **不真开窗口**：`window.opened` 恒 `false` 是 Wave 2 §3E 的选项，不是未完成。
   唤醒原生壳的先决条件是「谁来维护第二个 UI 壳」的论证
   （`core-chain-baseline.md` §3.6 / `AGENTS.md` 休眠台账）；
2. **Flutter 不消费本状态面**：`/app/` 里没有桌宠面板，主路径上也没有桌宠窗口
   UI。若用户要求「界面上看到这三个值」，那是**新的一项前端工作**，
   不属于本轨交付。

## 5. 验收证据（本轨实测，见汇报）

```text
cargo test -p live2d-ai-mod-pet-desktop                        # 15 passed; 0 failed
cargo fmt --all -- --check                                     # clean
cargo clippy -p live2d-ai-mod-pet-desktop --all-targets -- -D warnings   # 0 warning
cargo test --workspace --all-targets                           # 全绿（数字见汇报）
```
