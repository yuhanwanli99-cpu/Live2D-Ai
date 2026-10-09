# 导演把原文更好地送进 TTS · 交给下一个 agent

> 2026-10-07。管理员已定口径。下一个 agent 只做 T8。
> 不要提交、不要打 tag、不要推送、不要升版本。做完由管理员审查。

## 口径

送进 TTS 的文本由主模型原文定死（直送）。导演不改字、不删句、不拒答、不加声明，不做内容审查。它只把这句原文分段后送进 TTS，并让头、颈、表情跟着走。

分段就是现成表演层的 `segments`：拼接必须逐码点等于原文（`performance/plan.rs` 里 `segments.concat() != source` 整份失败，不要放宽）。拼不回、超时、坏 JSON，引擎退回今天的边流边送，原文一个字不丢。

cue 只留头和表情。颈用现有头部角 `ParamAngle*`，不要新参数，不要改 `presets.json`，不要写手臂。模型若交了 `field=body`，丢掉这一条并 warning，不要因此把整份 plan 判失败。

## 模型

导演这路默认复用主 LLM：同一个 `base_url`、模型名、密钥变量名。

| `staging_base_url` | `staging_model` | 行为 |
| --- | --- | --- |
| 空 | 空 | 复用主 LLM |
| 都有 | 都有 | 用这两项。`staging_api_key_env` 空则仍用主 LLM 的密钥变量名 |
| 只写了一项 |  | 不接管，保持直送 |

`staging_enabled` 不是第二道总闸。`live2d-ai.toml` 的 `[performance].enabled` 继续默认关，不能单独顶掉直送。不要改 `mods.json`、`.env`、`live2d-ai.toml`。

导演开着且接管成功时，每一轮两次请求：主模型先写完原文，再用选定端点非流式要一份 JSON。

## 开关

只有「这次进程加载时导演 Mod 已启用」。打开或关掉都要重载才生效。

| 加载时 | 送 TTS | 动作 |
| --- | --- | --- |
| 导演关 | 边流边切句，直接送 | 无导演 cue。口型照旧 |
| 导演开，且按上表会接管 | 等原文收齐，用分段送。失败则退回直送 | 头、颈、表情。失败则规则 cue |
| 导演开，但只填了一项二路端点 | 仍直送 | 规则层照旧 |

接管成功时，导演 Mod 自己的 HTTP（`should_fire_async`）必须返回 false，避免两份 cue。没接管时规则层保持今天的行为。

第二次请求的系统提示以 `crates/live2d-ai-runtime/src/performance/prompt.rs` 的 `SYSTEM_STRUCTURED` / `SYSTEM_JSON_ONLY` 为准（接管走 `PerformanceRuntime`）。那里现在还在邀请 `body`，要改成只切分、只给 `head` 和 `expression`，并写明禁止改写和审查。导演 `STAGING_SYSTEM` 在接管后不应再被发出。

## 接线

- `crates/live2d-ai-desktop/src/supervisor.rs` 的 `build_performance_runtime`：按上表装配现成 `PerformanceRuntime`。导演关、或只填了一项 → `None`。不要用 `[performance].enabled` 当开关。
- 装配测试四条：导演关 → `None`；开且两项空 → 运行时模型名等于主 LLM；开且两项都有 → 用导演的模型名；只填一项 → `None`。
- `crates/live2d-ai-mod-director/src/lib.rs`：五个 `staging_*` label 去掉「【遗留】」和「日常用表演层」。地址和模型名写明「留空则用对话模型」。`tests.rs` 里「label 必须含【遗留】」改成相反。
- `shell/flutter/lib/settings/mods/director_panel.dart`：说明改为「重载后，导演用对话模型把原文分段送 TTS，并驱动头、颈、表情；不改原文。地址和模型名留空就是这个对话模型；两项都填则改用那里。只填一项则仍直送。」状态行读已有 `state_json`：未开启 / 已接管（写明复用对话模型或自有模型名）/ 没接上；规则预设；实际下发。
- 更新 `test/performance_layer_copy_test.dart` 的导演组、`test/action_scales_wiring_test.dart` 的 C③、`test/director_panel_test.dart`。`kLlmPerformanceText` 与 `kLlmPerformanceDescription` 原文不要改。
- 新界面句子只用 Noto Sans SC 子集里已有的字。

## 禁止

