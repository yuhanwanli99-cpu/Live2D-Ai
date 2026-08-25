"""PC 端 Live2D 模型 zip 上传接口（model_upload.py）单元测试。"""

import io
import json
import os
import shutil
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

from fastapi import FastAPI
from fastapi.testclient import TestClient

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.model_upload import init_model_upload_routes  # noqa: E402


def make_zip(name: str, files: dict[str, bytes]) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as zf:
        for path, data in files.items():
            zf.writestr(path, data)
    return buf.getvalue()


def make_valid_zip(model_id: str = "mymodel") -> bytes:
    return make_zip(
        f"{model_id}.zip",
        {
            f"{model_id}/runtime/{model_id}.model3.json": b'{"Version":3}',
            f"{model_id}/runtime/{model_id}.moc3": b"moc",
            f"{model_id}/runtime/texture_00.png": b"png",
        },
    )


class ModelUploadTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="live2d-model-upload-test-"))
        self.live2d_dir = self.tmp / "live2d-models"
        self.live2d_dir.mkdir(parents=True, exist_ok=True)
        self.registry = self.tmp / "model_registry.json"
        self.registry.write_text(
            json.dumps({"version": "1.0", "models": []}, ensure_ascii=False),
            encoding="utf-8",
        )

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _client(self):
        app = FastAPI()
        app.include_router(
            init_model_upload_routes(
                live2d_models_dir=str(self.live2d_dir),
                registry_path=str(self.registry),
            )
        )
        return TestClient(app)

    def test_valid_zip_imports_and_updates_registry(self):
        client = self._client()
        resp = client.post(
            "/api/models/upload",
            files={"file": ("mymodel.zip", make_valid_zip("mymodel"), "application/zip")},
            data={"model_id": "mymodel", "display_name": "My Model"},
        )
        self.assertEqual(resp.status_code, 200, resp.text)
        body = resp.json()
        self.assertTrue(body["ok"])
        self.assertEqual(body["model_id"], "mymodel")
        self.assertIn("mymodel.model3.json", body["model3_json_path"])

        registry = json.loads(self.registry.read_text(encoding="utf-8"))
        self.assertEqual(registry["models"][0]["id"], "mymodel")
        self.assertTrue(
            (self.live2d_dir / "mymodel" / "runtime" / "mymodel.model3.json").is_file()
        )

    def test_missing_model3_rejected(self):
        client = self._client()
        zip_bytes = make_zip("bad.zip", {"readme.txt": b"no model"})
        resp = client.post(
            "/api/models/upload",
            files={"file": ("bad.zip", zip_bytes, "application/zip")},
        )
        self.assertEqual(resp.status_code, 400)

    def test_zip_slip_rejected(self):
        client = self._client()
        zip_bytes = make_zip(
            "slip.zip",
            {
                "../evil.txt": b"evil",
                "mymodel/runtime/mymodel.model3.json": b"{}",
            },
        )
        resp = client.post(
            "/api/models/upload",
            files={"file": ("slip.zip", zip_bytes, "application/zip")},
        )
        self.assertEqual(resp.status_code, 400)

    def test_invalid_model_id_rejected(self):
        client = self._client()
        resp = client.post(
            "/api/models/upload",
            files={"file": ("mymodel.zip", make_valid_zip("mymodel"), "application/zip")},
            data={"model_id": "BAD MODEL"},
        )
        self.assertEqual(resp.status_code, 400)

    def test_existing_model_requires_overwrite(self):
        client = self._client()
        first = client.post(
            "/api/models/upload",
            files={"file": ("mymodel.zip", make_valid_zip("mymodel"), "application/zip")},
            data={"model_id": "mymodel"},
        )
        self.assertEqual(first.status_code, 200)
        second = client.post(
            "/api/models/upload",
            files={"file": ("mymodel.zip", make_valid_zip("mymodel"), "application/zip")},
            data={"model_id": "mymodel"},
        )
        self.assertEqual(second.status_code, 409)


if __name__ == "__main__":
    unittest.main()
