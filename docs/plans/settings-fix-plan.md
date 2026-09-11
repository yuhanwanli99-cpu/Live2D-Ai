# 设置界面逻辑与调用链修复计划（PC + Android）

> 目标：修复「设置界面逻辑 + 相关调用逻辑链路」在 PC 设置中心与 Android 设置页中的已知缺陷。
> 范围：两端都要，按 P0–P3 全面修。
> 状态：**PC 端已按 2026-08-20 重构落地**（详见 `settings-redesign-spec.md` 与本文件 §2.1 各问题项，PC 侧 P0/P1/P2 主链已修复+验证）；Android 部分按下方 P0/P1(Android) 继续推进。
> 已落地（2026-08-20 PC 重构本轮）：/health、先校验后落盘+回滚、TTS 自动补齐、POST 回传 normalize config、
> 显式清空（null 删除）、log_level round-trip、端口生效、.env key 可见、apply 已连会话遍历或 requires_restart 诚实返回、
> 协议感知 check/test/discover、引擎 close/断线清理、init_vision 热换、proxy 挂起补发、UI 深色重构 + 惰性 discover/busy/占位/re-populate。

---

## 1. 现状调用链路

### 1.1 PC 设置中心
```
打开面板 open()
  ├─ GET /api/config          -> _get_common() 读 conf.yaml + shared/persona.yaml + .env
  ├─ GET /api/config/tree     -> 读原始 YAML（设定树）
  └─ populate()：
       ├─ 填充 LLM/TTS/ASR/口型/人设/系统表单
       ├─ 每打开逐个 autoDiscoverConfiguredModels()（串行调 /api/llm/discover-models）
       └─ renderModels()（GET /api/models）

保存 save(apply)
  └─ POST /api/config {collectPayload(), apply}
       ├─ update_config()：写 conf.yaml（llm/tts/asr/port/fps/character + 自动补齐）
       ├─ 写 .env（api_keys + log_level）
       ├─ 写 shared/persona.yaml（人设/提示词）
       ├─ AppConfig(**candidate) 校验（仅校验，不保证后续一致）
       └─ apply=true：AppConfig(**_config_dict_for_reload()) -> default_context_cache.load_from_config()

其它：/api/providers/check、/api/test-llm、/api/test-tts、/api/models(scan/select)、/api/config/tree
```

### 1.2 Android 设置页
```
SettingsScreen.kt
  ├─ 每个字段 remember { mutableStateOf(repo.xxx) }（一次性读 SharedPreferences）
  ├─ 部分字段：每个 onValueChange 即写 repo（qwen/zhipu/openai、model、renderQuality、asr、ttsProviderId…）
  ├─ 其余字段：仅在“保存”按钮集中写入（deepseekKey、temperature、ttsVoice、ttsSpeed、personaName、aliyun…）
  └─ 运行时：运行时常量在 App 启动时一次性构建（providers/渲染质量/模型），设置改动大多需重启才生效
```

---

## 2. 已定位问题

### 2.1 PC 端

**P0 级（会写坏配置 / 热重载失败 / 启动失败）**

- **P0-1 TTS 空配置破坏校验**
  - 前端 `collectPayload()` 仅当 configured/touched 才把 provider 放进 `tts_configs`（`renderer/src/settings-ui.ts:859-878`）；
  - 后端 `update_config()` 仅当 `has_any`（存在非空字段）才为新 pid 建条目（`routes.py:522-527`）；
  - voices/models 目录为空的 provider（`gpt_sovits_tts`、`piper_tts` 等）被选中但未填参数时，落成 `tts_cfg[pid]={}`（`routes.py:541-551`）；
  - 而 `EdgeTTSConfig.voice` 等是 `Field(..., ...)` 必填（`config_manager/tts.py`）→ `AppConfig(**candidate)` 校验失败，表现为“配置已保存但热重载失败”（`routes.py:659-667`）或下次启动失败。
  - 与已修复的 `b3fa1b9「conf.yaml 被空 TTS 配置破坏」` 同根因，属残留路径。

- **P0-2 热重载失败后 YAML 已落盘（不可回滚）**
  - `update_config()` 先 `_save_yaml(data)`（`routes.py:590`）再做热重载（`routes.py:651-667`）；
  - 校验/热重载失败时 YAML 已被写坏且不回滚，前端却只能提示“配置已保存但热重载失败”。

