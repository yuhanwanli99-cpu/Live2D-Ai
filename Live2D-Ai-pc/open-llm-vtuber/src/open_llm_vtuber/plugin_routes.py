"""HTTP management routes for the plugin registry.

除插件注册表外，本模块还暴露「长期记忆」的可视化管理端点（/api/memory/*），
供设置中心记忆面板读写 MemoryPlugin 的事实列表。
"""

from __future__ import annotations

from typing import Any

from fastapi import APIRouter
from pydantic import BaseModel
from starlette.responses import JSONResponse

from .logsetup import get_logger
from .plugins.health import aggregate_health
from .plugins.manager import PluginManager

_log = get_logger("memory")

_MEMORY_PLUGIN_ID = "memory"
_FACT_MAX_LEN = 200


class _FactAddRequest(BaseModel):
    fact: str


class _DocAddRequest(BaseModel):
    name: str
    content: str


class _WebSearchRequest(BaseModel):
    query: str


def _public(manager: PluginManager, manifest) -> dict[str, Any]:
    """Build the JSON-safe public dict for a plugin."""
    data = manifest.to_public()
    data["implementation"] = manager.implementation(manifest.id)
    data["enabled"] = manager.is_enabled(manifest.id)
    data["status"] = manager.status(manifest.id)
    # 健康/效果可观测：loaded、调用次数、最近事件/错误（回答「插件是否工作」）。
    runtime = manager.host.get_status().get(manifest.id, {})
    data["health"] = {
        "loaded": bool(runtime.get("loaded", False)),
        "calls": int(runtime.get("calls", 0)),
        "errors": int(runtime.get("errors", 0)),
        "last_event": runtime.get("last_event"),
        "last_error": runtime.get("last_error"),
    }
    return data


def _error(status_code: int, message: str) -> JSONResponse:
    return JSONResponse(status_code=status_code, content={"ok": False, "error": message})


def _memory_plugin(manager: PluginManager):
    """取已加载的 MemoryPlugin 实例（未加载或无公开方法 → None）。"""
    inst = manager.host.get_instance(_MEMORY_PLUGIN_ID)
    if inst is None or not hasattr(inst, "list_facts"):
        return None
    return inst


def _plugin_by_capability_method(manager: PluginManager, plugin_id: str, method: str):
    """按 id 取运行时实例并确认暴露指定公开方法（否则 None → 可见降级）。"""
    inst = manager.host.get_instance(plugin_id)
    if inst is None or not hasattr(inst, method):
        return None
    return inst


