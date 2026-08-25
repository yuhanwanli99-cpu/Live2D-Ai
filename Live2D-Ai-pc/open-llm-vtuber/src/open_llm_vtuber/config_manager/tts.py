# config_manager/tts.py
from pydantic import ValidationInfo, Field, model_validator
from typing import Literal, Optional, Dict, ClassVar
from .i18n import I18nMixin, Description

CartesiaLanguages = Literal[
    "en",
    "fr",
    "de",
    "es",
    "pt",
    "zh",
    "ja",
    "hi",
    "it",
    "ko",
    "nl",
    "pl",
    "ru",
    "sv",
    "tr",
    "tl",
    "bg",
    "ro",
    "ar",
    "cs",
    "el",
    "fi",
    "hr",
    "ms",
    "sk",
    "da",
    "ta",
    "uk",
    "hu",
    "no",
    "vi",
    "bn",
    "th",
    "he",
    "ka",
    "id",
    "te",
    "gu",
    "kn",
    "ml",
    "mr",
    "pa",
]

CartesiaEmotions = Literal[
    "neutral",
    "angry",
    "excited",
    "content",
    "sad",
    "scared",
    "happy",
    "enthusiastic",
    "elated",
    "euphoric",
    "triumphant",
    "amazed",
    "surprised",
    "flirtatious",
    "joking/comedic",
    "curious",
    "peaceful",
    "serene",
    "calm",
    "grateful",
    "affectionate",
    "trust",
    "sympathetic",
    "anticipation",
    "mysterious",
    "mad",
    "outraged",
    "frustrated",
    "agitated",
    "threatened",
    "disgusted",
    "contempt",
    "envious",
    "sarcastic",
    "ironic",
    "dejected",
    "melancholic",
    "disappointed",
    "hurt",
    "guilty",
    "bored",
    "tired",
    "rejected",
    "nostalgic",
    "wistful",
    "apologetic",
    "hesitant",
    "insecure",
    "confused",
    "resigned",
    "anxious",
    "panicked",
    "alarmed",
    "proud",
    "confident",
    "distant",
    "skeptical",
    "contemplative",
    "determined",
]


class AzureTTSConfig(I18nMixin):
    """Configuration for Azure TTS service."""

    api_key: str = Field(..., alias="api_key")
    region: str = Field(..., alias="region")
    voice: str = Field(..., alias="voice")
    pitch: str = Field(..., alias="pitch")
    rate: str = Field(..., alias="rate")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "api_key": Description(
            en="API key for Azure TTS service", zh="Azure TTS 服务的 API 密钥"
        ),
        "region": Description(
            en="Azure region (e.g., eastus)", zh="Azure 区域（如 eastus）"
        ),
        "voice": Description(
            en="Voice name to use for Azure TTS", zh="Azure TTS 使用的语音名称"
        ),
        "pitch": Description(en="Pitch adjustment percentage", zh="音高调整百分比"),
        "rate": Description(en="Speaking rate adjustment", zh="语速调整"),
    }


class BarkTTSConfig(I18nMixin):
    """Configuration for Bark TTS."""

    voice: str = Field(..., alias="voice")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "voice": Description(
            en="Voice name to use for Bark TTS", zh="Bark TTS 使用的语音名称"
        ),
    }


class EdgeTTSConfig(I18nMixin):
    """Configuration for Edge TTS."""

    voice: str = Field(..., alias="voice")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "voice": Description(
            en="Voice name to use for Edge TTS (use 'edge-tts --list-voices' to list available voices)",
            zh="Edge TTS 使用的语音名称（使用 'edge-tts --list-voices' 列出可用语音）",
        ),
    }


class CosyvoiceTTSConfig(I18nMixin):
    """Configuration for Cosyvoice TTS."""

    client_url: str = Field(..., alias="client_url")
    mode_checkbox_group: str = Field(..., alias="mode_checkbox_group")
    sft_dropdown: str = Field(..., alias="sft_dropdown")
    prompt_text: str = Field(..., alias="prompt_text")
    prompt_wav_upload_url: str = Field(..., alias="prompt_wav_upload_url")
    prompt_wav_record_url: str = Field(..., alias="prompt_wav_record_url")
    instruct_text: str = Field(..., alias="instruct_text")
    seed: int = Field(..., alias="seed")
    api_name: str = Field(..., alias="api_name")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "client_url": Description(
            en="URL of the CosyVoice Gradio web UI", zh="CosyVoice Gradio Web UI 的 URL"
        ),
        "mode_checkbox_group": Description(
            en="Mode checkbox group value", zh="模式复选框组值"
        ),
        "sft_dropdown": Description(en="SFT dropdown value", zh="SFT 下拉框值"),
        "prompt_text": Description(en="Prompt text", zh="提示文本"),
        "prompt_wav_upload_url": Description(
            en="URL for prompt WAV file upload", zh="提示音频文件上传 URL"
        ),
        "prompt_wav_record_url": Description(
            en="URL for prompt WAV file recording", zh="提示音频文件录制 URL"
        ),
        "instruct_text": Description(en="Instruction text", zh="指令文本"),
        "seed": Description(en="Random seed", zh="随机种子"),
        "api_name": Description(en="API endpoint name", zh="API 端点名称"),
    }


