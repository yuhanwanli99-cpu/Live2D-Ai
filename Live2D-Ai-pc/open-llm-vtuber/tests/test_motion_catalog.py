"""PC Preview 头/脸安全动作目录 / 能力过滤 / 协议元数据 / 时间轴去重测试。

覆盖任务要求：
1) 目录 = 基础六动作（nod/shake_no/tilt/look_around/listen/surprise）
   + 能力门控扩展十一动作（look_up/look_down/glance_left/glance_right/
   wink_left/wink_right/smile/frown/pout/wry_mouth/deep_breath）+ idle 仅 release；
   不再包含 bounce_happy / wave_big / happy / excited / sway / bow / lean_in 等
   模型无法诚实表达的动作；
2) 运行时能力过滤：模型参数集缺门槛参数（如 pout 的 Param4）的动作
   不进目录；参数集未知时 fail-closed 只保留基础六动作；
3) payload 可选 source/priority/duration 元数据，且旧客户端兼容（None 省略）；
4) 同一句时间轴去重 + 最小间隔；显式用户命令覆盖时间轴；
5) 导演/规则/括注/插件/用户命令任何来源的 motion 都必须过白名单，
   「鞠躬/靠近一点/鼓掌/挥挥手/跳起来」不产出任何动作；
6) catalog 中文描述只精确描述参数效果，不含挥手/鼓掌/蹦跳/鞠躬/凑近/摇摆语义。
"""

import asyncio
from types import SimpleNamespace

from open_llm_vtuber.motion_catalog import (
    ALLOWED_ACTIONS,
    BASE_MOTION_IDS,
    CAPABILITY_GATED_MOTION_IDS,
    MOTION_METADATA,
    MOTION_REQUIRED_PARAMS,
    RELEASE_ONLY_MOTIONS,
    SAFE_MOTION_IDS,
    allowed_actions_for_parameter_ids,
    motions_for_parameter_ids,
    sanitize_motion,
)
from open_llm_vtuber.director import (
    _VALID_MOTIONS,
    ExpressionDirector,
    DirectorResult,
    detect_user_motion_command,
    normalize_chunk_timeline,
    stage_gesture,
)
from open_llm_vtuber.agent.output_types import Actions, DisplayText, SentenceOutput

# 定版：公开目录精确等于 基础六动作 + 能力门控扩展十一动作 + idle(release)
_EXPECTED_BASE = ("nod", "shake_no", "tilt", "look_around", "listen", "surprise")
_EXPECTED_GATED = (
    "look_up",
    "look_down",
    "glance_left",
    "glance_right",
    "wink_left",
    "wink_right",
    "smile",
    "frown",
    "pout",
    "wry_mouth",
    "deep_breath",
)
_FORBIDDEN_SEMANTIC_WORDS = ("挥手", "鼓掌", "蹦跳", "鞠躬", "凑近", "摇摆")

# bai 形状模型的完整头/脸参数集（含非标 Param4 嘟嘴 / Param6 歪嘴）
_BAI_PARAM_IDS = frozenset({
    "ParamAngleX", "ParamAngleY", "ParamAngleZ",
    "ParamEyeLOpen", "ParamEyeROpen", "ParamEyeBallX", "ParamEyeBallY",
    "ParamBrowLY", "ParamBrowRY",
    "ParamMouthOpenY", "ParamMouthForm",
    "ParamBreath", "Param4", "Param5", "Param6",
})


# ─────────────────────────────────────────────
# 1) 安全动作目录
# ─────────────────────────────────────────────
def test_catalog_excludes_full_body_and_arm_actions():
    for bad in ("bounce_happy", "wave_big", "happy", "excited", "sway", "bow", "lean_in"):
        assert bad not in ALLOWED_ACTIONS, bad
        assert bad not in SAFE_MOTION_IDS, bad
        assert bad not in _VALID_MOTIONS, bad


def test_safe_motion_ids_exact_extended_catalog():
    assert BASE_MOTION_IDS == _EXPECTED_BASE
    assert CAPABILITY_GATED_MOTION_IDS == _EXPECTED_GATED
    assert SAFE_MOTION_IDS == _EXPECTED_BASE + _EXPECTED_GATED
    assert _VALID_MOTIONS == frozenset(SAFE_MOTION_IDS)
    assert ALLOWED_ACTIONS == frozenset(SAFE_MOTION_IDS) | frozenset({"idle"})


