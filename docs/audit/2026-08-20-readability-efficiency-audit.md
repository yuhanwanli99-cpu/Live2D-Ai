# Live2D-Ai — 代码可读性与高效性审计（2026-08-20）

> 本轮范围：**只审可读性与高效性**（不做安全/合规专项；安全问题只在顺手发现时一句话提及）。
> 与上一轮（`docs/audit/2026-08-19-code-audit-round.md`）衔接：上一轮是「常规审计 + 管理收尾」，
> 已落地一批低风险修复并归档报告；本轮聚焦**纯可读性/性能**，避免重复取数——上一轮已记录的
> 「.env 非原子写 / discover-models SSRF / port 死配置」等项在本轮只做状态更新，不重述结论。
>
> 审计对象只覆盖**本项目自研/改造过**的文件（依据 `git log --name-only` 统计：`routes.py`、`settings-ui.ts`
> 等反复改动文件），**不审上游未改动的第三方代码**。

## 1. 审计基线（本次实际执行）

| 项目 | 命令 / 结果 |
| --- | --- |
| PC 后端语法 | `python3 -m py_compile src/open_llm_vtuber/routes.py` → **通过**（`PY_COMPILE_OK`） |
| PC 后端 lint | `ruff check .` → **环境无 ruff**（`command not found`，未执行） |
| PC renderer 类型 | `cd renderer && npx tsc --noEmit` → **通过**（`TSC_EXIT=0`） |
| PC renderer 单测 | `npx vitest run` → **252/252 通过**（11 个测试文件，1.88s） |
| 模型注册表校验 | `python3 shared/validate_registry.py` → **通过**（1 entry，schema + 资源文件全过） |
| 跨端一致性 | `python3 shared/check_cross_platform_consistency.py` → **32/33 通过，1 失败**（C1：Android `assets/model_registry.json` 缺失——Gradle clean 后未重跑构建，该文件由 `copyModelRegistry` 生成，非代码缺陷） |
| Android 编译 | 本环境无 Android SDK/AGP 编译链 → **未编译**，仅静态抽样（同上一轮） |
| PC 完整 pytest | 环境缺 Python 依赖 → **未跑**（仅 `py_compile` 语法层） |

> 说明：上述「未执行」项均如实记录，未编造结果。

---

## 2. 可读性发现

> 每条：**编号 / 文件:行 / 现象 / 为什么影响可读性 / 建议改法（最小 diff 思路）/ 风险 / 成本**。
> 风险=高/中/低；成本=S(小,<1h)/M(中,1-4h)/L(大,>4h)。

### R-1 · `routes.py:24` — `init_config_routes` 是 1482 行的巨型函数
- **现象**：`init_config_routes` 从 L24 到 L1482 结束，内部定义了 **35 个缩进的 `def`/`async def`**（~20 个辅助函数 + 17 个路由处理器），挂 **21 个 `@router.*` 装饰器**；LLM/TTS/Provider 目录（`LLM_DEFAULTS`/`MODEL_CATALOG`/`TTS_VOICE_CATALOG`/`TTS_DEFAULTS` 等 6 张大表，L47-214）全部堆在函数体内。
- **为什么影响**：文件只有一个「真入口」，任何人改一个路由都要在 1400 行的缩进里定位；函数体闭包捕获 `config_abs`/`repo_root`/`env_path` 等外层变量，单元测试无法直接 import 单个 handler；目录表与函数强耦合无法独立复用。
- **建议改法（最小 diff 思路）**：把 6 张目录表抽到模块级常量（或独立 `provider_catalog.py`）；把 `_load_yaml`/`_save_yaml`/`_write_raw_atomic`/`_load_persona`/`_env_value`/`_parse_models_response`/`_llm_protocol` 等无状态辅助函数提到模块级（把 `config_abs` 等路径改为参数）；把 `/api/config`、`/api/persona`、`/api/config/tree`、`/api/models*`、`/api/llm/*`、`/api/test-*` 各自拆成独立 `APIRouter` 工厂函数。逐次迁移，不要求一次到位。
- **风险**：中（拆分是纯机械移动，但改动面大，需回归测试兜底）｜**成本**：L

