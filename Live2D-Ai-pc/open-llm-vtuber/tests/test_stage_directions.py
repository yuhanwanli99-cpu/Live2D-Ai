"""括注舞台说明：TTS/展示过滤 + 导演半身动作映射测试。

定版：括注只认识六个 Bai 可诚实表达的动作
（点头/摇头/歪头/张望/倾听/惊讶 及近义）；挥手/鼓掌/蹦跳/鞠躬/凑近/摇摆
等关键词一律不匹配（返回 None），不再产出任何近似冒充动作。
"""
from open_llm_vtuber.director import stage_gesture
from open_llm_vtuber.utils.tts_preprocessor import filter_parentheses


def test_filter_fullwidth_parentheses_from_tts():
    text = "（看到热字，轻轻用小手扇风）哎呀～是不是天气太热啦？"
    assert filter_parentheses(text) == "哎呀～是不是天气太热啦？"


def test_filter_ascii_parentheses_from_tts():
    text = "(nods) Hello there!"
    assert filter_parentheses(text) == "Hello there!"


def test_stage_gesture_fan_returns_none():
    # 扇风/摇摆已删：不再映射到 sway。
    assert stage_gesture("（看到热字，轻轻用小手扇风）哎呀") is None
    assert stage_gesture("（轻轻摇摆）来啦") is None


def test_stage_gesture_nod_maps_to_nod():
    assert stage_gesture("（轻轻点头）好的呢") == "nod"


def test_stage_gesture_returns_none_without_parens():
    assert stage_gesture("哎呀，天气好热啊") is None


def test_stage_gesture_six_action_keywords():
    """只保留六个可诚实表达动作的近义词；已删语义一律 None。"""
    assert stage_gesture("（轻轻点头）好的呢") == "nod"
    assert stage_gesture("（用力摇头）不行哦") == "shake_no"
    assert stage_gesture("（歪了歪头）嗯？") == "tilt"
    assert stage_gesture("（左右张望）有人吗") == "look_around"
    assert stage_gesture("（歪着脑袋想）我想想") == "listen"
    assert stage_gesture("（震惊）天哪") == "surprise"
    # 已删除语义 → 一律 None
    assert stage_gesture("（开心地挥手）你好呀") is None
    assert stage_gesture("（深深鞠躬）感谢大家") is None
    assert stage_gesture("（凑近屏幕）让我看看") is None
    assert stage_gesture("（欢呼）太棒啦") is None
    assert stage_gesture("（鼓掌）太棒啦") is None
    assert stage_gesture("（蹦蹦跳跳）太棒啦") is None