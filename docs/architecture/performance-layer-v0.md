# 表演层 v0（[performance] 段）：每轮一份合法化 JSON，speak 是 TTS/上屏真源

> **状态**：2026-09-22 用户敲定架构后落地。工作树 `/home/skystar/Live2D-Ai-l1`。
> **不 bump / 不 push / 不打 tag**。
>
> 代码单一真源：`crates/live2d-ai-runtime/src/performance/`（契约 + 校验 + 客户端）
> 与 `crates/live2d-ai-runtime/src/conversation/engine.rs`（接线）。
> 本文与代码冲突时**以代码 + 回归为准**，并回改本文。

## 0. 一句话

**主模型不负责表演。** 主模型（酒馆式角色扮演）只写剧情正文；**表演层**（导演 / 大脑）
每轮异步收一次「用户输入 + 主模型原文」，交回**一份合法化 JSON**：
`{"speak": string|null, "cues":[...]}`。其中 **speak 是本轮 TTS / 上屏的唯一真源**，
**cues** 进既有 WS `action_cue` 帧（锚该句音频的 `first_chunk`）。
表演层**默认关**；关掉时主链行为与没有本段时**逐字一致**（边流边切句边送 TTS，规则导演照旧）。

## 1. 分工（不可互换）

| 层 | 做什么 | 不做什么 |
| --- | --- | --- |
| **主模型**（`[llm]`，酒馆式） | 角色扮演；人设 + 记忆/上下文注入；写剧情正文 | **不暴露任何工具**；**不加表演类提示词预设**（不要求输出动作/JSON/舞台指示）；不决定说辞的最终形态 |
| **表演层**（`[performance]`，独立端点） | 每轮交回一份 JSON：`speak`（说什么）+ `cues`（演什么） | 不改剧情立场、不添加原文没有的事实；不直接驱动渲染面（经既有 action_cue 契约） |
| **规则导演**（director Mod / `rule_cues_for_text`） | **仅失败回退**：表演层关/超时/非 2xx/校验失败时给一条规则 cue | 当前实现：表演层开着时 host **不**把 `SentenceReady` 转给 Mod（避免两套 cue 打擂台）；两者是**两个可选提供者，都默认关、职责重叠**，**谁的 `speak` 能力该保留未定**（RESEARCH §3.7 Q1） |
| **确定性清洗**（`clean_for_tts`） | 任何路径的送 TTS 文本都过它（只拆标记、不改句界） | 不改写说辞、不改变句边界 |

### 1.1 为什么放在 runtime 而不是 Mod

因为 `speak` 必须成为**主链 TTS / 上屏的真源**——Mod API 没有改写 TTS 文本的通道
（那条纪律写在 [core-chain-baseline.md](core-chain-baseline.md)）。所以表演层住在
`live2d-ai-runtime`（与 TTS 同层），而不是 `live2d-ai-mod-director` 的二路 LLM。

### 1.2 主模型提示词审计结果（2026-09-22）

审计范围：主模型请求体的**全部** system / 注入来源——`[persona] system_prompt`、
persona Mod 合成（角色卡字段 + 纪律模板）、memory Mod 注入块、`live2d-ai.toml.example`
模板、`live2d-ai.toml` 现网配置。结论：

- **没有任何「请输出动作 / JSON / 舞台指示 / 调用工具」类预设**——工具层在 2026-09-11 已整体拆除，
  请求体里没有 `tools` / `tool_choice` / `functions`；persona 合成里也没有表演指令。
- persona 的【对话纪律】与 example 模板的 1–3 条是**投递约束**（句数、句读结尾、不要 Markdown），
  服务「一句一单元」的语音契约，**不是表演指令**。appearance 上它们是唯一保留的额外要求。
- **回归**：`conversation_engine_performance::performance_never_leaks_into_the_main_model_request`
  抓主模型请求体原文，断言不含 `speak` / `cues` / `preset_id` / `json_schema` / `sentence_seq`，
  且 system 仍是配置里的人设、只有 `system + user` 两条消息。
- **界面侧**：Flutter「LLM」分区写明「**主模型不负责表演；表演层每轮 JSON**」
  （`shell/flutter/lib/settings/sections/llm_section.dart`）。

## 2. 唯一输出 schema（终稿）