### R-2 · `routes.py:549-554 / 625-630 / 649-654 / 1399-1404` — TTS provider id 归一化字典重复 4 次
- **现象**：`{"gpt_sovits": "gpt_sovits_tts", "piper": "piper_tts", "sherpa_onnx": "sherpa_onnx_tts", "pyttsx3": "pyttsx3_tts"}` 这同一个映射在 `update_config`（L549、L625、L649）和 `test_tts`（L1399）各写了一遍。
- **为什么影响**：同一概念四处重复，未来新增/改名 TTS provider 极易漏改其中一处导致别名解析漂移（历史已有 `gpt_sovits`/`gpt_sovits_tts` 双拼别名问题）。
- **建议改法**：提一个模块级常量 + 辅助函数，例如
  ```python
  _TTS_ID_ALIASES = {"gpt_sovits": "gpt_sovits_tts", "piper": "piper_tts",
                     "sherpa_onnx": "sherpa_onnx_tts", "pyttsx3": "pyttsx3_tts"}
  def _normalize_tts_id(pid: str) -> str:
      return _TTS_ID_ALIASES.get(pid, pid)
  ```
  四处 `{...}.get(requested_tts, requested_tts)` 统一替换为 `_normalize_tts_id(requested_tts)`。
- **风险**：低｜**成本**：S

### R-3 · `routes.py:293-295 / 573 / 591 / 1292-1293` — `_normalize_provider_id` 已存在却未被统一使用
- **现象**：`_normalize_provider_id()` 在 L293 定义（`openai_compatible` → `openai_compatible_llm`），但只被 L305、L1099 调用；L573、L591 仍在原地内联 `"openai_compatible_llm" if raw_pid == "openai_compatible" else raw_pid`，L1292-1293 在 `test_llm` 又写了一遍。
- **为什么影响**：helper 存在但三处不用，读者会以为内联逻辑与 helper 有语义差异（实际没有），制造「为何不一致」的认知负担。
- **建议改法**：L573、L591、L1292 三处统一改为 `pid = _normalize_provider_id(raw_pid)`。
- **风险**：低｜**成本**：S

### R-4 · `routes.py:1122-1137 / 1211-1223 / 1327-1376` — LLM 协议（openai/anthropic/ollama）URL 与 header 构造重复 3 次
- **现象**：`discover_llm_models`、`check_providers`、`test_llm` 三个端点各自重复实现「判断 `_llm_protocol` → 拼 `/models` 或 `/api/tags` 或 `/v1/messages` → 组装 `Authorization`/`x-api-key` header」。
- **为什么影响**：三份近似逻辑，anthropic 的 `/v1` 去重（L1344-1348）只存在于 `test_llm`，discover/check 没有——协议处理已经出现「同 provider 不同路径」的漂移苗头。
- **建议改法**：抽 `_build_models_url(provider, base_url) -> str` 与 `_build_auth_headers(provider, key) -> dict` 两个 helper，三处复用；anthropic 的 `/v1` 归一化收敛到同一个 helper 后三处行为自动一致。
- **风险**：中（漂移会直接导致某个端点对某 provider 失效）｜**成本**：M

### R-5 · `websocket_handler.py:32-45` — `MessageType` 枚举定义后从未使用
- **现象**：`MessageType`（GROUP/HISTORY/CONVERSATION/CONFIG/CONTROL/DATA）枚举定义在 L32-45，但 `_init_message_handlers()`（L76-98）的路由表用的是裸字符串字面量；`grep MessageType` 全仓库只命中定义行。
- **为什么影响**：同一份「消息类型」有两个真相源（枚举 vs 字符串 dict），枚举成了死代码，误导读者以为类型是受控的。
- **建议改法**：要么删除 `MessageType`，要么把路由表键改为 `MessageType.xxx.value`（并让 `_route_message` 用枚举校验）。二选一，不要两存。
- **风险**：低｜**成本**：S

### R-6 · `routes.py:500 / 562-563` — `system_config.port` 仍是死配置（状态更新）
- **现象**：`_get_common` 读回 `port`（L500，默认 12393），`update_config` 写回 `port`（L562-563），但 `run_server.py` 仍硬编码 `12393`。**上一轮已记录，本轮确认仍未修复**。
- **为什么影响**：UI 给用户「改了端口」的假象，实际重启后不生效。
- **建议改法**：`run_server.py` 从 `system_config.port` 读端口，或在 UI 上明确标注「端口需改 run_server.py」。不属本轮纯可读性范围，仅登记。
- **风险**：中（功能误导）｜**成本**：S

### R-7 · `service_context.py:76/122/131/229/248/285/289` — `mcp_server_registery` 拼写错误
- **现象**：属性名 `mcp_server_registery`（registry 错拼为 registery）贯穿 7 处，而类名是 `ServerRegistry`。
- **为什么影响**：同一概念两种拼写，IDE 搜索/跳转断裂，且与类名不一致。
- **建议改法**：全局重命名 `mcp_server_registery` → `mcp_server_registry`（纯机械替换，含 `load_cache` 形参）。属上游改名遗留，本轮只登记。
- **风险**：低｜**成本**：S

