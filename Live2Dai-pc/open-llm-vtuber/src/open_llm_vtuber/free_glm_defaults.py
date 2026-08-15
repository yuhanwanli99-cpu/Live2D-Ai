# -*- coding: utf-8 -*-
"""Live2Dai LLM API key resolver (spoq-fix: no built-in shared key).

Background: keys are NOT committed (personal/local policy). The effective key must come
from the user's local configuration:
  - env var (e.g. ZHIPU_API_KEY / DEEPSEEK_API_KEY in a gitignored `.env` at repo root,
    loaded by run_server.py via python-dotenv), or
  - conf.yaml `llm_api_key` literal.

When the configured value is empty / None / an unresolved `${VAR}` placeholder (env var
not set -- read_yaml keeps the literal text), we now raise a clear ValueError with
configuration guidance instead of silently falling back to a built-in key.
"""


def resolve_llm_api_key(raw_key):
    """Return the effective API key.

    - raw_key empty/None          -> raise ValueError (with .env / conf.yaml guidance)
    - raw_key is an unresolved ${VAR} placeholder (env unset, kept literal by read_yaml)
                                  -> raise ValueError (with .env / conf.yaml guidance)
    - anything else               -> returned unchanged (user-supplied key, e.g. env override)

    Raises:
        ValueError: message contains ".env" and "conf.yaml" guidance keywords.
    """
    stripped = (raw_key or "").strip()
    if not stripped or (stripped.startswith("${") and stripped.endswith("}")):
        raise ValueError(
            "未配置 LLM API key：请在仓库根 .env 设置 ZHIPU_API_KEY（或 DEEPSEEK_API_KEY），"
            "或在 conf.yaml 的 llm_api_key 填写你的 key（本地配置，禁止提交）。"
        )
    return stripped
