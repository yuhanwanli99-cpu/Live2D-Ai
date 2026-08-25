import json

from open_llm_vtuber.services.model_service import ModelService


class DummyConfig:
    pass


def test_get_models_reports_missing_optional_asset_unavailable(tmp_path, monkeypatch):
    shared = tmp_path / "shared"
    shared.mkdir()
    (shared / "model_registry.json").write_text(
        json.dumps({"models": [{"id": "bai", "model3JsonPath": "bai/runtime/bai.model3.json", "available": True}]}),
        encoding="utf-8",
    )
    work = tmp_path / "pc"
    work.mkdir()
    monkeypatch.chdir(work)
    service = ModelService(str(tmp_path), DummyConfig())
    assert service.get_models()["models"][0]["available"] is False


def test_get_models_reports_installed_asset_available(tmp_path, monkeypatch):
    shared = tmp_path / "shared"
    shared.mkdir()
    (shared / "model_registry.json").write_text(
        json.dumps({"models": [{"id": "local", "model3JsonPath": "local/local.model3.json", "available": False}]}),
        encoding="utf-8",
    )
    work = tmp_path / "pc"
    model = work / "live2d-models" / "local" / "local.model3.json"
    model.parent.mkdir(parents=True)
    model.write_text("{}", encoding="utf-8")
    monkeypatch.chdir(work)
    service = ModelService(str(tmp_path), DummyConfig())
    assert service.get_models()["models"][0]["available"] is True
