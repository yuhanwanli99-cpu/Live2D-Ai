# Live2D-Ai-pc — config_manager 数据模型 / YAML 往返审计报告

**审计范围**：`src/open_llm_vtuber/config_manager/` 全部 14 个文件（~2700 行）
**交叉参考**：`conf.yaml`、`conf.yaml.template`（磁盘配置）；`src/open_llm_vtuber/routes.py`（仅配置读/写路径）；`renderer/src/settings-ui.ts`（前端 payload / GET 映射）；以及运行时消费方 `run_server.py`、`service_context.py`、`agent/stateless_llm_factory.py`、`agent/transformers.py`、`free_glm_defaults.py`。
**方法**：静态阅读 + `pydantic 2.12.5` 实测（首次 max/never）。未修改任何文件。

---

## 0. 全局事实（schema 层，影响所有模块）

- 所有模型继承 `I18nMixin`，其 `model_config = ConfigDict(populate_by_name=True)`（`i18n.py:84`）。
- **没有任何模型设置 `extra=`** → 全部使用 pydantic v2 默认 **`extra='ignore'`**：YAML 中 schema 未建模的键在 `Config(**data)` / `model_validate` / `model_dump` 时被**静默丢弃**（这是下文多个发现的根因）。
- `Field(..., alias="x")` 的 alias 均等于 python 字段名，模块内无 alias/字段名漂移；`populate_by_name=True` 使两种拼写都接受。
- 实际写盘路径是 `routes.py` 的 `ruamel.yaml YAML(typ="rt")`（`routes.py:201-212`），会保留未知键与注释；`utils.save_config` 走 `model_dump + yaml.dump`，但不被任何代码调用（死代码，见 F-R2）。

---

## 1. 发现（Findings）

### 1.1 根模型与工具层 — `main.py` / `__init__.py` / `system.py` / `utils.py` / `i18n.py` / `live.py`

#### F-R1 【Medium】根模型 `Config` 不是 conf.yaml 的忠实镜像 —— 顶层 `mcp_servers` 未被建模（校验时被丢弃）
- `main.py:12-20`
  ```python
  class Config(I18nMixin, BaseModel):
      system_config: SystemConfig = Field(default=None, alias="system_config")
      character_config: CharacterConfig = Field(..., alias="character_config")
      live_config: LiveConfig = Field(default=LiveConfig(), alias="live_config")
      vision_config: Optional[VisionConfig] = Field(default=None, alias="vision_config")
  ```
- `conf.yaml:402`
  ```yaml
  mcp_servers: 'shared/mcp_tools.json'
  ```
- 为什么是缺陷：`mcp_servers` 是顶层 YAML 键，但 `Config` 无此字段；`extra='ignore'` 使其在 `Config(**config_dict)`（`run_server.py:138`）后**静默消失**。`run_server.py:135-136` 在原始 dict 上解析路径，值到不了模型；真正的 MCP 路径在 `service_context.py:_resolve_mcp_servers_path()` 被硬编码。当前能跑纯粹靠旁路，任何 schema 驱动的写入方（如 `save_config`）都会丢掉这个键。

#### F-R2 【Medium，潜在】`utils.save_config` 往返不安全，且是死代码；`read_yaml` 会做 `${VAR}` 环境替换造成“写回即泄密”隐患
- `utils.py:40-46`（read_yaml 环境替换）
  ```python
  pattern = re.compile(r"\$\{(\w+)\}")
  def replacer(match):
      env_var = match.group(1)
      return os.getenv(env_var, match.group(0))
  content = pattern.sub(replacer, content)
  ```
- `utils.py:107-124`（save_config）
  ```python
  config_data = config.model_dump(
      by_alias=True, exclude_unset=True, exclude_none=True
  )
  ...
  yaml.dump(config_data, f, allow_unicode=True)
  ```
- 为什么是缺陷（潜在）：`read_yaml` 读取后 `${ZHIPU_API_KEY}` 已替换为真实值/未解析原样；若走“读→模型→`save_config` 写”路径，模型 `model_dump`（`extra='ignore'`）会丢掉 `mcp_servers/models/voices/max_tokens/load_persona_from_file` 等未建模键、丢掉全部注释，并把真实 Key 明文写回 conf.yaml。当前 `grep src/` 显示 `save_config` **没有任何调用者**（`routes.py` 用 ruamel 直写且会注入 persona），所以是“埋着的雷”，不是正在发生的数据丢失。

#### F-R3 【Low】`live2d_fps` 默认值三处不一致：schema 60 vs 路由/前端 120
- `system.py:16`
  ```python
  live2d_fps: int = Field(60, alias="live2d_fps")
  ```
