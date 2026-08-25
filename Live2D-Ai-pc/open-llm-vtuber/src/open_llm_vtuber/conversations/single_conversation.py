import asyncio
import json
import os
import time
from typing import Any

import numpy as np
from loguru import logger

# Import necessary types from agent outputs
from ..agent.output_types import AudioOutput, SentenceOutput
from ..chat_history_manager import store_message
from ..service_context import ServiceContext
from ..utils.stream_audio import prepare_emotion_payload
from .conversation_utils import (
    EMOJI_LIST,
    cleanup_conversation,
    create_batch_input,
    finalize_conversation_turn,
    interrupt_conversation,
    process_agent_output,
    process_user_input,
    send_conversation_start_signals,
)
from .tts_manager import TTSTaskManager
from .types import WebSocketSend


async def _send_emotion_frame(
    websocket_send: WebSocketSend,
    emo_data: dict,
) -> None:
    """构造并发送 emotion WS 帧（P7-02：检测到 [emo:xxx] 立即发送，先于 audio）。

    parameterOverrides：优先取调用方解析的 Live2D 参数（未来 resolve_emotion_parameters），
    否则空 dict（前端仍可按 emotion+strength 应用）。所有异常降级为日志，不崩溃（P7-05）。
    """
    frame = prepare_emotion_payload(
        emotion=emo_data.get("emotion", "neutral"),
        strength=emo_data.get("strength", 2),
        parameter_overrides=emo_data.get("parameterOverrides") or {},
        fade_ms=emo_data.get("fadeMs", 400),
    )
    await websocket_send(json.dumps(frame))


def _memory_scope(context) -> str:
    """推导长期记忆的 scope 键：优先会话键（不同聊天会话隔离），回退角色身份键。

    缺省空串=全局记忆；有 conf_uid/history_uid 时返回 "<conf_uid>::<history_uid>"，
    使不同 conversation/session 的长期记忆默认互不串味，同时提供字段供调用方做
    「全局 / 当前会话」清除语义。
    """
    conf = getattr(context, "character_config", None)
    cfg_uid = getattr(conf, "conf_uid", None) or ""
    hist_uid = getattr(context, "history_uid", "") or ""
    if hist_uid:
        return f"{cfg_uid}::{hist_uid}" if cfg_uid else hist_uid
    return cfg_uid or ""


def _spawn_memory_curation(context, user_text: str, ai_text: str) -> None:
    """派生后台任务执行 LLM 记忆管家（fire-and-forget；失败只记日志）。"""
    curator = getattr(context, "memory_curator", None)
    if curator is None or not (user_text or "").strip():
        return

    async def _run():
        try:
            plugin_manager = getattr(context, "plugin_manager", None)
            host = getattr(plugin_manager, "host", None) if plugin_manager else None
            inst = host.get_instance("memory") if host is not None else None
            if inst is None or not hasattr(inst, "list_facts"):
                return  # 记忆插件未加载 → 无处落盘，静默跳过
            from ..memory_curator import apply_curation

            scope = _memory_scope(context)
            # 异步往返前捕获写入代次；期间若用户清空/改写记忆（revision 变化）→ 放弃应用。
            rev = getattr(inst, "revision", None)
            expected_rev = rev() if callable(rev) else None
            actions = await curator.curate(user_text, ai_text, inst.list_facts(scope=scope))
            applied = apply_curation(inst, actions, expected_revision=expected_rev, scope=scope)
            if applied:
                logger.info(f"记忆管家本轮更新 {applied} 条长期事实")
        except asyncio.CancelledError:
            raise
        except Exception as exc:  # noqa: BLE001 - 管家任何失败不影响主链路
            logger.warning(f"记忆管家执行失败（忽略）: {exc}")

    import asyncio

    asyncio.create_task(_run())