def test_capability_required_params_declared_for_all_gated_actions():
    # 扩展动作必须声明门槛；基础六动作不声明（渲染端逐通道降级即可诚实表达）
    for mid in _EXPECTED_GATED:
        required = MOTION_REQUIRED_PARAMS.get(mid)
        assert required, mid
        assert isinstance(required, frozenset) and len(required) > 0, mid
    for mid in _EXPECTED_BASE:
        assert MOTION_REQUIRED_PARAMS.get(mid) is None, mid
    # 门槛参数本身必须是模型头/脸通道，绝不包含身体通道
    for required in MOTION_REQUIRED_PARAMS.values():
        for param_id in required:
            assert param_id.startswith("Param"), param_id
            assert "Body" not in param_id and "Leg" not in param_id and "Hip" not in param_id


def test_motions_for_parameter_ids_filters_by_runtime_capability():
    # 完整 bai 参数集 → 全部动作可用
    assert set(motions_for_parameter_ids(_BAI_PARAM_IDS)) == set(SAFE_MOTION_IDS)
    # 缺 Param4 → pout 消失，其余保留
    no_p4 = motions_for_parameter_ids(_BAI_PARAM_IDS - {"Param4"})
    assert "pout" not in no_p4
    assert "wry_mouth" in no_p4
    # 缺 Param6 → wry_mouth 消失，pout 保留
    no_p6 = motions_for_parameter_ids(_BAI_PARAM_IDS - {"Param6"})
    assert "wry_mouth" not in no_p6
    assert "pout" in no_p6
    # look_up 需要 AngleY+EyeBallY 同时存在：缺一即不可用
    no_geyeballY = motions_for_parameter_ids(_BAI_PARAM_IDS - {"ParamEyeBallY"})
    assert "look_up" not in no_geyeballY and "look_down" not in no_geyeballY
    # wink 单眼独立门控
    no_el = motions_for_parameter_ids(_BAI_PARAM_IDS - {"ParamEyeLOpen"})
    assert "wink_left" not in no_el and "wink_right" in no_el


def test_motions_for_parameter_ids_fail_closed_on_unknown_model():
    # 参数集为空/None（模型未加载或不可知）→ 只保留无门槛基础六动作
    assert motions_for_parameter_ids(set()) == _EXPECTED_BASE
    assert motions_for_parameter_ids(None) == _EXPECTED_BASE
    allowed = allowed_actions_for_parameter_ids(None)
    assert allowed == frozenset(_EXPECTED_BASE) | RELEASE_ONLY_MOTIONS
    # idle 始终作为 release 语义可用
    assert "idle" in allowed_actions_for_parameter_ids(_BAI_PARAM_IDS - {"Param4"})


def test_metadata_descriptions_without_forbidden_semantics():
    # 六个动作 + idle 的中文描述只精确描述参数效果，不得出现“挥手/鼓掌/蹦跳/
    # 鞠躬/凑近/摇摆”等 Bai 无法用参数诚实表达的语义。
    for mid in SAFE_MOTION_IDS:
        desc = str(MOTION_METADATA[mid]["description"])
        label = str(MOTION_METADATA[mid]["label"])
        for word in _FORBIDDEN_SEMANTIC_WORDS:
            assert word not in desc, (mid, word)
            assert word not in label, (mid, word)


def test_wave_keywords_no_longer_produce_any_motion():
    # 「挥手」语义不再映射到任何上身近似动作：直接返回 None。
    assert stage_gesture("（开心地挥手）你好呀") is None
    assert stage_gesture("（挥手打招呼）大家好啊") is None
    assert stage_gesture("（用力挥手）再见") is None


def test_keyword_fallback_table_is_empty_and_honest():
    # 关键词兜底词表定版为空：无法诚实映射到六动作的词一律不产动作。
    from open_llm_vtuber.live2d_model import MOTION_KEYWORDS, extract_motion_keyword

    assert MOTION_KEYWORDS == {}
    for text in ("挥挥手", "和大家打招呼", "再见啦", "拜拜～", "掰掰"):
        assert extract_motion_keyword(text) is None, text
    for bad_label in ("wave", "special_02"):
        assert bad_label not in MOTION_KEYWORDS.values()