- `routes.py:423` `"live2d_fps": sys_cfg.get("live2d_fps", 120),`
- `settings-ui.ts:150` `live2d_fps: 120,`
- 为什么是缺陷：当 conf.yaml 缺此键时，UI/routes 显示 120，而 schema 校验出的默认值是 60 —— 冷启动与热重载行为不一致。

#### F-R4 【Low】`Config.system_config` 类型非 Optional 却默认 `None`
- `main.py:17` `system_config: SystemConfig = Field(default=None, alias="system_config")` —— 若 YAML 缺整个 `system_config` 段，后续 `config.system_config.port` 会 `AttributeError`；应 `Optional[SystemConfig]` 或默认构造。

#### F-R5 【Low】`I18nMixin.get_field_options` 是死代码（恒返回 None）
- `i18n.py:135-139`
  ```python
  field = cls.model_fields.get(field_name)
  if field:
      if hasattr(field, "options"):
          return field.options
  return None
  ```
- 为什么是缺陷：pydantic v2 `FieldInfo` 没有 `.options` 属性，方法永远返回 `None`，无法提供选项。

---

### 1.2 `character.py`

#### F-C1 【High】schema 要求 `persona_prompt` 必填，但 conf.yaml 根本没有该键；YAML 里的 `load_persona_from_file` 又未被建模 —— 配置切换路径会校验失败
- `character.py:22`
  ```python
  persona_prompt: str = Field(..., alias="persona_prompt")
  ```
- `conf.yaml:28`
  ```yaml
  load_persona_from_file: 'shared/persona.yaml'
  ```
  (`grep persona_prompt conf.yaml conf.yaml.template` → 都不存在；`character.py` 字段集里也没有 `load_persona_from_file`)
- 为什么是缺陷/设计问题：
  - `Config(**raw_conf.yaml)` 缺 `persona_prompt` 直接 `ValidationError`。应用靠**schema 外**的预处理才不崩：`run_server.py:89` 与 `routes.py:298-309`（`_config_dict_for_reload`）从 `shared/persona.yaml` 注入；`routes.py` 注释甚至写明“conf.yaml 本身不含 persona_prompt…必须注入，否则 Pydantic 热重载会报 Field required”。
  - 关键副作用：`service_context.py:561-588` 的 `handle_config_switch(conf.yaml)` 分支用**原始 conf.yaml 的 character_config**（不注入 persona）直接 `validate_config(...)` → 必然 `Field required` → 前端“切换到默认配置”功能报错。
  - `load_persona_from_file` 因 `extra='ignore'` 被静默丢弃，`model_dump()` 后保存会丢失 persona 文件路径。

#### F-C2 【High】`set_default_character_name` 校验器写坏：空 `character_name` 触发裸 `TypeError`，回退逻辑永远不生效
- `character.py:80-84`
  ```python
  @field_validator("character_name")
  def set_default_character_name(cls, v, values):
      if not v and "conf_name" in values:
          return values["conf_name"]
      return v
  ```
  （守卫字段 `character.py:19` `character_name: str = Field(default="", alias="character_name")`）
- 为什么是缺陷（pydantic 2.12.5 实测）：第二个位置参数是 `ValidationInfo` 而非 dict，`"conf_name" in values` 抛 `TypeError`；且默认值不做校验，省略时直接得到 `''`。所以：**任何把 `character_name` 置空的 alt 配置，加载时以裸 TypeError 崩溃**，而不是干净报错或回退到 `conf_name`。现网 `conf.yaml:27` 填了 `'白'` 所以未触发。

---

### 1.3 `asr.py`

#### F-A1 【High】`ASRConfig.check_asr_config` 全部死分支 —— 用 CamelCase 显示名比对 lowercase Literal，永不匹配；选中 provider 缺子配置仍通过校验，运行时 `AttributeError`
- `asr.py:313-322`（字段为小写 Literal）
  ```python
  asr_model: Literal[
      "disabled", "faster_whisper", "whisper_cpp", "whisper",
      "azure_asr", "fun_asr", "groq_whisper_asr", "sherpa_onnx_asr",
  ] = Field(..., alias="asr_model")
  ```
- `asr.py:356-376`（比对的是显示名）
  ```python
  if asr_model == "AzureASR" and values.azure_asr is not None:
  elif asr_model == "Faster-Whisper" and values.faster_whisper is not None:
  elif asr_model == "WhisperCPP" and values.whisper_cpp is not None:
  ...
  ```
- 为什么是缺陷（实测）：`"faster_whisper" == "Faster-Whisper"` 永远为假 → 校验从不触发，也不校验“选中 provider 的子配置必须存在”。`ASRConfig(asr_model="faster_whisper", faster_whisper=None)` **通过校验**，随后 `service_context.py:397-400` `getattr(asr_config, asr_config.asr_model).model_dump()` → `None.model_dump()` → `AttributeError`。即设置页 `/api/config` POST 保存“未配置的 ASR provider”时校验放行、热重载/启动崩溃。
- 附带：docstring `# Only validate the selected ASR model` 与实现不符。

