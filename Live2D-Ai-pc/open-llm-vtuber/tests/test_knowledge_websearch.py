# -*- coding: utf-8 -*-
"""KnowledgePlugin / WebSearchPlugin 测试（plugins/native_runtime.py）。

覆盖：知识库增删查列表回环（KNOWLEDGE_DIR 指向 tmp_path 隔离）、中文检索
相关度、llm.before 注入来源标记与短查询跳过、文件名清洗防目录逃逸、
web_search 事件结果结构与上限、默认抓取器离线解析与网络失败降级、
Tavily 路径切换。全部离线：网络调用一律 monkeypatch，绝不发真实请求。
"""

import json
import urllib.request

import pytest

from open_llm_vtuber.plugins.native_runtime import (
    KnowledgePlugin,
    WebSearchPlugin,
)


# ─────────────────────────────────────────────────────────────
# 知识库：文档 CRUD 回环
# ─────────────────────────────────────────────────────────────
@pytest.fixture
def kb(tmp_path, monkeypatch):
    """隔离的知识库插件：先设 KNOWLEDGE_DIR 再构造实例，落盘全在 tmp_path 内。"""
    kb_dir = tmp_path / "knowledge"
    monkeypatch.setenv("KNOWLEDGE_DIR", str(kb_dir))
    return KnowledgePlugin(), kb_dir


class TestKnowledgeDocuments:
    def test_add_list_search_delete_round_trip(self, kb):
        plugin, kb_dir = kb
        added = plugin.add_document(
            "养猫指南.md", "猫粮要选无谷配方。\n\n猫每天需要喝水。"
        )
        assert added["name"] == "养猫指南.md" and added["size"] > 0
        docs = plugin.list_documents()
        assert [d["name"] for d in docs] == ["养猫指南.md"]
        assert {"name", "size", "updated"} <= set(docs[0])
        assert (kb_dir / "养猫指南.md").exists()
        hits = plugin.search("猫粮")
        assert hits and hits[0]["source"] == "养猫指南.md"
        removed = plugin.delete_document("养猫指南.md")
        assert removed["deleted"] is True
        assert plugin.list_documents() == []
        assert plugin.search("猫粮") == []

    def test_delete_missing_document_is_idempotent(self, kb):
        plugin, _ = kb
        removed = plugin.delete_document("不存在.md")
        assert removed["name"] == "不存在.md"
        assert removed["deleted"] is False

    @pytest.mark.parametrize(
        "raw,expected",
        [
            ("../x", "x.md"),  # 目录逃逸 → 剥成裸文件名并强制后缀
            # Windows 风格分隔符：反斜杠用 chr(92) 拼，规避源码转义歧义
            (f"..{chr(92)}..{chr(92)}notes.txt", "notes.txt"),
            ("no-suffix-doc", "no-suffix-doc.md"),  # 后缀缺失 → 强制 .md
            ("a" * 300 + ".md", "a" * 197 + ".md"),  # 超长名截断到 200 字符内
        ],
    )
    def test_sanitize_filename_strips_traversal(self, kb, raw, expected):
        plugin, kb_dir = kb
        result = plugin.add_document(raw, "内容")
        assert result["name"] == expected
        # 文件确实落在知识库目录内部，没有逃逸出去
        assert sorted(p.name for p in kb_dir.iterdir()) == [expected]

    def test_empty_after_sanitize_rejected(self, kb):
        plugin, kb_dir = kb
        with pytest.raises(ValueError):
            plugin.add_document("../..", "内容")
        assert list(kb_dir.iterdir()) == []


class TestKnowledgeSearch:
    def test_chinese_query_ranks_relevant_document_first(self, kb):
        plugin, _ = kb
        plugin.add_document(
            "weather.md",
            "上海今天小雨，气温十二到十八度。\n\n明天转晴，适合晾晒衣物出门散步。",
        )
        plugin.add_document(
            "cat.md",
            "猫咪喜欢吃罐头和猫粮，幼猫要少食多餐。\n\n成年猫每天喂两顿即可。",
        )
        hits = plugin.search("猫粮推荐", top_k=2)
        assert len(hits) <= 2
        assert hits[0]["source"] == "cat.md"
        assert all(len(h["snippet"]) <= 160 for h in hits)

    def test_search_without_overlap_returns_empty(self, kb):
        plugin, _ = kb
        plugin.add_document("cat.md", "猫咪喜欢吃罐头和猫粮。")
        assert plugin.search("天气预报") == []
        assert plugin.search("") == []

    def test_top_k_limits_result_count(self, kb):
        plugin, _ = kb
        paragraphs = "\n\n".join(f"第{i}段提到苹果和香蕉。" for i in range(6))
        plugin.add_document("fruit.md", paragraphs)
        assert len(plugin.search("苹果香蕉", top_k=2)) == 2
        assert len(plugin.search("苹果香蕉")) == 3  # 默认 top_k=3


class TestKnowledgeHandleEvent:
    def test_llm_before_injects_context_with_source_marker(self, kb):
        plugin, _ = kb
        plugin.add_document("cat.md", "猫粮按体重定量喂，幼猫要少食多餐。")
        result = plugin.handle_event(
            "llm.before", {"text": "帮我看看猫咪的猫粮应该怎么选比较好"}
        )
        assert "[知识库检索]" in result["context"]
        assert "(来源: cat.md)" in result["context"]

    def test_short_queries_skip_retrieval(self, kb):
        plugin, _ = kb
        plugin.add_document("cat.md", "猫粮按体重定量喂，幼猫要少食多餐。")
        assert plugin.handle_event("llm.before", {"text": "你好"}) == {}
        assert plugin.handle_event("llm.before", {"text": ""}) == {}

    def test_unmatched_event_and_no_hits_return_empty(self, kb):
        plugin, _ = kb
        plugin.add_document("cat.md", "猫粮按体重定量喂。")
        assert plugin.handle_event("sentence.after", {}) == {}
        assert (
            plugin.handle_event("llm.before", {"text": "完全无关的一句超长查询内容"})
            == {}
        )


