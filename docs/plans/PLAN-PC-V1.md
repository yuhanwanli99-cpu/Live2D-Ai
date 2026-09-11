# Live2D-Ai PC 端 v1 冲刺计划（只谈 PC）

> 范围：只谈 PC（WSL2/Linux 主力，Open-LLM-VTuber fork），不涉及 Android。
> 目标：把 PC 从“作者自测 v0”推进到“能拿去直播 / 能分发给别人 / 能算 v1”。
> 基线日期：2026-08-21，基于当前仓库 Live2D-Ai/。

---

## 0. 一句话结论

下一刀先做 S1：模型 zip 上传 + JSON 配置导入/导出/重置的产品化闭环。它是 v1 五项标准里“支持 Live2D 文件导入 + JSON 配置导入”的硬门，也是“分发给别人”的前提；做完 S1 后，PC 才真正从“我机器上能跑”变成“别人拿到也能配”。

当前已有相当多底层能力是半成品：model_upload.py、config_transfer.py、voice_library.py、live_event.py、mood_engine.py、emotion-consumer.ts、body-motion.ts、live-event.ts 都已存在，但缺产品化 UI、端到端串联和稳定性绿门。

---

## 1. v1 三条判断标准

| 标准 | PC 当前结论 | 关闭条件 |
| --- | --- | --- |
| 能不能拿去直播 | 还不能稳定直播。有通用 /api/live/event，但没接真实平台；模型只有参数动作、无 motion3；情绪/口型链路未全绿。 | 至少 B 站弹幕/礼物进得来、事件能触发动作与感谢语、断线/TTS/LLM 失败不把直播画面带崩。 |
| 能不能分发给别人 | 还不能。模型要靠手动放目录，配置导入/导出 UI 未闭环，API Key 依赖本地 .env，失败回滚和自检未成为产品能力。 | 别人拿到 zip/仓库后：导入模型、导入 JSON 配置、填 Key、点保存，即可跑起来；导入失败不污染环境。 |
| 能不能算 v1 | 不算。v1 五项标准里，“文件导入 + JSON 配置导入”未产品化，插件只有草案，motion 播放未做，全量测试未绿。 | 见第 4 节 v1 验收清单。 |

---

## 2. 当前基线（实测）

- Python 测试（tests/）：254 passed / 14 failed / 6 skipped，另有 1 个 collection error：
  - tests/test_pc_zero_config.py 找不到 open_llm_vtuber.free_glm_defaults
  - pymouth 相关 5 个失败（viseme 帧数为 0）
  - persona 一致性相关 6 个失败（desktop/Android/shared、temperature、vision_model、格式校验）
  - desktop zhipu 配置、emotion_protocol、consistency_parse 各 1 个失败
- 前端 renderer/ 已有 vitest 工程，但本阶段必须补跑并纳入绿门。
- 已有但未产品化：模型上传后端、配置导入导出后端、声音克隆库后端/前端纯逻辑、通用直播事件通道、MCP 工具。

---

## 3. 工作流拆分

### S0 — 基线修复与绿门（先做，防止边做边塌）

目标：让后续每一步都有可靠回归网。

- [ ] 修复 free_glm_defaults 缺失：恢复模块或删除过时测试，消掉 collection error。
- [ ] 修复 pymouth viseme 链路：process_audio 返回 0 帧根因（pymouth_viseme.py / prepare_audio_payload），恢复帧数与时间戳。
- [ ] 修复 persona 一致性：以 shared/persona.yaml 为单源，同步 Android assets 或调整测试基线，统一 temperature/vision_model/格式断言。
- [ ] 跑绿 pytest tests/，并加一条 scripts/verify_all.py 的本地绿门。
- [ ] 跑绿 renderer/ vitest；修复失败。
- [ ] 建立“提交前三条命令”：pytest tests/、renderer vitest、python scripts/smoke_check.py。

验收：三条命令全绿；pymouth、persona 不再失败。

---

### S1 — 模型 zip 上传 + JSON 配置导入/导出/重置（下一刀）

目标：把 v1 第 5 项硬门做成产品，而不是“接口存在”。

现状：
- 后端 src/open_llm_vtuber/model_upload.py 已有 zip slip/绝对路径/符号链接校验、model3 查找、注册表 upsert、部分回滚。
- 后端 src/open_llm_vtuber/config_transfer.py 已有 export/import 骨架。
- 前端 renderer/src/model-upload.ts 只有 56 行，缺拖拽/进度/预览/错误回滚 UI。