### R-8 · `service_context.py:108` — `__str__` 里 VAD 行的标签是复制粘贴错误
- **现象**：L108 `f"    Agent Config: {json.dumps(self.character_config.vad_config.model_dump(), ...)}"` 实际 dump 的是 `vad_config`，标签却写成 `Agent Config`（与 L106 的 agent 行重复）。
- **为什么影响**：调试输出误导——排查时看到两个 "Agent Config" 而 VAD 配置无法辨识。
- **建议改法**：标签改为 `VAD Config`。
- **风险**：低｜**成本**：S

### R-9 · `service_context.py:49 vs 682-692` — MCP 路径解析与 shared 路径解析风格不一致
- **现象**：`_resolve_mcp_servers_path()`（L49）用相对路径 `os.path.join("..", "..", "shared", "mcp_tools.json")`，**依赖 CWD**；而同文件 `_find_repo_shared_dir()`/`_resolve_shared_path()`（L665-692）刻意做成「不依赖 CWD，逐级向上找 shared/」。注释也明说「不依赖 CWD」。
- **为什么影响**：同一文件里两套路径策略，启动目录不同时 MCP 配置可能静默找不到；`_resolve_mcp_servers_path` 已有的 CWD 无关工具（`_find_repo_shared_dir`）却没用上。
- **建议改法**：`_resolve_mcp_servers_path` 复用 `_resolve_shared_path("shared/mcp_tools.json")`，回退 `"mcp_servers.json"`。
- **风险**：低｜**成本**：S

### R-10 · `service_context.py:60-62/69-71/83/90-91` — 类型标注风格新旧混用
- **现象**：`self.config: Config = None`、`self.system_config: SystemConfig = None`、`self.send_text: Callable = None` 等用「旧式 `Type = None`」，而同文件 L68/72/74 已用「新式 `Type | None`」。
- **为什么影响**：`Type = None` 对静态检查器是错误标注（实际可为 None），新旧混用让类型声明不可信。
- **建议改法**：统一为 `Config | None`（Python 3.10+）。文件顶部已 `from typing import Optional`，也可统一 `Optional[Config]`，二选一保持一致。
- **风险**：低｜**成本**：S

### R-11 · `main.ts:266-302` — S3 渲染日志探针（~36 行 console.log）内嵌在模型加载函数
- **现象**：`loadAndAttach` 里嵌了一段「S3 渲染日志探针」（L266-302），遍历全部 drawable 顶点求包围盒并连打 3 条 `console.log`，注释自述「只读：不改渲染逻辑，仅 console 输出」。
- **为什么影响**：调试代码混入生产加载路径，稀释了 `loadAndAttach` 的主线（加载→换模型→相机→纹理检查）；遍历顶点是额外计算。
- **建议改法**：把探针抽成 `logRenderProbe(model)` 独立函数，用 `if (DEBUG_RENDER_PROBE)` 门控（默认 false），或在确认验收完成后删除。
- **风险**：低｜**成本**：S

### R-12 · `settings-ui.ts:196-1350` — `initSettingsPanel` 单函数 1350 行、~40 个嵌套闭包
- **现象**：整个设置面板是 `initSettingsPanel(container, opts)` 一个函数，内部 ~40 个 `const xxx = () => {...}` 闭包（render 系列、readUiState、save、discover/test/check/scan/select、playAudio、事件接线全部闭包捕获 `overlay`/`currentConfig` 等）。
- **为什么影响**：与 R-1 同构——单点巨型函数，无法单测单个 render/save 流程（纯逻辑虽已抽到 settings-logic.ts，但 DOM 装配仍不可测）；任何修改都要在 1150 行里定位。
- **建议改法**：把「卡片构建」（buildLlm/TtsProviderCard、renderKeyCards）抽为模块级函数（入参容器 + 数据），把 save/discover/test 等异步流程抽成接收 `ctx` 对象的独立函数。可先抽卡片渲染（收益最高，见 R-13）。
- **风险**：中｜**成本**：L

### R-13 · `settings-ui.ts:465-487/552-574 与 489-550/576-627` — LLM/TTS 卡片渲染与建卡逻辑成对重复
- **现象**：`renderLlmCards` 与 `renderTtsCards` 的「分组 → 空态 → 逐组建卡」结构几乎逐行一致；`buildLlmProviderCard` 与 `buildTtsProviderCard` 的「row + badge + toggle 展开编辑器 + 事件接线」结构也高度重复，仅字段名（llm-*/tts-*、voice/model/ws_url）不同。
- **为什么影响**：两套并行实现漂移风险高（已有小差异：LLM 卡有「获取可用模型/测试连接」两个按钮，TTS 卡只有一个「试听」）；新增 Provider 字段要改两处。
- **建议改法**：抽象 `buildProviderCard(p, kind: 'llm'|'tts', fieldDef)`，字段集由 `kind` 决定；分组渲染抽 `renderGroupedCards(container, categories, items, builder)`。
- **风险**：低｜**成本**：M

