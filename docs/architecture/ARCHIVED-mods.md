# 已封存 / 已删除的 Mod（ARCHIVED）

> **这是什么**：项目里**曾经注册、现已明确不做**的 Mod 的封存 + 删除台账。
> **移出 `AVAILABLE_MOD_FACTORIES` = 不在产品路径上**；三者的 crate 已在
> **W2-A（D1 第一段，2026-10-01）**从 workspace 物理删除（`git rm -r`），
> 代码只存在于还原点 tag `checkpoint/pre-d1-dormant`（= `d140604f`）里。
>
> **恢复条件**写在每一节里——没有满足条件之前，任何人不得把这三个工厂加回
> `crates/live2d-ai-desktop/src/main.rs::AVAILABLE_MOD_FACTORIES`，也不得
> 未经裁决把 crate 从 tag 里取回 workspace。
> 数量护栏是 `main.rs::mod_count_is_five`（Wave 3 的 7 → 产品级加强波次的 **5**）：
> 本次删除**不改变 5**（三者早已不在表内）。

---

## 0. 当前注册面（封存后）

`AVAILABLE_MOD_FACTORIES` **恰好 5 个**（顺序即表内顺序）：

| # | id | 缺省 |
| --- | --- | --- |
| 1 | `external-input` | **on**（唯一缺省启用） |
| 2 | `persona` | off |
| 3 | `voice-input` | off |
| 4 | `memory` | off |
| 5 | `director` | off |

`live2d-ai-mod-template` 是模板 crate，**不注册**。

---

## 1. `wallpaper` — 封存

| 项 | 值 |
| --- | --- |
| 封存时间 | 2026-09-14（产品级加强波次，未 bump 版本） |
| crate | `crates/live2d-ai-mod-wallpaper` |
| 原职责 | 壁纸策略 v0：`mode`（off / follow_stage / interval）→ `state_json.prefs_patch` → Flutter `applyWallpaperPatch` → 既有 `DisplayPrefs` / `sendStageBg` |
| 现状 | 移出 `AVAILABLE_MOD_FACTORIES`（7 → 5）；移出 `live2d-ai-desktop` 依赖；Flutter 侧 `wallpaper_api.dart` / `shell_wallpaper.dart` / `applyWallpaperPatch` 一并拆除 |
| 架构文 | [wallpaper-mod-v0.md](wallpaper-mod-v0.md)（顶部有封存横幅，正文是封存前记录） |

### 1.1 为什么封存

1. **用户裁决（本波范围）**：wallpaper Mod 删除并封存，本波不做；**也不实现本地下载库 /
   自由切换**。理由：它是「自动换壁纸」策略，而产品上真正被用户使用的显示能力是
   **手动选舞台/壳背景**——后者属于 `DisplayPrefs`，与 Mod 无关，继续保留。
2. **没有独立的产品价值**：自动策略的落点（`DisplayPrefs` 的 `stageImage` /
   `syncShellStageBg`）本身就是用户可用手点到的能力；Mod 只负责「什么时候替你点」，
   收益（省一次点击）抵不上它带来的常驻轮询与状态面维护成本。

### 1.2 明确**没有**被删掉的东西（不要连坐）

- `DisplayPrefs.stageImage` / `shellImage` / `stagePlaylist` / `syncShellStageBg` /
  `effectiveShellImage`：**用户可手动选图/清图/增删排序**，全部保留；
- 「外观与互动」里的舞台/壳背景 UI、rc.5 的 `ShellBackdrop` 与 stage-bg 协议：保留；
- `l2d-wasm-demo` 的 framebuffer 背景预通道：**一行未动**。

### 1.3 恢复条件（满足全部才可挂回）

