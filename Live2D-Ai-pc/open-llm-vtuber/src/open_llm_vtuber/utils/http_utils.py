import asyncio
from typing import Any


def extract_upstream_message(resp) -> str:
    """从上游错误响应提取人类可读 message，不吞成裸 HTTP 数字。"""
    try:
        body = resp.json()
    except Exception:  # noqa: BLE001
        body = None
    if isinstance(body, dict):
        err = body.get("error")
        if isinstance(err, dict) and err.get("message"):
            return str(err["message"])
        if isinstance(err, str):
            return err
        if body.get("message"):
            return str(body["message"])
    text = resp.text or ""
    return text[:300]


def requests_get(url: str, headers: dict[str, Any], timeout: float):
    """同步 requests.get 移出事件循环（审计 E-1）。"""
    import requests as _requests

    return asyncio.to_thread(_requests.get, url, headers=headers, timeout=timeout)


def requests_post(url: str, headers: dict[str, Any], json_body: dict[str, Any], timeout: float):
    """同步 requests.post 移出事件循环（审计 E-1）。"""
    import requests as _requests

    return asyncio.to_thread(
        lambda: _requests.post(
            url, headers=headers, json=json_body, timeout=timeout
        )
    )
