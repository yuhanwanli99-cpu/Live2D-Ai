# 原生壳岛（ARCHIVED）· 移出构建台账（2026-10-01 W2-B / D1 第二段）

> **这是什么**：`live2d-ai-desktop` 里那份**非主线、不验收**的原生壳（egui 窗口 / 托盘 /
> 桌宠模式 / `--chat` 终端壳 / 窗口-模型冒烟 / benchmark）及其枢纽与连带类型的**移出台账**。
> 维护者裁决（2026-10-01，见 `docs/audit/2026-10-01-debloat/W2/D1-IMPACT-BRIEF.md` §4bis）：
> 走「物理移出」而不是 feature-gate；**对外 WS 帧字段名零改动**。
>
> **还原点 tag**：`checkpoint/pre-d1-dormant` = `d140604f66ba0f2c47e0d159058ad5deeb92fbc1`
> （注意：该 tag 早于 W2-A/W2-B，取回时同时会带回已删除的 3 个归档 Mod crate——
> 只 `git checkout <tag> -- <路径>` 取你真正要的那部分）。
>
> **红线 8（修订版 2026-10-01）口径**：主链**行为**测试只增不减；休眠资产测试随资产
> **物理移出**而退出，但必须逐条列全名 + 台账 + 恢复 ref（本文即那份台账）。
> 本次移出实测：**退出 111 条 / 新增 3 条 = 净 −108**，其中
> **100 条**住在被物理删除的文件里（⊆ 被移出目录），**11 条**住在保留文件里（逐条见 §5）。

## 1. 移出对象（31 文件 / 8,954 行）

| 目录 / 文件 | 文件数 | 行数 | 说明 |
|---|---:|---:|---|
| `src/app/` | 12 | 3,856 | egui 原生壳：窗口/设置面/交互/GPU 协商 |
| `src/tray/` | 2 | 539 | ksni SNI 托盘（菜单/图标/共享状态） |
| `src/repl.rs` | 1 | 173 | `--chat` 终端壳的行解析 |
| `src/benchmark/` | 5 | 1,900 | benchmark-only A/B（C7/C8） |
| `src/model_smoke/` | 3 | 688 | Bai 实时渲染冒烟驱动 |
| `src/adapter/` | 2 | 566 | `ParameterFrame` → Bai 参数 ID 映射 |
| `src/backend/` | 3 | 849 | 岛内枢纽：winit 后端 + chat 会话装配 |
| `src/platform/window.rs` | 1 | 129 | RawWindowHandle 实测分类 + 窗口定位 |
| `src/user_event.rs` | 1 | 191 | `PetUserEvent`（托盘/桌宠事件类型） |
| `src/cli/path_resolve.rs` | 1 | 63 | Bai 缺省模型路径解析（仅冒烟/chat/bench 用） |
| **合计** | **31** | **8,954** | `git rm -r` 一行未留（`--list` 证据见 §5） |

`code-stats` 前后（同一把尺子）：Rust 生产 **67,310 → 60,385（−6,925）** · 生产文件
**239 → 210（−29）** · 内联测试 15,307 → 13,349（−1,958）· 集成 13,127 → 12,176（−951）·
`>500` 超限文件 **58 → 48** · `live2d-ai-desktop` 依赖 **34 → 25**（真删 9 条，见 §4）。

## 2. 五处主链触点（维护者已批准；**对外 WS 帧零变化**）

| # | path:line（移出前） | 现状 | 处置 |
|---|---|---|---|
| 1 | `src/app_event.rs:21` | `use crate::user_event::PetUserEvent;` | 删除 |
| 2 | `src/app_event.rs:27` | `AppEvent::Tray(PetUserEvent)` 变体 | 删除变体 |
| 3 | `src/app_event.rs:251` | `pub fn send_app_event(&EventLoopProxy<AppEvent>, ...)` | 删除（顺带摘掉 `winit` 最后引用） |
| 4 | `src/web_api/ws/events.rs:203` | `\| AppEvent::Tray(_) => { None }` 投影臂 | 删除该臂（**该臂本就不投影**，零帧变化） |
| 5 | `src/supervisor.rs:859` | 测试 `non_conversation_events_yield_none` 用 `Tray` 当样本 | 换等价样本（`RootAudit::Dropped` + `RootAudit::PlaybackCleared{epoch:1}` + `ShutdownReady`），**测试名与断言强度不变** |

