import os
import json
from typing import Callable
from loguru import logger
from fastapi import WebSocket

from prompts import prompt_loader
from .live2d_model import Live2dModel
from .model_registry import ModelRegistry
from .asr.asr_interface import ASRInterface
from .tts.tts_interface import TTSInterface
from .vad.vad_interface import VADInterface
from .agent.agents.agent_interface import AgentInterface
from .vision_handler import VisionHandler

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

    Live2Dai ships MCP server definitions in the cross-platform shared/mcp_tools.json
    file rather than requiring a project-local (gitignored) mcp_servers.json, so a
    fresh clone works out of the box. Falls back to the local file if the shared one
    is missing (e.g. running upstream Open-LLM-VTuber standalone).
    """
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
        if self.agent_engine and hasattr(self.agent_engine, "close"):
            await self.agent_engine.close()
        if self.vision_handler:
            await self.vision_handler.close()
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
        if not self.config:
            self.config = config

        if not self.system_config:
            self.system_config = config.system_config

        if not self.character_config:
            self.character_config = config.character_config

        # (NEW) 加载跨平台模型注册表（缺失/损坏时返回 None，回退硬编码 mao_pro）
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

        self.config = config
        self.system_config = config.system_config or self.system_config
        self.character_config = config.character_config

    def _load_model_registry(self) -> ModelRegistry | None:
        """加载 shared/model_registry.json；缺失/损坏时返回 None（回退硬编码 mao_pro）。"""
        registry_path = _resolve_shared_path("shared/model_registry.json")
        if not os.path.exists(registry_path):
            logger.error(
                f"Model registry file not found at {registry_path}; "
                "model selection validation disabled (fallback to 'mao_pro')."
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
                "model selection validation disabled (fallback to 'mao_pro')."
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

        # 回退：registry 可用 → 默认模型；registry 不可用 → 硬编码 'mao_pro'
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
        """registry 可用时返回默认模型 id；不可用时返回硬编码 'mao_pro'。"""
        if self.model_registry is not None:
            return self.model_registry.get_default().id
        return "mao_pro"

    def init_asr(self, asr_config: ASRConfig) -> None:
        if not self.asr_engine or (self.character_config.asr_config != asr_config):
            logger.info(f"Initializing ASR: {asr_config.asr_model}")
            self.asr_engine = ASRFactory.get_asr_system(
                asr_config.asr_model,
                **getattr(asr_config, asr_config.asr_model).model_dump(),
            )
            self.character_config.asr_config = asr_config
        else:
            logger.info("ASR already initialized with the same config.")

    def init_tts(self, tts_config: TTSConfig) -> None:
        if not self.tts_engine or (self.character_config.tts_config != tts_config):
            logger.info(f"Initializing TTS: {tts_config.tts_model}")
            self.tts_engine = TTSFactory.get_tts_engine(
                tts_config.tts_model,
                **getattr(tts_config, tts_config.tts_model.lower()).model_dump(),
            )
            self.character_config.tts_config = tts_config
        else:
            logger.info("TTS already initialized with the same config.")

    def init_vad(self, vad_config: VADConfig) -> None:
        if vad_config.vad_model is None:
            logger.info("VAD is disabled.")
            self.vad_engine = None
            return

        if not self.vad_engine or (self.character_config.vad_config != vad_config):
            logger.info(f"Initializing VAD: {vad_config.vad_model}")
            self.vad_engine = VADFactory.get_vad_engine(
                vad_config.vad_model,
                **getattr(vad_config, vad_config.vad_model.lower()).model_dump(),
            )
            self.character_config.vad_config = vad_config
        else:
            logger.info("VAD already initialized with the same config.")

    def init_vision(self, config: Config) -> None:
        """Initialize the vision handler from configuration if vision is enabled."""
        if self.vision_handler is not None:
            logger.debug("VisionHandler already initialized.")
            return

        vision_cfg = config.vision_config
        if vision_cfg is None or not vision_cfg.vision_enabled:
            logger.info(
                "Vision pipeline disabled (vision_config.vision_enabled=False or not configured)."
            )
            self.vision_handler = None
            return

        # Determine API key: use vision-specific key, or fall back to zhipu_llm key
        api_key = vision_cfg.api_key
        if not api_key:
            zhipu = config.character_config.agent_config.llm_configs.zhipu_llm
            if zhipu and zhipu.llm_api_key:
                api_key = zhipu.llm_api_key
                logger.debug("Using zhipu_llm API key for vision.")

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
                model=vision_cfg.model,
                base_url=base_url,
            )
            logger.info(
                f"VisionHandler initialized (model={vision_cfg.model}, "
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

        except Exception as e:
            logger.error(f"Failed to initialize agent: {e}")
            raise

    async def construct_system_prompt(self, persona_prompt: str) -> str:
        """Append tool prompts to persona prompt."""
        logger.debug(f"constructing persona_prompt: '''{persona_prompt}'''")

        for prompt_name, prompt_file in self.system_config.tool_prompts.items():
            if (
                prompt_name == "group_conversation_prompt"
                or prompt_name == "proactive_speak_prompt"
            ):
                continue

            prompt_content = prompt_loader.load_util(prompt_file)

            if prompt_name == "live2d_expression_prompt":
                prompt_content = prompt_content.replace(
                    "[<insert_emomap_keys>]", self.live2d_model.emo_str
                )

            if prompt_name == "mcp_prompt":
                continue

            persona_prompt += prompt_content

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
                new_character_config_data = deep_merge(
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
    """向上查找仓库根目录下的 shared/ 目录（不依赖 CWD）。

    从本模块文件位置逐级向上，找到第一个含 shared/ 的目录即返回；
    找不到返回 None。
    """
    current = os.path.dirname(os.path.abspath(__file__))
    while True:
        candidate = os.path.join(current, "shared")
        if os.path.isdir(candidate):
            return candidate
        parent = os.path.dirname(current)
        if parent == current:
            return None
        current = parent


def _resolve_shared_path(config_path: str) -> str:
    """将 'shared/xxx' 解析为仓库 shared/ 目录下的绝对路径。

    与 run_server.py 的 `_resolve_shared_path()` 语义一致（plan 风险 #3）：
    由调用方解析后再传给 ModelRegistry，ModelRegistry 自身不假设工作目录。
    """
    if config_path.startswith("shared/"):
        shared_dir = _find_repo_shared_dir()
        if shared_dir is not None:
            return os.path.join(shared_dir, config_path[len("shared/") :])
    return config_path


def deep_merge(dict1, dict2):
    """Recursively merges dict2 into dict1, prioritizing values from dict2."""
    result = dict1.copy()
    for key, value in dict2.items():
        if key in result and isinstance(result[key], dict) and isinstance(value, dict):
            result[key] = deep_merge(result[key], value)
        else:
            result[key] = value
    return result
