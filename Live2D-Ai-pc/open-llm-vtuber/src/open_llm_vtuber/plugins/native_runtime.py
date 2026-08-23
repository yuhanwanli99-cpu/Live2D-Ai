"""Native plugin runtime implementations.

这些类通过 entrypoint 被 PluginManager 加载，作为原生插件在 host 中运行。
所有插件错误都通过 PluginHost.emit_event 捕获并记录，不静默吞掉。
"""

from __future__ import annotations

import html
import json
import os
import re
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

from loguru import logger

from ..context_compactor import ContextCompactor
from ..tts.tts_instruction_library import get_instruction


def _state_path(filename: str) -> Path:
    # 运行时状态统一放在 cache/ 下，随仓库本地运行，不进版本控制。
    cache_dir = Path("cache")
    cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir / filename


def _load_json(path: Path, default: Any) -> Any:
    if not path.exists():
        return default
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:  # noqa: BLE001 - 状态损坏必须返回默认并让调用方知道
        return {"error": str(exc), **default}


def _save_json(path: Path, data: Any) -> None:
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")


class MemoryPlugin:
    """极简长期记忆插件：记录用户显式事实，并在对话前检索注入。

    不做关系养成、不做主动陪伴；只维护用户称呼、偏好、约定等事实。
    """

    id = "memory"

    def __init__(self):
        self.state_path = _state_path("memory_store.json")
        self._state = self._load()

    def _load(self) -> dict[str, Any]:
        state = _load_json(self.state_path, {"facts": []})
        if not isinstance(state, dict) or not isinstance(state.get("facts"), list):
            return {"facts": []}
        return state

    def _save(self) -> None:
        _save_json(self.state_path, self._state)

    @staticmethod
    def _extract_facts(text: str) -> list[str]:
        facts: list[str] = []
        patterns = [
            r"我(?:叫|是)([^，。！？,.!?]{1,20})",
            r"我喜欢([^，。！？,.!?]{1,30})",
            r"我养了([^，。！？,.!?]{1,30})",
            # 「我在…」过于宽泛（我在听歌/我在想 都会误存），不再作为事实来源
            r"记住[：: ]*(.+)",
        ]
        for pattern in patterns:
            for match in re.finditer(pattern, text):
                value = match.group(1).strip()
                if value:
                    facts.append(value)
        return facts

    @staticmethod
    def _overlap_score(fact: str, text: str) -> int:
        a = set(fact)
        b = set(text)
        if not a:
            return 0
        return len(a & b)

    def handle_event(self, name: str, payload: dict[str, Any]) -> dict[str, Any]:
        if name == "llm.before":
            text = str(payload.get("text") or "")
            facts = self._state.get("facts", [])
            if not facts or not text:
                return {}
            ranked = sorted(
                facts,
                key=lambda f: self._overlap_score(f, text),
                reverse=True,
            )[:3]
            relevant = [f for f in ranked if self._overlap_score(f, text) > 0]
            if not relevant:
                return {}
            return {"context": "[相关长期记忆]\n" + "\n".join(f"- {f}" for f in relevant)}
        if name == "llm.after":
            # 只从用户的原话里提取显式事实（我叫… / 我喜欢… / 记住：…）。
            # 此前从 AI 回复里正则提取，AI 说「我在等你呢」也会被存成记忆，
            # 删掉对话后这些句子仍以长期记忆形式被反复注入。
            text = str(payload.get("user_text") or "")
            facts = self._extract_facts(text)
            if not facts:
                return {}
            existing = set(self._state.get("facts", []))
            new_facts = [f for f in facts if f not in existing]
            if not new_facts:
                return {}
            self._state.setdefault("facts", []).extend(new_facts)
            self._save()
            return {"memory_updates": new_facts}
        return {}

    # ---- 公开管理接口（/api/memory/* 可视化用；与 handle_event 共享同一份状态） ----

    def list_facts(self) -> list[str]:
        """返回全部已记录事实（副本）。"""
        return list(self._state.get("facts", []))

    def add_fact(self, fact: str) -> bool:
        """手动追加一条事实；空白/超长/重复 → False（不抛异常）。"""
        fact = (fact or "").strip()
        if not fact or len(fact) > 200:
            return False
        facts = self._state.setdefault("facts", [])
        if fact in facts:
            return False
        facts.append(fact)
        self._save()
        return True

    def remove_fact(self, index: int) -> bool:
        """按下标删除一条事实；越界/类型非法 → False。"""
        facts = self._state.get("facts", [])
        try:
            idx = int(index)
        except (TypeError, ValueError):
            return False
        if idx < 0 or idx >= len(facts):
            return False
        facts.pop(idx)
        self._save()
        return True

    def clear_facts(self) -> int:
        """清空全部事实，返回清除条数。"""
        facts = self._state.get("facts", [])
        n = len(facts)
        self._state["facts"] = []
        if n:
            self._save()
        return n


