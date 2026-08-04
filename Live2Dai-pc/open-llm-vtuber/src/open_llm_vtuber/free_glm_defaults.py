# -*- coding: utf-8 -*-
"""Live2Dai zero-config default provider: built-in shared free GLM key + resolver.

Background: on Android the shared free GLM-4.7-Flash key is baked into BuildConfig at
build time (sourced from the gitignored local.properties), giving an out-of-the-box chat
experience with no API key setup. The PC side is pure Python with no build step, so we
provide the equivalent here: when the configured llm_api_key is empty or is still an
unresolved ${ZHIPU_API_KEY} placeholder (env var not set -- read_yaml keeps the literal
text), we fall back to the shared free key below -- identical to the Android key -- so
users can chat immediately without first applying for a key.

Upgrade path: set the ZHIPU_API_KEY env var (or edit llm_api_key in conf.yaml) to use
your own key instead (personal / higher-concurrency scenarios).

NOTE: never print this key to logs or test output.
"""

# Shared free GLM-4.7-Flash key (same value as ZHIPU_API_KEY in Android local.properties).
# Free tier: 1 concurrent request per account; not suited for high-concurrency usage.
SHARED_FREE_GLM_API_KEY = "96881db73f364bae9b75b4754120fdbc.ZFadnPqVoCSygGa8"


def resolve_llm_api_key(raw_key):
    """Return the effective API key.

    - raw_key empty/None          -> built-in shared free key
    - raw_key is an unresolved ${VAR} placeholder (env unset, kept literal by read_yaml)
                                  -> built-in shared free key
    - anything else               -> returned unchanged (user-supplied key, e.g. env override)
    """
    stripped = (raw_key or "").strip()
    if not stripped:
        return SHARED_FREE_GLM_API_KEY
    if stripped.startswith("${") and stripped.endswith("}"):
        return SHARED_FREE_GLM_API_KEY
    return stripped