def test_rule_fallback_emotions_never_force_motions_except_surprise():
    from open_llm_vtuber.director import rule_fallback

    # joy / anger / sadness 等不再强行动作；只有 surprise 兜底 surprise。
    assert rule_fallback("哈哈太好了").motion_label is None
    assert rule_fallback("你真讨厌").motion_label is None
    assert rule_fallback("我好难过").motion_label is None
    surprise = rule_fallback("哇，真的吗？")
    assert surprise.emotion == "surprise"
    assert surprise.motion_label == "surprise"


def test_idle_is_release_only():
    assert "idle" in RELEASE_ONLY_MOTIONS
    assert "idle" not in SAFE_MOTION_IDS
    assert "idle" not in _VALID_MOTIONS
    assert MOTION_METADATA["idle"]["release_only"] is True


def test_all_safe_motions_have_full_metadata():
    for mid in SAFE_MOTION_IDS:
        meta = MOTION_METADATA.get(mid)
        assert meta is not None, mid
        assert meta["label"], mid
        assert meta["description"], mid
        assert meta["halfbody"] is True, mid
        assert isinstance(meta["suggested_duration_ms"], int) and meta["suggested_duration_ms"] > 0
        assert meta["release_only"] is False, mid


def test_gated_motion_metadata_is_honest_about_requirements():
    # 能力门控动作的中文说明必须诚实说明参数依赖（中文名/说明不许夸大）。
    for mid in ("pout", "wry_mouth"):
        desc = str(MOTION_METADATA[mid]["description"])
        for param_id in MOTION_REQUIRED_PARAMS[mid]:
            assert param_id in desc, (mid, param_id)
        assert "不出现在目录" in desc or "不提供" in desc, mid
    # look_up/look_down 说明同时提到 AngleY 与 EyeBallY
    for mid in ("look_up", "look_down"):
        desc = str(MOTION_METADATA[mid]["description"])
        assert "AngleY" in desc and "EyeBallY" in desc, mid


# ─────────────────────────────────────────────
# 1.5) 导演能力边界：模型缺门槛参数时 label 同样按越界处理
# ─────────────────────────────────────────────
def _analyze_with_params(payload: str, parameter_ids: set[str] | None = None):
    director = ExpressionDirector(llm=_FakeLLM(payload), parameter_ids=parameter_ids)
    return asyncio.run(director.analyze("随便一句话"))


def test_director_capability_gate_drops_pout_without_param4():
    # 无 Param4 的模型：pout 是能力越界 → 丢弃；joy 无情绪兜底 → None。
    result = _analyze_with_params(
        '{"emotion":"joy","strength":2,"motionLabel":"pout"}',
        parameter_ids=set(_BAI_PARAM_IDS) - {"Param4"},
    )
    assert result.motion_label != "pout"
    assert result.motion_label is None


def test_director_capability_gate_keeps_pout_with_param4():
    # 完整 bai 参数集：pout 合法保留；基础动作不受影响。
    full = set(_BAI_PARAM_IDS)
    assert _analyze_with_params('{"emotion":"joy","motionLabel":"pout"}', full).motion_label == "pout"
    assert _analyze_with_params('{"emotion":"joy","motionLabel":"smile"}', full).motion_label == "smile"
    assert _analyze_with_params('{"emotion":"joy","motionLabel":"nod"}', full).motion_label == "nod"


def test_director_unknown_model_fail_closed_to_base_six():
    # 参数集未知（空）→ 只有基础六动作可选，扩展 label 一律越界丢弃。
    empty = set()
    assert _analyze_with_params('{"emotion":"joy","motionLabel":"pout"}', empty).motion_label is None
    assert _analyze_with_params('{"emotion":"surprise","motionLabel":"deep_breath"}', empty).motion_label == "surprise"
    assert _analyze_with_params('{"emotion":"joy","motionLabel":"listen"}', empty).motion_label == "listen"


