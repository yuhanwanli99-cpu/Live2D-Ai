"""长期记忆可视化管理端点（/api/memory/*）测试。

契约：
- GET    /api/memory/facts        → {ok, facts}
- POST   /api/memory/facts {fact} → 去重追加；空串/超长 → 422
- DELETE /api/memory/facts/{idx}  → 越界 → 404
- DELETE /api/memory/facts        → 清空并返回条数
- 记忆插件未加载 → 一律 404（可见降级，不裸 500）
"""

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from open_llm_vtuber.plugin_routes import init_plugin_routes
from open_llm_vtuber.plugins.manager import PluginManager


class _FakeMemory:
    """与 MemoryPlugin 公开方法同形的假实例（路由只依赖该方法面）。"""

    id = "memory"

    def __init__(self):
        self._facts = []

    def list_facts(self):
        return list(self._facts)

    def add_fact(self, fact):
        fact = fact.strip()
        if not fact or fact in self._facts:
            return False
        self._facts.append(fact)
        return True

    def remove_fact(self, index):
        if 0 <= index < len(self._facts):
            self._facts.pop(index)
            return True
        return False

    def clear_facts(self):
        n = len(self._facts)
        self._facts.clear()
        return n


def _make_client(with_plugin: bool = True) -> TestClient:
    manager = PluginManager()
    if with_plugin:
        manager.host.register(_FakeMemory())
    app = FastAPI()
    app.include_router(init_plugin_routes(manager))
    return TestClient(app)


@pytest.fixture()
def client():
    return _make_client()


def test_list_empty(client):
    res = client.get("/api/memory/facts")
    assert res.status_code == 200
    assert res.json() == {"ok": True, "facts": []}


def test_add_and_dedupe(client):
    res = client.post("/api/memory/facts", json={"fact": "主人叫小白"})
    assert res.status_code == 200
    body = res.json()
    assert body["ok"] is True and body["deduplicated"] is False
    assert body["facts"] == ["主人叫小白"]

    res2 = client.post("/api/memory/facts", json={"fact": " 主人叫小白 "})
    assert res2.json()["deduplicated"] is True
    assert len(res2.json()["facts"]) == 1


def test_add_blank_rejected(client):
    assert client.post("/api/memory/facts", json={"fact": "   "}).status_code == 422
    assert client.post("/api/memory/facts", json={"fact": ""}).status_code == 422


def test_add_too_long_rejected(client):
    res = client.post("/api/memory/facts", json={"fact": "长" * 201})
    assert res.status_code == 422


def test_delete_by_index(client):
    client.post("/api/memory/facts", json={"fact": "a"})
    client.post("/api/memory/facts", json={"fact": "b"})
    res = client.delete("/api/memory/facts/0")
    assert res.status_code == 200
    assert res.json()["facts"] == ["b"]

    res404 = client.delete("/api/memory/facts/9")
    assert res404.status_code == 404


def test_clear_all(client):
    client.post("/api/memory/facts", json={"fact": "a"})
    client.post("/api/memory/facts", json={"fact": "b"})
    res = client.delete("/api/memory/facts")
    assert res.json()["cleared"] == 2
    assert client.get("/api/memory/facts").json()["facts"] == []


def test_plugin_missing_is_visible_404():
    client = _make_client(with_plugin=False)
    assert client.get("/api/memory/facts").status_code == 404
    assert client.post("/api/memory/facts", json={"fact": "x"}).status_code == 404
    assert client.delete("/api/memory/facts").status_code == 404
    assert client.delete("/api/memory/facts/0").status_code == 404


# ------------------------------------------------- 知识库 / 联网查询管理端点


class _FakeKnowledge:
    id = "knowledge"

    def __init__(self):
        self.docs = {}

    def list_documents(self):
        return [
            {"name": n, "size": len(c), "updated": 0.0} for n, c in self.docs.items()
        ]

    def add_document(self, name, content):
        name = name.strip()
        if not name or "/" in name or "\\" in name or ".." in name:
            raise ValueError("非法文件名")
        if not name.endswith((".md", ".txt")):
            name += ".md"
        self.docs[name] = content
        return name

    def delete_document(self, name):
        return self.docs.pop(name, None) is not None