async def _safe_mood(mood_engine, method: str, *args) -> None:
    """mood 事件调用兜底（P7-05：mood 模块异常不影响对话主链路）。"""
    if mood_engine is None:
        return
    try:
        await getattr(mood_engine, method)(*args)
    except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
        logger.warning(f"mood_engine.{method} failed (ignored): {e}")


# 主 LLM 原生工具动作的调度优先级（Preview 协议元数据）：
# user_command(100) > llm_tool(90) > stage_direction(60) > director(50)。
_LLM_TOOL_PRIORITY = 90


def _bind_pending_llm_action(output_item, pending: list[dict]) -> None:
    """把主 LLM 原生工具产出的待绑定动作挂到下一句 SentenceOutput 上。

    工具调用在文本流中先于其后的正文出现，因此「下一个切出的句子」即工具
    语义上的当前句/下一句；一次绑定消费一条，多余动作留给后续句子。
    """
    if not pending or not isinstance(output_item, SentenceOutput):
        return
    event = pending.pop(0)
    output_item._native_action = {
        "action": str(event.get("action") or ""),
        "strength": event.get("strength"),
        "reason": str(event.get("reason") or ""),
        "source": "llm_tool",
    }


def _apply_native_tool_action(
    context: ServiceContext, output_item, native_action: dict
) -> None:
    """主 LLM 显式工具动作 → Actions payload（优先级高于 ExpressionDirector）。

    动作 id 已在 live2d_action.execute 白名单层收敛过，这里仍过一遍目录白名单
    作为最终防线；emotion 保持 neutral、不跑导演 LLM（省一次每句调用，
    表情由既有 [emo] 拦截/mood 链路负责）。
    """
    from ..director import DirectorResult
    from ..motion_catalog import MOTION_METADATA, sanitize_motion

    actions = output_item.actions
    motion = sanitize_motion(native_action.get("action"))
    if actions is None or not motion:
        # 非法动作不产 payload（拒绝日志已在工具层打出）
        logger.warning(
            f"[llm_tool] 动作未通过白名单，忽略: {native_action.get('action')!r}"
        )
        return
    try:
        strength = max(1, min(3, int(native_action.get("strength") or 2)))
    except (TypeError, ValueError):
        strength = 2
    actions.motion = motion
    actions.motionStrength = strength
    actions.source = "llm_tool"
    actions.priority = _LLM_TOOL_PRIORITY
    try:
        meta = MOTION_METADATA.get(motion) or {}
        actions.suggested_duration_ms = int(meta.get("suggested_duration_ms", 1000))
    except Exception:  # noqa: BLE001 - 元数据缺失不影响动作下发
        pass
    # handle_sentence_output 从 _director_result 读 emotion/strength 下发表情帧；
    # 复用该通道保持单一下发路径（source 标记 llm_tool 便于开发者面板区分）。
    output_item._director_result = DirectorResult(
        emotion="neutral",
        strength=strength,
        motion_label=motion,
        source="llm_tool",
    )
    logger.info(
        "action payload: motion={} strength={} source=llm_tool priority={} reason={!r}",
        motion,
        strength,
        _LLM_TOOL_PRIORITY,
        native_action.get("reason") or None,
    )


