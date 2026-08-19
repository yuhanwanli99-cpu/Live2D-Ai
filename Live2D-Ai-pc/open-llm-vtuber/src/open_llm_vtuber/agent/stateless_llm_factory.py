from typing import Type

from loguru import logger

from .stateless_llm.stateless_llm_interface import StatelessLLMInterface
from .stateless_llm.stateless_llm_with_template import (
    AsyncLLMWithTemplate as StatelessLLMWithTemplate,
)
from .stateless_llm.openai_compatible_llm import AsyncLLM as OpenAICompatibleLLM
from .stateless_llm.ollama_llm import OllamaLLM
from .stateless_llm.claude_llm import AsyncLLM as ClaudeLLM
from ..free_glm_defaults import resolve_llm_api_key


# 设置页/路由支持的 OpenAI 兼容厂商别名；统一走 OpenAICompatibleLLM。
# 注意：这些别名在 StatelessLLMConfigs 中都有对应字段，避免热重载时被 Pydantic 丢弃。
_OPENAI_COMPATIBLE_PROVIDERS = {
    "openai_compatible_llm",
    "openai_compatible",
    "openai_llm",
    "gemini_llm",
    "zhipu_llm",
    "zhipu_free_llm",
    "deepseek_llm",
    "groq_llm",
    "mistral_llm",
    "lmstudio_llm",
    "qwen_llm",
    "openrouter",
    "openai",
    "moonshot",
    "together",
    "fireworks",
    "siliconflow",
    "opencode",
    "opencode_go",
    "groq",
    "mistral",
}


class LLMFactory:
    @staticmethod
    def create_llm(llm_provider, **kwargs) -> Type[StatelessLLMInterface]:
        """Create an LLM based on the configuration.

        Args:
            llm_provider: The type of LLM to create
            **kwargs: Additional arguments
        """
        logger.info(f"Initializing LLM: {llm_provider}")

        if llm_provider in _OPENAI_COMPATIBLE_PROVIDERS:
            # 零配置默认 Provider：llm_api_key 为空/未解析的 ${VAR} 占位符时
            # resolve_llm_api_key 抛明确报错（本地配置 key，见 free_glm_defaults.py）。
            api_key = kwargs.get("llm_api_key")
            if llm_provider == "zhipu_free_llm":
                api_key = resolve_llm_api_key(api_key)
            return OpenAICompatibleLLM(
                model=kwargs.get("model"),
                base_url=kwargs.get("base_url"),
                llm_api_key=api_key,
                organization_id=kwargs.get("organization_id"),
                project_id=kwargs.get("project_id"),
                temperature=kwargs.get("temperature"),
            )
        if llm_provider == "stateless_llm_with_template":
            return StatelessLLMWithTemplate(
                model=kwargs.get("model"),
                base_url=kwargs.get("base_url"),
                llm_api_key=kwargs.get("llm_api_key"),
                organization_id=kwargs.get("organization_id"),
                template=kwargs.get("template"),
                project_id=kwargs.get("project_id"),
            )
        if llm_provider == "ollama_llm":
            return OllamaLLM(
                model=kwargs.get("model"),
                base_url=kwargs.get("base_url"),
                llm_api_key=kwargs.get("llm_api_key"),
                organization_id=kwargs.get("organization_id"),
                project_id=kwargs.get("project_id"),
                temperature=kwargs.get("temperature"),
                keep_alive=kwargs.get("keep_alive"),
                unload_at_exit=kwargs.get("unload_at_exit"),
            )

        elif llm_provider == "llama_cpp_llm":
            from .stateless_llm.llama_cpp_llm import LLM as LlamaLLM

            return LlamaLLM(
                model_path=kwargs.get("model_path"),
            )
        elif llm_provider in ("claude_llm", "anthropic"):
            return ClaudeLLM(
                system=kwargs.get("system_prompt"),
                base_url=kwargs.get("base_url"),
                model=kwargs.get("model"),
                llm_api_key=kwargs.get("llm_api_key"),
            )
        else:
            raise ValueError(f"Unsupported LLM provider: {llm_provider}")


# Creating an LLM instance using a factory
# llm_instance = LLMFactory.create_llm("ollama", **config_dict)
