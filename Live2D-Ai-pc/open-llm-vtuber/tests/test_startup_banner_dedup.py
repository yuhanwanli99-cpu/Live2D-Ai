# -*- coding: utf-8 -*-
"""启动版本横幅去重 —— 静态源级验证。

用户实测：从 start.sh 启动时版本横幅打印了两次（start.sh 一次 + run_server.py 一次）。

契约：
1. start.sh 在 exec run_server.py 前打印一次版本横幅，并设置环境标记
   ``LIVE2DAI_BANNER_SHOWN=1``（export，随 exec 继承给子进程）。
2. run_server.py 读取该标记：仅当 ``!= "1"`` 时才打印版本横幅。
   —— 从 start.sh 启动只显示一次；直接运行 run_server.py 仍显示一次。

这里用静态文本断言（不执行服务器 / 不触碰网络），若两边实现偏离契约即失败。
"""

from __future__ import annotations

import os

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
START_SH = os.path.join(PROJECT_ROOT, "..", "start.sh")
RUN_SERVER = os.path.join(PROJECT_ROOT, "run_server.py")

BANNER_MARKER = "LIVE2DAI_BANNER_SHOWN"


def _read(*parts: str) -> str:
    path = os.path.join(PROJECT_ROOT, *parts)
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


def test_start_sh_sets_marker_before_exec():
    src = _read("..", "start.sh")
    # 注意：代理指尖命令里多处使用 `exec 3<>`（bash 描述符），此处要匹配真正的
    # 进程替换 `exec "$PY" run_server.py`，不能只搜 `exec `。
    assert "LIVE2DAI_BANNER_SHOWN" in src, "start.sh 必须设置横幅去重标记"
    marker_pos = src.index(BANNER_MARKER)
    exec_run = 'exec "$PY" run_server.py'
    assert ("exec $\"PY\" run_server.py" in src or exec_run in src), (
        "start.sh 必须最终 exec run_server.py"
    )
    exec_pos = src.index(exec_run)
    # 标记必须在最终 exec run_server.py 之前
    assert marker_pos < exec_pos, "START_BANNER 标记必须在 exec run_server.py 之前"
    # 确实以 export 形式设置，能随 exec 继承
    assert "export " + BANNER_MARKER + "=" in src, "标记必须 export 以便 exec 继承"


def test_run_server_gates_banner_on_marker():
    src = _read("run_server.py")
    assert BANNER_MARKER in src, "run_server.py 必须读取横幅去重标记"
    # 仅当未设置（!= "1"）时打印
    assert f'os.environ.get("{BANNER_MARKER}")' in src or (
        f'os.environ.get(\'{BANNER_MARKER}\')' in src
    )
    assert '!= "1"' in src, "run_server.py 必须在标记为 1 时跳过横幅"
    # 直接 python run_server.py（无标记）仍打印 —— 通过 default 检查：默认 os.environ 无标记
    assert 'format_banner()' in src