async def _apply_director(
    context: ServiceContext, output_item, user_text: str = ""
) -> None:
    """PC v4：把情绪/表情/动作分流到独立导演模型，覆盖主对话脑 actions。

    动作优先级（任务定版）：
        1. 显式用户动作命令（用户输入里的 点头/摇头/歪头/张望，带疑问/否定
           语境保护）——最高，覆盖导演/括注；
        2. 主 LLM 原生工具动作（live2d.perform_action，source=llm_tool）——
           高于导演与括注；命中时跳过导演分析（导演仅作 fallback）；
           若同一句同时存在显式用户命令，用户命令仍最高；
        3. 导演 motion_label（括注时间轴首帧也算导演产物）；
        4. 情绪映射兜底（克制：仅 surprise→surprise，其余情绪无动作）。
    任何来源的 motion 都会过 motion_catalog 白名单收敛，不在白名单清空为 None。
    导演不可用/失败时保留主对话脑原有 actions，不阻断说话。
    """
    if not isinstance(output_item, SentenceOutput):
        return

    # 优先级 2：主 LLM 原生工具动作。除非同句还有显式用户命令（优先级 1），
    # 否则直接生效并跳过导演 —— ExpressionDirector 仅在无工具动作时兜底。
    native_action = getattr(output_item, "_native_action", None)
    if native_action:
        user_cmd_override = False
        try:
            from ..director import detect_user_motion_command

            user_cmd_override = bool(detect_user_motion_command(user_text))
        except Exception:  # noqa: BLE001 - 命令识别失败按无命令处理
            user_cmd_override = False
        if not user_cmd_override:
            _apply_native_tool_action(context, output_item, native_action)
            return

    director = getattr(context, "director", None)
    if director is None:
        return
    try:
        result = await director.analyze(output_item.display_text.text or output_item.tts_text or "")
    except Exception as e:  # noqa: BLE001 - 导演失败保留原动作
        logger.warning(f"ExpressionDirector analyze failed, keep original actions: {e}")
        return

    actions = output_item.actions
    if actions is None:
        return
    # 括注动作优先：主脑把舞台说明写进（）时，解析全部括注为带文本位置的
    # motion 帧（下游切片成 motionTimeline，播放到对应进度才触发）；
    # 首个命中同时作为整句兜底 motion（旧前端仍可见一次性行为）。
    try:
        from ..director import stage_gesture_timeline

        timeline = stage_gesture_timeline(
            output_item.display_text.text or output_item.tts_text or ""
        )
    except Exception:  # noqa: BLE001 - 括注解析失败不阻断导演结果
        timeline = []
    # 优先级 1：显式用户动作命令覆盖导演与括注——用户明确要求就按用户来。
    # 命中时清空时间轴（避免前端按时间轴忽略一次性 motion 导致命令不执行），
    # 强度至少 2 级保证可见。
    try:
        from ..director import detect_user_motion_command

        user_cmd = detect_user_motion_command(user_text)
    except Exception:  # noqa: BLE001 - 命令识别失败退导演结果
        user_cmd = None
    if user_cmd:
        try:
            result.strength = max(int(result.strength or 2), 2)
        except (TypeError, ValueError):
            result.strength = 2
        result.motion_label = user_cmd
        timeline = []
    # 任何来源的 motion 都必须过白名单：括注时间轴逐帧收敛为六个安全动作。
    from ..motion_catalog import sanitize_motion

    timeline = [c for c in timeline if sanitize_motion(c.get("motion"))]
    result.motion_timeline = list(timeline)
    if timeline:
        result.motion_label = timeline[0]["motion"]
    # 最终防线：导演/命令/情绪兜底产出的动作也必须过白名单（已删 id → None）。
    safe_motion = sanitize_motion(result.motion_label)
    result.motion_label = safe_motion
    if safe_motion:
        actions.motion = safe_motion
        actions.motionStrength = result.strength
        # Preview 协议可选元数据（旧客户端忽略）：来源 + 调度优先级 + 建议时长。
        # 显式用户命令 > 括注时间轴 > 导演/情绪推理。
        if user_cmd:
            actions.source = "user_command"
            actions.priority = 100
        elif timeline:
            actions.source = "stage_direction"
            actions.priority = 60
        else:
            actions.source = "director"
            actions.priority = 50
        try:
            from ..motion_catalog import MOTION_METADATA

            meta = MOTION_METADATA.get(actions.motion)
            if meta is not None:
                actions.suggested_duration_ms = int(meta.get("suggested_duration_ms", 1000))
        except Exception:  # noqa: BLE001 - 元数据缺失不影响动作下发
            pass
        logger.info(
            "action payload: motion={} strength={} emotion={} timeline_items={} command={} source={} priority={}",
            result.motion_label,
            result.strength,
            result.emotion,
            len(timeline),
            bool(user_cmd),
            actions.source,
            actions.priority,
        )
    if result.parameter_overrides:
        actions.parameterOverrides = result.parameter_overrides
    if result.half_body:
        actions.halfBody = result.half_body
    emo_map = getattr(context.live2d_model, "emo_map", {}) or {}
    if result.emotion in emo_map:
        actions.expressions = [emo_map[result.emotion]]
    # 把导演结果挂在 SentenceOutput 上，供 handle_sentence_output 读取后下发
    # 独立 emotion 帧（emotion+strength），否则前端只会收到恒 neutral / 空 overrides。
    output_item._director_result = result