---

### 1.4 `vad.py`

#### F-V1 【High】`VADConfig.check_asr_config` 读错属性（`values.silero_vad` 应为 `values.vad_model`），守卫恒假 → 选中 VAD 缺子配置仍通过校验，运行时 `AttributeError`
- `vad.py:44-45, 56-64`
  ```python
  vad_model: Optional[Literal["silero_vad"]] = Field(None, alias="vad_model")
  silero_vad: Optional[SileroVADConfig] = Field(None, alias="silero_vad")
  ...
      def check_asr_config(cls, values: "VADConfig", info: ValidationInfo):
          vad_model = values.silero_vad          # <-- 错：应取 values.vad_model
          if vad_model == "silero_vad" and values.silero_vad is not None:
              values.silero_vad.model_validate(values.silero_vad.model_dump())
  ```
- 为什么是缺陷（实测）：`values.silero_vad` 是 `SileroVADConfig` 对象或 None，永不等于字符串 `"silero_vad"`，所以即使 `vad_model="silero_vad"` 且 `silero_vad=None` 也**通过校验**；随后 `service_context.py:424-426` `getattr(vad_config, vad_config.vad_model.lower()).model_dump()` → `None.model_dump()` → `AttributeError`。方法名 `check_asr_config` 也是 from ASR 复制粘贴的痕迹。

#### F-V2 【Medium】磁盘 `conf.yaml:374` `vad_model:` 为 **null**（VAD 静默关闭），模板却是 `'silero_vad'`；现网配置带了一整段永远不用的 `silero_vad` 配置
- `conf.yaml:373-375`
  ```yaml
  vad_config:
    vad_model:
    silero_vad:
  ```
- `conf.yaml.template:101` `vad_model: 'silero_vad'`
- 为什么是缺陷：schema 接受 `None` 与 `'silero_vad'`（`vad.py:44`），`service_context.py:417-420` 把 `None` 当作 **disabled**。因此随仓库下发的 `conf.yaml` 声明的 VAD 是关的（只有 INFO 日志，无任何报错），但 `silero_vad` 子块仍被校验/携带 —— 自相矛盾且不易察觉。

#### F-V3 【Low】`SileroVADConfig` 7 个字段全部 `Field(...)` 必填，注释却像默认值
- `vad.py:10-16` `orig_sr: int = Field(..., alias="orig_sr")  # 16000` 等。缺任一个就 `Field required`，而注释暗示可缺省用默认。

---

### 1.5 `vision.py`

- **No issue found（字段/类型/默认与 conf.yaml 一致）**：`vision_enabled/provider/api_key/model/capture_interval`（`vision.py:22-26`）与 `conf.yaml:394-399` 一一对应。
- ### F-Vi1 【Low】`provider: str` 是自由字符串（`vision.py:23`），拼错如 `'zhipu '`/未知 provider 通过校验、之后在 vision 管线才失败；`model` 默认 `"glm-4v"`（`vision.py:25`）与说明/线上 `glm-4.6v` 有漂移（现网 conf.yaml 显式写了 `glm-4.6v`，被覆盖）。

---

### 1.6 `stateless_llm.py` / `agent.py`

#### F-L1 【High】`max_tokens` 前端发送、routes 读写，但 schema **无此字段** → 校验/运行时被 `extra='ignore'` 丢弃，用户设置完全不生效
- 前端 `settings-ui.ts:853`
  ```ts
  max_tokens: Number(card.querySelector<HTMLInputElement>('.llm-max-tokens')?.value || 0) || undefined,
  ```
- routes 写入与读取 `routes.py:494` / `routes.py:357`
  ```python
  for field in ("model", "base_url", "temperature", "max_tokens", "models"):
      if field in override and override[field] not in (None, ""):
          target[field] = override[field]
  ...
  "max_tokens": cfg.get("max_tokens", None),
  ```
- schema `stateless_llm.py:28-34`（及 61-66）无 `max_tokens` 字段
- 运行时也不读：`agent/stateless_llm_factory.py:60-67` 构造只取 `model/base_url/llm_api_key/organization_id/project_id/temperature`。
- 为什么是缺陷（实测）：设置页“Max Tokens”保存进 conf.yaml（raw），热重载 `AppConfig(**candidate)` 解析时被丢弃；`service_context.py:496` 再 `llm_configs.model_dump()` 已无此键 → LLM 请求永远不带 max_tokens。**这是“已保存但零效果”的真 bug。**

