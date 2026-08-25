
_VALID_LOG_LEVELS = frozenset({"DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"})


def validate_log_level(level: str | None) -> str | None:
    """校验日志级别为合法值；无效时返回错误提示。"""
    if not level:
        return None
    normalized = str(level).strip().upper()
    if normalized not in _VALID_LOG_LEVELS:
        return f"非法日志级别 '{level}'，允许: {', '.join(sorted(_VALID_LOG_LEVELS))}"
    return None
