import os

try:
    from melo.api import TTS
except Exception as _import_err:  # noqa: BLE001 - 可选依赖导入失败降级外部 worker
    TTS = None
    _MELO_IMPORT_ERROR = _import_err
else:
    _MELO_IMPORT_ERROR = None

from .melo_external import MeloExternalClient
from .tts_interface import TTSInterface

# Melo 支持的语言（ZH 为实际中英混合模型，句子里夹英文也能念）
SUPPORTED_LANGUAGES = ("ZH", "EN", "ES", "FR", "JP", "KR")
SUPPORTED_DEVICES = ("cpu", "cuda", "auto")


def _clamp(value, lo, hi, default):
    """把数值钳制到 [lo, hi]；非法/缺失时回退 default。"""
    if value is None:
        return default
    try:
        v = float(value)
    except (TypeError, ValueError):
        return default
    return max(lo, min(hi, v))


class TTSEngine(TTSInterface):
    def __init__(
        self,
        language: str = "ZH",
        device: str = "cpu",
        speaker: str | None = None,
        speed: float = 1.0,
        sdp_ratio: float = 0.2,
        noise_scale: float = 0.6,
        noise_scale_w: float = 0.8,
        format: str = "wav",
    ):
        self.external = TTS is None
        self.speaker = speaker

        lang = (language or "ZH").upper()
        if lang not in SUPPORTED_LANGUAGES:
            raise ValueError(
                f"不支持的 Melo 语言: {language}，可选 {SUPPORTED_LANGUAGES}"
            )
        self.language = lang

        dev = (device or "cpu").lower()
        # 允许 cuda / cuda:0 / cuda:1 等，其余归一为 cpu 或 auto
        if dev not in ("cpu", "auto") and not dev.startswith("cuda"):
            dev = "cpu"
        self.device = dev

        # 四个韵律参数只影响节奏/语调，不影响音色
        self.speed = _clamp(speed, 0.5, 2.0, 1.0)
        self.sdp_ratio = _clamp(sdp_ratio, 0.0, 1.0, 0.2)
        self.noise_scale = _clamp(noise_scale, 0.0, 1.0, 0.6)
        self.noise_scale_w = _clamp(noise_scale_w, 0.0, 1.0, 0.8)

        fmt = (format or "wav").lower()
        if not fmt:
            fmt = "wav"
        self.format = fmt
        self.file_extension = self.format

        self.temp_audio_file = "temp"
        self.new_audio_dir = "cache"
        if not os.path.exists(self.new_audio_dir):
            os.makedirs(self.new_audio_dir)

        if self.external:
            # 进程内未安装 Melo 时，改用外部 conda/melo 环境的常驻 worker。
            self._client = MeloExternalClient.singleton()
            availability_error = self._client.availability_error()
            if availability_error:
                raise RuntimeError(
                    f"{availability_error}（本地 MeloTTS 也未安装：{_MELO_IMPORT_ERROR}）"
                )
            self.model = None
            self.speaker_id = None
            return

        # 与用户提供 API 对齐：TTS(language=..., device=...) + spk2id 解析
        self.model = TTS(language=self.language, device=self.device)
        data = getattr(getattr(self.model, "hps", None), "data", None)
        spk2id = dict(getattr(data, "spk2id", None) or {})
        if speaker and speaker in spk2id:
            self.speaker_id = spk2id[speaker]
        elif self.language in spk2id:
            self.speaker_id = spk2id[self.language]
        elif spk2id:
            self.speaker_id = next(iter(spk2id.values()))
        else:
            self.speaker_id = 0

    def generate_audio(self, text, file_name_no_ext=None):
        """
        生成语音音频文件（阻塞推理，基类 async_generate_audio 会包到线程池）。
        调用与本地 Melo API 对齐，传入四个韵律参数。
        """
        try:
            file_name = self.generate_cache_file_name(
                file_name_no_ext, self.file_extension
            )
            if self.external:
                self._client.synthesize(
                    text=text,
                    language=self.language,
                    device=self.device,
                    speaker=self.speaker,
                    speed=self.speed,
                    sdp_ratio=self.sdp_ratio,
                    noise_scale=self.noise_scale,
                    noise_scale_w=self.noise_scale_w,
                    format=self.format,
                    output_path=os.path.abspath(file_name),
                )
                return file_name
            self.model.tts_to_file(
                text=text,
                speaker_id=self.speaker_id,
                output_path=file_name,
                sdp_ratio=self.sdp_ratio,
                noise_scale=self.noise_scale,
                noise_scale_w=self.noise_scale_w,
                speed=self.speed,
                format=self.format,
            )
            return file_name
        except LookupError:
            # EN 场景首次需要下载 nltk 词性标注器
            import ssl

            import nltk

            try:
                _create_unverified_https_context = ssl._create_unverified_context
            except AttributeError:
                pass
            else:
                ssl._create_default_https_context = _create_unverified_https_context

            nltk.download("averaged_perceptron_tagger_eng")
            return self.generate_audio(text, file_name_no_ext)