class Cosyvoice2TTSConfig(I18nMixin):
    """Configuration for Cosyvoice2 TTS."""

    client_url: str = Field(..., alias="client_url")
    mode_checkbox_group: str = Field(..., alias="mode_checkbox_group")
    sft_dropdown: str = Field(..., alias="sft_dropdown")
    prompt_text: str = Field(..., alias="prompt_text")
    prompt_wav_upload_url: str = Field(..., alias="prompt_wav_upload_url")
    prompt_wav_record_url: str = Field(..., alias="prompt_wav_record_url")
    instruct_text: str = Field(..., alias="instruct_text")
    stream: bool = Field(..., alias="stream")
    seed: int = Field(..., alias="seed")
    speed: float = Field(..., alias="speed")
    api_name: str = Field(..., alias="api_name")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "client_url": Description(
            en="URL of the CosyVoice Gradio web UI", zh="CosyVoice Gradio Web UI 的 URL"
        ),
        "mode_checkbox_group": Description(
            en="Mode checkbox group value", zh="模式复选框组值"
        ),
        "sft_dropdown": Description(en="SFT dropdown value", zh="SFT 下拉框值"),
        "prompt_text": Description(en="Prompt text", zh="提示文本"),
        "prompt_wav_upload_url": Description(
            en="URL for prompt WAV file upload", zh="提示音频文件上传 URL"
        ),
        "prompt_wav_record_url": Description(
            en="URL for prompt WAV file recording", zh="提示音频文件录制 URL"
        ),
        "instruct_text": Description(en="Instruction text", zh="指令文本"),
        "stream": Description(en="Streaming inference", zh="流式推理"),
        "seed": Description(en="Random seed", zh="随机种子"),
        "speed": Description(en="Speech speed multiplier", zh="语速倍数"),
        "api_name": Description(en="API endpoint name", zh="API 端点名称"),
    }


class CosyVoiceCloudTtsConfig(I18nMixin):
    """Configuration for CosyVoice Cloud TTS (阿里云 DashScope inference API).

    V1.2 新增: 云端 CosyVoice（中国大陆直连，官方 dashscope SDK + run-task 协议）。
    V1.2.1 修正: 端点从 /api-ws/v1/realtime 改为 /api-ws/v1/inference（专属服务实测）。
    api_key 为空或 "${...}" 占位符时，引擎会回退读取环境变量 ALIYUN_API_KEY。
    """

    api_key: str = Field("", alias="api_key")
    voice: str = Field("longxiaochun", alias="voice")
    model: str = Field("cosyvoice-v1", alias="model")
    format: str = Field("mp3", alias="format")
    sample_rate: int = Field(22050, alias="sample_rate")
    volume: int = Field(50, alias="volume")
    rate: float = Field(1.0, alias="rate")
    pitch: float = Field(1.0, alias="pitch")
    ws_url: str = Field(
        "wss://dashscope.aliyuncs.com/api-ws/v1/inference", alias="ws_url"
    )
    timeout: float = Field(60.0, alias="timeout")
    max_retries: int = Field(2, alias="max_retries")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "api_key": Description(
            en="Aliyun DashScope API key (or env ALIYUN_API_KEY)",
            zh="阿里云 DashScope API Key（或环境变量 ALIYUN_API_KEY）",
        ),
        "voice": Description(
            en="Voice name (e.g., longxiaochun)", zh="音色名称（如 longxiaochun）"
        ),
        "model": Description(
            en="CosyVoice model name (e.g., cosyvoice-v1)",
            zh="CosyVoice 模型名（如 cosyvoice-v1）",
        ),
        "format": Description(
            en="Output audio format (mp3/pcm/wav)", zh="输出音频格式（mp3/pcm/wav）"
        ),
        "sample_rate": Description(
            en="Audio sample rate (e.g., 22050)", zh="音频采样率（如 22050）"
        ),
        "volume": Description(
            en="Speech volume (0-100)", zh="语音音量（0-100）"
        ),
        "rate": Description(
            en="Speech rate multiplier", zh="语速倍数"
        ),
        "pitch": Description(
            en="Speech pitch multiplier", zh="音高倍数"
        ),
        "ws_url": Description(
            en="DashScope inference WebSocket endpoint (run-task protocol)",
            zh="DashScope inference WebSocket 端点（run-task 协议）",
        ),
        "timeout": Description(
            en="Synthesis timeout in seconds", zh="合成超时时间（秒）"
        ),
        "max_retries": Description(
            en="Max retries for transient network errors",
            zh="网络瞬时错误的最大重试次数",
        ),
    }