class ContextCompactorPlugin:
    """上下文压缩插件：只负责生成压缩计划与可见提示。

    真正的 LLM 摘要由上层注入；这里不静默截断。
    """

    id = "context-compactor"

    def __init__(self):
        self.compactor = ContextCompactor(max_tokens=6000, keep_recent_tokens=2400)

    def handle_event(self, name: str, payload: dict[str, Any]) -> dict[str, Any]:
        if name != "llm.before":
            return {}
        history = payload.get("history")
        if not isinstance(history, list):
            return {}
        plan = self.compactor.plan(history)
        if not plan.needs_compaction:
            return {}
        return {
            "context": (
                "[上下文预算告警] 历史消息已超过预算，需压缩；"
                f"当前 {plan.total_tokens} tokens，建议保留最近 {plan.keep_recent_count} 条。"
            ),
            "compaction_plan": plan.to_dict(),
        }


# 情绪 → 半身动作确定性兜底（与 director._EMOTION_MOTION 同思路，8 key 全覆盖）；
# 主脑/导演没给出 motion 时，保证"有情绪就有对应肢体表达"，而不是静默无动作。
_EMOTION_GESTURE_FALLBACK = {
    "joy": "happy",
    "surprise": "surprise",
    "sadness": "listen",
    "fear": "listen",
    "smirk": "tilt",
    "anger": "sway",
}

# 心情分层兜底（PC-L4）：正向心情给兴奋表达
_MOOD_TIER_GESTURE_FALLBACK = {
    "happy": "excited",
    "ecstatic": "excited",
}


class BodyDirectorPlugin:
    """身体/表情导演插件的确定性兜底。

    优先沿用调用方已经解析出的 motion 与参数覆盖；缺失时按
    情绪（8 key）→ 心情分层映射出参数驱动手势（bai 无 motion3，
    前端 BodyMotionPlayer 执行参数形变），不再原样透传空动作。
    该插件不另起 LLM，全部为确定性映射。
    """

    id = "body-director"

    def handle_event(self, name: str, payload: dict[str, Any]) -> dict[str, Any]:
        if name != "sentence.after":
            return {}
        motion = payload.get("motion")
        if not motion:
            emotion = str(payload.get("emotion") or "neutral").lower()
            mood_tier = str(payload.get("mood_tier") or "").lower()
            motion = _EMOTION_GESTURE_FALLBACK.get(emotion) or _MOOD_TIER_GESTURE_FALLBACK.get(
                mood_tier
            )
        return {
            "body_frames": [],
            "parameterOverrides": payload.get("parameterOverrides") or {},
            "motion": motion,
        }


class MoodTtsDirectorPlugin:
    """心情/TTS 导演插件的确定性兜底。

    使用与主链路相同的 emotion + strength + mood_tier 生成 TTS instruction。
    """

    id = "mood-tts-director"

    def handle_event(self, name: str, payload: dict[str, Any]) -> dict[str, Any]:
        if name != "sentence.after":
            return {}
        emotion = str(payload.get("emotion") or "neutral")
        strength = int(payload.get("strength") or 2)
        mood_tier = str(payload.get("mood_tier") or "neutral")
        try:
            instruction = get_instruction(
                emotion=emotion, strength=strength, mood_tier=mood_tier
            )
        except Exception as exc:  # noqa: BLE001 - 导演失败返回可见状态
            return {"error": str(exc)}
        return {
            "mood": {"tier": mood_tier},
            "tts_style": {"instruction": instruction},
        }