1. 先立项论证「自动换壁纸」相对「手动选图」的**独立价值**（用户裁决已否决一次）；
2. 给出常驻轮询/状态面的成本结论，并说明它为什么必须是一个 Mod 而不是主链能力；
3. 同步恢复 `mod_count_is_*` / id 断言与 `mod-product-chain` §5 表格；
4. 恢复 Flutter 落点（`applyWallpaperPatch` 等）时**不得**改动 `DisplayPrefs` 的用户语义。

### 1.4 删除记录（W2-A / D1 第一段，2026-10-01）

| 项 | 值 |
| --- | --- |
| 还原点 tag | `checkpoint/pre-d1-dormant` = `d140604f66ba0f2c47e0d159058ad5deeb92fbc1` |
| 删除方式 | `git rm -r crates/live2d-ai-mod-wallpaper`（3 文件 / 1,400 行） |
| 主链触点 | **零**：`cargo metadata --no-deps` 反查「无任何 workspace 成员依赖它」；`live2d-ai-desktop` 的 `[dependencies]` 里只有注释提到它 |
| 退出构建的测试 | **37 条**（全名见下） |
| 性质 | 休眠资产测试**显式冻结**（红线 8 修订版 §4 的 1–6 条），不是覆盖损失 |

退出测试全名（`src/lib.rs` 内联 18 条 + `src/strategy.rs` 内联 19 条）：

```
strategy::tests::decision_json_kinds_match_contract
strategy::tests::follow_stage_emits_sync_exactly_once
strategy::tests::follow_stage_resyncs_after_reconfigure
strategy::tests::interval_advances_only_when_full_interval_elapsed
strategy::tests::interval_does_not_catch_up_after_long_suspend
strategy::tests::interval_first_tick_shows_first_image
strategy::tests::interval_holds_when_playlist_empty
strategy::tests::interval_overflow_is_saturating_not_wrapping
strategy::tests::interval_secs_defaults_and_clamps
strategy::tests::interval_wraps_around_playlist
strategy::tests::mode_from_config_maps_known_values_and_defaults_off
strategy::tests::mode_switch_between_off_and_follow_stage
strategy::tests::off_never_acts
strategy::tests::playlist_len_from_config_defaults_zero_and_clamps
strategy::tests::reason_distinguishes_off_from_not_due_and_empty
strategy::tests::reconfigure_resets_timing_and_cursor
strategy::tests::set_playlist_len_clamps_and_resets
strategy::tests::strategy_from_config_json_reads_playlist_len
strategy::tests::to_prefs_patch_projects_three_states
tests::descriptor_and_factory_identity
tests::playlist_len_from_config_is_clamped_at_runtime
tests::playlist_len_is_run_value_read_from_config_and_state
tests::reconfigure_applies_new_config
tests::reconfigure_rereads_playlist_len
tests::runtime_follow_stage_logs_and_respects_off
tests::runtime_reads_mode_and_interval_from_config
tests::runtime_tick_logs_decisions_and_advances
tests::set_playlist_len_is_capped
tests::start_registers_settings_only
tests::state_json_advances_the_clock_between_calls
tests::state_json_follow_stage_syncs_once
tests::state_json_interval_with_empty_playlist_reports_reason
tests::state_json_off_mode_never_acts
tests::state_json_shape_matches_contract
tests::state_trajectory_follow_stage_syncs_then_resyncs_after_reconfigure
tests::state_trajectory_interval_advances_index_over_time
tests::static_spec_fields_and_no_enabled
```

恢复步骤（任一即恢复到「封存」态；再往上是产品态）：

```bash
cd /home/skystar/Live2D-Ai-fe
git checkout checkpoint/pre-d1-dormant -- crates/live2d-ai-mod-wallpaper
# 再把 "crates/live2d-ai-mod-wallpaper" 加回根 Cargo.toml 的 members
cargo test -p live2d-ai-mod-wallpaper --all-targets   # 期望 37 条全绿
```

**不得**在恢复后把 `wallpaper::FACTORY` 挂回 `AVAILABLE_MOD_FACTORIES`（§1.3 的四条恢复条件先行）。

