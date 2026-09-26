# HANDOFF 2026-09-22 —— 表演层 v0（主模型不表演；每轮一份合法化 JSON）

> 工作树：`/home/skystar/Live2D-Ai-l1`。**不 bump / 不 push / 不打 tag。**
> 架构真源：用户 2026-09-22 敲定的五条（主模型 = 酒馆式角色扮演、不暴露工具、不加表演类预设；
> 表演层每轮结构化 JSON；接受双 API 延迟；noop 合法；clean_for_tts + 规则导演 = 仅失败回退）。
> 契约终稿：[../architecture/performance-layer-v0.md](../architecture/performance-layer-v0.md)。

---

## 1. 交付了什么

| 面 | 落点 |
| --- | --- |
| **唯一 schema + 严格校验** | `live2d-ai-runtime/src/performance/plan.rs`（`parse_plan` / `json_schema_strict` / 常量） |
| **HTTP 客户端（非流式）** | `performance/client.rs`（reqwest async；json_schema strict 优先，4xx 降级 prompt 重试；思考不进请求） |
| **提示词 + 宽容解析** | `performance/prompt.rs`（SYSTEM_STRUCTURED / SYSTEM_JSON_ONLY / strip_code_fence） |
| **运行时 + 回退 + 计数** | `performance/mod.rs`（`PerformanceRuntime::resolve`、`FallbackReason`、`PerformanceStats`、`assemble`） |
| **引擎接线** | `conversation/engine.rs`（收齐原文 → resolve → speak 切句入队 / ActionCue；关闸时逐字不变） |
| **事件** | `EngineEvent::ActionCue`（`conversation/mod.rs`）→ supervisor → `ConversationUiEvent::ActionCue` → **既有** WS `action_cue` 帧 |
| **配置段** | `[performance]`（`settings.rs` + `settings/view.rs` + `GET /api/v1/settings` 的 `performance` 块 + `live2d-ai.toml.example`） |
| **装配（启动 + 热重载）** | `supervisor.rs::build_performance_runtime`（能力集 = director `PRESET_IDS`；规则回退 = director `rule_cues_for_text`） |
| **观测** | 日志（`target="performance"`，成功一份摘要 / 回退原因码）+ `GET /api/v1/app/status` 的 `performance` 计数块 |
| **规则回退纯函数** | `live2d-ai-mod-director/src/presets.rs::rule_cues_for_text` |
| **面板文案** | `shell/flutter/lib/settings/sections/llm_section.dart`：「**主模型不负责表演；表演层每轮 JSON**」 |
| **文档** | `docs/architecture/performance-layer-v0.md`（schema 终稿 + 配置键 + 并存关系 + 审计 + 证据）；`docs/README.md` 索引；`director-mod-v0.md` 并存注记 |

## 2. schema 终稿（唯一真源 = 代码）

```json
{\"speak\": string|null,
 \"cues\": [{\"sentence_seq\": u64, \"preset_id\": string, \"intensity\": number, \"ttl_ms\": number}]}
```

- `speak` 的 `null`/`""` = 本轮不说；`cues=[]` = 本轮不动；两者同时 → **noop**（合法，不是失败）。
- `preset_id` ∉ 能力集 / 缺字段 / 坏 JSON → **整份失败**；`intensity` / `ttl_ms` 越界 → **钳位**；
  未知字段 → 丢弃；cue 上限 16；`speak` 上限 4000 字符（截断）。
- 回归 `schema_and_validator_share_the_same_bounds` 钉住「文档 == 发出去 == 校验器认的」。

## 3. 配置键（`live2d-ai.toml` 的 `[performance]`）

```toml
[performance]
enabled = false          # 默认关；关掉时主链行为逐字不变
base_url = ""            # 独立端点（OpenAI 兼容）；空 = 未配 → 每轮回退（degraded）
model = ""               # 表演层模型名
api_key_env = "..."      # 只写变量名；值住 .env（省略 = 不鉴权）
timeout_ms = 4000        # 钳 100..=30000
structured = "auto"      # auto | json_schema | prompt
```

密钥真源 = `.env`（`secrets::lookup`）；本波不提供 PATCH 入口（改 toml → 热重载生效）。

## 4. 三态 + 回退：各一条证据