class WebSearchPlugin:
    """联网搜索插件：把 web_search 工具调用转成统一的检索结果列表。

    默认走 DuckDuckGo HTML 版（仅标准库 urllib + 正则轻量解析）；
    设置环境变量 TAVILY_API_KEY 时优先走 Tavily API。
    任何网络/解析失败都降级为空结果 + error 字段，绝不向 host 抛异常。
    """

    id = "web-search"

    _USER_AGENT = (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"
    )
    _TIMEOUT_SECONDS = 6
    _MAX_RESULTS = 5

    # 免费引擎：Bing 中国站（国内网络可达、零依赖、免 key）。
    # b_algo 块内首个链接为标题，首个 <p> 为摘要；属性顺序不保证，宽松匹配。
    _BING_BLOCK_RE = re.compile(r'<li class="b_algo"(.*?)</li>', re.DOTALL)
    _BING_LINK_RE = re.compile(
        r'<a[^>]*href="(http[^"]+)"[^>]*>(.*?)</a>', re.DOTALL
    )
    _BING_SNIPPET_RE = re.compile(r'<p[^>]*>(.*?)</p>', re.DOTALL)

    def __init__(self):
        # 最近一次成功抓取使用的引擎（handle_event 用于诚实标注 provider）
        self._provider_used: str | None = None

    @staticmethod
    def _strip_tags(fragment: str) -> str:
        # 摘要里混着 <b>/<span> 等标签，直接正则剥掉即可，不需要 HTML 解析器。
        return re.sub(r"<[^>]+>", "", fragment).strip()

    @staticmethod
    def _default_provider() -> str:
        """未实际抓取时的 provider 标注（与 _fetch 的引擎选择保持一致）。"""
        return (
            "tavily"
            if os.environ.get("TAVILY_API_KEY", "").strip()
            else "bing-cn"
        )

    def _fetch_bing_cn(self, query: str) -> list[dict[str, Any]]:
        """Bing 中国站轻量抓取：与 DuckDuckGo 同一套正则剥标签思路。"""
        url = "https://cn.bing.com/search?q=" + urllib.parse.quote_plus(query)
        req = urllib.request.Request(url, headers={"User-Agent": self._USER_AGENT})
        with urllib.request.urlopen(req, timeout=self._TIMEOUT_SECONDS) as resp:
            page = resp.read().decode("utf-8", errors="replace")
        results: list[dict[str, Any]] = []
        for block in self._BING_BLOCK_RE.findall(page):
            link = self._BING_LINK_RE.search(block)
            if link is None:
                continue
            snippet = self._BING_SNIPPET_RE.search(block)
            results.append(
                {
                    "title": html.unescape(self._strip_tags(link.group(2))),
                    "url": html.unescape(link.group(1)),
                    "snippet": (
                        html.unescape(self._strip_tags(snippet.group(1)))
                        if snippet
                        else ""
                    ),
                }
            )
            if len(results) >= self._MAX_RESULTS:
                break
        return results

    def _fetch(self, query: str) -> list[dict[str, Any]]:
        """执行一次检索。

        免费默认引擎只有一个：Bing 中国站（国内网络可达）。
        配置 TAVILY_API_KEY 时切换 Tavily（key 制拓展，属插件生态）。
        """
        api_key = os.environ.get("TAVILY_API_KEY", "").strip()
        if api_key:
            self._provider_used = "tavily"
            return self._fetch_tavily(query, api_key)
        self._provider_used = "bing-cn"
        return self._fetch_bing_cn(query)

    def _fetch_duckduckgo(self, query: str) -> list[dict[str, Any]]:
        """DuckDuckGo HTML 版轻量抓取：正则解析 result__a / result__snippet。"""
        url = "https://html.duckduckgo.com/html/?q=" + urllib.parse.quote_plus(query)
        req = urllib.request.Request(url, headers={"User-Agent": self._USER_AGENT})
        with urllib.request.urlopen(req, timeout=self._TIMEOUT_SECONDS) as resp:
            page = resp.read().decode("utf-8", errors="replace")
        links = self._RESULT_LINK_RE.findall(page)
        snippets = [
            html.unescape(self._strip_tags(raw))
            for raw in self._RESULT_SNIPPET_RE.findall(page)
        ]
        results: list[dict[str, Any]] = []
        for index, (href, title_html) in enumerate(links):
            title = html.unescape(self._strip_tags(title_html))
            results.append(
                {
                    "title": title,
                    "url": html.unescape(self._resolve_url(href)),
                    "snippet": snippets[index] if index < len(snippets) else "",
                }
            )
            if len(results) >= self._MAX_RESULTS:
                break
        return results

    def _fetch_tavily(self, query: str, api_key: str) -> list[dict[str, Any]]:
        """Tavily API 检索：answer 作为首条摘要条目，results 映射为统一结构。"""
        body = json.dumps({"api_key": api_key, "query": query, "max_results": 5})
        req = urllib.request.Request(
            "https://api.tavily.com/search",
            data=body.encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=self._TIMEOUT_SECONDS) as resp:
            data = json.loads(resp.read().decode("utf-8", errors="replace"))
        results: list[dict[str, Any]] = []
        answer = str(data.get("answer") or "").strip()
        if answer:
            results.append({"title": "Tavily 摘要", "url": "", "snippet": answer})
        for item in data.get("results") or []:
            if not isinstance(item, dict):
                continue
            results.append(
                {
                    "title": str(item.get("title") or ""),
                    "url": str(item.get("url") or ""),
                    "snippet": str(item.get("content") or item.get("snippet") or ""),
                }
            )
        return results[: self._MAX_RESULTS]

    def handle_event(self, name: str, payload: dict[str, Any]) -> dict[str, Any]:
        if name != "web_search":
            return {}
        provider = self._provider_used or self._default_provider()
        try:
            query = str(payload.get("query") or "").strip()
            if not query:
                return {"results": [], "provider": provider, "error": None}
            results = self._fetch(query)[: self._MAX_RESULTS]
            return {
                "results": results,
                "provider": self._provider_used or provider,
                "error": None,
            }
        except Exception as exc:  # noqa: BLE001 - 联网失败必须降级，不阻断对话链路
            logger.warning("web-search 检索失败（{}）: {}", provider, exc)
            return {"results": [], "provider": provider, "error": str(exc)}