```json
{\"speak\": string|null,
 \"cues\": [{\"sentence_seq\": u64, \"preset_id\": string, \"intensity\": number, \"ttl_ms\": number}]}
```

- **代码单一真源**：`performance::plan`（结构体 + 常量 + `parse_plan` + `json_schema_strict`）。
  发给 provider 的 json_schema **由同一组常量拼出**；回归
  `schema_and_validator_share_the_same_bounds` 钉住「文档 == 发出去的 schema == 校验器认的东西」。
- **未知字段一律丢弃**（模型多写一个 `reason` 不该让整轮没有语音）。

### 2.1 校验规则（**整份失败，不做局部抢救**）

| 情形 | 处理 |
| --- | --- |
| 坏 JSON / 顶层不是对象 | **整份失败** → 回退 |
| 缺 `speak` / 缺 `cues` | **整份失败** → 回退 |
| `speak` 既非 string 也非 null | **整份失败** → 回退 |
| `cues` 不是数组 / 某条不是对象 | **整份失败** → 回退 |
| cue 缺 `sentence_seq`（或 <1）/ `preset_id` / `intensity` / `ttl_ms` | **整份失败** → 回退 |
| `preset_id` 不在能力集 | **整份失败** → 回退 |
| cue 条数 > `MAX_CUES`（16） | **整份失败** → 回退 |
| `intensity` 越界 | **钳位** 到 1..=3（不是失败） |
| `ttl_ms` 越界 | **钳位** 到 1..=5000（不是失败） |
| `speak` 超长 | 按字符截断到 4000（防无标点长文灌爆队列） |
| 未知字段 / 多余键 | 丢弃（宽容） |

**为什么不局部抢救**：丢一条坏 cue、留其余，会让「这一轮到底演了什么」变成不可解释的混合体；
整份回退到确定性规则层可解释、可单测、可观测。

### 2.2 三态（都是合法输出）

| 态 | 形状 | 行为 |
| --- | --- | --- |
| **noop** | `speak=null/"" 且 cues=[]` | 本轮**不说也不动**：不发 TTS、不上屏、发一份空 action_cue（**空 cues = 本轮不动，不清残留**——撤销只认 `preset_id='none'`，见下） |
| **只说** | `speak 非空 且 cues=[]` | speak 切句 → 逐句送 TTS + 上屏 |
| **只动** | `speak 空 且 cues 非空` | 引擎给一条**无声锚句**（空文本，不发 TTS HTTP）——cue 的锚点是该句音频的 `first_chunk`，没有句子就没有锚点 |

**撤销的唯一哨兵是 `preset_id='none'`（2026-09-23，W4 按实现改正）**：

- 上表 noop 行的「空 action_cue」**不等于撤销**。`cues` 是**按句**锚定的（每条 cue 绑
  `sentence_seq`，在该句音频开始时应用），所以前端把**空 cue 列表 = 本轮不动**
  （`main.dart::_applyDirectorCueForSeq` 直接返回）——用整份计划去归零会把**别的句**
  正在演的表演一起清掉。
- 「本轮判定为中性 / 没有预设」的显式归零走**另一条**通道：状态面
  `GET /api/v1/mods/director/state` 的 `latest.preset_id`。前端 `_applyDirectorPreset`
  在**完成 seq 去重之后**，把 `preset_id` 为 `null` / 空串 / `'none'` 的一律视为
  「本轮没有预设」→ 下发 `applyPreset('none', source:'director')`（= 渲染面 `Revoke`，
  两个槽同清）。顺序是契约：去重在前，归零在后，否则每个 `text_delta` 帧都会重复归零。
- **noop 轮不清上一轮残留**：上一轮的 preset 由它**自己的 `ttl`**（表情 2600ms /
  短动作 900ms）收敛；本轮若要立刻收掉它，唯一手段是状态面给出 `preset_id='none'`。
- 本文件此前写的「发一份空 action_cue（清上一轮残留）」与代码不符，**已按实现改正**。
  本轮**不**实现「空计划也归零」：它缺一个锚点定义（该清哪一句、在哪个事件上清），
  会让跨句表演互相打断。

## 3. 配置键（`[performance]` 段，`live2d-ai.toml`）