- **P0-3 设定树无 schema 校验、可写坏启动**
  - `/api/config/tree` 仅 `yaml.safe_load` 语法校验（`routes.py:733-734`），不跑 `AppConfig` 校验；结构非法但 YAML 合法时可保存 → 下次启动崩。

**P1 级（前后端状态漂移 / 一致性问题）**

- **P1-1 保存后 UI 与后端真实状态漂移**
  - `save()`（`settings-ui.ts:910-927`）POST 成功后只改文案，不重新 `populate()`；
  - 后端会自动补齐 default voice/voices（`routes.py:533-551`）、自动补选中 LLM 最小配置（`routes.py:498-514`）；
  - 前端 `currentConfig` 停留在旧值 → “我改了/保存了，界面却不对，重启又变了”。

- **P1-2 无法清空字段（保存语义缺陷）**
  - 人设/提示词/字段仅在 `not in (None, "")` 时写（`routes.py:644-648`、`708-709`）→ 想清空 `system_prompt/name/role/model` 被忽略；界面显示清空但旧值仍在。
  - `.env` 仅写非空 key（`routes.py:609-612`）→ 无法清除已保存的 API Key。

- **P1-3 测试按钮使用未保存的旧配置**
  - Keys 页“测试”只传 provider+api_key（`settings-ui.ts:558-564`）；`test_llm` 用 YAML **已保存**的 base_url/model（`routes.py:1001-1015`）→ 刚改未保存就测，结果误导。

- **P1-4 人设模型与 Provider 模型职责冲突**
  - `_apply_persona_to_data()`（`routes.py:258-296`）把 `persona.model` 覆盖进 `llm_configs[provider].model`，与 LLM 卡片手动改的 model 冲突，哪里改才生效不可预期。

**P2 级（响应 / 交互体验）**

- **P2-1 打开面板被模型发现拖慢/卡住**
  - `populate()` 末尾无条件 `autoDiscoverConfiguredModels()`（`settings-ui.ts:724-734`），对每个已配置且有 key 的 Provider 串行调 `/api/llm/discover-models`（timeout=15s，`routes.py:850`）→ Provider 一多就长时间卡“检查中”。

- **P2-2 TTS 检查结果误导**
  - `checkAll`（`settings-ui.ts:654-684`）对 TTS 仅按 key 是否存在判“已配置”，不真实合成；`routes.py:866-973` 的 TTS 分支同理，只在有 key 时返回 ok。

**P3 级（健壮性 / 可维护性）**

- **P3-1 前端类型错位**
  - `collectPayload`/`testLlm`/`testTts` 把 `<select>` 当 `HTMLInputElement` 取 `.value`（`settings-ui.ts:850、936、972`）——运行期恰好可用，但类型错位、易崩。
- **P3-2 `switchEdgeTts` 对未渲染 editor 失效**
  - `settings-ui.ts:1013-1023` 在对应 editor 不存在时不回填 voice，逻辑不一致。
- **P3-3 discover/test 竞态**
  - 无 AbortController/防抖，连续点击交错覆盖状态栏与模型下拉。
- **P3-4 设定树与表单两套读写 conf.yaml、无锁**
  - `/api/config` 与 `/api/config/tree` 各自读写同一文件，无互斥，可竞态覆盖。

**新增 PC 深挖结论（来自 `Live2D-Ai-pc/open-llm-vtuber/SETTINGS_SYSTEM_AUDIT.md`，主线已交叉核对）**

