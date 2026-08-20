# -*- coding: utf-8 -*-
"""角色背景图上传/选择测试（`src/open_llm_vtuber/backgrounds.py`）。

覆盖：文件名清洗与目录逃逸、魔数校验、体积上限、上传即选中、
列表/选择/删除的状态一致性、选择落盘与重启恢复、坏引用自愈。
全部在 tmp_path 内进行，不写仓库真实 backgrounds/。
"""

import os

import pytest

from open_llm_vtuber.backgrounds import (
    ALLOWED_EXTENSIONS,
    MAX_UPLOAD_BYTES,
    BackgroundStore,
    init_background_routes,
    list_background_files,
    sanitize_filename,
    sniff_extension_ok,
    unique_path,
)

PNG_HEADER = b"\x89PNG\r\n\x1a\n"
JPEG_HEADER = b"\xff\xd8\xff\xe0"
GIF_HEADER = b"GIF89a"


def png_bytes(size: int = 64) -> bytes:
    return PNG_HEADER + b"\x00" * max(size - len(PNG_HEADER), 1)


@pytest.fixture
def env(tmp_path):
    from fastapi import FastAPI
    from fastapi.testclient import TestClient

    bg_dir = tmp_path / "backgrounds"
    bg_dir.mkdir()
    store = BackgroundStore(
        directory=str(bg_dir), state_path=str(tmp_path / "background_state.json")
    ).load()
    app = FastAPI()
    app.include_router(init_background_routes(store=store))
    return type(
        "Env",
        (),
        {"client": TestClient(app), "store": store, "dir": bg_dir, "root": tmp_path},
    )


# ─────────────────────────────────────────────────────────────
# 纯函数
# ─────────────────────────────────────────────────────────────
class TestSanitize:
    @pytest.mark.parametrize(
        "raw",
        [
            "../../etc/passwd.png",
            "/etc/passwd.png",
            "..\\..\\windows\\system32\\a.png",
            "sub/dir/a.png",
        ],
    )
    def test_strips_directory_components(self, raw):
        cleaned = sanitize_filename(raw)
        assert "/" not in cleaned and "\\" not in cleaned
        assert not cleaned.startswith(".")

    def test_keeps_unicode_and_folds_whitespace(self):
        assert sanitize_filename("我的 房间 背景.png") == "我的_房间_背景.png"

    def test_truncates_long_names_keeping_extension(self):
        cleaned = sanitize_filename("a" * 200 + ".png")
        assert len(cleaned) <= 80 and cleaned.endswith(".png")

    def test_empty_input(self):
        assert sanitize_filename("") == ""


class TestSniff:
    def test_accepts_matching_headers(self):
        assert sniff_extension_ok(PNG_HEADER, ".png")
        assert sniff_extension_ok(JPEG_HEADER, ".jpg")
        assert sniff_extension_ok(JPEG_HEADER, ".jpeg")
        assert sniff_extension_ok(GIF_HEADER, ".gif")
        assert sniff_extension_ok(b"RIFF\x00\x00\x00\x00WEBPVP8 ", ".webp")

    def test_rejects_mismatch_and_unknown(self):
        assert not sniff_extension_ok(PNG_HEADER, ".jpg")
        assert not sniff_extension_ok(b"MZ\x90\x00", ".png")  # 可执行文件改后缀
        assert not sniff_extension_ok(b"", ".png")
        assert not sniff_extension_ok(b"RIFF\x00\x00\x00\x00AVI ", ".webp")


class TestUniquePath:
    def test_appends_counter(self, tmp_path):
        (tmp_path / "a.png").write_bytes(png_bytes())
        assert os.path.basename(unique_path(str(tmp_path), "a.png")) == "a-1.png"
        (tmp_path / "a-1.png").write_bytes(png_bytes())
        assert os.path.basename(unique_path(str(tmp_path), "a.png")) == "a-2.png"


class TestListing:
    def test_only_image_extensions_newest_first(self, tmp_path):
        (tmp_path / "note.txt").write_text("x", encoding="utf-8")
        (tmp_path / "old.png").write_bytes(png_bytes())
        os.utime(tmp_path / "old.png", (1000, 1000))
        (tmp_path / "new.jpg").write_bytes(JPEG_HEADER)
        os.utime(tmp_path / "new.jpg", (2000, 2000))
        names = [f["name"] for f in list_background_files(str(tmp_path))]
        assert names == ["new.jpg", "old.png"]
        assert all(f["url"].startswith("/bg/") for f in list_background_files(str(tmp_path)))

    def test_missing_directory_is_empty(self, tmp_path):
        assert list_background_files(str(tmp_path / "nope")) == []

    def test_allowed_extensions_documented(self):
        assert ".png" in ALLOWED_EXTENSIONS and ".webp" in ALLOWED_EXTENSIONS


