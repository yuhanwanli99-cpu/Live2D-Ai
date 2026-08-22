"""Translate interface stub.
This module provides a minimal translation interface for the TTS preprocessor.
Actual translation providers (DeepLX, Tencent) can be plugged in later.
"""

from abc import ABC, abstractmethod


class TranslateInterface(ABC):
    """Abstract interface for translation providers."""

    @abstractmethod
    def translate(self, text: str) -> str:
        """Translate text from source language to target language."""
        ...
