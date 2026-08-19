"""V1.2.1 PC 端 CosyVoice 云端 TTS + tts_factory 单元测试（官方 SDK 协议修正版）。

覆盖（不发起真实网络请求，官方 dashscope SDK 的 SpeechSynthesizer 用假件模拟）:
- tts_factory: 'cosyvoice_cloud' 分支返回 CosyVoiceCloudTTSEngine
- tts_factory: 现有 'edge_tts' 路径不被破坏（回归保护）
- CosyVoiceCloudTtsConfig / TTSConfig 解析（tts_model='cosyvoice_cloud' 可校验）
- 协议修正回归: 端点必须是 /api-ws/v1/inference（run-task 协议），绝非 /realtime
- 引擎合成流程: streaming_call(文本) -> streaming_complete() 的 run-task 顺序驱动
  （SDK 内部把 streaming_call 翻译为 run-task+continue-task、streaming_complete
    翻译为 finish-task；单测验证引擎按正确顺序驱动 SDK）
- 引擎失败路径: task-failed（业务错误，不重试）/ 空文本 / 未配置 API Key（不发网络）
- API Key 解析: 参数优先、${...} 占位符回退环境变量 ALIYUN_API_KEY
- dashscope.api_key / DASHSCOPE_API_KEY / DASHSCOPE_WEBSOCKET_BASE_URL 接线
- AudioFormat 枚举映射（format/sample_rate -> SDK AudioFormat）
"""
import asyncio
import json
import os
import sys
import unittest
from unittest.mock import MagicMock, patch

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.config_manager.tts import (  # noqa: E402
    CosyVoiceCloudTtsConfig,
    TTSConfig,
)
from open_llm_vtuber.tts.cosyvoice_cloud_tts import (  # noqa: E402
    CosyVoiceCloudTTSEngine,
    CosyVoiceSynthesisError,
)
from open_llm_vtuber.tts.edge_tts import TTSEngine as EdgeTTSEngine  # noqa: E402
from open_llm_vtuber.tts.tts_factory import TTSFactory  # noqa: E402

INFERENCE_ENDPOINT = "wss://dashscope.aliyuncs.com/api-ws/v1/inference"


class FakeSpeechSynthesizer:
    """官方 dashscope SpeechSynthesizer 的同步假件。

    模拟引擎驱动的 streaming 流程:
        streaming_call(text)    -> run-task + continue-task 语义
                                   （成功: on_data 音频帧；失败: on_error 业务错误）
        streaming_complete(...) -> finish-task 语义（on_complete）

    通过构造参数注入行为:
        _audio:        on_data 发送的音频字节（b"" = 无音频）
        _error_msg:    模拟 task-failed（on_error）
        _raise_on_call: streaming_call 抛出的网络/连接异常
    """

    instances: list = []
    default_audio = b"\x00\x01\x02\x03\x04"

    def __init__(self, **kwargs):
        self.kwargs = kwargs
        self.callback = kwargs.get("callback")
        self.calls: list = []
        self.started = False
        self.error_msg = kwargs.get("_error_msg")
        self.audio = kwargs.get("_audio", FakeSpeechSynthesizer.default_audio)
        self.raise_on_call = kwargs.get("_raise_on_call")
        FakeSpeechSynthesizer.instances.append(self)

    def streaming_call(self, text):
        self.calls.append(("streaming_call", text))
        self.started = True
        if self.raise_on_call is not None:
            raise self.raise_on_call
        if self.callback is None:
            return
        if self.error_msg:
            self.callback.on_error(self.error_msg)
            self.callback.on_close()
        elif self.audio:
            self.callback.on_data(self.audio)

    def streaming_complete(self, **kwargs):
        self.calls.append(("streaming_complete", kwargs))
        if self.error_msg:
            return
        if self.callback is not None:
            self.callback.on_complete()
            self.callback.on_close()

    def close(self):
        self.calls.append(("close",))


