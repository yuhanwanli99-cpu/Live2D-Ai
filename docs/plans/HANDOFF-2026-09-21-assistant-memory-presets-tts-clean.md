# HANDOFF 2026-09-21 —— 摘要含助手侧（扩事件）+ 多预设动作 + 主链 TTS 确定性清洗

> 工作树：`/home/skystar/Live2D-Ai-l1`。**不 bump / 不 push / 不打 tag。**
> 范围真源：本波任务书三件——①记忆摘要含助手侧（扩 Mod 事件）；②多预设动作
> （扩 `assets/actions/presets.json` + 规则/二路能选到）；③主链 TTS 确定性清洗
> （非导演改写）。
> **未做**（任务书禁止）：motion3 / exp3、让导演改写说辞、复活 Action 工具、
> 挂回 wallpaper|pet、重做 staging 客户端（异步导演 HTTP 上一波已接线）。

---

## 1. 摘要含助手侧（扩事件）

### 1.1 新事件名与 payload（**唯一入口**）

| 项 | 值 |
| --- | --- |
| 主题 | `ModEventTopic::AssistantReplied`（稳定 id **`assistant_replied`**） |
| payload | JSON `{"turn":<turn id>,"role":"assistant","text":"<清洗后正文>","interrupted":<bool>}` |
| 发点 | supervisor 空闲态、`turn::run_one_turn` 返回之后、**`TurnEnded` 之前** |
| 会话 | 与同轮 `TurnPrompt` **同一个 session**（host 第三个参数） |
| 投递 | **非阻塞**（`try_send`；host 只对 `TurnPrompt` 走 `dispatch_event_and_flush`） |
| 文本 | 引擎整轮正文经 `live2d_ai_runtime::clean_for_tts` 清洗后的**上屏口径** |

为什么新开主题而不是复用：`TurnPrompt` 只有用户侧正文；
`SentenceReady` 是按句锚点（一轮多条、语义是「即将送 TTS」）。改 `TurnEnded`
的 payload 会破坏导演「payload = turn id」的既有契约（回归
`tests_staging::epoch_zero_is_accepted_and_turn_prompt_resets_the_arbiter` 等），
所以扩的是**一条新主题**；`topics.rs::ALL` 与 `as_str` 同步，前端 Mod 发现面可见。

`TurnClose`（`supervisor/turn.rs`）是生产侧载体：`run_one_turn` 现在返回
`{assistant_text, interrupted}`（原来返回的 bool 无人消费）。

### 1.2 助手如何入桶

- **按 conversation 分桶**：有 session → `sessions/<id>.memory.jsonl`；无 session →
  老路径 `memory.jsonl`（只记，与用户侧完全对称）；
- 落盘多一个 `"role":"assistant"` 字段；**老行没有该字段 → 解析回 `user`**
  （升级前全是用户输入，老 id 派生公式不变）；
- 同轮用户记录与助手记录**共用同一个 `turn`**（轮末 `turn_seq` 尚未递增）——
  这是摘要「保留最近 K 轮」按 turn 分组的前提；
- id：user `<ts>-<turn>-<hash8>`；assistant `<ts>-<turn>-a<hash8>`（不撞 id，
  否则面板 delete 会误删两条）；
- **失败只 warn**：坏 payload / 空正文 / 非法会话 id / 写盘失败都只 warn + 跳过，
  不 panic、不阻塞主链；
- `state_json.writes` 拆出 `user_writes` / `assistant_writes`。

### 1.3 摘要 build_request（用户 + 助手交错）

- 待压窗口 = `[covers_upto, kept_start)`，记录按落盘顺序（user / assistant 交错）；
- **仍保留最近 K=4 轮原文**——但「轮」现在按 `turn` 分组（`recent_turn_window_start`）：
  4 条记录不再等于 4 轮，同轮的 user + assistant 一起留；
- 每条用 `MemoryRecord::line()`（`[用户] ` / `[助手] ` 前缀）进 user 消息；
  `SUMMARY_SYSTEM` 明确要求「归属不要搞反」；
- 触发判据（桶内原文总字符 / 注入预算 > `summary_ratio`）与冷却不变；
  **无 conversation 仍不摘要、不注入、不写全局 persona**。

### 1.4 回归

- `live2d-ai-mod-memory/src/assistant_tests.rs`（**新增 6 条**）：一轮后桶内
  user + assistant 两行且 role/turn 正确、角色前缀进注入、助手事实可检索、无会话
  只记不注入、坏 payload 只 warn、非法 id 回落老桶、user/assistant 不撞 id；