class QwenTTSConfig(I18nMixin):
    """Configuration for Qwen TTS (DashScope non-realtime)."""

    api_key: str = Field("", alias="api_key")
    voice: str = Field("Cherry", alias="voice")
    model: str = Field("qwen3-tts-flash", alias="model")
    language_type: str = Field("Auto", alias="language_type")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "api_key": Description(
            en="DashScope API key", zh="DashScope API Key"
        ),
        "voice": Description(
            en="Voice name (e.g., Cherry/Ethan/Chelsie)", zh="音色名称（如 Cherry/Ethan/Chelsie）"
        ),
        "model": Description(
            en="Qwen TTS model name", zh="Qwen TTS 模型名"
        ),
        "language_type": Description(
            en="Language type (Auto/Chinese/English/Japanese/Korean)",
            zh="语言类型（Auto/Chinese/English/Japanese/Korean）",
        ),
    }


class MeloTTSConfig(I18nMixin):
    """Configuration for local Melo TTS (MyShell MeloTTS, multi-lingual).

    v1.3（本地 TTS 计划）新增: 与本地 Melo API 对齐的 language/device/四韵律参数/format。
    ZH 是实际的中英混合模型；四参数只影响韵律节奏，不改音色。
    """

    language: str = Field("ZH", alias="language")
    device: str = Field("cpu", alias="device")
    speaker: Optional[str] = Field(None, alias="speaker")
    speed: float = Field(1.0, alias="speed")
    sdp_ratio: float = Field(0.2, alias="sdp_ratio")
    noise_scale: float = Field(0.6, alias="noise_scale")
    noise_scale_w: float = Field(0.8, alias="noise_scale_w")
    format: str = Field("wav", alias="format")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "language": Description(
            en="Language (ZH/EN/ES/FR/JP/KR); ZH is a mixed Chinese-English model",
            zh="语言（ZH/EN/ES/FR/JP/KR）；ZH 为实际的中英混合模型",
        ),
        "device": Description(
            en="Device to use (cpu, cuda, auto)", zh="使用的设备（cpu、cuda、auto）"
        ),
        "speaker": Description(
            en="Optional speaker name; defaults to the language key in spk2id",
            zh="可选说话人名称；缺省时按语言 key 取 spk2id",
        ),
        "speed": Description(
            en="Speech speed multiplier (0.5~2.0)", zh="语速倍数（0.5~2.0）"
        ),
        "sdp_ratio": Description(
            en="Stochastic duration predictor ratio (0.0~1.0, higher=more intonation)",
            zh="随机韵律预测占比（0.0~1.0，调高更有起伏）",
        ),
        "noise_scale": Description(
            en="Pitch randomness (0.0~1.0, low=smooth, high=natural but unstable)",
            zh="音高随机度（0.0~1.0，低平稳、高自然但可能不稳）",
        ),
        "noise_scale_w": Description(
            en="Phoneme duration randomness (0.0~1.0, high=more rhythm)",
            zh="音素时长随机度（0.0~1.0，高则抑扬顿挫）",
        ),
        "format": Description(
            en="Output audio format (wav)", zh="输出音频格式（wav）"
        ),
    }


class XTTSConfig(I18nMixin):
    """Configuration for XTTS."""

    api_url: str = Field(..., alias="api_url")
    speaker_wav: str = Field(..., alias="speaker_wav")
    language: str = Field(..., alias="language")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "api_url": Description(
            en="URL of the XTTS API endpoint", zh="XTTS API 端点的 URL"
        ),
        "speaker_wav": Description(
            en="Speaker reference WAV file", zh="说话人参考音频文件"
        ),
        "language": Description(
            en="Language code (e.g., en, zh)", zh="语言代码（如 en、zh）"
        ),
    }


class GPTSoVITSConfig(I18nMixin):
    """Configuration for GPT-SoVITS."""

    api_url: str = Field(..., alias="api_url")
    text_lang: str = Field(..., alias="text_lang")
    ref_audio_path: str = Field(..., alias="ref_audio_path")
    prompt_lang: str = Field(..., alias="prompt_lang")
    prompt_text: str = Field(..., alias="prompt_text")
    text_split_method: str = Field(..., alias="text_split_method")
    batch_size: str = Field(..., alias="batch_size")
    media_type: str = Field(..., alias="media_type")
    streaming_mode: str = Field(..., alias="streaming_mode")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "api_url": Description(
            en="URL of the GPT-SoVITS API endpoint", zh="GPT-SoVITS API 端点的 URL"
        ),
        "text_lang": Description(en="Language of the input text", zh="输入文本的语言"),
        "ref_audio_path": Description(
            en="Path to reference audio file", zh="参考音频文件路径"
        ),
        "prompt_lang": Description(en="Language of the prompt", zh="提示词语言"),
        "prompt_text": Description(en="Prompt text", zh="提示文本"),
        "text_split_method": Description(
            en="Method for splitting text", zh="文本分割方法"
        ),
        "batch_size": Description(en="Batch size for processing", zh="处理批次大小"),
        "media_type": Description(en="Output media type", zh="输出媒体类型"),
        "streaming_mode": Description(en="Enable streaming mode", zh="启用流式模式"),
    }


