import asyncio
import re
from dataclasses import replace
from typing import Optional, Union, Any, List, Dict
import numpy as np
import json
from ..logsetup import get_logger

from ..message_handler import message_handler
from .types import WebSocketSend, BroadcastContext
from .tts_manager import TTSTaskManager
from ..agent.output_types import SentenceOutput, AudioOutput, DisplayText
from ..agent.input_types import BatchInput, TextData, ImageData, TextSource, ImageSource
from ..asr.asr_interface import ASRInterface
from ..live2d_model import Live2dModel
from ..tts.tts_interface import TTSInterface
from ..director import normalize_chunk_timeline, project_motion_cues_into_chunks
from ..utils.stream_audio import prepare_audio_payload
from ..utils.tts_preprocessor import filter_parentheses
# 对话编排链路统一 "flow" 标签（字幕/分块/插件分发等关键节点）
logger = get_logger("flow")


def split_tts_chunks(text: str, max_chars: int = 36) -> list[str]:
    """把一段 TTS 文本切成小块，降低单次 CPU 合成时长、更快开始播放。

    优先在句子/逗号等自然断点处切，尽量保留标点；窗口内无断点时按 max_chars 硬切。
    """
    if not text:
        return []
    text = text.strip()
    if not text:
        return []
    if len(text) <= max_chars:
        return [text]
    chunks: list[str] = []
    rest = text
    while len(rest) > max_chars:
        window = rest[:max_chars]
        cut = -1
        # 在窗口内找最后一个自然断点（句子结束 → 中文/英文逗号等）
        for punct in "。！？！?!；;，、,.：:":
            idx = window.rfind(punct)
            if idx >= 0:
                cut = max(cut, idx)
        if cut < 0:
            cut = max_chars - 1
        chunk = rest[: cut + 1].strip()
        if chunk:
            chunks.append(chunk)
        rest = rest[cut + 1 :].strip()
    if rest:
        chunks.append(rest)
    return chunks



# Convert class methods to standalone functions
def create_batch_input(
    input_text: str,
    images: Optional[List[Dict[str, Any]]],
    from_name: str,
    metadata: Optional[Dict[str, Any]] = None,
) -> BatchInput:
    """Create batch input for agent processing"""
    return BatchInput(
        texts=[
            TextData(source=TextSource.INPUT, content=input_text, from_name=from_name)
        ],
        images=[
            ImageData(
                source=ImageSource(img["source"]),
                data=img["data"],
                mime_type=img["mime_type"],
            )
            for img in (images or [])
        ]
        if images
        else None,
        metadata=metadata,
    )


async def process_agent_output(
    output: Union[AudioOutput, SentenceOutput],
    character_config: Any,
    live2d_model: Live2dModel,
    tts_engine: TTSInterface,
    websocket_send: WebSocketSend,
    tts_manager: TTSTaskManager,
    translate_engine: Optional[Any] = None,
    mood_tier: str = "neutral",  # PC-L4: mood 分层透传（furious/upset/neutral/happy/ecstatic）
    plugin_manager=None,
) -> str:
    """Process agent output with character information and optional translation"""
    output.display_text.name = character_config.character_name
    output.display_text.avatar = character_config.avatar

    full_response = ""
    try:
        if isinstance(output, SentenceOutput):
            full_response = await handle_sentence_output(
                output,
                live2d_model,
                tts_engine,
                websocket_send,
                tts_manager,
                translate_engine,
                mood_tier,
                plugin_manager,
            )
        elif isinstance(output, AudioOutput):
            full_response = await handle_audio_output(output, websocket_send)
        else:
            logger.warning(f"Unknown output type: {type(output)}")
    except Exception as e:  # noqa: BLE001
        logger.error(f"Error processing agent output: {e}")
        await websocket_send(
            json.dumps(
                {"type": "error", "message": f"Error processing response: {str(e)}"}
            )
        )

    return full_response


