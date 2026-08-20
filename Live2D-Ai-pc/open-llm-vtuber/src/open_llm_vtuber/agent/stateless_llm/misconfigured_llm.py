# -*- coding: utf-8 -*-
"""配置缺失时的占位 LLM：让服务照常启动，但一开口就说清楚缺什么。

背景（为什么需要它）：
  本项目不内置任何共享/免费 Key（见 `api_key_resolver.py`），于是"全新克隆、还没填 Key"
  是一个**必然出现**的状态。如果这时直接在构造阶段抛错，`ServiceContext.init_agent`
  会把异常一路抛到 `WebSocketServer.initialize()` —— 后端根本起不来，用户连设置页都
  打不开，更没法在页面里填 Key，形成死锁。

因此这里做一次"延迟报错"：
  - **构造阶段**：只记一条 WARNING，返回本占位实现 → 服务正常启动，设置页可用；
  - **首次对话**：`chat_completion` 抛出带配置指引的 ValueError → 被
    `conversations/single_conversation.py` 捕获并以 `{"type":"error"}` 发给前端，
    用户在聊天面板里直接看到"去哪里填哪个 Key"。

这样既不牺牲"错误必须明确"，也不牺牲"用户能自助修好"。
"""

from typing import Any, AsyncIterator, Dict, List

from loguru import logger

from .stateless_llm_interface import StatelessLLMInterface


class MisconfiguredLLM(StatelessLLMInterface):
    """占位 LLM：任何一次对话都以明确的配置指引失败。

    Args:
        reason: 面向用户的原因说明（通常来自 `resolve_llm_api_key` 的报错文案）。
        provider: Provider id，仅用于日志与报错前缀。
    """

    def __init__(self, reason: str, provider: str = ""):
        self.reason = reason
        self.provider = provider
        logger.warning(
            f"LLM「{provider or 'unknown'}」未就绪（{reason}）："
            "服务已启动，可在设置页填写 Key；本次对话会给出同样的提示。"
        )

    async def chat_completion(
        self,
        messages: List[Dict[str, Any]],
        system: str = None,
        tools: List[Dict[str, Any]] = None,
    ) -> AsyncIterator[str]:
        raise ValueError(self.reason)
        # 让本方法保持 async generator 语义（与接口一致）；上面已抛错，不会执行到这里。
        yield ""  # pragma: no cover