| key | 类型 | 缺省 | 语义 |
| --- | --- | --- | --- |
| `enabled` | bool | **false** | 总闸。关 → 引擎走既有流式路径，逐字一致 |
| `base_url` | string | 空 | 独立端点（OpenAI 兼容；与 `[llm]` / `[tts]` 无关）。空 = 未配 → 每轮回退（degraded） |
| `model` | string | 空 | 表演层模型名。空 = 未配 → 同上 |
| `api_key_env` | string? | 省略 | 密钥的**变量名**（值只住 `.env`，同 llm/tts 纪律）。空 = 不鉴权 |
| `timeout_ms` | number | 4000 | 单次超时（钳 100..=30000）。比主模型宽：非流式一次往返，且本轮**开声要等它返回** |
| `structured` | string | `"auto"` | structured output 策略：`auto`（先 json_schema strict；上游 4xx 降级 prompt）/ `json_schema` / `prompt` |

- 密钥值走 `.env`（`secrets::lookup`，`.env` 快照 > 进程环境）；`live2d-ai.toml` 只持变量名。
- 本波**不提供 PATCH 入口**（不写进 `SettingsPatch`）：改 `live2d-ai.toml` 后热重载生效。
  配置进了 `AppSettings`，所以界面「保存」的 `merge_into_toml` **不会**把它删掉（那条静默覆盖的坑）。
- 只读展示：`GET /api/v1/settings` 的 `performance__ 块（`has_api_key` 只回布尔）。

### 3.1 structured output 优先，prompt 兜底

1. **json_schema strict**（`response_format={"type":"json_schema","json_schema":{"name":"performance_plan","strict":true,"schema":…}}`）——
   形状由 provider 在解码层保证；system 只交代角色。
2. 模型不支持（上游回 **4xx**）→ [StructuredMode::Auto] 原地降级：不带 `response_format`、
   system 写死「只输出 JSON」，再发一次；响应可剥 ```json``` 围栏（`strip_code_fence`）。
3. **prompt 路的输出仍必须过同一个校验器**——宽容的是围栏，不是契约。
4. 传输失败 / 超时 / 5xx **不重试**（那不是「模型不支持 structured」）。
5. **不采用** optional multi-tool + `tool_choice=auto`（那是赌模型愿不愿意调工具，缺席时整轮没有表演）。
6. **思考不进请求**：请求体只有 system + user 两条消息，没有 `reasoning_content` 字段，也不回灌上游思考
   （回归 `openai_client_posts_json_schema_and_parses_content` 抓原始请求文本）。

## 4. 接线（主链时序）

```text
run_turn:
  LLM 流式 ──► 只累积 assistant_text（表演层开着时**不切句、不入队**；TextDelta 仍透传给 UI 的链路耗时埋点与控制台回显）
  LLM 正常结束
    └─► performance.resolve(用户输入, assistant_text)      # 非流式一次调用，可取消
          ├─ 成功 JSON ─► speak 切句（**既有 SentenceAssembler**）→ 逐句 clean_for_tts → SentenceReady + TtsJob
          │               cues ─► EngineEvent::ActionCue（**先于**音频）→ WS action_cue
          └─ 失败 ─────► speak=clean_for_tts(原文) 切句 → 同上；cues=规则（host 注入）
  终态：Completed（noop 轮也是 Completed，只是没有音频与上屏）
