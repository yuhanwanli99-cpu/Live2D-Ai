import asyncio
import json
import os
import re
import time
import uuid
import wave
from datetime import datetime
from typing import List, Optional, Dict
from ..logsetup import get_logger

from ..agent.output_types import DisplayText, Actions
from ..live2d_model import Live2dModel
from ..tts.tts_interface import TTSInterface
from ..tts.tts_instruction_library import get_instruction
from ..utils.stream_audio import prepare_audio_payload
from .types import WebSocketSend

# 本模块全部记录带 "tts" 标签（控制台按标签分组，文件按标签检索）
logger = get_logger("tts")


class TTSTaskManager:
    """Manages TTS tasks and ensures ordered delivery to frontend while allowing parallel TTS generation"""

    def __init__(
        self,
        tts_engine: Optional[TTSInterface] = None,
        live2d_model: Optional[Live2dModel] = None,
        concurrency_limit: int = 2,  # PT-01: 构造参数名按契约改名（默认值语义不变）
        viseme_mode: str = "rms",    # rms/off/pymouth；默认 rms（NEKO 同款快速口型）
    ) -> None:
        self.task_list: List[asyncio.Task] = []
        self._lock = asyncio.Lock()
        # Queue to store ordered payloads
        self._payload_queue: asyncio.Queue[Dict] = asyncio.Queue()
        # Task to handle sending payloads in order
        self._sender_task: Optional[asyncio.Task] = None
        # Counter for maintaining order
        self._sequence_counter = 0
        self._next_sequence_to_send = 0
        # 是否已由首个序列号确定 sender 起始发送位置（兼容显式序列号从 1 起始的调用方）
        self._seq_started = False
        # 已入队 payload 的最大序列号（flush 排空等待用：next > max 即全部发送完毕）
        self._max_sequence_seen = 0
        # P5-04: 段级并发上限（同时进行中的 TTS 合成 ≤ concurrency_limit）
        self._semaphore = asyncio.Semaphore(concurrency_limit)
        # TTS 引擎 / Live2D 模型（构造注入，或经 speak() 兼容路径注入）
        self._tts_engine: Optional[TTSInterface] = tts_engine
        self._live2d_model: Optional[Live2dModel] = live2d_model
        self._viseme_mode: str = viseme_mode
        # 上一句 emotion（emotion 帧仅当与上一句不同时发送，前端按 emotion 去重）。
        # 初始为 "neutral"：前端默认表情即 neutral，首句 neutral 不发冗余 emotion 帧
        # （契约 P5-03：mock WS 发送数 = speak_async 调用数，3 段同 emotion 无额外帧）。
        self._last_emotion: str = "neutral"
        # 本轮已排队音频的预估播放总时长（秒）。用于让后端等待
        # frontend-playback-complete 的超时覆盖真实播放，而不是固定 5s 与语音竞速。
        self._expected_playback_seconds: float = 0.0
        # 无法解析 WAV 时长时的单 chunk 兜底估值（秒）。
        self._chunk_duration_fallback: float = 6.0

    @staticmethod
    def _wav_duration_seconds(path: Optional[str]) -> Optional[float]:
        """尽力读取 WAV 文件时长（秒）；非标准 WAV / 解析失败返回 None。"""
        if not path or not os.path.isfile(path):
            return None
        try:
            with wave.open(path, "rb") as wf:
                frames = wf.getnframes()
                rate = wf.getframerate()
                if rate <= 0:
                    return None
                return float(frames) / float(rate)
        except Exception:  # noqa: BLE001 - 时长只是超时估算，解析失败不阻断
            return None

    def expected_playback_seconds(self) -> float:
        """本轮已排队音频的预估播放总时长（秒），供最终播放确认超时使用。"""
        return self._expected_playback_seconds

    def reset_playback_estimate(self) -> None:
        """清空播放时长累计（打断后开始新的批次时调用）。"""
        self._expected_playback_seconds = 0.0

    def _accumulate_playback_estimate(self, audio_file_path: Optional[str]) -> None:
        duration = self._wav_duration_seconds(audio_file_path)
        self._expected_playback_seconds += (
            duration if duration is not None else self._chunk_duration_fallback
        )

    async def speak_async(
        self,
        tts_text: str,
        display_text: DisplayText,
        actions: Optional[Actions],
        emotion: str = "neutral",           # 新增：当前 emotion（用于 instruction 拼接）
        strength: int = 2,                  # 新增：当前 emotion 强度（1-3，用于 emotion 帧）
        mood_tier: str = "neutral",         # 新增：当前心情分层
        sequence_number: Optional[int] = None,
        websocket_send: Optional[WebSocketSend] = None,
        sentence_start: bool = False,       # 句首块标记：前端在它开播时才上屏该句字幕（字幕-声音同步）
    ) -> None:
        """
        新增方法：异步启动 TTS 合成（不阻塞后续句子）。
        - emotion 帧与 audio 帧一起入队，由发送管线按 sequence 顺序发出（T2 起不再同步发送）
        - 后台 asyncio.create_task 执行 TTS 合成
        - 合成完成后入队发送 audio 帧（顺序保持）

        Args:
            tts_text: 要合成的文本
            display_text: 前端展示文本
            actions: Live2D 动作（parameterOverrides 用于 emotion 帧）
            emotion: 当前句子 emotion（8 key 枚举）
            strength: 当前 emotion 强度（1-3）
            mood_tier: 当前心情分层（furious/upset/neutral/happy/ecstatic）
            sequence_number: 显式序列号（None = 由内部自增分配）
            websocket_send: WS 发送函数
        """
        if websocket_send is None:
            logger.warning("speak_async: websocket_send 未提供，跳过队列发送")
            return

        # 兼容 dict 形式的 display_text / actions（测试/调用方可能不经对象构造）：
        # 仅取 dataclass 已知字段，未知 key 忽略（对既有对象形态零影响）
        if isinstance(display_text, dict):
            display_text = DisplayText(
                **{k: v for k, v in display_text.items()
                   if k in set(DisplayText.__dataclass_fields__)}
            )
        if isinstance(actions, dict):
            actions = Actions(
                **{k: v for k, v in actions.items()
                   if k in set(Actions.__dataclass_fields__)}
            )

        # 分配序列号（未显式提供时自增；显式提供时由调用方管理）
        if sequence_number is None:
            sequence_number = self._sequence_counter
            self._sequence_counter += 1

        # 首个已分配序列号确定 sender 的起始发送位置：显式序列号调用方（如测试
        # 传 1,2,3）与自动自增（0,1,2）都能按序发送，不会因起始序列空洞卡死缓冲。
        if not self._seq_started:
            self._next_sequence_to_send = sequence_number
            self._seq_started = True

        # 空文本：直接入队 silent payload（不发引擎、不占并发槽）
        if len(re.sub(r'[\s.,!?，。！？\'"』」）】\s]+', "", tts_text)) == 0:
            logger.debug("Empty TTS text, sending silent display payload")
            self._ensure_sender(websocket_send)
            await self._send_silent_payload(
                display_text, actions, sequence_number, sentence_start=sentence_start
            )
            return

        # emotion 帧去重决策（T2 起不再同步发送：改为与 audio 帧一起入队，由发送管线
        # 按 sequence 顺序发出。这样 interrupt() 能抑制旧句的 emotion/audio 帧）。
        emotion_payload = None
        if emotion != self._last_emotion:
            emotion_payload = {
                "type": "emotion",
                "emotion": emotion,
                "strength": max(1, min(3, int(strength or 2))),
                "parameterOverrides": (actions.parameterOverrides if actions else None) or {},
                "fadeMs": 400,
            }
            self._last_emotion = emotion

        logger.debug(
            f"🏃Queuing TTS task for: '''{tts_text}''' (by {display_text.name})"
        )

        self._ensure_sender(websocket_send)

        # 创建后台合成 task（信号量限流并发，create_task 立即返回不阻塞）
        task = asyncio.create_task(
            self._process_tts_async(
                tts_text=tts_text,
                display_text=display_text,
                actions=actions,
                emotion=emotion,
                mood_tier=mood_tier,
                sequence_number=sequence_number,
                emotion_payload=emotion_payload,
                sentence_start=sentence_start,
            )
        )
        self.task_list.append(task)

    async def speak(
        self,
        tts_text: str,
        display_text: DisplayText,
        actions: Optional[Actions],
        live2d_model: Live2dModel,
        tts_engine: TTSInterface,
        websocket_send: WebSocketSend,
        emotion: str = "neutral",
        strength: int = 2,
        mood_tier: str = "neutral",
        sentence_start: bool = False,
    ) -> None:
        """
        兼容旧调用：委托 speak_async。

        首次调用时将 tts_engine / live2d_model 注入实例，供后续 speak_async 使用。
        PC-L4: emotion/mood_tier 可选透传（默认 neutral 保持旧行为，mood_tier
        不再恒 neutral——由对话/表情链路上游传入真实分层）。
        """
        if self._tts_engine is None and tts_engine is not None:
            self._tts_engine = tts_engine
        if self._live2d_model is None and live2d_model is not None:
            self._live2d_model = live2d_model
        await self.speak_async(
            tts_text=tts_text,
            display_text=display_text,
            actions=actions,
            emotion=emotion,
            strength=strength,
            mood_tier=mood_tier,
            sequence_number=None,
            websocket_send=websocket_send,
            sentence_start=sentence_start,
        )

    async def flush(self, timeout: float = 30.0) -> None:
        """等待所有排队的 TTS task 完成，并确保全部 audio 帧已按序发送。

        Args:
            timeout: 等待全部 TTS task 完成的总超时（秒）。超时后取消剩余任务并
                     清空 task_list、不抛异常（幂等，可安全重复调用）。
        """
        timed_out = False
        if self.task_list:
            try:
                await asyncio.wait_for(
                    asyncio.gather(*self.task_list, return_exceptions=True),
                    timeout=timeout,
                )
            except asyncio.TimeoutError:
                # PT-03 超时分支：取消剩余任务 + 清空 task_list、不抛异常、幂等
                timed_out = True
                for task in self.task_list:
                    if not task.done():
                        task.cancel()
                try:
                    await asyncio.gather(*self.task_list, return_exceptions=True)
                except Exception:  # noqa: BLE001
                    pass
                self.task_list.clear()

        self.task_list.clear()

        # 等待 sender 把队列中所有已分配序列发送完（next 越过最大已入队序列号 = 全部发完）。
        # 超时取消后可能产生序列空洞，跳过排空等待，直接停 sender。
        deadline = time.monotonic() + 30.0
        while (
            not timed_out
            and self._sender_task is not None
            and not self._sender_task.done()
            and self._next_sequence_to_send <= self._max_sequence_seen
            and time.monotonic() < deadline
        ):
            await asyncio.sleep(0.01)

        # 发完后停掉 sender（避免残留 while True 后台任务）
        if self._sender_task and not self._sender_task.done():
            self._sender_task.cancel()
            try:
                await self._sender_task
            except (asyncio.CancelledError, Exception):  # noqa: BLE001
                pass
            self._sender_task = None

        # 新一批次重新对齐序列状态（兼容显式序列号批次：序列号重新从 1 起始）
        self._seq_started = False
        self._max_sequence_seen = 0

    async def interrupt(self) -> None:
        """Barge-in 打断：停止当前播放、取消在途合成、清空待播队列、丢弃旧句。

        T2（spoq-fix）语义，对齐 barge-in 标准五步的 2/3 步：
            1. 停止 sender：正在发送的 audio 帧即刻中断，已缓冲未发送的帧丢弃；
            2. 取消所有在途 TTS 合成任务并等待其彻底结束（防取消后仍入队 payload）；
            3. 丢弃已入队未发送的 payload（重建队列，等价于排空）；
            4. 重置序列状态（下一句从干净状态开始，不影响后续 speak_async）。

        幂等：无任务 / 已打断时调用安全（各步均为 no-op）。
        叠加层：不破坏既有并发模型（信号量限流、按 sequence 顺序发送、flush 幂等清理）。
        """
        # 1) 停止顺序发送任务（正在发送的帧被中断，sender 局部缓冲的帧随之丢弃）
        if self._sender_task and not self._sender_task.done():
            self._sender_task.cancel()
            try:
                await self._sender_task
            except (asyncio.CancelledError, Exception):  # noqa: BLE001
                pass
            self._sender_task = None

        # 2) 取消在途 TTS 合成任务（快照后清空列表：并发新句不受影响）
        pending = list(self.task_list)
        self.task_list.clear()
        for task in pending:
            if not task.done():
                task.cancel()
        if pending:
            await asyncio.gather(*pending, return_exceptions=True)

        # 3) 丢弃已入队未发送的 payload（重建队列）
        self._payload_queue = asyncio.Queue()

        # 4) 重置序列状态（与 flush() 后对齐；_last_emotion 复位，下一句重新宣告情绪）
        self._sequence_counter = 0
        self._next_sequence_to_send = 0
        self._seq_started = False
        self._max_sequence_seen = 0
        self._last_emotion = "neutral"
        # 5) 打断后累计的播放时长估值作废，下一批次从零开始
        self.reset_playback_estimate()

    # ------------------------------------------------------------------ #
    # 内部
    # ------------------------------------------------------------------ #

    def _ensure_sender(self, websocket_send: WebSocketSend) -> None:
        """确保顺序发送任务在运行（队列消费 + 按序列顺序发帧）。"""
        if not self._sender_task or self._sender_task.done():
            self._sender_task = asyncio.create_task(
                self._process_payload_queue(websocket_send)
            )

    async def _process_tts_async(
        self,
        tts_text: str,
        display_text: DisplayText,
        actions: Optional[Actions],
        emotion: str,
        mood_tier: str,
        sequence_number: int,
        emotion_payload: Optional[Dict] = None,
        sentence_start: bool = False,
    ) -> None:
        """带并发限流的 TTS 合成包装（信号量内执行，保证同时合成数 ≤ 上限）。"""
        async with self._semaphore:
            _start = time.monotonic()
            await self._process_tts(
                tts_text=tts_text,
                display_text=display_text,
                actions=actions,
                emotion=emotion,
                mood_tier=mood_tier,
                sequence_number=sequence_number,
                emotion_payload=emotion_payload,
                sentence_start=sentence_start,
            )
            logger.info(
                f"[TTS] seq={sequence_number} text={tts_text[:20]!r} "
                f"synth_ms={(time.monotonic() - _start) * 1000:.0f}"
            )

    async def _process_tts(
        self,
        tts_text: str,
        display_text: DisplayText,
        actions: Optional[Actions],
        emotion: str,
        mood_tier: str,
        sequence_number: int,
        emotion_payload: Optional[Dict] = None,
        sentence_start: bool = False,
    ) -> None:
        """Process TTS generation and queue the result for ordered delivery

        PT-04: `_generate_audio` 异常时重试 1 次（总尝试 ≤ 2；重试在信号量内进行，
        不破并发上限）；仍失败才入 silent payload（不抛异常，保持队列顺序）。
        重试仅针对音频生成失败；prepare_audio_payload/入队失败不触发重试
        （非生成失败，保持改动前语义直接入 silent）。
        """
        audio_file_path = None
        # P4: 按当前 emotion + mood 动态拼接 instruction 传给 TTS 引擎
        instruction = get_instruction(emotion=emotion, mood_tier=mood_tier)
        try:
            audio_file_path = await self._generate_audio_with_retry(
                self._tts_engine, tts_text, instruction=instruction
            )
            payload = await asyncio.to_thread(
                prepare_audio_payload,
                audio_file_path,
                20,
                display_text,
                actions,
                False,
                None,
                self._viseme_mode,
            )
            # 字幕-声音同步：句首块打标记（省略=False，保持旧 payload 形状）
            if sentence_start:
                payload["sentence_start"] = True
            # 累积本块真实播放时长（WAV 时长；解析失败用兜底估值），供最终确认超时覆盖播放。
            self._accumulate_playback_estimate(audio_file_path)
            # T2: emotion 帧先入队（同一 sequence），再入队 audio payload，保持帧序
            if emotion_payload is not None:
                await self._enqueue_payload(emotion_payload, sequence_number)
            # Queue the payload with its sequence number
            await self._enqueue_payload(payload, sequence_number)

        except Exception as e:  # noqa: BLE001
            logger.error(f"Error preparing audio payload: {e}")
            # Queue silent payload for error case
            payload = prepare_audio_payload(
                audio_path=None,
                display_text=display_text,
                actions=actions,
            )
            if sentence_start:
                payload["sentence_start"] = True
            if emotion_payload is not None:
                await self._enqueue_payload(emotion_payload, sequence_number)
            await self._enqueue_payload(payload, sequence_number)

        finally:
            if audio_file_path and self._tts_engine is not None:
                try:
                    self._tts_engine.remove_file(audio_file_path)
                except Exception:  # noqa: BLE001
                    pass
                logger.debug("Audio cache file cleaned.")

    async def _generate_audio_with_retry(
        self,
        tts_engine: Optional[TTSInterface],
        text: str,
        instruction: str = "",
    ) -> str:
        """`_generate_audio` 失败重试 1 次（PT-04）。

        在信号量内调用（调用方 `_process_tts_async` 持有信号量），不破并发上限。
        两次尝试均失败时异常向上抛，由调用方入 silent payload。
        """
        try:
            return await self._generate_audio(tts_engine, text, instruction=instruction)
        except Exception as e:  # noqa: BLE001
            logger.error(
                f"TTS generation failed (attempt 1/2), retrying once: {e}"
            )
            return await self._generate_audio(tts_engine, text, instruction=instruction)

    async def _process_payload_queue(self, websocket_send: WebSocketSend) -> None:
        """
        Process and send payloads in correct order.
        Runs continuously until all payloads are processed.
        """
        buffered_payloads: Dict[int, List[Dict]] = {}

        while True:
            try:
                # Get payload from queue
                payload, sequence_number = await self._payload_queue.get()
                # T2: 同一 sequence 可含多条 payload（emotion 帧 + audio 帧），按入队序保持
                buffered_payloads.setdefault(sequence_number, []).append(payload)

                # Send payloads in order
                while self._next_sequence_to_send in buffered_payloads:
                    for next_payload in buffered_payloads.pop(
                        self._next_sequence_to_send
                    ):
                        await websocket_send(json.dumps(next_payload))
                    self._next_sequence_to_send += 1

                self._payload_queue.task_done()

            except asyncio.CancelledError:
                break

    async def _send_silent_payload(
        self,
        display_text: DisplayText,
        actions: Optional[Actions],
        sequence_number: int,
        sentence_start: bool = False,
    ) -> None:
        """Queue a silent audio payload"""
        audio_payload = prepare_audio_payload(
            audio_path=None,
            display_text=display_text,
            actions=actions,
        )
        # 静默帧没有可"开播"的音频：句首标记让前端立即上屏该句（不等待声音）
        if sentence_start:
            audio_payload["sentence_start"] = True
        await self._enqueue_payload(audio_payload, sequence_number)

    async def _enqueue_payload(self, payload: Dict, sequence_number: int) -> None:
        """入队 payload 并同步最大已入队序列号（供 flush 排空等待判断）。"""
        if sequence_number > self._max_sequence_seen:
            self._max_sequence_seen = sequence_number
        await self._payload_queue.put((payload, sequence_number))

    async def _generate_audio(
        self,
        tts_engine: Optional[TTSInterface],
        text: str,
        instruction: str = "",
    ) -> str:
        """Generate audio file from text"""
        if tts_engine is None:
            logger.warning("TTS engine not configured, sending silent payload")
            return ""
        logger.debug(f"🏃Generating audio for '''{text}'''...")
        file_name = (
            f"{datetime.now().strftime('%Y%m%d_%H%M%S')}_{str(uuid.uuid4())[:8]}"
        )
        try:
            audio_path = await tts_engine.async_generate_audio(
                text=text,
                file_name_no_ext=file_name,
                instruction=instruction,
            )
        except TypeError:
            # 兼容未实现 instruction 参数的 TTS 引擎（降级为不带 instruction 合成）
            logger.warning("TTS engine 不支持 instruction 参数，降级合成")
            audio_path = await tts_engine.async_generate_audio(
                text=text,
                file_name_no_ext=file_name,
            )
        if not audio_path:
            raise ValueError("TTS 引擎未生成音频文件（返回空路径）")
        return audio_path

    def clear(self) -> None:
        """Clear all pending tasks and reset state"""
        self.task_list.clear()
        if self._sender_task:
            self._sender_task.cancel()
        self._sequence_counter = 0
        self._next_sequence_to_send = 0
        self._seq_started = False
        self._max_sequence_seen = 0
        self._last_emotion = "neutral"
        # Create a new queue to clear any pending items
        self._payload_queue = asyncio.Queue()
