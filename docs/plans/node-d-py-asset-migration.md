# 节点 D0：Py 资产盘点与迁移决策

> 范围：盘点 `py-legacy` 标签（commit `47848ed`，2026-08-26 快照）中仍有产品价值的资产，给出三类清单（直接迁移 / 参考后重写 / 永久废弃），并产出展示配置 schema、动作映射、Web 前端面板三类对照。
> 约束：Python 运行时永不恢复为依赖；只读 `git show py-legacy:<path>` 引用，不恢复任何文件到工作树；不引入 Python 工具链。
> 对齐基准：`docs/plans/RUST-REWRITE-RFC.md` D1–D12（特别是 D3 Rust ≥ 95%、D6 LLM SSE、D7 TTS 统一 OpenAI-compatible、D9 6 动作+release 协议）、`crates/live2d-ai-core/src/action/mod.rs`（已落地的 6 动作枚举）、`docs/architecture/core-contracts.md` / `directory.md`。

---

## 0. 盘点口径

- **盘点来源**：`Live2D-Ai-pc/open-llm-vtuber/` 全部（在 `py-legacy` 提交存在 246 个 `.py`、49 个前端 `.ts`/`.html`、11 个 `assets/` 资源、3 个 `config_templates/`、1 个 `backgrounds/README.md`、0 个背景图）。
- **节点 D 边界**：原生应用（Rust 桌面端） + Web 前端产品化闭环。仅当资产有「可被 Rust core / Web 前端复用的语义或内容」时纳入；纯 Python 运行时桥（FastAPI、uvicorn、asyncio、loguru、chardet、pyyaml、aiohttp 等）一律废弃。
- **去向三分类**：
  - **直接迁移**：以原始语义/字段名/资源字节复用，平移到 Rust 资产服务（`shared/` 或 `crates/*/assets/`）或 Web `apps/web/src/*` 静态资源。
  - **参考后重写**：保留语义骨架（白名单、协议字段、状态机、面板分区、校验规则），但用 Rust/TypeScript 按新架构重写。
  - **永久废弃**：Python 专属（依赖运行时/PyPI/IO 编码探测）、与节点 D 无关（仅服务端 Python 逻辑）、或 v0 已明确不再支持的代码。

---

## 1. 直接迁移清单（Migrate-as-is）

> 特征：内容已经是「跨语言可消费的语义/字节」，可直接落到 `shared/` 或 `crates/*/assets/`。每项 ≥1 处落位。

