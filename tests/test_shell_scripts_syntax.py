"""仓库根 shell 脚本的语法与点火判据门禁（2026-10-19，0.3.9 rc.4 补）。

# 为什么要有这一条

0.3.9 rc.4 验证时抓到 `scripts/ignite.sh` 的一个**真实缺陷**：新增的一行用
**全角** `）` 去收 `$( ... )`，而命令替换只认 ASCII `)`——于是 `$(` 一直不
闭合，`bash -n` 报 `unexpected EOF while looking for matching '"'`，**整脚本
跑不起来**。cargo / flutter / pytest 都不检查 shell 语法，所以这类缺陷能穿过
整轮门禁；这里补一条可失败判据。

判据两条：
1. `scripts/*.sh` 每个都能被 `bash -n` 解析；
2. `ignite.sh` 的就绪等待只认 `[tts].mode`——真跑那段读配置 / 打印的代码，
   cloud 必须打印「云端端点，不等本机进程」，local 必须打印「本机进程随点火
   拉起并等待」（不是按 base_url 主机名猜）。
"""

from __future__ import annotations

import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = sorted((REPO_ROOT / "scripts").glob("*.sh"))
IGNITE = REPO_ROOT / "scripts" / "ignite.sh"


def test_scripts_dir_is_not_empty() -> None:
    assert SCRIPTS, "scripts/*.sh 一个都没找到（路径不对？）"


def test_every_shell_script_parses() -> None:
    failures = []
    for script in SCRIPTS:
        proc = subprocess.run(
            ["bash", "-n", str(script)], capture_output=True, text=True, check=False
        )
        if proc.returncode != 0:
            failures.append(f"{script.name}: {proc.stderr.strip()}")
    assert not failures, "shell 语法错误（bash -n）：\n" + "\n".join(failures)


def _ignite_readiness_block() -> str:
    """抽出 ignite.sh 里「读 [tts] 配置 → 打印就绪判据」那一段。"""
    lines = IGNITE.read_text(encoding="utf-8").splitlines()
    start = next(
        i for i, line in enumerate(lines) if line.startswith("mapfile -t ENDPOINTS")
    )
    end = next(
        i for i, line in enumerate(lines) if "TTS 地址主机" in line
    )
    return "\n".join(lines[start : end + 1])


def _run_readiness_block(tmp_path: Path, toml: str) -> str:
    (tmp_path / "live2d-ai.toml").write_text(toml, encoding="utf-8")
    block = tmp_path / "block.sh"
    block.write_text(_ignite_readiness_block(), encoding="utf-8")
    proc = subprocess.run(
        ["bash", str(block)],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, f"读配置段失败：{proc.stderr}"
    return proc.stdout


def test_ignite_readiness_follows_tts_mode_not_host(tmp_path: Path) -> None:
    cloud = _run_readiness_block(
        tmp_path,
        '[tts]\nmode = "cloud"\nbase_url = "https://tts.example.com/v1"\n',
    )
    assert "配置 TTS mode : cloud（云端端点，不等本机进程）" in cloud, cloud

    local = _run_readiness_block(
        tmp_path,
        '[tts]\nmode = "local"\nbase_url = "http://127.0.0.1:8091/v1"\n',
    )
    assert "配置 TTS mode : local（本机进程随点火拉起并等待）" in local, local

    # 关键反差：**云端地址写成本机**时仍按 mode 判「不等」——旧口径会去等一个
    # 永远不会起的 Melo（这正是 0.3.9 要修的那条）。
    cloud_via_loopback = _run_readiness_block(
        tmp_path,
        '[tts]\nmode = "cloud"\nbase_url = "http://127.0.0.1:8091/v1"\n',
    )
    assert "云端端点，不等本机进程" in cloud_via_loopback, cloud_via_loopback