- `summary_tests.rs`（**+3**）：`build_request` 交错 + 角色前缀 + 最近 K 轮、
  `recent_turn_window_start` 按轮分组、端到端 `summary_covers_include_assistant_records`
  （covers 数到助手位点、摘要输入含 `[助手]`、被覆盖的助手原文不再进注入）；
- `supervisor/tests_assistant_event.rs`（**新增**）：轮末投递、带 session、
  payload 清洗后、`AssistantReplied` 在 `TurnEnded` 之前；
- `live2d-ai-mod-system/src/topics.rs`：`assistant_replied_is_listed_and_stable`；
- 回滚语义不变（既有 `rollback_removes_the_summary_from_injection_and_restores_originals` 仍绿）。

---

## 2. 多预设动作（+10）

### 2.1 新增 preset id 列表

| id | 通道 | 幅值（头 / 身） | 波形 |
| --- | --- | --- | --- |
| `bow_slight` | motion | AngleY -14 / BodyAngleY -6 | single |
| `tilt_left` | motion | AngleZ 18, AngleX 4 / BodyAngleZ 4 | single |
| `tilt_right` | motion | AngleZ -18, AngleX -4 / BodyAngleZ -4 | single |
| `agree_nod_double` | motion | AngleY -24 / BodyAngleY -9 | single（竖向不 oscillate） |
| `deny_shake_strong` | motion | AngleX 27, AngleZ 6 / BodyAngleX 9.5, BodyAngleZ 2 | **oscillate 2.5** |
| `shy_look_down` | motion + face | AngleY -18, AngleX -8 / BodyAngleY -6；眼 0.7 / 嘴 -0.2 | single |
| `happy_bounce` | motion + face | AngleY 12 / BodyAngleY 5；嘴 1.0 / 眼笑 0.9 / 眉 0.5 | single |
| `look_up` | motion | AngleY 20 / BodyAngleY 6；眉 0.4 | single |
| `ponder_tilt` | motion + face | AngleZ 14, AngleX 10, AngleY -6 / BodyAngleZ 3, BodyAngleX 4；眉 -0.3 / 眼 0.8 | single |
| `surprised_recoil` | motion + face | AngleY 14, AngleX -6 / BodyAngleY 7, BodyAngleX -3；眼 1.3 / 眉 0.8 / 嘴 0.2 | single |

- 只动 面部 / 头 / 半身角度 通道（`ALLOWED_PARAMS`），**无手臂 / 无特效**；
- 幅值守红线：头 ≤30、身 ≤10（外置表越界会被钳位 + warn，回归按分档断言）；
- `oscillate` **只用于左右类**（`deny_shake_strong`）；竖向点头类一律
  `single`——所以 `agree_nod_double` 是一次**加深**的点头（id 保留任务书
  建议名，语义在 JSON 与本文写清）。

### 2.2 三处能选到新 id（单一真源）

`director::presets::PRESET_IDS` 一个常量同时决定：
① 规则映射表 allowlist（`PresetTable::from_value` 接受新 id）；
② 面板 Select 选项（`preset_field` + `preset_label` 中文标签）；
③ **二路 LLM 的能力集**（`staging::build_user_prompt` 直接用它拼 user 提示，
**不再写死旧 8 个**；`plan::parse_plan` 同源做第二道闸）。

调试面板：`shell/flutter/lib/settings/sections/dev_tools_section.dart` 的
`kDebugPresets` 从 8 条扩到 **18 条**（老 8 + 新 10），「全部扫一遍」逐条覆盖。

**出厂默认映射未改**：`PresetTable::default()` 仍是老 6 条非 none（那套「不做 > 做错」
的取舍写在 presets.rs 头注）——新 id 是**可配置**能力，是否把某档改成新预设由
Op/用户决定，不在本波擅自改产品行为。

### 2.3 内建 fallback id 集合策略（写清）

- **外置 `assets/actions/presets.json` = 完整集合**（18 条，产品实际能力）；
- **内建 `preset.rs::PRESETS` = fallback 子集**（老 8 条）；
- 断言是 **builtin ⊆ external**（不再相等）：回归
  `shipped_presets_json_parses_clean_and_is_a_superset_of_builtin_ids`；
- JSON 缺失 / 解析失败 / 一条都不合法 → 退回内建；新 id 在运行时 `resolve` 时
  **静默 `Ignore`**（未知 id 的既有语义），绝不让渲染面加载失败。

### 2.4 回归

