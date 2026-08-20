# -*- coding: utf-8 -*-
"""LLM API key 解析与校验（本项目策略：只用用户自己的 Key，不内置任何共享/免费 Key）。

策略（2026-08-20 起）：
  1. 仓库**不提交任何密钥**，也**不内置任何共享免费 Key**；
  2. 有效 Key 只有两个来源——仓库根 `.env`（由 run_server.py 经 python-dotenv 载入，
     例如 `DEEPSEEK_API_KEY=...`）或 `conf.yaml` 里 `llm_api_key` 的字面量；
  3. `conf.yaml` 里写的是 `'${DEEPSEEK_API_KEY}'` 这类占位符；环境变量未设置时
     `config_manager.utils.read_yaml` 会**保留字面文本**——把这串占位符当成 Key
     发给上游会得到一个语义模糊的 401，所以本模块在构造 LLM 前就明确拦下。

本模块替代已删除的 `free_glm_defaults.py`（那版会在 Key 缺失时回退到内置共享 Key，
与"移除免费模型与硬编码 Key"的项目方针冲突；模块名也已不再反映其真实职责）。
"""

from __future__ import annotations

# 本地/自托管推理服务（vLLM、LM Studio、Ollama、llama.cpp 等）通常不需要 Key，
# 空 Key 属于合法配置 → 只拦占位符，不强制要求非空。
_LOCAL_PROVIDERS = frozenset(
    {
        "openai_compatible_llm",
        "openai_compatible",
        "lmstudio_llm",
        "ollama_llm",
        "llama_cpp_llm",
    }
)


def is_unresolved_placeholder(raw_key: str | None) -> bool:
    """`${VAR}` 形态的未解析占位符（环境变量没设置，read_yaml 原样保留）。"""
    stripped = (raw_key or "").strip()
    return stripped.startswith("${") and stripped.endswith("}") and len(stripped) > 3


def resolve_llm_api_key(
    raw_key: str | None,
    provider: str | None = None,
    key_env: str | None = None,
) -> str:
    """返回可直接用于请求的 API Key，缺失/未解析时抛出带配置指引的 ValueError。

    Args:
        raw_key: `conf.yaml` 中 `llm_api_key` 的当前值（可能是 `${VAR}` 占位符）。
        provider: Provider id（用于报错文案，并判定是否为本地免 Key 服务）。
        key_env: 该 Provider 约定的环境变量名（用于报错文案），未知则从占位符里推断。

    Returns:
        去空白后的有效 Key；本地 Provider 允许返回空串。

    Raises:
        ValueError: Key 为空（非本地 Provider）或仍是未解析的 `${VAR}` 占位符。
            报错文案固定包含 ".env" 与 "conf.yaml" 两个关键词，便于用户按提示自查。
    """
    stripped = (raw_key or "").strip()
    hint_env = key_env or (stripped[2:-1] if is_unresolved_placeholder(stripped) else "")
    where = f"Provider「{provider}」" if provider else "当前 LLM Provider"

    if is_unresolved_placeholder(stripped):
        raise ValueError(
            f"{where} 的 API Key 仍是未解析的占位符 {stripped}："
            f"请在仓库根 .env 里设置 {hint_env or '对应环境变量'}，"
            "或直接在 conf.yaml 的 llm_api_key 填入你自己的 Key"
            "（本地配置，禁止提交；本项目不内置任何共享/免费 Key）。"
        )

    if not stripped and (provider or "") not in _LOCAL_PROVIDERS:
        raise ValueError(
            f"{where} 未配置 API Key："
            f"请在仓库根 .env 里设置 {hint_env or '对应环境变量'}，"
            "或在 conf.yaml 的 llm_api_key 填入你自己的 Key"
            "（本地配置，禁止提交；本项目不内置任何共享/免费 Key）。"
        )

    return stripped