要做：

1. 模型导入 UI
   - 拖拽 + 选择 .zip；上传进度条；200MB 上限、错误提示。
   - 显示解析结果：model_id、显示名、.model3.json、贴图/moc3 数量、是否含 motion/exp3。
   - 上传后调用 /api/models/upload，成功后热切换（复用 set-model-and-conf），失败不写注册表、不污染 live2d-models/。
   - 模型列表支持：预览缩略图、设为默认、删除（删除前二次确认，删除默认模型被拒绝）。
2. 配置导入/导出/重置 UI
   - 设置中心新增“系统/高级 → 配置”入口：导出当前 conf.yaml + shared/persona.yaml 的 JSON/YAML 包。
   - 支持粘贴 JSON 或上传 JSON；前端做基础校验，后端用 Pydantic 全量校验；显示合并预览。
   - 导入前自动备份，失败自动回滚。
   - 一键重置为 conf.yaml.template + shared/persona.yaml 默认值。
   - 版本迁移：写入 config_version，旧配置提示升级路径。
3. 安全与健壮性
   - zip slip、符号链接、超大文件、嵌套目录、同名覆盖、并发导入测试。
   - 回滚策略统一为“先备份、再写入、失败 restore”，与 config_transfer.py 现有实现对齐。
   - 注册表写入用 shared/model_registry.schema.json 校验。

验收：
- 拖一个含 .model3.json 的模型 zip → 上传成功 → 前端热切换可见模型。
- 传一个恶意 zip（../、绝对路径、symlink）→ 拒绝且模型目录无残留。
- 导出一个 JSON，重置默认，再导入 → 配置一致；导入非法 JSON → 拒绝且原配置不变。
- 删除非默认模型 → 列表消失；删除默认模型 → 被拒绝。

---

### S2 — TTS 厂商参数卡片 + 声音克隆（需求 1）

目标：编辑 → 高级设置 → 参数调整 / 声音克隆，按厂商能力动态渲染卡片，不做“所有参数给所有厂商”的假 UI。

现状：
- tts_factory.py 已支持 cosyvoice_cloud/qwen/edge/openai/azure/minimax/siliconflow/fish/spark/gpt_sovits/x_tts/melo/sherpa/piper/pyttsx3。
- config_manager/tts.py 有各 provider 配置模型；settings-ui.ts 已有 TTS 卡片，但字段是固定 voice/model/ws_url 三件套，未按厂商差异化。
- voice_library.py / voice-library.ts 已有声音资产 API，但未接入“厂商克隆参数”语义。

要做：

1. TTS 能力注册表（后端单一真相源）
   - 新增 TTS_CAPABILITIES，每个 provider 声明：
     - 可调参数：voice/model/api_key/base_url/ws_url/rate/pitch/volume/speed/sample_rate/format/language/ref_audio/prompt_text/streaming_mode 等。
     - 是否支持 instruction（CosyVoice 专属）。
     - 是否支持克隆、克隆方式：ref_audio（GPT-SoVITS/SparkTTS/x_tts）或 voice_id/reference_id（CosyVoice/Fish/MiniMax/SiliconFlow）。
   - 新增 /api/tts/capabilities，前端据此渲染；未知字段一律不显示。
2. 高级设置 UI
   - 在现有 TTS Provider 卡片下加“高级设置”层级：参数调整 / 声音克隆 两个子面板。
   - 参数调整：动态表单，支持范围/默认值/单位/说明；每次改动可点“试听”验证。
   - 声音克隆：上传参考音频 → 登记音色 → 列表管理 → 试听 → 设为当前 TTS 音色。
   - 克隆资产绑定 provider 与 voice_id/reference_id/ref_audio_path，应用时写对应 conf.yaml 字段并热重载。
3. 后端应用逻辑
   - 扩展 voice_library.py：记录 provider、provider_voice_id、ref_audio_path、preview_path、active。
   - /api/voice/apply 按 provider 能力把参考音频写入正确字段，并触发现有 TTS 热重载。
   - CosyVoice 的 instruction 继续复用 tts_instruction_library.py，但允许用户在高级面板覆盖/微调。