- `l2d-wasm-demo::preset::tests::shipped_presets_json_parses_clean_and_is_a_superset_of_builtin_ids`
  （JSON 全绿、无告警、builtin ⊆ external、10 个新 id 都在、通道 / 幅值分档、
  竖向类不得 oscillate）；`json_drops_unknown_channels_and_clamps_amplitudes` 仍绿（未知通道仍丢弃）；
- `live2d-ai-mod-director::presets::tests::new_preset_ids_are_selectable_by_the_rule_table`
  （新 id 可配、可解析、进选项；未知 id 回落缺省；`PRESET_IDS.len() >= 19`）；
- Flutter `developer_section_test.dart` 仍绿（点 `nod` 下发不变）。

---

## 3. 主链 TTS 确定性清洗（非导演改写）

### 3.1 纯函数与调用点

- 函数：`live2d-ai-runtime/src/dialogue/clean.rs::clean_for_tts`——纯函数、
  幂等、无 IO / 无随机；剥成对全 / 半角括号动作描写、成对 `*…*` 舞台指示、
  多余 Markdown（粗体标记 / 反引号 / 标题 / 引用 / 链接），**只拆标记**；
- 调用点：`conversation/engine.rs`，**切句之后、入队之前**每句洗一次；
  失败轮的 `TextFallback` 整段洗一次；
- **不重排句边界**：函数不增不减 `。！？….!?` / 换行（所以必须在切句之后）。

### 3.2 上屏 / 送 TTS 策略（写死）

**两者同清洗**：`SentenceReady.text == TtsJob.text == SentenceVoiced.text`，
一份产物三处消费（TTS / 上屏 / 记忆侧助手正文）。回灌 LLM 的历史
（`commit_completed_turn`）用**原文**。清洗后为空串 → 走既有**静音句**路径
（worker 见 `trim()` 为空即**不发 TTS HTTP**）——空串永远到不了上游。

### 3.3 清洗前后样例（各 2 条）

| 送 TTS 前（LLM 原文） | 清洗后（送 TTS / 上屏） |
| --- | --- |
| `你好呀（挥手）。*歪头* 再见。` | `你好呀。再见。` |
| `**重点**在第三点，看 \`config\` 这段。` | `重点在第三点，看 config 这段。` |

（另有表驱动用例覆盖：不成对括号原样保留、`3*4=12` 不动、ASCII 句读后的空格
不丢、整段清洗 == 逐句清洗再拼接。）

### 3.4 导演仍不改说辞（一句话证明）

**导演订阅的 `SentenceReady` 只读清洗产物、其下行面只有 `latest.preset_id` 状态字段
（`channel="preset"`），从不 `apply_settings`、从不 `action_tx`、也没有任何写 TTS 文本的通道
——回归 `director_never_writes_config_or_actions` 与
`action_tx_and_apply_settings_are_never_called` 守着；`staging_enabled=false`（导演
`Disabled`）时清洗仍在引擎里照常生效（清洗在 runtime，不在导演）。**

### 3.5 回归

- `dialogue::clean::tests`（表驱动 / 句读不变 / 幂等 / 落单标记保留，4 条）；
- `conversation_engine_tts_flow::tts_and_screen_share_the_deterministically_cleaned_sentence`
  （TTS 输入已清洗、`SentenceVoiced` 与 `SentenceReady` 逐字相同、`assistant_text` 仍是原文）；
- `supervisor::tests_assistant_event`（交给 Mod 的助手正文同为清洗产物）；
- 既有 `whitespace_only_sentence_never_reaches_tts_and_turn_completes` 仍绿
  （空串走静音句路径、不发 TTS 请求、不把整轮判失败）。

---

## 4. 门禁（本机实测）

| 检查 | 结果 |
| --- | --- |
| `cargo test --workspace --all-targets` | **1333 passed / 0 failed** |
| `cargo test --doc --workspace` | 3 passed / 0 failed |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0 warning** |
| `cargo run -p xtask -- rust-ratio` | 见 §4.1 |
| `flutter analyze` | 见 §4.1 |
| `flutter test` | 见 §4.1 |
| `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` | 见 §4.1 |

### 4.1 Flutter / 产物

| 检查 | 结果 |
| --- | --- |
| `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` | ok（wasm Rust 一行未改，按门禁仍跑） |
| `flutter analyze` | `No issues found!` |
| `flutter test` | **1001 passed**（本波未加 Dart 测试） |
| `flutter build web --release --base-href /app/ --no-web-resources-cdn` | ✓ Built build/web；`main.dart.js` **0** 处 `gstatic.com/flutter-canvaskit`（断网红线 ok） |

---