- 改写或审查原文；放宽「拼接必须等于原文」。
- 导演关时切断直送；两项都空时不复用主 LLM；只填一项仍发第二次请求。
- 两路 cue 同时发。新 WS 帧。接回 `action_tx`。
- 改 `mods.json`、`.env`、`live2d-ai.toml`、`presets.json`、口型、待机、拖动、动作幅度、wasm、舞台背景、语音。
- 新增手臂参数。用 `[performance].enabled` 当日常开关。
- 在仓库根目录跑 `flutter analyze`。把测试改成恒真。提交、推送、升版本。

语音封存，不在 T8。以后和 Live2D 透传 Mod 一起做。

## 验收

```bash
cargo test -p live2d-ai-mod-director
cargo test -p live2d-ai-desktop --bins
cd /home/skystar/Live2D-Ai/shell/flutter
/home/skystar/flutter/bin/flutter analyze
/home/skystar/flutter/bin/flutter test test/performance_layer_copy_test.dart test/director_panel_test.dart test/action_scales_wiring_test.dart test/font_subset_test.dart
```

都要 exit 0。贴出各自最后 30 行。桌面那条若因 `HTTP_PROXY` 把传输失败用例如成 `llm_upstream_502`，先去掉代理再跑，不要改断言，也不要改 reqwest 客户端。

## 提示词（整段复制给下一个 agent）

```text
你在 /home/skystar/Live2D-Ai，分支 main。先读 docs/plans/PROMPT-director-tts-2026-10-07.md。只做 T8。不要提交，不要推送，不要升版本。

口径：直送。主模型原文是唯一台词。导演不改字、不删句、不拒答、不加声明，不做内容审查。它只把原文更好地送进 TTS（segments 必须能逐字拼回原文），并驱动头、颈、表情。拼不回、超时、坏 JSON 就退回边流边送。

导演这路默认复用主 LLM：同一个 base_url、模型名、密钥变量名。staging_base_url 和 staging_model 都空 = 复用。两项都写了 = 用导演自己的端点（staging_api_key_env 空则仍用主 LLM 的密钥变量名）。只写了一项 = 不接管，保持直送。staging_enabled 不是第二道总闸。不要用 live2d-ai.toml 的 [performance].enabled 当开关。不要改 mods.json、.env、live2d-ai.toml。

开关是：本次进程加载时导演 Mod 已启用。关 = 边流边送，无导演 cue。

要做：
1. supervisor.rs 的 build_performance_runtime 按上表装配现成 PerformanceRuntime。补测试四条：导演关 → None；开且两项空 → 模型名等于主 LLM；开且两项都有 → 用导演的模型名；只填一项 → None。
2. 主链已接管时，导演 should_fire_async 返回 false。没接管时规则层行为不变。
3. 二路系统提示写明：只切分原文，只输出 head 与 expression，禁止改写和审查。校验遇到 field=body 丢掉并 warning。不改 presets.json。颈用现有 ParamAngle，不要新参数。
4. 导演设置五项 label 去掉「【遗留】」和「日常用表演层」。地址和模型名写明「留空则用对话模型」。导演卡片说明改成：重载后用对话模型把原文分段送 TTS，并驱动头、颈、表情；不改原文。两项留空即这个对话模型；两项都填则改用那里；只填一项则仍直送。卡片读已有 state_json，显示未开启 / 已接管（复用或自有模型名）/ 没接上，以及规则预设和实际下发。
5. 更新钉死旧文案的测试：director tests.rs、performance_layer_copy_test.dart 的导演组、action_scales_wiring_test.dart 的 C③、director_panel_test.dart。kLlmPerformanceText 与 kLlmPerformanceDescription 原文不要改。

禁止：改写或审查原文；导演关时切断直送；两项都空时不复用主 LLM；新 WS 帧；接回 action_tx；改口型、待机、拖动、动作幅度、wasm、舞台背景、语音；新增手臂参数；在仓库根目录跑 flutter analyze；把测试改成恒真。

验收并贴出最后 30 行：
cargo test -p live2d-ai-mod-director
cargo test -p live2d-ai-desktop --bins
cd shell/flutter && /home/skystar/flutter/bin/flutter analyze && /home/skystar/flutter/bin/flutter test test/performance_layer_copy_test.dart test/director_panel_test.dart test/action_scales_wiring_test.dart test/font_subset_test.dart
桌面那条若遇 HTTP_PROXY 导致传输失败用例变 502，先去掉代理再跑，不要改断言。
```