#### F-L2 【Medium】`models`（provider 可选模型清单）YAML/前端/routes 都在用，schema 无此字段 → 模型路径往返会丢失
- `conf.yaml:52-53` `models:\n- glm-4.7-flash`；前端 `settings-ui.ts:854` 回传 `models`；routes `routes.py:494` 写入、`routes.py:355` 读取。schema 里 `OpenAICompatibleConfig`（`stateless_llm.py:58-66`）与整个 `config_manager` **都没有 `models` 字段**（grep 证实）。
- 现状靠 routes 直读 raw YAML 存活；任何经 schema `model_dump` 的路径（`save_config`、`handle_config_switch` 的 `model_dump()` 基底）都会丢掉它，且 `modelz:` 之类拼写也静默通过。

#### F-L3 【High】provider id 无 schema 校验：`llm_provider` 自由 `str` + `StatelessLLMConfigs` 固定字段集 + `extra='ignore'` → 用户自定义 provider 被静默丢弃，热重载后等于没配
- `agent.py:19` `llm_provider: str = Field(..., alias="llm_provider")`
- `stateless_llm.py:232-270` 是固定命名字段（无 `Dict[str,...]` 兜底、无 `extra` 配置）
- 前端 “+ 添加 OpenAI 兼容 Provider” `settings-ui.ts:567-599` 用 `window.prompt` 接受任意 id；routes `routes.py:486-491` 照写 raw YAML；但 `routes.py:563-583`（保存前校验）与 `_config_dict_for_reload`（`routes.py:654`）用 `AppConfig(**candidate)` 解析时该自定义 id 被 `extra='ignore'` **丢弃**。
- 为什么是缺陷：GET 因为读 raw YAML 会展示该 provider，保存也通过校验，但类型化 `Config` 里没有它 → 热重载/测试路径永远加载不到 → 运行时才 `ValueError: Configuration not found for LLM provider ...`（`agent_factory.py:53-56`）。即 UI 宣称的功能（自定义兼容 provider）实际不可用。

#### F-L4 【Medium】别名字段 `openai`/`groq`/`mistral` 用通用 `OpenAICompatibleConfig`（要求 `base_url`），而 `*_llm` 兄弟字段用带默认 `base_url`/`interrupt_method` 的子类 → 同一 provider 因键名不同校验规则不同
- `stateless_llm.py:248` vs `:261`（openai_llm=OpenAIConfig vs openai=OpenAICompatibleConfig）；`:253` vs `:268`（groq_llm vs groq）；`:256` vs `:269`（mistral_llm vs mistral）。缺 `base_url` 的手改 YAML：`openai_llm` 能过、`openai` 报错（rules 常注入 base_url 掩盖了问题）。

#### F-L5 【Medium】`zhipu_free_llm` “零配置内置共享 Key” 文案过期：schema 是普通 `ZhipuConfig`（必填 `llm_api_key`），`free_glm_defaults.py` 已改为**没有内置 Key、空/未解析占位符直接 raise**
- `stateless_llm.py:251` `zhipu_free_llm: ZhipuConfig | None = ...`，而 `:298-301` 描述“Zero-config free … (built-in shared key)”。
- `free_glm_defaults.py:2` “spoq-fix: no built-in shared key”，`resolve_llm_api_key` 空/`${…}` 直接 `raise ValueError`（`free_glm_defaults.py:29-36`）；`stateless_llm_factory.py:57-58` 对它强制调用。
- 为什么是设计缺陷：UI 标“免费 GLM”、schema 与 conf 注释许诺开箱即用，但当前实现必须配 `ZHIPU_API_KEY`。文案与行为矛盾。

#### F-L6 【Low】`openai_compatible` 与 `openai_compatible_llm` 双字段并存（兼容旧配置），routes 统一归一化到 `*_llm`，scheme 里无守卫防发散
- `stateless_llm.py:239-245`；`routes.py:483/500-503` 归一化；`settings-ui.ts:96-97` 旧标签仍在、`LLM_CATEGORIES` 已只列 `openai_compatible_llm`（`settings-ui.ts:118`）。

#### F-L7 【Low】元数据/文案漂移（不影响功能）
- `agent.py:207-214` `AgentConfig.DESCRIPTIONS` 挂了不属于它的 `faster_first_response`、`segment_method`（应挂在 `BasicMemoryAgentConfig`/`LettaConfig`）。
- `agent.py:39-42` `use_mcpp` 英文注释 “default: True” vs 代码/中文 `Field(False)`。
- `agent.py:35-38` `segment_method` 描述只提 `regex|pysbd`，而 Literal 含 `priority`（conf.yaml:38 正用 `'priority'`）。
- `stateless_llm.py:272-312` `StatelessLLMConfigs.DESCRIPTIONS` 缺 `qwen_llm/openrouter/openai/moonshot/together/fireworks/siliconflow/opencode/opencode_go/groq/mistral/anthropic/mistral_llm`。

