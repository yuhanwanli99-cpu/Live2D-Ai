"""
Data model for a single crash report.

All fields are designed to be JSON-serializable and privacy-safe by default.
"""

from __future__ import annotations

import functools
import hashlib
import os
import platform
import sys
import time
import traceback
import uuid
from dataclasses import dataclass, field
from typing import Any

# Sentinel for fields that failed to serialize
_SENTINEL = "<serialization-failed>"


@dataclass
class CrashInfo:
    """Structured crash report payload, ready for JSON serialization."""

    report_id: str
    timestamp: str  # ISO 8601 with timezone
    app_version: str
    crash_type: str  # "uncaught_exception" | "unhandled_promise" | "signal"

    exception_type: str
    exception_message: str
    traceback_frames: list[str]

    # Sanitized local variables per frame (frame_index -> {name: repr(value)})
    locals_per_frame: list[dict[str, str]]

    system_info: dict[str, Any]
    process_info: dict[str, Any]
    recent_logs: list[str]

    # Free-form context the caller can attach
    context: dict[str, Any] = field(default_factory=dict)

    # -- factory -----------------------------------------------------------------

    @classmethod
    def from_exception(
        cls,
        exc: BaseException,
        *,
        app_version: str,
        crash_type: str = "uncaught_exception",
        recent_logs: list[str] | None = None,
        context: dict[str, Any] | None = None,
    ) -> CrashInfo:
        """Build a CrashInfo from an in-flight exception (uses sys.exc_info() internals)."""
        tb = exc.__traceback__
        frames = traceback.extract_tb(tb) if tb else []
        formatted_tb = traceback.format_exception(type(exc), exc, tb)

        # Collect locals per frame (sanitised later by PrivacyFilter)
        locals_per_frame: list[dict[str, str]] = []
        if tb is not None:
            current = tb
            while current is not None:
                frame_locals = {}
                for k, v in current.tb_frame.f_locals.items():
                    try:
                        frame_locals[k] = repr(v)
                    except Exception:
                        frame_locals[k] = _SENTINEL
                locals_per_frame.append(frame_locals)
                current = current.tb_next

        return cls(
            report_id=str(uuid.uuid4()),
            timestamp=cls._utcnow_iso(),
            app_version=app_version,
            crash_type=crash_type,
            exception_type=type(exc).__qualname__,
            exception_message=str(exc),
            traceback_frames=formatted_tb,
            locals_per_frame=locals_per_frame,
            system_info=cls._collect_system_info(),
            process_info=cls._collect_process_info(),
            recent_logs=list(recent_logs or []),
            context=context or {},
        )

    @classmethod
    def from_message(
        cls,
        message: str,
        *,
        app_version: str,
        crash_type: str = "signal",
        recent_logs: list[str] | None = None,
        context: dict[str, Any] | None = None,
    ) -> CrashInfo:
        """Build a CrashInfo from a plain message (e.g. signal handler)."""
        return cls(
            report_id=str(uuid.uuid4()),
            timestamp=cls._utcnow_iso(),
            app_version=app_version,
            crash_type=crash_type,
            exception_type="Signal",
            exception_message=message,
            traceback_frames=[],
            locals_per_frame=[],
            system_info=cls._collect_system_info(),
            process_info=cls._collect_process_info(),
            recent_logs=list(recent_logs or []),
            context=context or {},
        )

    # -- helpers ----------------------------------------------------------------

    @staticmethod
    def _utcnow_iso() -> str:
        from datetime import datetime, timezone

        return datetime.now(timezone.utc).isoformat()

    @staticmethod
    @functools.lru_cache(maxsize=1)
    def _collect_system_info() -> dict[str, Any]:
        import shutil

        info: dict[str, Any] = {
            "platform": sys.platform,
            "python_version": sys.version,
            "python_implementation": platform.python_implementation(),
            "machine": platform.machine(),
            "processor": platform.processor(),
            "cpu_count": os.cpu_count(),
        }
        try:
            total, used, free = shutil.disk_usage(os.getcwd())
            info["disk_total_gb"] = round(total / (1024**3), 2)
            info["disk_free_gb"] = round(free / (1024**3), 2)
        except Exception:
            pass
        return info

    @staticmethod
    def _collect_process_info() -> dict[str, Any]:
        pid = os.getpid()
        info: dict[str, Any] = {
            "pid": pid,
            "uptime_seconds": round(time.time() - _PROCESS_START_TIME, 1),
        }
        # Optionally enrich with psutil (already in dependency tree via other pkgs)
        try:
            import psutil

            proc = psutil.Process(pid)
            mem = proc.memory_info()
            info["memory_rss_mb"] = round(mem.rss / (1024 * 1024), 2)
            info["memory_vms_mb"] = round(mem.vms / (1024 * 1024), 2)
            info["cpu_percent"] = proc.cpu_percent(interval=0.05)
        except Exception:
            pass
        return info

    # -- serialization ----------------------------------------------------------

    @staticmethod
    def _hash_hostname(hostname: str) -> str:
        return hashlib.sha256(hostname.encode()).hexdigest()[:8]

    def to_dict(self) -> dict[str, Any]:
        """Convert to JSON-safe dict."""
        return {
            "report_id": self.report_id,
            "timestamp": self.timestamp,
            "app_version": self.app_version,
            "crash_type": self.crash_type,
            "exception": {
                "type": self.exception_type,
                "message": self.exception_message,
                "traceback": [line.rstrip("\n") for line in self.traceback_frames],
            },
            "locals_per_frame": self.locals_per_frame,
            "system_info": self.system_info,
            "process_info": self.process_info,
            "recent_logs": self.recent_logs,
            "context": self.context,
        }

    def to_json(self, indent: int | None = 2) -> str:
        import json

        return json.dumps(self.to_dict(), indent=indent, default=str, ensure_ascii=False)