### R-14 · `settings-ui.ts:370-392 vs 981-1045` — busy + last-click-wins 竞态逻辑有两套实现
- **现象**：`withBusy()`（L370）封装了「token 递增 + spinner + 恢复按钮 + 仅最新 token 更新状态」，被 saveTree/discover/test/check/scan 复用；但 `save()`（L981-1045）**自己又写了一遍**几乎相同的 token/spinner/恢复逻辑，未复用 `withBusy`。
- **为什么影响**：两套竞态防护语义需保持同步（`withBusy` 的 finally 用 `token === requestToken` 恢复，`save` 的 finally 用「无条件恢复」——行为已分叉），维护者需读两遍才能确认一致。
- **建议改法**：`save` 改用 `withBusy`，把「无条件恢复按钮」的差异作为 `withBusy` 的一个参数（如 `alwaysRestore: true`）。
- **风险**：低｜**成本**：S

### R-15 · 多处「兼容别名」疑似死代码（跨 PC-TS / Android-KT）
- **现象**（`grep` 验证均无调用方，仅定义）：
  - `idle-blink.ts:149-152` `blinkNow()`、`:220-221` `export const BlinkController = BlinkStateMachine`；
  - `emotion-consumer.ts:56` `EMOTION_KEYS`、`:109` `EMOTION_PARAM_MAP`；
  - `LoopCoordinator.kt:101` `dispatch()`、`:230-264` `LoopDependencies` 的 `llm/expr/speech/convergeDelaySeconds` 别名属性；
  - `param-arbiter.ts:241/249` `getValue()` / `getParameterValues()`（连测试都未用，只有 `getCurrentSource` 被 rm3 测试用）。
- **为什么影响**：这些「历史命名兼容」别名是分散在各文件里的噪音，读者无法判断哪些是活跃 API、哪些是死代码；积累下去会让「真正的改动点」更难定位。
- **建议改法**：`grep` 确认无引用后直接删除；确需对外兼容的（如 `BlinkController`）加 `@deprecated` 注释并给出删除日期。
- **风险**：低｜**成本**：S

### R-16 · `settings-logic.ts:216` — `buildModelOptions` 存在死分支
- **现象**：L211-215 已用 `[current, ...list]` 去重 push（current 非空必被加入），L216 `if (all.length === 0 && current) all.push(current);` 是永远走不到的冗余兜底。
- **为什么影响**：误导读者以为有「current 未被加入」的分支。
- **建议改法**：删除 L216。
- **风险**：低｜**成本**：S

### R-17 · `websocket_handler.py:160-171 vs 603-613` — `set-model-and-conf` 载荷构造重复
- **现象**：`_send_initial_messages` 与 `_handle_init_config_request` 各自构造一份 `{"type":"set-model-and-conf", model_info/conf_name/conf_uid/client_uid/live2d_fps}`。
- **为什么影响**：字段增删要改两处，漏改则初始下发与重连下发不一致。
- **建议改法**：抽 `_set_model_and_conf_payload(ctx, client_uid)` helper，两处复用。
- **风险**：低｜**成本**：S

### R-18 · `routes.py:1035 vs 1575` — `live2d-models` 目录解析与「扫描」逻辑重复
- **现象**：`scan_models` 用 `os.path.join(os.getcwd(), "live2d-models")`（L1035，依赖 CWD），`get_live2d_folder_info` 用相对 `"live2d-models"`（L1575）；两个端点各自 `os.scandir` 遍历模型目录并产出不同结构，`model3` 文件名拼接、`folder.replace("\\","/")` 等逻辑重复。
- **为什么影响**：同一「扫描 live2d-models」概念两处实现且路径基准不一致（CWD 依赖 vs 相对）。
- **建议改法**：抽一个 `_iter_local_models(live2d_dir) -> list[dict]`，两个端点只做输出结构差异。
- **风险**：低｜**成本**：M

### R-19 · 魔法数字/字符串散落（`44`/`32768`/`12393`/`0.8`/`120`）
- **现象**：WAV 头 `44`（`routes.py:1628/1632`）、`32768.0`（`:1643`）、默认端口 `12393`（`routes.py:500`、`settings-ui.ts:167/793`、`main.ts:71`）、默认温度 `0.8`（`routes.py:434/507/604/910`、`settings-ui.ts:174`）、默认 FPS `120`（`routes.py:501`、`settings-ui.ts:168/794`）等散落多处，前后端各有一份默认值。
- **为什么影响**：默认值双端漂移（改后端默认没改前端 `EMPTY_CONFIG`，或反之）会导致「空配置」与「服务端默认」不一致。
- **建议改法**：前后端各自定义具名常量（`DEFAULT_PORT`/`DEFAULT_FPS`/`DEFAULT_TEMPERATURE`）；`44`/`32768.0` 抽 `WAV_HEADER_SIZE`/`PCM_INT16_SCALE`。
- **风险**：低｜**成本**：S

