# -*- coding: utf-8 -*-
"""PC Preview 版本可观测性 —— 单一产品版本与统一版本 API。

设计目标
--------
- ``product_version``: Preview 产品发布版本，唯一事实来源（``v0.2.0-preview.1``），
  与 ``conf.yaml`` 的 conf schema 版本（``v1.2.1``）明确区分。
- ``backend_version``: 后端包（pyproject.toml / open-llm-vtuber 分发包）版本。
- ``frontend_version``: 前端 renderer 包版本（构建时注入）。
- ``commit``: 构建/启动时的 Git commit（best-effort，取不到返回 ``unknown``）。

所有取版本都在 import 时一次性读取并缓存，运行期稳定；不带依赖注入、不读敏感信息。
"""

from __future__ import annotations

import subprocess
from functools import lru_cache
from pathlib import Path

# --------------------------------------------------------------------------- #
# 单一产品版本（唯一 Preview 事实来源）
# --------------------------------------------------------------------------- #
PRODUCT_VERSION = "v0.2.0-preview.1"

# renderer 前端包版本（build 时可通过 import.meta.env.VITE 覆盖，见版本注入说明）
FRONTEND_PACKAGE_VERSION = "1.0.0"


@lru_cache(maxsize=1)
def _package_version() -> str:
    """后端分发包版本：优先 importlib.metadata，取不到回退 pyproject 兜底。"""
    try:
        from importlib.metadata import PackageNotFoundError, version

        return version("open-llm-vtuber")
    except (PackageNotFoundError, Exception):  # noqa: BLE001 - 版本探测失败只是降级
        pass
    # 兜底：读 pyproject.toml（相对本文件向上溯源，存在即用）
    here = Path(__file__).resolve()
    for parent in (here.parent, here.parent.parent, here.parent.parent.parent):
        pyproject = parent / "pyproject.toml"
        if pyproject.is_file():
            try:
                import tomllib

                with pyproject.open("rb") as f:
                    data = tomllib.load(f)
                v = data.get("project", {}).get("version")
                if v:
                    return str(v)
            except Exception:  # noqa: BLE001
                break
    return "unknown"


@lru_cache(maxsize=1)
def backend_version() -> str:
    """后端包版本（open-llm-vtuber 分发版本，区别于产品版本）。"""
    try:
        return _package_version()
    except Exception:  # noqa: BLE001
        return "unknown"


@lru_cache(maxsize=1)
def _git_commit(repo_root: str) -> str:
    """取 Git 短 commit（best-effort）。repo_root 为空时尝试从 cwd 上升探测。"""
    base = repo_root or "."
    try:
        out = subprocess.run(
            ["git", "-C", base, "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
            timeout=3,
        )
        if out.returncode == 0 and out.stdout.strip():
            return out.stdout.strip()[:12]
    except Exception:  # noqa: BLE001
        pass
    return "unknown"


@lru_cache(maxsize=1)
def commit() -> str:
    """构建 commit（best-effort，取不到返回 unknown）。"""
    here = Path(__file__).resolve()
    for candidate in (here.parent, here.parent.parent, here.parent.parent.parent):
        if (candidate / ".git").exists() or (candidate / ".git").is_file():
            return _git_commit(str(candidate))
    return _git_commit(".")


@lru_cache(maxsize=1)
def common_version_info(commit_override: str | None = None) -> dict:
    """统一的版本信息块。供 /api/version 与 /health 使用。

    Args:
        commit_override: 便于测试注入，默认 None 走 commit()。
    """
    return {
        "product_version": PRODUCT_VERSION,
        "backend_version": backend_version(),
        "frontend_version": FRONTEND_PACKAGE_VERSION,
        "commit": commit_override if commit_override is not None else commit(),
    }


def format_banner() -> str:
    """start.sh / 后端启动日志使用的单行版本横幅。"""
    info = common_version_info()
    return (
        "Live2D-Ai "
        f"product={info['product_version']} "
        f"backend={info['backend_version']} "
        f"frontend={info['frontend_version']} "
        f"commit={info['commit']}"
    )