class FishAPITTSConfig(I18nMixin):
    """Configuration for Fish API TTS."""

    api_key: str = Field(..., alias="api_key")
    reference_id: str = Field(..., alias="reference_id")
    latency: Literal["normal", "balanced"] = Field(..., alias="latency")
    base_url: str = Field(..., alias="base_url")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "api_key": Description(
            en="API key for Fish TTS service", zh="Fish TTS 服务的 API 密钥"
        ),
        "reference_id": Description(
            en="Voice reference ID from Fish Audio website",
            zh="来自 Fish Audio 网站的语音参考 ID",
        ),
        "latency": Description(
            en="Latency mode (normal or balanced)", zh="延迟模式（normal 或 balanced）"
        ),
        "base_url": Description(
            en="Base URL for Fish TTS API", zh="Fish TTS API 的基础 URL"
        ),
    }


class CoquiTTSConfig(I18nMixin):
    """Configuration for Coqui TTS."""

    model_name: str = Field(..., alias="model_name")
    speaker_wav: str = Field("", alias="speaker_wav")
    language: str = Field(..., alias="language")
    device: str = Field("", alias="device")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "model_name": Description(
            en="Name of the TTS model to use", zh="要使用的 TTS 模型名称"
        ),
        "speaker_wav": Description(
            en="Path to speaker WAV file for voice cloning",
            zh="用于声音克隆的说话人音频文件路径",
        ),
        "language": Description(
            en="Language code (e.g., en, zh)", zh="语言代码（如 en、zh）"
        ),
        "device": Description(
            en="Device to use (cuda, cpu, or empty for auto)",
            zh="使用的设备（cuda、cpu 或留空以自动选择）",
        ),
    }


class SherpaOnnxTTSConfig(I18nMixin):
    """Configuration for Sherpa Onnx TTS."""

    vits_model: str = Field(..., alias="vits_model")
    vits_lexicon: Optional[str] = Field(None, alias="vits_lexicon")
    vits_tokens: str = Field(..., alias="vits_tokens")
    vits_data_dir: Optional[str] = Field(None, alias="vits_data_dir")
    vits_dict_dir: Optional[str] = Field(None, alias="vits_dict_dir")
    tts_rule_fsts: Optional[str] = Field(None, alias="tts_rule_fsts")
    max_num_sentences: int = Field(2, alias="max_num_sentences")
    sid: int = Field(1, alias="sid")
    provider: Literal["cpu", "cuda", "coreml"] = Field("cpu", alias="provider")
    num_threads: int = Field(1, alias="num_threads")
    speed: float = Field(1.0, alias="speed")
    debug: bool = Field(False, alias="debug")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "vits_model": Description(en="Path to VITS model file", zh="VITS 模型文件路径"),
        "vits_lexicon": Description(
            en="Path to lexicon file (optional)", zh="词典文件路径（可选）"
        ),
        "vits_tokens": Description(en="Path to tokens file", zh="词元文件路径"),
        "vits_data_dir": Description(
            en="Path to espeak-ng data directory (optional)",
            zh="espeak-ng 数据目录路径（可选）",
        ),
        "vits_dict_dir": Description(
            en="Path to Jieba dictionary directory (optional)",
            zh="结巴词典目录路径（可选）",
        ),
        "tts_rule_fsts": Description(
            en="Path to rule FSTs file (optional)", zh="规则 FST 文件路径（可选）"
        ),
        "max_num_sentences": Description(
            en="Maximum number of sentences per batch", zh="每批次最大句子数"
        ),
        "sid": Description(
            en="Speaker ID for multi-speaker models", zh="多说话人模型的说话人 ID"
        ),
        "provider": Description(
            en="Computation provider (cpu, cuda, or coreml)",
            zh="计算提供者（cpu、cuda 或 coreml）",
        ),
        "num_threads": Description(en="Number of computation threads", zh="计算线程数"),
        "speed": Description(en="Speech speed multiplier", zh="语速倍数"),
        "debug": Description(en="Enable debug mode", zh="启用调试模式"),
    }


class SiliconFlowTTSConfig(I18nMixin):
    """Configuration for SiliconFlow TTS."""

    api_url: str = Field("https://api.siliconflow.cn/v1/audio/speech", alias="api_url")
    api_key: str = Field(..., alias="api_key")
    default_model: str = Field("FunAudioLLM/CosyVoice2-0.5B", alias="default_model")
    default_voice: str = Field(
        "speech:Dreamflowers:5bdstvc39i:xkqldnpasqmoqbakubom", alias="default_voice"
    )
    sample_rate: int = Field(32000, alias="sample_rate")
    response_format: str = Field("mp3", alias="response_format")
    stream: bool = Field(True, alias="stream")
    speed: float = Field(1, alias="speed")
    gain: int = Field(0, alias="gain")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "api_key": Description(
            en="API key for SiliconFlow TTS service",
            zh="SiliconFlow TTS 服务的 API 密钥",
        ),
        "url": Description(
            en="API endpoint URL for SiliconFlow TTS",
            zh="SiliconFlow TTS 的 API 端点 URL",
        ),
        "model": Description(
            en="Model to use for SiliconFlow TTS", zh="SiliconFlow TTS 使用的模型"
        ),
        "voice": Description(
            en="Voice name to use for SiliconFlow TTS",
            zh="SiliconFlow TTS 使用的语音名称",
        ),
        "sample_rate": Description(
            en="Sample rate of the output audio", zh="输出音频的采样率"
        ),
        "stream": Description(en="Enable streaming mode", zh="启用流式模式"),
        "speed": Description(en="Speaking speed multiplier", zh="语速倍数"),
        "gain": Description(en="Audio gain adjustment", zh="音频增益调整"),
    }


