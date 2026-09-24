# 交接：Core-Next 三件套（2026-09-15）

> 范围真源：外部任务书（本轮三件必做）+ 用户补充**覆盖条款**（响应速度整段作废；
> 思考开关是**产品开关**不是性能优化；记忆验收要抓本轮请求体；导演预设可测优先）。
> 分支：`mod/l1-product`；**版本号未 bump**（仍 `0.2.0-rc.3`）、未 push、未打 tag。

## 1. 做了什么

### 必做 1 — 记忆**同轮生效** + 会话绑定（时序修复）

- **根因**：`TurnPrompt` 投递排在 `set_system_prompt_override` **之后**，且
  `dispatch_event` 只是 `try_send`（worker 是另一个线程）——Mod 写表与 supervisor
  读表是竞态，注入实际落到下一轮。
- **修法**：
  1. `mod_registry`：`EventMsg` 增加**可选回执**；新增
     `dispatch_event_and_flush`（入队后等 worker 处理完，1s 上限兜底）与
     **唯一的 sink 工厂** `mod_event_sink`（`TurnPrompt` 同步、其余话题仍非阻塞）；
  2. `supervisor`：**先投递 `TurnPrompt`（同步）→ 再决议本轮 system_prompt**；
  3. 启动路径（`cli_entry`）与动态装配路径（`supervisor_slot`）共用同一个 sink，
     两条路径不再分叉。
- **会话绑定**：有会话 → 记忆桶 `sessions/<id>.memory.jsonl` + 写该会话的 MEMORY
  owner 槽（与 persona 槽叠加）；无会话 → 旧全局路径（`persona.system_prompt`，
  面板与文档标明是降级）。
- **回归**（`crates/live2d-ai-desktop/src/supervisor/tests_loop.rs`）：
  `memory_written_on_turn_prompt_lands_in_the_same_turn_request` —— 一次请求体里
  必须同时看到「A 会话刚写的 MEMORY 块」与「本轮 TurnPrompt 的正文参与检索」；
  B 会话不得带 A 的记忆（无注入 → 回落全局 `persona.system_prompt`）。
  **实测会红**：把顺序改回旧版即失败（`实际 system="人设"`）。
  `mod_registry::tests::dispatch_event_and_flush_means_processed_not_queued` 用
  50 条积压断言「回执 = 已处理完」。

### 必做 2 — LLM 思考可选开关 `llm.show_reasoning`（**默认 false**）

- 配置：`[llm].show_reasoning`（缺省省略 = 不展示）、三态 PATCH、`GET /settings` 回
  **生效值**（`LlmView.show_reasoning`）。
- 闸门在 **WS 投影层**（`web_api::ws::should_broadcast`）：`false` → `reasoning_delta`
  **不下发**（前端看不到思考区）；正文 / 音频 / 状态 / **错误**帧一律不受影响。
  解析层一行未动（思考仍单列 `LlmEvent::ReasoningDelta`、仍不进句子装配器、仍不进 TTS）。
- 读源是设置快照，PATCH 与 `file_watcher` 都会刷新 → **热生效**，不必重启。
- UI：设置 → 对话模型 → 「展示思考（reasoning）」开关。
- **这是产品开关**（要不要看内心独白），**不是**性能开关：思考与正文共用 `max_tokens`，
  关掉它一个 token 都不省。收工摘要与文案都不写「为了加速」。
- 未接的部分（文档写明）：部分 reasoner 的「强制思考」extra body 没接——当前没有
  这样的配置位；本波只做**展示闸**。
- 回归：`settings::settings_tests::show_reasoning_*`（3 条：默认 false / 三态补丁 /
  toml round-trip）、`web_api::tests_ws::reasoning_frame_is_gated_by_show_reasoning`
  （关=无帧、开=有帧、正文与错误不受影响）、Flutter `settings_api_test.dart` 2 条。

### 必做 3 — 导演**动作预设**（可测，范围严格）

- **映射**：`emotion|intent → preset_id`，内置 **6 条非 none** 默认：

  | 触发 | 预设 | 通道 |
  | --- | --- | --- |
  | happy / affectionate | `expr_smile` | 表情 |
  | sad | `expr_sad` | 表情 |
  | angry | `expr_angry` | 表情 |
  | surprised | `expr_surprised` | 表情 |
  | greeting | `nod` | 短动作 |
  | anxious / farewell / neutral 闲聊 | `none`（不做 > 做错） | — |

  可选 id：`none` / `expr_smile` / `expr_sad` / `expr_angry` / `expr_surprised` /
  `nod` / `shake` / `look_left` / `look_right`（面板 8 个 Select，`preset_<档位>`）。
  `intent` 非 none 时优先于 emotion（问候选动作、其余看表情）。