- **B3 High【调用链断链】** “保存并应用”只 `load_from_config` 共享 `default_context_cache`（`routes.py:651-657`），已连接会话仍持有旧 engine 引用（`websocket_handler.py:183-202` 每会话深拷贝 config 但共引用对象）→ 界面显示 `applied:true`，但已连客户端仍用旧引擎；且 `load_from_config` 在 `init_agent` **之后**才改 `system_config/character_config`（`service_context.py:306-308 vs :301-304`），新 agent 用旧 system_config/avatar 构建。→ PC 端等价于 Android 的“保存了但运行没生效”。
- **B4 Medium** API 写进 `.env` 的 key 运行进程看不到（`run_server.py:21` 只 `load_dotenv` 一次，`utils.py:44` 用过期 `os.getenv`）→ check/test 对新 key 成功，运行引擎仍旧 key。
- **B5/B6 Medium** `system_config.port` 是**死配置**（`run_server.py:145`、`_start_server.py:27` 硬编码 12393，`server.py:121-123` 却按配置 port 拼 `/proxy-ws` 地址）；`_start_server.py` 是分叉启动器（`safe_load` 无 `${VAR}` 替换、无人设注入、无 MCP）→ 设置改 port 不生效，用 `_start_server.py` 启动配置不对。
- **B7/B8 Medium** `handle_disconnect` 先 `pop` 再查（`websocket_handler.py:302-314` → context 恒为 None，`close()` 死代码），每断线泄漏 MCP/agent/vision；reload 替换 engine 不 `close()` 旧引擎、`init_vision` 有 handler 直接 return（`service_context.py:395-514`）→ 热更不可靠 + 资源泄漏。
- **A1 Medium** `log_level` **从不加载但总是提交**（`settings-ui.ts:779-783` 的 renderSystem 不写 `.settings-log-level`、`:906` 恒读）→ 每次“保存全部”把 `.env` 日志级别静默重置回 INFO。
- **A2/A6/A7 Medium** 加载失败仍可点保存（空下拉 + `port:Number('')===0` → 提交 `''`/`0`）；按钮无 busy/disable，双击可并发双 POST；`fillSelect` 无“未选择”占位 → 空 `llm_provider` 时浏览器自动选第一个并保存。
- **B10 Medium** 设定树仅 YAML 语法校验（`routes.py:733-736`）→ 合法 YAML 但结构崩可写盘，下次启动才炸（同 P0-3，补充证据）。
- **B12 Medium** 播放确认超时路径不发 `send_conversation_end_signal`（`conversation_utils.py:179-195`），proxy 会话卡住不再转发 text-input。
- **B9 Medium** test/discover/check 对**所有** provider 硬编码 OpenAI 形态的 `GET /models` + `POST /chat/completions`（`routes.py:846-1040`），运行层却用各厂家专有协议 → “检查通过”与实际 agent 不一致。
- **A11/A9/A8 Nit** `Audio.play()` 拒绝未处理 + 预览 URL 拼接脆（`settings-ui.ts:997-1002`）；`selectModel()` 不刷新列表致“默认”徽标残留（`:1043-1055`）；`switchEdgeTts()` 无对应项也报成功（`:1008-1024`）。
- **明确无问题**：GET/POST `/api/config` 形状“数组 vs pid-keyed dict”非对称但一致（A10）；所有 provider 的 id↔label、YAML↔Pydantic↔UI 字段名一致；`run_server.py` 主管启动序、`server.py` 路由挂载序正确。

### 2.2 Android 端

> 以下结论经 `analysis/tmp/subtask1_ui_persistence.md`、`subtask2_runtime_propagation.md`、`subtask3_model_persona_config.md` 三份主线交叉核对的只读审计确认（file:line 均已验证）。

**A. 运行时根本不吃这些设置（高危调用链断链 —— 对应你说的“相关调用逻辑链路有问题”）**
- **F2.1 High** LLM provider / base_url / api_key 在 `onCreate` 一次 `buildHttpLlmLink()` 快照进不可变的 `HttpLlmLink`（`MainActivity.kt:146,229-243`、`HttpLlmLink.kt:31-32`），`LoopCoordinator.llmLink` 是 `private val` 无 setter → 改 Key/Base URL/Provider **必须重启才生效**，与 `LLMProviderManager.kt:134-135` 文档“下一次请求即可生效”矛盾。
- **F2.2 High** `SettingsRepository.model`（聊天模型选择器）**只写不读**；出站请求恒用 `persona.getModel()`（`HttpLlmLink.kt:145-149`、`PersonaConfig.kt:125`），会话模型的”保存“等于无效。
- **F2.3 High** `ttsProviderId`（首选 TTS 引擎）只写不读；`VoiceIoController` 用构造期 `TtsProviderRegistry.cascade()` 快照（`VoiceIoController.kt:49,350-358`），`SpeechSinkAdapter.speak` 不带 providerId（`SpeechSinkAdapter.kt:44`）→ TTS 引擎选择 UI **纯装饰**，始终按 priority 降级链。
- **F2.4 Medium** 只有 CosyVoice 可通过 `refreshCosyVoiceProvider` 重建（`TtsProviderRegistry.kt:112-116`），且运行中 controller 的快照看不到新实例；Qwen 兜底层保留启动期旧 Key（`TtsProviderRegistry.kt:77-84`）→ 改 Key 后兜底仍用旧 Key 鉴权失败。

