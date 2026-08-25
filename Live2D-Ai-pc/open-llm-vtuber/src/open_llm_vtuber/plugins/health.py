"""插件系统健康/效果报告：启动日志与 API 共用同一份聚合逻辑。

回答「插件是否真的工作」：
- 每个插件的 loaded/enabled/status/调用次数/最近错误；
- 原生工具（live2d.perform_action）的注册状态、白名单与调用统计。
"""

from __future__ import annotations

from typing import Any

# 原生工具所在插件 id → 工具注册信息读取方法
_NATIVE_TOOL_PLUGINS: tuple[str, ...] = ("live2d-action",)


def native_tool_reports(manager) -> list[dict[str, Any]]:
    """枚举已注册的原生 LLM 工具及其注册/统计状态。"""
    reports: list[dict[str, Any]] = []
    for plugin_id in _NATIVE_TOOL_PLUGINS:
        instance = manager.host.get_instance(plugin_id)
        registration = getattr(instance, "registration", None)
        health = getattr(instance, "health", None)
        if instance is None or not callable(registration):
            reports.append(
                {
                    "plugin": plugin_id,
                    "registered": False,
                    "available": False,
                }
            )
            continue
        try:
            data = dict(registration())
        except Exception as exc:  # noqa: BLE001 - 状态损坏必须可见，不能 500
            data = {"registered": False, "available": False, "error": str(exc)}
        data["plugin"] = plugin_id
        if callable(health):
            try:
                data.update(health())
            except Exception as exc:  # noqa: BLE001
                data["error"] = str(exc)
        reports.append(data)
    return reports


def startup_report(manager) -> list[str]:
    """生成插件系统启动日志行（server 启动时逐条 logger.info/warning）。"""
    lines: list[str] = []
    manifests = manager.list_plugins()
    statuses = manager.health()
    loaded_n = sum(1 for p in statuses.values() if p["loaded"])
    enabled_n = sum(1 for p in statuses.values() if p["enabled"])
    runtime_n = sum(1 for m in manifests if manager.implementation(m.id) == "runtime")
    error_ids = [pid for pid, p in statuses.items() if p["status"] == "error"]
    lines.append(
        f"[Plugins] 原生插件注册完成：共 {len(manifests)} 个"
        f"（runtime {runtime_n}），host 已加载 {loaded_n} 个，启用 {enabled_n} 个"
    )
    if error_ids:
        lines.append(f"[Plugins] 加载异常插件: {', '.join(error_ids)}")
    for manifest in manifests:
        info = statuses.get(manifest.id, {})
        suffix = ""
        if info.get("last_error"):
            suffix = f" last_error={info['last_error']!r}"
        lines.append(
            f"[Plugins]   {manifest.id}: enabled={info.get('enabled')} "
            f"status={info.get('status')} implementation={info.get('implementation')}"
            f"{suffix}"
        )

    for tool in native_tool_reports(manager):
        if tool.get("registered") and tool.get("available"):
            whitelist = ", ".join(tool.get("whitelist") or [])
            lines.append(
                f"[NativeTool] {tool.get('tool')} 已注册并可用（白名单: {whitelist}）；"
                "主对话 LLM 为 OpenAI 兼容 provider 时可自主调用"
            )
        else:
            reason = tool.get("error") or f"插件 {tool.get('plugin')} 未加载或被禁用"
            lines.append(
                f"[NativeTool] {tool.get('tool', 'live2d.perform_action')} 未生效"
                f"（{reason}）；ExpressionDirector 兜底继续负责动作"
            )
    return lines


def aggregate_health(manager) -> dict[str, Any]:
    """构建 /api/plugins/health 响应体（JSON 安全）。"""
    plugins = manager.health()
    summary = {
        "total": len(plugins),
        "loaded": sum(1 for p in plugins.values() if p["loaded"]),
        "enabled": sum(1 for p in plugins.values() if p["enabled"]),
        "errors": sum(1 for p in plugins.values() if p["status"] == "error"),
        "native_tools_registered": sum(
            1 for t in native_tool_reports(manager) if t.get("registered")
        ),
    }
    return {
        "ok": True,
        "summary": summary,
        "plugins": plugins,
        "native_tools": native_tool_reports(manager),
    }
