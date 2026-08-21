"""零配置 API key 解析（PC 端 spoq-fix M-SEC：密钥不提交）。

本模块不内置任何共享/免费 Key。conf.yaml 中的 llm_api_key 只写
${XXX_API_KEY} 占位符，真实 Key 放仓库根 .env / local.properties
（gitignored）。这里只负责在构造 LLM 前校验并解析用户提供的 Key。
"""
from __future__ import annotations

from typing import Optional


def resolve_llm_api_key(raw_key: Optional[str]) -> str:
    """解析并校验用户提供的 LLM API Key。

    规则：
    - 空值 / None / 纯空白 → 抛 ValueError（含 .env / conf.yaml 指引）
    - 未解析的 ${VAR} 占位符 → 抛 ValueError（不内置共享 Key，绝不静默回退）
    - 用户显式 Key → 原样保留（去首尾空白后返回）

    目的：密钥不提交到仓库，也不在代码里内置任何一把真实 Key。
    """
    key = (raw_key or "").strip()
    if not key:
        raise ValueError(
            "LLM API key 为空。请在仓库根目录 .env 或 conf.yaml 中配置 "
            "对应提供商的 API key（如 DEEPSEEK_API_KEY / ZHIPU_API_KEY）。"
        )
    if key.startswith("${") and key.endswith("}"):
        raise ValueError(
            "LLM API key 仍是未解析的环境变量占位符。请在仓库根目录 .env "
            "或 conf.yaml 中配置真实 API key（如 DEEPSEEK_API_KEY / "
            "ZHIPU_API_KEY），不要提交共享 Key。"
        )
    return key