**B. 持久化时机不一致 / 非原子**
- 部分字段每个 `onValueChange` 即写 `repo`（qwen/zhipu/openai、model、renderQuality、asr、ttsProviderId，`SettingsScreen.kt:315-537`）；其余仅“保存”按钮集中写（`SettingsScreen.kt:759-767`）。
- **F1.4/F1.5 Medium** `onDispose` 无条件回写整个快照（`SettingsScreen.kt`），会覆盖外部改动、进程被杀丢编辑；混合 write-through + dispose 保存非原子。

**C. 无“设置模型 + commit + 应用到运行时”统一入口**
- `SettingsRepository.kt` 是纯 SharedPreferences 单 key 直写（`.apply()` 异步缓冲），无统一事务；运行时常量在启动时一次性构建 → “界面改了但运行仍用旧值”。

**D. 人设/模型/Persist 一致性**
- **F3.2 High** 聊天模型下拉 + Temperature 滑条也是只写不读，请求恒用 `persona.yaml` 值。
- **F3.3 Medium** 持久化的人设选择启动时从未生效（`MainActivity.kt:137` 恒加载根 `persona.yaml`）。
- **F3.6 High** `build.gradle` 硬编码 adapter 拷贝清单 → 非 bai 模型的 adapter 不上 assets → 静默回退 bai。
- **F3.7 High** `shared/persona.yaml` 与 Android `assets/persona.yaml` 已分叉。
- **C** 硬编码 `AVAILABLE_MODELS`（`SettingsRepository.kt:277-285`）与当前 provider 可用模型无关；全局 `model` 与各 provider model 并存，语义不明。

**E. 死代码 / 接线缺口**
- **F2.9 / F1.8** `UiModeConfig`/`UiModeStore` 生产代码完全未接线（layout 模式零运行时效果）。
- **F2.8** `resetAll()` 只清 `"live2dai_settings"`，`"llm_provider"` 与 `"live2dai_ui_mode"` 不清（`SettingsRepository.kt:210-212`、`LLMProviderManager.kt:37`、`UiModeStore.kt:38`）。
- **F2.16** `AppRoot` 运行时未使用、`Live2DAiApp.repo` 是死实例。
- **F2.11** `LoopWire.pump(coordinator, llmLink, scope)` 是空 no-op 且重复映射。

**F. 状态机 / 竞态（非设置 UI 但被触发）**
- **F2.6 Medium** 空文本发送会把状态机永久卡在 SENDING（`LoopCoordinator.kt:65-72`、`HttpLlmLink.kt:58-59`）。
- **F2.7 Medium** `HttpLlmLink.historyBuilder` 在 send(IO) 与 reset(main) 间无锁竞态。
- **F2.10 Medium** `LoopStateMachine.state` 非同步 `var` + 默认 `Unconfined` scope（生产注入 Main.immediate 缓解）。
- **F2.5 Medium** `getProviderType()` 与 `getActiveProvider()` 在 LOCAL 空 URL 时结果不一致。

**G. 未发现问题的区域（审计确认）**
- 启动期无 prefs `.commit()`/`getAll()`/网络主线程阻塞；全用 `.apply()`。
- 零配置/未知 provider 默认 DeepSeek 回退健壮且有测（`LLMProviderManagerTest.kt:67-93`）。
- Live2D 模型切换是唯一**真正热生效**（`Live2DAiApp.kt:66-81` + 成功回调才落盘 `SettingsScreen.kt:150-174`）。
- ASR 开关运行时实时读取（`MainActivity.kt:175`）。

---

## 3. 修复计划（按优先级）

### P0 — 消灭“写坏 conf.yaml / 热重载失败 / 启动失败”

1. **统一 TTS 落盘校验与自动补齐**
   - `update_config()` 写 TTS 前，对每个被选中/被写入的 provider 跑一次对应 typed config（`AppConfig`）校验；
   - 缺失必填字段（voice/model）自动补目录默认值；补不出来则 **拒绝落盘** 并返回 400 明确字段，而非写入后再失败。
   - 文件：`routes.py`（`update_config` 的 TTS 段）+ `config_manager/tts.py`（如需要 defaults helper）。