def test_director_system_prompt_matches_capability_set():
    # 提示词与实例能力集同源：缺 Param4 时提示词不出现 pout。
    director = ExpressionDirector(
        llm=None,
        parameter_ids=set(_BAI_PARAM_IDS) - {"Param4", "Param6"},
    )
    assert "pout" not in director.system_prompt
    assert "wry_mouth" not in director.system_prompt
    assert "smile" in director.system_prompt and "nod" in director.system_prompt
    # 模块级默认提示词仍覆盖完整静态目录（供测试/日志）。
    from open_llm_vtuber.director import _DIRECTOR_SYSTEM

    for motion in SAFE_MOTION_IDS:
        assert motion in _DIRECTOR_SYSTEM, motion


# ─────────────────────────────────────────────
# 2) 导演链路绝不产出已删动作 / idle 作为逐句 motion
# ─────────────────────────────────────────────
class _FakeLLM:
    def __init__(self, payload):
        self._payload = payload

    async def chat_completion(self, messages, system=None):
        yield self._payload


def _analyze(payload):
    director = ExpressionDirector(llm=_FakeLLM(payload))
    return asyncio.run(director.analyze("随便一句话"))


def test_llm_bounce_happy_produces_no_motion():
    # bounce_happy 已删：越界 label 丢弃；joy 无情绪兜底 → 不产任何动作。
    result = _analyze('{"emotion":"joy","strength":2,"motionLabel":"bounce_happy"}')
    assert result.motion_label != "bounce_happy"
    assert result.motion_label is None


def test_llm_surprise_label_and_fallback_still_work():
    # surprise 是唯一保留情绪兜底的情绪：显式 label 与兜底都指向 surprise。
    assert _analyze('{"emotion":"surprise","motionLabel":"surprise"}').motion_label == "surprise"
    assert _analyze('{"emotion":"surprise","motionLabel":"sway"}').motion_label == "surprise"


def test_llm_removed_labels_fall_back_to_emotion_map_or_none():
    # joy + 已删 label（sway）→ 丢弃且无兜底（None，不强行动作）；
    # anger/sadness + 无 label → None（不产动作）。
    assert _analyze('{"emotion":"joy","motionLabel":"sway"}').motion_label is None
    assert _analyze('{"emotion":"anger","motionLabel":null}').motion_label is None
    assert _analyze('{"emotion":"sadness","motionLabel":"bow"}').motion_label is None


def test_llm_neutral_without_motion_is_none():
    result = _analyze('{"emotion":"neutral","motionLabel":null}')
    assert result.motion_label is None


def test_stage_gesture_bounce_bow_lean_no_longer_map():
    assert stage_gesture("（蹦蹦跳跳）太棒啦") is None
    assert stage_gesture("（跳起来）耶") is None
    assert stage_gesture("（深深鞠躬）感谢大家") is None
    assert stage_gesture("（凑近屏幕）让我看看") is None
    assert stage_gesture("（轻轻摇摆）来啦") is None


def test_detect_user_motion_command_removed_intents_produce_none():
    assert detect_user_motion_command("鞠个躬") is None
    assert detect_user_motion_command("靠近一点") is None
    assert detect_user_motion_command("鼓掌") is None
    assert detect_user_motion_command("挥挥手") is None
    assert detect_user_motion_command("跳起来") is None
    assert detect_user_motion_command("点点头") == "nod"
    assert detect_user_motion_command("你摇摇头") == "shake_no"
    assert detect_user_motion_command("歪一下头") == "tilt"
    assert detect_user_motion_command("张望一下") == "look_around"


def test_sanitize_motion_whitelist_boundary():
    assert sanitize_motion("nod") == "nod"
    assert sanitize_motion("  NOD  ") == "nod"
    assert sanitize_motion(None) is None
    assert sanitize_motion("") is None
    for bad in ("happy", "excited", "sway", "bow", "lean_in", "wave_big", "bounce_happy", "idle"):
        assert sanitize_motion(bad) is None, bad


# ─────────────────────────────────────────────
# 3) 时间轴去重 + 最小间隔
# ─────────────────────────────────────────────
def test_normalize_dedupes_same_motion_in_chunk():
    cues = [
        {"at": 0.1, "motion": "nod"},
        {"at": 0.4, "motion": "nod"},
        {"at": 0.7, "motion": "nod"},
    ]
    assert normalize_chunk_timeline(cues) == [{"at": 0.1, "motion": "nod"}]


