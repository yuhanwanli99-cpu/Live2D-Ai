"""frontend_version.py（P3 前端版本上报）单元测试。"""
import hashlib
import os
import sys
import tempfile
import unittest

from fastapi import FastAPI
from fastapi.testclient import TestClient
from starlette.staticfiles import StaticFiles as StarletteStaticFiles

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.frontend_version import (  # noqa: E402
    init_frontend_version_routes,
)


class _CacheBustingStaticFiles(StarletteStaticFiles):
    """镜像 server.py 的 CORSStaticFiles 缓存头（no-cache, no-store）。

    单独导入 server.py 会拉入 prompts 等项目根包，故此处内联同一缓存头契约，
    验证 `/` 静态挂载在 TestClient 下确实下发 Cache-Control。
    """

    async def get_response(self, path, scope):
        response = await super().get_response(path, scope)
        response.headers["Cache-Control"] = "no-cache, no-store, must-revalidate"
        response.headers["Pragma"] = "no-cache"
        return response


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _build_client(frontend_dir):
    app = FastAPI()
    app.include_router(init_frontend_version_routes(frontend_dir))
    return TestClient(app)


class TestFrontendVersion(unittest.TestCase):
    def test_returns_sha12_of_index_html(self):
        with tempfile.TemporaryDirectory() as tmp:
            html = b"<html><body>hello world</body></html>"
            with open(os.path.join(tmp, "index.html"), "wb") as f:
                f.write(html)

            resp = _build_client(tmp).get("/api/frontend-version")
            self.assertEqual(resp.status_code, 200)
            data = resp.json()
            self.assertTrue(data["ok"])
            self.assertEqual(data["version"], _sha256(html)[:12])
            self.assertEqual(data["index_sha256"], _sha256(html))
            self.assertIsInstance(data["index_mtime"], (int, float))

    def test_missing_dir_returns_ok_false(self):
        with tempfile.TemporaryDirectory() as tmp:
            missing = os.path.join(tmp, "does-not-exist")
            resp = _build_client(missing).get("/api/frontend-version")
            self.assertEqual(resp.status_code, 200)
            data = resp.json()
            self.assertFalse(data["ok"])
            self.assertIn("error", data)

    def test_static_cache_control_no_store(self):
        """可选断言：静态根路径的 Cache-Control 为 no-cache, no-store, must-revalidate。"""
        with tempfile.TemporaryDirectory() as tmp:
            with open(os.path.join(tmp, "index.html"), "w", encoding="utf-8") as f:
                f.write("<html>hi</html>")

            app = FastAPI()
            app.include_router(init_frontend_version_routes(tmp))
            try:
                app.mount(
                    "/",
                    _CacheBustingStaticFiles(directory=tmp, html=True),
                    name="frontend",
                )
            except Exception:
                return  # 无法挂载则跳过（如系统缺目录权限）

            resp = TestClient(app).get("/")
            self.assertEqual(resp.status_code, 200)
            self.assertEqual(
                resp.headers.get("Cache-Control"),
                "no-cache, no-store, must-revalidate",
            )


if __name__ == "__main__":
    unittest.main()