2. **“先校验后落盘 + 失败回滚”**
   - 把 normalize（自动补齐）+ `AppConfig` 校验移到 `_save_yaml(data)` **之前**；
   - 校验/热重载失败时保证 YAML 未被改写（或备份回滚），前端收到的是“配置未保存/保存失败”而非“已保存但热重载失败”。
   - 文件：`routes.py`；前端 `settings-ui.ts:910-927` 错误文案对齐。

3. **设定树落盘前做 schema 校验 + 热重载预检**
   - `update_config_tree` 在写文件前执行 `AppConfig` 校验与 reload 预检；失败返回 400 且不落盘。
   - 文件：`routes.py:728-740`。

### P1 — 前后端同步，消除漂移

4. **POST 成功回传 normalize 后完整 config + 前端 re-populate**
   - `/api/config` 成功响应里带 `config`（normalize 后的完整视图）；
   - `save()` 成功后调用 `populate(res.config)`，保证 UI = 磁盘 = 运行时一致。
   - 文件：`routes.py`、`settings-ui.ts:910-927`。

5. **抽取统一 normalize 函数**
   - 把 `update_config` 的自动补齐（TTS voice/voices、选中 LLM 最小配置）抽成前后端共用语义的 normalize 函数，消除两处实现不一致。

6. **保存语义支持“显式清空”**
   - 前端对可清空字段传 `null`；后端对 `null` 执行删除（区分“未传”与“显式清空”）；
   - person/提示词与 `.env` 的 key 都支持清除（提供“清除”按钮 / null 删除）。
   - 文件：`routes.py`（persona 写段、env 写段）、`settings-ui.ts`（collectPayload 与按键）。

7. **测试按钮统一使用“saved ∪ UI”合成视图**
   - `test_llm`/`test_tts` 请求携带当前 UI 的 base_url/model/voice 等；后端以已保存配置为基底叠加 UI 未保存参数。
   - 文件：`routes.py`、`settings-ui.ts:929-1006`。

8. **收敛 persona.model 与 provider model 职责**
   - UI 上去重（只保留一处“聊天模型”入口）或明确标注优先级；`_apply_persona_to_data` 注释/逻辑明确覆盖语义。
   - 文件：`routes.py:258-296`、`settings-ui.ts` person 卡片。


### P1(PC) 追加 — 运行时应用 / 日志 / 端口一致性

14. **“保存并应用”真正应用到已连接会话 + 修正 reload 顺序（对应 B3/D1）**
    - `apply=true` 时：先 `load_from_config` 到共享 cache，再遍历 `client_contexts` 为每个会话重跑 load；把 `self.system_config/character_config` 赋值移到 `init_agent` **之前**（`service_context.py:306-308 vs :301-304`）。
    - 若对已连接会话做即时热更超出范围，则返回 `{"applied":false, "requires_restart":true}` 并把 UI 文案改为“已保存，重启/重连后生效”，杜绝“applied:true 但没生效”的误导。
    - 文件：`routes.py:651-657`、`websocket_handler.py:183-202`、`service_context.py`。

15. **round-trip `log_level`（对应 A1 / D4）**
    - `_get_common` 返回 `log_level`（从 `.env` 读 `LIVE2DAI_LOG_LEVEL`），`renderSystem` 填充下拉；仅当用户改动时才随保存提交，避免每次“保存全部”重置回 INFO。
    - 文件：`routes.py:397-433,594-595`、`settings-ui.ts:779-783,906`。

16. **honor `system_config.port` + 统一启动器（对应 B5/B6 / D3）**
    - `run_server.py:145` 用 `config.system_config.port`；`_start_server.py` 改走与 `run_server.py` 同一 bootstrap（`${VAR}` 替换、人设注入、MCP path）或直接删除分叉启动器。
    - 文件：`run_server.py`、`_start_server.py`、`server.py:121-123`。

17. **让 `.env` 新 key 对运行进程可见（对应 B4）**
    - apply/reload 前重新 `load_dotenv(override=True)` 或在 openllm 各 factory 的 key 解析处支持运行时重读，避免 check/test 用新 key、引擎仍旧 key。
    - 文件：`run_server.py:21`、`config_manager/utils.py:44`、`routes.py:654`。

18. **统一 provider 协议感知的 check/test（对应 B9）**
    - `discover-models/check_providers/test-llm` 按 provider 选择协议（Anthropic `/v1/messages`、Ollama `/api/chat`…），不能对全部一律 OpenAI 形态。
    - 文件：`routes.py:816-1041`。


