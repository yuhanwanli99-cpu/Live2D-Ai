import os
import json
from typing import Callable
from loguru import logger
from fastapi import WebSocket

from prompts import prompt_loader
from .live2d_model import Live2dModel
from .model_registry import ModelRegistry
from .utils.repo_paths import resolve_shared_path as _resolve_shared_path_impl
from .asr.asr_interface import ASRInterface
from .tts.tts_interface import TTSInterface
from .vad.vad_interface import VADInterface
from .agent.agents.agent_interface import AgentInterface
from .vision_handler import VisionHandler
from .mood_engine import MoodEngine

from .mcpp.server_registry import ServerRegistry
from .mcpp.tool_manager import ToolManager
from .mcpp.mcp_client import MCPClient
from .mcpp.tool_executor import ToolExecutor
from .mcpp.tool_adapter import ToolAdapter

from .asr.asr_factory import ASRFactory
from .tts.tts_factory import TTSFactory
from .vad.vad_factory import VADFactory
from .agent.agent_factory import AgentFactory

from .config_manager import (
    Config,
    AgentConfig,
    CharacterConfig,
    SystemConfig,
    ASRConfig,
    TTSConfig,
    VADConfig,
    read_yaml,
    validate_config,
)


def _resolve_mcp_servers_path() -> str:
    """Resolve the MCP servers config path.

    Live2D-Ai ships MCP server definitions in the cross-platform shared/mcp_tools.json
    file rather than requiring a project-local (gitignored) mcp_servers.json, so a
    fresh clone works out of the box. Falls back to the local file if the shared one
    is missing (e.g. running upstream Open-LLM-VTuber standalone).
    """


def _strip_legacy_expression_sections(text: str) -> str:
    """从主对话脑 system prompt 中剥离旧的表情控制区块。

    PC v4 已把 joy 等表情/动作分流到独立导演模型；主对话脑只输出自然语言。
    shared/persona.yaml 仍为双端共享保留这些区块，这里只在 PC 主脑侧剥离。
    """
    if not text:
        return text
    lines = text.splitlines()
    out: list[str] = []
    skip = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("## 表情控制") or stripped.startswith("## 表情强度控制"):
            skip = True
            continue
        if skip and stripped.startswith("## "):
            skip = False
        if not skip:
            out.append(line)
    return "\n".join(out)
    shared_path = os.path.join("..", "..", "shared", "mcp_tools.json")
    if os.path.exists(shared_path):
        return shared_path
    return "mcp_servers.json"


