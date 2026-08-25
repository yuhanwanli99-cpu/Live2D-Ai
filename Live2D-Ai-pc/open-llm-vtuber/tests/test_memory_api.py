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
        self._scopes = {}
        self._revision = 0

    def _facts_for(self, scope):
        return self._facts if not scope else self._scopes.setdefault(scope, [])

    def revision(self):
        return self._revision

    def _touch(self):
        self._revision += 1

    def list_facts(self, scope=""):
        return list(self._facts_for(scope))

    def add_fact(self, fact, scope=""):
        fact = fact.strip()
        facts = self._facts_for(scope)
        if not fact or fact in facts:
            return False
        facts.append(fact)
        self._touch()
        return True

    def remove_fact(self, index, scope=""):
        facts = self._facts_for(scope)
        if 0 <= index < len(facts):
            facts.pop(index)
            self._touch()
            return True
        return False

    def clear_facts(self, scope=""):
        facts = self._facts_for(scope)
        n = len(facts)
        facts.clear()
        if n:
            self._touch()
        return n

    def clear_all_facts(self):
        n = len(self._facts)
        self._facts.clear()
        for key in list(self._scopes.keys()):
            n += len(self._scopes[key])
        self._scopes = {}
        if n:
            self._touch()
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


# ------------------------------------------------- 会话隔离 + 清空后不再注入


def _memory_client(inst):
    """以给定 MemoryPlugin 实例构建带 /api/memory/* 路由的 TestClient。"""
    from fastapi import FastAPI
    from fastapi.testclient import TestClient

    manager = PluginManager()
    manager.host.register(inst)
    app = FastAPI()
    app.include_router(init_plugin_routes(manager))
    return TestClient(app)


def test_scope_isolation_between_sessions(tmp_path, monkeypatch):
    """不同会话 scope 互不串味：会话A 的记忆不写入会话B，清空 A 也不影响 B。"""
    monkeypatch.chdir(tmp_path)  # _state_path 落 tmp，不污染仓库
    from open_llm_vtuber.plugins.native_runtime import MemoryPlugin

    inst = MemoryPlugin()
    c = _memory_client(inst)

    # 会话 A 写入
    assert c.post("/api/memory/facts", json={"fact": "A喜欢猫"}, params={"scope": "charA::sess1"}).json()["ok"] is True
    # 会话 B 初始为空（隔离）
    assert c.get("/api/memory/facts", params={"scope": "charA::sess2"}).json()["facts"] == []
    # 全局（scope 缺省）初始为空（不同会话不写进全局）
    assert c.get("/api/memory/facts").json()["facts"] == []

    # 会话 B 写入自己的事实，与 A 互不影响
    c.post("/api/memory/facts", json={"fact": "B喜欢猫"}, params={"scope": "charA::sess2"})
    assert c.get("/api/memory/facts", params={"scope": "charA::sess1"}).json()["facts"] == ["A喜欢猫"]
    assert c.get("/api/memory/facts", params={"scope": "charA::sess2"}).json()["facts"] == ["B喜欢猫"]

    # 清空会话 A：只清 A，B 保持
    assert c.delete("/api/memory/facts", params={"scope": "charA::sess1"}).json()["cleared"] == 1
    assert c.get("/api/memory/facts", params={"scope": "charA::sess1"}).json()["facts"] == []
    assert c.get("/api/memory/facts", params={"scope": "charA::sess2"}).json()["facts"] == ["B喜欢猫"]
    # 持久层同样反映隔离
    import json as _json

    store = _json.loads((tmp_path / "cache" / "memory_store.json").read_text(encoding="utf-8"))
    assert store["scopes"]["charA::sess1"] == []
    assert store["scopes"]["charA::sess2"] == ["B喜欢猫"]


def test_clear_then_same_process_next_llm_before_is_empty(tmp_path, monkeypatch):
    """清空后当前进程内新一轮 llm.before 不再注入已删除事实。"""
    monkeypatch.chdir(tmp_path)
    from open_llm_vtuber.plugins.native_runtime import MemoryPlugin

    inst = MemoryPlugin()
    c = _memory_client(inst)

    # 先注入并存一条事实
    inst.handle_event("llm.after", {"user_text": "我叫小白，我喜欢猫"})
    assert c.get("/api/memory/facts").json()["facts"] == ["小白", "猫"]

    # 清空（持久层 + 进程内状态）
    assert c.delete("/api/memory/facts").json()["cleared"] >= 1

    # 同一进程、新一轮 llm.before：不应返回 context（无记忆可注入）
    out = inst.handle_event("llm.before", {"text": "小白你是谁？"})
    assert "context" not in out