4. 厂商差异化范围
   - cosyvoice_cloud：voice/model/format/sample_rate/volume/rate/pitch/ws_url/instruction；克隆走 voice_id（系统音色）或后续 DashScope customization。
   - gpt_sovits_tts / spark_tts / x_tts：参考音频克隆。
   - fish_api_tts：reference_id 克隆。
   - minimax_tts / siliconflow_tts：voice_id/model 克隆或预置音色。
   - edge_tts / openai_tts / azure_tts：只做音色/语速/音高/音量，不做克隆。
   - 本地引擎（sherpa/piper/pyttsx3/melo）：只做模型/音色/语速，不做云端克隆。

验收：
- 切到 CosyVoice 显示 instruction/rate/pitch/volume/sample_rate；切到 Edge 不显示克隆面板。
- 上传参考音频并设为 GPT-SoVITS 当前音色后，试听与对话都用新音色。
- 每个 provider 的试听按钮使用当前高级参数，失败有可读错误。

---

### S3 — System Prompt / 人设对齐酒馆（需求 2）

结论：能对齐，但目标不是复刻酒馆 UI，而是兼容 SillyTavern Character Card（v2/v3）的数据契约。酒馆本质是前端聊天系统；我们只接入它的“角色卡 + prompt 字段”，不搬 lorebook/世界书/多角色管理/jailbreak 那套重功能。

现状：
- shared/persona.yaml 已是单源真理，但只有 name/profile/system_prompt/model/temperature/max_tokens。
- 没有 persona.schema.json，没有 ST 卡片导入导出。

要做：

1. 建立 shared/persona.schema.json
   - 扩展字段：greeting、alternate_greetings、mes_example、scenario、post_history_instructions、creator、character_version、tags、extensions。
   - 保持现有字段向后兼容。
2. SillyTavern 导入
   - 支持 .json 与 .png（PNG tEXt:chara base64）角色卡。
   - 映射：name→name；description/personality→profile.personality_tags/speaking_style；scenario→system_prompt 场景块；system_prompt/post_history_instructions→追加到 system_prompt；first_mes→greeting；mes_example→示例对话。
   - 导入时保留本项目的 Live2D 表情/动作控制段，ST 卡内容只能插到人设段，不得覆盖控制协议。
   - 失败回滚到备份 persona.yaml。
3. SillyTavern 导出
   - 由当前 persona.yaml 生成 v2/v3 兼容 JSON（可选 PNG 卡），可被酒馆直接加载。
4. 设置 UI
   - “人设/提示词”页加：导入角色卡、导出角色卡、预览、重置人设。
   - 显示“酒馆字段 → 本项目字段”映射结果，让用户知道丢了什么/保留了什么。

验收：
- 导入一个 ST v3 角色卡 JSON → 保存后重启，LLM 按卡内人设说话。
- 导入含 character_book 的卡不崩溃，明确提示“世界书暂不导入，已保留为 metadata”。
- 导出当前 persona.yaml → 酒馆可加载，name/description/personality/first_mes 正确。
- 导入失败后 persona.yaml 与导入前一致。

---

### S4 — Live2D 与 LLM 联动对齐 Neuro（需求 3）

目标：从“只有嘴巴动”升级到“表情 + 半身动作 + motion3 + 事件动作”同步，接近 Neuro 的直播观感。

现状：
- LLM 已支持 [emotion] 标签和结构化 expressionMix/parameterOverrides/motionLabel（live2d_model.py）。
- 前端有 EmotionConsumer、ParamArbiter、BodyMotionPlayer、LipSync，但 bai 无 motion3，motionLabel 别名表为空，所以动作只靠参数。
- 模型加载后未解析 motion3.json 动作目录，未播放 Cubism motion。

要做：

1. 模型能力扫描
   - 后端 Live2dModel 解析 model3.json 的 FileReferences.Motions，输出 motion_groups 与 expressions 清单。
   - 模型注册表 / model_dict.json 增加 motionGroups、hasMotion、hasExp3 字段。
   - set-model-and-conf 下发 motion 目录给前端。
2. Motion3 播放器
   - 前端新增 motion-library.ts / motion-player.ts：按 group/index 播放 Cubism motion（Live2DModel.motion(group, index)）。
   - 动作分组映射：Idle / TapBody / Joy / Anger / Sadness / Surprise / Fear / Disgust / Smirk / Wave / Special。
   - 优先级融入 ParamArbiter：新增 ParamSource.Motion，并明确 Motion 与 Emotion/LiveEvent/LipSync 的覆盖关系——口型参数仍由 LipSync 保底，眨眼保底。
   - 无 motion 模型继续走 BodyMotionPlayer + IdleMotionController 参数兜底。
