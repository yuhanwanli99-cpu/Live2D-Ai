# BATCH-0021 · 设置的真源与三态补丁 —— 维度 E 主战场

Phase 1 · 域覆盖 → `crates/live2d-ai-runtime/**`（**本目录收尾**）

## 读过的文件（全部来自 `git ls-files` / `wc -l`）
1. `crates/live2d-ai-runtime/src/settings/view.rs` — 294（**全读**）
2. `crates/live2d-ai-runtime/src/conversation/mod.rs` — （片段：ConversationConfig 四个常量）
3. `shell/flutter/lib/api/settings_models.dart` — 931（**结构 + 关键片段全读**）— 跨端对照
4. `shell/flutter/lib/settings/sections/llm_section.dart` — （片段 :30-45 / :145-155）
5. `shell/flutter/lib/settings/sections/dev_tools_section.dart` — （片段 :1075-1189）

## 跑过的命令（全部只读）
```
wc -l shell/flutter/lib/api/settings_models.dart
grep -n "class |final |@JsonKey|json\[" shell/flutter/lib/api/settings_models.dart
grep -rni "performance" shell/flutter/lib/
grep -rn "表演" shell/flutter/lib/
grep -n "status\[" shell/flutter/lib/settings/sections/dev_tools_section.dart
grep -rn "max_history_pairs|sentence_max_chars|tts_queue_capacity|audio_chunk_samples" conversation/mod.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度 E 的结论（跨端逐字段）
| Rust 字段 | Dart 对应 | 判定 |
|---|---|---|
| `LlmView{base_url,model,has_api_key,max_tokens,show_reasoning}` | `LlmSettingsView` 同 5 键（snake→camel） | ✔ |
| `TtsView{base_url,model,voice,has_api_key,sample_rate,channels}` | `TtsSettingsView` 同 6 键 | ✔ |
| `PersonaView{system_prompt,max_history_pairs}` | `PersonaSettingsView` 同 2 键 | ✔ |
| `ActionView{head/body/expression_scale, models}` | `ActionSettingsView` 同 4 键 | ✔ |
| `ActionModelView{3 × Option<f32>}` **三键恒出现不省略** | `ActionModelOverrideView` 3 × `double?` | ✔ 契约被遵守（view.rs:115-117 明确「刻意不用 skip_serializing_if」） |
| `SettingsView.active_model_id`（**顶层**） | `ActionSettingsView.activeModelId`，但 **Dart 解析器从顶层取**（`settings_models.dart:317-332` 的 `activeModelId` 形参 + 注释「不在 action 段里」） | ✔ **对齐**（我先误判为错位，重读解析器后推翻） |
| `SettingsView.performance`（7 键） | **无** | ✘ **F-0021-01** |
| `SettingsView.dev_mode` | `SettingsView.devMode` | ✔ |

## 四个运行时常量
`sentence_max_chars` 缺省取 `SentenceAssembler::DEFAULT_MAX_CHARS`（mod.rs:156，**单一来源**，
B0015 已读该常量 = 200）；`max_history_pairs` 缺省 **0**（:155，= 永不保留历史，
由 `commit_completed_turn` 的 `if == 0 { return }` 兑现，mod.rs:147-149）；两个音频/TTS 队列常量走
`DEFAULT_TTS_QUEUE_CAPACITY` / `DEFAULT_AUDIO_CHUNK_SAMPLES`。**测试 `mod.rs:416-431` 逐个钉死**。

## 未核实项
1. `settings.rs`(826) 与 `settings/patch.rs`(780) **未逐行读**——本批只从 view.rs 与
   conversation/mod.rs 侧取用它们的字段。三态补丁的实现细节（三层 double_option 的
   合并语义）**仍未审**，顺延 Phase 3 端到端链路③「改设置 → 偏好 → 落盘 → 热重载 → 生效」。
2. `settings/patch_tests.rs`(992) / `settings/settings_tests.rs`(772) 未读。
3. F-0021-01 的「用户因此看不到」依赖前端确实无其它入口——已三重 grep 验证，但
   **未跑真实服务看实际响应体**（本任务禁跑门禁）。

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0 ｜ 另：**回填 F-0002-03**（补上同仓正确参照实现 `key_is_ready`）