---

## 2. `pet-desktop` — 封存

| 项 | 值 |
| --- | --- |
| 封存时间 | 2026-09-14（产品级加强波次，未 bump 版本） |
| crate | `crates/live2d-ai-mod-pet-desktop` |
| 原职责 | 桌宠配置 / 事件态的**可测状态面**（`GET /api/v1/mods/pet-desktop/state`），`window.opened` 恒 `false`、`reason=native_shell_dormant` |
| 现状 | 移出 `AVAILABLE_MOD_FACTORIES`（7 → 5）；移出 desktop 依赖；不再出现在 `GET /api/v1/mods` |
| 架构文 | [pet-desktop-mod-v0.md](pet-desktop-mod-v0.md)（顶部有封存横幅） |

### 2.1 为什么封存

1. **用户裁决**：pet-desktop 是目前最重的一块，**封存、暂时不推**；本波**不做真窗 /
   应用级桌宠**。
2. 它从来没有硬闭环：`window.opened` 恒 `false` 是既定裁决，所谓「软闭环」的终点
   只是一个**状态面**——既然窗口不做，这个状态面本身也没有产品出口；
3. 真窗的前置条件（`core-chain-baseline.md` §3.6 的休眠台账：先论证「谁来维护第二个
   UI 壳」）从未被满足。

### 2.2 明确**没有**被删掉的东西

- `crates/live2d-ai-desktop` 的 egui 原生壳 / 托盘 / `--pet-mode` 代码：按
  `core-chain-baseline.md` §3.6 仍是**休眠保留**（能编译、非主线、不验收）；
- Rust 侧 `live2d-ai-mod-pet-desktop` crate 本身：**已于 W2-A 删除**（见 §2.4），
  只存在于 tag `checkpoint/pre-d1-dormant` 里。

### 2.3 恢复条件（满足全部才可挂回）

1. 先立项论证「谁来维护第二个 UI 壳」（`core-chain-baseline.md` §3.6 的红线）；
2. 明确「硬开窗口」的产品定义（与 Flutter `/app/` 的关系、验收人、验收方法）；
3. 同步恢复数量/id 断言与 `mod-product-chain` §5 表格；
4. 真窗落地必须单独做 wasm/平台验收，不得顺手在状态面上「假装闭环」。

### 2.4 删除记录（W2-A / D1 第一段，2026-10-01）

| 项 | 值 |
| --- | --- |
| 还原点 tag | `checkpoint/pre-d1-dormant` = `d140604f66ba0f2c47e0d159058ad5deeb92fbc1` |
| 删除方式 | `git rm -r crates/live2d-ai-mod-pet-desktop`（3 文件 / 793 行） |
| 主链触点 | **零**（同上：`cargo metadata` 反查无反向依赖） |
| 退出构建的测试 | **17 条**（全部来自集成测试 `tests/pet_desktop_state.rs`；`src/lib.rs` 无内联测试，0 条） |
| 性质 | 休眠资产测试**显式冻结**（红线 8 修订版 §4）；见下方「假绿灯提醒」 |

退出测试全名（`tests/pet_desktop_state.rs`，17 条）：

```
bad_types_fall_back_per_field
build_state_json_matches_runtime_shape
descriptor_is_static_and_api_version_matches_host
factory_create_carries_config_into_state
opacity_is_clamped_and_bad_numbers_fall_back
reconfigure_is_reflected_immediately
shutdown_resets_readiness_and_voice
start_subscribes_voice_topics_only
state_falls_back_to_documented_defaults
state_json_is_a_read_only_snapshot
state_json_key_set_is_pinned
state_reflects_config_values
state_tracks_voice_events_and_ignores_others
static_settings_spec_equals_start_registered_spec
static_settings_spec_shape_is_pinned
window_is_always_dormant
window_reason_string_is_stable_and_ascii
```

