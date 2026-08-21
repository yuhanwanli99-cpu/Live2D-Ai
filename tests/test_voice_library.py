"""PC 端声音克隆资产管理（voice_library.py）单元测试。"""

import io
import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

from fastapi import FastAPI
from fastapi.testclient import TestClient

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2D-Ai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.voice_library import init_voice_library_routes  # noqa: E402


class VoiceLibraryTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="live2d-voice-test-"))
        self.voice_dir = self.tmp / "voice_clone"
        self.state_path = self.tmp / "voice_library.json"
        self.conf_path = self.tmp / "conf.yaml"
        self.conf_path.write_text(
            "character_config:\n  tts_config:\n    tts_model: gpt_sovits_tts\n",
            encoding="utf-8",
        )

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _client(self):
        app = FastAPI()
        app.include_router(
            init_voice_library_routes(
                voice_dir=str(self.voice_dir),
                state_path=str(self.state_path),
                conf_path=str(self.conf_path),
            )
        )
        return TestClient(app)

    def _upload(self, client, name="ref.wav"):
        return client.post(
            "/api/voice/upload",
            files={"file": (name, b"RIFFfakewave", "audio/wav")},
            data={"voice_name": "My Voice"},
        )

    def test_upload_and_list(self):
        client = self._client()
        resp = self._upload(client)
        self.assertEqual(resp.status_code, 200, resp.text)
        voice = resp.json()["voice"]
        self.assertTrue(voice["id"])
        self.assertEqual(voice["name"], "My Voice")
        self.assertEqual(voice["ext"], ".wav")

        listed = client.get("/api/voice/list").json()
        self.assertEqual(len(listed["voices"]), 1)
        self.assertFalse(listed["voices"][0]["active"])

    def test_upload_rejects_bad_extension(self):
        client = self._client()
        resp = client.post(
            "/api/voice/upload",
            files={"file": ("bad.txt", b"nope", "text/plain")},
        )
        self.assertEqual(resp.status_code, 400)

    def test_select_then_delete(self):
        client = self._client()
        voice_id = self._upload(client).json()["voice"]["id"]
        sel = client.post("/api/voice/select", json={"voice_id": voice_id})
        self.assertEqual(sel.status_code, 200)
        self.assertTrue(sel.json()["voice"]["active"])

        deleted = client.post("/api/voice/delete", json={"voice_id": voice_id})
        self.assertEqual(deleted.status_code, 200)
        self.assertEqual(client.get("/api/voice/list").json()["voices"], [])
        self.assertEqual(list(self.voice_dir.iterdir()), [])

    def test_apply_updates_conf_for_clone_tts(self):
        client = self._client()
        voice_id = self._upload(client).json()["voice"]["id"]
        resp = client.post("/api/voice/apply", json={"voice_id": voice_id})
        self.assertEqual(resp.status_code, 200, resp.text)
        conf = self.conf_path.read_text(encoding="utf-8")
        self.assertIn("ref_audio_path", conf)
        self.assertIn("voice_clone/", conf)

    def test_apply_rejects_non_clone_tts(self):
        self.conf_path.write_text(
            "character_config:\n  tts_config:\n    tts_model: cosyvoice_cloud\n",
            encoding="utf-8",
        )
        client = self._client()
        voice_id = self._upload(client).json()["voice"]["id"]
        resp = client.post("/api/voice/apply", json={"voice_id": voice_id})
        self.assertEqual(resp.status_code, 400)
        self.assertIn("不支持克隆参考音频", resp.json()["error"])


if __name__ == "__main__":
    unittest.main()
