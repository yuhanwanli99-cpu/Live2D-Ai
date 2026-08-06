"""阿里云 DashScope CosyVoice 云端 TTS（官方 dashscope SDK，WebSocket run-task 协议）。

V1.2.1 修正（task-v12-fix-cosyvoice-pc）:
    协议从 start-synthesis + /api-ws/v1/realtime 修正为官方 run-task 协议 +
    /api-ws/v1/inference 端点（专属服务实测打通，真实 key 合成成功）。

接入方式（方案 A: 官方 SDK）:
    内部使用 dashscope.audio.tts_v2.SpeechSynthesizer（官方 Python SDK）:
        streaming_call(text)    -> 发送 run-task（首帧，含 parameters）+ continue-task（input.text）
        streaming_complete(...) -> 发送 finish-task 并阻塞等待 task-finished
    音频二进制帧（MP3）通过 ResultCallback.on_data 实时收集；
    task-failed 通过 ResultCallback.on_error 上报（业务错误，不重试）。

端点:
    默认  wss://dashscope.aliyuncs.com/api-ws/v1/inference（公共端点）
    专属服务用户把 conf.yaml cosyvoice_cloud.ws_url 改为自己的专属 host，例如:
        wss://ws-s8erlxkb5f8b1j59.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference
    引擎会同时把 ws_url 写入环境变量 DASHSCOPE_WEBSOCKET_BASE_URL 并显式传给 SDK。

API Key:
    解析顺序（与旧版一致）: 构造参数 api_key -> 回退环境变量 ALIYUN_API_KEY
    -> 仍为空则记错返回失败（不发网络）。运行时同时设置
    DASHSCOPE_API_KEY 环境变量与 dashscope.api_key（SDK 构造时读取）。

依赖:
    dashscope>=1.26（新增，官方 SDK；PyPI 国内可安装）。
    模块顶层不 import dashscope（可选依赖，运行时延迟导入），因此未安装时
    tts_factory 的 edge_tts 等既有路径不受影响。
"""

import asyncio
import os

from loguru import logger

from .tts_interface import TTSInterface


class CosyVoiceSynthesisError(RuntimeError):
    """DashScope 服务端业务错误（task-failed）。属业务错误，不重试。"""