# ─────────────────────────────────────────────────────────────
# WebSearch：事件处理与抓取器
# ─────────────────────────────────────────────────────────────
class TestWebSearchHandleEvent:
    @pytest.fixture(autouse=True)
    def _force_duckduckgo(self, monkeypatch):
        """本组用例强制走 DuckDuckGo 分支，避免外部环境变量串扰。"""
        monkeypatch.delenv("TAVILY_API_KEY", raising=False)

    def test_handle_event_shape_with_canned_fetch(self, monkeypatch):
        plugin = WebSearchPlugin()

        def canned(query):
            return [
                {
                    "title": f"{query}-{i}",
                    "url": f"https://example.com/{i}",
                    "snippet": "s",
                }
                for i in range(7)
            ]

        monkeypatch.setattr(plugin, "_fetch", canned)
        result = plugin.handle_event("web_search", {"query": "天气"})
        assert result["provider"] == "duckduckgo"
        assert result["error"] is None
        assert len(result["results"]) == 5  # 结果上限 5 条
        assert set(result["results"][0]) == {"title", "url", "snippet"}

    def test_unknown_event_returns_empty(self):
        assert WebSearchPlugin().handle_event("tool.other", {}) == {}

    def test_empty_query_short_circuits(self):
        result = WebSearchPlugin().handle_event("web_search", {"query": "   "})
        assert result == {"results": [], "provider": "duckduckgo", "error": None}


_DDG_SAMPLE_HTML = (
    "<div>"
    '<a rel="nofollow" class="result__a" '
    'href="//duckduckgo.com/l/?uddg='
    'https%3A%2F%2Fweather.example.com%2Fshanghai&amp;rut=a1">'
    "上海天气 &amp; 空气质量</a>"
    '<a class="result__snippet" href="//click/1">今天 <b>多云</b>，最高 12 度。</a>'
    "</div>"
    "<div>"
    '<a href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fnews.example.com&amp;rut=b2" '
    'rel="nofollow" class="result__a">科技新闻</a>'
    '<a class="result__snippet" href="//click/2">最新 <span>AI</span> 动态汇总。</a>'
    "</div>"
)


class _FakeResponse:
    """带上下文管理器协议的最小 urlopen 返回值（只暴露 read）。"""

    def __init__(self, payload: bytes):
        self._payload = payload

    def read(self) -> bytes:
        return self._payload

    def __enter__(self):
        return self

    def __exit__(self, *exc_info):
        return False


class TestDefaultFetcher:
    def test_parses_ddg_html_and_decodes_uddg_redirect(self, monkeypatch):
        monkeypatch.delenv("TAVILY_API_KEY", raising=False)
        monkeypatch.setattr(
            urllib.request,
            "urlopen",
            lambda req, timeout=None: _FakeResponse(_DDG_SAMPLE_HTML.encode("utf-8")),
        )
        result = WebSearchPlugin().handle_event("web_search", {"query": "上海天气"})
        assert result["provider"] == "duckduckgo"
        assert result["error"] is None
        first, second = result["results"]
        # 第一条：uddg= 跳转参数解码出真实 URL，标签剥离得到纯文本
        assert first["title"] == "上海天气 & 空气质量"
        assert first["url"] == "https://weather.example.com/shanghai"
        assert first["snippet"] == "今天 多云，最高 12 度。"
        # 第二条属性顺序相反（href 在 class 前），同样可解析
        assert second["title"] == "科技新闻"
        assert second["url"] == "https://news.example.com"
        assert second["snippet"] == "最新 AI 动态汇总。"

    def test_network_failure_degrades_to_empty_with_error(self, monkeypatch):
        monkeypatch.delenv("TAVILY_API_KEY", raising=False)

        def boom(*args, **kwargs):
            raise OSError("network unreachable")

        monkeypatch.setattr(urllib.request, "urlopen", boom)
        result = WebSearchPlugin().handle_event("web_search", {"query": "天气"})
        assert result["results"] == []
        assert result["provider"] == "duckduckgo"
        assert result["error"]  # 失败必须可见，不允许静默吞掉


class TestTavilyPath:
    def test_tavily_used_when_api_key_set(self, monkeypatch):
        monkeypatch.setenv("TAVILY_API_KEY", "tvly-test-key")
        captured = {}

        def fake_urlopen(req, timeout=None):
            captured["url"] = req.full_url
            captured["method"] = req.get_method()
            captured["body"] = json.loads(req.data.decode("utf-8"))
            payload = {
                "answer": "上海今天多云转晴。",
                "results": [
                    {
                        "title": "天气网",
                        "url": "https://t.example.com/1",
                        "content": "多云",
                    }
                ],
            }
            return _FakeResponse(
                json.dumps(payload, ensure_ascii=False).encode("utf-8")
            )

        monkeypatch.setattr(urllib.request, "urlopen", fake_urlopen)
        result = WebSearchPlugin().handle_event("web_search", {"query": "上海天气"})
        assert result["provider"] == "tavily"
        assert result["error"] is None
        assert captured["url"].startswith("https://api.tavily.com/search")
        assert captured["method"] == "POST"
        assert captured["body"]["api_key"] == "tvly-test-key"
        assert captured["body"]["max_results"] == 5
        # answer 作为首条摘要条目，results 映射到统一结构
        assert result["results"][0]["snippet"] == "上海今天多云转晴。"
        assert result["results"][1] == {
            "title": "天气网",
            "url": "https://t.example.com/1",
            "snippet": "多云",
        }