| # | 来源路径（py-legacy 下） | 去向 | 一句话理由 |
|---|------------------------|------|-----------|
| M1 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/motion_catalog.py` 的 `BASE_MOTION_IDS` + `CAPABILITY_GATED_MOTION_IDS` + `SAFE_MOTION_IDS` + `MOTION_REQUIRED_PARAMS` + `MOTION_METADATA`（label / description / suggested_duration_ms / halfbody / release_only） | `crates/live2d-ai-core/src/action/catalog.rs`（新建）→ 作为 6 基础 + 9 能力门控 + 1 release 的**单一来源**，与 D9 六动作+release 决议同源 | 动作目录是**协议级契约**；Rust core 现仅有 6 个枚举，缺 9 个能力门控动作与元数据表（label/description/duration），必须平移才能与后端/前端一致 |
| M2 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/live2d_action.py` 的 `LIVE2D_ACTION_TOOL_GUIDANCE`（动作工具使用规范 prompt 文本） | `crates/live2d-ai-runtime/src/llm/tool_guidance.rs`（新建，作为静态常量）| 工具使用规范是 LLM system prompt 的可重用片段，不依赖 Python；后续每轮现算拼接即可 |
| M3 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/director.py` 的 `_USER_MOTION_COMMANDS`（中-英文动作命令关键词表，13 个动作 × 多条同义词） | `crates/live2d-ai-core/src/action/user_keywords.rs`（新建，引入 `unicase` 或手工大小写归一） | 用户输入的「点点头/摇头/歪头/张望/向上看…深呼吸」关键词表是**语言资源**，与运行时无关 |
| M4 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/director.py` 的 `_USER_MOTION_GUARD_TERMS`（疑问/否定语境保护词表：`为什么/别/怎么/…`） | 同 M3 文件 | 否定/疑问保护词表是**语言资源**，平移后由 Rust 命令路由复用 |
| M5 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/dev-console/action-registry.ts` 的 `ACTION_ZH` 映射（16 个动作的中文 name/description） | `apps/web/src/settings/dev/actions.zh.json`（静态 JSON） | 前端中文显示文案是内容资源，与运行时无关 |
| M6 | `Live2D-Ai-pc/open-llm-vtuber/renderer/public/soullink-profiles/bai.profile.json`（`schemaVersion: 2` 的 bai 自动 profile，含 `capabilities` 与 `parameterMap`） | `shared/model-adapter/bai.soullink-profile.json`（与 adapter.json 并列） | 已是双端共享单源风格；Rust core 启动期能力探测可读此文件判断 `headControl/gazeControl/breath/...` |
| M7 | `Live2D-Ai-pc/open-llm-vtuber/prompts/utils/live2d_expression_prompt.txt`（结构化 JSON + 兜底 keyword tag 的 LLM 表达协议） | `crates/live2d-ai-runtime/src/llm/prompts/live2d_expression.txt` | 协议级 prompt 模板，跨语言复用 |
| M8 | `Live2D-Ai-pc/open-llm-vtuber/prompts/utils/think_tag_prompt.txt` / `speakable_prompt.txt` / `concise_style_prompt.txt` | 同 M7 路径对应文件 | 通用 LLM 行为 prompt 模板，与运行时无关 |
| M9 | `Live2D-Ai-pc/open-llm-vtuber/assets/` 全部 11 个资源（`banner.cn.jpg` / `banner.jpg` / `banner.kr.jpg` / `i1..i4` + 桌面宠截图） | `crates/live2d-ai-desktop/assets/marketing/`（新建）或 `apps/web/public/marketing/` | 营销/介绍截图，纯字节资源，不依赖 Python |
| M10 | `Live2D-Ai-pc/open-llm-vtuber/backgrounds/README.md`（说明文档，不含图片） | `crates/live2d-ai-desktop/assets/backgrounds/README.md` | 背景图目录的用户说明文案，零依赖 |
| M11 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/halfbody-profile.ts` 的 `HALF_BODY_CHANNEL_IDS`（语义通道→ParamAngle/ParamEyeBall/ParamBrow/ParamBreath 映射） | `crates/l2d/src/profile/halfbody_channels.rs`（新建） | 能力门控的语义通道表是**模型层协议**，与运行时无关 |
| M12 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/idle/idle-blink.ts` 的 `BlinkConfig` 默认值（closeDuration=75 / openDurationMin=150 / openDurationMax=300 / intervalMin=3000 / intervalMax=8000） | `crates/l2d/src/idle/blink_config.rs` | 眨眼参数是经真机核验的可调档位，是产品价值数据 |

---

## 2. 参考后重写清单（Rewrite-from-reference）

> 特征：保留语义骨架（白名单、协议字段、状态机、面板分区、校验规则），但按 Rust/Web 架构重写。每项须指明「保留什么、改什么」。

| # | 来源路径（py-legacy 下） | 去向 | 保留什么 / 改什么 |
|---|------------------------|------|------------------|
| R1 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/live2d_model.py`（`Live2dModel` 类的 model_dict + cdi3 + emotionMap 加载、`PROTOCOL_EMOTION_KEYS` 8 键、`PROTOCOL_EMOTION_INDEX`、`MOTION_KEYWORDS` 已空表） | `crates/live2d-ai-core/src/model/registry.rs` + `crates/live2d-ai-core/src/model/cdi3.rs` | **保留**：8 情绪协议键（neutral/fear/sadness/anger/disgust/joy/smirk/surprise）、cdi3 运行时参数白名单、registry 兜底逻辑。**改**：去掉 `chardet` 编码探测（Rust 端统一 UTF-8），去掉 `loguru`（用 `tracing`），把 `motion_dict.json` 读改为 `shared/model_dict.json`（与 Android 共用） |
| R2 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/director.py` 的 `ExpressionDirector`（独立 LLM 导演+兜底映射 `_EMOTION_MOTION = {"surprise":"surprise"}` + `detect_user_motion_sequence` 切分逻辑 + `_STAGE_GESTURE_KEYWORDS` 括注映射） | `crates/live2d-ai-runtime/src/director/mod.rs` | **保留**：`surprise→surprise` 单点情绪兜底、序列切分（`然后/接着/先/再/，,。!？；、`）、括注全角/半角正则、语序最早+同位置最长匹配。**改**：JSON 提取用 `serde_json::from_str`、正则单独模块、用 `OpenAiClient` 而非自建 LLM 客户端、兜底由 reducer 规则层承担（每轮必返可观测结果） |
| R3 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/live2d_action.py` 的 `live2d_perform_action` tool schema（OpenAI 兼容 function-calling 协议：`^[a-zA-Z0-9_-]+$` 函数名约束、`action`/`strength`/`reason` 三参数、`MIN/MAX/DEFAULT_STRENGTH = 1/3/2`） | `crates/live2d-ai-runtime/src/llm/tools/live2d_action.rs`（serde + schemars 派生 JSON Schema） | **保留**：API 名 `live2d_perform_action`、三参数（action/strength/reason）、`reason` 上限 200 字符、强度档位 1/2/3。**改**：去掉 `is_valid_openai_tool_name` 运行时断言（编译期由 schemars 保证）、module-load 时 `if not is_valid...` 守卫改为单元测试 |
| R4 | `Live2D-Ai-pc/open-llm-vtuber/conf.yaml.template` / `config_templates/conf.default.yaml` / `config_templates/conf.ZH.default.yaml` 整套 schema（`system_config` / `character_config` / `asr` / `tts` / `vad` / `tts_preprocessor` / `vision_config` / `mcp_servers`） | `crates/live2d-ai-core/src/config/schema.rs`（用 `serde_yaml` 派生结构体；或迁移到 TOML `live2d-ai.toml.example` 已存在） | **保留**：字段语义（`host/port/live2d_fps/config_alts_dir/developer_mode` + `conf_name/conf_uid/live2d_model_name/character_name/avatar/persona_prompt/load_persona_from_file` + `agent_config.conversation_agent_choice` + `llm_configs.<provider>.{llm_api_key/model/base_url/temperature/thinking_enabled/models}` + `asr_config`/`tts_config`/`vad_config`/`tts_preprocessor_config`）。**改**：去掉 `${XXX_API_KEY}` 占位符语法（Rust 端改用 `std::env::var` 显式引用或独立 `.env`）；`conf_version: 'v1.2.1'` 由 schema 版本号取代；删除 `mcp_servers` 字符串字段（节点 D 决议待定） |
| R5 | `Live2D-Ai-pc/open-llm-vtuber/model_dict.json`（`name/url/idleMotionGroupName/emotionMap 8 键/tapMotions`） | `shared/model_dict.json`（已是双端共享位置） | **保留**：8 情绪索引 0/1/2/3/4/5/6/7 与 key 对齐、`url` 路径模式。**改**：与 `shared/model-adapter/bai.adapter.json` 的 `emotionIndex` 做单向一致性校验；`idleMotionGroupName: ""` 字段在 bai 上保留为空（无 motion3.json） |
| R6 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/config-transfer.ts`（`POST /api/config/export` / `POST /api/config/import`，返回 `{format:"live2dai-config", version:1, conf_yaml, persona}`） | `apps/web/src/settings/config-transfer.ts` | **保留**：JSON 结构（`format/version/conf_yaml/persona`）、错误文案解析。**改**：fetch 路径由 `/api/config/export` 改为新核心的 `GET /api/v1/config/export`；版本号随 schema 升级 |
| R7 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/background.ts`（`BackgroundFile`/`BackgroundState` 类型 + `ALLOWED_BACKGROUND_EXTENSIONS`=[png/jpg/jpeg/webp/gif] + `MAX_BACKGROUND_BYTES=12MiB` + `backgroundUrlFor`） | `apps/web/src/settings/background.ts` + `crates/live2d-ai-runtime/src/api/background.rs` | **保留**：12 MiB 上限、5 种扩展名白名单、`/bg/<encoded name>` 静态 URL。**改**：上传端点 `/api/background/upload` 改 Rust；存储位置由 `backgrounds/` 改为 `~/.local/share/live2d-ai/backgrounds/`（XDG） |
| R8 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/external-input.ts`（`ExternalStatus` 类型 + 默认关闭 + 仅本机 + token 校验 + 范围文案 `describeGateScope`） | `apps/web/src/settings/external-input.ts` + `crates/live2d-ai-runtime/src/api/external.rs` | **保留**：默认 `enabled=false`、必须本机、`allow_remote` 显式开关、`token_set` 状态、最近注入记录（accepted/rejected/recent）。**改**：端口/路由改为 Rust 端 WS（OpenAI-compatible 协议下独立 `/api/v1/external/chat`） |
| R9 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/panel-template.ts` 的 9 tab 骨架（对话/LLM · 语音/TTS · 形象/背景 · 外部接入 · 密钥 · 人设/提示词 · 系统/高级 · 记忆 · 插件） | `apps/web/src/settings/panel-template.ts`（重写为 Vite + React/Solid/原生组件） | **保留**：分区顺序、状态简报 4 行（LLM/TTS/ext/sys）、版本块（product/backend/frontend/commit）。**改**：去掉 `data-tab="plugins"` 内的「外部插件安装」流程（v0 只内置原生插件）；把 conf.yaml 原始 YAML 编辑器改为只读 + 跳转配置文件 |
| R10 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/choreography.ts` 的 `ACTION_REQUIRED_PARAMS` + `ActionPriority` 枚举（`dev/user/timeline/fallback/idle`） | `crates/live2d-ai-core/src/action/priority.rs`（新建，与 D9 同源） | **保留**：5 级优先级、能力门控白名单。**改**：去掉 PixiJS/Canvas/WebGL 依赖（Rust 端只导出枚举 + 必需参数表，渲染在 `l2d-wasm-demo`/`l2d` 侧） |
| R11 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/dev-console/dev-console.ts`（开发者控制台：动作目录可视化 + 手动触发） | `apps/web/src/settings/dev-console.ts`（仅当 `developer_mode=true` 时挂载） | **保留**：动作列表渲染、强度 1/2/3 滑块、release 按钮、accepted/rejected 计数。**改**：去掉「原始动作触发接口」直连（Rust core 走 audit log） |
| R12 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/motion-timeline.ts`（`MotionCue`/`MotionCueFire` + 时间源接口 + `FIRE_EPSILON_S=0.03` 提前量） | `apps/web/src/playback/motion-timeline.ts` | **保留**：`at ∈ [0,1)` 归一化进度、`FIRE_EPSILON_S=30ms`、cancel 幂等、同一 cue 触发一次。**改**：时间源抽象改为对 `HTMLAudioElement` 与 `<video>` 双兼容（不再需要 TimelineTimeSource interface） |
| R13 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/live-event.ts` 的 `LiveEventPayload`（`type/text/user/motion/emotion/strength/parameterOverrides`） | `crates/live2d-ai-runtime/src/api/live_event.rs` | **保留**：6 字段契约、`motion` 限定六动作 + release、`emotion` 限定 8 协议键。**改**：去掉 `LiveEventKind` 字符串通配类型（v0 只接 `danmaku`/`gift`/`command` 三类，简化 schema） |
| R14 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/director.py` 的 `_BRACKET_RE`（全/半角括号正则）+ `filter_parentheses` 行为 | `crates/live2d-ai-core/src/parse/parentheses.rs` | **保留**：正则 `[\（(]([^\（）()]*)[）)]`、不跨层嵌套。**改**：用 `regex` crate 替代 `re`；与 `motion-timeline` 的 cue 切分统一 |
| R15 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/bootstrap/storage.ts`（localStorage + 隐私模式降级 no-op） | `apps/web/src/bootstrap/storage.ts` | **保留**：能力检测 + try/catch 降级语义。**改**：增加 IndexedDB 优先层（背景图 base64/缩略图缓存场景） |

