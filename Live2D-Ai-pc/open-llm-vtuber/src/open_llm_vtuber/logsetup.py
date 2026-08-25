# -*- coding: utf-8 -*-
"""统一日志配置：可读的控制台输出 + 带诊断细节的滚动文件。

设计（对齐 docs/architecture/observability.md §3 关键路径日志建议）：
- 控制台（stderr）：紧凑单行「HH:mm:ss | 级别 | 标签 │ 消息」，彩色；
  只显示 LIVE2DAI_LOG_LEVEL（默认 INFO）以上——运行态一眼能读。
- 文件（logs/server.log）：完整诊断上下文（毫秒时间戳 + 模块:函数:行号），
  10MB 轮转、保留 5 份、UTF-8——排障时回放全貌。logs/ 已在 .gitignore。
- uvicorn.access 访问日志逐请求刷屏：非 DEBUG 时压到 WARNING。
- 各模块用 get_logger("标签") 取带标签的 logger，未绑定的记录落 "app"。

用法：
    from open_llm_vtuber.logsetup import setup_logging, get_logger

    setup_logging()            # 进程入口调用一次
    log = get_logger("tts")    # 模块级取 logger
    log.info("入队 seq={} text={!r}", seq, text[:30])
"""

from __future__ import annotations

import logging
import os
import re
import sys
from pathlib import Path

from loguru import logger

_VALID_LEVELS = ("DEBUG", "INFO", "WARNING", "ERROR")

# 高频轮询 GET 端点：其逐请求 access INFO 是纯噪音，INFO 下静默（DEBUG 全放）。
# 只抑制这些轮询路径，其余 access（含 4xx/5xx 记录）与 app 错误日志均保留。
_POLLING_PATHS = frozenset({"/api/external/status"})


class _DropPollingAccess(logging.Filter):
    """INFO 下丢弃高频轮询的 GET access 记录；DEBUG 全部放行；错误级别始终放行。"""

    def __init__(self, enabled: bool) -> None:
        super().__init__("live2d-drops-polling-access")
        self._enabled = enabled

    def filter(self, record: logging.LogRecord) -> bool:
        if not self._enabled or record.levelno >= logging.WARNING:
            return True
        m = re.search(r'"(GET|POST) ([^ ]+) HTTP', record.getMessage())
        if m and m.group(1) == "GET" and m.group(2) in _POLLING_PATHS:
            return False
        return True

# 控制台：人读——短时间 + 对齐级别 + 标签 + 消息（彩色由 loguru <tags> 渲染）
_CONSOLE_FORMAT = (
    "<green>{time:HH:mm:ss}</green> | "
    "<level>{level: <7}</level> | "
    "<cyan>{extra[tag]}</cyan> │ "
    "<level>{message}</level>"
)

# 文件：机器回放——毫秒时间戳 + 全定位 + 标签（对齐）+ 消息
_FILE_FORMAT = (
    "{time:YYYY-MM-DD HH:mm:ss.SSS} | "
    "{level: <7} | "
    "{extra[tag]: <7} | "
    "{name}:{function}:{line} - "
    "{message}"
)


def _resolve_level(level: str | None) -> tuple[str, str | None]:
    """解析生效级别；(生效值, 非法输入原值或 None)。非法一律回落 INFO 并可见。"""
    raw = (level or os.getenv("LIVE2DAI_LOG_LEVEL") or "INFO").strip()
    upper = raw.upper()
    if upper not in _VALID_LEVELS:
        return "INFO", raw  # 保留用户原始写法，便于在提示里对号入座
    return upper, None


def setup_logging(
    level: str | None = None,
    log_dir: str | Path = "logs",
    console: bool = True,
    file_sink: bool = True,
) -> str:
    """安装统一的控制台 + 文件日志 sink（进程入口调用一次，幂等可重复调）。

    Args:
        level: 显式级别；缺省读环境变量 LIVE2DAI_LOG_LEVEL，再缺省 INFO。
        log_dir: 文件日志目录（默认 logs/，相对 cwd；已 gitignore）。
        console: 是否装 stderr 彩色紧凑 sink。
        file_sink: 是否装 logs/server.log 滚动文件 sink。

    Returns:
        实际生效的日志级别字符串（便于启动横幅回显与测试断言）。
    """
    effective, invalid = _resolve_level(level)
    if invalid is not None:
        # 此时还没装任何 sink：用 stderr 兜底提示一次
        print(f"[Live2DAI] 未知的 LIVE2DAI_LOG_LEVEL={invalid!r}，回落 INFO", file=sys.stderr)

    logger.remove()
    # 未 bind(tag=...) 的记录统一落 "app"，避免格式化缺 key
    logger.configure(extra={"tag": "app"})

    if console:
        logger.add(
            sys.stderr,
            level=effective,
            format=_CONSOLE_FORMAT,
            filter=lambda record: True,
            backtrace=False,
            diagnose=False,
        )
    if file_sink:
        log_path = Path(log_dir)
        try:
            log_path.mkdir(parents=True, exist_ok=True)
            logger.add(
                log_path / "server.log",
                level=effective,
                format=_FILE_FORMAT,
                rotation="10 MB",
                retention=5,
                encoding="utf-8",
                backtrace=True,   # 文件里保留完整堆栈（排障用）
                diagnose=False,   # 不泄露变量值（避免把 Key 打进文件）
            )
        except OSError as exc:
            # 目录不可写时文件降级为不启用，控制台不受影响
            print(f"[LiveDAI] 日志目录不可写（{exc}），仅使用控制台日志", file=sys.stderr)

    _tame_uvicorn_access(effective)
    return effective


def _tame_uvicorn_access(level: str) -> None:
    """压制 uvicorn 访问日志逐请求刷屏（可重复调用，幂等）。

    - 非 DEBUG：uvicorn.access 压到 WARNING 起，并额外挂一个轮询端点过滤器
      （即使 DEBUG 中途放开到 INFO，高频 /api/external/status 等 GET 也不会刷屏）。
    - DEBUG：uvicorn.access 放开到 INFO；轮询过滤器关闭，全部 access 可见。
    """
    access = logging.getLogger("uvicorn.access")
    is_debug = level == "DEBUG"
    access.setLevel(logging.DEBUG if is_debug else logging.WARNING)
    # 幂等：移除旧过滤器再挂新（同一实例只挂一个）。
    for existing in list(access.filters):
        if isinstance(existing, _DropPollingAccess):
            access.removeFilter(existing)
    access.addFilter(_DropPollingAccess(enabled=not is_debug))


def apply_uvicorn_access_policy(level: str) -> None:
    """在 uvicorn 完成其自身 logger 配置之后再调用，重新施加访问日志压制。

    uvicorn.Config(log_level=...) 在服务启动时会覆盖 uvicorn.access 级别，
    可能冲掉 setup_logging() 的压制；在创建 config/server 之后调用本函数即可兜底。
    """
    _tame_uvicorn_access(level)


def get_logger(tag: str):
    """取带模块标签的 loguru logger（控制台按标签着色分组，文件按标签检索）。"""
    return logger.bind(tag=tag)
