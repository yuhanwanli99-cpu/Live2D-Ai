"""Description: This file contains the implementation of the `AsyncLLM` class.
This class is responsible for handling asynchronous interaction with OpenAI API compatible
endpoints for language generation.
"""

from typing import AsyncIterator, List, Dict, Any
from openai import (
    AsyncStream,
    AsyncOpenAI,
    APIError,
    APIConnectionError,
    BadRequestError,
    RateLimitError,
    NotGiven,
    NOT_GIVEN,
)
from openai.types.chat import ChatCompletionChunk
from openai.types.chat.chat_completion_chunk import ChoiceDeltaToolCall
from loguru import logger

from .stateless_llm_interface import StatelessLLMInterface
from ...mcpp.types import ToolCallObject


class ReasoningDelta(str):
    """思维链增量标记：isinstance(str) 但可被中间层识别并单独转发，
    不混入正文（basic_memory_agent 不累积、不入记忆）。"""


class LLMChatError(RuntimeError):
    """LLM API 调用失败（连接失败 / 限流 / HTTP 错误等）。

    以异常（而非 yield 错误字符串）向外传播：任何中间层都不会把它误当成
    assistant 正文送进 TTS/字幕/记忆，也不会触发多段续聊；
    agent/transformers 的 pump 会统一转成 ``{"type": "stream_error"}`` 事件，
    由对话层向前端发送单一可读错误提示。
    """