class OpenAITTSConfig(I18nMixin):
    """Configuration for OpenAI-compatible TTS client."""

    model: Optional[str] = Field(None, alias="model")
    voice: Optional[str] = Field(None, alias="voice")
    api_key: Optional[str] = Field(None, alias="api_key")
    base_url: Optional[str] = Field(None, alias="base_url")
    file_extension: Literal["mp3", "wav"] = Field("mp3", alias="file_extension")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "model": Description(
            en="Model name for the TTS server (overrides default)",
            zh="TTS 服务器的模型名称（覆盖默认值）",
        ),
        "voice": Description(
            en="Voice name(s) for the TTS server (overrides default)",
            zh="TTS 服务器的语音名称（覆盖默认值）",
        ),
        "api_key": Description(
            en="API key if required by the TTS server (overrides default)",
            zh="TTS 服务器所需的 API 密钥（覆盖默认值）",
        ),
        "base_url": Description(
            en="Base URL of the TTS server (overrides default)",
            zh="TTS 服务器的基础 URL（覆盖默认值）",
        ),
        "file_extension": Description(
            en="Audio file format (mp3 or wav, defaults to mp3)",
            zh="音频文件格式（mp3 或 wav，默认为 mp3）",
        ),
    }


class SparkTTSConfig(I18nMixin):
    """Configuration for Spark TTS."""

    api_url: str = Field(..., alias="api_url")
    prompt_wav_upload: str = Field(..., alias="prompt_wav_upload")
    api_name: str = Field(..., alias="api_name")
    gender: str = Field(..., alias="gender")
    pitch: int = Field(..., alias="pitch")
    speed: int = Field(..., alias="speed")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "prompt_wav_upload": Description(
            en="Reference audio (used when using voice cloning)",
            zh="参考音频（使用语音克隆时候使用）",
        ),
        "api_url": Description(
            en="API address of the spark tts gradio web frontend. For example: http://127.0.0.1:7860/voice_clone",
            zh="你的API地址。举例：http://127.0.0.1:7860/voice_clone",
        ),
        "api_name": Description(
            en="The API endpoint name. For example: voice_clone,voice_creation",
            zh="你的API名称。举例：voice_clone，voice_creation",
        ),
        "gender": Description(
            en="Gender of the voice (male or female)", zh="声音性别（男或女）"
        ),
        "pitch": Description(
            en="Pitch shift (in semitones) default 3,range 1-5.",
            zh="音高（以半音为单位）默认3，范围1-5",
        ),
        "speed": Description(
            en="Speed of the voice (in percent) default 3,range 1-5.",
            zh="声音速度（以百分比为单位）默认3，范围1-5",
        ),
    }


class MinimaxTTSConfig(I18nMixin):
    """Configuration for Minimax TTS."""

    group_id: str = Field(..., alias="group_id")
    api_key: str = Field(..., alias="api_key")
    model: str = Field("speech-02-turbo", alias="model")
    voice_id: str = Field("male-qn-qingse", alias="voice_id")
    pronunciation_dict: str = Field("", alias="pronunciation_dict")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "group_id": Description(en="Minimax group_id", zh="Minimax 的 group_id"),
        "api_key": Description(en="Minimax API key", zh="Minimax 的 API key"),
        "model": Description(en="Minimax model name", zh="Minimax 模型名称"),
        "voice_id": Description(en="Minimax voice id", zh="Minimax 语音 id"),
        "pronunciation_dict": Description(
            en="Custom pronunciation dictionary (string)", zh="自定义发音字典（字符串）"
        ),
    }


class PiperTTSConfig(I18nMixin):
    """Configuration for Piper TTS."""

    model_path: str = Field("models/piper/zh_CN-huayan-medium.onnx", alias="model_path")
    speaker_id: int = Field(0, alias="speaker_id")
    length_scale: float = Field(1.0, alias="length_scale")
    noise_scale: float = Field(0.667, alias="noise_scale")
    noise_w: float = Field(0.8, alias="noise_w")
    volume: float = Field(1.0, alias="volume")
    normalize_audio: bool = Field(True, alias="normalize_audio")
    use_cuda: bool = Field(False, alias="use_cuda")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "model_path": Description(
            en="Path to Piper ONNX model file",
            zh="Piper ONNX 模型文件路径",
        ),
        "speaker_id": Description(
            en="Speaker ID for multi-speaker models (default 0 for single-speaker)",
            zh="多说话人模型的说话人 ID（单说话人模型默认为 0）",
        ),
        "length_scale": Description(
            en="Speech speed control (0.5=2x faster, 1.0=normal, 2.0=2x slower)",
            zh="语速控制（0.5=快一倍，1.0=正常，2.0=慢一倍）",
        ),
        "noise_scale": Description(
            en="Audio variation degree (0.0-1.0, higher=more varied)",
            zh="音频变化程度（0.0-1.0，越高音频越丰富多变）",
        ),
        "noise_w": Description(
            en="Speaking style variation (0.0-1.0, higher=more diverse)",
            zh="说话风格变化（0.0-1.0，越高说话风格越多样）",
        ),
        "volume": Description(
            en="Output volume (0.0-1.0, 1.0=normal)", zh="音量（0.0-1.0，1.0=正常音量）"
        ),
        "normalize_audio": Description(
            en="Whether to normalize audio output (recommended)",
            zh="是否归一化音频输出（推荐启用）",
        ),
        "use_cuda": Description(
            en="Whether to use GPU acceleration (requires onnxruntime-gpu)",
            zh="是否使用 GPU 加速（需要安装 onnxruntime-gpu）",
        ),
    }


