"""Regression tests for non-destructive, transactional configuration upgrades."""

import logging
from pathlib import Path

import pytest
from ruamel.yaml import YAML

from upgrade_codes.config_sync import ConfigSynchronizer


def _write(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")


def _load(path: Path):
    yaml = YAML()
    return yaml.load(path.read_text(encoding="utf-8"))


def _synchronizer(
    tmp_path: Path, user_text: str, default_text: str
) -> ConfigSynchronizer:
    user = tmp_path / "conf.yaml"
    default = tmp_path / "conf.default.yaml"
    backup = tmp_path / "conf.yaml.backup"
    _write(user, user_text)
    _write(default, default_text)
    sync = ConfigSynchronizer("en", logging.getLogger("test-config-upgrade"))
    sync.user_path = str(user)
    sync.default_path = str(default)
    sync.backup_path = str(backup)
    return sync


def test_merge_adds_defaults_and_preserves_fork_and_plugin_keys(tmp_path, monkeypatch):
    sync = _synchronizer(
        tmp_path,
        """# user comment must survive
system_config:
  conf_version: 'v1.2.1'
  live2d_fps: 120
character_config:
  load_persona_from_file: 'shared/persona.yaml'
  agent_config:
    llm_configs:
      deepseek_llm:
        llm_api_key: '${DEEPSEEK_API_KEY}'
        thinking_enabled: true
plugin_custom:
  enabled: true
mcp_servers: 'shared/mcp_tools.json'
vision_config:
  vision_enabled: true
""",
        """system_config:
  conf_version: 'v1.2.1'
  host: '127.0.0.1'
character_config:
  agent_config:
    llm_configs:
      deepseek_llm:
        temperature: 0.8
""",
    )
    monkeypatch.setattr(sync, "compare_comments", lambda: True)
    sync.update_user_config()
    result = _load(Path(sync.user_path))
    assert result["system_config"]["host"] == "127.0.0.1"
    assert result["system_config"]["live2d_fps"] == 120
    assert result["character_config"]["load_persona_from_file"] == "shared/persona.yaml"
    deepseek = result["character_config"]["agent_config"]["llm_configs"]["deepseek_llm"]
    assert deepseek["temperature"] == 0.8
    assert deepseek["thinking_enabled"] is True
    assert deepseek["llm_api_key"] == "${DEEPSEEK_API_KEY}"
    assert result["plugin_custom"] == {"enabled": True}
    assert result["mcp_servers"] == "shared/mcp_tools.json"
    assert result["vision_config"]["vision_enabled"] is True
    assert "user comment must survive" in Path(sync.user_path).read_text(
        encoding="utf-8"
    )
    assert Path(sync.backup_path).exists()


def test_extra_keys_alone_do_not_trigger_merge_or_deletion(tmp_path, monkeypatch):
    sync = _synchronizer(
        tmp_path,
        """system_config:
  conf_version: 'v1.2.1'
custom_future_key:
  nested: keep-me
""",
        """system_config:
  conf_version: 'v1.2.1'
""",
    )
    monkeypatch.setattr(sync, "compare_comments", lambda: True)
    monkeypatch.setattr(
        sync,
        "merge_and_update_user_config",
        lambda: pytest.fail("extra keys must not make config stale"),
    )
    sync.update_user_config()
    assert _load(Path(sync.user_path))["custom_future_key"]["nested"] == "keep-me"


def test_failed_update_restores_exact_backup_and_reraises(tmp_path, monkeypatch):
    original = """# original
system_config:
  conf_version: 'v1.2.1'
fork_key: keep
"""
    sync = _synchronizer(
        tmp_path,
        original,
        """system_config:
  conf_version: 'v1.2.1'
  host: '127.0.0.1'
""",
    )

    def fail_after_partial_write():
        Path(sync.user_path).write_text("corrupted: true\n", encoding="utf-8")
        raise RuntimeError("simulated update failure")

    monkeypatch.setattr(sync, "compare_field_keys", lambda: False)
    monkeypatch.setattr(sync, "merge_and_update_user_config", fail_after_partial_write)
    with pytest.raises(RuntimeError, match="simulated update failure"):
        sync.update_user_config()
    assert Path(sync.user_path).read_text(encoding="utf-8") == original
    assert Path(sync.backup_path).read_text(encoding="utf-8") == original


def test_destructive_template_cleanup_is_disabled(tmp_path):
    sync = _synchronizer(
        tmp_path,
        "system_config: {conf_version: 'v1.2.1'}\ncustom: keep\n",
        "system_config: {conf_version: 'v1.2.1'}\n",
    )
    with pytest.raises(RuntimeError, match="Refusing to delete"):
        sync.delete_extra_keys()
    assert _load(Path(sync.user_path))["custom"] == "keep"


def test_repository_config_merge_preserves_every_existing_value(tmp_path):
    """The shipped fork config may grow ahead of the upstream template."""
    project_root = Path(__file__).resolve().parents[1]
    user_text = (project_root / "conf.yaml").read_text(encoding="utf-8")
    default_text = (project_root / "config_templates" / "conf.default.yaml").read_text(
        encoding="utf-8"
    )
    sync = _synchronizer(tmp_path, user_text, default_text)
    before = _load(Path(sync.user_path))

    def leaves(value, path=()):
        if isinstance(value, dict):
            result = {}
            for key, child in value.items():
                result.update(leaves(child, (*path, str(key))))
            return result
        return {path: value}

    before_leaves = leaves(before)
    sync.merge_configs()
    after_leaves = leaves(_load(Path(sync.user_path)))

    assert before_leaves
    assert set(before_leaves).issubset(after_leaves)
    for path, value in before_leaves.items():
        assert after_leaves[path] == value, (
            "existing config value changed: " + ".".join(path)
        )
