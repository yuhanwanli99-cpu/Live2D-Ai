"""split_tts_chunks 小节化单元测试。

覆盖：短句不切；长句按自然断点切块（单块 ≤ max_chars）；无断点按字数硬切；
空输入返回空列表。
"""
from open_llm_vtuber.conversations.conversation_utils import split_tts_chunks


def test_short_text_returns_single_chunk():
    assert split_tts_chunks("你好，这是短句。") == ["你好，这是短句。"]


def test_long_text_splits_at_punctuation():
    text = "这是第一句，比较长所以要切。第二句也很长，继续拆分。第三句收尾。"
    chunks = split_tts_chunks(text, max_chars=16)
    assert len(chunks) >= 3
    assert all(len(c) <= 16 for c in chunks)
    # 切块应保留标点结尾，拼回去和原文一致（忽略空白差异）
    assert "".join(chunks).replace(" ", "") == text.replace(" ", "")


def test_no_punctuation_hard_cut():
    text = "一二三四五六七八九十一二三四五六七八九十"
    chunks = split_tts_chunks(text, max_chars=8)
    assert all(len(c) <= 8 for c in chunks)
    assert "".join(chunks) == text


def test_empty_text_returns_empty():
    assert split_tts_chunks("") == []
    assert split_tts_chunks("   ") == []
