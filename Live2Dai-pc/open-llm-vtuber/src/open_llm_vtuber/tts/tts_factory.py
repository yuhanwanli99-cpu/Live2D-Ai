from typing import Type
from .tts_interface import TTSInterface


class TTSFactory:
    @staticmethod
    def get_tts_engine(engine_type, **kwargs) -> Type[TTSInterface]:
        if engine_type == "azure_tts":
            from .azure_tts import TTSEngine as AzureTTSEngine

            return AzureTTSEngine(
                kwargs.get("api_key"),
                kwargs.get("region"),
                kwargs.get("voice"),
                kwargs.get("pitch"),
                kwargs.get("rate"),
            )
        elif engine_type == "edge_tts":
            from .edge_tts import TTSEngine as EdgeTTSEngine

            return EdgeTTSEngine(kwargs.get("voice"))
        elif engine_type == "pyttsx3_tts":
            from .pyttsx3_tts import TTSEngine as Pyttsx3TTSEngine

            return Pyttsx3TTSEngine()
        elif engine_type == "melo_tts":
            from .melo_tts import TTSEngine as MeloTTSEngine

            return MeloTTSEngine(
                speaker=kwargs.get("speaker"),
                language=kwargs.get("language"),
                device=kwargs.get("device"),
                speed=kwargs.get("speed"),
            )
        elif engine_type == "x_tts":
            from .x_tts import TTSEngine as XTTSEngine

            return XTTSEngine(
                api_url=kwargs.get("api_url"),
                speaker_wav=kwargs.get("speaker_wav"),
                language=kwargs.get("language"),
            )
        elif engine_type == "gpt_sovits_tts":
            from .gpt_sovits_tts import TTSEngine as GSVEngine

            return GSVEngine(
                api_url=kwargs.get("api_url"),
                text_lang=kwargs.get("text_lang"),
                ref_audio_path=kwargs.get("ref_audio_path"),
                prompt_lang=kwargs.get("prompt_lang"),
                prompt_text=kwargs.get("prompt_text"),
                text_split_method=kwargs.get("text_split_method"),
                batch_size=kwargs.get("batch_size"),
                media_type=kwargs.get("media_type"),
                streaming_mode=kwargs.get("streaming_mode"),
            )
        elif engine_type == "siliconflow_tts":
            from .siliconflow_tts import SiliconFlowTTS

            return SiliconFlowTTS(
                api_url=kwargs.get("api_url"),
                api_key=kwargs.get("api_key"),
                default_model=kwargs.get("default_model"),
                default_voice=kwargs.get("default_voice"),
                sample_rate=kwargs.get("sample_rate"),
                response_format=kwargs.get("response_format"),
                stream=kwargs.get("stream"),
                speed=kwargs.get("speed"),
                gain=kwargs.get("gain"),
            )
        elif engine_type == "fish_api_tts":
            from .fish_api_tts import TTSEngine as FishAPITTSEngine

            return FishAPITTSEngine(
                api_key=kwargs.get("api_key"),
                reference_id=kwargs.get("reference_id"),
                latency=kwargs.get("latency"),
                base_url=kwargs.get("base_url"),
            )
        elif engine_type == "minimax_tts":
            from .minimax_tts import TTSEngine as MinimaxTTSEngine

            return MinimaxTTSEngine(
                group_id=kwargs.get("group_id"),
                api_key=kwargs.get("api_key"),
                model=kwargs.get("model", "speech-02-turbo"),
                voice_id=kwargs.get("voice_id", "male-qn-qingse"),
                pronunciation_dict=kwargs.get("pronunciation_dict", ""),
            )
        elif engine_type == "sherpa_onnx_tts":
            from .sherpa_onnx_tts import TTSEngine as SherpaOnnxTTSEngine

            return SherpaOnnxTTSEngine(**kwargs)
        elif engine_type == "openai_tts":
            from .openai_tts import TTSEngine as OpenAITTSEngine

            # Pass relevant config options, allowing defaults in openai_tts.py if not provided
            return OpenAITTSEngine(
                model=kwargs.get("model"),  # Will use default "kokoro" if not in kwargs
                voice=kwargs.get(
                    "voice"
                ),  # Will use default "af_sky+af_bella" if not in kwargs
                api_key=kwargs.get(
                    "api_key"
                ),  # Will use default "not-needed" if not in kwargs
                base_url=kwargs.get(
                    "base_url"
                ),  # Will use default "http://localhost:8880/v1" if not in kwargs
                file_extension=kwargs.get(
                    "file_extension"
                ),  # Will use default "mp3" if not in kwargs
            )

        elif engine_type == "spark_tts":
            from .spark_tts import TTSEngine as SparkTTSEngine

            return SparkTTSEngine(
                api_url=kwargs.get("api_url"),
                prompt_wav_upload=kwargs.get("prompt_wav_upload"),
                api_name=kwargs.get("api_name"),
                gender=kwargs.get("gender"),
                pitch=kwargs.get("pitch"),
                speed=kwargs.get("speed"),
            )
        elif engine_type == "cosyvoice_cloud":
            # V1.2 新增: 阿里云 DashScope CosyVoice 云端 TTS（中国大陆直连，可选）
            from .cosyvoice_cloud_tts import CosyVoiceCloudTTSEngine

            return CosyVoiceCloudTTSEngine(
                api_key=kwargs.get("api_key", ""),
                voice=kwargs.get("voice", "longxiaochun"),
                model=kwargs.get("model", "cosyvoice-v1"),
                format=kwargs.get("format", "mp3"),
                sample_rate=kwargs.get("sample_rate", 22050),
                volume=kwargs.get("volume", 50),
                rate=kwargs.get("rate", 1.0),
                pitch=kwargs.get("pitch", 1.0),
                ws_url=kwargs.get("ws_url", CosyVoiceCloudTTSEngine.DEFAULT_WS_URL),
                timeout=kwargs.get("timeout", 60.0),
                max_retries=kwargs.get("max_retries", 2),
            )
        else:
            raise ValueError(f"Unknown TTS engine type: {engine_type}")