#### F-L8 【Low】
- `agent.py:24` `mcp_enabled_servers: Optional[List[str]] = Field([])` 可变默认（pydantic v2 会深拷贝，危害低于看起来，但仍是不良模式）。
- `agent.py:54/70/84` `config: Dict` 裸 Dict（无类型参数 → 内部无校验）。
- 嵌套 `Optional` 的 null vs `{}` 不对称：`basic_memory_agent: null` 通过，`{}`（缺 `llm_provider`）`Field required` 报错；routes `routes.py:453-456` 可能保存出空 dict。

---

### 1.7 `tts.py` / `tts_preprocessor.py`

#### F-T1 【High】TTS 的 `voices` / `models` 在 conf.yaml 与前端都在用，但三个 provider schema 都没有 → `model_dump` 丢数据
- `tts.py:150-153` `EdgeTTSConfig` 只有 `voice`；`tts.py:235-255` `CosyVoiceCloudTtsConfig`、`tts.py:298-304` `QwenTTSConfig` 均无 `voices`/`models`。
- `conf.yaml:330-371`（cosyvoice_cloud/qwen_tts/edge_tts 均含 `voices` 与 `models` 列表）；前端 `settings-ui.ts:29-37` `TtsProviderCfg{voices?, models?}`、`:871-876` 回传；routes `routes.py:529-531` 写入、`routes.py:374-375` 读取。
- 为什么是缺陷（pydantic 实测）：`model_dump()` 后自定义 `voices/models` 全部消失。目前 `/api/config` 走 raw YAML 所以 UI 目录还活着；但任何 schema 往返（`save_config`、`handle_config_switch.model_dump()` 基底）都会静默丢掉用户加的音色/模型列表。

#### F-T2 【High】`TTSConfig.check_tts_config` 从不强制“选中 provider 子配置必须存在” → 缺块通过校验，`init_tts` 崩溃
- `tts.py:897-945` 每个分支 `elif tts_model == "cosyvoice_cloud" and values.cosyvoice_cloud is not None:` 均不 raise 缺失。
- 运行时假设存在 —— `service_context.py:409-410`
  ```python
  tts_config.tts_model,
  **getattr(tts_config, tts_config.tts_model.lower()).model_dump(),
  ```
- 为什么是缺陷（实测/推演）：手改/alt 配置 `tts_model: 'edge_tts'` 缺 `edge_tts:` 块时校验通过，`init_tts` 里 `None.model_dump()` → `AttributeError`。所谓 `model_validate(model_dump())` 也仅是对已校验对象再校验，空转。

#### F-T3 【Medium】`tts_model` 是严格 Literal，旧 id（`gpt_sovits/piper/sherpa_onnx/pyttsx3`）只在 **routes 写路径** remap；`tts_configs` 块循环不做 remap → 旧配置双写/孤儿块
- `tts.py:780-802` `tts_model: Literal["azure_tts", …, "pyttsx3_tts"]`
- routes remap 只对标量 `tts_model`：`routes.py:459-464` / `535-540`；`tts_configs` 块循环 `routes.py:517-531` 无 remap（对比 LLM 侧 `routes.py:483` 有 remap）。
- 为什么是缺陷：手改或 alt YAML 里 `tts_model: 'gpt_sovits'` 直接 `ValidationError` 崩启动/拒存；经 UI 保存后产生一个新的 `gpt_sovits_tts` 块 + 遗留 `gpt_sovits` 孤儿块，配置重复。

#### F-T4 【Medium】未选中 provider 的块也被全量校验（all-or-nothing）—— 一个坏掉的未使用块会让启动/保存全崩
- `tts.py:812-838` 每个 provider 都是普通 `Optional` 字段，pydantic 会校验**所有出现**的块；`GPTSoVITSConfig`（`tts.py:367-378`）等要求必填字段。
- 为什么是缺陷：`tts_model: 'edge_tts'` 但配置里有个 `gpt_sovits_tts: {}` 时，`Config(**…)` 与保存前 `AppConfig(**candidate)`（`routes.py:557-569`）都会 `Field required` 失败；注释 `# Only validate the selected TTS model`（`tts.py:901`）名不副实。

#### F-T5 【Low】`conf.yaml.template` 与 schema/conf.yaml 漂移
- 模板 `conf.yaml.template:76-99`：无 `qwen_tts` 块、无 `viseme_mode`；edge_tts 只有 `voice`。而 `conf.yaml:327-372` 有 `qwen_tts`/`cosyvoice_cloud`/`edge_tts`/`viseme_mode: rms`。
- `conf.yaml.template:82-98` 的 cosyvoice_cloud 有 `format/sample_rate/volume/rate/pitch/timeout/max_retries`，而 `conf.yaml:330-345` 的 cosyvoice_cloud 有 `voices/models` 却没这些字段 —— 模板与磁盘互缺。新装用户从模板得不到 Qwen/viseme 文档。