### R-20 · `spoq-fix` 历史注释核查结论
- **现象/结论**：全仓 grep `spoq-fix` 命中 107 处，本轮抽查了 PC renderer 与后端的活跃 `spoq-fix` 注释——`ws-bridge.ts:124/256`（音频帧 base64 WAV 播放）、`main.ts:423`（双端对话框）、`camera.ts:132`（fit 返工）、`server.py:46`（禁用缓存）等**注释与当前代码相符，未发现已过期**；其余集中在 Android/测试文件（`AnimationSystem.kt`、`Live2DRenderer.kt` 等）与 `.pi/spoq/` 历史文档引用，不在本轮 PC 热点范围。
- **为什么影响**：确认无需清理，避免误删仍在生效的历史修复说明。
- **建议**：维持现状；未来若重构相关段落，再同步更新注释。
- **风险**：低｜**成本**：—

---

## 3. 高效性发现

### E-1 · `routes.py:1138/1224/1331/1349/1364` — 同步 `requests` 阻塞 async 事件循环（高危）
- **现象**：`discover_llm_models`、`check_providers`、`test_llm` 三个 `async def` 端点内用同步 `requests.get/post`。尤其 `check_providers`（L1183-1251）在 async 路径里**串行**遍历 ~16 个 LLM provider，每个 `timeout=8`，最坏约 128s 全程阻塞事件循环。
- **为什么影响**：FastAPI 是单线程事件循环；一个用户点「全部检查可用性」期间，**所有**已连接 WebSocket 会话（对话/口型/字幕/心跳）与其它 HTTP 请求全部卡死——这是本项目最实打实的性能/可用性缺陷。
- **建议改法**：最小改动是 `await asyncio.to_thread(_requests.get, url, headers=..., timeout=...)`（Python 3.9+），把阻塞 I/O 移出事件循环；更好是并发——`check_providers` 用 `asyncio.gather(*(asyncio.to_thread(...) for pid in ...))` 并行探测（限并发量，如信号灯 4）。长期可换 `httpx.AsyncClient`。
- **风险**：高｜**成本**：M

### E-2 · `routes.py:389-401/403-404/437/470` — `.env` 文件每次读 key 都全量重读
- **现象**：`_env_value(name)` 每次调用都 `open(env_path)` 逐行扫描；`_has_key` 包装它；`_get_common` 对**每个** LLM provider（~16 个）和 TTS provider（~13 个）各调一次 `_has_key`（L437、L470）→ 一次 `GET /api/config` 约重读 `.env` 29 次。
- **为什么影响**：单次请求约 29 次小文件 open+read+close（`update_config` 的 `normalized_config` 也再触发一轮）；文件虽小，但属纯无谓 I/O。
- **建议改法**：`_get_common` 开头 `env_map = _load_env_once()`（一次性读成 dict），provider 循环里直接 `bool(env_map.get(key_env))`；`_env_value` 保留但仅在单点场景（如 discover/test）使用，或也改为传入缓存。
- **风险**：低｜**成本**：S

### E-3 · `routes.py:538/683/801/820` — 单次请求重复解析 YAML/persona
- **现象**：`update_config` 一次请求内：L538 `_load_yaml()` → L683 `_load_persona()`（校验）→ L801 `_load_persona()`（落盘）→ L820 `_load_yaml()`+`_load_persona()`（`_get_common` 内又 `_load_persona()` L415）。即 **conf.yaml 解析 2 次、persona.yaml 解析 ~4 次**，每次都是 ruamel 全量 parse。
- **为什么影响**：conf.yaml/persona.yaml 随人设增长，重复解析放大写路径耗时（虽然对单用户项目量级仍小）。
- **建议改法**：`update_config` 开头一次性 `data = _load_yaml()`、`persona = _load_persona()`，后续校验/落盘/normalized 视图全部复用内存对象，最后统一写盘。
- **风险**：低｜**成本**：S