其余主链改动（同属 task-13 文件范围，非行为改动）：`src/main.rs` 模块声明与 CLI 收敛、
`src/platform/mod.rs` 收窄 `pub use`、`src/platform/capabilities.rs` 裁掉
`ObservedAlphaMode` / `choose_alpha_mode` / `RuntimeCapabilities::alpha_mode|backend` /
`summarize()`、`src/cli/{mod,usage,tests}.rs` 收敛到 Info / `--audio-smoke*` / `--web`。

## 3. 连带对象处置（只删点名对象一定编译不过）

| 连带对象 | 为什么必须一起处理 | 处置 |
|---|---|---|
| `src/backend/`（849 行） | `chat.rs` 引 `app::{ChatBridge,ShellApp,run_shell}` 等 | 整体删除 |
| `src/adapter/`（566 行） | keeper 引用数 = 0（只有 `app/` 与 `benchmark/` 用它） | 整体删除 |
| `src/platform/window.rs`（129 行） | 只有 `app/` 用（`mod.rs` 的 `pub use`） | 删除 + 收窄 `pub use` |
| `app_event::send_app_event()` | 唯一调用方是 `tray/` 与 `backend/chat.rs` | 删除（主链触点 3） |
| `src/user_event.rs`（191 行） | 只被托盘/桌宠/`app_event` 用 | 删除（主链触点 1/2） |

### 3.1 休眠保留（**无生产调用方**，用 `#[allow(dead_code)]` 显式钉住）

这四处**不是**忘删，而是「壳半边」的休眠契约或回归探针，删掉会丢语义或丢覆盖；
逐个在代码里带注释 + 指回本文，等后续裁决（D4/D6 可收）：

| 位置 | 内容 | 为什么留 |
|---|---|---|
| `audio/output.rs` / `audio/ring.rs` | `AudioOutputFacade::mouth_snapshot()` / `PlaybackHandle::mouth_snapshot()` | 口型电平快照，原唯一消费方是壳的 `ShellApp`；RMS 计算与 `MouthSnapshot` 类型仍是音频链路的可测面 |
| `platform/capabilities.rs` | `CapabilityState::{Available,Unavailable}` | 三态词汇由默认模式 Info 表打印；构造方（窗口/托盘/穿透实测）随壳移出 |
| `supervisor.rs` | `SupervisorHandle::finish_tx` + `report_action_finished()` | AGENTS「动作与表演的休眠台账」把动作完成回报列为**休眠契约的壳半边**（不得私自接回，也不随手删） |

## 4. 依赖与门禁（F-V0-9：降了必须收紧）

真删 9 条 `[dependencies]`：`wgpu` `winit` `pollster` `egui` `egui-winit` `egui-wgpu`
`raw-window-handle` `ksni` `url`（`url` 在移出前**就已经是死依赖**：全 crate `url::` 0 命中）。
`xtask` 的棘轮 `RATCHET_DESKTOP_DEPS` **同一 commit 34 → 25**，
`cargo run -p xtask -- code-stats --check --only deps` → PASS（25 ≤ 25）。
**未**顺手收紧的棘轮：`RATCHET_SRC_RS_500` 仍是 58（实测 48）——留给 leader 裁决
（收紧属 D0 刻度纪律，本段范围外）。

## 5. 退出构建的测试（逐条全名）

规则（红线 8 修订版）：`退出集合 ⊆ 被移出目录`，且在**保留文件**里的退出必须逐条给出原因。
机械证据 = `cargo test --workspace --all-targets -- --list` 前后两份（1410 → 1302）取 multiset 差。

### 5.1 住在被删文件里的退出（100 条，⊆ 被移出目录）

#### adapter · 7 条

`src/adapter/`（2 文件 / 566 行，仅被 app/ 与 benchmark/ 引用）