#### F-T6 【Low】`SiliconFlowTTSConfig.DESCRIPTIONS` 键名与字段名不符
- 字段 `tts.py:502-512`（`api_url/default_model/default_voice/sample_rate/response_format/stream/speed/gain`）vs 描述 `tts.py:514-536`（`url/model/voice/…`，缺 `response_format`）→ `get_field_description("api_url")` 返回 None。

#### F-T7 【Low】空/auto 语义只在 routes，schema 接受空串
- `tts.py:303` `model: str = Field("qwen3-tts-flash", ...)` 接受 `""`；`OpenAITTSConfig`（`tts.py:542-546`）为 `Optional=None`；“空则取第一个已知”回退仅在 `routes.py:376-381` 与 `routes.py:546-551`。

#### F-T8 【Medium】`TTSPreprocessorConfig()` 无参构造必抛 `ValidationError`，却被 `transformers.py` 当作默认回退使用
- `tts_preprocessor.py:103-108`：`remove_special_char` 与 `translator_config` 均为 `Field(...)` 必填（实测 `TTSPreprocessorConfig()` → ValidationError）。
- `agent/transformers.py:364`
  ```python
  config = tts_preprocessor_config or TTSPreprocessorConfig()
  ```
- 为什么是缺陷：任何 `tts_filter` 在未传 `tts_preprocessor_config` 时的默认路径都会抛 ValidationError（主路径 `service_context.py:499` 会传参，故为潜在崩溃）。

#### F-T9（No issue）`tts_preprocessor.py` ↔ `conf.yaml` 键映射干净
`conf.yaml:384-392` 的 `remove_special_char/ignore_brackets/ignore_parentheses/ignore_asterisks/ignore_angle_brackets/translator_config.{translate_audio,translate_provider}` 与 `tts_preprocessor.py:103-108`、`:59-64` 一一对应；`deeplx/tencent` 正确 Optional。唯一瑕疵：`TTSPreprocessorConfig.DESCRIPTIONS`（`tts_preprocessor.py:110-117`）漏了四个 `ignore_*`。

### 1.8 前端 → 路由保存轴（`settings-ui.ts` × `routes.py`）

#### F-FE1 【Medium】清空某个字段不会清掉 YAML 旧值（空值被跳过）
- 前端 `settings-ui.ts:850-851, 853, 872-874, 902` 发送 `''` 或 `undefined`；后端 `routes.py:494-496, 523-530, 644-647` 只写非空：
  ```python
  if field in override and override[field] not in (None, ""):
      target[field] = override[field]
  ```
- 为什么是缺陷：用户在 UI 清空 Base URL/模型/ws_url/Max Tokens 等，字段视觉置空但 conf.yaml/persona.yaml 保留旧值，改动“静默不生效”；除原始 YAML 树 tab 外没有任何删除途径。

#### F-FE2 【Medium】`log_level` 只写不读 —— 打开面板即回退 INFO，随手点一次保存就把 `.env` 重写回 INFO
- GET `_get_common`（`routes.py:397-433`）无 `log_level` 键；前端下拉硬编码（`settings-ui.ts:255-261`，无 selected 绑定，浏览器默认 `INFO`）；写入 `routes.py:594-595` → `.env LIVE2DAI_LOG_LEVEL`。
- 症状：`.env` 原本 `DEBUG`，打开设置点保存 → 变成 `INFO`，日志级别被日常保存静默降级。

#### F-FE3 【Low】`port` / `live2d_fps` 用真值判断，`0` 被忽略
- `routes.py:472-477` `if "port" in payload and payload["port"]:` —— 0 是 falsy 不写入；而 `SystemConfig.check_port`（`system.py:39-44`）允许 `0<=port<=65535`。属 `is not None` 误用。

#### F-FE4 【Low】`fillSelect` 没有“未知当前值”兜底 → 下次保存把未知值悄悄换成第一个选项
- `settings-ui.ts:302-315` `opt.selected = v === current;`，current 不在列表时浏览器选中第一项；`collectPayload` 遂发送第一项。叠加 F-FE 系列/旧 id 场景更脆。

#### F-FE5（正面确认，不列为缺陷）实际保存路径对未知键/注释是安全的
- `routes.py:207-212` `_save_yaml` 用 `YAML(typ="rt")` + `preserve_quotes=True`，在 `_load_yaml` 返回的同对象上增量修改 → 注释与未知键保留；没有“前端没发某块就删块”的逻辑。schema 层的 `save_config` 才是会丢数据的那条路（F-R2，死代码）。

---

## 2. Schema ↔ YAML ↔ Frontend 关键键交叉引用表

> 列：YAML(conf.yaml) / Schema → 前端。`match`=键名一致；“✗ MISMATCH/丢”=schema 未建模或三层冲突。`(form)`=设置表单可编辑，`(未暴露)`=仅能通过 YAML 树改。