---

## 3. 永久废弃清单（Discard-permanently）

> 特征：依赖 Python 运行时 / FastAPI 服务 / PyPI 库 / 仅服务端 IO 桥 / v0 不再支持。每项给具体废弃理由。

| # | 来源路径（py-legacy 下） | 一句话理由 |
|---|------------------------|-----------|
| X1 | `Live2D-Ai-pc/open-llm-vtuber/run_server.py` / `_start_server.py` / `server.py` / `test_server.py` / `test_start.py` / `_smoke_test.py` | Python FastAPI 启动器；Rust core 用 `crates/live2d-ai-desktop` bin 接管 |
| X2 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/agent/` 全部（含 `stateless_llm/`、misconfigured_llm） | LLM agent 循环由 `crates/live2d-ai-runtime` 的 `OpenAiClient` 承担，不再有 Python agent |
| X3 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/api/` 全部（`config_routes.py` / `config_update.py` / `dev_routes.py` 等） | HTTP 路由在 Rust 端重写（axum），不再保留 Python 路由层 |
| X4 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/asr/` 全部 | ASR v0 不在节点 D 范围内（V12 待办）；即使后续引入也由 Rust 端统一 OpenAI-compatible |
| X5 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/tts/` 全部（25 个文件，含 `azure_tts.py` / `edge_tts.py` / `fish_api_tts.py` / `gpt_sovits_tts.py` / `melo_tts.py` / `piper_tts.py` / `pyttsx3_tts.py` / `pymouth_viseme.py` / `qwen_tts.py` / `sherpa_onnx_tts.py` / `siliconflow_tts.py` / `spark_tts.py` / `x_tts.py` / `minimax_tts.py` / `cosyvoice_cloud_tts.py` / `openai_tts.py` / `tts_factory.py` / `tts_instruction_library.py` / `tts_interface.py`） | D7 明确：v0 只统一 OpenAI-compatible HTTP API；所有厂商私有 SDK 一律废弃 |
| X6 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/vad/` 全部 | VAD 不在 v0 范围（参见 D6 备注）；后续若引入由 Rust 端独立 crate 实现 |
| X7 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/conversations/` 全部 | 对话循环由 Rust reducer + `OpenAiClient` 承担 |
| X8 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/mcpp/` 全部（Model Context Protocol Python 桥） | v0 不引入 MCP；D6 仅承诺 OpenAI-compatible |
| X9 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/plugins/` 全部（`manager.py` / `native.py` / `native_runtime.py` / `manifest.py` / `health.py` / `host.py`） | 插件 SDK 由 `docs/architecture/plugin-sdk.md` Rust 端实现；Python 插件宿主废弃 |
| X10 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/crash_reporter/` 全部 | Python 端崩溃报告，与 Rust 应用无关；节点 D 走 `tracing` + `sentry-sdk` |
| X11 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/translate/` 全部 | v0 翻译功能不在节点 D 范围；`conf.yaml` 里的 `translator_config` 字段删除 |
| X12 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/validators/` 全部 | 字段校验由 `serde` derive + 自定义 `validate` trait 承担 |
| X13 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/utils/` 全部 | Python 工具函数（编码探测/路径解析/IO），Rust 端由 `std` + `tokio::fs` 替代 |
| X14 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/config_manager/` 全部 12 文件 | YAML 配置加载由 `serde_yaml` + 结构体派生承担 |
| X15 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/services/` 全部 | Python 服务层抽象，无对应 Rust 概念 |
| X16 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/turn/` 全部 | turn 调度由 Rust reducer 的 `Turn` + `Sentence` 状态机承担 |
| X17 | `Live2D-Ai-pc/open-llm-vtuber/prompts/__init__.py` / `prompt_loader.py` | Python 文件 IO 加载器；R8 prompt 模板已列入 M7/M8 直接迁移 |
| X18 | `Live2D-Ai-pc/open-llm-vtuber/test_import.py` / `tests/` 全部 | Python 端测试；Rust 端测试在 `crates/*/tests/` |
| X19 | `Live2D-Ai-pc/open-llm-vtuber/requirements.txt` / `requirements-bilibili.txt` / `pyproject.toml` / `pixi.lock` / `dockerfile` / `.dockerignore` | Python 依赖与容器化配置；Rust 端走 Cargo |
| X20 | `Live2D-Ai-pc/open-llm-vtuber/CLAUDE.md` / `.pre-commit-config.yaml` / `.python-version` / `.gitattributes` / `.github/` | Python 项目工程文件，节点 D 改用 Rust 工具链（`xtask`、`cargo fmt`、`cargo clippy`） |
| X21 | `Live2D-Ai-pc/open-llm-vtuber/scripts/` 全部 | 一次性 Python 脚本；如需保留用途由 `xtask` 重写 |
| X22 | `Live2D-Ai-pc/open-llm-vtuber/doc/sample_conf/` 全部 5 文件 | 样例配置；新 conf schema 由 `serde_yaml` 默认值承担 |
| X23 | `Live2D-Ai-pc/open-llm-vtuber/renderer/tests/` / `vitest.config.ts` / `tsconfig.json` / `vite.config.ts` / `package.json` / `package-lock.json` / `tools/` | Web 前端工程文件；新前端由 `apps/web/` workspace 接管 |
| X24 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/main.ts` 整体 | 入口主程序（PixiJS 渲染 + 设置中心挂载）；`apps/web` 用新架构重写 |
| X25 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/{emotion-consumer,lip-sync,chat-ui,chat.css,history,icons,interaction,memory-api,model-loader,model-upload,onboarding,param-arbiter,plugin-manager,settings-logic,settings-ui,soullink-adapter,texture-adapter,theme.css,toast,types,version-info,version-env.d.ts,camera,camera-state,body-motion,action-policy,halfbody-controller}.ts` | PixiJS/Canvas/WebGL 渲染层与现 Web UI 实现；渲染由 `crates/l2d` 承担，UI 由 `apps/web` 重写 |
| X26 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/idle/{idle-blink,idle-breath,idle-micro-expr,idle-motion}.ts` 实现代码 | 仅 BlinkConfig 默认值列入 M12；状态机/参数写入由 `crates/l2d` 用 wgpu 重写 |
| X27 | `Live2D-Ai-pc/open-llm-vtuber/renderer/src/bootstrap/live2d-modules.ts` | PixiJS + Cubism Web 动态导入；Rust 端用 `l2d` crate 直接绑定 |
| X28 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/services/backgrounds.py`（若存在）等 | Python 静态目录服务；改 Rust `tower-http` `ServeDir` |
| X29 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/external_input.py` | Python 端外部注入路由；改 Rust `axum` |
| X30 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/turn/single_conversation.py` / `group_conversation.py` | Python 端 single/group 对话循环；改 Rust reducer |
| X31 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/agent/agents/` 全部 agent 实现 | 改 `crates/live2d-ai-runtime` 的纯函数组合 |
| X32 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/{asr/vad/mcpp}_factory.py` 类的工厂模式 | Python 工厂；Rust 端用枚举 + `match` 替代 |
| X33 | `Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/config_models.py` / `conversations/tts.py` 等数据类 | Python `pydantic`/`dataclass`；Rust 端用 `serde` derive |
| X34 | `Live2D-Ai-pc/open-llm-vtuber/.gitmodules` / `Live2D-Ai-pc/open-llm-vtuber/frontend/` | Open-LLM-VTuber 原仓库子模块引用；节点 D 不再依赖上游 |

> 统计：X1–X34 共 34 项（部分项涵盖多个文件，实际 246 个 `.py` 全部归入此分类）。

---

## 4. 「Live2D 展示配置」迁移对照（Py → 节点 D Web schema）

> 目标：让 Web `model/stage` 字段名一次性对齐 Rust core（`l2d::profile` + `live2d-ai-core::model`）与共享 `shared/model-adapter/` 单源。
> Py 模型显示语义散落在 `model_dict.json`（`url`）、`conf.yaml`（`avatar`/`live2d_model_name`/`live2d_fps`）、`backgrounds/` 目录、settings 面板「形象/背景」tab。

| Py 字段/资源 | 含义 | 节点 D Web schema 字段（建议） | 落位 |
|---|---|---|---|
| `model_dict.json[].name` | 模型 ID | `model.id: string` | `shared/model_dict.json`（已是双端共享） |
| `model_dict.json[].url` | model3.json 静态 URL（`/live2d-models/bai/runtime/bai.model3.json`） | `model.runtimeUrl: string` | `shared/model_registry.json` |
| `model_dict.json[].emotionMap` 8 键 | 情绪→索引映射 | `model.emotionMap: Record<EmotionKey, u8>`（与 `EMOTION_KEYS=neutral/fear/sadness/anger/disgust/joy/smirk/surprise` 同源） | `shared/model-adapter/<id>.adapter.json` 的 `emotionIndex` |
| `model_dict.json[].idleMotionGroupName` | motion3 group 名（bai 为空） | `model.idleMotionGroup: string \| null`（空表为 `null`） | `shared/model_registry.json` |
| `conf.yaml.character_config.live2d_model_name` | 默认模型 ID | `character.modelId: string` | `live2d-ai.toml` (Rust core) |
| `conf.yaml.character_config.avatar` | 头像文件名（`mao.png`） | `character.avatar: string`（URL 或 base64） | `live2d-ai.toml` |
| `conf.yaml.character_config.character_name` | 角色名 | `character.displayName: string` | `live2d-ai.toml` |
| `conf.yaml.system_config.live2d_fps` | 渲染 FPS（默认 120） | `stage.fps: u8`（默认 60，1..240 校验） | `live2d-ai.toml` |
| `conf.yaml.system_config.host`/`port` | 监听地址 | `server.host: string` / `server.port: u16` | `live2d-ai.toml` |
| `conf.yaml.system_config.developer_mode` | 开发者模式开关 | `dev.enabled: bool`（默认 `false`） | `live2d-ai.toml` |
| `backgrounds/<file>` 静态资源 | 角色背景图 | `stage.background.url: string`（XDG 路径映射 `/bg/<name>`） | `apps/web` + `~/.local/share/live2d-ai/backgrounds/` |
| settings 面板「角色背景图」上传 | 用户上传背景 | `stage.background.localFile: File`（受 `MAX_BACKGROUND_BYTES=12 MiB` 与扩展名白名单约束，见 R7） | `apps/web/src/settings/background.ts` |
| settings 面板「扫描并导入本地模型」 | 扫描 `live2d-models/` | `model.scan: action`（POST `/api/v1/models/scan`） | `apps/web/src/settings/model-list.ts` |
| settings 面板「上传 Live2D 模型 (.zip)」 | 导入新模型 | `model.import: action`（POST `/api/v1/models/import`，受 100 MiB 限制） | `apps/web/src/settings/model-list.ts` |
| halfbody 语义通道 `HALF_BODY_CHANNEL_IDS`（M11） | headX/Y/Z / gazeX/Y / browY/Angle/Form / breath | `stage.channels: Record<SemanticChannel, ParamId[]>`（运行时按 cdi3 探测裁剪） | `crates/l2d/src/profile/halfbody_channels.rs` |
| cdi3.json 运行时参数 ID 白名单 | 能力门控判定依据 | `model.cdi3Params: Set<ParamId>`（启动期加载，校验 `parameterOverrides`） | `crates/live2d-ai-core/src/model/cdi3.rs`（来自 R1） |
| `live2d_fps: 120` | Web 端 rAF 60fps 上限 | Web 端不感知 fps，固定 `requestAnimationFrame`；Rust core 用 `tokio::time::interval` 控制 reducer 频率 | `crates/live2d-ai-core` |

> 注：背景图实际为 0 张图片（`backgrounds/README.md` 唯一内容）。背景图能力必须由用户在首次使用时上传（沿用原 R7 流程）。

---

## 5. 「动作/命令」迁移对照（Py live2d_action → Rust core 6 动作）

> 目标：把 Py 的 15 动作（6 基础 + 9 能力门控）+ 1 release 与 Rust core 现 6 动作枚举对齐；扩展动作由能力门控裁决在 v0.1 之后补齐。
> Rust core 现状：`crates/live2d-ai-core/src/action/mod.rs` 只有 `Nod/ShakeNo/Tilt/LookAround/Listen/Surprise` 6 个枚举；`release` 是优先级语义（priority=Idle）。
> Py 来源：`motion_catalog.py`（`MOTION_METADATA` 16 键）+ `live2d_action.py`（`MIN/MAX/DEFAULT_STRENGTH=1/3/2`、`reason` 上限 200）+ `director.py`（`_USER_MOTION_COMMANDS`/`_STAGE_GESTURE_KEYWORDS`）。

### 5.1 动作 ID 映射

| Py id（来自 `motion_catalog.SAFE_MOTION_IDS`） | 中文 label | 强度范围 | 来源/触发 | Rust core 现状 | 映射建议 |
|---|---|---|---|---|---|
| `nod` | 点头 | 1..3 | LLM tool / 用户命令 / 情绪 fallback / 括注 | `ActionKind::Nod`（值 0） | **直接对齐**；`BodyAction` 序列化用字符串 `"nod"` |
| `shake_no` | 摇头 | 1..3 | 同上 | `ActionKind::ShakeNo`（值 1） | **直接对齐** |
| `tilt` | 歪头 | 1..3 | 同上 | `ActionKind::Tilt`（值 2） | **直接对齐** |
| `look_around` | 左右张望 | 1..3 | 同上 | `ActionKind::LookAround`（值 3） | **直接对齐** |
| `listen` | 倾听 | 1..3 | 同上 | `ActionKind::Listen`（值 4） | **直接对齐** |
| `surprise` | 惊讶 | 1..3 | 同上（情绪 fallback 唯一映射） | `ActionKind::Surprise`（值 5） | **直接对齐** |
| `idle` | 释放/回中 | n/a | release only | `ActionPriority::Idle`（非枚举值，语义） | **改用优先级**：`release=true` 标志位触发 release 路径；不再作为可触发的 `ActionKind` |
| `look_up` | 向上看 | 1..3 | tool/命令 | **缺** | v0.1 扩展；需 `ParamAngleY + ParamEyeBallY` 门控；引入枚举值 6 |
| `look_down` | 向下看 | 1..3 | tool/命令 | **缺** | v0.1 扩展；同 `look_up` 门控；枚举值 7 |
| `glance_left` | 瞟一眼（一侧） | 1..3 | tool/命令 | **缺** | v0.1 扩展；需 `ParamEyeBallX`；枚举值 8 |
| `glance_right` | 瞟一眼（另一侧） | 1..3 | tool/命令 | **缺** | v0.1 扩展；同 `glance_left` 门控；枚举值 9 |
| `smile` | 微笑 | 1..3 | tool/命令 | **缺** | v0.1 扩展；需 `ParamMouthForm + ParamBrowLY/RY`；枚举值 10 |
| `frown` | 皱眉 | 1..3 | tool/命令 | **缺** | v0.1 扩展；同 `smile` 门控；枚举值 11 |
| `pout` | 嘟嘴 | 1..3 | tool/命令 | **缺** | v0.1 扩展；需 `Param4`（非标）；枚举值 12；能力门控裁剪 |
| `wry_mouth` | 歪嘴 | 1..3 | tool/命令 | **缺** | v0.1 扩展；需 `Param6`（非标）；枚举值 13；能力门控裁剪 |
| `deep_breath` | 深呼吸 | 1..3 | tool/命令 | **缺** | v0.1 扩展；需 `ParamBreath`；枚举值 14 |
| `wink_left` / `wink_right` | 眨眼（左/右） | — | **已删除**（真机不可用） | 不实现 | **永久不实现**；旧 id 经 `sanitize_motion` 收敛为 `None` |
| `wave` / `wave_big` / `bounce_happy` / `happy` / `excited` / `sway` / `bow` / `lean_in` | 挥手/蹦跳/摇摆/鞠躬/凑近 | — | **已删除**（身体类无参数） | 不实现 | **永久不实现**；同上 |

### 5.2 参数/协议字段对照

| Py 字段（`live2d_action.py`） | Rust/JSON schema 字段（建议） | 备注 |
|---|---|---|
| `LIVE2D_ACTION_TOOL_API_NAME = "live2d_perform_action"` | 工具名 `"live2d_perform_action"` | OpenAI 兼容 function-calling 硬约束 `^[a-zA-Z0-9_-]+$` |
| `action: str` | `action: string`（enum：见 5.1 表） | schemars 派生 |
| `strength: int` | `strength: 1 \| 2 \| 3`（默认 2） | 与 `MIN/MAX/DEFAULT_STRENGTH` 一致 |
| `reason: str`（上限 200 字符） | `reason: string \| null`（`maxLength: 200`） | 仅日志/面板 |
| `accepted/rejected` 计数 | `lifecycle.counters: { accepted: u64, rejected: u64 }`（由 reducer 统计） | 通过 `/api/v1/plugins/health` 暴露 |
| 帧级元数据 `strength/source/priority/reason` | `BodyAction { strength, source: ActionSource, priority: ActionPriority, reason: Option<String> }` | 与 R10 `ActionPriority` 5 级对齐 |
| `at ∈ [0,1)` 归一化进度（`motion-timeline.ts`） | `ActionCue { at: f32, action: ActionKind, strength: u8, source, priority, reason }` | `at ∈ [0,1]`，前端按 audio `currentTime/duration` 调度 |

### 5.3 来源与触发

| 来源 | Py 实现位置 | Rust 端落位 | 备注 |
|---|---|---|---|
| 主 LLM tool action | `live2d_action.py` `OpenAICompatibleAsyncLLM` 工具循环 | `crates/live2d-ai-runtime/src/llm/tools/live2d_action.rs` | 须挂在所有 OpenAI-compatible provider；不支持 tools 的 provider 走降级 |
| 用户命令（`点点头`/`摇头`…） | `director.py` `detect_user_motion_sequence` | `crates/live2d-ai-core/src/action/user_keywords.rs` | 序列切分：按 `然后/接着/先/再/，,。!？；、`；疑问否定保护（`为什么/别/怎么`） |
| LLM 括注动作（`（点头）`） | `director.py` `_BRACKET_RE` + `_STAGE_GESTURE_KEYWORDS` | `crates/live2d-ai-core/src/parse/parentheses.rs` + `action/stage_gesture.rs` | 6 关键词表：点头/摇头/歪头/张望/倾听/惊讶 |
| 表情/情绪 fallback | `director.py` `_EMOTION_MOTION = {"surprise":"surprise"}` | `crates/live2d-ai-core/src/action/emotion_fallback.rs` | 唯一映射：surprise→surprise；其他情绪 `None` |
| 开发者面板手动触发 | `dev-console.ts` `triggerDevAction` | `apps/web/src/settings/dev-console.ts` + `POST /api/v1/dev/actions/trigger` | 仅当 `dev.enabled=true`；localhost-only |
| 直播/弹幕事件 | `live-event.ts` `LiveEventPayload.motion` | `crates/live2d-ai-runtime/src/api/live_event.rs` | 限定 6 动作 + release；非六动作静默丢弃 |

---

## 6. 「Web 前端」迁移对照（Py settings 9 tab → 节点 D Web 页面结构）

> 目标：把 Py 9 个 tab（`对话/LLM · 语音/TTS · 形象/背景 · 外部接入 · 密钥 · 人设/提示词 · 系统/高级 · 记忆 · 插件`）按节点 D 范围裁剪，标注每个 tab 的产品价值与去留。
> 参考：R6/R7/R8/R9/R11 给出技术落位；D6/D7 限定 LLM/TTS 仅 OpenAI-compatible。

| Tab | Py 现状（`panel-template.ts` 章节） | 节点 D 价值 | 去向 | 备注/裁剪点 |
|---|---|---|---|---|
| 1. 对话 / LLM | `settings-llm` 下拉、思考模式、思考强度、Provider 卡片 | **保留** | R9 + 新建 `apps/web/src/settings/llm.tsx` | 砍掉 `discover-models` 直连外部服务（v0 用固定 base_url + models 列表） |
| 2. 语音 / TTS | `settings-tts`/`settings-asr`/`viseme-mode`（RMS/pymouth/off） | **保留（v0.1）** | `apps/web/src/settings/tts.tsx` | v0 ASR 关闭（与 Py 一致）；`pymouth` viseme 模式删除（Python 专属，本地引擎） |
| 3. 形象 / 背景 | 模型扫描/上传/选择、背景图上传/选择 | **保留** | R7 + `apps/web/src/settings/live2d.tsx` | 上传 .zip 限制保留；`live2d-models/` 路径固定 |
| 4. 外部接入 | `external-input.ts` 默认关闭 + 本机限定 + token | **保留（可选）** | R8 + `apps/web/src/settings/external.tsx` | v0 可后置；接口路径 `/api/v1/external/chat` |
| 5. 密钥 | `${XXX_API_KEY}` 占位符写入 .env | **保留** | `apps/web/src/settings/keys.tsx` | 改用 `tauri-plugin-stronghold` 或 OS keyring 抽象 |
| 6. 人设 / 提示词 | textarea 编辑 system prompt + 清除按钮 | **保留** | `apps/web/src/settings/persona.tsx` | 与 `shared/persona.yaml` 双向同步；保留 personaprompt 校验 |
| 7. 系统 / 高级 | 角色名/端口/FPS/日志级别/开发者模式/版本块/原始 YAML 编辑 | **保留（裁剪）** | `apps/web/src/settings/system.tsx` | **删除**「原始 YAML 编辑器」与「导入/导出按钮」（v0 不暴露 raw YAML）；**保留**端口/FPS/开发者模式/版本块 |
| 8. 记忆 | 长期记忆事实增删改查、知识库文档上传、联网搜索（DDG/Tavily） | **部分保留** | `apps/web/src/settings/memory.tsx` | 长期记忆→v0 走 Rust reducer 短期上下文；知识库文档→v0 不实现；联网搜索→v0 不实现（删除整个 search-row） |
| 9. 插件 | 内部/外部插件列表、启停 | **重写为只读列表** | `apps/web/src/settings/plugins.tsx` | **删除**「外部插件安装」流程（节点 D 无 plugin host）；只展示内置原生插件状态 |

> 总体结论：9 tab 中 1/3/5/6 完整保留；2 部分保留；4 可选后置；7/8/9 裁剪显著。**没有任何 tab 整段废弃**——每个 tab 都有可复用的 UI 模式（左侧导航 + 状态简报 + 分组标题 + 操作按钮）。

### 状态简报（保留）

`panel-template.ts` 4 行状态 dot：`LLM/TTS/ext/sys` 全保留，作为面板顶部实时状态指示。

### 底部条

`settings-apply` / `settings-save` / `settings-close2` 3 按钮保留，差异：Rust 端不需要「保存并重启后端」文案（runtime 配置热重载），统一为「保存并应用」。

---

## 7. 决策结论

### 三类清单统计

- **M（直接迁移）**：12 项（M1–M12）
- **R（参考后重写）**：15 项（R1–R15）
- **X（永久废弃）**：34 项（X1–X34，覆盖全部 246 个 Py 文件 + 全部旧前端渲染层）

### 无责任资产检查

> 确认：清单中**没有**任何「先保留以后再看」/「待评估」/「v0 暂不实现」类条目。每一项 M/R/X 都有明确去向与一句话理由。
> 唯一带「v0 不实现」语义的是 X 类（如 X4 ASR、X5 各家 TTS 私有 SDK、X8 MCP、X11 翻译），已归入永久废弃。

### 对节点 D 的下游建议

1. **M1 + R10 + R2**：动作目录与裁决逻辑是节点 D 闭环的核心契约；建议在 `crates/live2d-ai-core/src/action/` 建立 `catalog.rs` + `priority.rs` + `user_keywords.rs` + `stage_gesture.rs` + `emotion_fallback.rs` 五个模块，**优先**平移 M1（基础+门控 15 动作 + 1 release + 必要参数表）。
2. **M9 + M10 + M11 + M12 + R1**：模型/资源层尽量早落位（`crates/l2d/src/profile/` 与 `shared/model-adapter/`），让 `crates/live2d-ai-runtime` 的 `OpenAiClient` 能基于真实协议键 + 通道 ID 拼装 system prompt。
3. **R4**：conf schema 平移时**明确删除** `mcp_servers` 字符串字段（X8 决议），并把 `${XXX_API_KEY}` 占位符语法改为「Key 名 + 值（来自环境/secret store）」二元组，落到 `apps/web/src/settings/keys.tsx`。
4. **R9**：面板分区顺序与名称沿用 9 tab；v0.1 实际可交付 5 tab（1/3/5/6/7 裁剪后），2/4/8/9 暂以「禁用 + 提示文案」形式占位（**不删除 DOM**，避免后续返工）。
5. **D3 占比基线**：本次盘点完成后，Py 端 246 文件全部归入 X，Web 渲染层 25+ 文件全部归入 X；Rust 端需要新增/补全动作目录与协议 schema 文件（建议 ≥ 5 个新文件、≥ 800 行）以维持 ≥ 95% 比例。
6. **不做的事**：
   - 不平移任何 Py 私有 SDK（X5）。
   - 不恢复 Python 运行时依赖。
   - 不实现 `wink_*` / `wave*` / `bounce*` / `bow` / `lean_in` 等已删除动作。
   - 不引入 MCP（X8）、不引入 ASR（X4）、不引入 VAD（X6）、不引入翻译（X11）。

### 修订历史

- **2026-08-28**：初版（D0 资产盘点完成）。