_CONTINUE_DIALOGUE_PROMPT = (
    "（自然地继续刚才的话题，像日常闲聊接着聊下去；如果觉得已经说完，只回复 [DONE]）"
)


def _multi_turn_extra() -> int:
    """一次用户输入触发的额外对话段数（MELO_MULTI_TURN_EXTRA，默认 2，上限 5）。"""
    try:
        return max(0, min(5, int(os.environ.get("MELO_MULTI_TURN_EXTRA", "2"))))
    except (TypeError, ValueError):
        return 2


async def _consume_extra_agent_stream(
    context: ServiceContext,
    websocket_send: WebSocketSend,
    tts_manager: TTSTaskManager,
    mood_engine,
    agent_output_stream,
    pending_llm_actions: list[dict] | None = None,
) -> tuple[str, bool, bool]:
    """消费多段对话的后续 agent 流：每段走 process_agent_output（含字幕/TTS 队列）。

    返回 (累计文本, 是否遇到 [DONE] 终止标记, 本段是否失败)。
    播放/打断仍由 tts_manager 统一负责。
    pending_llm_actions：主 LLM 原生工具动作跨段共享队列（动作绑定下一段首句）。
    失败（stream_error / 异常）时置第三返回值为 True —— 调用方必须停止继续续聊，
    LLM API 错误绝不触发额外多段对话。
    """
    collected = ""
    done = False
    failed = False
    pending = pending_llm_actions if pending_llm_actions is not None else []
    try:
        async for output_item in agent_output_stream:
            if isinstance(output_item, (SentenceOutput, AudioOutput)):
                _bind_pending_llm_action(output_item, pending)
                await _apply_director(context, output_item)
                _mood_tier = "neutral"
                if mood_engine is not None:
                    try:
                        _mood_tier = mood_engine.get_mood_tier().value
                    except Exception:  # noqa: BLE001 - 降级 neutral 不阻断
                        pass
                part = await process_agent_output(
                    output=output_item,
                    character_config=context.character_config,
                    live2d_model=context.live2d_model,
                    tts_engine=context.tts_engine,
                    websocket_send=websocket_send,
                    tts_manager=tts_manager,
                    translate_engine=context.translate_engine,
                    mood_tier=_mood_tier,
                    plugin_manager=getattr(context, "plugin_manager", None),
                )
                part = str(part or "")
                if "[DONE]" in part:
                    part = part.replace("[DONE]", "").strip()
                    done = True
                collected += part
                if done:
                    break
            elif isinstance(output_item, dict) and output_item.get("type") == "stream_error":
                message = str(output_item.get("message") or "对话失败")
                failed = True
                await websocket_send(json.dumps({"type": "error", "message": message}))
    except asyncio.CancelledError:
        raise
    except Exception as exc:  # noqa: BLE001 - 额外段失败要可见，不静默
        logger.exception(f"Extra dialogue turn failed: {exc}")
        failed = True
        try:
            await websocket_send(json.dumps({"type": "error", "message": str(exc)}))
        except Exception:  # noqa: BLE001
            pass
    return collected, done, failed