class CosyVoiceCloudTTSEngine(TTSInterface):
    """阿里云 DashScope CosyVoice 云端 TTS 引擎（官方 SDK，WebSocket 流式）。

    用法（与其它 TTS 引擎一致，通过 tts_factory 创建）:
        engine = TTSFactory.get_tts_engine("cosyvoice_cloud", api_key="sk-...")
        path = await engine.async_generate_audio("你好，我是猫娘~")

    API Key 解析顺序:
        1. 构造参数 api_key（来自 conf.yaml cosyvoice_cloud.api_key）
        2. 若为空或仍是 "${...}" 占位符，回退到环境变量 ALIYUN_API_KEY
        3. 仍为空 -> 记录错误并返回失败（不发起网络请求）
    """

    #: DashScope inference WebSocket 端点（官方 SDK 默认值一致，中国大陆直连）
    DEFAULT_WS_URL = "wss://dashscope.aliyuncs.com/api-ws/v1/inference"

    def __init__(
        self,
        api_key: str = "",
        voice: str = "longxiaochun",
        model: str = "cosyvoice-v1",
        format: str = "mp3",
        sample_rate: int = 22050,
        volume: int = 50,
        rate: float = 1.0,
        pitch: float = 1.0,
        ws_url: str = DEFAULT_WS_URL,
        timeout: float = 60.0,
        max_retries: int = 2,
    ):
        self.api_key = self._resolve_api_key(api_key)
        self.voice = voice
        self.model = model
        self.format = format
        self.sample_rate = sample_rate
        self.volume = volume
        self.rate = rate
        self.pitch = pitch
        self.ws_url = ws_url
        self.timeout = timeout
        self.max_retries = max_retries
        self.file_extension = format

    # ------------------------------------------------------------------ #
    # TTSInterface 契约
    # ------------------------------------------------------------------ #

    def generate_audio(self, text: str, file_name_no_ext=None) -> str:
        """同步生成音频（TTSInterface 抽象方法）。

        内部启动一个临时事件循环完成合成。
        异步调用方请使用 async_generate_audio（真正的异步实现，无需线程跳转）。

        Returns:
            str: 生成的音频文件路径；失败时返回空字符串 ""。
        """
        return asyncio.run(self.async_generate_audio(text, file_name_no_ext))

    async def async_generate_audio(self, text: str, file_name_no_ext=None) -> str:
        """异步流式合成音频（官方 SDK 为阻塞式，经 asyncio.to_thread 隔离）。

        Returns:
            str: 生成的音频文件路径；失败时返回空字符串 ""。
        """
        if not text or not str(text).strip():
            logger.error("CosyVoice Cloud: 输入文本为空，跳过合成。")
            return ""

        if not self.api_key:
            logger.error(
                "CosyVoice Cloud: 未配置 API Key（conf.yaml cosyvoice_cloud.api_key "
                "或环境变量 ALIYUN_API_KEY）。"
            )
            return ""

        file_name = self.generate_cache_file_name(file_name_no_ext, self.file_extension)
        text = str(text).strip()

        last_error = None
        for attempt in range(1, self.max_retries + 1):
            try:
                audio_bytes = await asyncio.wait_for(
                    asyncio.to_thread(self._synthesize_with_sdk, text),
                    timeout=self.timeout,
                )
            except CosyVoiceSynthesisError as e:
                # 业务错误（task-failed）：快速失败，不重试
                last_error = str(e)
                logger.error(f"CosyVoice Cloud: {last_error}")
                break
            except (ValueError, TypeError) as e:
                # 配置错误（如 format/sample_rate 组合非法）：快速失败，不重试
                last_error = f"{type(e).__name__}: {e}"
                logger.error(f"CosyVoice Cloud: 配置错误（不重试）: {last_error}")
                break
            except asyncio.TimeoutError:
                last_error = f"合成超时（>{self.timeout}s）"
                logger.warning(f"CosyVoice Cloud: {last_error}（第 {attempt} 次尝试）")
                continue
            except Exception as e:  # 网络/连接/协议异常（可重试）
                last_error = f"{type(e).__name__}: {e}"
                logger.warning(
                    f"CosyVoice Cloud: SDK 异常 {last_error}（第 {attempt} 次尝试）"
                )
                continue

            if not audio_bytes:
                last_error = "服务端未返回任何音频数据"
                logger.warning(f"CosyVoice Cloud: {last_error}（第 {attempt} 次尝试）")
                continue

            with open(file_name, "wb") as f:
                f.write(audio_bytes)
            logger.info(
                f"CosyVoice Cloud: 合成成功 -> {file_name}（{len(audio_bytes)} bytes）"
            )
            return file_name

        logger.error(f"CosyVoice Cloud: 合成失败（已尝试 {self.max_retries} 次）: {last_error}")
        return ""

    # ------------------------------------------------------------------ #
    # 内部实现
    # ------------------------------------------------------------------ #

    @staticmethod
    def _resolve_api_key(api_key: str) -> str:
        """解析 API Key：参数 -> 环境变量 ALIYUN_API_KEY；跳过 "${...}" 占位符。"""
        if api_key and not str(api_key).startswith("${"):
            return str(api_key).strip()
        return os.environ.get("ALIYUN_API_KEY", "").strip()

    def _synthesize_with_sdk(self, text: str) -> bytes:
        """用官方 dashscope SDK 合成（阻塞式，调用方需经 to_thread 隔离）。

        协议流（SDK 内部实现，真实线上实测）:
            Client -> run-task（input 空，parameters: model/voice/volume/format/
                      sample_rate/rate/pitch/seed/type ...）
            Client -> continue-task（payload.input.text = 要合成的文本）
            Server -> task-started
            Server -> 二进制音频帧（MP3）*
            Client -> finish-task（input 空）
            Server -> task-finished | task-failed

        Raises:
            CosyVoiceSynthesisError: 服务端业务错误（task-failed）。
            Exception: 网络/连接/协议异常（可重试）。
        """
        # 先写环境变量再 import：保证 SDK 首次 import 时读到我们的 key/专属 host
        os.environ["DASHSCOPE_API_KEY"] = self.api_key
        os.environ["DASHSCOPE_WEBSOCKET_BASE_URL"] = self.ws_url

        import dashscope  # 官方 SDK（可选依赖，延迟导入）
        from dashscope.audio.tts_v2 import ResultCallback, SpeechSynthesizer

        # SDK 在 SpeechSynthesizer 构造时读取 dashscope.api_key（模块属性）
        dashscope.api_key = self.api_key

        audio_format = self._resolve_audio_format(self.format, self.sample_rate)

        class _Callback(ResultCallback):
            """收集音频帧 / 业务错误（SDK 在 ws 线程回调，全部同步收集）。"""

            def __init__(self):
                self.audio_chunks: list[bytes] = []
                self.error: str = ""

            def on_data(self, data: bytes):
                self.audio_chunks.append(data)

            def on_error(self, message):
                self.error = str(message)

            def on_event(self, message):
                logger.debug(f"CosyVoice Cloud: result-generated {str(message)[:120]}")

            def on_complete(self):
                logger.debug("CosyVoice Cloud: task-finished")

        logger.debug(
            f"CosyVoice Cloud: 连接 {self.ws_url}（model={self.model}, "
            f"voice={self.voice}, format={self.format}@{self.sample_rate}Hz）"
        )

        callback = _Callback()
        synthesizer = SpeechSynthesizer(
            model=self.model,
            voice=self.voice,
            format=audio_format,
            volume=self.volume,
            speech_rate=self.rate,
            pitch_rate=self.pitch,
            callback=callback,
            url=self.ws_url,
        )
        try:
            # run-task（首帧）+ continue-task（input.text）
            synthesizer.streaming_call(text)
            # finish-task 并等待 task-finished / task-failed
            synthesizer.streaming_complete(
                complete_timeout_millis=int(self.timeout * 1000)
            )
        except Exception:
            # 连接/协议异常：尽量关闭 WS，避免残留半开连接
            self._safe_close(synthesizer)
            raise

        if callback.error:
            raise CosyVoiceSynthesisError(
                f"DashScope 合成失败（task-failed）: {callback.error}"
            )
        if not callback.audio_chunks:
            logger.warning("CosyVoice Cloud: 服务端未返回音频数据")
            return b""
        return b"".join(callback.audio_chunks)

    @staticmethod
    def _safe_close(synthesizer) -> None:
        """尽力关闭 SDK 的 WebSocket 连接（失败静默）。"""
        try:
            if synthesizer is not None:
                synthesizer.close()
        except Exception:
            pass

    @staticmethod
    def _resolve_audio_format(format: str, sample_rate: int):
        """把配置的 format/sample_rate 映射到 SDK 的 AudioFormat 枚举。

        先精确匹配（format+sample_rate），再退化为 format-only 匹配，
        仍无则抛 ValueError（配置错误，不重试）。
        """
        from dashscope.audio.tts_v2.speech_synthesizer import AudioFormat

        fmt = (format or "mp3").lower()
        for member in AudioFormat:
            if member.sample_rate == 0:  # DEFAULT 哨兵项，跳过
                continue
            if member.format == fmt and member.sample_rate == sample_rate:
                return member
        # sample_rate 未指定（0/None）时才退化为 format-only 匹配
        if not sample_rate:
            for member in AudioFormat:
                if member.sample_rate == 0:
                    continue
                if member.format == fmt:
                    return member
        supported = sorted({m.format for m in AudioFormat if m.sample_rate})
        raise ValueError(
            f"不支持的 CosyVoice 音频格式组合 format={format!r} sample_rate={sample_rate}。"
            f"支持的格式: {supported}"
        )


if __name__ == "__main__":
    # 手动冒烟入口: python -m open_llm_vtuber.tts.cosyvoice_cloud_tts "你好"
    import sys

    text = sys.argv[1] if len(sys.argv) > 1 else "你好，我是猫娘~"
    engine = CosyVoiceCloudTTSEngine()
    path = engine.generate_audio(text)
    print(f"result: {path}")
