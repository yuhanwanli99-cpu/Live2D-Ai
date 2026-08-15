"""
Crash Reporter Module
=====================
Automatic crash detection, collection, and reporting for Live2Dai.

Provides:
- Global exception hook for uncaught exceptions
- Async event loop exception handler
- Structured JSON crash reports with system info, traceback, and sanitized locals
- Configurable reporting pipeline: real-time HTTP POST + retry + offline cache
- Sensitive data redaction (API keys, tokens, passwords)
- FastAPI/ASGI middleware for request-level crash capture
- Low-overhead design with fire-and-forget semantics

Usage:
    from .crash_reporter import CrashReporter

    reporter = CrashReporter(
        report_url="https://your-endpoint/api/crash",
        app_version="1.2.1",
    )
    reporter.install()  # Hook into sys.excepthook + asyncio
"""

from .reporter import CrashReporter
from .collector import CrashInfo
from .privacy import PrivacyFilter

__all__ = [
    "CrashReporter",
    "CrashInfo",
    "PrivacyFilter",
]