async def handle_sentence_output(
    output: SentenceOutput,
    live2d_model: Live2dModel,
    tts_engine: TTSInterface,
    websocket_send: WebSocketSend,
    tts_manager: TTSTaskManager,
    translate_engine: Optional[Any] = None,
    mood_tier: str = "neutral",  # PC-L4: mood 分层透传（furious/upset/neutral/happy/ecstatic）
    plugin_manager=None,
) -> str:
    """Handle sentence output type with optional translation support"""
    full_response = ""
    async for display_text, tts_text, actions in output:
        logger.debug(f"🏃 Processing output: '''{tts_text}'''...")

        if translate_engine:
            if len(re.sub(r'[\s.,!?，。！？\'"』」）】\s]+', "", tts_text)):
                tts_text = translate_engine.translate(tts_text)
            logger.info(f"🏃 Text after translation: '''{tts_text}'''...")
        else:
            logger.debug("🚫 No translation engine available. Skipping translation.")

        # 括注舞台说明（（）/()）不展示、不朗读：TTS 侧由 filter_parentheses 处理，
        # 展示侧在这里同步清洗（包括中文全角括号）。
        if display_text.text:
            clean_display_text = filter_parentheses(display_text.text)
            if clean_display_text != display_text.text:
                display_text = DisplayText(
                    text=clean_display_text,
                    name=display_text.name,
                    avatar=display_text.avatar,
                )

        full_response += display_text.text

        # 流式字幕：句子切出后立即上屏，不等 TTS 合成/后处理完成
        if display_text.text:
            await websocket_send(
                json.dumps(
                    {
                        "type": "subtitle",
                        "text": display_text.text,
                        "name": display_text.name,
                    }
                )
            )

        # PC v4 多脑编排：从 ExpressionDirector 读取当前句 emotion/strength，
        # 让 tts_manager 在下发 audio 前先发独立 emotion 帧，驱动 Live2D 表情。
        _director = getattr(output, "_director_result", None)
        sentence_emotion = getattr(_director, "emotion", "neutral") or "neutral"
        try:
            sentence_strength = int(getattr(_director, "strength", 2) or 2)
        except (TypeError, ValueError):
            sentence_strength = 2

        # PC v4 多脑编排：sentence.after 允许 body/mood 导演插件对当前句做兜底决策。
        if plugin_manager is not None and getattr(plugin_manager, "host", None) is not None:
            try:
                events = plugin_manager.host.emit_event(
                    "sentence.after",
                    {
                        "text": display_text.text,
                        "emotion": sentence_emotion,
                        "strength": sentence_strength,
                        "mood_tier": mood_tier,
                        "parameterOverrides": (actions.parameterOverrides or {}) if actions else {},
                        "motion": actions.motion if actions else None,
                    },
                )
                for event in events:
                    if not event.get("ok"):
                        logger.warning(
                            f"plugin {event.get('plugin')} failed on sentence.after: {event.get('error')}"
                        )
                        continue
                    result = event.get("result")
                    if not isinstance(result, dict):
                        continue
                    if isinstance(result.get("parameterOverrides"), dict) and actions is not None:
                        actions.parameterOverrides = result["parameterOverrides"] or actions.parameterOverrides
                    if result.get("motion") and actions is not None:
                        actions.motion = result["motion"]
            except Exception as e:  # noqa: BLE001 - 导演失败必须可见，不打断说话
                logger.error(f"sentence.after plugin dispatch failed (visible fallback): {e}")

        # 小节化：把句子切成小块再逐块进 TTS 队列，降低单次 CPU 合成时长，
        # 让首块音频更早进播放队列（顺序播放由 tts_manager 保证）。
        tts_chunks = split_tts_chunks(tts_text)
        # 括注动作时间轴（my-neuro 式"播放位置触发"）：导演解析出的句子级帧
        # （clean_position 锚定清洗后文本）按 TTS 分块切片成 motionTimeline，
        # at = 块内归一化播放进度，随各 audio 帧透传给前端调度触发。
        _timeline = getattr(
            getattr(output, "_director_result", None), "motion_timeline", None
        ) or []
        per_chunk_cues = project_motion_cues_into_chunks(
            _timeline,
            tts_text,
            tts_chunks,
            len(filter_parentheses(display_text.text or "")),
        )

        # 显式用户动作命令（priority=100）覆盖本句括注时间轴：用户明确要求就按
        # 用户来，防止前端按括注时间轴忽略一次性命令动作（命令不执行）。
        if (
            actions is not None
            and getattr(actions, "priority", None) == 100
            and getattr(actions, "source", None) == "user_command"
        ):
            per_chunk_cues = [[] for _ in tts_chunks]
        else:
            # 同一句内时间轴：去重 + 最小间隔，避免冲突动作在同一块/相邻块打架
            # （同一 motion 只触发一次；靠得太近的不同动作保留前者）。
            per_chunk_cues = [normalize_chunk_timeline(cues) for cues in per_chunk_cues]
        has_timeline = any(per_chunk_cues)
        base_actions = actions
        if has_timeline and actions is not None:
            # 时间轴接管整句动作：清掉一次性 motion，避免新前端双重触发；
            # 投影失败时 has_timeline=False，完整保留旧行为（整句一次触发）。
            base_actions = replace(actions, motion=None)

        for i, tts_chunk in enumerate(tts_chunks):
            chunk_actions = base_actions
            # 一次性 motion 只挂句首首个 audio payload，避免同一动作每个 chunk 重复触发；
            # motionTimeline（括注播放位置触发）不受影响，仍按块携带。
            if (
                i > 0
                and chunk_actions is not None
                and getattr(chunk_actions, "motion", None) is not None
            ):
                chunk_actions = replace(chunk_actions, motion=None)
            if base_actions is not None and per_chunk_cues[i]:
                chunk_actions = replace(base_actions, motionTimeline=list(per_chunk_cues[i]))
            await tts_manager.speak(
                tts_text=tts_chunk,
                display_text=display_text,
                actions=chunk_actions,
                live2d_model=live2d_model,
                tts_engine=tts_engine,
                websocket_send=websocket_send,
                emotion=sentence_emotion,
                strength=sentence_strength,
                mood_tier=mood_tier,
                sentence_start=(i == 0),
            )
    return full_response


