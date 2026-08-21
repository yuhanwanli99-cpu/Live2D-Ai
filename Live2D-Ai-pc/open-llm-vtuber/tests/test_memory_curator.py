"""LLM 记忆管家（MemoryCurator）与决策应用测试。全部离线（LLM 用假桩、asyncio.run 驱动）。"""

import asyncio

from open_llm_vtuber.memory_curator import MemoryCurator, apply_curation


class FakeLLM:
    """chat_completion 返回异步迭代字符串块的最小桩。"""

    def __init__(self, chunks):
        self._chunks = chunks
        self.last_messages = None
        self.last_system = None

    async def chat_completion(self, messages, system=None):  # noqa: RUF029 — 与真实接口一致（异步生成器）
        self.last_messages = messages
        self.last_system = system
        for c in self._chunks:
            yield c


class FakeMemory:
    def __init__(self, facts=None):
        self._facts = list(facts or [])

    def list_facts(self):
        return list(self._facts)

    def add_fact(self, fact):
        fact = (fact or "").strip()
        if not fact or fact in self._facts:
            return False
        self._facts.append(fact)
        return True

    def remove_fact(self, index):
        if 0 <= index < len(self._facts):
            self._facts.pop(index)
            return True
        return False


def _curate(chunks, user_text, ai_text, facts=None):
    curator = MemoryCurator(llm=FakeLLM(chunks))
    return asyncio.run(curator.curate(user_text, ai_text, facts or []))


def test_curate_parses_add_and_remove():
    actions = _curate(
        ['[{"action":"add","fact":"主人叫南凉"},{"action":"remove","fact":"旧称呼"}]'],
        "我叫南凉",
        "记住啦",
        ["旧称呼"],
    )
    assert {"action": "add", "fact": "主人叫南凉"} in actions
    assert {"action": "remove", "fact": "旧称呼"} in actions


def test_curate_strips_code_fence():
    payload = fence_payload()
    actions = _curate([payload], "我喜欢猫", "好呀")
    assert actions == [{"action": "add", "fact": "喜欢猫"}]


def fence_payload():
    fence = chr(96) * 3  # markdown 围栏
    return (fence + "json" + chr(10)
            + '[{"action":"add","fact":"喜欢猫"}]' + chr(10) + fence)


def test_curate_malformed_returns_empty():
    assert _curate(["我觉得应该记点东西"], "随便聊聊", "好的呀", ["x"]) == []


def test_curate_filters_invalid_entries():
    long = chr(38271) * 121
    raw = (
        '[{"action":"delete","fact":"x"},{"action":"add","fact":""},'
        + '{"action":"add","fact":"' + long + '"},'
        + '{"action":"add","fact":"有效事实"}]'
    )
    actions = _curate([raw], "输入", "输出")
    assert actions == [{"action": "add", "fact": "有效事实"}]


def test_apply_curation_add_remove_dedup():
    inst = FakeMemory(["旧称呼"])
    applied = apply_curation(inst, [
        {"action": "add", "fact": "旧称呼"},          # 重复 → 不计
        {"action": "add", "fact": "新称呼南凉"},
        {"action": "remove", "fact": "旧称呼"},
        {"action": "remove", "fact": "不存在"},       # 匹配不到 → 不计
    ])
    assert applied == 2
    assert inst.list_facts() == ["新称呼南凉"]


def test_apply_curation_remove_by_containment():
    inst = FakeMemory(["了哦～主播=老大=南凉=0=给，小南凉会陪着你"])
    applied = apply_curation(inst, [
        {"action": "remove", "fact": "主播=老大=南凉"},
    ])
    assert applied == 1
    assert inst.list_facts() == []


def test_prompt_contains_existing_facts_and_dialogue():
    llm = FakeLLM(["[]"])
    curator = MemoryCurator(llm=llm)
    asyncio.run(curator.curate("用户说", "AI 答", ["事实A", "事实B"]))
    prompt = llm.last_messages[0]["content"]
    assert "- 事实A" in prompt and "- 事实B" in prompt
    assert "用户：用户说" in prompt and "AI：AI 答" in prompt