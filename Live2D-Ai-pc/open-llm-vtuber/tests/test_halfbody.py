from open_llm_vtuber.director import ExpressionDirector
from open_llm_vtuber.halfbody import (
    available_halfbody_channels,
    detect_halfbody_capabilities,
    sanitize_halfbody_intent,
)

PARAM_IDS = {
    "ParamAngleX", "ParamEyeBallX", "ParamBrowLY", "ParamBrowRY",
    "ParamBreath", "ParamBodyAngleZ",
}


def test_capability_detection_degrades_per_channel():
    caps = detect_halfbody_capabilities(PARAM_IDS)
    assert caps["headX"] is True
    assert caps["headY"] is False
    assert caps["gazeX"] is True
    assert caps["browY"] is True
    assert caps["bodyZ"] is True
    assert caps["bodyX"] is False
    assert available_halfbody_channels(PARAM_IDS) == (
        "headX", "gazeX", "browY", "breath", "bodyZ"
    )


def test_sanitize_filters_unknown_missing_and_mouth_channels():
    result = sanitize_halfbody_intent(
        {
            "headX": 2.4,
            "gazeX": -0.25,
            "headY": 0.6,
            "breath": -1,
            "mouthOpen": 1,
            "tear": 1,
            "bodyZ": "bad",
        },
        PARAM_IDS,
    )
    assert result == {"headX": 1.0, "gazeX": -0.25, "breath": 0.0}


class FakeLLM:
    async def chat_completion(self, messages, system=None):
        yield '{"emotion":"joy","strength":2,"halfBody":{"headX":0.4,"headY":0.9,"bodyZ":-0.5,"mouthOpen":1}}'


def test_director_filters_halfbody_by_current_model_profile():
    import asyncio

    director = ExpressionDirector(llm=FakeLLM(), parameter_ids=PARAM_IDS)
    result = asyncio.run(director.analyze("你好"))
    # Preview: LLM 只选择语义动作；连续 pose/gaze 输出一律忽略，防止参数粘住。
    assert result.half_body == {}
    assert result.to_dict()["halfBody"] == {}
