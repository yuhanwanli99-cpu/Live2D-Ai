from typing import Type

from loguru import logger

from .stateless_llm.stateless_llm_interface import StatelessLLMInterface
from .stateless_llm.stateless_llm_with_template import (
    AsyncLLMWithTemplate as StatelessLLMWithTemplate,
)
from .stateless_llm.openai_compatible_llm import AsyncLLM as OpenAICompatibleLLM
from .stateless_llm.ollama_llm import OllamaLLM
from .stateless_llm.claude_llm import AsyncLLM as ClaudeLLM
from .stateless_llm.misconfigured_llm import MisconfiguredLLM
from ..api_key_resolver import resolve_llm_api_key


# 设置页/路由支持的 OpenAI 兼容厂商别名；统一走 OpenAICompatibleLLM。
# 注意：这些别名在 StatelessLLMConfigs 中都有对应字段，避免热重载时被 Pydantic 丢弃。
_OPENAI_COMPATIBLE_PROVIDERS = {
    "openai_compatible_llm",
    "openai_compatible",
    "openai_llm",
    "gemini_llm",
    "zhipu_llm",
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
            # Key 缺失 / 仍是未解析的 ${VAR} 占位符 → 不把占位符当 Key 发给上游换回一个
            # 语义模糊的 401（见 api_key_resolver.py），但也**不能**在此阻断服务启动，
            # 否则用户打不开设置页去填 Key。改为返回占位 LLM：启动照常，一开口就说清缺什么。
            try:
                api_key = resolve_llm_api_key(
                    kwargs.get("llm_api_key"), provider=llm_provider
                )
            except ValueError as key_error:
                return MisconfiguredLLM(str(key_error), provider=llm_provider)
            return OpenAICompatibleLLM(
                model=kwargs.get("model"),
                base_url=kwargs.get("base_url"),
                llm_api_key=api_key,
                organization_id=kwargs.get("organization_id"),
                project_id=kwargs.get("project_id"),
                temperature=kwargs.get("temperature"),
                max_tokens=kwargs.get("max_tokens"),
                top_p=kwargs.get("top_p"),
                thinking_enabled=kwargs.get("thinking_enabled"),
                thinking_effort=kwargs.get("thinking_effort"),
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
