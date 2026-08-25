"""Regression tests for MCP retirement in the stable PC build."""

import asyncio
from unittest.mock import MagicMock, patch

from src.open_llm_vtuber.agent.agent_factory import AgentFactory
from src.open_llm_vtuber.config_manager.agent import BasicMemoryAgentConfig
from src.open_llm_vtuber.service_context import ServiceContext


def test_legacy_mcp_true_is_ignored_and_warns():
    """An old config with use_mcpp=True must not crash startup."""
    ctx = ServiceContext()
    with patch("src.open_llm_vtuber.service_context.logger.warning") as mock_warn:
        asyncio.run(
            ctx._init_mcp_components(use_mcpp=True, enabled_servers=["legacy_server"])
        )

    # All MCP runtime handles must stay None; no external mcp package is touched.
    assert ctx.mcp_client is None
    assert ctx.tool_manager is None
    assert ctx.tool_executor is None
    assert ctx.mcp_server_registry is None
    assert ctx.tool_adapter is None
    assert ctx.mcp_prompt == ""

    # A clear retirement warning is emitted so users know why tools are absent.
    assert mock_warn.call_count == 1
    message = mock_warn.call_args[0][0]
    assert "retired" in message.lower()


def test_legacy_mcp_fields_still_load_in_config_model():
    """The config model must accept legacy MCP fields without validation errors."""
    cfg = BasicMemoryAgentConfig(
        llm_provider="deepseek_llm",
        use_mcpp=True,
        mcp_enabled_servers=["legacy_server"],
    )
    assert cfg.use_mcpp is True
    assert cfg.mcp_enabled_servers == ["legacy_server"]


def test_agent_factory_forces_use_mcpp_false():
    """AgentFactory must never hand use_mcpp=True down to BasicMemoryAgent."""
    dummy_llm = MagicMock()
    agent_settings = {
        "basic_memory_agent": {
            "llm_provider": "deepseek_llm",
            "use_mcpp": True,
            "mcp_enabled_servers": ["legacy_server"],
        }
    }
    llm_configs = {
        "deepseek_llm": {
            "base_url": "https://api.example.com/v1",
            "model": "test-model",
            "api_key": "sk-test",
        }
    }

    with patch(
        "src.open_llm_vtuber.agent.agent_factory.StatelessLLMFactory.create_llm",
        return_value=dummy_llm,
    ):
        agent = AgentFactory.create_agent(
            conversation_agent_choice="basic_memory_agent",
            agent_settings=agent_settings,
            llm_configs=llm_configs,
            system_prompt="You are a test assistant.",
        )

    assert isinstance(agent._use_mcpp, bool)
    assert agent._use_mcpp is False
    assert agent._tool_manager is None
    assert agent._tool_executor is None