3. LLM 输出 → 动作路由
   - 每句回复同时下发 emotion、parameterOverrides、motion；motion 可为空（无 motion 模型）。
   - 后端按 emotion 选择 motion 组，按关键词（现有 MOTION_KEYWORDS）选择 wave/special。
   - 礼物/弹幕事件进入 live-event.ts 后触发具体 motion 与感谢语。
4. 同步与打断
   - 动作在 TTS 音频开始前 0–150ms 触发，与字幕/口型对齐。
   - 用户打断/新消息时释放 Motion 源，回落到 mood/idle 基线。
5. 可视化调试
   - 直播事件调试面板加“动作测试”按钮，列出当前模型可用 motion。
   - 开发者模式显示当前 motion/emotion/mood 状态。

验收：
- 导入一个带 .motion3.json 的模型 → Idle 动作循环、[joy] 回复触发 Joy 动作。
- 无 motion 的 bai 行为不回退，仍走参数动作。
- 触发 motion 时口型、眨眼不被错误压掉。
- 弹幕事件能触发 wave/special 动作与感谢语。
- 相关渲染单测（motion catalog/priority/fallback）绿。

---

### S5 — 插件运行时 + 知识库/记忆（需求 4、5）

结论：需要接入知识库和记忆，但要先有插件运行时，否则会继续往核心塞逻辑。PC 端先做 Python 运行时插件，不追求 Android 热加载。

现状：
- docs/architecture/plugin-sdk.md 是 Kotlin 接口草案，PC 无运行时。
- PC 已有 MCP 工具链（shared/mcp_tools.json）、basic_memory_agent、聊天历史持久化。
- 记忆/知识已有设计文档 docs/plans/plan-task-plugin-memory.md，但只覆盖 Android。

要做：

1. PC 插件运行时
   - 新增 src/open_llm_vtuber/plugins/：Plugin ABC、PluginHost、ChatHook、ToolDefinition。
   - PluginManager：扫描 plugins/ 目录、加载/卸载、启停、错误隔离（单插件异常不影响对话）。
   - 接线到 ServiceContext：before_send / after_receive 钩子，MCP 工具注册。
   - 设置 UI 新增“插件管理页”：列表、启用/停用、配置表单。
2. 知识库插件
   - 本地知识库：knowledge/ 目录，支持 markdown/txt/json 文件上传、删除、列表。
   - 检索：v1 用 SQLite FTS5 + 关键词/BM25；有 embedding Key 时可选向量检索（OpenAI 兼容 embedding）。
   - 每次 before_send 注入 top-k 片段；保留来源与截断保护。
3. 记忆插件
   - 短期记忆：继续用现有聊天历史。
   - 长期记忆：事实/偏好/用户信息写入 SQLite；每 N 轮或检测到“记住”触发 LLM 摘要。
   - before_send 注入相关记忆，after_receive 写入新记忆。
   - 管理 UI：记忆列表、删除、清空、关闭记忆注入。
4. 与 MCP 的关系
   - 知识库检索可以作为 ToolDefinition 暴露给 LLM，也可以作为自动 before_send 注入；v1 默认自动注入 + 提供“查知识库”工具。

验收：
- 一个示例插件（如自动回复/记忆）可启用/停用，插件抛异常不打断聊天。
- 上传知识文档后，问文档内问题能命中并引用来源。
- 用户说“记住我叫小明”，之后问“我叫什么”能正确回答。
- 记忆/知识 UI 可清空和关闭，且关闭后上下文不再注入。

---

### S6 — 直播平台桥（需求 6）

目标：把通用 /api/live/event 接到至少一个真实平台，形成直播可用闭环。

现状：
- live_event.py 已有通用事件通道、本机/局域网鉴权、60/min 限流、最近事件记录。
- scripts/run_bilibili_live.py、requirements-bilibili.txt 已有雏形，但未与前端事件路由和 LLM 策略打通。

要做：

1. B 站桥
   - 完善 scripts/run_bilibili_live.py：弹幕、礼物、大航海 → /api/live/event。
   - 支持房间号配置、cookie/登录态、重连退避。