```text
adapter::tests::apply_frame_writes_input_layer_and_reports_counts
adapter::tests::eye_open_scale_maps_to_both_eyes_around_bai_default_one
adapter::tests::frame_entries_covers_every_field_with_bai_standard_ids
adapter::tests::missing_parameters_degrade_silently_but_are_recorded_once
adapter::tests::mouth_channel_is_highest_priority_and_never_covered_by_surprise_curve
adapter::tests::release_all_clears_everything_tracked
adapter::tests::sparse_write_only_owned_channels_and_release_on_finish
```

#### app · 39 条

`src/app/`（12 文件 / 3,856 行，egui 原生壳）

```text
app::capability::tests::click_through_to_hittest_inverts_click_through_intent
app::capability::tests::click_through_to_hittest_is_idempotent_under_double_negation
app::capability::tests::click_through_to_hittest_keeps_interactive_intent
app::settings_ui::tests::draft_to_settings_preserves_all_fields
app::settings_ui::tests::route_f10_toggle_is_decoupled_from_consumed
app::settings_ui::tests::route_hidden_always_false_even_if_egui_consumed
app::settings_ui::tests::route_visible_only_when_egui_actually_consumed
app::settings_ui::tests::settings_to_draft_round_trip_back_to_equal_draft
app::settings_ui_tests::api_key_env_never_carries_a_real_secret
app::settings_ui_tests::dev_mode_round_trips_through_draft
app::settings_ui_tests::invalid_api_key_env_does_not_modify_disk
app::settings_ui_tests::invalid_llm_url_does_not_modify_disk
app::settings_ui_tests::invalid_tts_url_does_not_modify_disk
app::settings_ui_tests::save_draft_empty_key_env_normalizes_to_none_on_reload
app::settings_ui_tests::save_draft_writes_draft_to_disk_and_reload_round_trips
app::settings_ui_tests::saving_visible_fields_preserves_hidden_tts_fields
app::tests::acquire_outcome_from_current_maps_plain_variants
app::tests::advance_validation_streak_first_validation_is_skip_first
app::tests::advance_validation_streak_non_validation_clears_streak
app::tests::advance_validation_streak_resets_after_non_validation
app::tests::advance_validation_streak_second_validation_is_fatal_code
app::tests::advance_validation_streak_success_breaks_streak
app::tests::advance_validation_streak_three_in_a_row_fatal_at_second
app::tests::alpha_mode_conversion_round_trip_is_lossless_for_explicit_modes
app::tests::classify_poll_failure_returns_gpu_environment
app::tests::gpu_fault_kind_as_str_is_stable
app::tests::lost_action_triggers_recreate_branch
app::tests::outdated_action_skips_present_and_only_reconfigures
app::tests::panic_payload_message_extraction
app::tests::raw_display_handle_classification_matches_window_side
app::tests::raw_window_handle_classification_marks_x11_and_wayland
app::tests::resize_burst_then_drain_pipeline_keeps_latest_size
app::tests::resize_multiple_events_collapse_to_last_nonzero_size
app::tests::resize_zero_size_does_not_overwrite_existing_pending
app::tests::route_gpu_fault_three_tier_mapping_matches_c2
app::tests::suboptimal_action_marks_present_then_reconfigure_in_order
app::tests::success_outcome_is_plain_present
app::tests::surface_format_prefers_non_srgb_with_alpha
app::tests::transient_outcomes_are_silent_skip
```

#### backend · 8 条

`src/backend/`（3 文件 / 849 行，岛内枢纽；chat.rs 是 winit 壳唯一实现）

```text
backend::tests::chat_config_path_resolves_explicit_and_cwd_fallback
backend::tests::description_declares_pet_capabilities_and_runtime_gate
backend::tests::only_winit_wgpu_backend_is_registered
backend::tests::request_feature_declares_implemented_code_paths_only
backend::tests::run_options_validation_and_default_timeout
backend::tests::run_report_includes_model_smoke_section_when_present
backend::tests::run_report_summary_mentions_real_capabilities_only
backend::tests::stop_reason_display_is_stable
```

#### benchmark · 19 条

`src/benchmark/`（5 文件 / 1,900 行，benchmark-only A/B）

