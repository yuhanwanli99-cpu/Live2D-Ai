"""显式用户动作命令 + 动作优先级 + neutral 无动作 + 句首唯一动作 测试。

契约（发布定版）：
- detect_user_motion_command：用户输入里的 点头/摇头/歪头/张望
  映射合法动作；鞠躬/靠近/鼓掌/挥挥手/跳起来 不产出任何动作；
  疑问/否定语境（为什么/别/怎么/…）整句不触发；
- 优先级：显式用户命令 > 导演 motion_label（含括注时间轴首帧）> 情绪兜底；
- neutral 不产出动作（None）；anger/sadness 等也无情绪兜底动作；
- 一次性 actions.motion 只挂句首首个 audio payload，后续 chunk 清掉
  （motionTimeline 按块携带不受影响）。
"""

from types import SimpleNamespace

from open_llm_vtuber.agent.output_types import Actions, DisplayText, SentenceOutput
from open_llm_vtuber.conversations.conversation_utils import (
    handle_sentence_output,
    split_tts_chunks,
)
from open_llm_vtuber.conversations.single_conversation import _apply_director
from open_llm_vtuber.director import (
    ExpressionDirector,
    detect_user_motion_command,
    rule_fallback,
)


# ------------------------------------------------------------- detect_user_motion_command


def test_command_mapping_basic():
    assert detect_user_motion_command("点点头") == "nod"
    assert detect_user_motion_command("你摇摇头") == "shake_no"
    assert detect_user_motion_command("歪一下头") == "tilt"
    assert detect_user_motion_command("张望一下") == "look_around"
    # 已删除的命令不产出任何动作
    assert detect_user_motion_command("鞠个躬") is None
    assert detect_user_motion_command("靠近一点") is None
    assert detect_user_motion_command("鼓掌") is None
    assert detect_user_motion_command("挥挥手") is None
    assert detect_user_motion_command("跳起来") is None


def test_command_requires_explicit_keyword():
    assert detect_user_motion_command("你好呀") is None
    assert detect_user_motion_command("今天天气不错") is None
    assert detect_user_motion_command("") is None
    assert detect_user_motion_command(None) is None


def test_command_guard_blocks_question_negation():
    # 疑问/否定语境：讨论动作，不是命令动作 → 不触发
    assert detect_user_motion_command("为什么摇头") is None
    assert detect_user_motion_command("你刚才为什么摇头") is None
    assert detect_user_motion_command("别摇头") is None
    assert detect_user_motion_command("不要点头") is None
    assert detect_user_motion_command("怎么张望") is None
    assert detect_user_motion_command("什么叫鞠躬") is None
    assert detect_user_motion_command("他不会点头") is None


def test_command_polite_request_still_works():
    # 礼貌请求/允许语气没有 guard 词 → 正常触发
    assert detect_user_motion_command("你点点头好吗") == "nod"
    assert detect_user_motion_command("给我摇摇头看看") == "shake_no"


# ------------------------------------------------------------------ rule_fallback 兜底


def test_rule_fallback_neutral_has_no_motion():
    result = rule_fallback("就是一句普通的话")
    assert result.emotion == "neutral"
    assert result.motion_label is None
    assert result.strength == 1


def test_rule_fallback_joy_has_no_motion_surprise_maps():
    # 定版：情绪兜底只允许 surprise→surprise；joy 不再强行动作。
    joy = rule_fallback("哈哈太好了")
    assert joy.emotion == "joy"
    assert joy.motion_label is None

    surprise = rule_fallback("哇，天哪！")
    assert surprise.emotion == "surprise"
    assert surprise.motion_label == "surprise"


# -------------------------------------------------------------- _apply_director 优先级


class _FakeLLM:
    def __init__(self, payload):
        self._payload = payload

    async def chat_completion(self, messages, system=None):
        yield self._payload


def _make_context(payload: str):
    """真实 ExpressionDirector(FakeLLM) + 最小 context 桩。"""
    director = ExpressionDirector(llm=_FakeLLM(payload))
    live2d_model = SimpleNamespace(emo_map={})
    return SimpleNamespace(director=director, live2d_model=live2d_model)


def _sentence(motion: str | None = None) -> SentenceOutput:
    return SentenceOutput(
        display_text=DisplayText(text="好的呢"),
        tts_text="好的呢",
        actions=Actions(motion=motion),
    )


def _run(coro):
    import asyncio

    return asyncio.run(coro)


def test_user_command_overrides_director_motion():
    ctx = _make_context('{"emotion":"joy","strength":3,"motionLabel":"sway"}')
    out = _sentence()
    _run(_apply_director(ctx, out, user_text="点点头"))
    # 用户显式命令最高优先：覆盖导演 sway，并清掉时间轴（无括注）
    assert out.actions.motion == "nod"
    assert out.actions.motionStrength >= 2
    assert out._director_result.motion_timeline == []