2. 抖音/第三方桥
   - 抖音不硬接逆向协议；提供“弹幕姬/第三方消息桥”标准：JSON Lines 或 WebSocket 适配器。
   - 优先支持 B 站，抖音作为 P2。
3. 事件策略
   - 弹幕队列、限流、去重、黑名单、关键词过滤。
   - 规则：@主播/提及/提问 进 LLM；普通弹幕只触发动作；礼物触发特定动作与感谢语；大航海优先插队。
   - 队列满时丢弃策略可配置（drop-oldest / drop-newest）。
4. 调试面板
   - 显示连接状态、队列长度、最近事件、手动发送测试事件。

验收：
- B 站直播弹幕能进入事件通道并触发前端动作。
- 弹幕风暴下 LLM 不会被每条消息打断；策略命中率可观测。
- 断线后桥自动重连，不影响 Live2D 画面。

---

### S7 — 高级 UI 补齐（需求 7）

- [ ] 模型上传拖拽、进度、预览、删除、设为默认。
- [ ] 音色克隆上传、试听、列表、设为默认。
- [ ] 插件管理页。
- [ ] 配置导入/导出/重置入口。
- [ ] 日志查看器（server.log + 前端 console 分级导出）。
- [ ] 镜头模式设置 UI（半身/全身/自定义相机 FOV/缩放，写回配置）。
- [ ] 直播事件调试面板。
- [ ] TTS 高级设置层级卡片（S2）。

验收：所有高级能力可从设置中心完成，不要求用户改代码或手写 JSON。

---

### S8 — 稳定性 / 测试 / 工程化（需求 8）

- [ ] pytest tests/ 全绿。
- [ ] renderer vitest 全绿，并补 e2e（Playwright）：设置流程、模型上传、配置导入、直播事件。
- [ ] 故障演练：WebSocket 断线重连、TTS 失败降级、LLM 超时/失败、ASR 失败。
- [ ] 上传安全测试：zip slip、符号链接、超大文件、路径穿越、并发导入。
- [ ] 性能：120fps 目标下压测；纹理 > 4K 或模型超大时自动降采样/提示。
- [ ] 把 scripts/verify_all.py / scripts/smoke_check.py 串成本地 CI 绿门。

---

## 4. PC v1 验收清单（Definition of Done）

1. 文本/语音 → LLM → TTS → Live2D 表情/口型/动作闭环可用。
2. PC 插件运行时 + 至少 1 个真实插件（记忆或知识库）按接口运行。
3. 设置全部应用内可配置：LLM/TTS/ASR/人设/Live2D/直播/插件/知识库/记忆。
4. 应用内可改配置，不靠改代码；保存后热重载或明确提示重启。
5. 支持 Live2D 文件导入 + JSON 配置导入，失败自动回滚。
6. pytest tests/ + 前端测试 + e2e 冒烟全绿。
7. 拿去直播：至少 B 站弹幕/礼物桥可用，断线/TTS/LLM 故障不影响画面。
8. 分发给别人：一份文档 + 一个启动脚本，他人可完成导入模型/导入配置/填 Key/跑通。

---

## 5. 依赖与建议顺序

S0 基线绿门
   └─> S1 模型上传 + 配置导入/导出/重置   ← 下一刀，先做
         ├─> S2 TTS 高级参数 + 声音克隆
         ├─> S3 人设/酒馆兼容
         └─> S7 高级 UI（随 S1/S2/S3 增量交付）
S0/S1 稳定后：
   ├─> S4 Motion3 + Neuro 式联动（可与 S2/S3 并行）
   ├─> S5 插件运行时 + 知识库/记忆（依赖 S1 配置闭环）
   └─> S6 直播平台桥（可与 S4/S5 并行）
S8 稳定性/e2e/压测贯穿始终，v1 前全绿。

---

## 6. 立即开工项（下一刀）

1. S0：修复 free_glm_defaults、pymouth、persona 一致性三类测试失败。
2. S1-1：把现有 model_upload.py 用前端拖拽/进度/回滚 UI 包起来。
3. S1-2：把 config_transfer.py 做成设置中心“导入/导出/重置”完整闭环。
4. S1-3：补齐上传安全测试（zip slip/symlink/超大/并发）。
5. S1-4：跑通“导入模型 → 热切换 → 删除模型”端到端。