async def handle_audio_output(
    output: AudioOutput,
    websocket_send: WebSocketSend,
) -> str:
    """Process and send AudioOutput directly to the client"""
    full_response = ""
    async for audio_path, display_text, transcript, actions in output:
        full_response += transcript
        audio_payload = prepare_audio_payload(
            audio_path=audio_path,
            display_text=display_text,
            actions=actions.to_dict() if actions else None,
        )
        await websocket_send(json.dumps(audio_payload))
    return full_response


async def send_conversation_start_signals(websocket_send: WebSocketSend) -> None:
    """Send initial conversation signals"""
    await websocket_send(
        json.dumps(
            {
                "type": "control",
                "text": "conversation-chain-start",
            }
        )
    )
    await websocket_send(json.dumps({"type": "full-text", "text": "Thinking..."}))


async def process_user_input(
    user_input: Union[str, np.ndarray],
    asr_engine: ASRInterface,
    websocket_send: WebSocketSend,
) -> str:
    """Process user input, converting audio to text if needed"""
    if isinstance(user_input, np.ndarray):
        # 可观测性：此前语音链路完全静默——前端到底送没送音频、ASR 认出了
        # 什么，日志里毫无痕迹，「语音输入有问题」无从排查。这里记录时长与结果。
        duration = len(user_input) / 16000.0
        logger.info(f"Transcribing audio input ({duration:.1f}s)...")
        input_text = await asr_engine.async_transcribe_np(user_input)
        logger.info(f"ASR transcript: {input_text!r}")
        await websocket_send(
            json.dumps({"type": "user-input-transcription", "text": input_text})
        )
        return input_text
    return user_input