### E-4 · `websocket_handler.py:494-497 / 515-518` — `np.append` 逐 chunk 复制，O(n²) 累积（中高危）
- **现象**：`_handle_audio_data` 与 `_handle_raw_audio_data` 每收到一个音频 chunk 就 `self.received_data_buffers[uid] = np.append(buffer, new_array)`。`np.append` 每次都分配全新数组并拷贝旧内容。
- **为什么影响**：缓冲区随会话时长线性增长，总拷贝量 O(n²)；长时间对话（数分钟连续语音）下分配与拷贝开销持续放大，且产生大量垃圾数组压力 GC。
- **建议改法**：`received_data_buffers[uid]` 改为 `list[float]` 累积（`extend`），只在「mic-audio-end」取出时 `np.asarray(buf, dtype=np.float32)` 一次性转换；两处 append 逻辑也可合并为一个 `_append_audio(uid, values)`。
- **风险**：中｜**成本**：M

### E-5 · `param-arbiter.ts:160-177` — `update()` 每帧对所有参数重写 coreModel，即使值未变
- **现象**：`update(deltaMs)` 遍历 `paramStates`，对每个参数无论是否在 fade、值是否变化，都执行 `coreModel?.setParameterValueById?.(paramId, state.currentValue)`。fade 完成后的稳定参数（如 mood 基线、表情到位后）仍每帧重写。
- **为什么影响**：渲染循环里每帧对 ~10-20 个参数做无谓的跨 JS↔Cubism 写入；虽单次便宜，但属「每帧固定开销 + 不必要调用」的典型模式，参数越多越明显。
- **建议改法**：给 `ParamState` 加 `dirty` 标志（`write`/`activate`/`fadeTo` 置位，`update` 写后清除），`update` 里 `if (!state.dirty && state.fadeMs === 0) continue;`。fade 进行中的自然每帧更新。
- **风险**：低｜**成本**：S

### E-6 · `settings-ui.ts:802-829` — 保存/打开即全量重建 Provider/Key 卡片 DOM
- **现象**：`populate()` 每次被调用（打开面板、每次保存成功后的 re-populate）都 `renderLlmCards()`+`renderTtsCards()`+`renderKeyCards()`，三者均 `innerHTML=''` 后重建全部 ~50 个节点并重绑事件。
- **为什么影响**：设置面板非每帧热路径，绝对开销可接受；但「每次保存整页重绘」会丢失用户正在编辑但未保存的字段焦点/内容（key 输入框尤其明显——服务端不回传 key 明文，保存后 key 框被清空），属可读性之外的 UX 副作用。
- **建议改法**：save 成功后仅回填 `has_key`/`configured`/状态简报等「服务端权威字段」，不整体重渲染；或为 Key 卡做「不覆盖已填未提交值」的合并策略。
- **风险**：低｜**成本**：M

### E-7 · `idle-blink.ts:188-207` / `idle-breath.ts:56-63` — 每帧 `arbiter.write` 分配小对象
- **现象**：呼吸每帧 `write`（L56）构造 `ParamWrite` 对象字面量 + `SourceRecord` + 2 次 Map 操作；眨眼闭眼期每帧两次 `write`（左右眼）。60fps 下 = 每秒几十个短生命周期对象。
- **为什么影响**：绝对量小（现代 JS 引擎可承受），但属「渲染循环内持续分配」的可避免开销；且 `fadeMs=0` 的写走完整仲裁流程，对高频源（呼吸/口型）无必要。
- **建议改法**：在 `ParamArbiter.write` 加 `fadeMs===0 && source 已是 activeSource` 的 fast path（直接改 `currentValue`，跳过 `activate` 的 from/to/elapsed 赋值）；或呼吸用「只在值变化超过阈值才写」。
- **风险**：低｜**成本**：S

### E-8 · `ws-bridge.ts` — 无重复 JSON 序列化（正面核实）
- **现象/结论**：`sendText` 每条消息只 `JSON.stringify` 一次（L105）；`_dispatch` 只解析一次；字幕去重用 `Array.includes`（L251，20 元素窗口，O(n) 可忽略）；`_recentSubtitleTexts.shift()`（L197，20 元素）开销可忽略。**未发现任务提示的「WS 处理里重复 JSON 序列化」问题**，此项记为已核实通过。
- **风险**：—｜**成本**：—

### E-9 · `service_context.py:554-571` — `construct_system_prompt` 对 `mcp_prompt` 先读文件再丢弃
- **现象**：循环里 `group_conversation_prompt`/`proactive_speak_prompt` 在 `load_util` **之前** `continue`（正确跳过），但 `mcp_prompt` 的 `continue` 放在 `prompt_loader.load_util(prompt_file)` **之后**（L561 先读文件，L568-569 才跳过）——读了文件却丢弃内容。
- **为什么影响**：每次构造 system prompt 多一次无效文件读取。
- **建议改法**：把 `if prompt_name == "mcp_prompt": continue` 上移到 `load_util` 之前，与另两个 skip 并列。
- **风险**：低｜**成本**：S

---

