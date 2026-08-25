"""Focused unit tests for melo_worker.

These tests import melo_worker directly without triggering Melo/TTS model
loading. They only exercise pure helper functions.
"""
import pytest

from open_llm_vtuber.tts import melo_worker


@pytest.mark.parametrize(
    ("text", "expected"),
    [
        ("", 0),
        ("plain text", 0),
        ("hello world", 0),
        ("你好，世界", 0),
        ("Hello world.", 1),
        ("你好。世界！", 2),
        ("真的？好吧。", 2),
        ("……", 1),
        ("......", 1),
        ("..!!??", 1),
        ("?!。", 1),
        ("A! B? C.", 3),
        ("A!!! B???", 2),
        ("第一。第二！第三？", 3),
        ("Line one\nLine two", 1),
        ("Line one\n\nLine two", 1),
        ("End.\nNext!", 2),
    ],
)
def test_count_sentences(text, expected):
    assert melo_worker._count_sentences(text) == expected


def test_count_sentences_none_treated_as_empty():
    assert melo_worker._count_sentences(None) == 0
