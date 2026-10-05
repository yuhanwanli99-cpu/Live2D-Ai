# BATCH-0123 · ⭐ `SettingsView` ↔ `AppSettings` 字段对等性：**差集全是刻意的替换，无遗漏**

Phase 1 · 域覆盖 · `live2d-ai-runtime`（红线 R 的**序列化面**收口）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/settings.rs` — 826（机械扫各段 `pub` 字段）
2. `crates/live2d-ai-runtime/src/settings/view.rs` — 294（机械扫各 View 段）
3. `shell/flutter/lib/settings/sections/tts_section.dart` — （定点 7：`response_format` 的取舍说明）

## 跑过的命令（全部只读）
```
python3 - <<'PY'   # 机械集合差：settings 各段 pub 字段 vs view 各 View 段
  … 逐段 diff …
PY
grep -rn "response_format" runtime/src/settings.rs
grep -rn "response_format|responseFormat" shell/flutter/lib/ runtime/src/settings/view.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。**未执行任何 Dart/Rust 代码**（纯文本集合运算）。

## 本批产出：**0 条新发现**；红线 R 的**序列化面**闭环
### 机械差集结果（每段逐项核对）
| 段 | settings 有、view 无 | view 多出 | 判定 |
|---|---|---|---|
| Llm | `api_key_env` | `has_api_key` | ✔ **敏感 → 派生布尔** |
| Tts | `api_key_env`、**`response_format`** | `has_api_key` | ✔ 逐项见下 |
| Persona | 无 | 无 | ✔ 完全对等 |
| Performance | `api_key_env` | `has_api_key`、`wired` | ✔ `wired` 是派生布尔 |
| Action | 无 | 无 | ✔ 完全对等 |

⇒ **四处 `api_key_env` 全部被 `has_api_key: bool` 取代，一处都没漏**
⇒ 而这比 B0009 记的「不回值」**更进一步**：**连环境变量名都不出门**
（`api_key_env` 装的是 `DEEPSEEK_API_KEY` 这类**名字**；不给它，就无法从
`GET /api/v1/settings` 推断出用户用了哪个变量）。

### 唯一需要追问的 `response_format`：**刻意的，且两侧都有说明**
`settings.rs:222-223` 有该字段（带 `#[serde(default = "default_response_format")]`），
而 view 与 patch 里都没有 ⇒ 前端 [tts_section.dart:7](</home/skystar/Live2D-Ai-fe/shell/flutter/lib/settings/sections/tts_section.dart:7>) 逐字写着：
> 「`response_format` **不在 view 也不在 patch 里** → UI 上**不存在**这个控件。」

⇒ **不是遗漏，是取舍**，且**两侧都写明了**。⇒ 不记发现。
（背景见 `audio/wav.dart:22`：本机 TTS **实测出不了 mp3/wav，只有 `pcm` 是 200**
⇒ 这个字段对用户没有可选空间，不做成控件是合理的。）

⇒ **红线 R 在序列化面上的最后一块拼图补齐**：
`GET /api/v1/settings` 的脱敏视图与真实设置的字段差集，**全部是「敏感字段 → 派生布尔」的替换**，
**没有任何字段未经脱敏直接出门**。

## 未核实项
1. `patch.rs` 中段（tts / persona / performance / action 各段的三态）未逐段核
2. `secrets.rs` 余下方法（`parse` / `lookup` / `refresh_from_disk` / `known_keys`）未审
3. `performance/plan.rs`(857) / `llm.rs`(530) / `conversation/{engine,mod,worker}.rs` 未审
4. `sse.rs`(361) / `audio/{resample,rms}.rs` / `error_code.rs` 未审
5. **B0120 留的未核实项仍在**：是否有代码在别处 chmod 过 `live2d-ai.toml`