class Pyttsx3TTSConfig(I18nMixin):
    """Configuration for pyttsx3 local TTS (no required parameters)."""

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {}


class ElevenLabsTTSConfig(I18nMixin):
    """Configuration for ElevenLabs TTS."""

    api_key: str = Field(..., alias="api_key")
    voice_id: str = Field(..., alias="voice_id")
    model_id: str = Field("eleven_multilingual_v2", alias="model_id")
    output_format: str = Field("mp3_44100_128", alias="output_format")
    stability: float = Field(0.5, alias="stability")
    similarity_boost: float = Field(0.5, alias="similarity_boost")
    style: float = Field(0.0, alias="style")
    use_speaker_boost: bool = Field(True, alias="use_speaker_boost")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "api_key": Description(
            en="API key for ElevenLabs TTS service", zh="ElevenLabs TTS 服务的 API 密钥"
        ),
        "voice_id": Description(
            en="Voice ID from ElevenLabs (e.g., JBFqnCBsd6RMkjVDRZzb)",
            zh="来自 ElevenLabs 的语音 ID（如 JBFqnCBsd6RMkjVDRZzb）",
        ),
        "model_id": Description(
            en="Model ID for ElevenLabs (e.g., eleven_multilingual_v2)",
            zh="ElevenLabs 模型 ID（如 eleven_multilingual_v2）",
        ),
        "output_format": Description(
            en="Output audio format (e.g., mp3_44100_128)",
            zh="输出音频格式（如 mp3_44100_128）",
        ),
        "stability": Description(
            en="Voice stability (0.0 to 1.0)", zh="语音稳定性（0.0 到 1.0）"
        ),
        "similarity_boost": Description(
            en="Voice similarity boost (0.0 to 1.0)", zh="语音相似度增强（0.0 到 1.0）"
        ),
        "style": Description(
            en="Voice style exaggeration (0.0 to 1.0)",
            zh="语音风格夸张度（0.0 到 1.0）",
        ),
        "use_speaker_boost": Description(
            en="Enable speaker boost for better quality",
            zh="启用说话人增强以获得更好的质量",
        ),
    }


class CartesiaTTSConfig(I18nMixin):
    """Configuration for Cartesia TTS."""

    model_id: Literal[
        "sonic-3", "sonic-2", "sonic-turbo", "sonic-multilingual", "sonic"
    ] = Field("sonic-3", alias="model_id")

    api_key: str = Field(..., alias="api_key")
    voice_id: str = Field(..., alias="voice_id")
    output_format: Literal["wav", "mp3"] = Field("wav", alias="output_format")
    language: CartesiaLanguages = Field("en", alias="language")
    emotion: CartesiaEmotions = Field("neutral", alias="emotion")
    volume: float = Field(1.0, alias="volume")
    speed: float = Field(1.0, alias="speed")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "api_key": Description(
            en="API key for Cartesia TTS service", zh="Cartesia TTS 服务的 API 密钥"
        ),
        "voice_id": Description(
            en="Voice ID from Cartesia (e.g., 6ccbfb76-1fc6-48f7-b71d-91ac6298247b)",
            zh="来自 Cartesia 的语音 ID（如 6ccbfb76-1fc6-48f7-b71d-91ac6298247b）",
        ),
        "model_id": Description(
            en="Model ID for Cartesia (e.g., sonic-3)",
            zh="Cartesia 模型 ID（如 sonic-3）",
        ),
        "output_format": Description(
            en="Output audio format (e.g., wav)",
            zh="输出音频格式（如 wav）",
        ),
        "language": Description(
            en="The language that the given voice should speak (e.g., en)",
            zh="给定语音应使用的语言（如 en）",
        ),
        "emotion": Description(
            en="Emotional guidance for a generation (e.g., neutral)",
            zh="生成的情感指导（如 neutral）",
        ),
        "volume": Description(
            en="volume of the generation, ranging from 0.5 to 2.0 (e.g., 1)",
            zh="生成的音量，范围从 0.5 到 2.0（如 1）",
        ),
        "speed": Description(
            en="Speed of the generation, ranging from 0.6 to 1.5 (e.g., 1)",
            zh="生成的速度，范围从 0.6 到 1.5（如 1）",
        ),
    }


