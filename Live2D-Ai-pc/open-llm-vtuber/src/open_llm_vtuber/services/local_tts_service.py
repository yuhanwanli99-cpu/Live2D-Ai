import asyncio
import os
from uuid import uuid4

from .tts_routes_helpers import audio_duration_ms, make_local_tts_params, safe_local_tts_name


class LocalTTSService:
    """本地 Melo TTS 试听服务（缓存引擎、文件生成、音频下载）。"""

    def __init__(self, cache_dir: str, max_text_len: int, file_prefix: str):
        self.cache_dir = cache_dir
        self.max_text_len = max_text_len
        self.file_prefix = file_prefix
        self._engine = None
        self._engine_key = None
        self._lock = asyncio.Lock()

    async def _get_engine(self, params: dict):
        key = tuple(sorted(params.items()))
        async with self._lock:
            if self._engine is None or key != self._engine_key:
                from open_llm_vtuber.tts.tts_factory import TTSFactory

                self._engine = await asyncio.to_thread(
                    TTSFactory.get_tts_engine, "melo_tts", **params
                )
                self._engine_key = key
            return self._engine

    async def synthesize(self, payload) -> dict:
        if not isinstance(payload, dict):
            raise ValueError("请求体必须是 JSON 对象")
        text = str(payload.get("text") or "").strip()
        if not text:
            raise ValueError("text 不能为空")
        if len(text) > self.max_text_len:
            raise ValueError(f"文本长度超过上限 {self.max_text_len} 字符")

        params = make_local_tts_params(payload)
        engine = await self._get_engine(params)
        file_stem = f"{self.file_prefix}{uuid4().hex[:12]}"
        async with self._lock:
            audio_file = await engine.async_generate_audio(
                text=text, file_name_no_ext=file_stem
            )
        if not audio_file:
            raise RuntimeError("Melo 合成失败：引擎未返回音频文件")

        abs_path = audio_file if os.path.isabs(audio_file) else os.path.abspath(audio_file)
        safe_name = safe_local_tts_name(os.path.basename(abs_path), self.file_prefix)
        if safe_name is None or not os.path.isfile(abs_path):
            raise RuntimeError("Melo 合成失败：音频文件未生成")

        return {
            "ok": True,
            "audio_path": f"cache/{safe_name}",
            "audio_url": f"/api/tts/local/audio?name={safe_name}",
            "format": params["format"],
            "duration_ms": audio_duration_ms(abs_path),
        }

    def audio_path(self, name: str) -> str:
        safe_name = safe_local_tts_name(name, self.file_prefix)
        if safe_name is None:
            raise ValueError("非法音频文件名")
        audio_path = os.path.abspath(os.path.join(self.cache_dir, safe_name))
        if os.path.dirname(audio_path) != os.path.abspath(self.cache_dir):
            raise ValueError("非法音频路径")
        if not os.path.isfile(audio_path):
            raise FileNotFoundError("音频不存在")
        return audio_path
