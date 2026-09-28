# BATCH-0017 — Phase 2 · 横扫 E：契约漂移（含错误码全集）

> 账本：`AUDIT-B/`。

## 一、`SettingsView` 逐字段对账（Rust `settings/view.rs` ↔ Dart `settings_models.dart`）

| 段 | Rust | Dart | 判定 |
|---|---|---|---|
| `llm` | `LlmView` | `LlmSettingsView` | ✓ 逐字段一致（含 `has_api_key` 单一语义、`max_tokens` 生效值、`show_reasoning`） |
| `tts` | `TtsView` | `TtsSettingsView` | ✓ |
| `persona` | `PersonaView` | `PersonaSettingsView` | ✓ |
| `action` | `ActionView` | `ActionSettingsView` | ✓ |
| `dev_mode` | `bool` | `bool` | ✓ |
| `active_model_id` | `String`（顶层） | 顶层喂给 `ActionSettingsView` | ✓ |
| **`performance`** | **`PerformanceView`（enabled / wired / base_url / model / has_api_key / timeout_ms / structured）** | **完全未解析** | **F-0017-1** |

## 二、错误码全集对账（`ErrorKind::code()` ↔ 前端按码分支）

- 后端全集：`llm_{upstream_<status> | transport | base_url_invalid | sse_parse | json}`、`tts_{upstream_<status> | transport | audio_format | pcm_unaligned | audio_config}`、`tts_backpressure`、`decode_{同上后缀}`。
- 前端 `error_actions.dart:32-42`：`startsWith('llm_')` → LLM 分区；`startsWith('tts_') || startsWith('decode_')` → TTS 分区；`code == 'busy'` → 打断并重发；其余 → 「重试」。
- **判定：前缀匹配覆盖了后端的每一个码 ✓**（含 `tts_backpressure`，它以 `tts_` 开头）。API 层码（`busy` / `no_supervisor` / `invalid_payload` / `dev_mode_required` / Mod 命令码）各有专门文案或走兜底。
- 唯一瑕疵（前瞻）：`api_client._errorFrom` 在**响应体不是 JSON** 时合成 `http_<status>` —— 该码在后端日志里搜不到（后端永远发标准错误体），且文案会是整段 HTML。记 P3。

## 三、已在此前对过账的（不重复）
- WS 帧 9 个 type + 字段（0010）✓；v1 渲染面协议 6 type + 10 payload 键（0010）✓；`show_reasoning` 三态（0008）✓；`ModsApi` 的 secret 不回值（0008）✓。

## 发现
- **F-0017-1（P2）** 后端 `SettingsView` 有 `performance` 段（enabled / wired / base_url / model / has_api_key / timeout_ms / structured），前端**一个字段都不解析**；LLM 分区因此把「表演层」渲染成**写死的「（默认关）」**——用户真开了表演层，界面仍在说「默认关」。

## 本批核对过、不成发现的（正面记录）
- 错误码纪律在**前缀层面**是严密的：后端 `code()` 的构造（`<stage>_<suffix>`）与前端的三条 `startsWith` 分支**逐条对得上**——这在跨语言契约里并不常见（本仓做到了）。
- `error.rs:102` 专门暴露了 `upstream_status()` 并写明理由「调用方常需只按状态码分流，不该去解析 Display 字符串」——后端自己也守同一条纪律。
- `ChatAccepted.session_id` 前端没读：核对后确认**无害**（入参就是它，回包仅作确认；换皮走 `model_url`）。
- `director_panel.dart:52-59` 知道「表演层开启时本轮 cue 以 performance 为准」——**前端在别处知道这个特性**，偏偏在设置页把它写死成「默认关」，这让 F-0017-1 更像遗漏而不是设计。