| 态 | 证据（测试） |
| --- | --- |
| **noop** | `conversation_engine_performance::noop_plan_sends_no_tts_and_no_text` |
| **只说** | `conversation_engine_performance::speak_is_the_tts_source_and_cues_reach_action_cue` |
| **只动** | `conversation_engine_performance::cue_only_plan_emits_a_silent_anchor_sentence` |
| **失败回退** | `conversation_engine_performance::invalid_plan_falls_back_to_cleaned_raw_and_rule_cue` |
| **端到端（mock 表演层）** | `live2d-ai-desktop::supervisor::tests_performance::performance_layer_end_to_end_drives_speak_and_cues`（真实 `[performance]` 配置 → 真发一次 `/chat/completions`（json_schema/非流式）→ speak 进 TTS → ActionCue） |
| 契约表驱动 | `performance::tests::legal_samples_…` / `illegal_samples_fail_as_a_whole` |
| 客户端 wire | `performance::tests::openai_client_posts_json_schema_and_parses_content` / `auto_mode_degrades_to_prompt_on_4xx` / `server_errors_and_blank_content_fall_back_to_none` |
| 主模型审计 | `performance_never_leaks_into_the_main_model_request` |
| WS 帧复用 | `web_api::ws::events::tests::performance_action_cue_projects_to_the_same_action_cue_frame` |
| 装配三态 | `supervisor::tests_performance_assembly::disabled_is_none_and_partial_config_is_degraded_and_wired_is_openai` |

## 5. 主模型提示词审计结果

- 主模型路径**没有任何**「请输出动作 / JSON / 舞台指示 / 调用工具」类预设；请求体无 `tools`/`tool_choice`/`functions`。
- persona 合成与 example 模板保留的是**投递约束**（1–5 句、句读结尾、不要 Markdown），服务「一句一单元」语音契约，不是表演指令。
- memory 只注入事实行；摘要 system 是独立压缩调用，不含表演指令。
- 回归见上表「主模型审计」；UI 侧文案见 `llm_section.dart`。

## 6. 与 director Mod staging 的并存关系

**表演层 = 主路由；staging_* = 遗留并行实现**。表演层开着时 supervisor **不**把
`SentenceReady` 转给 Mod（`forward_sentence_ready_to_mods = !engine.performance_enabled()`）——
否则两份 `action_cue` 会在前端互相**整份覆盖**。规则回退仍走 director 的纯函数（host 注入闭包）。

**2026-09-24 阶段3 收口（D10–D12）**：驱动舞台的**唯一**通道是 WS `action_cue`；
`cues[].preset_id == "none"` = 该句音频开始时**撤销两槽**，`cues: []` = **本轮不动**。
前端拉取 director `latest.preset_id` 驱动舞台的通道**已退役**（`latest` 仅面板只读，D12）。
**D11 不对称（明文）**：performance 开的中性轮 = **noop**（靠上一轮 preset 的 `ttl`
到点收敛）；performance 关的 director 规则路径 = 中性轮产 `preset_id=="none"` cue
**立即撤销**（`DirectorPlan::rule` 已从「空 plan」改为「none cue」）。
回归：`live2d-ai-mod-director` 的 `rule_none_cue_is_the_revoke_sentinel` /
`rule_with_preset_is_field_for_field_unchanged` / `neutral_turn_emits_exactly_one_none_cue`。

## 7. 门禁（本机实测）

| 检查 | 结果 |
| --- | --- |
| `cargo test --workspace --all-targets` | 见 §8 收束数字 |
| `cargo test --doc --workspace` | 见 §8 |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 warning |
| `cargo run -p xtask -- rust-ratio` | **97.1202% PASS** |
| `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` | 见 §8（wasm 一行未改，按门禁仍跑） |
| `flutter analyze` | No issues found! |
| `flutter test` | **1001 passed** |

## 8. 诚实标注

1. **真端点未冒烟**：本波用 loopback/mock 表演层（含 desktop 端到端），没有可用真端点做肉眼验收；structured 降级由 4xx mock 回归证明。
2. **ignite 未跑**：本 worktree 无 `assets/models`，且改了 Dart 需重建 web 产物；端到端改由 `tests_performance`（真实配置 + mock 表演层）覆盖，**不伪造 ignite 绿灯**。
3. **只动依赖前端空句播放**：见契约文档 §9.1。
4. **热重载重置计数**：`performance` 计数随运行时重建清零。
5. **未做**（任务书禁止 / 非目标）：主模型 tool、optional 多 tool 赌调用、每轮写盘交接文件、motion3、把原始 JSON 念进 TTS、bump/push/tag。
