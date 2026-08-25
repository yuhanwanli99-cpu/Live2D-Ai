"""PC Preview secure-default release contracts."""

from pathlib import Path

import yaml

ROOT = Path(__file__).parents[1]


def test_runtime_config_binds_to_loopback_by_default():
    config = yaml.safe_load((ROOT / "conf.yaml").read_text(encoding="utf-8"))
    assert config["system_config"]["host"] in {"127.0.0.1", "localhost", "::1"}


def test_server_uses_configured_host_not_hardcoded_all_interfaces():
    source = (ROOT / "run_server.py").read_text(encoding="utf-8")
    assert 'host="0.0.0.0"' not in source
    assert "host=listen_host" in source
    assert 'config.system_config.host or "127.0.0.1"' in source


def test_server_creates_model_import_directory_for_asset_free_preview():
    source = (ROOT / "src/open_llm_vtuber/server.py").read_text(encoding="utf-8")
    assert 'os.makedirs("live2d-models")' in source