async def finalize_conversation_turn(
    tts_manager: TTSTaskManager,
    websocket_send: WebSocketSend,
    client_uid: str,
    broadcast_ctx: Optional[BroadcastContext] = None,
) -> None:
    """Finalize a conversation turn"""
    if tts_manager.task_list:
        await asyncio.gather(*tts_manager.task_list)
        await websocket_send(json.dumps({"type": "backend-synth-complete"}))

        # P7-04: 正常多段语音的播放确认等待时长必须覆盖真实播放，而非固定 5s 与
        # 语音竞速超时。以已排队音频的预估播放总时长为基准 + 缓冲，并设下限；
        # 打断仍由 task 取消（CancelledError → finally → interrupt_conversation）即时释放。
        _expected = tts_manager.expected_playback_seconds()
        _playback_timeout = max(8.0, _expected + 8.0)
        response = await message_handler.wait_for_response(
            client_uid, "frontend-playback-complete", timeout=_playback_timeout
        )

        if not response:
            # P4：播放确认超时降级为 info —— 非致命，仅缺失一次播放完成回执；
            # 兜底逻辑（force-new-message / conversation-end）保持不变。
            logger.info(f"No playback completion response from {client_uid} (non-fatal)")
            # 修复 B12：播放确认超时也必须补发 conversation-end 信号，
            # 否则 proxy 的 conversation_active 一直 gating，排队 text-input 不再转发。
            await websocket_send(json.dumps({"type": "force-new-message"}))
            if broadcast_ctx and broadcast_ctx.broadcast_func:
                await broadcast_ctx.broadcast_func(
                    broadcast_ctx.group_members,
                    {"type": "force-new-message"},
                    broadcast_ctx.current_client_uid,
                )
            await send_conversation_end_signal(websocket_send, broadcast_ctx)
            return

    await websocket_send(json.dumps({"type": "force-new-message"}))

    if broadcast_ctx and broadcast_ctx.broadcast_func:
        await broadcast_ctx.broadcast_func(
            broadcast_ctx.group_members,
            {"type": "force-new-message"},
            broadcast_ctx.current_client_uid,
        )

    await send_conversation_end_signal(websocket_send, broadcast_ctx)


async def send_conversation_end_signal(
    websocket_send: WebSocketSend,
    broadcast_ctx: Optional[BroadcastContext],
    session_emoji: str = "😊",
) -> None:
    """Send conversation chain end signal"""
    chain_end_msg = {
        "type": "control",
        "text": "conversation-chain-end",
    }

    await websocket_send(json.dumps(chain_end_msg))

    if broadcast_ctx and broadcast_ctx.broadcast_func and broadcast_ctx.group_members:
        await broadcast_ctx.broadcast_func(
            broadcast_ctx.group_members,
            chain_end_msg,
        )

    logger.info(f"😎👍✅ Conversation Chain {session_emoji} completed!")


async def cleanup_conversation(tts_manager: TTSTaskManager, session_emoji: str) -> None:
    """Clean up conversation resources for normal completion.

    Normal completion waits for all queued TTS tasks to finish and ensures
    ordered delivery completes (flush).  Do not use this for interruption.
    """
    await tts_manager.flush()
    logger.debug(f"🧹 Flushed conversation {session_emoji}.")


async def interrupt_conversation(tts_manager: TTSTaskManager, session_emoji: str) -> None:
    """Interrupt conversation: stop playback, cancel in-flight synthesis, drop queued payloads."""
    await tts_manager.interrupt()
    logger.debug(f"🛑 Interrupted conversation {session_emoji}.")


EMOJI_LIST = [
    "🐶",
    "🐱",
    "🐭",
    "🐹",
    "🐰",
    "🦊",
    "🐻",
    "🐼",
    "🐨",
    "🐯",
    "🦁",
    "🐮",
    "🐷",
    "🐸",
    "🐵",
    "🐔",
    "🐧",
    "🐦",
    "🐤",
    "🐣",
    "🐥",
    "🦆",
    "🦅",
    "🦉",
    "🦇",
    "🐺",
    "🐗",
    "🐴",
    "🦄",
    "🐝",
    "🌵",
    "🎄",
    "🌲",
    "🌳",
    "🌴",
    "🌱",
    "🌿",
    "☘️",
    "🍀",
    "🍂",
    "🍁",
    "🍄",
    "🌾",
    "💐",
    "🌹",
    "🌸",
    "🌛",
    "🌍",
    "⭐️",
    "🔥",
    "🌈",
    "🌩",
    "⛄️",
    "🎃",
    "🎄",
    "🎉",
    "🎏",
    "🎗",
    "🀄️",
    "🎭",
    "🎨",
    "🧵",
    "🪡",
    "🧶",
    "🥽",
    "🥼",
    "🦺",
    "👔",
    "👕",
    "👜",
    "👑",
]