### P0/P1(Android) — 修复“设置写了但运行时不用”的调用链断链

9. **让 LLM 设置真正应用到运行请求（不重启）**
    - 改 provider/base_url/api_key/model 后无需重启即重建 `HttpLlmLink` 并替换 `LoopCoordinator.llmLink`（加 setter/`rebuild()`，或把 baseUrl/apiKey/model 改为运行时可读的 `StateFlow` 源）。
    - `HttpLlmLink.buildRequest.model` 改读“当前生效模型”，不再硬用 `persona.getModel()`。
    - 文件：`MainActivity.kt:229-243`、`HttpLlmLink.kt`、`LoopCoordinator.kt:28-35`、`LLMProviderManager.kt`、`SettingsScreen.kt`（对应 F2.1/F2.2）。

10. **让 TTS 引擎选择真实生效**
    - `SpeechSinkAdapter.speak`/`VoiceIoController.speak` 传入 `ttsProviderId`；`VoiceIoController.resolveChain` 每次从 registry 读最新 cascade（去掉构造期快照）或提供 `refreshProviders()`。
    - `refreshCosyVoiceProvider` 后同步注入运行中 controller；QwenTts 等其它 provider 也纳入可刷新。
    - 文件：`VoiceIoController.kt:49,350-358`、`SpeechSinkAdapter.kt:44`、`TtsProviderRegistry.kt:77-116`、`SettingsScreen.kt:559`（对应 F2.3/F2.4）。

11. **人设生效与一致性**
    - 启动应用持久化人设选择（F3.3）；聊天模型下拉与 Temperature 接上真实请求（F3.2/F2.2）。
    - 修 `shared/persona.yaml` 与 Android `assets/persona.yaml` 分叉（F3.7）；解 `build.gradle` 硬编码 adapter 清单导致的静默 bai 回退（F3.6）。
    - 收敛全局 `model` 与各 provider model 的“当前生效模型”语义；按当前 provider 动态给可用模型。
    - 文件：`MainActivity.kt:137`、`PersonaConfig`、`SettingsRepository.kt:277-285`、`build.gradle`、`LLMProviderManager.kt`。

12. **统一“设置模型 + commit + 应用到运行时”入口**
    - `SettingsRepository` 增加 `readSettings()/commit(settings)` 事务式快照；统一持久化时机（消除每 keystroke vs 仅保存按钮分裂），`onDispose` 不再无条件回写。
    - 明确每项“即时生效 / 重启生效”并 UI 标注。
    - 文件：`data/SettingsRepository.kt`、`SettingsScreen.kt:759-767`、`MainActivity.kt`/`Live2DAiApp.kt`（对应 F1.4/F1.5）。

13. **健壮性 / 死代码清理**
    - `resetAll()` 清空 `"llm_provider"` 与 `"live2dai_ui_mode"`（F2.8）；修 `getProviderType()` 与 `getActiveProvider()` 在 LOCAL 空 URL 的一致性（F2.5）。
    - 接线 `UiModeStore` 或在产品确认前标记未启用（F2.9/F1.8）；移除死 `AppRoot`/`Live2DAiApp.repo`/空 `pump`（F2.16/F2.11）。
    - 状态机健壮性：空文本发送不进入 SENDING 或超时回退（F2.6）；`historyBuilder` 加锁/序列化（F2.7）；`LoopStateMachine.state` 改 `@Volatile`/原子（F2.10）。

### P2 — 响应与交互体验

9. **打开面板不自动 discover**
   - 移除 `populate()` 无条件 `autoDiscoverConfiguredModels()`；改惰性（点击“获取可用模型”才拉）+ 结果缓存。
   - 文件：`settings-ui.ts:724-734、785-817`。

10. **批量检查并发 + 总超时 + 可取消**
    - `checkAll` 用 `Promise.all` + 总超时；discover/test 用 `AbortController` + 防抖，取消旧请求。
    - 文件：`settings-ui.ts:654-684、686-722`。

11. **TTS 检查语义明确**
    - `checkAll` 对 TTS 要么真实合成一次，要么明确标注“仅检查 Key 是否已配置”，避免误导。
    - 文件：`settings-ui.ts:654-684`、`routes.py:866-973`。