- **状态面**：`state_json.latest.preset_id` + `preset_id`（每条决策）+ `presets_chosen`
  + `presets`（生效表）；`channel` 由 `"none"` 改为 `"preset"`，`delivered` = 最近一轮
  是否选出了预设。**host 下行通道仍然零调用**（`action_tx` / `apply_settings`）。
- **投递**：Mod **不推**（它没有下行通道）→ 前端在正文开始上屏（`text_delta`，TTS
  故障时是 `text_fallback`）时拉一次**已有的只读状态面**
  `GET /api/v1/mods/director/state` → 新 preset 就 `live2d_bridge` 发 `preset` 消息 →
  渲染面按表写参数（表情保持 2.6s / 短动作 0.9s 正弦包络，到点整批撤销）。
- **渲染面**：`l2d-wasm-demo/src/preset.rs`（原生可测：表 + 包络 + 时长）、HUD 新增
  `preset: <id>`、未知 id **静默降级**。红线机器可检：预设只允许
  `ParamMouthForm/Eye*/Brow*/AngleX|Y|Z`（回归 `presets_stay_inside_the_allowed_channel`）。
- **不做**（逐条守住）：手臂 / 手指 / 特效 / 第二路 LLM / 改 TTS pitch·speed 实写 /
  复活旧 Action 工具调用；`AVAILABLE_MOD_FACTORIES` 数量未动；wallpaper/pet 未挂回。

## 2. 点火步骤（3 步）

1. `./scripts/ignite.sh`（渲染面 `dist/` 与 Flutter `build/web` 已在本轮重建），
   浏览器开 `http://127.0.0.1:18080/app/`。
2. **设置 → Mod → 导演 → 启用**（memory 已启用）；回聊天说一句带情绪的话
   （例：`说爱我` / `你好呀` / `气死我了`）。
3. 看两处：角色出现对应**表情或短点头**，且导演面板「本轮预设」显示
   `表情（expr_smile）` 之类的稳定码；把舞台 HUD 打开（`/render?hud=1`）还能看到
   `preset: expr_smile`。停用导演再说一句 → 不再触发。

## 3. 记忆同轮怎么验收（两种抓法）

- **看得见**：设置 → Mod → memory → **导入一条带独特关键词**的记忆（例：
  `我的幸运色是钴蓝色`）→ 立刻发一句含该词的话（例：`我的幸运色是什么？`）→
  后端日志出现 `memory 已注入 1 条记忆到会话 … 的提示词（本轮请求体已带上）`，
  且**本轮回复**就用上了这条记忆。
- **抓请求体**（确定性）：把 `[llm].base_url` 指到一个本地回声 mock，它会打印每轮
  `messages[0].content`（= 有效 system_prompt）并回一句固定文本，然后照上面发消息。

## 4. 门禁（本轮实跑）

```text
cargo test --workspace --all-targets   # 全绿（EXIT=0）
cargo test --doc --workspace           # 3 passed
cargo fmt --all -- --check             # clean
cargo clippy --workspace --all-targets -- -D warnings   # 0 error / 0 warning
cargo run -p xtask -- rust-ratio       # 96.7141% PASS（门槛 95%）
cargo test -p l2d-wasm-demo            # 18 passed（含 preset 6 条）
cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo  # ok
cd crates/l2d-wasm-demo && trunk build # ✅ success（dist/ 已重建）
cd shell/flutter && flutter analyze    # No issues found
cd shell/flutter && flutter test       # 944 passed
cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn  # ✓
```

## 5. 已知缺口 / 本波**没做**的事

1. **TTS 上游 503 不在本波范围**（用户口径）：不修 TTS、不改「一句一单元」、不加延迟打点。
2. 没有新增面板式「抓请求体」工具——上面给的是日志 + mock 两种外部办法。
3. 预设是**短反应**，不是编排：一条决策只投一条预设，没有动作队列 / 混合 / 优先级。
4. 活服务肉眼验收（Windows 侧）仍需用户执行；本波只到 `cargo test` + `flutter test` +
   产物重建级别。

---

## 6. 点火反馈修复（2026-09-15 第二段，同分支 / 同版本）

用户点火反馈四条（A 导演崩、B 语音无主按钮、C 常态唤醒词、D 设置瘦身）。逐条落点：

### 6.1 A｜「启用导演时崩」的崩因

**一句话**：不是 Flutter / wasm 的逻辑缺陷，而是**本机磁盘 100% 满**（`df` 实测
只余 1.7 MB）——`dmesg` 直指 `rust-lld ... signal: 7`（SIGBUS）反复出现，
构建产物与写盘全链退化。清掉另一个 worktree（`/home/skystar/Live2D-Ai/target/debug`）
44 G 构建缓存后，同一套代码能编、能跑。