⚠ **假绿灯提醒（不得当覆盖证据）**：`shell/flutter/test/pet_desktop_state_test.dart`
（2026-10-05 债轮 D6 更名 `shell/flutter/test/mod_state_surface_test.dart`，断言内容未变）
用**硬编码夹具**断言 `GET /api/v1/mods/pet-desktop/state` 的形状，它不经过 Rust 生产者，
删除本 crate 后**仍然全绿**。它**不是**上述 17 条的等价断言，不许拿来当「行为已被其它断言覆盖」。

恢复步骤同 §1.4（把目录名换成 pet-desktop；`cargo test -p live2d-ai-mod-pet-desktop --all-targets` 期望 17 条全绿）。

---

## 3. 与 `local-llm`（DEPRECATED）的关系

三者是同一口径的不同档位；**三者的 crate 都已在 W2-A 物理删除**（只剩 tag）：

| 状态 | 含义 | 现行例子 |
| --- | --- | --- |
| 注册在册 | 在产品路径上 | external-input / persona / voice-input / memory / director |
| **ARCHIVED（封存）→ 已删除** | 裁决「本波不做」，crate 已从 workspace 删除，**禁止挂回** | wallpaper / pet-desktop |
| **DEPRECATED（已废除启动）→ 已删除** | 能力被新基线取代，crate 已从 workspace 删除，**禁止挂回** | local-llm |

三者都由 `main.rs` 的**同一个**数量/id 断言护栏守住（`mod_count_is_five`）：
任何「顺手挂回去」都会让门禁变红。**本次删除不改变这个 5**——它们早已不在表内。

### 3.1 删除记录（W2-A / D1 第一段，2026-10-01）

| 项 | 值 |
| --- | --- |
| 还原点 tag | `checkpoint/pre-d1-dormant` = `d140604f66ba0f2c47e0d159058ad5deeb92fbc1` |
| 删除方式 | `git rm -r crates/live2d-ai-mod-local-llm`（2 文件 / 846 行） |
| 主链触点 | **零**（`cargo metadata --no-deps` 反查无反向依赖；desktop 依赖面早在 `0.2.0-rc.1` 就移除了） |
| 退出构建的测试 | **10 条**（`src/lib.rs` 内联） |
| 性质 | 休眠资产测试**显式冻结**（红线 8 修订版 §4） |
| 仍生效的护栏 | `web_api/cli_entry.rs` 的两条字符串断言：缺省 manifest 里**没有** `local-llm` / `wallpaper`（删 crate 后依旧成立，**未改动**） |

退出测试全名（`src/lib.rs`，10 条）：

```
tests::auto_start_false_no_spawn
tests::config_defaults
tests::descriptor_is_static
tests::externally_managed_defaults_true
tests::externally_managed_true_does_not_spawn
tests::http_get_fails_on_unreachable_port
tests::invalid_config_falls_back_without_panic
tests::probe_and_configure_on_failure_does_not_send_patch
tests::probe_and_configure_sends_correct_patch_on_ready
tests::start_registers_spec_and_subscription
```

恢复步骤同 §1.4（`cargo test -p live2d-ai-mod-local-llm --all-targets` 期望 10 条全绿）；
另：`0.2.0-rc.1` 起「废除启动」是**裁决**，恢复 crate ≠ 恢复启动，禁止挂回 FACTORIES。

---

## 4. 相关文件

- [Mod 产品链路](mod-product-chain.md) §5（现行注册表）
- [Mod 社区许可与注册边界](mod-community-license.md) §4（许可状态表）
- [核心链基线 §3 休眠台账](core-chain-baseline.md)
- 删除前目录（**已不在工作树**，只在 `checkpoint/pre-d1-dormant` 里）：
  `crates/live2d-ai-mod-wallpaper/`、`crates/live2d-ai-mod-pet-desktop/`、
  `crates/live2d-ai-mod-local-llm/`（三者的 crate 头注曾带 ARCHIVED/DEPRECATED 横幅）