### P2(PC) 追加 — UI 防护 / 引擎生命周期 / 挂起堵死 / 测试架

12. **UI 防护与竞态（对应 A2/A6/A7/A11 / D6）**
    - 加载失败/加载完成前禁用 保存/保存并应用；所有异步按钮加 busy/disable；共享 status 用请求 token/AbortController 保证“最后一次点击”胜出，双击只发一次 POST。
    - `fillSelect` 加“未选择”占位（避免空 `llm_provider` 被浏览器自动选第一个）；POST 前 `checkValidity()`（挡 `port:0`）。
    - 处理 `Audio.play()` 拒绝；预览 URL 拼接统一走后端绝对/相对约定。
    - 文件：`settings-ui.ts:819,910,302-316,1083-1102,997-1002`。

13. **引擎生命周期与断线清理（对应 B7/B8 / D5）**
    - 给 engine 加 `close()`，替换前调用旧引擎清理；修 `handle_disconnect` 先查后 pop（`websocket_handler.py:302-314`），per-session close 引用计数（或停用跨会话共享引擎）；`init_vision` 支持热换。
    - 文件：`service_context.py:395-514`、`websocket_handler.py:302-314`。

14. **proxy 会话挂起修复（对应 B12）**
    - 播放确认超时路径也补发 `send_conversation_end_signal`，或由 proxy 端加回退复位 `conversation_active`，避免队列永久 gating 不再转发 text-input。
    - 文件：`conversations/conversation_utils.py:179-219`、`proxy_handler.py:224-229`。

15. **HTTP / apply 测试架（对应 D8，PC 测试缺口）**
    - `tests/conftest.py` 加 FastAPI `TestClient`/ASGITransport + 临时 conf/.env/persona fixture；新增 `tests/test_settings_api.py` 覆盖 GET/POST `/api/config`、tree、persona、models、provider-check（respx mock 外呼）、apply。
    - 把 `settings-ui.ts` 的可测纯逻辑抽出来供 Vitest（当前 `initSettingsPanel` 内部私有、`renderer/tests/*.test.ts` 无一个 import 它）。
    - 文件：`tests/conftest.py`、`tests/test_settings_api.py`、`renderer/tests/settings-ui.test.ts`。

### P3(Android) — 剩余机械 / 卫生项（与 P0/P1(Android) 不重复）

14. **引擎选择器与降级链一致性**
    - 引擎选择对未知/空 id 有 fallback（F1.6）；“降级链”摘要与顺序改用 `cascade()`（priority）而非 `all()`（插入序）（F1.7）。
    - `getById` 落到 unavailable/pc-only id 时回退默认（F3.5）。
    - 文件：`EngineSelectorUi.kt`、`VoiceIoController`、`ModelRegistry`。

15. **防护与类型安全**
    - 空串值不再静默覆盖默认并透传 provider（F1.10）；typed prefs getter 加类型/解析防护（F1.12）；`hasCustomDeepseekKey` 去 `!!`（F1.13）。
    - 模型下载状态正确联动 UI、不因离开页面取消（F1.1/F1.2）。
    - 文件：`SettingsRepository.kt`、`SettingsScreen.kt`、`ModelDownloadManager.kt`。

16. **测试补强（关键）**
    - 现存测试全部是纯/单元层，**没有任何一项覆盖“设置→运行时传播”边界**（F2 报告 §Test coverage gaps）——因此 F2.1/F2.3 的缺陷会全绿通过。
    - 新增：改设置后运行中 `HttpLlmLink` 复用新配置、`VoiceIoController` 用新 cascade/`ttsProviderId`、空文本发送不卡 SENDING、`resetAll()` 清空所有 store、`Shared/persona.yaml` 与 `assets/persona.yaml` 一致、`settings→repository 存取 round-trip`。
    - 清理陈旧测试头（`AppRootUiAssemblyTest.kt:16-17`“符号不存在”已过时）与命名误导（`SettingsRepositoryTest.kt:79` `ttsProviderId_defaultsToSherpa` 实为 cosyvoice_ws）。

### 3.5 文件行数 / 结构问题（可维护性 P3+）

**实测行数**