def _plugin_context(context: ServiceContext, name: str, payload: dict) -> str:
    """把插件钩子返回的上下文片段合并成一段前缀文本。

    只接受插件返回的 context 字符串；插件错误由 PluginHost 记录并返回 error
    事件，不会从这里静默消失。
    """
    manager = getattr(context, "plugin_manager", None)
    if manager is None or getattr(manager, "host", None) is None:
        return ""
    events = manager.host.emit_event(name, payload)
    chunks: list[str] = []
    for event in events:
        if not event.get("ok"):
            logger.warning(
                f"plugin {event.get('plugin')} failed on {name}: {event.get('error')}"
            )
            continue
        result = event.get("result")
        if isinstance(result, dict) and isinstance(result.get("context"), str):
            text = result["context"].strip()
            if text:
                chunks.append(text)
    return "\n".join(chunks)


_VISION_PROMPT = (
    "You are a screenshot analysis assistant. Describe the current screen content "
    "briefly but thoroughly. Focus on visible applications, windows, code, "
    "documents, images, or any content the user might be referring to. "
    "Your description will be used as context for a conversation agent."
)


async def _get_vision_context(
    context: ServiceContext,
    input_text: str,
) -> str:
    """
    If vision is enabled, capture a screenshot and prepend its description
    to the user's input text for richer conversational context.

    Returns the original or enhanced input text.
    """
    if context.vision_handler is None:
        return input_text

    logger.debug("Vision pipeline: capturing screenshot for context...")
    try:
        description = await context.vision_handler.capture_and_describe(
            prompt=_VISION_PROMPT
        )
        if description:
            # Prepend vision context to the user's input
            enhanced = f"[Screen Context: {description}]\n\n{input_text}"
            logger.info(f"Vision context injected ({len(description)} chars).")
            return enhanced
        else:
            logger.debug("Vision returned no description, using raw input.")
            return input_text
    except Exception as e:  # noqa: BLE001
        logger.warning(f"Vision pipeline error (non-fatal): {e}")
        return input_text


