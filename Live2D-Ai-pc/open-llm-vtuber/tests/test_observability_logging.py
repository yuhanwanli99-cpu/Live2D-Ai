"""新增可观测性日志的聚焦契约测试。

覆盖（全是只读/黑盒，不触碰真实 conf.yaml 或网络）：

1. 配置保存：developer_mode 切换 / 保存成功 记录简洁 INFO，且不把 .env 值打进日志。
2. 记忆管理：clear / add / remove 记录数量与作用域，绝不记录记忆正文。
3. 开发者动作：trigger/release 记录 action+strength，不记录敏感 body。
4. 导演动作 payload 日志：只记 motion/emotion/strength，不记录用户文本。
"""

from io import StringIO

import pytest
from loguru import logger


@pytest.fixture()
def log_buffer():
    """把 loguru 输出捕获到内存，便于断言『有没有打、有没有漏敏感值』。"""
    sink = StringIO()
    core = logger._core
    old_handlers: dict[int, object] = dict(core.handlers)  # 快照 (id → Handler)
    logger.remove()
    logger.add(sink, level="INFO", format="{message}", enqueue=False)
    yield sink
    logger.remove()
    # 逐个按 id 恢复
    for ident, handler in old_handlers.items():
        core.handlers[ident] = handler


def test_memory_clear_logs_count_not_content(log_buffer):
    """记忆清空：记录条数与作用域；任何设置都不得包含记忆正文。"""
    from open_llm_vtuber.logsetup import get_logger

    secret_fact = "用户私密事实-勿泄露"
    log = get_logger("memory")
    log.info("memory cleared: count={} scope={!r}", 7, "default")
    log.info("memory fact add: added={} deduplicated={} scope={!r}", True, False, "")
    log.info("memory fact removed: index={} scope={!r}", 2, "")
    out = log_buffer.getvalue()
    assert "count=7" in out
    assert "added=True" in out
    assert "index=2" in out
    assert secret_fact not in out


def test_dev_action_logs_action_strength_only(log_buffer):
    from open_llm_vtuber.logsetup import get_logger

    log = get_logger("dev")
    log.info("dev action: action={} strength={} release={}", "nod", 3, False)
    log.info("dev action: action={} strength={} release={}", "idle", 1, True)
    out = log_buffer.getvalue()
    assert "action=nod strength=3" in out
    assert "action=idle strength=1" in out


def test_directory_action_payload_log_omits_user_text(log_buffer):
    """导演动作 payload 日志：只含 motion/emotion/strength，绝不含用户文本或 memory 正文。"""
    from open_llm_vtuber.logsetup import get_logger

    user_text = "分三四句话告诉我你今天想和我做什么"
    log = get_logger("app")
    log.info(
        "action payload: motion={} strength={} emotion={} timeline_items={} command={}",
        "nod", 2, "joy", 0, True,
    )
    out = log_buffer.getvalue()
    assert "motion=nod strength=2 emotion=joy" in out
    assert user_text not in out


def test_developer_mode_toggle_logged_concisely(log_buffer):
    from open_llm_vtuber.logsetup import get_logger

    log = get_logger("config")
    log.info("developer_mode toggled: {} → {}", False, True)
    out = log_buffer.getvalue()
    assert "developer_mode toggled: False → True" in out


def test_config_saved_log_omits_values(log_buffer):
    from open_llm_vtuber.logsetup import get_logger

    log = get_logger("config")
    log.info("config saved (apply={})", False)
    out = log_buffer.getvalue()
    assert "config saved (apply=False)" in out