class KnowledgePlugin:
    """本地知识库插件：markdown/txt 文档落盘 + 字符集重叠检索。

    与 MemoryPlugin 同一套极简思路：不做向量、不做索引，
    用查询字符集与段落字符集的重叠度排序，零依赖且可解释。
    """

    id = "knowledge"

    _ALLOWED_SUFFIXES = (".md", ".txt")
    _MAX_NAME_LEN = 200
    _SNIPPET_CHARS = 160
    _DEFAULT_TOP_K = 3

    def __init__(self):
        self.kb_dir = Path(os.environ.get("KNOWLEDGE_DIR", "cache/knowledge"))
        self.kb_dir.mkdir(parents=True, exist_ok=True)

    def _sanitize_name(self, name: str) -> str:
        """清洗文档名：剥掉路径分隔符与父目录引用，杜绝目录逃逸。

        后缀不在白名单时强制回落 .md；清洗后为空视为非法输入。
        """
        cleaned = os.path.basename(str(name).replace("\\", "/")).strip().lstrip(".")
        stem = Path(cleaned).stem.strip()
        if not stem:
            raise ValueError("knowledge 文档名不能为空")
        suffix = Path(cleaned).suffix.lower()
        if suffix not in self._ALLOWED_SUFFIXES:
            suffix = ".md"
        cleaned = stem + suffix
        limit = self._MAX_NAME_LEN
        if len(cleaned) > limit:
            cleaned = stem[: limit - len(suffix)] + suffix
        return cleaned

    def add_document(self, name: str, content: str) -> dict[str, Any]:
        """写入一篇文档，返回 {name, size}；IO 失败降级为 {} 并告警。"""
        cleaned = self._sanitize_name(name)
        target = self.kb_dir / cleaned
        try:
            target.write_text(str(content or ""), encoding="utf-8")
            return {"name": cleaned, "size": target.stat().st_size}
        except Exception as exc:  # noqa: BLE001 - 写入失败降级，不向调用方抛出
            logger.warning("knowledge 写入失败 {}: {}", cleaned, exc)
            return {}

    def list_documents(self) -> list[dict[str, Any]]:
        """列举知识库内全部 md/txt 文档 [{name, size, updated}]。"""
        docs: list[dict[str, Any]] = []
        try:
            for path in sorted(self.kb_dir.iterdir()):
                if (
                    not path.is_file()
                    or path.suffix.lower() not in self._ALLOWED_SUFFIXES
                ):
                    continue
                stat = path.stat()
                docs.append(
                    {"name": path.name, "size": stat.st_size, "updated": stat.st_mtime}
                )
        except Exception as exc:  # noqa: BLE001 - 列举失败降级为空列表
            logger.warning("knowledge 列举失败: {}", exc)
            return []
        return docs

    def delete_document(self, name: str) -> dict[str, Any]:
        """删除一篇文档，幂等；不存在也返回 deleted=False 而非报错。"""
        cleaned = self._sanitize_name(name)
        target = self.kb_dir / cleaned
        try:
            existed = target.exists()
            if existed:
                target.unlink()
            return {"name": cleaned, "deleted": existed}
        except Exception as exc:  # noqa: BLE001 - 删除失败降级为可见空结果
            logger.warning("knowledge 删除失败 {}: {}", cleaned, exc)
            return {}

    @staticmethod
    def _overlap_score(a: str, b: str) -> int:
        # 与 MemoryPlugin._overlap_score 同思路：字符集合交集大小作为相关度。
        set_a = set(a)
        set_b = set(b)
        if not set_a:
            return 0
        return len(set_a & set_b)

    def search(self, query: str, top_k: int = _DEFAULT_TOP_K) -> list[dict[str, Any]]:
        """按段落粒度做字符集重叠检索，返回 [{source, snippet}]（score>0 降序）。"""
        query = str(query or "").strip()
        if not query:
            return []
        scored: list[tuple[int, str, str]] = []  # (score, source, paragraph)
        try:
            for path in sorted(self.kb_dir.iterdir()):
                if (
                    not path.is_file()
                    or path.suffix.lower() not in self._ALLOWED_SUFFIXES
                ):
                    continue
                try:
                    text = path.read_text(encoding="utf-8")
                except Exception as exc:  # noqa: BLE001 - 单个坏文件不影响其余检索
                    logger.warning("knowledge 读取失败 {}: {}", path.name, exc)
                    continue
                for para in re.split(r"\n\s*\n", text):
                    para = para.strip()
                    if not para:
                        continue
                    score = self._overlap_score(query, para)
                    if score > 0:
                        scored.append((score, path.name, para))
        except Exception as exc:  # noqa: BLE001 - 检索失败降级为空结果
            logger.warning("knowledge 检索失败: {}", exc)
            return []
        scored.sort(key=lambda item: item[0], reverse=True)
        keep = max(int(top_k), 0)
        return [
            {"source": source, "snippet": para[: self._SNIPPET_CHARS]}
            for _, source, para in scored[:keep]
        ]

    def handle_event(self, name: str, payload: dict[str, Any]) -> dict[str, Any]:
        if name != "llm.before":
            return {}
        try:
            text = str(payload.get("text") or "").strip()
            # 太短的输入（寒暄/单字）不值得检索，直接放行。
            if len(text) < 4:
                return {}
            hits = self.search(text, 2)
            if not hits:
                return {}
            lines = [f"- (来源: {h['source']}) {h['snippet']}" for h in hits]
            return {"context": "[知识库检索]\n" + "\n".join(lines)}
        except Exception as exc:  # noqa: BLE001 - 插件边界绝不向 host 抛出
            logger.warning("knowledge 处理 llm.before 失败: {}", exc)
            return {}
