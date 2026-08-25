"""统一日志配置（logsetup）测试：格式、级别过滤、标签、uvicorn 降噪、幂等。

契约：
- 文件 sink 落地 logs/server.log，含毫秒时间戳 / 级别 / 标签 / 定位；
- get_logger(tag) 的记录在文件中可按标签检索；
- INFO 生效时 DEBUG 不落盘；非法级别回落 INFO（不抛异常）；
- 非 DEBUG 时 uvicorn.access 压到 WARNING，DEBUG 时放开到 INFO；
- setup_logging 可重复调用（幂等，不叠加重复行）。
"""

import logging

import pytest
from loguru import logger

from open_llm_vtuber.logsetup import (
    apply_uvicorn_access_policy,
    get_logger,
    setup_logging,
)


@pytest.fixture()
def log_env(tmp_path, monkeypatch):
    """隔离的日志目录 + 清理全局 loguru 状态。"""
    monkeypatch.chdir(tmp_path)
    monkeypatch.delenv("LIVE2DAI_LOG_LEVEL", raising=False)
    yield tmp_path
    logger.remove()


def _read_log(tmp_path):
    return (tmp_path / "logs" / "server.log").read_text(encoding="utf-8")


def test_file_sink_contains_tag_level_position(log_env):
    setup_logging(level="DEBUG", log_dir=log_env / "logs")
    get_logger("tts").info("入队 seq={} text={!r}", 3, "你好")

    content = _read_log(log_env)
    assert "入队 seq=3 text='你好'" in content
    assert "| tts     |" in content  # 标签列（左对齐宽度 7，原样小写）
    assert "| INFO    |" in content
    assert "test_logsetup:test_file_sink_contains_tag_level_position" in content  # 模块:函数:行 定位进文件


def test_info_level_filters_debug(log_env):
    setup_logging(level="INFO", log_dir=log_env / "logs")
    logger.debug("这条不该出现")
    get_logger("app").info("这条应该出现")

    content = _read_log(log_env)
    assert "这条应该出现" in content
    assert "这条不该出现" not in content


def test_invalid_level_falls_back_to_info(log_env, capsys):
    effective = setup_logging(level="banana", log_dir=log_env / "logs")
    assert effective == "INFO"
    # 非法值提示可见（stderr 兜底 print）
    assert "banana" in capsys.readouterr().err


def test_level_from_env(log_env, monkeypatch):
    monkeypatch.setenv("LIVE2DAI_LOG_LEVEL", "warning")
    assert setup_logging(log_dir=log_env / "logs") == "WARNING"


def test_uvicorn_access_tamed_unless_debug(log_env, monkeypatch):
    monkeypatch.setenv("LIVE2DAI_LOG_LEVEL", "INFO")
    setup_logging(log_dir=log_env / "logs")
    assert logging.getLogger("uvicorn.access").level == logging.WARNING

    monkeypatch.setenv("LIVE2DAI_LOG_LEVEL", "DEBUG")
    setup_logging(log_dir=log_env / "logs")
    assert logging.getLogger("uvicorn.access").level == logging.DEBUG


def test_setup_is_idempotent(log_env):
    setup_logging(level="DEBUG", log_dir=log_env / "logs")
    get_logger("flow").info("第一遍")
    setup_logging(level="DEBUG", log_dir=log_env / "logs")  # 重新安装不叠加 handler
    get_logger("flow").info("第二遍")

    content = _read_log(log_env)
    assert content.count("第一遍") == 1
    assert content.count("第二遍") == 1


def test_unbound_records_use_app_tag(log_env):
    setup_logging(level="DEBUG", log_dir=log_env / "logs")
    logger.info("未绑定标签的记录")
    assert "| app     |" in _read_log(log_env)


def test_polling_access_filtered_under_info():
    """INFO 下：/api/external/status 轮询 access 记录被过滤器丢弃；其它 access 放行。"""
    from open_llm_vtuber.logsetup import _DropPollingAccess

    flt = _DropPollingAccess(enabled=True)
    logging.getLogger("uvicorn.access").addFilter(flt)
    try:
        access = logging.getLogger("uvicorn.access")
        poll = access.makeRecord(
            "uvicorn.access", logging.INFO, __file__, 1,
            '127.0.0.1:49160 - "GET /api/external/status HTTP/1.1" 200 OK',
            (), None,
        )
        keep = access.makeRecord(
            "uvicorn.access", logging.INFO, __file__, 1,
            '127.0.0.1:49160 - "GET /api/config HTTP/1.1" 200 OK',
            (), None,
        )
        assert flt.filter(poll) is False
        assert flt.filter(keep) is True
    finally:
        logging.getLogger("uvicorn.access").removeFilter(flt)


def test_polling_access_kept_under_debug():
    """DEBUG 下轮询 access 记录放行。"""
    from open_llm_vtuber.logsetup import _DropPollingAccess

    flt = _DropPollingAccess(enabled=False)
    access = logging.getLogger("uvicorn.access")
    poll = access.makeRecord(
        "uvicorn.access", logging.INFO, __file__, 1,
        '127.0.0.1:49160 - "GET /api/external/status HTTP/1.1" 200 OK',
        (), None,
    )
    assert flt.filter(poll) is True


def test_apply_uvicorn_access_policy_reaplies_tame(log_env):
    """uvicorn 重置 uvicorn.access 后，apply 能重新压回 WARNING（对应用户看到的刷屏根因）。"""
    from open_llm_vtuber.logsetup import apply_uvicorn_access_policy

    access = logging.getLogger("uvicorn.access")
    access.setLevel(logging.INFO)  # 模拟 uvicorn.Config(log_level=info) 重置
    apply_uvicorn_access_policy("INFO")
    assert access.level == logging.WARNING
    assert any(
        isinstance(f, type(None)) is False for f in access.filters
    )