class ServiceContext:
    """Initializes, stores, and updates the asr, tts, and llm instances and other
    configurations for a connected client."""

    def __init__(self):
        self.config: Config = None
        self.system_config: SystemConfig = None
        self.character_config: CharacterConfig = None

        self.live2d_model: Live2dModel = None
        # (NEW) P3/P7：流式心情引擎（对话级 start/stop，prompt 注入用）
        self.mood_engine: MoodEngine = MoodEngine()
        # (NEW) 跨平台模型注册表（shared/model_registry.json），init_live2d 校验/回退用
        self.model_registry: ModelRegistry | None = None
        self.asr_engine: ASRInterface = None
        self.tts_engine: TTSInterface = None
        self.agent_engine: AgentInterface = None
        self.vad_engine: VADInterface | None = None
        self.translate_engine = None  # Kept for backward compatibility
        self.vision_handler: VisionHandler | None = None

        self.mcp_server_registery: ServerRegistry | None = None
        self.tool_adapter: ToolAdapter | None = None
        self.tool_manager: ToolManager | None = None
        self.mcp_client: MCPClient | None = None
        self.tool_executor: ToolExecutor | None = None

        # the system prompt is a combination of the persona prompt and live2d expression prompt
        self.system_prompt: str = None

        # Store the generated MCP prompt string (if MCP enabled)
        self.mcp_prompt: str = ""

        self.history_uid: str = ""

        self.send_text: Callable = None
        self.client_uid: str = None

        self._vision_config_raw: dict | None = None

        # PC v4 插件系统：由 server 注入 PluginManager，所有非核心能力经此登记。
        self.plugin_manager = None
        # PC v4 多脑编排：独立导演模型，把 joy 等表情/动作从主对话脑分流。
        self.director = None
        # 热重载变化检测：记录各引擎最近一次生效的配置，避免 load_from_config 先把
        # 新配置赋到 character_config 后，init_* 的 `!=` 保护失效导致永不重建引擎。
        self._last_asr_config = None
        self._last_tts_config = None
        self._last_vad_config = None

    def __str__(self):
        return (
            f"ServiceContext:\n"
            f"  System Config: {'Loaded' if self.system_config else 'Not Loaded'}\n"
            f"    Details: {json.dumps(self.system_config.model_dump(), indent=6) if self.system_config else 'None'}\n"
            f"  Live2D Model: {self.live2d_model.model_info if self.live2d_model else 'Not Loaded'}\n"
            f"  ASR Engine: {type(self.asr_engine).__name__ if self.asr_engine else 'Not Loaded'}\n"
            f"    Config: {json.dumps(self.character_config.asr_config.model_dump(), indent=6) if self.character_config.asr_config else 'None'}\n"
            f"  TTS Engine: {type(self.tts_engine).__name__ if self.tts_engine else 'Not Loaded'}\n"
            f"    Config: {json.dumps(self.character_config.tts_config.model_dump(), indent=6) if self.character_config.tts_config else 'None'}\n"
            f"  LLM Engine: {type(self.agent_engine).__name__ if self.agent_engine else 'Not Loaded'}\n"
            f"    Agent Config: {json.dumps(self.character_config.agent_config.model_dump(), indent=6) if self.character_config.agent_config else 'None'}\n"
            f"  VAD Engine: {type(self.vad_engine).__name__ if self.vad_engine else 'Not Loaded'}\n"
            f"    Agent Config: {json.dumps(self.character_config.vad_config.model_dump(), indent=6) if self.character_config.vad_config else 'None'}\n"
            f"  System Prompt: {self.system_prompt or 'Not Set'}\n"
            f"  MCP Enabled: {'Yes' if self.mcp_client else 'No'}"
        )

    # ==== Initializers

    async def _init_mcp_components(self, use_mcpp, enabled_servers):
        """Initializes MCP components based on configuration, dynamically fetching tool info."""
        logger.debug(
            f"Initializing MCP components: use_mcpp={use_mcpp}, enabled_servers={enabled_servers}"
        )

        # Reset MCP components first
        self.mcp_server_registery = None
        self.tool_manager = None
        self.mcp_client = None
        self.tool_executor = None
        self.json_detector = None
        self.mcp_prompt = ""

        if use_mcpp and enabled_servers:
            # 1. Initialize ServerRegistry
            self.mcp_server_registery = ServerRegistry(_resolve_mcp_servers_path())
            logger.info("ServerRegistry initialized or referenced.")

            # 2. Use ToolAdapter to get the MCP prompt and tools
            if not self.tool_adapter:
                logger.error(
                    "ToolAdapter not initialized before calling _init_mcp_components."
                )
                self.mcp_prompt = "[Error: ToolAdapter not initialized]"
                return

            try:
                (
                    mcp_prompt_string,
                    openai_tools,
                    claude_tools,
                ) = await self.tool_adapter.get_tools(enabled_servers)
                self.mcp_prompt = mcp_prompt_string
                logger.info(
                    f"Dynamically generated MCP prompt string (length: {len(self.mcp_prompt)})."
                )
                logger.info(
                    f"Dynamically formatted tools - OpenAI: {len(openai_tools)}, Claude: {len(claude_tools)}."
                )

                _, raw_tools_dict = await self.tool_adapter.get_server_and_tool_info(
                    enabled_servers
                )
                self.tool_manager = ToolManager(
                    formatted_tools_openai=openai_tools,
                    formatted_tools_claude=claude_tools,
                    initial_tools_dict=raw_tools_dict,
                )
                logger.info("ToolManager initialized with dynamically fetched tools.")

            except Exception as e:
                logger.error(
                    f"Failed during dynamic MCP tool construction: {e}", exc_info=True
                )
                self.tool_manager = None
                self.mcp_prompt = "[Error constructing MCP tools/prompt]"

            # 4. Initialize MCPClient
            if self.mcp_server_registery:
                self.mcp_client = MCPClient(
                    self.mcp_server_registery, self.send_text, self.client_uid
                )
                logger.info("MCPClient initialized for this session.")
            else:
                logger.error(
                    "MCP enabled but ServerRegistry not available. MCPClient not created."
                )
                self.mcp_client = None

            # 5. Initialize ToolExecutor
            if self.mcp_client and self.tool_manager:
                self.tool_executor = ToolExecutor(self.mcp_client, self.tool_manager)
                logger.info("ToolExecutor initialized for this session.")
            else:
                logger.warning(
                    "MCPClient or ToolManager not available. ToolExecutor not created."
                )
                self.tool_executor = None

            logger.info("StreamJSONDetector initialized for this session.")

        elif use_mcpp and not enabled_servers:
            logger.warning(
                "use_mcpp is True, but mcp_enabled_servers list is empty. MCP components not initialized."
            )
        else:
            logger.debug(
                "MCP components not initialized (use_mcpp is False or no enabled servers)."
            )

    async def close(self):
        """Clean up resources, especially the MCPClient."""
        logger.info("Closing ServiceContext resources...")
        if self.mcp_client:
            logger.info(f"Closing MCPClient for context instance {id(self)}...")
            await self.mcp_client.aclose()
            self.mcp_client = None
        self._safe_close(self.agent_engine)
        self.agent_engine = None
        self._safe_close(self.vision_handler)
        self.vision_handler = None
        logger.info("ServiceContext closed.")

    async def load_cache(
        self,
        config: Config,
        system_config: SystemConfig,
        character_config: CharacterConfig,
        live2d_model: Live2dModel,
        asr_engine: ASRInterface,
        tts_engine: TTSInterface,
        vad_engine: VADInterface,
        agent_engine: AgentInterface,
        mcp_server_registery: ServerRegistry | None = None,
        tool_adapter: ToolAdapter | None = None,
        send_text: Callable = None,
        client_uid: str = None,
        plugin_manager=None,
    ) -> None:
        """Load the ServiceContext with the reference of the provided instances."""
        if not character_config:
            raise ValueError("character_config cannot be None")
        if not system_config:
            raise ValueError("system_config cannot be None")

        self.config = config
        self.system_config = system_config
        self.character_config = character_config
        self.live2d_model = live2d_model
        self.asr_engine = asr_engine
        self.tts_engine = tts_engine
        self.vad_engine = vad_engine
        self.agent_engine = agent_engine
        self.mcp_server_registery = mcp_server_registery
        self.tool_adapter = tool_adapter
        self.send_text = send_text
        self.client_uid = client_uid
        self.plugin_manager = plugin_manager

        await self._init_mcp_components(
            self.character_config.agent_config.agent_settings.basic_memory_agent.use_mcpp,
            self.character_config.agent_config.agent_settings.basic_memory_agent.mcp_enabled_servers,
        )

        # Initialize vision handler from config (if vision is enabled)
        if config and config.vision_config and config.vision_config.vision_enabled:
            self.init_vision(config)

        logger.debug(f"Loaded service context with cache: {character_config}")

    async def load_from_config(self, config: Config) -> None:
        """Load the ServiceContext with the config."""
        # 重要（修复 B3 顺序错误）：先把新的 system_config/character_config 赋到
        # 本 context 上，再 init_* —— 这样 init_agent/init_live2d/init_asr/init_tts
        # 等都基于新快照构建，而不是启动期旧 system_config/avatar。
        self.config = config
        self.system_config = config.system_config or self.system_config
        self.character_config = config.character_config

        # (NEW) 加载跨平台模型注册表（缺失/损坏时返回 None，回退硬编码 bai）
        self.model_registry = self._load_model_registry()
        self.init_live2d(config.character_config.live2d_model_name)
        self.init_asr(config.character_config.asr_config)
        self.init_tts(config.character_config.tts_config)
        self.init_vad(config.character_config.vad_config)
        self.init_vision(config)

        if (
            not self.tool_adapter
            and config.character_config.agent_config.agent_settings.basic_memory_agent.use_mcpp
        ):
            if not self.mcp_server_registery:
                logger.info(
                    "Initializing shared ServerRegistry within load_from_config."
                )
                self.mcp_server_registery = ServerRegistry(_resolve_mcp_servers_path())
            logger.info("Initializing shared ToolAdapter within load_from_config.")
            self.tool_adapter = ToolAdapter(server_registery=self.mcp_server_registery)

        await self._init_mcp_components(
            config.character_config.agent_config.agent_settings.basic_memory_agent.use_mcpp,
            config.character_config.agent_config.agent_settings.basic_memory_agent.mcp_enabled_servers,
        )

        await self.init_agent(
            config.character_config.agent_config,
            config.character_config.persona_prompt,
        )

    def _load_model_registry(self) -> ModelRegistry | None:
        """加载 shared/model_registry.json；缺失/损坏时返回 None（回退硬编码 bai）。"""
        registry_path = _resolve_shared_path("shared/model_registry.json")
        if not os.path.exists(registry_path):
            logger.error(
                f"Model registry file not found at {registry_path}; "
                "model selection validation disabled (fallback to 'bai')."
            )
            return None
        try:
            registry = ModelRegistry(registry_path)
            logger.info(
                f"Model registry loaded: {len(registry.get_all())} entries "
                f"from {registry_path}"
            )
            return registry
        except (ValueError, OSError) as e:
            logger.error(
                f"Failed to load model registry at {registry_path}: {e}; "
                "model selection validation disabled (fallback to 'bai')."
            )
            return None

    def init_live2d(self, live2d_model_name: str) -> None:
        logger.info(f"Initializing Live2D: {live2d_model_name}")
        try:
            model_name = self._resolve_live2d_model_name(live2d_model_name)
            self.live2d_model = Live2dModel(
                model_name, registry=self.model_registry
            )
            self.character_config.live2d_model_name = model_name
            return
        except RuntimeError:
            # registry 无可用模型 → 服务器启动失败（plan 边界表）
            raise
        except Exception as e:
            logger.critical(f"Error initializing Live2D: {e}")

        # 回退：registry 可用 → 默认模型；registry 不可用 → 硬编码 'bai'
        try:
            fallback_name = self._fallback_live2d_model_name()
            logger.warning(f"Falling back to Live2D model: {fallback_name}")
            self.live2d_model = Live2dModel(
                fallback_name, registry=self.model_registry
            )
            self.character_config.live2d_model_name = fallback_name
        except RuntimeError:
            # registry 无可用模型 → 服务器启动失败（plan 边界表）
            raise
        except Exception as fallback_e:
            logger.critical(
                f"Error initializing fallback Live2D model: {fallback_e}"
            )
            logger.critical("Try to proceed without Live2D...")

    def _resolve_live2d_model_name(self, live2d_model_name: str) -> str:
        """校验请求的模型名；未登记或已登记但不可用时回退到注册表默认模型。

        P2-1（plan-e2e.md §7）：命中后追加 `entry.available` 检查——conf 指向
        「已注册但不可用」的 id 时回退 get_default()，不再原样放行（服务器拒绝
        启动于不可用模型；全部 unavailable 时 get_default() 抛 RuntimeError）。
        守卫逻辑位于 ModelRegistry.resolve_model_name()，便于在无 fastapi 的
        轻量测试环境直接单测。
        """
        if self.model_registry is None:
            return live2d_model_name
        resolved = self.model_registry.resolve_model_name(live2d_model_name)
        if resolved != live2d_model_name:
            logger.warning(
                f"Live2D model '{live2d_model_name}' not found or unavailable in "
                f"model registry; falling back to registry default '{resolved}'."
            )
        return resolved

    def _fallback_live2d_model_name(self) -> str:
        """registry 可用时返回默认模型 id；不可用时返回硬编码 'bai'（C13：与 Android 兜底一致）。"""
        if self.model_registry is not None:
            return self.model_registry.get_default().id
        return "bai"

    def _safe_close(self, engine) -> None:
        """幂等关闭一个引擎：已在关闭状态或未实现 close() 时直接跳过。

        修复 B8：reload 替换引擎前不再泄漏旧句柄；幂等性避免多 context 共享同一
        引擎对象时被重复 close（防 B7 修复引入的双重关闭）。
        """
        if engine is None:
            return
        if getattr(engine, "_closed", False):
            return
        close_fn = getattr(engine, "close", None)
        if close_fn is None:
            return
        try:
            engine._closed = True
            close_fn()
        except Exception as e:
            logger.warning(f"Error closing engine {type(engine).__name__}: {e}")

    def init_asr(self, asr_config: ASRConfig) -> None:
        if asr_config.asr_model == "disabled":
            self._safe_close(self.asr_engine)
            self.asr_engine = None
            logger.info("ASR disabled (asr_model='disabled'; 启用需在 conf.yaml 改为 faster_whisper 等并配置模型)")
            return
        if not self.asr_engine or (self._last_asr_config != asr_config):
            logger.info(f"Initializing ASR: {asr_config.asr_model}")
            self._safe_close(self.asr_engine)
            self.asr_engine = ASRFactory.get_asr_system(
                asr_config.asr_model,
                **getattr(asr_config, asr_config.asr_model).model_dump(),
            )
            self.character_config.asr_config = asr_config
            self._last_asr_config = asr_config
        else:
            logger.info("ASR already initialized with the same config.")

    def init_tts(self, tts_config: TTSConfig) -> None:
        if not self.tts_engine or (self._last_tts_config != tts_config):
            logger.info(f"Initializing TTS: {tts_config.tts_model}")
            self._safe_close(self.tts_engine)
            self.tts_engine = TTSFactory.get_tts_engine(
                tts_config.tts_model,
                **getattr(tts_config, tts_config.tts_model.lower()).model_dump(),
            )
            self.character_config.tts_config = tts_config
            self._last_tts_config = tts_config
        else:
            logger.info("TTS already initialized with the same config.")

    def init_vad(self, vad_config: VADConfig) -> None:
        if vad_config.vad_model is None:
            logger.info("VAD is disabled.")
            self.vad_engine = None
            return

        if not self.vad_engine or (self._last_vad_config != vad_config):
            logger.info(f"Initializing VAD: {vad_config.vad_model}")
            self.vad_engine = VADFactory.get_vad_engine(
                vad_config.vad_model,
                **getattr(vad_config, vad_config.vad_model.lower()).model_dump(),
            )
            self.character_config.vad_config = vad_config
            self._last_vad_config = vad_config
        else:
            logger.info("VAD already initialized with the same config.")

    def init_vision(self, config: Config) -> None:
        """Initialize the vision handler from configuration if vision is enabled.

        修复 B8：支持热换——当启用状态/模型/key 变化时，先关闭旧 VisionHandler
        再重建，而不是看到 handler 已存在就 return。
        """
        vision_cfg = config.vision_config
        enabled = vision_cfg is not None and vision_cfg.vision_enabled

        # 计算将要使用的 key/model（与旧实现同逻辑），用于判断是否真的变化
        api_key = ""
        model = ""
        if enabled:
            api_key = vision_cfg.api_key
            if not api_key:
                zhipu = config.character_config.agent_config.llm_configs.zhipu_llm
                if zhipu and zhipu.llm_api_key:
                    api_key = zhipu.llm_api_key
            model = vision_cfg.model or ""

        if not enabled:
            if self.vision_handler is not None:
                self._safe_close(self.vision_handler)
                self.vision_handler = None
            return

        if self.vision_handler is not None:
            # 若配置未变，保持现状；避免热重载重建
            cur_key = getattr(self.vision_handler, "_api_key", None)
            cur_model = getattr(self.vision_handler, "_model", None)
            if cur_key == api_key and cur_model == model:
                logger.debug("VisionHandler unchanged, keeping existing instance.")
                return
            logger.info("Vision model/config changed; hot-replacing VisionHandler.")
            self._safe_close(self.vision_handler)
            self.vision_handler = None

        if not api_key:
            logger.warning(
                "Vision enabled but no API key found (neither vision_config.api_key "
                "nor llm_configs.zhipu_llm.llm_api_key is set)."
            )
            self.vision_handler = None
            return

        base_url = "https://open.bigmodel.cn/api/paas/v4/"
        try:
            self.vision_handler = VisionHandler(
                api_key=api_key,
                model=model,
                base_url=base_url,
            )
            self.vision_handler._api_key = api_key
            self.vision_handler._model = model
            logger.info(
                f"VisionHandler initialized (model={model}, "
                f"interval={vision_cfg.capture_interval}s)."
            )
        except Exception as e:
            logger.error(f"Failed to initialize VisionHandler: {e}")
            self.vision_handler = None

    async def init_agent(self, agent_config: AgentConfig, persona_prompt: str) -> None:
        """Initialize or update the LLM engine based on agent configuration."""
        logger.info(f"Initializing Agent: {agent_config.conversation_agent_choice}")

        if (
            self.agent_engine is not None
            and agent_config == self.character_config.agent_config
            and persona_prompt == self.character_config.persona_prompt
        ):
            logger.debug("Agent already initialized with the same config.")
            return

        system_prompt = await self.construct_system_prompt(persona_prompt)
        avatar = self.character_config.avatar or ""

        try:
            # 替换前关闭旧 agent，避免句柄/客户端泄漏（B8）
            self._safe_close(self.agent_engine)
            self.agent_engine = AgentFactory.create_agent(
                conversation_agent_choice=agent_config.conversation_agent_choice,
                agent_settings=agent_config.agent_settings.model_dump(),
                llm_configs=agent_config.llm_configs.model_dump(),
                system_prompt=system_prompt,
                live2d_model=self.live2d_model,
                tts_preprocessor_config=self.character_config.tts_preprocessor_config,
                character_avatar=avatar,
                system_config=self.system_config.model_dump(),
                tool_manager=self.tool_manager,
                tool_executor=self.tool_executor,
                mcp_prompt_string=self.mcp_prompt,
            )

            logger.debug(f"Agent choice: {agent_config.conversation_agent_choice}")
            logger.debug(f"System prompt: {system_prompt}")
            self.character_config.agent_config = agent_config
            self.system_prompt = system_prompt

            # PC v4：初始化独立导演模型，把 joy 等表情/动作从主对话脑分流。
            try:
                from .director import ExpressionDirector
                from .agent.stateless_llm_factory import LLMFactory as StatelessLLMFactory

                basic = agent_config.agent_settings.basic_memory_agent
                main_provider = basic.llm_provider if basic else "deepseek_llm"
                director_provider = (
                    getattr(self.system_config, "director_llm_provider", None)
                    or main_provider
                )
                director_cfg = dict(
                    agent_config.llm_configs.model_dump().get(director_provider, {})
                )
                director_model = getattr(self.system_config, "director_model", None)
                if director_model:
                    director_cfg["model"] = director_model
                # 导演只输出一行 JSON：低温 + 极小 max_tokens，降低每句额外延迟。
                director_cfg.setdefault("temperature", 0.0)
                director_cfg.setdefault("max_tokens", 64)
                if director_cfg:
                    director_llm = StatelessLLMFactory.create_llm(
                        director_provider, **director_cfg
                    )
                    self.director = ExpressionDirector(llm=director_llm)
                    logger.info(
                        f"ExpressionDirector initialized with provider {director_provider} "
                        f"model {director_cfg.get('model')}"
                    )
                else:
                    self.director = None
                    logger.warning("Director provider config missing; using rule fallback")
            except Exception as e:  # noqa: BLE001 - 导演失败不阻塞主模型
                self.director = None
                logger.error(f"Failed to initialize ExpressionDirector, fallback to rule: {e}")

            # LLM 记忆管家：对话链结束后异步沉淀/合并/清理长期事实。
            # 复用导演 provider（低温、稍大 max_tokens 容纳 JSON 数组）；
            # LIVE2DAI_MEMORY_CURATOR=off 可关闭；初始化失败自动缺席（正则兜底仍在）。
            self.memory_curator = None
            try:
                if os.getenv("LIVE2DAI_MEMORY_CURATOR", "").lower() != "off":
                    from .memory_curator import MemoryCurator

                    curator_cfg = dict(
                        agent_config.llm_configs.model_dump().get(director_provider, {})
                    )
                    if director_model := getattr(self.system_config, "director_model", None):
                        curator_cfg["model"] = director_model
                    curator_cfg.setdefault("temperature", 0.0)
                    curator_cfg.setdefault("max_tokens", 300)
                    if curator_cfg:
                        curator_llm = StatelessLLMFactory.create_llm(
                            director_provider, **curator_cfg
                        )
                        self.memory_curator = MemoryCurator(llm=curator_llm)
                        logger.info(
                            f"MemoryCurator initialized with provider {director_provider} "
                            f"model {curator_cfg.get('model')}"
                        )
            except Exception as e:  # noqa: BLE001 - 管家缺席不影响对话
                self.memory_curator = None
                logger.warning(f"MemoryCurator init failed, memory stays regex-only: {e}")

        except Exception as e:
            logger.error(f"Failed to initialize agent: {e}")
            raise

    async def construct_system_prompt(self, persona_prompt: str) -> str:
        """Append tool prompts to persona prompt.

        PC v4：主对话脑不再输出 joy 等表情标签；先剥离 persona 里的旧表情控制
        区块，并跳过 live2d_expression_prompt 工具提示。表情/动作由独立导演模型
        在句子阶段生成。
        """
        persona_prompt = _strip_legacy_expression_sections(persona_prompt)
        logger.debug(f"constructing persona_prompt: '''{persona_prompt}'''")

        for prompt_name, prompt_file in self.system_config.tool_prompts.items():
            if (
                prompt_name == "group_conversation_prompt"
                or prompt_name == "proactive_speak_prompt"
                or prompt_name == "live2d_expression_prompt"
            ):
                continue

            prompt_content = prompt_loader.load_util(prompt_file)

            if prompt_name == "mcp_prompt":
                continue

            persona_prompt += prompt_content

        # P7-04：mood 注入——mood_engine.get_prompt_injection() 追加到 system prompt 尾部
        # （neutral 档返回 ""，不污染 prompt；正向/负向档追加心情提示）。
        if self.mood_engine is not None:
            try:
                persona_prompt += self.mood_engine.get_prompt_injection()
            except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
                logger.warning(f"mood prompt injection failed (ignored): {e}")

        logger.debug("\n === System Prompt ===")
        logger.debug(persona_prompt)

        return persona_prompt

    async def handle_config_switch(
        self,
        websocket: WebSocket,
        config_file_name: str,
    ) -> None:
        """Handle the configuration switch request."""
        try:
            new_character_config_data = None

            if config_file_name == "conf.yaml":
                new_character_config_data = read_yaml("conf.yaml").get(
                    "character_config"
                )
            else:
                characters_dir = self.system_config.config_alts_dir
                file_path = os.path.normpath(
                    os.path.join(characters_dir, config_file_name)
                )
                if not file_path.startswith(characters_dir):
                    raise ValueError("Invalid configuration file path")

                alt_config_data = read_yaml(file_path).get("character_config")
                new_character_config_data = _deep_merge(
                    self.config.character_config.model_dump(), alt_config_data
                )

            if new_character_config_data:
                new_config = {
                    "system_config": self.system_config.model_dump(),
                    "character_config": new_character_config_data,
                }
                new_config = validate_config(new_config)
                await self.load_from_config(new_config)
                logger.debug(f"New config: {self}")
                logger.debug(
                    f"New character config: {self.character_config.model_dump()}"
                )

                await websocket.send_text(
                    json.dumps(
                        {
                            "type": "set-model-and-conf",
                            "model_info": self.live2d_model.model_info,
                            "conf_name": self.character_config.conf_name,
                            "conf_uid": self.character_config.conf_uid,
                            "live2d_fps": self.system_config.live2d_fps,
                        }
                    )
                )

                await websocket.send_text(
                    json.dumps(
                        {
                            "type": "config-switched",
                            "message": f"Switched to config: {config_file_name}",
                        }
                    )
                )

                logger.info(f"Configuration switched to {config_file_name}")
            else:
                raise ValueError(
                    f"Failed to load configuration from {config_file_name}"
                )

        except Exception as e:
            logger.error(f"Error switching configuration: {e}")
            logger.debug(self)
            await websocket.send_text(
                json.dumps(
                    {
                        "type": "error",
                        "message": f"Error switching configuration: {str(e)}",
                    }
                )
            )
            raise e


def _find_repo_shared_dir() -> str | None:
    """仓库 shared/ 目录——已收敛至 utils.repo_paths 单源（AUDIT P-1）。

    保留旧名兼容模块内调用；目录不存在时返回 None（与旧向上搜索语义一致）。
    """
    from .utils.repo_paths import shared_dir

    p = shared_dir()
    return str(p) if p.is_dir() else None


def _resolve_shared_path(config_path: str) -> str:
    """将 'shared/xxx' 解析为仓库 shared/ 目录下的绝对路径。

    由调用方解析后再传给 ModelRegistry，ModelRegistry 自身不假设工作目录。
    """
    return _resolve_shared_path_impl(config_path)


def _deep_merge(dict1, dict2):
    """Recursively merges dict2 into dict1, prioritizing values from dict2.

    仅服务本文件 handle_config_switch 的角色配置合并（私有，非通用工具）。
    """
    result = dict1.copy()
    for key, value in dict2.items():
        if key in result and isinstance(result[key], dict) and isinstance(value, dict):
            result[key] = _deep_merge(result[key], value)
        else:
            result[key] = value
    return result
