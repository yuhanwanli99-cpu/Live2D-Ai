# -*- coding: utf-8 -*-
"""仓库根与 shared/ 路径单源（AUDIT P-1：此前五处实现归一）。

约定：shared/ 永远位于仓库根 <repo>/shared。本模块由自身文件位置推导，
不依赖 CWD；固定层级推导为主、向上搜索兜底，兼容非常规安装布局。
"""

from __future__ import annotations

from pathlib import Path


def find_repo_root() -> Path:
    """仓库根目录（含 shared/ 的那层，Live2D-Ai/）。

    主路径：按本文件固定层级推导（utils/ → open_llm_vtuber → src →
    open-llm-vtuber → Live2D-Ai-pc → Live2D-Ai）；推导结果不存在 shared/
    时回退为逐级向上搜索第一个含 shared/ 的祖先。
    """
    derived = Path(__file__).resolve().parents[5]
    if (derived / "shared").is_dir():
        return derived
    current = Path(__file__).resolve().parent
    while current != current.parent:
        if (current / "shared").is_dir():
            return current
        current = current.parent
    return derived  # 兜底返回推导值，让调用方的存在性检查自然暴露问题


def shared_dir() -> Path:
    """仓库 shared/ 目录绝对路径。"""
    return find_repo_root() / "shared"


def resolve_shared_path(config_path: str) -> str:
    """把 "shared/xxx" 形式的路径解析为仓库 shared/ 下绝对路径；其余原样返回。

    语义与旧 service_context._resolve_shared_path / run_server._resolve_shared_path
    一致（配置文件里允许写相对形式，由调用方负责解析，ModelRegistry 不假设 CWD）。
    """
    if config_path.startswith("shared/"):
        return str(shared_dir() / config_path[len("shared/") :])
    return config_path