def test_director_motion_used_without_user_command():
    ctx = _make_context('{"emotion":"surprise","strength":3,"motionLabel":"sway"}')
    out = _sentence()
    _run(_apply_director(ctx, out, user_text="你好呀"))
    # sway 已从目录删除 → 丢弃越界 label；surprise 情绪兜底 → surprise
    assert out.actions.motion == "surprise"
    assert out.actions.motionStrength == 3


def test_director_invalid_label_without_fallback_is_none():
    ctx = _make_context('{"emotion":"joy","strength":3,"motionLabel":"sway"}')
    out = _sentence()
    _run(_apply_director(ctx, out, user_text="你好呀"))
    # sway 已删 + joy 无情绪兜底 → 不产任何动作（None，不强行动作）。
    assert out.actions.motion is None
    assert "motion" not in out.actions.to_dict()


def test_neutral_has_no_actions_motion():
    ctx = _make_context('{"emotion":"neutral","motionLabel":null}')
    out = _sentence()
    _run(_apply_director(ctx, out, user_text=""))
    assert out.actions.motion is None


def test_user_command_overrides_bracket_timeline():
    # 回复文本带括注（（挥手扇）→ sway）但用户明确要求点头 → 命令优先、时间轴清空
    out = SentenceOutput(
        display_text=DisplayText(text="（挥手扇）好的呢"),
        tts_text="好的呢",
        actions=Actions(motion=None),
    )
    ctx = _make_context('{"emotion":"joy","strength":2,"motionLabel":"happy"}')
    _run(_apply_director(ctx, out, user_text="点点头"))
    assert out.actions.motion == "nod"
    assert out._director_result.motion_timeline == []


# ------------------------------------------- 一次性 motion 仅句首（不要每个 chunk 重复）


class _RecorderTtsManager:
    def __init__(self):
        self.calls = []

    async def speak(self, **kwargs):
        self.calls.append(kwargs)


async def _noop_ws_send(_payload):
    return None


def test_one_shot_motion_only_on_first_chunk():
    # 40 个"啊"+句号 → split_tts_chunks 切成两块（36 / 4+。）
    tts_text = "啊" * 40 + "。"
    chunks = split_tts_chunks(tts_text)
    assert len(chunks) == 2

    out = SentenceOutput(
        display_text=DisplayText(text=tts_text),
        tts_text=tts_text,
        actions=Actions(motion="nod"),
    )
    manager = _RecorderTtsManager()
    import asyncio

    asyncio.run(handle_sentence_output(out, None, None, _noop_ws_send, manager))

    assert len(manager.calls) == 2
    first, second = manager.calls
    assert first["sentence_start"] is True
    assert second["sentence_start"] is False
    # 一次性动作只挂句首；第二个 chunk 清掉 motion，避免每个 chunk 重复触发
    assert first["actions"].motion == "nod"
    assert second["actions"].motion is None
    # 原 actions 不被污染
    assert out.actions.motion == "nod"


# ------------------------------------------- sentence.after 广播不互相清空动作


class _FakePluginHost:
    """按预置结果列表回放 emit_event 的桩（模拟多插件广播）。"""

    def __init__(self, results):
        self._results = results
        self.calls = []

    def emit_event(self, name, payload):
        self.calls.append((name, payload))
        return [
            {"plugin": f"p{i}", "ok": True, "result": r}
            for i, r in enumerate(self._results)
        ]


def _manager_with(results):
    from types import SimpleNamespace

    return SimpleNamespace(host=_FakePluginHost(results))


def test_plugin_without_motion_key_does_not_clobber_motion():
    # sentence.after 是广播：mood-tts 等未参与动作决策的插件（结果无 "motion"
    # 键）不得把已定动作清空。
    import asyncio

    out = SentenceOutput(
        display_text=DisplayText(text="好的呢"),
        tts_text="好的呢",
        actions=Actions(motion="nod"),
    )
    manager = _RecorderTtsManager()
    plugins = _manager_with([{"mood": {"tier": "neutral"}, "tts_style": {}}])
    asyncio.run(
        handle_sentence_output(out, None, None, _noop_ws_send, manager, plugin_manager=plugins)
    )
    assert manager.calls[0]["actions"].motion == "nod"


def test_plugin_motion_still_goes_through_whitelist():
    # 插件显式给出的 motion 仍然过目录白名单：合法保留、已删 id 清空、可覆盖。
    import asyncio

    out = SentenceOutput(
        display_text=DisplayText(text="好的呢"),
        tts_text="好的呢",
        actions=Actions(motion="nod"),
    )
    manager = _RecorderTtsManager()
    plugins = _manager_with([{"motion": "tilt"}, {"motion": "happy"}])
    asyncio.run(
        handle_sentence_output(out, None, None, _noop_ws_send, manager, plugin_manager=plugins)
    )
    # body-director 给 tilt，随后插件给已删 id happy → 收敛为 None（不回退旧值）
    assert manager.calls[0]["actions"].motion is None