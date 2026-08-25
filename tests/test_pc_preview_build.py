"""PC-only Preview source archive builder tests.

Covers scripts/build_pc_preview.py without network or model downloads.
The tests verify that dirty workspace artifacts never leak into the published tree.
"""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

_SCRIPTS = os.path.join(os.path.dirname(__file__), "..", "scripts")
if _SCRIPTS not in sys.path:
    sys.path.insert(0, _SCRIPTS)

import build_pc_preview  # noqa: E402


class TestIsForbidden(unittest.TestCase):
    def test_model_binaries_forbidden(self):
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.moc3")))
        self.assertTrue(build_pc_preview.is_forbidden(Path("app/src/main/assets/live2d/bai/runtime/bai.model3.json")))

    def test_cubism_core_forbidden(self):
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/frontend/libs/live2dcubismcore.min.js")))
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/renderer/public/libs/live2dcubismcore.min.js")))

    def test_conf_yaml_forbidden_but_template_allowed(self):
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/conf.yaml")))
        self.assertFalse(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/conf.yaml.template")))

    def test_env_files_forbidden(self):
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/.env")))
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/.env.local")))

    def test_android_excluded(self):
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-Android/app/src/main/res/values/strings.xml")))

    def test_venv_and_node_modules_excluded(self):
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/.venv/lib/python3.12/site-packages/x.py")))
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/renderer/node_modules/vitest/package.json")))

    def test_databases_forbidden(self):
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/memory.db")))
        self.assertTrue(build_pc_preview.is_forbidden(Path("Live2D-Ai-pc/open-llm-vtuber/cache/state.sqlite3")))

    def test_absolute_local_paths_detected(self):
        import tempfile

        with tempfile.TemporaryDirectory(prefix="path-scan-") as tmp:
            root = Path(tmp)
            bad = root / "Live2D-Ai-pc/open-llm-vtuber/note.txt"
            bad.parent.mkdir(parents=True, exist_ok=True)
            bad.write_text(
                "generated on /home/administrator/deepseekharness/Live2D-Ai\n",
                encoding="utf-8",
            )
            good = root / "readme.md"
            good.write_text("see https://example.com and use ~/miniconda3/envs/melo\n", encoding="utf-8")
            findings = build_pc_preview.scan_local_paths(root)
            self.assertTrue(findings, "expected an absolute local path finding")
            self.assertTrue(any("linux home" in f for f in findings))
            # No false positive on tilde-relative / textual paths.
            good_findings = [f for f in findings if "readme.md" in f]
            self.assertEqual(good_findings, [])


class TestBuildDoesNotLeak(unittest.TestCase):
    """Run the real builder in a temporary clone seeded with dirty files."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="pc-preview-build-"))
        self.src = self.tmp / "src"
        shutil.copytree(
            Path(__file__).resolve().parents[1],
            self.src,
            ignore=shutil.ignore_patterns(".git", ".venv", "node_modules", ".uv-*"),
        )
        # Turn the copied tree into a git repo so `git ls-files` works, then add
        # every copied file to the index.  Dirty files created afterwards stay
        # untracked, mirroring a real dirty workspace.
        subprocess.run(["git", "init", "-q"], cwd=self.src, check=True)
        subprocess.run(["git", "add", "-A"], cwd=self.src, check=True)
        self.output = self.tmp / "preview"

        # Dirty artifacts that must NOT appear in the published tree.
        dirty = [
            "Live2D-Ai-pc/open-llm-vtuber/conf.yaml",
            "Live2D-Ai-pc/open-llm-vtuber/.env",
            "Live2D-Ai-pc/open-llm-vtuber/.env.local",
            "Live2D-Ai-pc/open-llm-vtuber/chat_history/session.json",
            "Live2D-Ai-pc/open-llm-vtuber/cache/tts_local_fake.wav",
            "Live2D-Ai-pc/open-llm-vtuber/logs/server.log",
            "Live2D-Ai-pc/open-llm-vtuber/frontend/libs/live2dcubismcore.min.js",
            "Live2D-Ai-pc/open-llm-vtuber/renderer/public/libs/live2dcubismcore.min.js",
            "Live2D-Ai-pc/open-llm-vtuber/memory.db",
            "Live2D-Ai-pc/open-llm-vtuber/local_note.txt",
            "Live2D-Ai-Android/app/src/main/assets/live2d/bai/runtime/bai.moc3",
            "Live2D-Ai-Android/local.properties",
        ]
        for rel in dirty:
            path = self.src / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("sk-fake-dirty-key-1234567890abcdef", encoding="utf-8")
        # A dirty file carrying an absolute local path, to prove the output-tree
        # absolute-path scan would reject it if it ever leaked in.
        (self.src / "Live2D-Ai-pc/open-llm-vtuber/leak_path.txt").write_text(
            "created under /home/administrator/deepseekharness/Live2D-Ai\n",
            encoding="utf-8",
        )

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_build_succeeds_and_excludes_dirty_files(self):
        env = os.environ.copy()
        env["PYTHONPATH"] = str(self.src / "scripts")
        subprocess.run(
            [sys.executable, "-m", "build_pc_preview", "--output", str(self.output)],
            cwd=str(self.src / "scripts"),
            env=env,
            check=True,
        )
        archive = self.output.with_suffix(".zip")
        self.assertTrue(archive.is_file())

        with zipfile.ZipFile(archive) as zf:
            names = zf.namelist()

        forbidden_samples = [
            "/.env",
            "/.env.local",
            "chat_history/",
            "cache/",
            "logs/",
            "frontend/libs/",
            "renderer/public/libs/",
            "live2dcubismcore.min.js",
            "Live2D-Ai-Android/",
            ".moc3",
        ]
        leaks = []
        for n in names:
            # conf.yaml itself must never leak; conf.yaml.template is allowed.
            if n.endswith("/conf.yaml"):
                leaks.append((n, "conf.yaml"))
                continue
            for sample in forbidden_samples:
                if sample in n:
                    leaks.append((n, sample))
                    break
        self.assertEqual(leaks, [], f"dirty artifacts leaked into archive: {leaks[:20]}")

        # Required release notes must be present.
        self.assertTrue(
            any("docs/releases/v0.2.0-preview.1.md" in n for n in names),
            "release notes missing from archive",
        )


if __name__ == "__main__":
    unittest.main()