# ─────────────────────────────────────────────────────────────
# HTTP
# ─────────────────────────────────────────────────────────────
class TestBackgroundHttp:
    def test_list_empty_initially(self, env):
        body = env.client.get("/api/background/list").json()
        assert body == {"ok": True, "files": [], "current": None, "current_url": None}

    def test_upload_then_selected_and_listed(self, env):
        r = env.client.post(
            "/api/background/upload",
            files={"file": ("房间.png", png_bytes(256), "image/png")},
        )
        assert r.status_code == 200
        body = r.json()
        assert body["ok"] is True
        assert body["file"] == "房间.png"
        assert body["current"] == "房间.png"
        assert body["current_url"] == "/bg/房间.png"
        assert (env.dir / "房间.png").exists()
        assert [f["name"] for f in body["files"]] == ["房间.png"]

    def test_upload_rejects_bad_extension(self, env):
        r = env.client.post(
            "/api/background/upload",
            files={"file": ("x.svg", b"<svg/>", "image/svg+xml")},
        )
        assert r.status_code == 400
        assert "只支持" in r.json()["error"]

    def test_upload_rejects_content_mismatch(self, env):
        r = env.client.post(
            "/api/background/upload",
            files={"file": ("evil.png", b"MZ\x90\x00 not an image", "image/png")},
        )
        assert r.status_code == 400
        assert "有效图片" in r.json()["error"]
        assert list(env.dir.iterdir()) == []

    def test_upload_rejects_empty(self, env):
        r = env.client.post(
            "/api/background/upload", files={"file": ("a.png", b"", "image/png")}
        )
        assert r.status_code == 400

    def test_upload_rejects_oversize(self, env):
        blob = PNG_HEADER + b"\x00" * (MAX_UPLOAD_BYTES + 1)
        r = env.client.post(
            "/api/background/upload", files={"file": ("big.png", blob, "image/png")}
        )
        assert r.status_code == 413

    def test_upload_path_traversal_lands_in_directory(self, env):
        r = env.client.post(
            "/api/background/upload",
            files={"file": ("../../evil.png", png_bytes(), "image/png")},
        )
        assert r.status_code == 200
        assert r.json()["file"] == "evil.png"
        assert (env.dir / "evil.png").exists()
        assert not (env.root / "evil.png").exists()

    def test_upload_same_name_does_not_overwrite(self, env):
        env.client.post(
            "/api/background/upload", files={"file": ("a.png", png_bytes(64), "image/png")}
        )
        r = env.client.post(
            "/api/background/upload", files={"file": ("a.png", png_bytes(128), "image/png")}
        )
        assert r.json()["file"] == "a-1.png"
        assert {"a.png", "a-1.png"} <= {p.name for p in env.dir.iterdir()}

    def test_select_none_clears_background(self, env):
        env.client.post(
            "/api/background/upload", files={"file": ("a.png", png_bytes(), "image/png")}
        )
        r = env.client.post("/api/background/select", json={"file": None})
        assert r.status_code == 200
        assert r.json()["current"] is None
        assert r.json()["current_url"] is None

    def test_select_missing_file_404(self, env):
        r = env.client.post("/api/background/select", json={"file": "ghost.png"})
        assert r.status_code == 404

    def test_select_existing_switches(self, env):
        env.client.post(
            "/api/background/upload", files={"file": ("a.png", png_bytes(), "image/png")}
        )
        env.client.post(
            "/api/background/upload", files={"file": ("b.png", png_bytes(), "image/png")}
        )
        assert env.client.get("/api/background/list").json()["current"] == "b.png"
        r = env.client.post("/api/background/select", json={"file": "a.png"})
        assert r.json()["current"] == "a.png"

    def test_delete_removes_and_clears_current(self, env):
        env.client.post(
            "/api/background/upload", files={"file": ("a.png", png_bytes(), "image/png")}
        )
        r = env.client.post("/api/background/delete", json={"file": "a.png"})
        assert r.status_code == 200
        assert r.json()["current"] is None
        assert r.json()["files"] == []
        assert not (env.dir / "a.png").exists()

    def test_delete_missing_404(self, env):
        assert env.client.post("/api/background/delete", json={"file": "x.png"}).status_code == 404

    def test_selection_persists_across_reload(self, env, tmp_path):
        env.client.post(
            "/api/background/upload", files={"file": ("a.png", png_bytes(), "image/png")}
        )
        reloaded = BackgroundStore(
            directory=str(env.dir), state_path=str(tmp_path / "background_state.json")
        ).load()
        assert reloaded.current == "a.png"

    def test_stale_selection_self_heals(self, env, tmp_path):
        env.client.post(
            "/api/background/upload", files={"file": ("a.png", png_bytes(), "image/png")}
        )
        (env.dir / "a.png").unlink()  # 用户在文件管理器里手删
        reloaded = BackgroundStore(
            directory=str(env.dir), state_path=str(tmp_path / "background_state.json")
        ).load()
        assert reloaded.current is None