def test_normalize_enforces_min_gap_between_different_motions():
    # 0.05 间隔 < min_gap(0.2) → 保留前者
    cues = [{"at": 0.1, "motion": "nod"}, {"at": 0.15, "motion": "shake_no"}]
    assert normalize_chunk_timeline(cues) == [{"at": 0.1, "motion": "nod"}]
    # 0.5 间隔 ≥ min_gap → 两者保留
    spaced = [{"at": 0.1, "motion": "nod"}, {"at": 0.5, "motion": "tilt"}]
    assert normalize_chunk_timeline(spaced) == [
        {"at": 0.1, "motion": "nod"},
        {"at": 0.5, "motion": "tilt"},
    ]


def test_normalize_empty_unsorted_and_removed_motions():
    assert normalize_chunk_timeline([]) == []
    unsorted = [{"at": 0.8, "motion": "tilt"}, {"at": 0.2, "motion": "nod"}]
    out = normalize_chunk_timeline(unsorted)
    assert out == [{"at": 0.2, "motion": "nod"}, {"at": 0.8, "motion": "tilt"}]
    # 无 motion / 已删 id（happy/bow）→ 过滤或清空
    assert normalize_chunk_timeline([{"at": 0.1, "motion": ""}]) == []
    removed = [
        {"at": 0.1, "motion": "nod"},
        {"at": 0.3, "motion": "happy"},
        {"at": 0.5, "motion": "bow"},
    ]
    assert normalize_chunk_timeline(removed) == [{"at": 0.1, "motion": "nod"}]


# ─────────────────────────────────────────────
# 4) payload 可选元数据 + 旧客户端兼容
# ─────────────────────────────────────────────
def test_actions_metadata_serialized_when_set():
    payload = Actions(
        motion="nod",
        motionStrength=2,
        source="user_command",
        priority=100,
        suggested_duration_ms=900,
    ).to_dict()
    assert payload["source"] == "user_command"
    assert payload["priority"] == 100
    assert payload["suggested_duration_ms"] == 900


def test_actions_metadata_omitted_when_none_compat():
    # 旧客户端兼容：未设元数据时 to_dict 与原协议完全相同（不出现新字段）。
    plain = Actions(motion="nod", motionStrength=2).to_dict()
    assert set(plain.keys()) == {"motion", "motionStrength"}
    assert "source" not in plain
    assert "priority" not in plain
    assert "suggested_duration_ms" not in plain


# ─────────────────────────────────────────────
# 5) 显式用户命令：设置优先级/来源并清空时间轴
# ─────────────────────────────────────────────
def test_user_command_sets_priority_and_clears_timeline():
    from open_llm_vtuber.conversations.single_conversation import _apply_director

    class _FakeDirector:
        halfbody_channels = []
        timeout = 0.1

        async def analyze(self, text):
            return DirectorResult(emotion="joy", strength=2, motion_label="listen")

    ctx = SimpleNamespace(
        director=_FakeDirector(),
        live2d_model=SimpleNamespace(emo_map={}),
    )
    out = SentenceOutput(
        display_text=DisplayText(text="好的呢（点头）"),
        tts_text="好的呢",
        actions=Actions(),
    )
    asyncio.run(_apply_director(ctx, out, user_text="你点点头好吗"))

    assert detect_user_motion_command("你点点头好吗") == "nod"
    assert out.actions.motion == "nod"
    assert out.actions.source == "user_command"
    assert out.actions.priority == 100
    assert out._director_result.motion_timeline == []
    assert out.actions.suggested_duration_ms == MOTION_METADATA["nod"]["suggested_duration_ms"]


def test_plugin_and_emotion_default_priority():
    from open_llm_vtuber.conversations.single_conversation import _apply_director

    class _FakeDirector:
        halfbody_channels = []
        timeout = 0.1

        async def analyze(self, text):
            return DirectorResult(emotion="sadness", strength=1, motion_label=None)

    ctx = SimpleNamespace(
        director=_FakeDirector(),
        live2d_model=SimpleNamespace(emo_map={}),
    )
    out = SentenceOutput(
        display_text=DisplayText(text="有点难过呢"),
        tts_text="有点难过呢",
        actions=Actions(),
    )
    asyncio.run(_apply_director(ctx, out, user_text=""))
    # sadness / bow 等已删动作一律清空为 None；无动作不下发。
    assert out.actions.motion is None