"""Abstract translation interface.

This module provides an abstract base class for translation providers
used by the TTS preprocessor to optionally translate text before synthesis.
"""

from abc import ABC, abstractmethod


class TranslateInterface(ABC):
    """Abstract interface for translation providers.

    Subclass this and implement translate() to add a new translation backend
    (e.g., DeepLX, Tencent Translate).
    """

    @abstractmethod
    def translate(self, text: str) -> str:
        """Translate text from the source language to the target language.

        Args:
            text: The text to translate.

        Returns:
            The translated text.
        """
        ...
