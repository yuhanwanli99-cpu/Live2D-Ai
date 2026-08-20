# -*- coding: utf-8 -*-
"""API Key 解析测试（`src/open_llm_vtuber/api_key_resolver.py`）。

项目方针：**不内置任何共享/免费 Key**。因此：
  - 未解析的 `${VAR}` 占位符一律拒绝（否则会被当 Bearer 发给上游，换回语义模糊的 401）；
  - 云端 Provider 空 Key 拒绝，本地自托管 Provider 空 Key 放行；
  - 报错文案必须同时含 ".env" 与 "conf.yaml" 指引关键词。
"""

import pytest

from open_llm_vtuber.api_key_resolver import is_unresolved_placeholder, resolve_llm_api_key


class TestPlaceholderDetection:
    @pytest.mark.parametrize("raw", ["${ZHIPU_API_KEY}", "  ${DEEPSEEK_API_KEY}  "])
    def test_detects_placeholder(self, raw):
        assert is_unresolved_placeholder(raw) is True

    @pytest.mark.parametrize("raw", ["", None, "sk-real-key", "${}", "$ZHIPU", "{VAR}"])
    def test_rejects_non_placeholder(self, raw):
        assert is_unresolved_placeholder(raw) is False


class TestResolve:
    def test_passes_through_real_key_trimmed(self):
        assert resolve_llm_api_key("  sk-abc123  ", provider="deepseek_llm") == "sk-abc123"

    def test_placeholder_raises_with_guidance(self):
        with pytest.raises(ValueError) as exc:
            resolve_llm_api_key("${DEEPSEEK_API_KEY}", provider="deepseek_llm")
        message = str(exc.value)
        assert ".env" in message and "conf.yaml" in message
        assert "DEEPSEEK_API_KEY" in message

    def test_placeholder_raises_even_for_local_provider(self):
        # 占位符永远是配置事故：本地 Provider 也不该把 "${X}" 当 Key 用
        with pytest.raises(ValueError):
            resolve_llm_api_key("${OPENAI_API_KEY}", provider="openai_compatible_llm")

    @pytest.mark.parametrize("provider", ["deepseek_llm", "zhipu_llm", "qwen_llm", "openai"])
    def test_empty_key_rejected_for_cloud_providers(self, provider):
        with pytest.raises(ValueError) as exc:
            resolve_llm_api_key("", provider=provider)
        assert ".env" in str(exc.value) and "conf.yaml" in str(exc.value)

    @pytest.mark.parametrize(
        "provider", ["openai_compatible_llm", "lmstudio_llm", "ollama_llm", "llama_cpp_llm"]
    )
    def test_empty_key_allowed_for_local_providers(self, provider):
        assert resolve_llm_api_key("", provider=provider) == ""
        assert resolve_llm_api_key(None, provider=provider) == ""

    def test_key_env_hint_used_when_given(self):
        with pytest.raises(ValueError) as exc:
            resolve_llm_api_key("", provider="qwen_llm", key_env="DASHSCOPE_API_KEY")
        assert "DASHSCOPE_API_KEY" in str(exc.value)

    def test_no_builtin_shared_key_is_ever_returned(self):
        """回归保护：曾经的 free_glm_defaults 会回退到内置共享 Key —— 现在必须抛错。"""
        with pytest.raises(ValueError):
            resolve_llm_api_key(None, provider="zhipu_llm")


class TestFactoryIntegration:
    def test_factory_returns_placeholder_llm_instead_of_blocking_startup(self):
        """Key 未配置时不能在构造阶段抛错（否则后端起不来、设置页打不开）。

        约定：返回 MisconfiguredLLM 占位实现 → 服务照常启动；
        真正对话时才抛出带 .env / conf.yaml 指引的错误。
        """
        import asyncio

        from open_llm_vtuber.agent.stateless_llm.misconfigured_llm import MisconfiguredLLM
        from open_llm_vtuber.agent.stateless_llm_factory import LLMFactory

        llm = LLMFactory.create_llm(
            "deepseek_llm",
            model="deepseek-v4-flash",
            base_url="https://api.deepseek.com/v1",
            llm_api_key="${DEEPSEEK_API_KEY}",
        )
        assert isinstance(llm, MisconfiguredLLM)

        async def consume():
            async for _ in llm.chat_completion([{"role": "user", "content": "hi"}]):
                pass

        with pytest.raises(ValueError) as exc:
            asyncio.run(consume())
        assert ".env" in str(exc.value) and "conf.yaml" in str(exc.value)

    def test_factory_builds_real_llm_when_key_present(self):
        from open_llm_vtuber.agent.stateless_llm.misconfigured_llm import MisconfiguredLLM
        from open_llm_vtuber.agent.stateless_llm_factory import LLMFactory

        llm = LLMFactory.create_llm(
            "deepseek_llm",
            model="deepseek-v4-flash",
            base_url="https://api.deepseek.com/v1",
            llm_api_key="sk-real-key",
        )
        assert not isinstance(llm, MisconfiguredLLM)

    def test_free_provider_removed_from_factory(self):
        """`zhipu_free_llm` 已整体移除：不再是受支持的 provider。"""
        from open_llm_vtuber.agent.stateless_llm_factory import (
            _OPENAI_COMPATIBLE_PROVIDERS,
            LLMFactory,
        )

        assert "zhipu_free_llm" not in _OPENAI_COMPATIBLE_PROVIDERS
        with pytest.raises(ValueError, match="Unsupported LLM provider"):
            LLMFactory.create_llm("zhipu_free_llm", model="glm-4.7-flash")

    def test_free_provider_removed_from_schema(self):
        from open_llm_vtuber.config_manager.stateless_llm import StatelessLLMConfigs

        assert "zhipu_free_llm" not in StatelessLLMConfigs.model_fields