## 5. 改了哪些文件

**Rust**

- `crates/live2d-ai-runtime/src/dialogue/clean.rs`（**新增**：清洗纯函数 + 4 条单测）
- `crates/live2d-ai-runtime/src/dialogue/mod.rs`（导出 `clean_for_tts`）
- `crates/live2d-ai-runtime/src/conversation/engine.rs`（切句后清洗；兜底清洗）
- `crates/live2d-ai-runtime/src/conversation/mod.rs`（`TurnReport` / 事件契约文档）
- `crates/live2d-ai-runtime/src/lib.rs`（re-export）
- `crates/live2d-ai-runtime/tests/conversation_engine_tts_flow.rs`（**+1** 清洗集成回归）
- `crates/live2d-ai-mod-system/src/topics.rs`（**新主题** `AssistantReplied` + 测试）
- `crates/live2d-ai-desktop/src/supervisor/turn.rs`（`TurnClose` + 返回助手正文）
- `crates/live2d-ai-desktop/src/supervisor.rs`（轮末投递 `AssistantReplied`）
- `crates/live2d-ai-desktop/src/supervisor/tests_assistant_event.rs`（**新增** 回归）
- `crates/live2d-ai-mod-memory/src/strategy.rs`（`MemoryRole` / `role` / `line()` / `recent_turn_window_start`）
- `crates/live2d-ai-mod-memory/src/store.rs`（role 落盘 / 解析）
- `crates/live2d-ai-mod-memory/src/summary.rs`（角色前缀 + 按轮分组 + 提示词）
- `crates/live2d-ai-mod-memory/src/assistant.rs`（**新增**：助手侧入桶 / 事件解析 / 唯一写入实现——从 `lib.rs` 拆出以守住 ≤1000 行豁免）
- `crates/live2d-ai-mod-memory/src/summary_flow.rs` / `lib.rs`（订阅 / 计数 / 注入接线）
- `crates/live2d-ai-mod-memory/src/commands.rs`（`record_json` 带 role；导入 = user）
- `crates/live2d-ai-mod-memory/src/assistant_tests.rs`（**新增** 6 条）
- `crates/live2d-ai-mod-memory/src/{tests,commands_tests,quality_tests,summary_tests}.rs`
- `crates/live2d-ai-mod-director/src/presets.rs`（`PRESET_IDS` + 测试）
- `crates/live2d-ai-mod-director/src/lib.rs`（`preset_label`）
- `crates/l2d-wasm-demo/src/preset.rs`（内建 vs 外置策略 / 红线头注）
- `crates/l2d-wasm-demo/src/preset_tests.rs`（superset 回归）
- `assets/actions/presets.json`（**+10 预设**，version 2）

**Flutter**

- `shell/flutter/lib/settings/sections/dev_tools_section.dart`（`kDebugPresets` 18 条 + 文案）

**文档**

- `docs/architecture/memory-mod-v0.md`（状态头 / §1 / §15.2 / **§16 助手侧记忆**）
- `docs/architecture/director-mod-v0.md`（§9 预设 id 集合 + 能力集单一真源）
- `docs/architecture/core-chain-baseline.md`（**§2.2 确定性清洗契约**）
- `docs/plans/HANDOFF-2026-09-21-assistant-memory-presets-tts-clean.md`（本文）

---

## 6. 诚实标注

1. **真机肉眼验收待做**：多预设动作的实际观感、`AssistantReplied` 在真实 LLM 下
   对摘要的影响，本机只给到「请求体原文 + 单测」；
2. **Flutter 产物需重建**：本波改了 Dart（`kDebugPresets`），要在 18080 界面上手验
   必须先 `flutter build web --release --base-href /app/ --no-web-resources-cdn`；
3. **wasm 无 Rust 改动**：新增预设只改 `assets/actions/presets.json`（运行时经
   `/actions/presets.json` 读取），渲染面一行未改——按任务书仍会跑
   `cargo check --target wasm32-unknown-unknown`；
4. **`agree_nod_double` 不是竖向下上多周期**：红线「`oscillate` 仅左右类」优先，
   实现为一次加深点头；id 保留任务书建议名，语义已在 §2.1 与 JSON `_doc` 写清；
5. **摘要仍可能只压到用户话**：若某轮助手清洗后为空（整句都是动作描写）或助手侧
   写盘失败，该轮就只有用户侧记录——这是如实投递，不补写；
6. **`/stop` 中断轮也投递**（`interrupted=true`）：正文可能只说了一半，memory
   照记；消费方若要跳过可自行读该字段。