| 键（路径） | conf.yaml | Schema (file:line) | 前端 settings-ui.ts | 结论 |
|---|---|---|---|---|
| system_config.conf_version | :12 | system.py:10 | 未暴露 | match |
| system_config.host | :13 | system.py:11 | 未暴露 | match |
| system_config.port | :14 | system.py:12 | `port` GET+POST :782/:892 | match |
| system_config.live2d_fps | :15 `120` | system.py:16 **默认60** | :150 默认 `120` / :782 | **✗ 默认值 60 vs 120** (F-R3) |
| system_config.config_alts_dir | :16 | system.py:13 | 未暴露 | match |
| system_config.tool_prompts | :17-19 | system.py:14 | 未暴露 | match |
| system_config.enable_proxy | 无 | system.py:15 | 无 | schema-only（可选，不崩） |
| character_config.conf_name | :23 | character.py:16 | 未暴露 | match |
| character_config.conf_uid | :24 | character.py:17 | 未暴露 | match |
| character_config.live2d_model_name | :25 | character.py:18 | 未暴露 | match |
| character_config.character_name | :27 | character.py:19 | `character_name` GET+POST :780/:891 | match（+校验器 bug F-C2） |
| character_config.human_name / avatar | :26/:28 | character.py:20-21 | 未暴露 | match |
| character_config.load_persona_from_file | :28 | **无字段** | 未暴露 | **✗ schema 丢** (F-C1) |
| character_config.persona_prompt | **无** | character.py:22 必填 | 经 persona.system_prompt 注入 | **✗ schema 要求 YAML 没有的键** (F-C1) |
| character_config.agent_config.conversation_agent_choice | :35 | agent.py:191-193 | 未暴露 | match（`'basic_memory_agent'` 合法） |
| agent_settings.basic_memory_agent.llm_provider | :37 | agent.py:19 | `llm_provider` :790/:887 | match（+无 schema 校验 F-L3） |
| …faster_first_response | :38 | agent.py:21 | 未暴露 | match（bool 正确解析） |
| …segment_method | :39 | agent.py:22 | 未暴露 | match（`priority` 在 Literal，文档漏 F-L7） |
| …use_mcpp | :40 | agent.py:23 | 未暴露 | match（文档默认值矛盾 F-L7） |
| …mcp_enabled_servers | :41 | agent.py:24 | 未暴露 | match |
| llm_configs.<pid>.llm_api_key | :48… | stateless_llm.py:29/62 | 不覆盖现有块，key 走 .env | match（写路径不写回；密钥策略） |
| llm_configs.<pid>.model | :49… | stateless_llm.py:30/63 | `llm_configs[].model` | match |
| llm_configs.<pid>.base_url | :52… | stateless_llm.py:28/61 | `llm_configs[].base_url` | match |
| llm_configs.<pid>.temperature | :51… | stateless_llm.py:34/66 | `llm_configs[].temperature` | match |
| llm_configs.<pid>.max_tokens | routes 写入 | **无字段** | `llm_configs[].max_tokens` :853 | **✗ schema 丢 / 运行时零效果** (F-L1) |
| llm_configs.<pid>.models | :53… | **无字段** | `llm_configs[].models` :854 | **✗ schema 丢** (F-L2) |
| asr_config.asr_model | :318 | asr.py:313 | `asr_model` :806/:890 | match（校验器死 F-A1） |
| asr_config.faster_whisper.* | :319-325 | asr.py:31-38 | 未暴露 | match |
| tts_config.tts_model | :328 | tts.py:780-802 | `tts_model` :795/:888 | match（旧 id 崩 F-T3） |
| tts_config.concurrency_limit | :329 | tts.py:805 | 未暴露 | match |
| tts_config.viseme_mode | :372 | tts.py:808-810 | `viseme_mode` :808-809/:889 | match（模板缺 F-T5） |
| tts_config.cosyvoice_cloud.{api_key,model,ws_url,voice} | :330-346 | tts.py:243-255 | `tts_configs[].voice/model/ws_url` | match |
| tts_config.cosyvoice_cloud.{voices,models} | :332-345 | **无字段** | `tts_configs[].voices/models` :871-876 | **✗ schema 丢** (F-T1) |
| tts_config.cosyvoice_cloud.{format,sample_rate,volume,rate,pitch,timeout,max_retries} | 无 | tts.py:246-255 | 未暴露 | schema 独有默认；模板有、磁盘无 (F-T5) |
| tts_config.qwen_tts.{api_key,voice,model} | :346-350 | tts.py:301-303 | `tts_configs[]` | match |
| tts_config.qwen_tts.{voices,models} | :351-356 | **无字段** | `tts_configs[]` | **✗ schema 丢** (F-T1) |
| tts_config.edge_tts.voice | :358 | tts.py:153 | `tts_configs[]` | match |
| tts_config.edge_tts.voices | :360-371 | **无字段** | `tts_configs[]` | **✗ schema 丢** (F-T1) |
| vad_config.vad_model | :374 `null`（模板 :101 `silero_vad`） | vad.py:44 | 未暴露 | 类型 match；**磁盘 null=关闭 VAD** (F-V2) |
| vad_config.silero_vad.* | :375-382 | vad.py:10-16 | 未暴露 | match（全部必填 F-V3） |
| tts_preprocessor_config.* | :384-392 | tts_preprocessor.py:103-108 | 未暴露 | match（F-T9 no issue） |
| vision_config.{vision_enabled,provider,api_key,model,capture_interval} | :394-399 | vision.py:22-26 | 仅 model 经 persona 隐写 | match（其余未暴露） |
| mcp_servers | :402 | **Config 无此字段** | 未暴露 | **✗ schema 丢** (F-R1) |
| persona.*（name/role/model/temperature/max_tokens/vision_model/system_prompt） | persona.yaml | （schema 无对应模型；注入 character.persona_prompt） | `persona` :898-904 | match（独立文件；空值不写 F-FE1） |
| api_keys / log_level | .env | 无 schema | `api_keys` :881-885 / `log_level` :906 | 正常（log_level 只写不读 F-FE2） |

