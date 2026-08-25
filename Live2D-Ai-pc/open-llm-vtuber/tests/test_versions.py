# -*- coding: utf-8 -*-
"""版本可观测性测试：统一 /api/version、/health 与 versions 模块。

覆盖：
- /api/version 返回 product_version / backend_version / frontend_version / commit；
- product_version 恒为单一 Preview 版本 v0.2.0-preview.1（不与 conf schema 混淆）；
- /health 上报 product_version 等；
- /api/frontend-version 兼容端点仍在；
- versions.format_banner 稳定可解析；
- commit 取不到时返回 "unknown"（mock 失败路径）。
"""

from __future__ import annotations

import pytest


def _build_app(tmp_path):
    """构造同时含 init_config_routes + init_frontend_version_routes 的隔离应用。"""
    from fastapi import FastAPI
    from fastapi.testclient import TestClient

    from open_llm_vtuber.routes import init_config_routes
    from open_llm_vtuber.frontend_version import init_frontend_version_routes
    from tests.conftest import _FakeContext, _FakeWSHandler, _write_env_layout, _base_conf_text

    conf_path, env_path, persona_path = _write_env_layout(
        tmp_path, _base_conf_text(), "DUMMY=1\n", "name: bai\nsystem_prompt: 'x'\nmodel: m\n"
    )
    app = FastAPI()
    app.include_router(
        init_config_routes(
            config_path=str(conf_path),
            default_context_cache=_FakeContext(),
            ws_handler=_FakeWSHandler(),
        )
    )
    app.include_router(init_frontend_version_routes())
    return TestClient(app)


@pytest.fixture
def version_client(tmp_path):
    return _build_app(tmp_path)


class TestVersionApi:
    def test_api_version_shape(self, version_client):
        r = version_client.get("/api/version")
        assert r.status_code == 200
        data = r.json()
        assert data["product_version"] == "v0.2.0-preview.1"
        assert isinstance(data["backend_version"], str) and data["backend_version"]
        assert isinstance(data["frontend_version"], str) and data["frontend_version"]
        assert isinstance(data["commit"], str)

    def test_product_version_is_single_source(self, version_client):
        # 产品版本不混入 conf schema v1.2.1；health 与 /api/version 一致。
        v = version_client.get("/api/version").json()
        h = version_client.get("/health").json()
        assert v["product_version"] == h["product_version"]
        assert v["product_version"] == "v0.2.0-preview.1"
        assert h["version"] == "v0.2.0-preview.1"
        assert h["status"] == "ok"
        assert "backend_version" in h and "commit" in h

    def test_health_uptime_present(self, version_client):
        h = version_client.get("/health").json()
        assert isinstance(h.get("uptime_s"), int)

    def test_frontend_version_backcompat(self, version_client):
        # 文件不存在时仍返回 ok=false 而不是 500。
        r = version_client.get("/api/frontend-version")
        body = r.json()
        assert "ok" in body


class TestVersionsModule:
    def test_product_constant(self):
        from open_llm_vtuber import versions

        assert versions.PRODUCT_VERSION == "v0.2.0-preview.1"
        assert versions.FRONTEND_PACKAGE_VERSION == "1.0.0"

    def test_common_version_info_has_all_keys(self):
        from open_llm_vtuber import versions

        info = versions.common_version_info(commit_override="abc123")
        assert set(info) == {
            "product_version",
            "backend_version",
            "frontend_version",
            "commit",
        }
        assert info["commit"] == "abc123"

    def test_banner_parseable(self):
        from open_llm_vtuber import versions

        banner = versions.format_banner()
        assert "product=v0.2.0-preview.1" in banner
        assert "backend=" in banner
        assert "frontend=" in banner
        assert "commit=" in banner

    def test_backend_version_falls_back_unknown(self, monkeypatch):
        from open_llm_vtuber import versions

        def boom():
            raise RuntimeError("no package")

        monkeypatch.setattr(versions, "_package_version", boom)
        versions.backend_version.cache_clear()
        assert versions.backend_version() == "unknown"
        versions.backend_version.cache_clear()
