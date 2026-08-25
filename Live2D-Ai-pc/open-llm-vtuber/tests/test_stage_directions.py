"""括注舞台说明：TTS/展示过滤 + 导演半身动作映射测试。"""
from open_llm_vtuber.director import stage_gesture
from open_llm_vtuber.utils.tts_preprocessor import filter_parentheses


def test_filter_fullwidth_parentheses_from_tts():
    text = "（看到热字，轻轻用小手扇风）哎呀～是不是天气太热啦？"
    assert filter_parentheses(text) == "哎呀～是不是天气太热啦？"


def test_filter_ascii_parentheses_from_tts():
    text = "(nods) Hello there!"
    assert filter_parentheses(text) == "Hello there!"


def test_stage_gesture_fan_maps_to_sway():
    assert stage_gesture("（看到热字，轻轻用小手扇风）哎呀") == "sway"


def test_stage_gesture_nod_maps_to_nod():
    assert stage_gesture("（轻轻点头）好的呢") == "nod"


def test_stage_gesture_returns_none_without_parens():
    assert stage_gesture("哎呀，天气好热啊") is None


def test_stage_gesture_choreography_keywords():
    """大幅度编排动作：新关键词映射到 choreography 库（前端按名自动路由）。"""
    # bai 无手臂参数：挥手语义映射到安全的上身「开心」摆动近似，不再产出 wave_big。
    assert stage_gesture("（开心地挥手）你好呀") == "happy"
    assert stage_gesture("（深深鞠躬）感谢大家") == "bow"
    assert stage_gesture("（用力摇头）不行哦") == "shake_no"
    assert stage_gesture("（凑近屏幕）让我看看") == "lean_in"
    assert stage_gesture("（左右张望）有人吗") == "look_around"


def test_stage_gesture_wave_maps_to_safe_happy():
    # 挥手/挥手打招呼/用力挥手 → happy（上身开心摆动；模型无手臂，不上真实挥手）。
    assert stage_gesture("（挥手）你好呀") == "happy"
    assert stage_gesture("（用力挥手）再见") == "happy"


def test_stage_gesture_bounce_maps_to_safe_excited():
    # 蹦跳属全身动作（与半身参数驱动冲突），映射到安全的上身「兴奋」替代，
    # 不再产出 bounce_happy。
    assert stage_gesture("（蹦蹦跳跳）太棒啦") == "excited"
    assert stage_gesture("（高兴得跳起来）太棒啦") == "excited"
