# -*- coding: utf-8 -*-
"""LLM 记忆管家：每轮对话后由独立小模型沉淀/合并/清理长期事实。

与 MemoryPlugin 的正则提取互补：
- 正则（llm.after 钩子）：零成本即时记录，宁可错存不可漏存；
- 本管家（对话链结束后异步执行）：把"用户说了什么 + AI 答了什么"交给 LLM
  对照现有事实做增/删/合并决策——用户定义的内容照单管理，但由 LLM 去重、
  合并矛盾、清理过时条目，事实库不会越长越乱。

失败语义：任何异常只记日志、绝不影响对话主链路；LLM 不可用时管家自动缺席。
"""

from __future__ import annotations

import asyncio
import json
import re
from typing import Any

from loguru import logger

from .logsetup import get_logger

logger = get_logger("memory")

_CURATOR_SYSTEM = (
    "你是虚拟角色的长期记忆管理员。输入是【现有记忆】和【本轮对话(用户/AI)】，"
    "输出 JSON 数组（只输出 JSON，不要解释）：\n"
    '[{"action":"add","fact":"..."},{"action":"remove","fact":"..."}]\n'
    "规则：\n"
    "1. 只管理值得长期记住的用户相关事实：称呼/昵称、偏好、身份背景、约定、重要经历；"
    "寒暄、客套、一次性问答不记。\n"
    "2. add 的新事实必须是独立陈述句，不超过 60 字；与现有记忆重复的不要再 add。\n"
    "3. 现有记忆与本轮对话矛盾或明显过时 → remove 旧条目（fact 填旧条目原文），"
    "必要时再 add 新版本。\n"
    "4. 用户主动要求记住/忘记某事必须执行。\n"
    "5. 最多输出 5 条；没有需要变更的输出 []。"
)


class MemoryCurator:
    """用独立小模型对照现有事实做增量管理决策。"""

    def __init__(self, llm: Any, timeout: float = 12.0, max_actions: int = 5):
        self._llm = llm
        self.timeout = timeout
        self.max_actions = max_actions

    async def curate(
        self,
        user_text: str,
        ai_text: str,
        existing_facts: list[str],
    ) -> list[dict[str, str]]:
        """返回经校验的动作列表 [{"action": "add"|"remove", "fact": str}]。

        LLM 失败 / 输出不合法 → 返回 []（调用方零变更，不影响对话）。
        """
        existing = [f for f in (existing_facts or []) if f][-30:]  # 防提示膨胀
        prompt = (
            f"【现有记忆】\n"
            + ("\n".join(f"- {f}" for f in existing) if existing else "（暂无）")
            + f"\n\n【本轮对话】\n用户：{user_text[:500]}\nAI：{ai_text[:500]}\n\nJSON:"
        )
        raw = await self._call(prompt)
        actions = self._parse(raw)
        return actions[: self.max_actions]

    async def _call(self, prompt: str) -> str:
        messages = [{"role": "user", "content": prompt}]
        stream = self._llm.chat_completion(messages, system=_CURATOR_SYSTEM)
        chunks: list[str] = []

        async def collect():
            async for item in stream:
                if isinstance(item, str):
                    chunks.append(item)
                # list 形态为工具调用分支，管理员场景不存在，跳过

        await asyncio.wait_for(collect(), timeout=self.timeout)
        return "".join(chunks).strip()

    @staticmethod
    def _parse(raw: str) -> list[dict[str, str]]:
        """宽容解析：剥 code fence → 取首个 JSON 数组 → 校验字段。"""
        if not raw:
            return []
        text = re.sub(r"```(?:json)?", "", raw).strip().strip("`")
        m = re.search(r"\[.*\]", text, re.DOTALL)
        if not m:
            return []
        try:
            data = json.loads(m.group(0))
        except Exception:  # noqa: BLE001
            logger.warning("记忆管家输出不是合法 JSON，忽略本轮")
            return []
        if not isinstance(data, list):
            return []
        valid: list[dict[str, str]] = []
        for item in data:
            if not isinstance(item, dict):
                continue
            action = str(item.get("action") or "").strip().lower()
            fact = str(item.get("fact") or "").strip()
            if action in ("add", "remove") and fact and len(fact) <= 120:
                valid.append({"action": action, "fact": fact})
        return valid


def apply_curation(inst: Any, actions: list[dict[str, str]], expected_revision=None, scope: str = "") -> int:
    """把管家决策应用到 MemoryPlugin 实例，返回实际生效条数。

    - add：插件内去重（已存在 → False 不计）；
    - remove：先精确匹配，再双向包含匹配第一条。

    陈旧写入防护：异步管家在「查询快照 → LLM 往返 → 应用」期间，若用户清空/改写了
    记忆（revision 变化），则放弃本轮应用——这是「设置页清空后，进程内迟到管家不再
    把已删除事实灌回」的关键。
    """
    if expected_revision is not None:
        # 只对比插件暴露的代次；调用方在查询快照后、异步往返前捕获它。
        current = getattr(inst, "revision", None)
        if callable(current) and current() != expected_revision:
            logger.info("记忆管家命中陈旧快照（期间发生清空/改写），放弃应用")
            return 0

    applied = 0
    for action in actions[:8]:
        kind = action.get("action")
        fact = action.get("fact") or ""
        if kind == "add":
            if inst.add_fact(fact, scope=scope):
                applied += 1
                logger.info(f"记忆 + {fact}")
        elif kind == "remove":
            facts = inst.list_facts(scope=scope)
            idx = next((i for i, f in enumerate(facts) if f == fact), None)
            if idx is None:
                idx = next(
                    (i for i, f in enumerate(facts) if fact in f or f in fact), None
                )
            if idx is not None and inst.remove_fact(idx, scope=scope):
                applied += 1
                logger.info(f"记忆 − {facts[idx]}")
    return applied
