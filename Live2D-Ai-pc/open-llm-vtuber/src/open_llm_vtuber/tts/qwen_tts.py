# -*- coding: utf-8 -*-
"""Qwen-TTS (DashScope) engine — non-realtime speech synthesis.

Uses the same DashScope key as CosyVoice. Model: qwen3-tts-flash.
"""
from __future__ import annotations

import json
import os

import requests
from loguru import logger

from .tts_interface import TTSInterface

DASHSCOPE_TTS_URL = (
    "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation"
)


class QwenTTSEngine(TTSInterface):
    def __init__(
        self,
        api_key: str = "",
        voice: str = "Cherry",
        model: str = "qwen3-tts-flash",
        language_type: str = "Auto",
    ) -> None:
        self.api_key = api_key
        self.voice = voice
        self.model = model
        self.language_type = language_type

    def generate_audio(self, text: str, file_name_no_ext: str | None = None) -> str:
        if not self.api_key:
            raise RuntimeError("Qwen TTS API Key 未配置")
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }
        payload = {
            "model": self.model,
            "input": {
                "text": text,
                "voice": self.voice,
                "language_type": self.language_type,
            },
        }
        logger.info(f"[QwenTTS] synthesizing text={text[:30]!r} voice={self.voice}")
        resp = requests.post(DASHSCOPE_TTS_URL, headers=headers, json=payload, timeout=60)
        if resp.status_code != 200:
            raise RuntimeError(f"Qwen TTS HTTP {resp.status_code}: {resp.text[:200]}")
        data = resp.json()
        audio_url = data.get("output", {}).get("audio", {}).get("url")
        if not audio_url:
            raise RuntimeError("Qwen TTS 响应缺少 audio.url")
        audio = requests.get(audio_url, timeout=60)
        if audio.status_code != 200:
            raise RuntimeError(f"Qwen TTS 音频下载失败: {audio.status_code}")
        out_path = self.generate_cache_file_name(file_name_no_ext, "wav")
        with open(out_path, "wb") as f:
            f.write(audio.content)
        logger.info(f"[QwenTTS] saved {out_path} ({len(audio.content)} bytes)")
        return out_path