## 4. Top 10 建议修复顺序（收益 / 成本排序）

| # | 编号 | 内容 | 收益 | 成本 | 理由 |
| --- | --- | --- | --- | --- | --- |
| 1 | E-1 | 同步 `requests` 改 `asyncio.to_thread` + `check_providers` 并行化 | 高 | M | 消除「点一次检查卡死全站」的最坏缺陷，收益最直接 |
| 2 | E-4 | `np.append` 改 list 累积，一次转换 | 高 | M | 长对话 O(n²) 拷贝，改动小、收益随会话时长放大 |
| 3 | R-2 + R-3 | TTS/Provider id 归一化去重 + 统一用 helper | 中 | S | 纯机械收敛，立刻消除最易漂移的四处重复 |
| 4 | E-2 + E-3 | `.env`/YAML/persona 单请求一次性读入复用 | 中 | S | 同一批文件重复解析，改动极小 |
| 5 | R-4 | 协议 URL/header 构造抽 helper（三端点复用） | 中 | M | 堵住 anthropic `/v1` 已出现的漂移苗头 |
| 6 | R-1 | `routes.py` 巨型函数拆分（目录表 + 辅助函数先提模块级） | 中 | L | 最高杠杆的结构债，建议分步做、不阻塞其它 |
| 7 | E-5 | `ParamArbiter.update` 加 dirty 标志 | 中 | S | 渲染循环固定开销，几行改动 |
| 8 | R-13 + R-12 | `settings-ui` 卡片渲染去重/泛化 | 中 | M/L | 消掉成对重复 + 巨型函数，为后续设置页改动铺路 |
| 9 | R-15 + R-5 | 删除死代码（MessageType 枚举、blinkNow/BlinkController/EMOTION_* 别名、getValue/getParameterValues、dispatch） | 低 | S | 零风险降噪，一次 grep 一次删 |
| 10 | R-7 + R-8 + R-9 + R-10 + R-19 | 命名/拼写/标注/魔法数字统一（registery、VAD 标签、路径风格、`Config | None`、常量） | 低 | S | 一批低风险一致性小修，适合一次打包提交 |

---

## 5. 本轮不建议动清单（附原因）

| 项 | 原因 |
| --- | --- |
| `routes.py` `update_config` / `update_config_tree` 的**校验逻辑去重**（上一轮已记为「易漂移」） | 两处校验各自带有「persona 注入 + field_errors 回传」的细微差异，合并需改响应契约，超出纯可读性范围，且易引入回归 |
| `_save_persona` 改原子写（上一轮遗留项） | 涉及 `shared/persona.yaml` 与 Android `assets/persona.yaml` 的**字节一致**契约（C23），改原子写需同步确认 Android 侧复制脚本仍产出同字节，属「跨端契约」改动而非纯 PC 可读性 |
| `_env_value` 自定义解析器 vs `dotenv` 去重（上一轮遗留） | 语义边界（`.`/引号/export 前缀等）已在上轮讨论过，本轮改会动 Key 读取的正确性，需专项验证 |
| `discover-models`/`test-llm` 的 SSRF/密钥外泄（上一轮高危） | 属**安全专项**，不在本轮范围；只登记：这些端点把存储 Key 作为 Bearer 发给客户端传入的任意 `base_url`，建议另开安全轮处理 |
| Android 端 `LocalLlmLink`/运行时传播面的单元测试补充（上一轮遗留） | 需 CI 编译链回归，本环境无 Android SDK，无法在本轮落地 |
| `settings-ui` 的「保存后不丢 key 输入框焦点/内容」UX 合并 | 属于产品行为决策（服务端不回传 key 明文是安全设计），需产品定调「未提交值是否保留」，不在本轮 |
| `MessageType` 枚举「改为受控校验」方向（只删不改为枚举路由） | 改为枚举驱动路由会扩大改动面；本轮建议只删死代码，不动路由结构 |
| `idle-blink` 的 `setEyeOpen` 左右眼双写合并 | 涉及 `ParamArbiter` 的 Blink 眼部特权语义与 rm3 测试断言，合并不当会破坏「闭眼不被表情打断」契约，需专项验证 |

---

## 附：数据核对说明

- 所有行号均经 `read` 工具逐行核对（`routes.py` 1730 行、`settings-ui.ts` 1350 行已完整读完；`service_context.py`/`websocket_handler.py`/`param-arbiter.ts`/`lip-sync.ts`/`emotion-consumer.ts`/`main.ts`/`ws-bridge.ts`/`chat-ui.ts`/`settings-logic.ts`/`idle/*` 全文已读）。
- 关键计数经 `grep`/`bash` 复核：`routes.py` 缩进 `def` 35 个、`@router` 21 个；TTS 归一化字典 4 处（L550/626/650/1400）；同步 `requests` 调用 5 处；`_normalize_provider_id` 仅 2 处调用、3 处内联。
- 死代码判定均以「`grep` 全仓仅命中定义行、含测试目录无引用」为准（`MessageType`、`getValue`、`getParameterValues`、`blinkNow`、`BlinkController`、`EMOTION_KEYS`、`EMOTION_PARAM_MAP`）。