```

- **上屏 = speak**：前端气泡文字来自 `SentenceVoiced`，其文本与送 TTS 的字符串**逐字相同**（沿用 2026-09-10 的「一句一单元」契约）。
  主模型的原始流**不上屏**（`EngineEvent::TextDelta` 在 supervisor 只做控制台回显）。
- **cues 锚 first_chunk**：前端在音频开始播放时按 `sentence_seq` 应用预设（既有 `ActionCueEvent` 消费路径，一行未改）。
- **历史回灌仍是主模型原文**（`TurnReport.assistant_text`）：表演层的整理不进模型上下文。
- **超时/失败的开声延迟**：本轮 TTS 起点从「流式第一句」变成「等表演层返回」；用户已接受这份双 API 延迟。

## 5. 回退与原因码

| 触发 | reason 码 | 行为 |
| --- | --- | --- |
| `enabled=false`（关闸） | `performance_disabled` | 引擎**不装配**表演层，走既有流式路径（不是「回退」，是没开） |
| 开了但缺端点/模型 | `performance_disabled` | 运行时存在但每轮回退：speak=clean_for_tts(原文) + 规则 cue |
| 超时 / 连接失败 / 非 2xx / 响应无正文 | `performance_request_failed` | 同上 |
| 坏 JSON / 缺字段 / 未知 preset / 校验失败 | `performance_plan_invalid`（或更细的 `performance_plan_*`） | 同上 |
| 主模型本轮没有正文 | `performance_empty_assistant` | noop（无可表演内容） |

- 规则 cue 的**单一真源** = director 的 `presets::rule_cues_for_text`（词表打分 + 缺省映射表），
  由 host 以闭包注入 runtime；runtime **不**再抄一份映射表。
- **干净文本 + 规则导演 = 仅失败回退**：当前实现里它们只在表演层关 / 失败时兜底（不是主路由）。

## 6. 观测面

- **日志**（`target="performance"`）：
  - 成功一份 **JSON 摘要**：`structured=<bool> performance plan ok：speak_chars=N, cues=["nod", …]`
    ——**不打正文全文**、绝无密钥；
  - 回退：`code=<reason> …（静默回退规则层）`；校验失败再带 `code=performance_plan_*`。
- **状态面**：`GET /api/v1/app/status` 的 `performance__ 块 = 配置（enabled/wired/base_url/model/has_api_key/mode/timeout_ms）
  + 运行计数（plans / fallbacks / noops / speak_turns / cue_turns / last_fallback / last_structured）。
- **引擎事件**：`EngineEvent::ActionCue`（`ev_epoch` / `ev_ts_ms` 已覆盖）。

## 7. 与 director Mod staging 的并存关系（**当前实现事实**）

**一句话**：产品 = 「**酒馆（类酒馆角色扮演内核）+ Live2D 皮套壳子**」，各功能由现有 mod
矩阵承担；**导演是一个 AI**（不是给用户操控皮套的辅助）。主链 `[performance]` 与 Mod
`staging_*` 是**两个可选提供者，都默认关、职责重叠**；**谁的 `speak` 能力该保留未定**
（[RESEARCH-actions-director-audit-2026-09-21.md](../plans/RESEARCH-actions-director-audit-2026-09-21.md) §3.7 Q1），
本节**不裁决**。当前实现事实：两者同开时 host 只把 `SentenceReady` 转给表演层一侧
（`forward_sentence_ready_to_mods = !engine.performance_enabled()`），不会双投递。

| 提供者 | 当前产出 | 默认 | 说明 |
| --- | --- | --- | --- |
| **主链 `[performance]`**（runtime 主链） | `speak`（本轮 TTS / 上屏真源）+ `cues` → `action_cue` | `enabled=false` | 开着时 `speak` 是 TTS/上屏真源 |
| **director `staging_*`**（P1-3/P1-4，Mod） | 只有按句 `cues` → `action_cue`（不改送 TTS 的文本） | `staging_enabled=false` | 规则层常驻；二路默认关 |
| **规则导演 `presets::rule_cues_for_text`** | 本地纯函数，给一条规则 cue | 常驻 | 表演层关/失败时的回退来源 |

**当前实现事实**：两者都默认关、职责重叠；**谁的 `speak` 能力该保留未定**（RESEARCH
§3.7 Q1），本节不推荐「只开哪一个」。无论 `staging_enabled` 开不开，规则层都是 host
注入的本地纯函数。界面侧一致：LLM 分区写「主模型不负责表演；表演层每轮 JSON」
+ 三态说明（只说 / 只动 / noop）；director 面板对两者关系另有一句兼容说明。

### 7.1 细节对照（实现面）

| 能力 | 表演层（`[performance]`，本波） | director Mod `staging_*`（P1-3/P1-4） |
| --- | --- | --- |
| 位置 | runtime（主链内，TTS 同层） | Mod（worker 线程，经 `ModServices.cues`） |
| 输出去向 | **speak = TTS/上屏真源** + cues → action_cue | 只有 cues → action_cue |
| 默认 | 关 | 关 |
| 结构化 | json_schema strict 优先 + prompt 兜底 | 提示词 + 宽解析 |
| 状态 | 可选提供者（默认关；开着时 `speak` 为 TTS/上屏真源） | 可选提供者（默认关）；只出 `cues`，不改写送 TTS 的文本 |

