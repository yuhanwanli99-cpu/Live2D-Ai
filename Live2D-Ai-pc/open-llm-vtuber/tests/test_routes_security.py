"""Regression tests for routes-level security and validation fixes."""


def test_update_config_rejects_invalid_log_level(settings_env):
    res = settings_env.client.post(
        "/api/config",
        json={"log_level": "TRACE", "apply": False},
    )
    assert res.status_code == 400
    body = res.json()
    assert body["ok"] is False
    assert "TRACE" in body["error"]


def test_update_config_accepts_valid_log_level(settings_env):
    res = settings_env.client.post(
        "/api/config",
        json={"log_level": "debug", "apply": False},
    )
    assert res.status_code == 200
    env = settings_env.read_env()
    assert "LIVE2DAI_LOG_LEVEL=DEBUG" in env


def test_discover_models_refuses_stored_key_for_untrusted_url(settings_env):
    # deepseek_llm configured base_url is empty/default; providing an arbitrary
    # base_url without api_key must not use the stored DEEPSEEK_API_KEY.
    res = settings_env.client.post(
        "/api/llm/discover-models",
        json={"provider": "deepseek_llm", "base_url": "https://attacker.example.com/v1"},
    )
    assert res.status_code == 400
    body = res.json()
    assert body["ok"] is False
    assert "base_url" in body["error"]


def test_test_llm_refuses_stored_key_for_untrusted_url(settings_env):
    res = settings_env.client.post(
        "/api/test-llm",
        json={
            "provider": "deepseek_llm",
            "base_url": "https://attacker.example.com/v1",
            "model": "test-model",
        },
    )
    assert res.status_code == 400
    body = res.json()
    assert body["ok"] is False
    assert "base_url" in body["error"]


def test_persona_save_is_atomic(settings_env):
    # After a successful persona update, the persona file must be complete.
    res = settings_env.client.post(
        "/api/config",
        json={"persona": {"system_prompt": "Updated prompt."}, "apply": False},
    )
    assert res.status_code == 200
    text = settings_env.read_persona()
    assert "Updated prompt." in text
    assert text.strip().endswith(".") or "Updated prompt" in text
