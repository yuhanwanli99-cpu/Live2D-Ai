from typing import Any


def is_trusted_base_url(base_url: str, provider: str, cfg: dict[str, Any], default: dict[str, Any]) -> bool:
    """仅当 base_url 与已配置/默认 URL 匹配时才允许使用存储的 Key。

    防止客户端传入任意 base_url 触发 SSRF / 密钥外泄。
    """
    if not base_url:
        return False
    candidates = {
        (cfg.get("base_url") or "").strip().rstrip("/"),
        (default.get("base_url") or "").strip().rstrip("/"),
    }
    return base_url.rstrip("/") in candidates
