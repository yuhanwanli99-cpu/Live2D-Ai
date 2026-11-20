"""v0.7.0-rc.1 模板、完整包与发布 workflow 的小型回归门禁。"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
import tomllib
import zipfile
from pathlib import Path

import pytest
import yaml

from scripts import pack_windows_glue as pack

REPO_ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.7.0-rc.1"
DEEPSEEK_CHAN_SOURCE_URL = "https://www.bilibili.com/video/BV1KmxY66EZ3/"
DEEPSEEK_CHAN_TITLE = "【免费live2D模型】大肥鱼人手一只的时代到了！？｜Deepseek小鲸鱼｜连带psd一起分享！"


def _complete_stage(tmp_path: Path, prompt: str = "") -> Path:
    stage = tmp_path / "windows"
    for relative in sorted(set(pack.ZIP_REQUIRED_PATHS) | pack.ASSET_PUBLISH_ALLOWLIST):
        path = stage / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        if relative == "live2d-ai.toml":
            content = "[persona]\nsystem_prompt = " + json.dumps(prompt) + "\n"
        elif relative == "pack-manifest.json":
            content = json.dumps({"version": VERSION, "includes_bai": True}) + "\n"
        elif relative == "start-desk-pet-obs.cmd":
            content = pack.DESK_PET_OBS_LAUNCHER
        elif relative == "README.txt":
            content = pack.PACKAGE_README
        elif relative == "RELEASE_NOTES.md":
            content = (REPO_ROOT / "docs/versions/0.7.md").read_text(encoding="utf-8")
        else:
            content = relative + "\n"
        path.write_text(content, encoding="utf-8")
    return stage


def test_template_and_release_version_are_rc_consistent() -> None:
    cargo = tomllib.loads((REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    assert cargo["workspace"]["package"]["version"] == VERSION
    pubspec = (REPO_ROOT / "shell/flutter/pubspec.yaml").read_text(encoding="utf-8")
    match = re.search(r"^version:\s*(\S+)\s*$", pubspec, re.MULTILINE)
    assert match is not None
    # 保留既有 build number；prerelease 与 build metadata 都是合法 semver 部分。
    assert match.group(1) == VERSION + "+22"
    assert pack.PACKAGE_VERSION == VERSION
    assert pack.PACKAGE_ZIP_NAME == "live2d-ai-windows-0.7.0-rc.1.zip"


def test_example_template_has_no_factory_persona() -> None:
    raw = (REPO_ROOT / "live2d-ai.toml.example").read_text(encoding="utf-8")
    parsed = tomllib.loads(raw)
    assert parsed["persona"]["system_prompt"] == ""
    assert "你是长期陪伴的对方" not in raw
    assert "你是桌面上的 Live2D 桌宠" not in raw


def test_deepseek_chan_release_texts_use_author_source_and_no_distribution() -> None:
    materials = {
        "release notes": (REPO_ROOT / "docs/versions/0.7.md").read_text(encoding="utf-8"),
        "credits": (REPO_ROOT / "CREDITS.md").read_text(encoding="utf-8"),
        "package README": pack.PACKAGE_README,
    }
    for label, text in materials.items():
        assert DEEPSEEK_CHAN_SOURCE_URL in text, label
        assert DEEPSEEK_CHAN_TITLE in text, label
        assert "原画及拆分" in text and "神宫凉子" in text, label
        assert "Live2D 建模" in text and "霓虹猫_NeonNeko" in text, label
        assert "模型来源" in text and "神宫凉子免费配布模型" in text, label
        assert "DeepSeek-chan 模型与 PSD 不随主仓、不随包分发" in text, label
        assert "未经作者明确授权不得再分发" in text, label
        assert "用户原 zip 保持不动" in text, label


def test_release_workflow_uses_only_the_complete_package() -> None:
    path = REPO_ROOT / ".github/workflows/release-build.yml"
    raw = path.read_text(encoding="utf-8")
    parsed = yaml.safe_load(raw)
    steps = parsed["jobs"]["windows-complete-package"]["steps"]
    run_blocks = "\n".join(step.get("run", "") for step in steps)
    upload_paths = "\n".join(
        step.get("with", {}).get("path", "")
        for step in steps
        if step.get("uses", "").startswith("actions/upload-artifact@")
    )
    assert "./scripts/pack-windows.sh" in run_blocks
    assert "from scripts.pack_windows_glue import INFO, MOD_NAMES, PINNED" in run_blocks
    assert "sha256sum -c live2d-ai-windows-0.7.0-rc.1.zip.sha256" in run_blocks
    assert "dtolnay/rust-toolchain@1.98.1" in raw
    assert "flutter-version: 3.47.3" in raw
    assert "mingw-w64" in run_blocks
    assert "${{ runner." not in raw
    assert "$SIBLING_ROOT" not in run_blocks
    clone_run = next(
        step["run"]
        for step in steps
        if step.get("name") == "Checkout six Mod repositories at pack script pins"
    )
    build_run = next(
        step["run"]
        for step in steps
        if step.get("name") == "Build and verify the complete package through the single entrypoint"
    )
    for command in (clone_run, build_run):
        assert "$RUNNER_TEMP/live2d-ai-pack-siblings" in command
    assert pack.PACKAGE_ZIP_NAME in upload_paths
    assert pack.PACKAGE_ZIP_NAME + ".sha256" in upload_paths
    assert "dist/RELEASE_NOTES.md" in upload_paths
    assert "target/release/live2d-ai-desktop" not in upload_paths


def test_release_workflow_run_blocks_parse_as_bash(tmp_path: Path) -> None:
    parsed = yaml.safe_load((REPO_ROOT / ".github/workflows/release-build.yml").read_text(encoding="utf-8"))
    steps = parsed["jobs"]["windows-complete-package"]["steps"]
    for index, step in enumerate(steps):
        run = step.get("run")
        if not run:
            continue
        # GitHub 表达式不是 shell；语法检查前换成普通字符串，命令体保持原样。
        script = tmp_path / f"workflow-step-{index}.sh"
        script.write_text(re.sub(r"\$\{\{[^}]+\}\}", "github-expression", run), encoding="utf-8")
        checked = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True, check=False)
        assert checked.returncode == 0, f"step {index}: {checked.stderr}"


@pytest.mark.parametrize("skip", ["--skip-cargo", "--skip-flutter", "--skip-trunk"])
def test_partial_flags_cannot_create_formal_zip(skip: str, tmp_path: Path) -> None:
    proc = subprocess.run(
        [sys.executable, "scripts/pack_windows_glue.py", skip, "--dist", str(tmp_path / "windows")],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 2, proc.stderr
    assert "只能与 --no-zip 同用" in proc.stderr
    assert not (tmp_path / pack.PACKAGE_ZIP_NAME).exists()


def test_required_and_allowlisted_assets_are_real_tracked_payload() -> None:
    for relative in sorted(set(pack.ZIP_REQUIRED_PATHS) | pack.ASSET_PUBLISH_ALLOWLIST):
        if not relative.startswith(pack.ASSET_PUBLISH_ROOTS):
            continue
        path = REPO_ROOT / relative
        assert path.is_file() and path.stat().st_size > 0, relative
        tracked = subprocess.run(
            ["git", "ls-files", "--error-unmatch", "--", relative],
            cwd=REPO_ROOT,
            capture_output=True,
            check=False,
        )
        assert tracked.returncode == 0, f"git archive 不会包含未跟踪文件：{relative}"
    head_assets = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", "HEAD", "--", "assets/models", "assets/actions"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert head_assets.returncode == 0, head_assets.stderr
    assert set(head_assets.stdout.splitlines()) - {"assets/models/.gitkeep"} == pack.ASSET_PUBLISH_ALLOWLIST


def test_final_zip_crc_required_entries_and_standard_sha256(tmp_path: Path) -> None:
    stage = _complete_stage(tmp_path)
    zip_path = tmp_path / pack.PACKAGE_ZIP_NAME
    sha_path, digest = pack.write_release_zip(stage, zip_path)

    with zipfile.ZipFile(zip_path) as zf:
        assert zf.testzip() is None
        assert set(pack.ZIP_REQUIRED_PATHS) <= set(zf.namelist())
        manifest = json.loads(zf.read("pack-manifest.json"))
        assert manifest["version"] == VERSION
        config = tomllib.loads(zf.read("live2d-ai.toml").decode("utf-8"))
        assert config["persona"]["system_prompt"] == ""
        launcher = zf.read("start-desk-pet-obs.cmd").decode("ascii")
        assert launcher == pack.DESK_PET_OBS_LAUNCHER
        asset_entries = {
            name for name in zf.namelist() if name.startswith(pack.ASSET_PUBLISH_ROOTS)
        }
        assert asset_entries == pack.ASSET_PUBLISH_ALLOWLIST
        release_notes = zf.read("RELEASE_NOTES.md").decode("utf-8")
        assert DEEPSEEK_CHAN_SOURCE_URL in release_notes
        assert "不随包分发" in release_notes
        assert DEEPSEEK_CHAN_SOURCE_URL in zf.read("README.txt").decode("utf-8")
    assert digest == hashlib.sha256(zip_path.read_bytes()).hexdigest()
    assert sha_path.read_text(encoding="utf-8") == f"{digest}  {zip_path.name}\n"
    checked = subprocess.run(
        ["sha256sum", "-c", sha_path.name],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=False,
    )
    assert checked.returncode == 0, checked.stderr
    assert pack.PACKAGE_ZIP_NAME + ": OK" in checked.stdout


@pytest.mark.parametrize(
    "relative",
    [
        "assets/models/大肥鱼/runtime/大肥鱼.model3.json",
        "assets/models/大肥鱼/作者原始发布.zip",
        "assets/models/大肥鱼/大肥鱼.psd",
        "assets/models/ordinary-model/runtime/renamed.moc3",
        "assets/models/DeepSeek-chan/runtime/DeepSeek-chan.model3.json",
        "assets/models/deepseek_chan/runtime/renamed.moc3",
        "assets/models/DeepSeek chan/renamed.psd",
        "assets/actions/new-presets.json",
    ],
)
def test_unapproved_asset_payload_is_rejected(tmp_path: Path, relative: str) -> None:
    stage = _complete_stage(tmp_path)
    payload = stage / relative
    payload.parent.mkdir(parents=True, exist_ok=True)
    payload.write_bytes(b"payload is rejected by its path, not by inspecting its contents")
    zip_path = tmp_path / pack.PACKAGE_ZIP_NAME

    with pytest.raises(SystemExit, match="只允许明确发布白名单"):
        pack.write_release_zip(stage, zip_path)
    assert not zip_path.exists()
    assert not zip_path.with_name(zip_path.name + ".sha256").exists()
    assert list(tmp_path.glob(".*.tmp")) == []


def test_nonempty_persona_is_rejected_without_same_name_artifacts(tmp_path: Path) -> None:
    stage = _complete_stage(tmp_path, prompt="固定陪伴人设")
    zip_path = tmp_path / pack.PACKAGE_ZIP_NAME
    with pytest.raises(SystemExit, match="system_prompt 必须为空"):
        pack.write_release_zip(stage, zip_path)
    assert not zip_path.exists()
    assert not zip_path.with_name(zip_path.name + ".sha256").exists()
    assert list(tmp_path.glob(".*.tmp")) == []


@pytest.mark.parametrize("occupied", ["zip", "sha256"])
def test_existing_formal_files_fail_without_content_changes(
    tmp_path: Path, occupied: str
) -> None:
    stage = _complete_stage(tmp_path)
    zip_path = tmp_path / pack.PACKAGE_ZIP_NAME
    sha_path = zip_path.with_name(zip_path.name + ".sha256")
    existing_path = zip_path if occupied == "zip" else sha_path
    other_path = sha_path if occupied == "zip" else zip_path
    original = ("existing " + occupied + " payload\n").encode()
    existing_path.write_bytes(original)

    with pytest.raises(SystemExit, match="人工备份/移动旧产物或换输出位置"):
        pack.write_release_zip(stage, zip_path)
    assert existing_path.read_bytes() == original
    assert not other_path.exists()
    assert list(tmp_path.glob(".*.tmp")) == []


def test_checksum_publish_failure_preserves_verified_zip(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    stage = _complete_stage(tmp_path)
    zip_path = tmp_path / pack.PACKAGE_ZIP_NAME
    sha_path = zip_path.with_name(zip_path.name + ".sha256")
    real_replace = pack.os.replace

    def fail_checksum_replace(src: Path, dst: Path) -> None:
        if str(dst).endswith(".sha256"):
            raise OSError("simulated checksum publish failure")
        real_replace(src, dst)

    monkeypatch.setattr(pack.os, "replace", fail_checksum_replace)
    with pytest.raises(OSError, match="simulated"):
        pack.write_release_zip(stage, zip_path)
    with zipfile.ZipFile(zip_path) as zf:
        assert zf.testzip() is None
    assert not sha_path.exists()
    assert list(tmp_path.glob(".*.tmp")) == []


def test_obs_launcher_is_required_and_uses_expected_cmd_environment() -> None:
    assert "start-desk-pet-obs.cmd" in pack.ZIP_REQUIRED_PATHS
    launcher = pack.DESK_PET_OBS_LAUNCHER
    assert launcher.isascii()
    lines = launcher.splitlines()
    assert lines[0] == "@echo off"
    assert "setlocal" in lines
    assert 'cd /d "%~dp0"' in lines
    assert 'set "LIVE2D_AI_WEB_AUDIO=server"' in lines
    assert 'set "LIVE2D_AI_DESK_PET_CHROMA=green"' in lines
    assert 'set "LIVE2D_AI_WS_AUDIO="' in lines
    assert '"%~dp0live2d-ai-desktop.exe" --desk-pet --http-port 18080' in lines
    assert not any(line.lower().startswith("start ") for line in lines)
    assert lines[-1] == "endlocal"
    assert "OBS Window Capture + Chroma Key #00FF00" in pack.PACKAGE_README
    assert "强制 opacity=1" in pack.PACKAGE_README
    assert "不设变量默认透明" in pack.PACKAGE_README
    assert "原生 RMS 口型要求 server/cpal 成功" in pack.PACKAGE_README


def test_package_notes_record_bai_exception_and_optional_desk_pet() -> None:
    assert "本包包含 Bai" in pack.PACKAGE_README
    assert "未明确授权公开再分发" in pack.PACKAGE_README
    assert "不表示已获授权或法律合规结论" in pack.PACKAGE_README
    assert "作者提出后即移除" in pack.PACKAGE_README
    assert "desk-pet 是较大的独立可选 Mod，不是核心" in pack.PACKAGE_README