### provider id 三端对照（要点）
- **LLM**：routes `LLM_DEFAULTS`/`MODEL_CATALOG`（`routes.py:32-151`）与前端标签（`settings-ui.ts:80-98`）的 16 个 id 全部落在 `StatelessLLMConfigs` 固定字段（`stateless_llm.py:236-270`）内 —— 一致；`openai_compatible` 旧 id 归一化为 `openai_compatible_llm`（`routes.py:483`）。问题在“自定义任意 id”不被 schema 承载（F-L3）。
- **TTS**：`TTS_DEFAULTS`/`TTS_VOICE_CATALOG`（`routes.py:185-199`）13 个 id 与 `tts.py:780-838` 字段一致；旧裸 id（`gpt_sovits`/`piper`/`sherpa_onnx`/`pyttsx3`）不在 schema、只在标量保存时 remap（F-T3）。
- **ASR/VAD/vision**：无 provider 目录分叉问题；asr_model/vad_model 是已定义的 Literal。

---

## 3. 明确“No issue found”区域

- **ASR YAML 键名 ↔ schema**：`conf.yaml:317-325` 与 `ASRConfig`/`FasterWhisperConfig` 别名 1:1（问题在校验器逻辑 F-A1，不在键名）。
- **vision_config ↔ schema**：键名与类型完全一致（仅自由字符串 provider 与默认模型文案 F-Vi1）。
- **数字/布尔解析**：conf.yaml 用原生 YAML 布尔与数字，pydantic v2 正确解析；`"true"` 字符串也能 lax 转换，现网无字符串布尔问题。
- **tts_preprocessor_config ↔ schema**：1:1（F-T9）。
- **GET /api/config ↔ 前端 AppConfig**：`routes.py:397-433` 返回键与 `settings-ui.ts:50-62` 接口字段**完全对齐**，无 GET 键名错配（问题集中在保存路径/写回）。
- **实际保存路径往返**：`routes.py` ruamel round-trip 保留未知键与注释（F-FE5）；无“删块”逻辑。丢失风险只在 schema 层 dump 路径（`save_config` 死代码）。

---

## 4. 严重性小结（Top 需要先修的高危项）

| # | Severity | 位置 | 一句话 |
|---|---|---|---|
| F-L1 | High | stateless_llm.py(缺字段)–settings-ui.ts:853–routes.py:494 | `max_tokens` schema 丢弃，用户设置零效果 |
| F-L3 | High | agent.py:19 / stateless_llm.py:232-270 | 自定义 LLM provider 被静默丢弃，热重载不可用 |
| F-C1 | High | character.py:22 / conf.yaml:28 | `persona_prompt` 必填但 YAML 无 → 配置切换崩/依赖注入救场 |
| F-C2 | High | character.py:80-84 | 空 character_name → 裸 TypeError |
| F-A1 | High | asr.py:356-376 | ASR selected-provider 校验全死 → 运行时 AttributeError |
| F-V1 | High | vad.py:56-64 | VAD 校验读错字段 → 运行时 AttributeError |
| F-T1 | High | tts.py:150/243/301 | TTS voices/models schema 丢 |
| F-T2 | High | tts.py:897-945 / service_context.py:409 | 选中 TTS provider 缺块 → 运行时崩溃 |
| F-FE1/F-FE2 | Medium | routes.py:494 / :594 | 清空不生效；log_level 被日常保存重置 |
| F-R2 | Medium(潜在) | utils.py:107-124 | save_config 死代码=往返丢键+泄密雷 |

（其余见上文各小节。）
