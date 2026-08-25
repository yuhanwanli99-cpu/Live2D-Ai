"""Focused unit tests for the crash reporter collector (CrashInfo).

The ``crash_reporter`` package ``__init__`` currently references modules that
are not present in this tree (``reporter`` / ``privacy``), so the package
cannot be imported through its normal package path. The collector itself is
stdlib-only and self-contained; these tests load it directly with importlib to
keep the focus on the collector logic (including the ``_PROCESS_START_TIME``
uptime computation).
"""

from __future__ import annotations

import importlib.util
import json
import sys
import time
from pathlib import Path

_COLLECTOR_PATH = (
    Path(__file__).resolve().parents[1]
    / "src"
    / "open_llm_vtuber"
    / "crash_reporter"
    / "collector.py"
)


def _load_collector():
    spec = importlib.util.spec_from_file_location(
        "crash_reporter_collector", _COLLECTOR_PATH
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    # Register in sys.modules so @dataclass (which looks up the module's
    # __dict__ by module name) and relative lookups work correctly.
    sys.modules[module.__name__] = module
    spec.loader.exec_module(module)
    return module


collector = _load_collector()
CrashInfo = collector.CrashInfo


def test_process_start_time_defined_and_recent():
    # _PROCESS_START_TIME is the F821 regression guard: it must exist and be an
    # epoch timestamp taken at module import (i.e. not in the future, and
    # within a few seconds of "now" since the module was just loaded).
    start = collector._PROCESS_START_TIME
    assert isinstance(start, float)
    assert 0 < time.time() - start < 60


def test_from_exception_uptime_is_positive_and_finite():
    try:
        raise ValueError("boom")
    except ValueError as exc:
        info = CrashInfo.from_exception(exc, app_version="1.2.1")

    assert info.report_id
    assert info.crash_type == "uncaught_exception"
    assert info.exception_type == "ValueError"
    assert info.exception_message == "boom"
    assert info.traceback_frames
    assert info.app_version == "1.2.1"

    uptime = info.process_info["uptime_seconds"]
    assert isinstance(uptime, float)
    assert uptime >= 0
    assert uptime < 60  # module just loaded in this test process


def test_from_exception_system_info_present():
    try:
        raise RuntimeError("sys")
    except RuntimeError as exc:
        info = CrashInfo.from_exception(exc, app_version="1.2.1")
    assert info.system_info["python_version"]
    assert info.system_info["platform"]
    assert info.process_info["pid"] >= 0


def test_from_message_signal_report():
    info = CrashInfo.from_message(
        "SIGTERM received", app_version="1.2.1", crash_type="signal"
    )
    assert info.crash_type == "signal"
    assert info.exception_type == "Signal"
    assert info.traceback_frames == []
    assert info.locals_per_frame == []
    assert info.process_info["uptime_seconds"] >= 0


def test_to_dict_and_to_json_roundtrip():
    info = CrashInfo.from_message("test", app_version="1.2.1")
    payload = info.to_dict()
    assert payload["report_id"] == info.report_id
    assert payload["exception"]["type"] == "Signal"
    assert payload["process_info"]["pid"] >= 0

    text = info.to_json()
    parsed = json.loads(text)
    assert parsed["report_id"] == info.report_id
    assert parsed["process_info"]["uptime_seconds"] >= 0


def test_from_exception_locals_captured():
    def inner():
        secret = "abc"
        assert len(secret) == 3  # read so the local is intentionally captured
        raise KeyError("k")

    try:
        inner()
    except KeyError as exc:
        info = CrashInfo.from_exception(exc, app_version="1.2.1")

    # At least one frame should carry locals (the inner() frame includes
    # `secret`); privacy filtering is a separate layer, not asserted here.
    assert any("secret" in frame for frame in info.locals_per_frame)