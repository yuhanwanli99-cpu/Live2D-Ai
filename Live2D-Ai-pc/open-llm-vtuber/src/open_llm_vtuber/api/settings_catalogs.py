"""Static provider and model catalogs used by the settings dashboard."""

# Provider -> 默认 base_url / 环境变量名
_LLM_DEFAULTS = {
    "deepseek_llm": {
        "name": "DeepSeek",
        "base_url": "https://api.deepseek.com/v1",
        "key_env": "DEEPSEEK_API_KEY",
    },
    "zhipu_llm": {
        "name": "智谱 GLM",
        "base_url": "https://open.bigmodel.cn/api/paas/v4",
        "key_env": "ZHIPU_API_KEY",
    },
    "qwen_llm": {
        "name": "Qwen",
        "base_url": "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "key_env": "DASHSCOPE_API_KEY",
    },
    "openrouter": {
        "name": "OpenRouter",
        "base_url": "https://openrouter.ai/api/v1",
        "key_env": "OPENROUTER_API_KEY",
    },
    "openai": {
        "name": "OpenAI",
        "base_url": "https://api.openai.com/v1",
        "key_env": "OPENAI_API_KEY",
    },
    "anthropic": {
        "name": "Anthropic",
        "base_url": "https://api.anthropic.com/v1",
        "key_env": "ANTHROPIC_API_KEY",
    },
    "moonshot": {
        "name": "Moonshot",
        "base_url": "https://api.moonshot.cn/v1",
        "key_env": "MOONSHOT_API_KEY",
    },
    "groq": {
        "name": "Groq",
        "base_url": "https://api.groq.com/openai/v1",
        "key_env": "GROQ_API_KEY",
    },
    "mistral": {
        "name": "Mistral",
        "base_url": "https://api.mistral.ai/v1",
        "key_env": "MISTRAL_API_KEY",
    },
    "together": {
        "name": "Together AI",
        "base_url": "https://api.together.xyz/v1",
        "key_env": "TOGETHER_API_KEY",
    },
    "fireworks": {
        "name": "Fireworks AI",
        "base_url": "https://api.fireworks.ai/inference/v1",
        "key_env": "FIREWORKS_API_KEY",
    },
    "siliconflow": {
        "name": "SiliconFlow",
        "base_url": "https://api.siliconflow.cn/v1",
        "key_env": "SILICONFLOW_API_KEY",
    },
    "opencode": {
        "name": "OpenCode",
        "base_url": "",
        "key_env": "OPENCODE_API_KEY",
    },
    "opencode_go": {
        "name": "OpenCode Go",
        "base_url": "",
        "key_env": "OPENCODE_GO_API_KEY",
    },
    "openai_compatible_llm": {
        "name": "OpenAI 兼容",
        "base_url": "",
        "key_env": "OPENAI_API_KEY",
    },
}
_MODEL_CATALOG = {
    "deepseek_llm": ["deepseek-v4-flash", "deepseek-v4-pro"],
    "zhipu_llm": ["glm-4.7-flash", "glm-4.6v"],
    "qwen_llm": ["qwen3.7-flash", "qwen3.7-plus", "qwen-max"],
    "openrouter": [
        "openrouter/auto",
        "deepseek/deepseek-chat",
        "qwen/qwen3-7b",
    ],
    "openai": ["gpt-5", "gpt-4o", "gpt-4o-mini"],
    "anthropic": [
        "claude-sonnet-4-5",
        "claude-opus-4-1",
        "claude-haiku-4-5",
    ],
    "moonshot": ["moonshot-v1-8k", "moonshot-v1-32k", "moonshot-v1-128k"],
    "groq": [
        "llama-3.3-70b-versatile",
        "llama-3.1-8b-instant",
    ],
    "mistral": ["mistral-large-latest", "mistral-small-latest"],
    "together": [
        "meta-llama/Llama-3.3-70B-Instruct-Turbo",
        "mistralai/Mixtral-8x7B-Instruct-v0.1",
    ],
    "fireworks": [
        "accounts/fireworks/models/llama-v3p3-70b-instruct",
        "accounts/fireworks/models/qwen2p5-coder-32b-instruct",
    ],
    "siliconflow": [
        "deepseek-ai/DeepSeek-V3",
        "Qwen/Qwen2.5-7B-Instruct",
    ],
    "opencode": [],
    "opencode_go": [],
    "openai_compatible_llm": [],
}
_TTS_VOICE_CATALOG = {
    "edge_tts": [
        "zh-CN-XiaoxiaoNeural", "zh-CN-XiaoyiNeural", "zh-CN-YunjianNeural",
        "zh-CN-YunxiNeural", "zh-CN-YunxiaNeural", "zh-CN-liaoning-XiaobeiNeural",
        "zh-CN-shaanxi-XiaoniNeural", "en-US-JennyNeural", "en-US-GuyNeural",
        "ja-JP-NanamiNeural", "ko-KR-SunHiNeural",
    ],
    "cosyvoice_cloud": [
        "longxiaochun", "longxiaoxia", "longshu", "longcheng",
        "longhua", "longyu", "longmei", "longyi",
    ],
    "qwen_tts": ["Cherry", "Ethan", "Serena", "Charlotte"],
    "openai_tts": ["alloy", "echo", "fable", "onyx", "nova", "shimmer"],
    "azure_tts": [
        "zh-CN-XiaoxiaoNeural", "zh-CN-XiaoyiNeural", "zh-CN-YunxiNeural",
        "zh-CN-YunjianNeural", "en-US-JennyNeural", "ja-JP-NanamiNeural",
    ],
    "minimax_tts": ["male-qn-qingse", "female-shaonv", "female-tianmei", "male-jingdian"],
    "siliconflow_tts": ["FunAudioLLM/CosyVoice2-0.5B", "iic/SpeechT5", "Qwen/Qwen2-Audio"],
    "fish_api_tts": ["fish-speech-1.5"],
    "spark_tts": ["xiaoyan", "aisjouxu", "aisjiushi", "aisjinger"],
    "gpt_sovits_tts": [],
    "piper_tts": [],
    "sherpa_onnx_tts": [],
    "pyttsx3_tts": [],
    "melo_tts": ["ZH", "EN", "ES", "FR", "JP", "KR"],
}
_TTS_MODEL_CATALOG = {
    "gpt_sovits_tts": ["GPT-SoVITS"],
    "piper_tts": ["piper"],
    "sherpa_onnx_tts": ["sherpa-onnx"],
    "pyttsx3_tts": ["pyttsx3"],
    "melo_tts": ["cpu", "cuda", "auto"],
}