def patch_speech_synthesizer(**behavior):
    """把 dashscope.audio.tts_v2.SpeechSynthesizer 替换为 FakeSpeechSynthesizer。

    引擎在 _synthesize_with_sdk 内延迟 `from dashscope.audio.tts_v2 import
    SpeechSynthesizer`，因此 patch 源模块属性即可在调用时生效。
    """
    FakeSpeechSynthesizer.instances.clear()

    def factory(**kwargs):
        merged = dict(behavior)
        merged.update(kwargs)  # 引擎显式传参优先，behavior 仅注入行为参数
        return FakeSpeechSynthesizer(**merged)

    patcher = patch("dashscope.audio.tts_v2.SpeechSynthesizer", factory)
    patcher.start()
    return patcher


def fake_task_failed(message="invalid voice"):
    """构造一条与官方 task-failed 事件一致的错误消息字符串。"""
    return json.dumps(
        {
            "header": {"task_id": "t1", "event": "task-failed"},
            "payload": {"message": message},
        },
        ensure_ascii=False,
    )


class TestTTSFactory(unittest.TestCase):
    def test_cosyvoice_cloud_branch_returns_engine(self):
        engine = TTSFactory.get_tts_engine(
            "cosyvoice_cloud", api_key="sk-test", voice="longxiaochun"
        )
        self.assertIsInstance(engine, CosyVoiceCloudTTSEngine)
        self.assertEqual(engine.api_key, "sk-test")
        self.assertEqual(engine.voice, "longxiaochun")
        self.assertEqual(engine.model, "cosyvoice-v1")
        self.assertEqual(engine.format, "mp3")
        self.assertEqual(engine.ws_url, CosyVoiceCloudTTSEngine.DEFAULT_WS_URL)
        self.assertEqual(engine.ws_url, INFERENCE_ENDPOINT)

    def test_edge_tts_path_unchanged(self):
        """回归保护: 默认 edge_tts 路径必须仍可用（不破坏现有链路）。"""
        engine = TTSFactory.get_tts_engine(
            "edge_tts", voice="zh-CN-XiaoxiaoNeural"
        )
        self.assertIsInstance(engine, EdgeTTSEngine)
        self.assertEqual(engine.voice, "zh-CN-XiaoxiaoNeural")

    def test_unknown_engine_raises(self):
        with self.assertRaises(ValueError):
            TTSFactory.get_tts_engine("not_a_real_engine")


class TestConfigParsing(unittest.TestCase):
    def test_cosyvoice_cloud_config_defaults(self):
        cfg = CosyVoiceCloudTtsConfig(api_key="sk-1")
        self.assertEqual(cfg.api_key, "sk-1")
        self.assertEqual(cfg.model, "cosyvoice-v1")
        self.assertEqual(cfg.voice, "longxiaochun")
        self.assertEqual(cfg.format, "mp3")
        self.assertEqual(cfg.ws_url, INFERENCE_ENDPOINT)
        self.assertNotIn("realtime", cfg.ws_url)

    def test_tts_config_accepts_cosyvoice_cloud(self):
        cfg = TTSConfig(
            tts_model="cosyvoice_cloud",
            cosyvoice_cloud={"api_key": "${ALIYUN_API_KEY}"},
        )
        self.assertEqual(cfg.tts_model, "cosyvoice_cloud")
        self.assertIsNotNone(cfg.cosyvoice_cloud)
        self.assertEqual(cfg.cosyvoice_cloud.api_key, "${ALIYUN_API_KEY}")

    def test_tts_config_edge_tts_still_valid(self):
        cfg = TTSConfig(tts_model="edge_tts", edge_tts={"voice": "zh-CN-XiaoxiaoNeural"})
        self.assertEqual(cfg.tts_model, "edge_tts")


class TestProtocolFix(unittest.TestCase):
    """协议修正回归: start-synthesis + /realtime -> run-task + /inference。"""

    def test_default_ws_url_is_inference_not_realtime(self):
        self.assertEqual(
            CosyVoiceCloudTTSEngine.DEFAULT_WS_URL,
            "wss://dashscope.aliyuncs.com/api-ws/v1/inference",
        )
        self.assertNotIn("realtime", CosyVoiceCloudTTSEngine.DEFAULT_WS_URL)
        self.assertTrue(CosyVoiceCloudTTSEngine.DEFAULT_WS_URL.endswith("/api-ws/v1/inference"))

    def test_audio_format_resolution(self):
        from dashscope.audio.tts_v2.speech_synthesizer import AudioFormat

        self.assertIs(
            CosyVoiceCloudTTSEngine._resolve_audio_format("mp3", 22050),
            AudioFormat.MP3_22050HZ_MONO_256KBPS,
        )
        self.assertIs(
            CosyVoiceCloudTTSEngine._resolve_audio_format("wav", 24000),
            AudioFormat.WAV_24000HZ_MONO_16BIT,
        )
        self.assertIs(
            CosyVoiceCloudTTSEngine._resolve_audio_format("pcm", 16000),
            AudioFormat.PCM_16000HZ_MONO_16BIT,
        )
        with self.assertRaises(ValueError):
            CosyVoiceCloudTTSEngine._resolve_audio_format("mp3", 99999)