| 文件 | 总行数 | 问题 |
|---|---|---|
| `src/open_llm_vtuber/routes.py` | 1377 | `init_config_routes()` 单个函数 **1119 行**：catalog 目录 + helper + 11 个端点全在一个闭包内 |
| `renderer/src/settings-ui.ts` | 1126 | `initSettingsPanel()` 单函数约 **958 行**（L159-1117） |
| `Android ui/settings/SettingsScreen.kt` | 1220 | `SettingsScreen` Composable 单函数 **721 行**；ModelCard 118、PersonaSelector 89 |
| `config_manager/tts.py` | 945 | 多为数据类/Literal 目录（声明型），复杂度低，可接受 |
| `config_manager/agent.py` | 215 | 正常 |
| `data/SettingsRepository.kt` | 296 | 正常 |
| `ui/settings/EngineSelectorUi.kt` | 91 | 正常（纯函数映射） |

**与行数直接绑定的隐患**
- 巨型闭包/巨 Composable → helper 与状态不可复用、难以单测，直接阻碍 P0-2/P1-4 的“抽取统一 normalize / 校验 / 回滚”。
- `SettingsScreen` 将全部 `remember{}` 状态与 UI 揉在一起，是“持久化时机不一致、界面改了但运行仍用旧值”的根源。
- 巨型 innerHTML 字符串模板 + 事件绑定，回归/崩溃风险高。

**行内质量检查**
- `settings-ui.ts`：30 行超 120 字符（目录表为主）；无尾随空白/tab；有最终换行。
- `routes.py`：0 超长行；干净。
- `SettingsScreen.kt`：1 行超长（L610，133 字符）。
- 上述文件均无尾随空白、无 tab、无缺失末行换行。

**拆解策略（低风险、先治标再治本）**
- 不一次性大重构；在落地 P0/P1 修复时顺手抽离公共 helper：`tts`（校验+自动补齐）、`normalize`、`env/persona` 读写、模型下拉构建。
- 后续再把 `init_config_routes`（拆分到 config_api.py / catalogs.py）、`initSettingsPanel`（拆分 settings-ui → settings-state / settings-render 子模块）、`SettingsScreen`（状态提升到 `SettingsViewModel` + 拆成小节 Composable）逐步拆解。

---

## 4. 验证方案

**后端（PC）**
- 单测：`update_config` 对 `gpt_sovits_tts`/`edge_tts`“选中但不填参数”场景 → 落盘校验通过（自动补齐）或明确 400，且**不写坏 YAML**。
- 单测：热重载失败时 YAML 未被改写 / 已回滚。
- 单测：`update_config_tree` 对结构非法但 YAML 合法的输入 → 400 且不落盘。
- 单测：清空字段（null）能真正删除而非被忽略。
- 回归：`POST /api/config` 响应携带 normalize 后 config，与 `GET /api/config` 一致。
- 集成：`apply=true` 时已连接 WS 会话用新配置（mock engine 断言）；`load_from_config` 顺序修复后 `init_agent` 读到新 `system_config/avatar`。
- 集成：`log_level` round-trip（`.env`=DEBUG → 保存全部 → 仍 DEBUG）；`port` 设 12399 → 监听与 `/proxy-ws` 均在 12399；`.env` 新 key 对运行引擎可见。

**前端（PC）**
- 模拟 打开-修改-保存-再打开，断言 UI 与 `GET /api/config` 一致（无漂移）。
- 模拟连续 discover / 点“全部检查”，断言旧请求被取消、无交错覆盖。

**Android**
- `SettingsRepositoryTest` 增加 `commit/readSettings` 事务一致性用例。
- 增加“模型选择仅含当前 provider 可用模型”用例。
- 手工：修改需重启项 → 提示“重启生效”；修改即时项 → 立即生效；离开页面不丢已保存项。

---

## 5. 备注 / 待办

- 本计划已并入两份独立只读审计的交叉核对结论：
  - PC：`Live2D-Ai-pc/open-llm-vtuber/SETTINGS_SYSTEM_AUDIT.md`（A1-A11 / B1-B12 / schema 对照 / D1-D8）
  - Android：`analysis/tmp/subtask1_ui_persistence.md`(F1.*)、`subtask2_runtime_propagation.md`(F2.*)、`subtask3_model_persona_config.md`(F3.*)
- 两端共同根因：**配置被快照进 engine 构造器、无单一“当前设置”真源** → “保存了但运行没生效”，PC 靠重启/重连、Android 靠 Activity 重建，这在两端是同一类调用链断链。
- 本计划只描述修复方向，**未改动任何实现代码**。
