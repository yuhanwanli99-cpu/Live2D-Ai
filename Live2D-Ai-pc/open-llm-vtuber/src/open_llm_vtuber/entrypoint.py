"""asyncio 入口封装（仅轻依赖，便于单测与子进程验证）。

用户实测：Ctrl+C 停止 uvicorn 时，即使已优雅关闭，仍会打印
``KeyboardInterrupt`` / ``CancelledError`` 的整段 Python traceback。
原因是 uvicorn 的 ``capture_signals`` 在优雅关闭后会用默认信号处理器
``signal.raise_signal(SIGINT)`` 重新递交 SIGINT，从而在 ``serve()`` 内抛出
``KeyboardInterrupt``；若用户连按或主任务被取消，则可能是
``asyncio.CancelledError``。

本模块把 ``asyncio.run`` 包一层：Ctrl+C / 取消只输出一条简洁停止日志并以
128+2=130 退出，不打印 traceback；真实启动异常（非这两种 BaseException）
仍原样向上传播，Python 照常打印 traceback 并返回非零码。
"""

from __future__ import annotations

import asyncio
from typing import Awaitable, Callable

# SIGINT 规范退出码：128 + 2
_CTRL_C_EXIT_CODE = 130


def run_entry(coro_factory: Callable[[], Awaitable[object]]) -> int:
    """运行一个异步入口，并把用户中断转成简洁停止消息。

    参数:
        coro_factory: 返回待运行协程的零参工厂（延迟构造，便于测试注入）。

    返回:
        进程应使用的退出码；正常结束时为 0。调用方可用 ``sys.exit(ret)``。
    """
    try:
        asyncio.run(coro_factory())
    except KeyboardInterrupt:
        print("[Live2D-Ai] Server stopped by user (Ctrl+C).", flush=True)
        return _CTRL_C_EXIT_CODE
    except asyncio.CancelledError:
        print("[Live2D-Ai] Server stopped (shutdown requested).", flush=True)
        return _CTRL_C_EXIT_CODE
    return 0