三处候选层逐一**实测排除**：

- **Flutter 面板**：无头 Chrome 里真实展开导演卡片、拉运行态、渲染面板——无异常；
- **bridge preset 拉取**：`GET /api/v1/mods/director/state` 200，前端在
  `text_delta` 时拉到 `latest.preset_id`；
- **wasm preset 通道**：原生（llvmpipe GPU）把 8 条预设逐条写进**真实 Bai 皮套**并跑帧
  循环 / 撤销（临时探针已删，结论：`set_parameter` 对不存在的参数返回 false，绝不 panic）。

**顺手修掉三个真正让导演「启用也不可用」的缺陷**：

1. **表单把缺省映射写成 `none`**——`ModSettingField::String/Select` 增加可选
   `default`，导演 3 个预设 Select 带上真缺省（`expr_smile` / `expr_sad` / `nod`）；
   之前面板显示「不投递 (none)」，点一次保存就把 6 条内置映射全关掉（表情/短动作**再也
   不触发**）。回归：`settings_spec_has_no_enabled_and_matches_static_spec`。
2. **前端去重不认「Mod 重启」**——`_lastDirectorSeq` 遇到 seq **变小**（账本复位）
   自动归零；并改成**一轮只拉一次**（轮末 `turn_state` 复位），不再每个
   `text_delta` 打一次本地 GET。
3. **面板文案过时**——删「本轮不做 L1 / 未接任何下行通道 / 未结项 / 建议语速」等，
   主面板只剩**本轮预设** + 情绪/意图一行注脚 + 清空；通用运行态块整块不渲染
   （`ModPanel.showRuntimeState`）。

**启动日志**已从「骨架：只记决策日志、不投递」改成反映 preset 现状
（"按情绪/意图选动作预设：expression|motion 经状态面 latest.preset_id 交给渲染面"）。

### 6.2 B + C｜语音主路径 + 默认唤醒词

- **默认唤醒词「小可爱」**：键**缺失** → `DEFAULT_WAKE_PHRASE`（总闸默认开）；
  键**存在但为空** → 总闸关（`403 voice_gate_closed`）；已有自定义**不覆盖**。
  真源 `live2d-ai-mod-voice-input/src/gate.rs`；前端 `kDefaultWakePhrase` 与之
  逐字一致（跨语言回归 `voice_wake_default_consistency_test.dart`）。
- **聊天主界面常驻「听」按钮**（`ui/chat_panel.dart` + `voice/`）：
  说「**小可爱 今天天气怎么样**」→ 命中唤醒词 → 原文 POST
  `/api/v1/voice/transcript` → 服务端剥词 → `say` → LLM → TTS → 口型。
  浏览器 Web Speech（桌面 Chrome/Edge）；不支持则按钮禁用并说明；静音 `onEnd`
  自动重启（常态监听）；「已唤醒窗口」8 秒防串台；失败在按钮旁可读。

### 6.3 D｜设置瘦身

- **导演**：settings_spec 10 字段 → **3 个常用预设 Select**（其余走缺省）；
- **语音**：settings_spec v3 主区只留 `wake_phrase` + `manual_enabled`，
  `backend`/`locale`/`token`/`sidecar_*` 由 `VoiceInputPanel.advancedKeys`
  收进「高级」折叠；面板主区只剩 一行状态 + 一句链路 + 检查配置，
  sidecar / 验证闸门 / 退出码长文全部进「高级」；
- **memory / persona / external-input**：删重复长说明，保留「一句话 + 必填控件 +
  运行态一行」。

### 6.4 点火验证（3 步）

1. `./scripts/ignite.sh`（**需重启**：Rust 侧有改动；前端产物已重建）；浏览器开
   `http://127.0.0.1:18080/app/`。
2. **语音**：聊天输入框左侧点「**听**」→ 对着麦克风说「**小可爱 今天天气怎么样**」→
   输入框上方出现「已发送：「今天天气怎么样」」→ 角色回复并开口。
   权限/设备失败时按钮旁直接给原因。
3. **导演**：设置 → Mod → 导演（启用）→ 回聊天说一句带情绪的话（例「气死我了」/
   「你好呀」）→ 角色出现表情或点头；打开 `/render?hud=1` 可见 `preset: expr_angry` /
   `preset: nod`；面板「本轮预设」显示同一稳定码。停用导演再说一句 → 不再触发。

**门禁（本轮实跑）**：cargo test workspace **全绿**（0 失败）/ doc 3 /
fmt clean / clippy 0 warning / rust-ratio **96.7215% PASS** /
`cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` ok /
`flutter analyze` No issues / `flutter test` **959 passed** /
`flutter build web --release --base-href /app/ --no-web-resources-cdn` ✓。