class _FakeSearch:
    id = "web-search"

    def handle_event(self, name, payload):
        assert name == "web_search"
        q = payload.get("query") or ""
        return {
            "results": [{"title": "r", "url": "https://e.example", "snippet": q}],
            "provider": "duckduckgo",
            "error": None,
        }


def test_knowledge_documents_roundtrip():
    manager = PluginManager()
    manager.host.register(_FakeKnowledge())
    app = FastAPI()
    app.include_router(init_plugin_routes(manager))
    c = TestClient(app)

    assert c.get("/api/knowledge/documents").json()["documents"] == []
    res = c.post(
        "/api/knowledge/documents", json={"name": "设定集", "content": "白住在云之城"}
    )
    assert res.status_code == 200 and res.json()["name"] == "设定集.md"

    docs = c.get("/api/knowledge/documents").json()["documents"]
    assert docs[0]["name"] == "设定集.md" and docs[0]["size"] > 0

    bad = c.post("/api/knowledge/documents", json={"name": "../x", "content": "v"})
    assert bad.status_code == 422

    assert c.delete("/api/knowledge/documents/设定集.md").json()["ok"] is True
    assert c.delete("/api/knowledge/documents/不存在.md").status_code == 404


def test_knowledge_plugin_missing_is_404():
    manager = PluginManager()
    app = FastAPI()
    app.include_router(init_plugin_routes(manager))
    c = TestClient(app)
    assert c.get("/api/knowledge/documents").status_code == 404
    assert (
        c.post("/api/knowledge/documents", json={"name": "a", "content": "b"}).status_code
        == 404
    )


def test_web_search_endpoint_shape_and_validation():
    manager = PluginManager()
    manager.host.register(_FakeSearch())
    app = FastAPI()
    app.include_router(init_plugin_routes(manager))
    c = TestClient(app)

    res = c.post("/api/tools/web-search", json={"query": "今天天气"})
    body = res.json()
    assert res.status_code == 200 and body["ok"] is True
    assert body["provider"] == "duckduckgo"
    assert body["results"][0]["snippet"] == "今天天气"

    assert c.post("/api/tools/web-search", json={"query": "  "}).status_code == 422


def test_web_search_plugin_missing_is_404():
    manager = PluginManager()
    app = FastAPI()
    app.include_router(init_plugin_routes(manager))
    c = TestClient(app)
    assert c.post("/api/tools/web-search", json={"query": "x"}).status_code == 404


# ------------------------------------ 真实 MemoryPlugin × 路由集成（tmp 隔离）


def test_real_memory_plugin_roundtrip_via_routes(tmp_path, monkeypatch):
    """load_native 真链路：MemoryPlugin 实例满足 /api/memory/* 契约。"""
    monkeypatch.chdir(tmp_path)  # _state_path 相对 cache/ → 落在 tmp，不污染仓库
    from open_llm_vtuber.plugins.native_runtime import MemoryPlugin

    manager = PluginManager()
    manager.host.register(MemoryPlugin())
    app = FastAPI()
    app.include_router(init_plugin_routes(manager))
    c = TestClient(app)

    assert c.get("/api/memory/facts").json()["facts"] == []
    assert c.post("/api/memory/facts", json={"fact": "主人喜欢猫"}).json()["ok"] is True
    assert c.post("/api/memory/facts", json={"fact": "周末加班"}).json()["ok"] is True

    facts = c.get("/api/memory/facts").json()["facts"]
    assert facts == ["主人喜欢猫", "周末加班"]

    # 删除后状态持久化到 cache/memory_store.json（重启不丢）
    assert c.delete("/api/memory/facts/0").json()["facts"] == ["周末加班"]
    store = (tmp_path / "cache" / "memory_store.json").read_text(encoding="utf-8")
    import json as _json

    assert "周末加班" in _json.loads(store)["facts"]
