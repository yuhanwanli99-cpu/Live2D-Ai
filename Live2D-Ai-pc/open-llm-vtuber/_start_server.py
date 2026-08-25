"""Converged launcher for Open-LLM-VTuber (Live2D-Ai PC).

原此文件是与 run_server.py 分叉的启动器（缺 `${VAR}` 环境替换、无人设注入、无 MCP
路径解析、端口写死 12393 —— B6）。现已收敛：直接复用 run_server.py 的 bootstrap，
从而获得与主管启动完全一致的 env 替换 + 人设注入 + MCP path + 按 system_config.port
监听。默认服务器以端口 12393 启动。
"""

import asyncio
import os
import sys

# 与 run_server.py 相同的 sys.path 注入，确保 import run_server 可解析 src。
_project_root = os.path.dirname(os.path.abspath(__file__))
if _project_root not in sys.path:
    sys.path.insert(0, _project_root)
_src = os.path.join(_project_root, "src")
if _src not in sys.path:
    sys.path.insert(0, _src)

from run_server import main  # noqa: E402  (复用 run_server 的 bootstrap)


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("[Live2D-Ai] Server stopped")