class TTSConfig(I18nMixin):
    """Configuration for Text-to-Speech."""

    tts_model: Literal[
        "azure_tts",
        "bark_tts",
        "edge_tts",
        "cosyvoice_tts",
        "cosyvoice2_tts",
        "cosyvoice_cloud",
        "qwen_tts",
        "melo_tts",
        "coqui_tts",
        "x_tts",
        "gpt_sovits_tts",
        "fish_api_tts",
        "sherpa_onnx_tts",
        "siliconflow_tts",
        "openai_tts",  # Add openai_tts here
        "spark_tts",
        "minimax_tts",
        "elevenlabs_tts",
        "cartesia_tts",
        "piper_tts",
        "pyttsx3_tts",
    ] = Field(..., alias="tts_model")

    # P2-3 (spoq-fix): 段级 TTS 合成并发上限（TTSTaskManager 信号灯），默认 2，≥1
    concurrency_limit: int = Field(2, alias="concurrency_limit", ge=1)

    # 口型驱动模式：rms=快速音量驱动（默认，NEKO 同款）/ off=完全关闭 viseme / pymouth=精细元音识别（慢）
    viseme_mode: Literal["off", "rms", "pymouth"] = Field(
        "rms", alias="viseme_mode"
    )

    azure_tts: Optional[AzureTTSConfig] = Field(None, alias="azure_tts")
    bark_tts: Optional[BarkTTSConfig] = Field(None, alias="bark_tts")
    edge_tts: Optional[EdgeTTSConfig] = Field(None, alias="edge_tts")
    cosyvoice_tts: Optional[CosyvoiceTTSConfig] = Field(None, alias="cosyvoice_tts")
    cosyvoice2_tts: Optional[Cosyvoice2TTSConfig] = Field(None, alias="cosyvoice2_tts")
    cosyvoice_cloud: Optional[CosyVoiceCloudTtsConfig] = Field(
        None, alias="cosyvoice_cloud"
    )
    qwen_tts: Optional[QwenTTSConfig] = Field(None, alias="qwen_tts")
    melo_tts: Optional[MeloTTSConfig] = Field(None, alias="melo_tts")
    coqui_tts: Optional[CoquiTTSConfig] = Field(None, alias="coqui_tts")
    x_tts: Optional[XTTSConfig] = Field(None, alias="x_tts")
    gpt_sovits_tts: Optional[GPTSoVITSConfig] = Field(None, alias="gpt_sovits_tts")
    fish_api_tts: Optional[FishAPITTSConfig] = Field(None, alias="fish_api_tts")
    sherpa_onnx_tts: Optional[SherpaOnnxTTSConfig] = Field(
        None, alias="sherpa_onnx_tts"
    )
    siliconflow_tts: Optional[SiliconFlowTTSConfig] = Field(
        None, alias="siliconflow_tts"
    )
    openai_tts: Optional[OpenAITTSConfig] = Field(None, alias="openai_tts")
    spark_tts: Optional[SparkTTSConfig] = Field(None, alias="spark_tts")
    minimax_tts: Optional[MinimaxTTSConfig] = Field(None, alias="minimax_tts")
    elevenlabs_tts: ElevenLabsTTSConfig | None = Field(None, alias="elevenlabs_tts")
    cartesia_tts: CartesiaTTSConfig | None = Field(None, alias="cartesia_tts")
    piper_tts: Optional[PiperTTSConfig] = Field(None, alias="piper_tts")
    pyttsx3_tts: Optional[Pyttsx3TTSConfig] = Field(None, alias="pyttsx3_tts")

    DESCRIPTIONS: ClassVar[Dict[str, Description]] = {
        "tts_model": Description(
            en="Text-to-speech model to use", zh="要使用的文本转语音模型"
        ),
        "viseme_mode": Description(
            en="Lip-sync mode: rms (fast, default) / off / pymouth (slow, precise)",
            zh="口型模式：rms（快速，默认）/ off（关闭）/ pymouth（精细但慢）",
        ),
        "azure_tts": Description(en="Configuration for Azure TTS", zh="Azure TTS 配置"),
        "bark_tts": Description(en="Configuration for Bark TTS", zh="Bark TTS 配置"),
        "edge_tts": Description(en="Configuration for Edge TTS", zh="Edge TTS 配置"),
        "cosyvoice_tts": Description(
            en="Configuration for Cosyvoice TTS", zh="Cosyvoice TTS 配置"
        ),
        "cosyvoice2_tts": Description(
            en="Configuration for Cosyvoice2 TTS", zh="Cosyvoice2 TTS 配置"
        ),
        "cosyvoice_cloud": Description(
            en="Configuration for CosyVoice Cloud TTS (DashScope)",
            zh="CosyVoice 云端 TTS 配置（DashScope）",
        ),
        "qwen_tts": Description(
            en="Configuration for Qwen TTS (DashScope)",
            zh="Qwen TTS 配置（DashScope）",
        ),
        "melo_tts": Description(en="Configuration for Melo TTS", zh="Melo TTS 配置"),
        "coqui_tts": Description(en="Configuration for Coqui TTS", zh="Coqui TTS 配置"),
        "x_tts": Description(en="Configuration for XTTS", zh="XTTS 配置"),
        "gpt_sovits_tts": Description(
            en="Configuration for GPT-SoVITS", zh="GPT-SoVITS 配置"
        ),
        "fish_api_tts": Description(
            en="Configuration for Fish API TTS", zh="Fish API TTS 配置"
        ),
        "sherpa_onnx_tts": Description(
            en="Configuration for Sherpa Onnx TTS", zh="Sherpa Onnx TTS 配置"
        ),
        "siliconflow_tts": Description(
            en="Configuration for SiliconFlow TTS", zh="SiliconFlow TTS 配置"
        ),
        "openai_tts": Description(
            en="Configuration for OpenAI-compatible TTS", zh="OpenAI 兼容 TTS 配置"
        ),
        "spark_tts": Description(en="Configuration for Spark TTS", zh="Spark TTS 配置"),
        "minimax_tts": Description(
            en="Configuration for Minimax TTS", zh="Minimax TTS 配置"
        ),
        "elevenlabs_tts": Description(
            en="Configuration for ElevenLabs TTS", zh="ElevenLabs TTS 配置"
        ),
        "cartesia_tts": Description(
            en="Configuration for Cartesia TTS", zh="Cartesia TTS 配置"
        ),
        "piper_tts": Description(en="Configuration for Piper TTS", zh="Piper TTS 配置"),
        "pyttsx3_tts": Description(en="Configuration for pyttsx3 TTS", zh="pyttsx3 TTS 配置"),
    }

    @model_validator(mode="after")
    def check_tts_config(cls, values: "TTSConfig", info: ValidationInfo):
        tts_model = values.tts_model

        # Only validate the selected TTS model
        if tts_model == "azure_tts" and values.azure_tts is not None:
            values.azure_tts.model_validate(values.azure_tts.model_dump())
        elif tts_model == "bark_tts" and values.bark_tts is not None:
            values.bark_tts.model_validate(values.bark_tts.model_dump())
        elif tts_model == "edge_tts" and values.edge_tts is not None:
            values.edge_tts.model_validate(values.edge_tts.model_dump())
        elif tts_model == "cosyvoice_tts" and values.cosyvoice_tts is not None:
            values.cosyvoice_tts.model_validate(values.cosyvoice_tts.model_dump())
        elif tts_model == "cosyvoice2_tts" and values.cosyvoice2_tts is not None:
            values.cosyvoice2_tts.model_validate(values.cosyvoice2_tts.model_dump())
        elif tts_model == "cosyvoice_cloud" and values.cosyvoice_cloud is not None:
            values.cosyvoice_cloud.model_validate(values.cosyvoice_cloud.model_dump())
        elif tts_model == "qwen_tts" and values.qwen_tts is not None:
            values.qwen_tts.model_validate(values.qwen_tts.model_dump())
        elif tts_model == "melo_tts" and values.melo_tts is not None:
            values.melo_tts.model_validate(values.melo_tts.model_dump())
        elif tts_model == "coqui_tts" and values.coqui_tts is not None:
            values.coqui_tts.model_validate(values.coqui_tts.model_dump())
        elif tts_model == "x_tts" and values.x_tts is not None:
            values.x_tts.model_validate(values.x_tts.model_dump())
        elif tts_model == "gpt_sovits_tts" and values.gpt_sovits_tts is not None:
            values.gpt_sovits_tts.model_validate(values.gpt_sovits_tts.model_dump())
        elif tts_model == "fish_api_tts" and values.fish_api_tts is not None:
            values.fish_api_tts.model_validate(values.fish_api_tts.model_dump())
        elif tts_model == "sherpa_onnx_tts" and values.sherpa_onnx_tts is not None:
            values.sherpa_onnx_tts.model_validate(values.sherpa_onnx_tts.model_dump())
        elif tts_model == "siliconflow_tts" and values.siliconflow_tts is not None:
            values.siliconflow_tts.model_validate(values.siliconflow_tts.model_dump())
        elif tts_model == "openai_tts" and values.openai_tts is not None:
            values.openai_tts.model_validate(values.openai_tts.model_dump())
        elif tts_model == "spark_tts" and values.spark_tts is not None:
            values.spark_tts.model_validate(values.spark_tts.model_dump())
        elif tts_model == "minimax_tts" and values.minimax_tts is not None:
            values.minimax_tts.model_validate(values.minimax_tts.model_dump())
        elif tts_model == "elevenlabs_tts" and values.elevenlabs_tts is not None:
            values.elevenlabs_tts.model_validate(values.elevenlabs_tts.model_dump())
        elif tts_model == "cartesia_tts" and values.cartesia_tts is not None:
            values.cartesia_tts.model_validate(values.cartesia_tts.model_dump())

        elif tts_model == "piper_tts" and values.piper_tts is not None:
            values.piper_tts.model_validate(values.piper_tts.model_dump())
        elif tts_model == "pyttsx3_tts" and values.pyttsx3_tts is not None:
            values.pyttsx3_tts.model_validate(values.pyttsx3_tts.model_dump())
        return values
