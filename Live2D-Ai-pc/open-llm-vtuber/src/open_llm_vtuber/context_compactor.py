"""上下文压缩器（纯逻辑，不依赖 LLM）。

负责：
- 粗略估算 token 数量
- 决定哪些历史消息需要压缩
- 生成分层摘要计划（turn -> topic -> long-term fact）

真实摘要生成由上层调用方（context-compactor 插件）注入 summarizer。
本模块不吞异常；所有错误都通过返回值或抛出异常显式暴露。
"""

from __future__ import annotations

from dataclasses import dataclass, field


def estimate_tokens(text: str) -> int:
    """粗略 token 估算：中文按每字约 0.6 token，其他按每 4 字符 1 token。

    这是预算器，不是精确 tokenizer；只用于决定何时压缩。
    """
    if not text:
        return 0
    cjk = sum(1 for ch in text if "一" <= ch <= "鿿")
    other = max(0, len(text) - cjk)
    return int(cjk * 0.6 + other / 4)


@dataclass
class CompactionChunk:
    """一段需要被压缩的连续消息。"""

    start_index: int
    end_index: int  # exclusive
    token_estimate: int
    level: str = "turn"  # turn / topic / long_term
    reason: str = ""


@dataclass
class CompactionPlan:
    chunks: list[CompactionChunk] = field(default_factory=list)
    keep_recent_count: int = 0
    total_tokens: int = 0
    budget_tokens: int = 0

    @property
    def needs_compaction(self) -> bool:
        return bool(self.chunks)

    def to_dict(self) -> dict:
        return {
            "chunks": [
                {
                    "start_index": c.start_index,
                    "end_index": c.end_index,
                    "token_estimate": c.token_estimate,
                    "level": c.level,
                    "reason": c.reason,
                }
                for c in self.chunks
            ],
            "keep_recent_count": self.keep_recent_count,
            "total_tokens": self.total_tokens,
            "budget_tokens": self.budget_tokens,
        }


class ContextCompactor:
    """按 token 预算规划历史压缩，但真正摘要动作留给调用方。"""

    def __init__(self, max_tokens: int = 6000, keep_recent_tokens: int = 2400):
        if max_tokens <= keep_recent_tokens:
            raise ValueError("max_tokens must be greater than keep_recent_tokens")
        self.max_tokens = int(max_tokens)
        self.keep_recent_tokens = int(keep_recent_tokens)

    def estimate_messages(self, messages: list[dict]) -> int:
        """估算一组消息的总 token 数。"""
        total = 0
        for msg in messages:
            content = msg.get("content", "")
            total += estimate_tokens(str(content))
        return total

    def plan(self, messages: list[dict]) -> CompactionPlan:
        """规划哪些旧消息需要压缩。

        从最旧消息开始累计，直到剩余（尾部）消息 token 数 <= keep_recent_tokens。
        被累计的部分形成 chunk，交给调用方生成摘要。
        """
        total = self.estimate_messages(messages)
        plan = CompactionPlan(
            total_tokens=total,
            budget_tokens=self.max_tokens,
        )
        if total <= self.max_tokens:
            return plan

        # 从尾部保留最近消息，直到达到 keep_recent_tokens
        keep_count = 0
        kept_tokens = 0
        for idx in range(len(messages) - 1, -1, -1):
            t = estimate_tokens(str(messages[idx].get("content", "")))
            if kept_tokens + t > self.keep_recent_tokens and keep_count > 0:
                break
            kept_tokens += t
            keep_count += 1

        plan.keep_recent_count = keep_count
        if keep_count < len(messages):
            older = messages[: len(messages) - keep_count]
            older_tokens = self.estimate_messages(older)
            plan.chunks.append(
                CompactionChunk(
                    start_index=0,
                    end_index=len(older),
                    token_estimate=older_tokens,
                    level="topic",
                    reason="recent window exceeds token budget",
                )
            )
        return plan

    @staticmethod
    def apply_summaries(
        messages: list[dict],
        plan: CompactionPlan,
        summaries: list[str],
    ) -> list[dict]:
        """用摘要替换被压缩的旧消息块。

        每个 chunk 对应一个 summary；摘要作为 system 消息插入，剩余尾部原样保留。
        """
        if not plan.chunks:
            return list(messages)
        if len(summaries) < len(plan.chunks):
            raise ValueError("summaries length does not match compaction chunks")

        chunks = sorted(plan.chunks, key=lambda c: c.start_index)
        result: list[dict] = []
        cursor = 0
        for chunk, summary in zip(chunks, summaries):
            result.extend(messages[cursor : chunk.start_index])
            result.append({"role": "system", "content": summary})
            cursor = chunk.end_index
        result.extend(messages[cursor:])
        return result