def test_stale_curator_aborted_after_clear():
    """管家在查询快照后、应用前用户清空 → expected_revision 不匹配 → 放弃应用。"""
    from open_llm_vtuber.memory_curator import apply_curation
    from open_llm_vtuber.plugins.native_runtime import MemoryPlugin

    inst = MemoryPlugin()  # 用普通实例（不落盘也行，add_fact 会 _save 但无害）

    # 准备现有事实，捕获 revision 快照
    inst.add_fact("主人叫小白")
    rev_before = inst.revision()

    # 期间发生清空 → revision 变化（与 expected 不一致）
    inst.clear_facts()
    assert inst.revision() != rev_before

    # 拿到的是清空前的快照决策，现在应用应被放弃
    applied = apply_curation(
        inst,
        [{"action": "add", "fact": "主人叫小白"}],
        expected_revision=rev_before,
    )
    assert applied == 0
    assert inst.list_facts() == []  # 没有重新灌回


def test_scoped_clear_uses_scoped_facts(tmp_path, monkeypatch):
    """带 scope 的 add/clear 互不干扰且持久化隔离。"""
    monkeypatch.chdir(tmp_path)
    from open_llm_vtuber.plugins.native_runtime import MemoryPlugin

    inst = MemoryPlugin()
    c = _memory_client(inst)

    c.post("/api/memory/facts", json={"fact": "全局1"})
    c.post("/api/memory/facts", json={"fact": "会话1"}, params={"scope": "char::s1"})
    c.post("/api/memory/facts", json={"fact": "会话2"}, params={"scope": "char::s2"})

    # 显式 scope 只清该 scope，且推进代次让迟到管家放弃过期写入
    assert c.delete("/api/memory/facts", params={"scope": "char::s1"}).json()["cleared"] == 1
    assert c.get("/api/memory/facts", params={"scope": "char::s1"}).json()["facts"] == []
    assert c.get("/api/memory/facts", params={"scope": "char::s2"}).json()["facts"] == ["会话2"]
    assert c.get("/api/memory/facts").json()["facts"] == ["全局1"]


def test_no_scope_clear_all_wipes_global_and_all_sessions(tmp_path, monkeypatch):
    """无 scope 的 DELETE /api/memory/facts = 清空全部（全局 + 所有会话 scope）。

    清空后：(1) 进程内各 scope 与持久层全部为空；(2) 任何查询在途/迟到管家快照
    的应用都无法复活任一 scope 的事实（revision 已推进）。
    """
    monkeypatch.chdir(tmp_path)
    from open_llm_vtuber.memory_curator import apply_curation
    from open_llm_vtuber.plugins.native_runtime import MemoryPlugin

    inst = MemoryPlugin()
    c = _memory_client(inst)

    # 全局 + 两个会话都写入
    c.post("/api/memory/facts", json={"fact": "全局1"})
    c.post("/api/memory/facts", json={"fact": "会话A-喜欢猫"}, params={"scope": "char::A"})
    c.post("/api/memory/facts", json={"fact": "会话B-周末加班"}, params={"scope": "char::B"})

    # 捕获全部快照和 revision（模拟在途管家已经读到旧状态）
    rev_before = inst.revision()
    stale_actions = [
        {"action": "add", "fact": "会话A-喜欢猫"},
        {"action": "add", "fact": "会话B-周末加班"},
    ]

    # 用户点击「清空全部」→ 无 scope DELETE
    assert c.delete("/api/memory/facts").json()["cleared"] == 3

    # 清空后 3 处全部为空
    assert c.get("/api/memory/facts").json()["facts"] == []
    assert c.get("/api/memory/facts", params={"scope": "char::A"}).json()["facts"] == []
    assert c.get("/api/memory/facts", params={"scope": "char::B"}).json()["facts"] == []

    # 持久层同样全空
    import json as _json

    store = _json.loads((tmp_path / "cache" / "memory_store.json").read_text(encoding="utf-8"))
    assert store["facts"] == []
    assert store["scopes"] == {}

    # revision 已推进 → 在途/迟到管家拿到的是清空前的旧快照，应用必须放弃
    assert inst.revision() != rev_before
    applied = apply_curation(
        inst,
        stale_actions,
        expected_revision=rev_before,
    )
    assert applied == 0
    # 没有任何 scope 被复活
    assert c.get("/api/memory/facts").json()["facts"] == []
    assert c.get("/api/memory/facts", params={"scope": "char::A"}).json()["facts"] == []
    assert c.get("/api/memory/facts", params={"scope": "char::B"}).json()["facts"] == []