```text
benchmark::stats::tests::action_counts_default_is_all_zero
benchmark::stats::tests::mode_caliber_distinguishes_blocking_vs_submit
benchmark::stats::tests::skip_buckets_default_is_all_zero
benchmark::stats::tests::skip_buckets_separates_suboptimal_from_skipped_and_recovery
benchmark::stats::tests::skip_buckets_total_sums_all_kinds
benchmark::stats::tests::timing_stats_does_not_panic_on_nan_inputs
benchmark::stats::tests::timing_stats_handles_empty_and_single_sample
benchmark::stats::tests::timing_stats_percentiles_use_floor_index_and_match_avg_max
benchmark::tests::benchmark_backend_distinguishes_surface_vs_headless
benchmark::tests::benchmark_mode_roundtrips_cli_strings
benchmark::tests::benchmark_report_summary_contains_key_channels
benchmark::tests::phase_advance_done_always_returns_finish
benchmark::tests::phase_advance_enter_formal_triggers_caller_side_reset
benchmark::tests::phase_advance_formal_target_ten_finishes_on_tenth_attempt
benchmark::tests::phase_advance_formal_target_zero_finishes_on_first_formal_attempt
benchmark::tests::phase_advance_panics_on_invalid_incremented
benchmark::tests::phase_advance_signature_pure_and_unit_granular
benchmark::tests::phase_advance_warmup_target_three_enters_formal_on_third_attempt
benchmark::tests::phase_advance_warmup_target_zero_enters_formal_on_first_attempt
```

#### model_smoke · 8 条

`src/model_smoke/`（3 文件 / 688 行，Bai 实时渲染冒烟）

```text
model_smoke::driver::tests::action_sequence_plays_fixed_order_then_wraps
model_smoke::driver::tests::frame_clock_accumulates_fixed_steps_with_caps
model_smoke::driver::tests::smoke_driver_finishes_each_action_before_the_next
model_smoke::report::tests::stats_summary_covers_key_channels
model_smoke::tests::error_categories_map_to_exit_code_contract
model_smoke::tests::render_error_classification_matches_stage_semantics
model_smoke::tests::scripted_actions_are_always_medium_rule_fallback
model_smoke::tests::synthesized_mouth_level_is_bounded_gated_and_deterministic
```

#### platform::window · 2 条

`src/platform/window.rs`（1 文件 / 129 行，RawWindowHandle 实测分类 + 定位）

```text
platform::window::tests::bottom_right_target_places_window_inside_monitor_with_margin
platform::window::tests::window_backend_kind_support_matrix_matches_protocol_reality
```

#### repl · 7 条

`src/repl.rs`（1 文件 / 173 行，`--chat` 终端壳）

```text
repl::tests::blank_and_comment_lines_yield_none
repl::tests::chat_text_is_trimmed_with_inner_spaces_kept
repl::tests::chinese_chat_text_passes_through_verbatim
repl::tests::command_words_tolerate_extra_whitespace
repl::tests::known_commands_match_case_insensitively
repl::tests::slash_inside_text_stays_chat
repl::tests::unrecognized_slash_word_yields_unknown
```

#### tray · 5 条

`src/tray/`（2 文件 / 539 行，ksni 托盘）

```text
tray::menu::tests::menu_model_has_required_entries_in_order
tray::tests::icon_renderer_shape_and_size
tray::tests::ksni_error_classification_is_user_visible_chinese_reasons
tray::tests::menu_model_reflects_state_changes
tray::tests::shared_state_snapshot_round_trip
```

#### user_event · 5 条

`src/user_event.rs`（1 文件 / 191 行，PetUserEvent 事件类型）

```text
user_event::tests::event_target_resolves_toggle_against_current_state
user_event::tests::flip_is_boolean_negation
user_event::tests::launch_plan_forbids_click_through_without_tray
user_event::tests::launch_plan_full_when_tray_confirmed
user_event::tests::launch_plan_non_pet_mode_stays_interactive
```

### 5.2 住在**保留文件**里的退出（11 条；3 条被改名/新增替代，见 5.3）

`cli/tests.rs` 与 `platform/capabilities.rs` 都是保留文件；下列 11 条退出里，
**9 条**所测对象已随壳删除（岛 CLI 入口 / 岛 alpha 选择器），**2 条**是**改名**（同名同断言换名，
计为「退出 + 新增」）：

