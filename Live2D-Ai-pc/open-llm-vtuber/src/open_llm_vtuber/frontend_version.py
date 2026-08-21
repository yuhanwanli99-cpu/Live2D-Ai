# -*- coding: utf-8 -*-
"""前端版本上报接口（P3）。

提供一个轻量 GET /api/frontend-version 端点，供前端在启动时确认自己加载的
index.html 是否为后端预期的最新构建产物：

    GET /api/frontend-version
    -> {"ok": true,
        "version": "<前12位小写sha256>",
        "index_sha256": "<完整sha256>",
        "index_mtime": <float 或 int>}

计算对象为 frontend_dir 下的 index.html（默认相对进程 cwd 的 frontend/index.html）。
文件不存在时返回 ok=false 与 error，而不抛 500，便于前端据降级到其他提示。
"""

from __future__ import annotations

import hashlib
import os
from typing import Optional

from fastapi import APIRouter
from loguru import logger
from starlette.responses import JSONResponse


def _index_sha256(index_path: str) -> str:
    """返回 index.html 的完整 sha256（hex 小写）。"""
    sha = hashlib.sha256()
    with open(index_path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            sha.update(chunk)
    return sha.hexdigest()


def init_frontend_version_routes(frontend_dir: Optional[str] = None) -> APIRouter:
    """工厂函数：创建 frontend-version 路由。

    Args:
        frontend_dir: 前端静态目录。默认相对进程 cwd 的 ``frontend``。
            只用于定位 index.html，供测试注入临时目录。
    """
    router = APIRouter()

    frontend_path = frontend_dir or "frontend"

    @router.get("/api/frontend-version")
    async def frontend_version() -> JSONResponse:
        index_path = os.path.join(frontend_path, "index.html")
        if not os.path.isfile(index_path):
            logger.warning(f"frontend-version: index.html not found at {index_path}")
            return JSONResponse(
                {"ok": False, "error": f"index.html not found at {index_path}"}
            )

        try:
            full_sha = _index_sha256(index_path)
            mtime = os.path.getmtime(index_path)
        except OSError as exc:
            logger.warning(f"frontend-version: failed to read {index_path}: {exc}")
            return JSONResponse(
                {"ok": False, "error": f"failed to read index.html: {exc}"}
            )

        return JSONResponse(
            {
                "ok": True,
                "version": full_sha[:12],
                "index_sha256": full_sha,
                "index_mtime": mtime,
            }
        )

    return router
