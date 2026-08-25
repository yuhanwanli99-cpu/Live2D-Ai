"""Unit tests for the Windows portable packaging pipeline (task-pkg-build).

Covers pure logic of scripts/assemble_zip.py, scripts/check_resources.py and
scripts/verify_package.py — no network, no uv, no big downloads.
"""
import os
import shutil
import sys
import tempfile
import unittest
import zipfile

_SCRIPTS = os.path.join(os.path.dirname(__file__), "..", "scripts")
if _SCRIPTS not in sys.path:
    sys.path.insert(0, _SCRIPTS)

import assemble_zip  # noqa: E402
import check_resources  # noqa: E402
import verify_package  # noqa: E402


class TestCheckResources(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp(prefix="pkg-check-")

    def tearDown(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def _touch(self, rel):
        path = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write("x")

    def test_all_sentinels_present(self):
        for rel, _sev in check_resources.SENTINELS:
            self._touch(rel)
        self.assertEqual(check_resources.collect_missing(self.root), [])

    def test_required_missing_is_reported(self):
        self._touch("shared/persona.yaml")
        self._touch("Live2D-Ai-pc/open-llm-vtuber/frontend/index.html")
        self._touch("Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.model3.json")
        missing = check_resources.collect_missing(self.root)
        self.assertTrue(any(rel == "shared/mcp_tools.json" for rel, _ in missing))
        # ffmpeg sentinels must be required, not warn
        ff = [sev for rel, sev in check_resources.SENTINELS if "ffmpeg.exe" in rel or "ffprobe.exe" in rel]
        self.assertEqual(ff, ["required", "required"])


class TestZipExclusions(unittest.TestCase):
    def test_top_level_runtime_dirs_excluded(self):
        self.assertTrue(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/models", True))
        self.assertTrue(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/avatars", True))
        self.assertTrue(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/cache", True))
        self.assertTrue(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/logs", True))

    def test_deep_models_dir_preserved(self):
        # src/open_llm_vtuber/asr/models ships silero_vad.onnx — must NOT be excluded
        self.assertFalse(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/asr/models", True))
        self.assertFalse(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/asr/models/silero_vad.onnx", False))

    def test_conf_yaml_excluded_template_included(self):
        self.assertTrue(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/conf.yaml", False))
        self.assertFalse(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/conf.yaml.template", False))

    def test_pyc_and_git_excluded(self):
        self.assertTrue(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/src/__pycache__/x.pyc", False))
        # .git is pruned as a directory in the walker (its files are never visited)
        self.assertTrue(assemble_zip.zip_excluded(".git", True))
        self.assertTrue(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/__pycache__", True))
        self.assertFalse(assemble_zip.zip_excluded("shared/persona.yaml", False))

    def test_torch_old_only_under_torch_lib(self):
        self.assertTrue(
            assemble_zip.zip_excluded(
                "Live2D-Ai-pc/open-llm-vtuber/.venv/Lib/site-packages/torch/lib/foo.old", False
            )
        )
        self.assertFalse(assemble_zip.zip_excluded("Live2D-Ai-pc/open-llm-vtuber/data/something.old", False))


class TestPyvenvRewrite(unittest.TestCase):
    def setUp(self):
        self.venv = tempfile.mkdtemp(prefix="pkg-venv-")

    def tearDown(self):
        shutil.rmtree(self.venv, ignore_errors=True)

    def test_home_rewritten(self):
        cfg = os.path.join(self.venv, "pyvenv.cfg")
        with open(cfg, "w", encoding="utf-8") as fh:
            fh.write("home = C:\\Users\\x\\AppData\\Roaming\\uv\\python\\cpython-3.12\n")
        assemble_zip.rewrite_pyvenv_cfg(self.venv, "python")
        with open(cfg, "r", encoding="utf-8") as fh:
            self.assertEqual(fh.read().splitlines()[0], "home = python")

    def test_home_added_when_absent(self):
        cfg = os.path.join(self.venv, "pyvenv.cfg")
        with open(cfg, "w", encoding="utf-8") as fh:
            fh.write("include-system-site-packages = false\n")
        assemble_zip.rewrite_pyvenv_cfg(self.venv, "python")
        with open(cfg, "r", encoding="utf-8") as fh:
            self.assertIn("home = python", fh.read().splitlines())


class TestVerifyPackageMembers(unittest.TestCase):
    def test_member_presence_and_absence(self):
        tmp = tempfile.mkdtemp(prefix="pkg-zip-")
        try:
            zip_path = os.path.join(tmp, "pkg.zip")
            app = "Live2D-Ai-pc/open-llm-vtuber"
            with zipfile.ZipFile(zip_path, "w") as zf:
                for member in verify_package.REQUIRED_MEMBERS:
                    zf.writestr(member, "x")
                zf.writestr(f"{app}/models/whisper/model.bin", "x")
                zf.writestr(f"{app}/__pycache__/x.pyc", "x")
                zf.writestr(f"{app}/conf.yaml", "x")
            missing, forbidden = verify_package.check_members(zip_path)
            self.assertEqual(missing, [])
            self.assertEqual(
                sorted(forbidden),
                [f"{app}/__pycache__/x.pyc", f"{app}/conf.yaml", f"{app}/models/whisper/model.bin"],
            )
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


class TestCopyTreeFiltered(unittest.TestCase):
    def test_top_level_excludes_only(self):
        src = tempfile.mkdtemp(prefix="pkg-src-")
        dst = tempfile.mkdtemp(prefix="pkg-dst-")
        try:
            os.makedirs(os.path.join(src, "models", "whisper"))
            os.makedirs(os.path.join(src, "src", "open_llm_vtuber", "asr", "models"))
            with open(os.path.join(src, "models", "whisper", "a.bin"), "w") as fh:
                fh.write("x")
            with open(os.path.join(src, "src", "open_llm_vtuber", "asr", "models", "silero_vad.onnx"), "w") as fh:
                fh.write("x")
            with open(os.path.join(src, "run_server.py"), "w") as fh:
                fh.write("x")
            count = assemble_zip.copy_tree_filtered(src, dst)
            self.assertFalse(os.path.exists(os.path.join(dst, "models")))
            self.assertTrue(os.path.isfile(os.path.join(dst, "run_server.py")))
            self.assertTrue(
                os.path.isfile(os.path.join(dst, "src", "open_llm_vtuber", "asr", "models", "silero_vad.onnx"))
            )
            self.assertEqual(count, 2)
        finally:
            shutil.rmtree(src, ignore_errors=True)
            shutil.rmtree(dst, ignore_errors=True)


class TestAbsorbStandalonePython(unittest.TestCase):
    """P0 fix: .venv/Scripts/ must become a self-contained portable Python."""

    def setUp(self):
        self.base = tempfile.mkdtemp(prefix="pkg-base-")
        self.venv = tempfile.mkdtemp(prefix="pkg-venv-")
        os.makedirs(os.path.join(self.base, "DLLs"))
        os.makedirs(os.path.join(self.base, "Lib", "site-packages"))
        for rel in ("python.exe", "pythonw.exe", "python312.dll", "python3.dll",
                    "vcruntime140.dll", "vcruntime140_1.dll"):
            with open(os.path.join(self.base, rel), "wb") as fh:
                fh.write(b"X" * 4)
        with open(os.path.join(self.base, "Lib", "os.py"), "w") as fh:
            fh.write("stdlib")
        with open(os.path.join(self.base, "Lib", "site-packages", "pip"), "w") as fh:
            fh.write("bundled pip")
        with open(os.path.join(self.base, "DLLs", "_ctypes.pyd"), "wb") as fh:
            fh.write(b"Y" * 4)
        with open(os.path.join(self.venv, "placeholder.txt"), "w") as fh:
            fh.write("keep")

    def tearDown(self):
        shutil.rmtree(self.base, ignore_errors=True)
        shutil.rmtree(self.venv, ignore_errors=True)

    def test_runtime_items_land_in_scripts(self):
        copied = assemble_zip.absorb_standalone_python(self.base, self.venv)
        scripts = os.path.join(self.venv, "Scripts")
        self.assertTrue(os.path.isfile(os.path.join(scripts, "python.exe")))
        self.assertTrue(os.path.isfile(os.path.join(scripts, "python312.dll")))
        self.assertTrue(os.path.isfile(os.path.join(scripts, "Lib", "os.py")))
        self.assertTrue(os.path.isfile(os.path.join(scripts, "DLLs", "_ctypes.pyd")))
        self.assertEqual(len(copied), len(assemble_zip.STANDALONE_RUNTIME_ITEMS))

    def test_standalone_site_packages_dropped(self):
        assemble_zip.absorb_standalone_python(self.base, self.venv)
        scripts = os.path.join(self.venv, "Scripts")
        # bundled pip must NOT leak into the venv's Scripts/Lib/site-packages
        self.assertFalse(os.path.exists(os.path.join(scripts, "Lib", "site-packages")))

    def test_real_exe_overwrites_trampoline(self):
        trampoline = os.path.join(self.venv, "Scripts", "python.exe")
        os.makedirs(os.path.dirname(trampoline), exist_ok=True)
        with open(trampoline, "wb") as fh:  # pre-existing uv trampoline (small)
            fh.write(b"TRAMPOLINE")
        assemble_zip.absorb_standalone_python(self.base, self.venv)
        with open(trampoline, "rb") as fh:
            self.assertEqual(fh.read(), b"X" * 4)  # replaced by the real exe bytes


class TestVerifyPackageHomeGate(unittest.TestCase):
    def test_relative_home_ok_absolute_rejected(self):
        app = tempfile.mkdtemp(prefix="pkg-home-")
        try:
            os.makedirs(os.path.join(app, ".venv"))
            cfg = os.path.join(app, ".venv", "pyvenv.cfg")
            with open(cfg, "w", encoding="utf-8") as fh:
                fh.write("home = .\n")
            ok, home = verify_package.check_pyvenv_cfg(app)
            self.assertTrue(ok)
            self.assertEqual(home, ".")
            with open(cfg, "w", encoding="utf-8") as fh:
                fh.write("home = C:\\Users\\x\\AppData\\Roaming\\uv\\python\\cpython-3.12\n")
            ok, home = verify_package.check_pyvenv_cfg(app)
            self.assertFalse(ok)
        finally:
            shutil.rmtree(app, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()