_TTS_DEFAULTS = {
    "cosyvoice_cloud": {"name": "CosyVoice", "key_env": "ALIYUN_API_KEY"},
    "qwen_tts": {"name": "Qwen TTS", "key_env": "DASHSCOPE_API_KEY"},
    "edge_tts": {"name": "Edge TTS", "key_env": None},
    "openai_tts": {"name": "OpenAI TTS", "key_env": "OPENAI_API_KEY"},
    "azure_tts": {"name": "Azure TTS", "key_env": "AZURE_SPEECH_KEY"},
    "minimax_tts": {"name": "MiniMax", "key_env": "MINIMAX_API_KEY"},
    "siliconflow_tts": {"name": "SiliconFlow TTS", "key_env": "SILICONFLOW_API_KEY"},
    "fish_api_tts": {"name": "Fish Audio", "key_env": "FISH_API_KEY"},
    "spark_tts": {"name": "Spark TTS", "key_env": "SPARK_API_KEY"},
    "gpt_sovits_tts": {"name": "GPT-SoVITS", "key_env": None},
    "piper_tts": {"name": "Piper", "key_env": None},
    "sherpa_onnx_tts": {"name": "Sherpa-ONNX", "key_env": None},
    "pyttsx3_tts": {"name": "pyttsx3", "key_env": None},
    "melo_tts": {"name": "Melo TTS（本地）", "key_env": None},
}
