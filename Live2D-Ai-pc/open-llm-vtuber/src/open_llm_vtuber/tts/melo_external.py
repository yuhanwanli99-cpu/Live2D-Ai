"""MeloTTS 外部进程客户端（对接独立 conda/melo 环境）。

项目运行环境（Python 3.12）不安装 Melo；Melo 位于用户的
'~/miniconda3/envs/melo/bin/python'（Python 3.10）。本模块维护一个常驻
worker 子进程，通过 stdin/stdout JSON-Lines 调用 melo.api.TTS，从而：
  - 复用同一个 TTS 实例，避免每句 10~30s 模型加载；
  - 支持完整四韵律参数（CLI 只暴露 speed）。
"""
from __future__ import annotations

import atexit
import json
import os
import subprocess
import threading
from collections import deque

_WORKER_SCRIPT = os.path.join(os.path.dirname(__file__), "melo_worker.py")


class MeloExternalClient:
    """Melo worker 子进程的线程安全客户端。"""

    _instance: MeloExternalClient | None = None
    _instance_lock = threading.Lock()

    def __init__(self, python_path: str | None = None):
        self.python_path = python_path or os.environ.get("MELO_PYTHON") or self._default_python()
        self._proc: subprocess.Popen | None = None
        self._lock = threading.Lock()
        self._next_id = 0
        self._pending: dict[int, dict] = {}
        self._stderr_tail: deque[str] = deque(maxlen=80)
        self._reader_started = False
        self._started = False
        self._start_error: str | None = None

    @staticmethod
    def singleton() -> MeloExternalClient:
        with MeloExternalClient._instance_lock:
            if MeloExternalClient._instance is None:
                MeloExternalClient._instance = MeloExternalClient()
                atexit.register(MeloExternalClient._instance.close)
            return MeloExternalClient._instance

    @staticmethod
    def _default_python() -> str:
        candidates = [
            os.path.expanduser("~/miniconda3/envs/melo/bin/python"),
            os.path.expanduser("~/miniconda3/envs/melo/bin/python3"),
            "python3",
        ]
        for candidate in candidates:
            if candidate == "python3" or os.path.isfile(candidate):
                return candidate
        return candidates[0]

    def availability_error(self) -> str | None:
        if not os.path.isfile(self.python_path):
            return (
                "未找到外部 Melo Python 解释器："
                f"{self.python_path}。请确认 melo conda 环境已安装，"
                "或设置环境变量 MELO_PYTHON 指向其 python 可执行文件。"
            )
        if not os.path.isfile(_WORKER_SCRIPT):
            return f"未找到 Melo worker 脚本：{_WORKER_SCRIPT}"
        return None

    def start(self) -> None:
        err = self.availability_error()
        if err:
            raise RuntimeError(err)
        with self._lock:
            if self._proc is not None and self._proc.poll() is None:
                return
            env = os.environ.copy()
            env.setdefault("HF_ENDPOINT", "https://hf-mirror.com")
            try:
                self._proc = subprocess.Popen(
                    [self.python_path, _WORKER_SCRIPT],
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                    bufsize=1,
                    env=env,
                )
            except Exception as exc:  # noqa: BLE001
                self._start_error = str(exc)
                raise RuntimeError(f"无法启动外部 Melo worker：{exc}") from exc
            self._stderr_tail.clear()
            self._reader_started = False
            self._started = True

    def _ensure_readers(self) -> None:
        with self._lock:
            if self._reader_started or self._proc is None:
                return
            self._reader_started = True
        threading.Thread(target=self._read_stdout, daemon=True).start()
        threading.Thread(target=self._read_stderr, daemon=True).start()

    def _read_stdout(self) -> None:
        proc = self._proc
        if proc is None or proc.stdout is None:
            return
        try:
            for raw_line in proc.stdout:
                line = raw_line.strip()
                if not line:
                    continue
                try:
                    msg = json.loads(line)
                except json.JSONDecodeError:
                    continue
                holder = self._pending.pop(msg.get("id"), None)
                if holder is not None:
                    holder["result"] = msg
                    holder["event"].set()
        finally:
            with self._lock:
                pending = list(self._pending.values())
                self._pending.clear()
                if self._proc is proc:
                    self._proc = None
            for holder in pending:
                holder["result"] = {
                    "id": None,
                    "ok": False,
                    "error": "Melo worker 进程已退出",
                }
                holder["event"].set()

    def _read_stderr(self) -> None:
        proc = self._proc
        if proc is None or proc.stderr is None:
            return
        try:
            for raw_line in proc.stderr:
                line = raw_line.rstrip()
                if line:
                    self._stderr_tail.append(line)
        except Exception:  # noqa: BLE001, S110 - stderr 读取失败不影响主流程
            pass

    def synthesize(self, *, text: str, language: str, device: str,
                   speaker: str | None, speed: float, sdp_ratio: float,
                   noise_scale: float, noise_scale_w: float, format: str,
                   output_path: str, timeout: float = 300.0) -> str:
        self.start()
        with self._lock:
            proc = self._proc
            if proc is None or proc.poll() is not None:
                tail = "\n".join(self._stderr_tail)
                raise RuntimeError(
                    "Melo worker 进程未运行。"
                    + (f"\nstderr:\n{tail}" if tail else "")
                )
            rid = self._next_id
            self._next_id += 1
            event = threading.Event()
            holder: dict = {"event": event, "result": None}
            self._pending[rid] = holder
            payload = {
                "id": rid,
                "text": text,
                "language": language,
                "device": device,
                "speaker": speaker,
                "speed": speed,
                "sdp_ratio": sdp_ratio,
                "noise_scale": noise_scale,
                "noise_scale_w": noise_scale_w,
                "format": format,
                "output": output_path,
            }
            try:
                proc.stdin.write(json.dumps(payload, ensure_ascii=False) + "\n")
                proc.stdin.flush()
            except Exception as exc:  # noqa: BLE001
                self._pending.pop(rid, None)
                raise RuntimeError(f"写入 Melo worker 失败：{exc}") from exc

        self._ensure_readers()

        if not event.wait(timeout=timeout):
            with self._lock:
                self._pending.pop(rid, None)
            raise RuntimeError(
                f"Melo 合成超时（{timeout:.0f}s）。首次加载模型约 10~30s，"
                "请确认外部环境可用。"
            )
        result = holder.get("result")
        if not result or not result.get("ok"):
            tail = "\n".join(self._stderr_tail)
            error = (result or {}).get("error") or "未知错误"
            raise RuntimeError(
                f"外部 Melo 合成失败：{error}"
                + (f"\nstderr:\n{tail}" if tail else "")
            )
        return str(result["path"])

    def close(self) -> None:
        with self._lock:
            proc, self._proc = self._proc, None
        if proc is not None and proc.poll() is None:
            try:
                proc.stdin.close()
            except Exception:  # noqa: BLE001, S110 - 关闭 stdin 失败不阻断清理
                pass
            try:
                proc.terminate()
                proc.wait(timeout=3)
            except Exception:  # noqa: BLE001 - 终止失败时继续尝试强杀
                try:
                    proc.kill()
                except Exception:  # noqa: BLE001, S110 - 强杀失败已无法继续清理
                    pass