class AsyncLLM(StatelessLLMInterface):
    def __init__(
        self,
        model: str,
        base_url: str,
        llm_api_key: str = "z",
        organization_id: str = "z",
        project_id: str = "z",
        temperature: float = 1.0,
        max_tokens: int | None = None,
        top_p: float | None = None,
        thinking_enabled: bool | None = None,
        thinking_effort: str | None = None,
    ):
        """
        Initializes an instance of the `AsyncLLM` class.

        Parameters:
        - model (str): The model to be used for language generation.
        - base_url (str): The base URL for the OpenAI API.
        - organization_id (str, optional): The organization ID for the OpenAI API. Defaults to "z".
        - project_id (str, optional): The project ID for the OpenAI API. Defaults to "z".
        - llm_api_key (str, optional): The API key for the OpenAI API. Defaults to "z".
        - temperature (float, optional): What sampling temperature to use, between 0 and 2. Defaults to 1.0.
        - max_tokens (int, optional): Maximum tokens for the completion; None means provider default.
        - top_p (float, optional): Nucleus sampling; None means provider default.
        - thinking_enabled (bool, optional): DeepSeek thinking mode toggle;
          None/False keeps it disabled (VTuber chat default).
        - thinking_effort (str, optional): low/high/max, passed as reasoning_effort
          when thinking is enabled.
        """
        self.base_url = base_url
        self.model = model
        self.temperature = temperature
        self.max_tokens = max_tokens
        self.top_p = top_p
        self.thinking_enabled = bool(thinking_enabled)
        self.thinking_effort = thinking_effort
        self.client = AsyncOpenAI(
            base_url=base_url,
            organization=organization_id,
            project=project_id,
            api_key=llm_api_key,
        )
        self.support_tools = True

        logger.info(
            f"Initialized AsyncLLM with the parameters: {self.base_url}, {self.model}"
        )

    async def chat_completion(
        self,
        messages: List[Dict[str, Any]],
        system: str = None,
        tools: List[Dict[str, Any]] | NotGiven = NOT_GIVEN,
    ) -> AsyncIterator[str | List[ChoiceDeltaToolCall]]:
        """
        Generates a chat completion using the OpenAI API asynchronously.

        Parameters:
        - messages (List[Dict[str, Any]]): The list of messages to send to the API.
        - system (str, optional): System prompt to use for this completion.
        - tools (List[Dict[str, str]], optional): List of tools to use for this completion.

        Yields:
        - str: The content of each chunk from the API response.
        - List[ChoiceDeltaToolCall]: The tool calls detected in the response.

        Raises:
        - LLMChatError: 当 API 调用失败（连接失败 / 限流 / HTTP 错误）。
          绝不以正文形式 yield 错误字符串（错误不得进入 TTS/字幕/记忆）。

        Notes:
        - 携带 tools 的请求被 provider 以 HTTP 400 拒绝时（典型原因：工具函数名
          不满足 ^[a-zA-Z0-9_-]+$），自动去掉 tools 重试一次，并把本进程内
          ``support_tools`` 置为 False（后续请求不再携带 tools，安全降级）。
        """
        stream = None
        # Tool call related state variables
        accumulated_tool_calls = {}
        in_tool_call = False

        try:
            # If system prompt is provided, add it to the messages
            messages_with_system = messages
            if system:
                messages_with_system = [
                    {"role": "system", "content": system},
                    *messages,
                ]
            logger.debug(f"Messages: {messages_with_system}")

            available_tools = tools if self.support_tools else NOT_GIVEN
            # 只有真的携带了非空工具列表时，才允许「400 → 无 tools 重试」降级路径
            tools_actually_offered = (
                self.support_tools
                and available_tools is not NOT_GIVEN
                and bool(available_tools)
            )

            # 思考模式：默认关闭（VTuber 对话低延迟优先）；配置开启时按
            # DeepSeek 官方约定传 extra_body.thinking + reasoning_effort。
            # 注意：思考模式下 DeepSeek 忽略 temperature/top_p（兼容性忽略，不报错）。
            extra_body = None
            if self.thinking_enabled:
                if "deepseek" in self.model.lower():
                    extra_body = {"thinking": {"type": "enabled"}}
            elif "deepseek" in self.model.lower():
                # V4 系列默认开启思考；桌宠对话显式关闭
                extra_body = {"thinking": {"type": "disabled"}}

            completion_kwargs: Dict[str, Any] = {
                "messages": messages_with_system,
                "model": self.model,
                "stream": True,
                "temperature": self.temperature,
                "extra_body": extra_body,
                "tools": available_tools,
            }
            if self.top_p is not None:
                completion_kwargs["top_p"] = self.top_p
            if self.thinking_enabled and self.thinking_effort:
                completion_kwargs["reasoning_effort"] = self.thinking_effort
            if self.max_tokens is not None:
                completion_kwargs["max_tokens"] = self.max_tokens

            try:
                stream: AsyncStream[
                    ChatCompletionChunk
                ] = await self.client.chat.completions.create(**completion_kwargs)
            except BadRequestError as e:
                status_code = getattr(e, "status_code", None)
                if not (tools_actually_offered and status_code == 400):
                    raise
                # tools 参数被 provider 拒绝（HTTP 400）：识别后自动无 tools 重试一次，
                # 并在本进程内永久停用 tools（安全降级，不再反复撞同一堵墙）。
                # 典型根因：function.name 含点号等非法字符（必须匹配
                # ^[a-zA-Z0-9_-]+$）。错误绝不作为正文下发。
                self.support_tools = False
                logger.warning(
                    "Provider rejected the request with HTTP 400 while tools were "
                    f"offered (model={self.model}, base_url={self.base_url}). "
                    "Tool function names must match ^[a-zA-Z0-9_-]+$. "
                    f"Retrying ONCE without tools. Detail: {e}"
                )
                completion_kwargs["tools"] = NOT_GIVEN
                stream: AsyncStream[
                    ChatCompletionChunk
                ] = await self.client.chat.completions.create(**completion_kwargs)
            logger.debug(
                f"Tool Support: {self.support_tools}, Available tools: {available_tools}"
            )

            async for chunk in stream:
                # Guard against chunks with missing choices field (e.g., from OpenWebUI)
                if not chunk.choices:
                    continue

                if self.support_tools:
                    has_tool_calls = (
                        hasattr(chunk.choices[0].delta, "tool_calls")
                        and chunk.choices[0].delta.tool_calls
                    )

                    if has_tool_calls:
                        logger.debug(
                            f"Tool calls detected in chunk: {chunk.choices[0].delta.tool_calls}"
                        )
                        in_tool_call = True
                        # Process tool calls in the current chunk
                        for tool_call in chunk.choices[0].delta.tool_calls:
                            index = (
                                tool_call.index if hasattr(tool_call, "index") else 0
                            )

                            # Initialize tool call for this index if needed
                            if index not in accumulated_tool_calls:
                                accumulated_tool_calls[index] = {
                                    "index": index,
                                    "id": getattr(tool_call, "id", None),
                                    "type": getattr(tool_call, "type", None),
                                    "function": {"name": "", "arguments": ""},
                                }

                            # Update tool call information
                            if hasattr(tool_call, "id") and tool_call.id:
                                accumulated_tool_calls[index]["id"] = tool_call.id
                            if hasattr(tool_call, "type") and tool_call.type:
                                accumulated_tool_calls[index]["type"] = tool_call.type

                            # Update function information
                            if hasattr(tool_call, "function"):
                                if (
                                    hasattr(tool_call.function, "name")
                                    and tool_call.function.name
                                ):
                                    accumulated_tool_calls[index]["function"][
                                        "name"
                                    ] = tool_call.function.name
                                if (
                                    hasattr(tool_call.function, "arguments")
                                    and tool_call.function.arguments
                                ):
                                    accumulated_tool_calls[index]["function"][
                                        "arguments"
                                    ] += tool_call.function.arguments

                        continue

                    # If we were in a tool call but now we're not, yield the tool call result
                    elif in_tool_call and not has_tool_calls:
                        in_tool_call = False
                        # Convert accumulated tool calls to the required format and output
                        logger.info(f"Complete tool calls: {accumulated_tool_calls}")

                        # Use the from_dict method to create a ToolCallObject instance from a dictionary
                        complete_tool_calls = [
                            ToolCallObject.from_dict(tool_data)
                            for tool_data in accumulated_tool_calls.values()
                        ]

                        yield complete_tool_calls
                        accumulated_tool_calls = {}  # Reset for potential future tool calls

                # Process regular content chunks
                if len(chunk.choices) == 0:
                    logger.info("Empty chunk received")
                    continue

                # 思维链增量（DeepSeek reasoning_content）单独成通道，
                # 不进入正文/记忆；无思考时该属性缺省为 None。
                reasoning_piece = getattr(
                    chunk.choices[0].delta, "reasoning_content", None
                )
                if reasoning_piece:
                    yield ReasoningDelta(reasoning_piece)

                if chunk.choices[0].delta.content is None:
                    chunk.choices[0].delta.content = ""
                if chunk.choices[0].delta.content:
                    yield chunk.choices[0].delta.content

            # If stream ends while still in a tool call, make sure to yield the tool call
            if in_tool_call and accumulated_tool_calls:
                logger.info(f"Final tool call at stream end: {accumulated_tool_calls}")

                # Create a ToolCallObject instance from a dictionary using the from_dict method.
                complete_tool_calls = [
                    ToolCallObject.from_dict(tool_data)
                    for tool_data in accumulated_tool_calls.values()
                ]

                yield complete_tool_calls

        except APIConnectionError as e:
            logger.error(
                f"Error calling the chat endpoint: Connection error. Failed to connect to the LLM API. \nCheck the configurations and the reachability of the LLM backend. \nSee the logs for details. \nTroubleshooting with documentation: https://open-llm-vtuber.github.io/docs/faq#%E9%81%87%E5%88%B0-error-calling-the-chat-endpoint-%E9%94%99%E8%AF%AF%E6%80%8E%E4%B9%88%E5%8A%9E \n{e.__cause__}"
            )
            # 以异常传播（绝不 yield 错误字符串）：错误不得被当作 assistant
            # 正文送进 TTS/字幕/记忆；由上层转成前端单一可读错误。
            raise LLMChatError(
                "无法连接到 LLM 服务：请检查网络与 API 地址（base_url）配置。"
            ) from e

        except RateLimitError as e:
            logger.error(
                f"Error calling the chat endpoint: Rate limit exceeded: {e.response}"
            )
            raise LLMChatError("LLM 服务限流（429），请稍后再试。") from e

        except APIError as e:
            if "does not support tools" in str(e):
                self.support_tools = False
                logger.warning(
                    f"{self.model} does not support tools. Disabling tool support."
                )
                yield "__API_NOT_SUPPORT_TOOLS__"
                return
            logger.error(f"LLM API: Error occurred: {e}")
            logger.info(f"Base URL: {self.base_url}")
            logger.info(f"Model: {self.model}")
            logger.info(f"Messages: {messages}")
            logger.info(f"temperature: {self.temperature}")
            raise LLMChatError(
                f"LLM 接口调用失败（{type(e).__name__}），请查看后端日志获取详情。"
            ) from e

        finally:
            # make sure the stream is properly closed
            # so when interrupted, no more tokens will being generated.
            if stream:
                logger.debug("Chat completion finished.")
                await stream.close()
                logger.debug("Stream closed.")