def init_plugin_routes(manager: PluginManager) -> APIRouter:
    """Create an APIRouter exposing /api/plugins management endpoints."""
    router = APIRouter()

    # 注意：/api/plugins/health 是固定路径，必须先于 /api/plugins/{plugin_id}
    # 注册，否则会被动态路由吞掉。
    @router.get("/api/plugins/health")
    async def plugins_health() -> dict[str, Any]:
        """插件系统健康/效果聚合：loaded/enabled、调用次数、原生工具注册状态。"""
        try:
            return aggregate_health(manager)
        except Exception as exc:  # noqa: BLE001 - 状态损坏必须可见，不能裸 500
            _log.error("plugins health aggregation failed: {}", exc)
            return _error(500, f"插件健康聚合失败: {exc}")

    @router.get("/api/plugins")
    async def list_plugins() -> dict[str, Any]:
        return {"ok": True, "plugins": [_public(manager, m) for m in manager.list_plugins()]}

    @router.get("/api/plugins/{plugin_id}")
    async def get_plugin(plugin_id: str):
        manifest = manager.get_plugin(plugin_id)
        if manifest is None:
            return _error(404, f"Unknown plugin id: {plugin_id}")
        return {"ok": True, "plugin": _public(manager, manifest)}

    @router.post("/api/plugins/{plugin_id}/enable")
    async def enable_plugin(plugin_id: str):
        try:
            manifest = manager.enable(plugin_id)
        except ValueError as exc:
            return _error(404, str(exc))
        return {"ok": True, "plugin": _public(manager, manifest)}

    @router.post("/api/plugins/{plugin_id}/disable")
    async def disable_plugin(plugin_id: str):
        try:
            manifest = manager.disable(plugin_id)
        except ValueError as exc:
            return _error(404, str(exc))
        return {"ok": True, "plugin": _public(manager, manifest)}

    # ---------------- 长期记忆可视化管理（/api/memory/*） ----------------

    @router.get("/api/memory/facts")
    async def list_memory_facts(scope: str = ""):
        plugin = _memory_plugin(manager)
        if plugin is None:
            return _error(404, "记忆插件未加载（memory runtime 不可用）")
        try:
            return {"ok": True, "facts": list(plugin.list_facts(scope=scope))}
        except Exception as exc:  # noqa: BLE001 - 状态损坏必须可见，不能 500 裸抛
            return _error(500, f"读取记忆失败: {exc}")

    @router.post("/api/memory/facts")
    async def add_memory_fact(req: _FactAddRequest, scope: str = ""):
        fact = (req.fact or "").strip()
        if not fact:
            return _error(422, "fact 不能为空")
        if len(fact) > _FACT_MAX_LEN:
            return _error(422, f"fact 过长（≤{_FACT_MAX_LEN} 字）")
        plugin = _memory_plugin(manager)
        if plugin is None:
            return _error(404, "记忆插件未加载（memory runtime 不可用）")
        try:
            added = bool(plugin.add_fact(fact, scope=scope))
        except Exception as exc:  # noqa: BLE001
            _log.error("memory fact add failed: {}", exc)
            return _error(500, f"写入记忆失败: {exc}")
        _log.info("memory fact add: added={} deduplicated={} scope={!r}", added, not added, scope)
        return {"ok": added, "deduplicated": not added, "facts": list(plugin.list_facts(scope=scope))}

    @router.delete("/api/memory/facts/{index}")
    async def delete_memory_fact(index: int, scope: str = ""):
        plugin = _memory_plugin(manager)
        if plugin is None:
            return _error(404, "记忆插件未加载（memory runtime 不可用）")
        try:
            removed = plugin.remove_fact(index, scope=scope)
        except Exception as exc:  # noqa: BLE001
            _log.error("memory fact remove failed: index={} scope={!r}: {}", index, scope, exc)
            return _error(500, f"删除记忆失败: {exc}")
        if not removed:
            return _error(404, f"下标越界: {index}")
        _log.info("memory fact removed: index={} scope={!r}", index, scope)
        return {"ok": True, "facts": list(plugin.list_facts(scope=scope))}

    @router.delete("/api/memory/facts")
    async def clear_memory_facts(scope: str = ""):
        plugin = _memory_plugin(manager)
        if plugin is None:
            return _error(404, "记忆插件未加载（memory runtime 不可用）")
        try:
            # 未提供 scope → 清空全部（全局 + 所有会话 scope），匹配设置页「清空全部」按钮；
            # 显式 scope → 只清该 scope。
            if not scope:
                cleared = int(plugin.clear_all_facts())
            else:
                cleared = int(plugin.clear_facts(scope=scope))
        except Exception as exc:  # noqa: BLE001
            _log.error("memory clear failed: {}", exc)
            return _error(500, f"清空记忆失败: {exc}")
        _log.info("memory cleared: count={} scope={!r}", cleared, scope)
        return {"ok": True, "cleared": cleared}

    # ---------------- 知识库管理（/api/knowledge/*） ----------------

    @router.get("/api/knowledge/documents")
    async def list_knowledge_documents():
        plugin = _plugin_by_capability_method(manager, "knowledge", "list_documents")
        if plugin is None:
            return _error(404, "知识库插件未加载（knowledge runtime 不可用）")
        try:
            return {"ok": True, "documents": list(plugin.list_documents())}
        except Exception as exc:  # noqa: BLE001
            return _error(500, f"读取知识库失败: {exc}")

    @router.post("/api/knowledge/documents")
    async def add_knowledge_document(req: _DocAddRequest):
        if not (req.name or "").strip() or not (req.content or "").strip():
            return _error(422, "name / content 不能为空")
        plugin = _plugin_by_capability_method(manager, "knowledge", "add_document")
        if plugin is None:
            return _error(404, "知识库插件未加载（knowledge runtime 不可用）")
        try:
            name = plugin.add_document(req.name, req.content)
        except ValueError as exc:
            return _error(422, str(exc))
        except Exception as exc:  # noqa: BLE001
            return _error(500, f"写入知识文档失败: {exc}")
        return {"ok": True, "name": name}

    @router.delete("/api/knowledge/documents/{name}")
    async def delete_knowledge_document(name: str):
        plugin = _plugin_by_capability_method(manager, "knowledge", "delete_document")
        if plugin is None:
            return _error(404, "知识库插件未加载（knowledge runtime 不可用）")
        try:
            removed = plugin.delete_document(name)
        except Exception as exc:  # noqa: BLE001
            return _error(500, f"删除知识文档失败: {exc}")
        if not removed:
            return _error(404, f"文档不存在: {name}")
        return {"ok": True}

    # ---------------- 联网查询（/api/tools/web-search） ----------------

    @router.post("/api/tools/web-search")
    async def web_search(req: _WebSearchRequest):
        query = (req.query or "").strip()
        if not query:
            return _error(422, "query 不能为空")
        plugin = _plugin_by_capability_method(manager, "web-search", "handle_event")
        if plugin is None:
            return _error(404, "联网查询插件未加载（web-search runtime 不可用）")
        try:
            result = plugin.handle_event("web_search", {"query": query})
        except Exception as exc:  # noqa: BLE001
            return _error(500, f"联网查询失败: {exc}")
        if not isinstance(result, dict):
            return _error(500, "联网查询返回了意外结构")
        return {"ok": True, **result}

    return router