**同轮只有一个 cue 产者**：表演层开着时 supervisor 不把 `SentenceReady` 转给 Mod
（`handle_engine_event` 的 `forward_sentence_ready_to_mods` = `!engine.performance_enabled()`）。
否则两份 `action_cue` 会在前端互相**整份覆盖**（前端按帧整表替换）。

## 8. 收工证据（每态一条回归）

| 态 | 回归 | 位置 |
| --- | --- | --- |
| **成功·只说** | `speak_is_the_tts_source_and_cues_reach_action_cue`（TTS 输入 == speak 切句；cues 进 ActionCue） | `crates/live2d-ai-runtime/tests/conversation_engine_performance.rs` |
| **成功·noop** | `noop_plan_sends_no_tts_and_no_text`（零 TTS、零上屏，仍 Completed；空计划照发） | 同上 |
| **成功·只动** | `cue_only_plan_emits_a_silent_anchor_sentence`（无声锚句 + `first_chunk` 边界帧 + cue） | 同上 |
| **失败回退** | `invalid_plan_falls_back_to_cleaned_raw_and_rule_cue`（speak=清洗(原文)、cues=规则、照常出声） | 同上 |
| **主模型审计** | `performance_never_leaks_into_the_main_model_request` | 同上 |
| 契约 / 表驱动 | `legal_samples_parse_with_clamping_and_unknown_fields_dropped` / `illegal_samples_fail_as_a_whole` / `schema_and_validator_share_the_same_bounds` | `crates/live2d-ai-runtime/src/performance/tests.rs` |
| 客户端 wire | `openai_client_posts_json_schema_and_parses_content` / `auto_mode_degrades_to_prompt_on_4xx` / `server_errors_and_blank_content_fall_back_to_none` | 同上 |
| 引擎三态 resolve | `resolve_success_uses_the_plan_not_the_rule` / `resolve_three_states_noop_speak_only_cue_only` / `resolve_fallback_paths_use_clean_text_and_rule_cues` | 同上 |
| 装配三态 | `assemble_off_is_none_and_missing_endpoint_is_degraded`（runtime）/ `disabled_is_none_and_partial_config_is_degraded_and_wired_is_openai`（desktop） | tests.rs / `supervisor.rs` 的 `tests_performance_assembly` |
| WS 帧复用 | `performance_action_cue_projects_to_the_same_action_cue_frame` | `crates/live2d-ai-desktop/src/web_api/ws/events.rs` |
| 规则回退纯函数 | `rule_cues_for_text_is_deterministic_and_uses_the_default_table` | `crates/live2d-ai-mod-director/src/presets.rs` |

## 9. 已知缺口（如实标注）

1. **只动依赖前端的空句播放**：cue 的锚点是「该句音频开始」；无声锚句的 PCM 为空（0 样本 → 44 字节 WAV 头）。
   若某前端把空句整段丢弃，则只动的 cue 不会触发——引擎侧事实（ActionCue + `first_chunk` 边界帧）已正确发出。
2. **本轮开声等待表演层**：表演层超时（缺省 4s）会把首声推后；用户已接受双 API 延迟，但**不要把 timeout_ms 调大**当作调优。
3. **热重载重置计数**：`performance` 计数挂在当前运行时上，`PATCH /api/v1/settings` 触发的重建会清零（配置与计数一起换）。
4. **真端点只到 mock + loopback 回归**：本波没有可用的表演层真端点做肉眼验收（structured 降级路径由 loopback 4xx 回归证明）。
5. **`api_key_env` 缺值不算 degraded**：只在日志/状态面提示「请求不带 Authorization」；上游会回 401 → 那才算 request_failed。
6. **StructuredMode::Auto 的 4xx 判定是启发式**：只看响应体是否为 OpenAI 风格错误（含 error + type/param/invalid/unsupported/unknown）；
   误判代价是多重试一次 prompt 路（然后仍按失败回退）。

## 10. 门禁（本机实测，见 HANDOFF）

- `cargo test --workspace --all-targets` / `cargo test --doc --workspace` / `cargo fmt --all -- --check` /
  `cargo clippy --workspace --all-targets -- -D warnings` / `cargo run -p xtask -- rust-ratio`
- `flutter analyze` + `flutter test`（改了 `llm_section.dart`）