---

## 附 2：本轮实际落地的修复（2026-08-20 同日，由实现侧回填）

> 报告本体由审计侧只读产出；以下是同日在**实现侧**真正合并的修复与撤回记录，便于下一轮审计对账。
> 全量说明见 `CHANGELOG.md` 首节。

| 编号 | 状态 | 落地内容 | 回归 |
| --- | --- | --- | --- |
| **E-1** | ✅ 已修 | `routes.py`：新增 `_requests_get/_requests_post`（`asyncio.to_thread`），`discover-models`/`providers/check`/`test-llm` 全部改为不阻塞事件循环；`check_providers` 改 `asyncio.gather` + `Semaphore(4)` 并发（保序），最坏耗时由 Σ(16×8s) 降为 max | pytest `test_settings_api.py` 30 passed |
| **E-2** | ✅ 已修 | 新增 `_load_env_map()`，`_get_common` 内 provider 循环改查内存（单请求近 30 次 `.env` 全量读 → 1 次） | 同上 |
| **E-3** | 🔶 部分 | `_get_common` 的 `.env` 重复读已消除；`update_config` 内 conf/persona 的多次 parse **未合并**（该路径含「校验候选配置 vs 落盘」两份语义，合并需动响应契约，沿用报告 §5「不建议动」判断） | — |
| **E-4** | ✅ 已修 | `websocket_handler`：新增 `_append_audio()`，缓冲改 `Dict[str, List[np.ndarray]]`；`conversation_handler` 在 mic-audio-end 处一次 `np.concatenate` 并清空 | 新增 `tests/test_audio_buffer.py`（顺序/清空/空缓冲/多会话/1000 chunk） |
| **E-5** | ⛔ 撤回 | dirty 标志已实现且 `rm3-arbiter` 29/29 通过，但它改变「每帧重申参数所有权」的语义（与 Cubism 内建 blink/breath/motion 写入竞争），**只能真机视觉确认**；本环境无 GPU/浏览器 → `git checkout` 撤回，留待真机 | — |
| **R-2** | ✅ 已修 | TTS id 归一化字典 4 处 → 模块级 `normalize_tts_id()`；`("tts_model","concurrency_limit","viseme_mode")` → 常量 `_TTS_NON_PROVIDER_KEYS` | pytest 261 passed |
| **R-3** | ✅ 已修 | `openai_compatible` 内联判断 3 处 → 统一 `_normalize_provider_id()` | 同上 |
| **R-4** | ✅ 已修 | 新增 `_models_endpoint()` / `_models_from_response()`，discover 与 check 共用（含 anthropic header 与 ollama `/api/tags` 分支） | 同上 |
| **R-5** | ✅ 已修 | 删除 `websocket_handler.MessageType` 死枚举（及随之无用的 `from enum import Enum`）；顺带删除被 env_map 取代的 `routes._has_key` | 同上 |
| **R-15** | ⏸ 未做 | 其余「兼容别名」死代码（`getValue`/`getParameterValues`/`blinkNow`/`BlinkController`/`EMOTION_*`）本轮未删：它们位于渲染热路径模块，与 E-5 同属「需真机确认无隐式依赖」的一类，建议与 E-5 一起在真机轮次处理 | — |
| R-1 / R-12 / R-13 | ⏸ 未做 | `routes.py` / `settings-ui.ts` 巨型函数拆分：本轮这两个文件都**同时在做功能改动**（外部接入、背景图、UI 令牌化），结构性拆分与功能改动叠加会让回归定位困难，故按报告建议留作独立轮次 | — |

### 本轮新增代码的自我约束（避免新增即新债）
- 新功能一律**独立模块**，不再往 `routes.py` / `settings-ui.ts` 里堆：
  后端 `external_input.py` / `backgrounds.py` / `local_state.py` / `api_key_resolver.py` / `agent/stateless_llm/misconfigured_llm.py`；
  前端 `external-input.ts` / `background.ts` / `toast.ts` / `icons.ts` / `theme.css` / `chat.css`。
- 两处落盘（外部接入开关、背景选择）共用 `local_state.JsonState` 的原子写，**不重复实现第三份** tmp+rename。
- 所有新增纯逻辑都有对应单测（后端 87 例、前端 35 例），且断言贴合真实契约而非实现细节。