```text
cli::tests::model_smoke_defaults_and_explicit_paths
cli::tests::model_smoke_path_resolution_prefers_explicit_then_cwd_then_manifest
cli::tests::pet_mode_alone_implies_resident_window_shell
cli::tests::pet_mode_combines_with_window_and_model_smoke
cli::tests::pet_mode_rejects_audio_smoke
cli::tests::smoke_frames_implies_window_smoke_without_forced_timeout
cli::tests::web_is_mutually_exclusive_with_other_modes
cli::tests::window_and_audio_smoke_modes_conflict_and_orphan_flags_rejected
cli::tests::window_smoke_plain
platform::capabilities::tests::choose_alpha_mode_prefers_post_multiplied_for_straight_alpha_output
platform::capabilities::tests::runtime_capabilities_table_and_summary_cover_all_fields
```

### 5.3 新增（3 条，净变化 = 111 − 3 = 108）

```text
cli::tests::removed_native_shell_flags_are_unknown_parameters_not_silently_ignored   （新钉子：被移除入口必须报错）
cli::tests::web_is_mutually_exclusive_with_the_only_remaining_mode                  （改名：原 web_is_mutually_exclusive_with_other_modes）
platform::capabilities::tests::runtime_capabilities_table_covers_all_fields          （改名：原 …_table_and_summary_cover_all_fields）
```

**⚠ 需要 leader 明确签署的一条**：严格口径下「主链集合差 = 0」在本段**不成立**——
保留文件里净 **−8** 条（cli 22→15、platform 9→6）。原因逐条：
`cli` 的 8 条测的是已删除的 CLI 入口（`--window-smoke`/`--model-smoke`/`--pet-mode`/
`--smoke-frames`），`platform::capabilities` 的 1 条测的是已删除的 `choose_alpha_mode`。
**没有一条**主链行为（web_api / supervisor / runtime / core / l2d / wasm / mod-system /
5 个在册 Mod）的测试退出：这些模块的前后条数差全部为 **0**（web_api 372→372、
supervisor 42→42、audio 55→55、mod_registry 18→18、session_scope 16→16、logging 8→8、
app_event 3→3、main.rs tests 4→4）。

## 6. 恢复条件与步骤

1. 先读懂本文 §2：**对外 WS 帧字段名零改动**是硬条件；恢复时不得顺手改帧；
2. 取回文件（只取需要的部分）：

```bash
cd /home/skystar/Live2D-Ai-fe
git checkout checkpoint/pre-d1-dormant -- crates/live2d-ai-desktop/src/app \
  crates/live2d-ai-desktop/src/backend crates/live2d-ai-desktop/src/adapter \
  crates/live2d-ai-desktop/src/tray crates/live2d-ai-desktop/src/benchmark \
  crates/live2d-ai-desktop/src/model_smoke crates/live2d-ai-desktop/src/repl.rs \
  crates/live2d-ai-desktop/src/user_event.rs crates/live2d-ai-desktop/src/platform/window.rs \
  crates/live2d-ai-desktop/src/cli/path_resolve.rs
```

3. 恢复 `Cargo.toml` 的 9 条依赖与 `main.rs` 的模块声明/CLI 分支、
   `app_event.rs` 的 `Tray` 变体与 `send_app_event`、`ws/events.rs` 的投影臂、
   `platform/mod.rs` 的 `pub use window::Ellipsis`；
4. 恢复后跑 `cargo test --workspace --all-targets`：§5.1 + §5.2 的 111 条应**全部回来**；
5. **恢复 = 重新立项**：需要先回答「谁来维护第二个 UI 壳 / 谁来承担它的验收」
   （`docs/architecture/core-chain-baseline.md` §3.6 的红线），并同步收紧/放开
   `RATCHET_DESKTOP_DEPS`。

## 7. 相关文件

- 裁决与影响简报：`docs/audit/2026-10-01-debloat/W2/D1-IMPACT-BRIEF.md`
- 归档 Mod 台账（W2-A 的第一段）：`docs/architecture/ARCHIVED-mods.md`
- 休眠台账（主链侧）：`AGENTS.md` §「原生第二壳的归属（休眠台账）」——**本节已按本次移出重写**
- 核心链基线：`docs/architecture/core-chain-baseline.md` §3.6
