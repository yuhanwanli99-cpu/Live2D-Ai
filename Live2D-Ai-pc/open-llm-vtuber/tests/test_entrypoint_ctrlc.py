# -*- coding: utf-8 -*-
"""asyncio 入口 —— Ctrl+C 优雅停止（不打印 traceback）+ 双日志去重。

用户实测：Ctrl+C 停止 uvicorn 时打印整段 KeyboardInterrupt / CancelledError
traceback；正常停止应只输出一条简洁停止日志并无 traceback。

契约：
1. `run_server.py` 的 `__main__` 通过 `open_llm_vtuber.entrypoint.run_entry`
   运行 `main()`，并把 KeyboardInterrupt / CancelledError 转成简洁停止日志，
   以 130 退出，不打印 traceback。
2. 真实启动异常（非 BaseException 中断类）仍由 Python 打印 traceback 并返回
   非零码。
3. WebSocket 断线只记录一次 "Client ... disconnected"（去重）。

子进程实测会发 SIGINT；只读/不触碰网络、不启动真实服务器。
"""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import time

_SRC = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "src"
)


def _py(cmd: str) -> list[str]:
    return [sys.executable, "-c", cmd]


# --------------------------------------------------------------------------- #
# 静态源级契约
# --------------------------------------------------------------------------- #

def _read_run_server() -> str:
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    with open(os.path.join(root, "run_server.py"), "r", encoding="utf-8") as f:
        return f.read()


def test_run_server_uses_run_entry_and_clean_exit():
    src = _read_run_server()
    assert "run_entry" in src, "run_server.py 必须使用 run_entry"
    assert "from open_llm_vtuber.entrypoint import run_entry" in src
    # 必须把 run_entry 的返回值接成进程退出码
    assert "sys.exit(run_entry(" in src


def test_run_entry_handles_interrupt_to_130():
    # 子进程：run_entry 跑一个永不返回的协程，随后父进程发 SIGINT。
    code = (
        "import asyncio, sys\n"
        "from open_llm_vtuber.entrypoint import run_entry\n"
        "async def forever():\n"
        "    await asyncio.Event().wait()\n"
        "sys.exit(run_entry(forever))\n"
    )
    proc = subprocess.Popen(
        _py(code),
        env={**os.environ, "PYTHONPATH": _SRC},
        cwd=os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    time.sleep(1.5)
    proc.send_signal(signal.SIGINT)
    out, err = proc.communicate(timeout=20)
    assert proc.returncode == 130, f"expected 130, got {proc.returncode}\nstdout={out}\nstderr={err}"
    assert "stopped by user" in out, f"expected clean stop message, stdout={out!r}"
    # 不打印 traceback / 中断异常名
    assert "Traceback" not in err.lower(), f"unexpected traceback in stderr={err!r}"
    assert "KeyboardInterrupt" not in err, f"unexpected KeyboardInterrupt in stderr={err!r}"
    assert "CancelledError" not in err, f"unexpected CancelledError in stderr={err!r}"


def test_run_entry_passthrough_on_real_exception():
    # 真实启动异常必须照常打印 traceback 并以非零码退出。
    code = (
        "import sys\n"
        "from open_llm_vtuber.entrypoint import run_entry\n"
        "async def boom():\n"
        "    raise RuntimeError('kaboom-startup')\n"
        "sys.exit(run_entry(boom))\n"
    )
    proc = subprocess.run(
        _py(code),
        env={**os.environ, "PYTHONPATH": _SRC},
        cwd=os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        capture_output=True,
        text=True,
        timeout=20,
    )
    assert proc.returncode != 0, "real startup exception must exit non-zero"
    assert "RuntimeError" in proc.stderr, f"expected traceback in stderr={proc.stderr!r}"
    assert "kaboom-startup" in proc.stderr


# --------------------------------------------------------------------------- #
# WebSocket 断线日志去重（源码级）
# --------------------------------------------------------------------------- #

def _read_ws_handler() -> str:
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    path = os.path.join(
        root, "src", "open_llm_vtuber", "websocket_handler.py"
    )
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


def test_websocket_disconnect_log_not_duplicated_in_loop():
    src = _read_ws_handler()
    # 回环内的 WebSocketDisconnect 分支不应再重复打印 "Client ... disconnected"，
    # 统一由 handle_disconnect() 记录一次。
    idx = src.find("except WebSocketDisconnect:")
    assert idx != -1
    # 该 except 分支（到下一个 raise）里不应再出现 connected 日志
    segment = src[idx : src.find("raise", idx)]
    assert "disconnected" not in segment, "loop-level WebSocketDisconnect branch must not log"