async def process_single_conversation(
    context: ServiceContext,
    websocket_send: WebSocketSend,
    client_uid: str,
    user_input: str | np.ndarray,
    images: list[dict[str, Any]] | None = None,
    session_emoji: str = np.random.choice(EMOJI_LIST),
    metadata: dict[str, Any] | None = None,
) -> str:
    """Process a single-user conversation turn

    Args:
        context: Service context containing all configurations and engines
        websocket_send: WebSocket send function
        client_uid: Client unique identifier
        user_input: Text or audio input from user
        images: Optional list of image data
        session_emoji: Emoji identifier for the conversation
        metadata: Optional metadata for special processing flags

    Returns:
        str: Complete response text
    """
    # Create TTSTaskManager for this conversation
    # P2-3 (spoq-fix): 段级并发上限从 conf 读取（防御 None → 默认 2）
    _tts_conf = getattr(context, "character_config", None)
    _tts_conf = getattr(_tts_conf, "tts_config", None) if _tts_conf else None
    tts_manager = TTSTaskManager(
        concurrency_limit=getattr(_tts_conf, "concurrency_limit", 2) or 2,
        viseme_mode=getattr(_tts_conf, "viseme_mode", "rms") or "rms",
    )
    full_response = ""  # Initialize full_response here
    mood_engine = getattr(context, "mood_engine", None)
    _latency_start = time.monotonic()
    _interrupted = False
    # 本轮是否发生 LLM API/流失败：失败时不再触发额外多段续聊，
    # 也不把失败轮计入历史/记忆沉淀（错误绝不进入下游链路）。
    _turn_failed = False

    try:
        # Send initial signals
        await send_conversation_start_signals(websocket_send)
        logger.info(f"New Conversation Chain {session_emoji} started!")

        # P7-03：对话开始 → mood_engine.start()（衰减任务启动；异常降级不阻塞）
        await _safe_mood(mood_engine, "start")

        # Process user input
        input_text = await process_user_input(
            user_input, context.asr_engine, websocket_send
        )
        # 空转写守卫：麦克风没收到帧 / 纯噪声时不再把空输入灌给 LLM
        # （此前 AI 会对着空气回答「诶？」）。前端据空文本提示重说。
        if isinstance(user_input, np.ndarray) and not input_text.strip():
            logger.warning("ASR transcript empty (no/unclear audio) — skip conversation")
            await websocket_send(json.dumps({"type": "control", "text": "conversation-chain-end"}))
            return ""
        logger.info(
            f"[LATENCY] user_input_ready_ms={(time.monotonic() - _latency_start) * 1000:.0f}"
        )

        # P7-03：用户消息到达 → mood_engine.on_user_message_sentiment()。
        # 文本输入已在 conversation_handler 触发过（防双计）；此处仅补 mic（np 输入）路径。
        if not isinstance(user_input, str):
            await _safe_mood(mood_engine, "on_user_message_sentiment", input_text)

        # 原始用户文本快照（注入记忆/知识前缀与视觉上下文之前），供记忆管家使用
        raw_user_text = input_text

        # Capture screenshot and inject vision context (if enabled)
        input_text = await _get_vision_context(context, input_text)

        # PC v4 插件钩子：llm.before 允许记忆/知识/上下文压缩插件注入前缀。
        plugin_ctx = _plugin_context(
            context,
            "llm.before",
            {
                "text": input_text,
                "history": list(getattr(context.agent_engine, "_memory", []) or []),
                "scope": _memory_scope(context),
            },
        )
        if plugin_ctx:
            input_text = f"{plugin_ctx}\n\n{input_text}"
            logger.info(f"plugin llm.before injected {len(plugin_ctx)} chars")

        # Create batch input
        batch_input = create_batch_input(
            input_text=input_text,
            images=images,
            from_name=context.character_config.human_name,
            metadata=metadata,
        )

        # Store user message (check if we should skip storing to history)
        skip_history = metadata and metadata.get("skip_history", False)
        if context.history_uid and not skip_history:
            store_message(
                conf_uid=context.character_config.conf_uid,
                history_uid=context.history_uid,
                role="human",
                content=input_text,
                name=context.character_config.human_name,
            )

        if skip_history:
            logger.debug("Skipping storing user input to history (proactive speak)")

        logger.info(f"User input: {input_text}")
        if images:
            logger.info(f"With {len(images)} images")

        # 主 LLM 原生工具动作队列：live2d_action 事件入队，绑定到下一句
        # （在 agent 流 try 之外初始化，异常路径也不会 NameError）
        pending_llm_actions: list[dict] = []

        try:
            # agent.chat yields Union[SentenceOutput, Dict[str, Any]]
            agent_output_stream = context.agent_engine.chat(batch_input)
            _first_llm_item_logged = False

            async for output_item in agent_output_stream:
                if not _first_llm_item_logged:
                    logger.info(
                        f"[LATENCY] first_llm_item_ms={(time.monotonic() - _latency_start) * 1000:.0f}"
                    )
                    _first_llm_item_logged = True
                if (
                    isinstance(output_item, dict)
                    and output_item.get("type") == "live2d_action"
                ):
                    # 主 LLM 原生工具（live2d.perform_action）动作：先入队，
                    # 绑定到对应下一句/当前句的 Actions payload 与 TTS 音频。
                    # 事件本身不朗读、不入记忆。
                    pending_llm_actions.append(dict(output_item))
                    logger.info(
                        "[llm_tool] 动作待绑定: action={} strength={} queue={}",
                        output_item.get("action"),
                        output_item.get("strength"),
                        len(pending_llm_actions),
                    )

                elif (
                    isinstance(output_item, dict)
                    and output_item.get("type") == "tool_call_status"
                ):
                    # Handle tool status event: send WebSocket message
                    output_item["name"] = context.character_config.character_name
                    logger.debug(f"Sending tool status update: {output_item}")

                    await websocket_send(json.dumps(output_item))

                elif (
                    isinstance(output_item, dict)
                    and output_item.get("type") == "emo_token"
                ):
                    # P7-02：StreamInterceptor 命中 [emo:xxx] → 立即发 emotion 帧（先于 audio）
                    # + mood_engine.on_emotion_event()（流式心情事件）。异常降级为日志（P7-05）。
                    try:
                        await _send_emotion_frame(websocket_send, output_item)
                        await _safe_mood(
                            mood_engine,
                            "on_emotion_event",
                            output_item.get("emotion", "neutral"),
                            output_item.get("strength", 2),
                        )
                    except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
                        logger.warning(f"emo_token handling failed (ignored): {e}")

                elif (
                    isinstance(output_item, dict)
                    and output_item.get("type") == "reasoning_delta"
                ):
                    # 思维链增量：独立消息类型下发，前端按「展示思考」开关决定渲染。
                    # 不进字幕/记忆/TTS 链路。
                    rdelta = str(output_item.get("text") or "")
                    if rdelta:
                        await websocket_send(
                            json.dumps({"type": "reasoning-delta", "text": rdelta})
                        )

                elif (
                    isinstance(output_item, dict)
                    and output_item.get("type") == "text_delta"
                ):
                    # P1-2：token 级增量 → 前端逐字上屏（`text-delta`）。
                    # 句子级 subtitle/audio.display_text 仍照旧发送，前端在流式期间
                    # 用它们作为"权威最终文本"收尾，不会重复上屏。
                    delta = str(output_item.get("text") or "")
                    if delta:
                        await websocket_send(
                            json.dumps({"type": "text-delta", "text": delta})
                        )

                elif (
                    isinstance(output_item, dict)
                    and output_item.get("type") == "stream_error"
                ):
                    # 上游流硬失败（未配置 API Key / 401 / 400 / 网络不可达）：
                    # transformers 的 pump/consume 只降级记日志、不崩溃，这里负责让用户**看得见**
                    # （前端 ws-bridge 把 error 渲染成聊天错误气泡 + Toast）。
                    message = str(output_item.get("message") or "对话失败")
                    _turn_failed = True
                    logger.error(f"LLM 流失败，已通知前端: {message}")
                    await websocket_send(
                        json.dumps({"type": "error", "message": message})
                    )

                elif isinstance(output_item, (SentenceOutput, AudioOutput)):
                    # Handle SentenceOutput or AudioOutput
                    # 主 LLM 原生工具动作绑定到本句（若有待绑定动作）
                    _bind_pending_llm_action(output_item, pending_llm_actions)
                    # PC v4：把表情/动作分析分流到导演模型，覆盖主脑 actions。
                    # 传入原始用户文本，使显式动作命令（点头/摇头等）优先级最高。
                    await _apply_director(context, output_item, user_text=raw_user_text)

                    # PC-L4: 当前 mood 分层传入 TTS（P7 mood_engine 产出；
                    # 每句现取以反映流式 emotion 事件对 mood 的实时影响；异常降级 neutral 不阻断）
                    _mood_tier = "neutral"
                    if mood_engine is not None:
                        try:
                            _mood_tier = mood_engine.get_mood_tier().value
                        except Exception as e:  # noqa: BLE001 - P7-05 降级兜底
                            logger.warning(
                                f"mood_engine.get_mood_tier failed (ignored): {e}"
                            )
                    response_part = await process_agent_output(
                        output=output_item,
                        character_config=context.character_config,
                        live2d_model=context.live2d_model,
                        tts_engine=context.tts_engine,
                        websocket_send=websocket_send,  # Pass websocket_send for audio/tts messages
                        tts_manager=tts_manager,
                        translate_engine=context.translate_engine,
                        mood_tier=_mood_tier,
                        plugin_manager=getattr(context, "plugin_manager", None),
                    )
                    # Ensure response_part is treated as a string before concatenation
                    response_part_str = (
                        str(response_part) if response_part is not None else ""
                    )
                    full_response += response_part_str  # Accumulate text response
                else:
                    logger.warning(
                        f"Received unexpected item type from agent chat stream: {type(output_item)}"
                    )
                    logger.debug(f"Unexpected item content: {output_item}")

        except Exception as e:  # noqa: BLE001
            logger.exception(
                f"Error processing agent response stream: {e}"
            )  # Log with stack trace
            _turn_failed = True
            await websocket_send(
                json.dumps(
                    {
                        "type": "error",
                        "message": f"Error processing agent response: {e!s}",
                    }
                )
            )
            # full_response will contain partial response before error
        # --- End processing agent response ---

        # 多段对话：一次用户输入额外续聊 N 段（MELO_MULTI_TURN_EXTRA，默认 2）。
        # 用户新消息/打断会取消整个 conversation task，天然支持打断。
        # LLM API 错误/工具协议错误发生时绝不触发额外续聊（单一可读错误已下发）。
        for _ in range(_multi_turn_extra()):
            if _turn_failed or "[DONE]" in full_response:
                break
            if not hasattr(context.agent_engine, "chat"):
                break
            cont_batch = create_batch_input(
                input_text=_CONTINUE_DIALOGUE_PROMPT,
                images=None,
                from_name=context.character_config.human_name,
                metadata=metadata,
            )
            cont_stream = context.agent_engine.chat(cont_batch)
            part, done, seg_failed = await _consume_extra_agent_stream(
                context,
                websocket_send,
                tts_manager,
                mood_engine,
                cont_stream,
                pending_llm_actions=pending_llm_actions,
            )
            full_response += part
            if seg_failed:
                _turn_failed = True
                break
            if done:
                break

        # Wait for any pending TTS tasks
        if tts_manager.task_list:
            await asyncio.gather(*tts_manager.task_list)
            await websocket_send(json.dumps({"type": "backend-synth-complete"}))

        await finalize_conversation_turn(
            tts_manager=tts_manager,
            websocket_send=websocket_send,
            client_uid=client_uid,
        )

        if context.history_uid and full_response:  # Check full_response before storing
            store_message(
                conf_uid=context.character_config.conf_uid,
                history_uid=context.history_uid,
                role="ai",
                content=full_response,
                name=context.character_config.character_name,
                avatar=context.character_config.avatar,
            )
            logger.info(f"AI response: {full_response}")

        # PC v4 插件钩子：llm.after 允许记忆插件沉淀长期事实（正则即时层）。
        # text=回复、user_text=原始用户输入：正则记忆只应从【用户的话】里
        # 提取显式事实——此前只传回复，AI 说「我在等你呢」也会被存成记忆，
        # 已删对话的内容就这样以「长期记忆」的形式阴魂不散。
        if full_response:
            _plugin_context(
                context,
                "llm.after",
                {
                    "text": full_response,
                    "user_text": raw_user_text or "",
                    "scope": _memory_scope(context),
                },
            )
            # LLM 记忆管家（异步层）：对照现有事实做增/删/合并，不阻塞返回。
            _spawn_memory_curation(context, raw_user_text, full_response)

        return full_response  # Return accumulated full_response

    except asyncio.CancelledError:
        _interrupted = True
        logger.info(f"🤡👍 Conversation {session_emoji} cancelled because interrupted.")
        raise
    except Exception as e:
        logger.error(f"Error in conversation chain: {e}")
        await websocket_send(
            json.dumps({"type": "error", "message": f"Conversation error: {e!s}"})
        )
        raise
    finally:
        # P7-03：对话结束 → mood_engine.stop()（幂等；异常降级不阻塞清理）
        await _safe_mood(mood_engine, "stop")
        # 取消/打断必须停止播放并丢弃在途合成与待播队列；正常结束才 flush()。
        if tts_manager is not None:
            if _interrupted:
                await interrupt_conversation(tts_manager, session_emoji)
            else:
                await cleanup_conversation(tts_manager, session_emoji)