class TestApiKeyResolution(unittest.TestCase):
    def test_explicit_key_wins(self):
        engine = CosyVoiceCloudTTSEngine(api_key="sk-explicit")
        self.assertEqual(engine.api_key, "sk-explicit")

    def test_placeholder_falls_back_to_env(self):
        with patch.dict(os.environ, {"ALIYUN_API_KEY": "sk-env"}, clear=False):
            engine = CosyVoiceCloudTTSEngine(api_key="${ALIYUN_API_KEY}")
            self.assertEqual(engine.api_key, "sk-env")

    def test_empty_key_without_env(self):
        with patch.dict(os.environ, {}, clear=True):
            engine = CosyVoiceCloudTTSEngine(api_key="")
            self.assertEqual(engine.api_key, "")

    def test_placeholder_without_env_stays_empty(self):
        with patch.dict(os.environ, {}, clear=True):
            engine = CosyVoiceCloudTTSEngine(api_key="${ALIYUN_API_KEY}")
            self.assertEqual(engine.api_key, "")


class TestEngineSynthesis(unittest.TestCase):
    def tearDown(self):
        cache_dir = os.path.join(os.path.dirname(__file__), "..", "cache")
        if os.path.isdir(cache_dir):
            for name in os.listdir(cache_dir):
                if name.startswith("test_cosyvoice_"):
                    os.remove(os.path.join(cache_dir, name))

    def test_happy_path_writes_audio_file(self):
        patcher = patch_speech_synthesizer()
        try:
            engine = CosyVoiceCloudTTSEngine(api_key="sk-test", max_retries=1)
            result = asyncio.run(
                engine.async_generate_audio("你好", "test_cosyvoice_happy")
            )
        finally:
            patcher.stop()

        self.assertTrue(os.path.exists(result))
        with open(result, "rb") as f:
            self.assertEqual(f.read(), FakeSpeechSynthesizer.default_audio)
        self.assertTrue(result.endswith(".mp3"))

        inst = FakeSpeechSynthesizer.instances[-1]
        # run-task 协议: streaming_call(文本) 在先, streaming_complete(finish) 在后
        self.assertEqual(
            [c[0] for c in inst.calls],
            ["streaming_call", "streaming_complete"],
        )
        self.assertEqual(inst.calls[0][1], "你好")
        self.assertEqual(inst.calls[1][1]["complete_timeout_millis"], 60000)
        # SDK 构造参数（与 conf.yaml cosyvoice_cloud 字段一一对应）
        from dashscope.audio.tts_v2.speech_synthesizer import AudioFormat

        self.assertEqual(inst.kwargs["model"], "cosyvoice-v1")
        self.assertEqual(inst.kwargs["voice"], "longxiaochun")
        self.assertIs(inst.kwargs["format"], AudioFormat.MP3_22050HZ_MONO_256KBPS)
        self.assertEqual(inst.kwargs["volume"], 50)
        self.assertEqual(inst.kwargs["speech_rate"], 1.0)
        self.assertEqual(inst.kwargs["pitch_rate"], 1.0)
        self.assertEqual(inst.kwargs["url"], INFERENCE_ENDPOINT)
        self.assertIsNotNone(inst.kwargs["callback"])

    def test_run_task_streaming_sequence_drives_sdk(self):
        """引擎按 run-task 协议顺序驱动 SDK（SDK 内部翻译为 WS 三事件）。"""
        patcher = patch_speech_synthesizer()
        try:
            engine = CosyVoiceCloudTTSEngine(api_key="sk-test", max_retries=1)
            asyncio.run(
                engine.async_generate_audio("你好世界", "test_cosyvoice_proto")
            )
        finally:
            patcher.stop()
        inst = FakeSpeechSynthesizer.instances[-1]
        self.assertEqual(inst.calls[0][0], "streaming_call")
        self.assertEqual(inst.calls[0][1], "你好世界")
        self.assertEqual(inst.calls[1][0], "streaming_complete")
        self.assertTrue(inst.started)

    def test_engine_sets_sdk_api_key_and_ws_url(self):
        with patch.dict(
            os.environ,
            {"DASHSCOPE_API_KEY": "", "DASHSCOPE_WEBSOCKET_BASE_URL": ""},
        ):
            engine = CosyVoiceCloudTTSEngine(
                api_key="sk-abc",
                ws_url="wss://custom.example/api-ws/v1/inference",
            )
            patcher = patch_speech_synthesizer()
            try:
                asyncio.run(
                    engine.async_generate_audio("hi", "test_cosyvoice_env")
                )
            finally:
                patcher.stop()
            self.assertEqual(os.environ["DASHSCOPE_API_KEY"], "sk-abc")
            self.assertEqual(
                os.environ["DASHSCOPE_WEBSOCKET_BASE_URL"],
                "wss://custom.example/api-ws/v1/inference",
            )
            inst = FakeSpeechSynthesizer.instances[-1]
            self.assertEqual(
                inst.kwargs["url"], "wss://custom.example/api-ws/v1/inference"
            )

    def test_task_failed_business_error_fails_fast_no_retry(self):
        """task-failed 是业务错误: 快速失败，即使 max_retries>1 也不重试。"""
        patcher = patch_speech_synthesizer(_error_msg=fake_task_failed("invalid voice"))
        try:
            engine = CosyVoiceCloudTTSEngine(api_key="sk-test", max_retries=2)
            result = asyncio.run(
                engine.async_generate_audio("你好", "test_cosyvoice_fail")
            )
        finally:
            patcher.stop()
        self.assertEqual(result, "")
        self.assertEqual(len(FakeSpeechSynthesizer.instances), 1)

    def test_no_audio_chunks_returns_empty(self):
        patcher = patch_speech_synthesizer(_audio=b"")
        try:
            engine = CosyVoiceCloudTTSEngine(api_key="sk-test", max_retries=1)
            result = asyncio.run(
                engine.async_generate_audio("你好", "test_cosyvoice_empty")
            )
        finally:
            patcher.stop()
        self.assertEqual(result, "")

    def test_empty_text_skips_network(self):
        engine = CosyVoiceCloudTTSEngine(api_key="sk-test")
        with patch(
            "dashscope.audio.tts_v2.SpeechSynthesizer", MagicMock()
        ) as mock_synth:
            result = asyncio.run(engine.async_generate_audio("   "))
            mock_synth.assert_not_called()
        self.assertEqual(result, "")

    def test_missing_api_key_skips_network(self):
        with patch.dict(os.environ, {}, clear=True):
            engine = CosyVoiceCloudTTSEngine(api_key="")
        with patch(
            "dashscope.audio.tts_v2.SpeechSynthesizer", MagicMock()
        ) as mock_synth:
            result = asyncio.run(engine.async_generate_audio("你好"))
            mock_synth.assert_not_called()
        self.assertEqual(result, "")

    def test_network_error_retries_then_fails(self):
        """连接/网络异常（非业务错误）按 max_retries 重试。"""
        patcher = patch_speech_synthesizer(
            _raise_on_call=ConnectionError("connection refused")
        )
        try:
            engine = CosyVoiceCloudTTSEngine(api_key="sk-test", max_retries=2)
            result = asyncio.run(
                engine.async_generate_audio("你好", "test_cosyvoice_retry")
            )
        finally:
            patcher.stop()
        self.assertEqual(result, "")
        self.assertEqual(len(FakeSpeechSynthesizer.instances), 2)  # 初始 + 1 次重试

    def test_synthesis_error_type_exposed(self):
        self.assertTrue(issubclass(CosyVoiceSynthesisError, RuntimeError))


if __name__ == "__main__":
    unittest.main()
