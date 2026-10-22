#!/usr/bin/env python3
"""0.4.0 Windows 包的实现体（由 scripts/pack-windows.sh 调用，不单独对外）。

职责（与 docs/versions/0.4.md 一致）：
  1. 用 mktemp 临时根 + 「git worktree add --detach <BASE_SHA>」起一次性干净树；
  2. 七个兄弟仓库存在才「git archive HEAD」导出到临时根（目录名必须是
     Live2D-Ai / live2d-ai-mod-*，让它们 Cargo.toml 的 ../Live2D-Ai/... path 依赖自洽）；
  3. 在临时树里按 docs/architecture/two-repo-mods.md 第 4 节五步链挂载：根/desktop
     Cargo.toml path 依赖、main.rs 工厂表+工厂数断言、宿主集成代码（从拆仓前
     提交 6a0a1ef 恢复 external/voice/a2a/coalesce + mod.rs/cli_entry.rs glue）、
     缺省 manifest 只启用 external-input + local-tts-melo(with_app)；
  4. 验证：Linux cargo check、windows-msvc check（缺 MSVC 原生工具链，记录原文）、
     windows-gnu check（本机唯一可走通）、gnu release 编 exe；
  5. trunk 构建 /render wasm、flutter build web、组装 dist/windows/、可选 zip；
  6. 临时树一律删除（含失败路径），并验证 git worktree list 恢复原状。

红线：不改主仓、不提交、不 push；不克隆兄弟仓库、不下载权重；不拷维护者的
live2d-ai.toml / .env / mods.json；不点火、不占 18080；没有 Windows 实机就写未验。
"""
from __future__ import annotations

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

MOD_NAMES = [
    "external-input",
    "persona",
    "voice-input",
    "memory",
    "local-tts",
    "local-engine",
    "a2a",
]
ORDER = MOD_NAMES

# name -> (workspace dep key, crate ident, factory const)
INFO = {
    "external-input": ("live2d-ai-mod-external-input", "live2d_ai_mod_external_input", "FACTORY"),
    "persona": ("live2d-ai-mod-persona", "live2d_ai_mod_persona", "FACTORY"),
    "voice-input": ("live2d-ai-mod-voice-input", "live2d_ai_mod_voice_input", "FACTORY"),
    "memory": ("live2d-ai-mod-memory", "live2d_ai_mod_memory", "FACTORY"),
    "local-tts": ("live2d-ai-mod-local-tts", "live2d_ai_mod_local_tts", "MELO_FACTORY"),
    "local-engine": ("live2d-ai-mod-local-engine", "live2d_ai_mod_local_engine", "FACTORY"),
    "a2a": ("live2d-ai-mod-a2a", "live2d_ai_mod_a2a", "FACTORY"),
}

PROXY_KEYS = [
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
]

MSVC_TARGET = "x86_64-pc-windows-msvc"
GNU_TARGET = "x86_64-pc-windows-gnu"


def log(msg: str) -> None:
    print("==> " + msg, flush=True)


def note(msg: str) -> None:
    print("    " + msg, flush=True)


def warn(msg: str) -> None:
    print("[warn] " + msg, file=sys.stderr, flush=True)


def eprint(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


def die(msg: str) -> None:
    raise SystemExit("[error] " + msg)


def clean_env() -> dict:
    """清代理 / NO_COLOR（0.4 既有纪律：跑 cargo/flutter 前清掉）。"""
    env = os.environ.copy()
    for key in PROXY_KEYS + ["NO_COLOR"]:
        env.pop(key, None)
    return env


def run_logged(label: str, cmd: list, log_path: Path, cwd=None,
               env=None, fatal: bool = True) -> bool:
    log("运行：" + label)
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with open(log_path, "wb") as fh:
        rc = subprocess.run(cmd, cwd=None if cwd is None else str(cwd), env=env,
                            stdout=fh, stderr=subprocess.STDOUT).returncode
    if rc == 0:
        note("[ok] " + label + "（日志 " + str(log_path) + "）")
        return True
    warn("[fail] " + label + "（exit " + str(rc) + "，日志 " + str(log_path) + "）")
    try:
        tail = log_path.read_text(encoding="utf-8", errors="replace").splitlines()[-30:]
        eprint("\n".join(tail))
    except OSError:
        pass
    if fatal:
        die(label + " 失败（原文见 " + str(log_path) + "）")
    return False


def git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess:
    proc = subprocess.run(["git", "-C", str(repo), *args],
                          capture_output=True, text=True)
    if check and proc.returncode != 0:
        die("git " + " ".join(args) + " 失败：" + (proc.stderr.strip() or proc.stdout.strip()))
    return proc


def target_installed(triple: str) -> bool:
    rustup = shutil.which("rustup")
    if rustup:
        proc = subprocess.run([rustup, "target", "list", "--installed"],
                              capture_output=True, text=True)
        if proc.returncode == 0:
            return triple in proc.stdout.split()
    proc = subprocess.run(["rustc", "--print", "target-libdir", "--target", triple],
                          capture_output=True, text=True)
    return proc.returncode == 0 and bool(proc.stdout.strip())


def export_repo_head(src: Path, dest: Path) -> None:
    """git archive HEAD | tar -x：只导出已提交内容（无 .git/target/权重）。"""
    dest.mkdir(parents=True, exist_ok=True)
    p1 = subprocess.Popen(["git", "-C", str(src), "archive", "HEAD"],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    p2 = subprocess.Popen(["tar", "-x", "-C", str(dest)], stdin=p1.stdout)
    assert p1.stdout is not None
    p1.stdout.close()
    rc2 = p2.wait()
    rc1 = p1.wait()
    if rc1 != 0 or rc2 != 0:
        err = p1.stderr.read().decode("utf-8", "replace") if p1.stderr else ""
        die("导出兄弟仓库失败：" + str(src) + "（git archive rc=" + str(rc1)
            + "，tar rc=" + str(rc2) + "）" + err)


def restore_from_commit(wtree: Path, commit: str, rel: str) -> None:
    proc = subprocess.run(["git", "-C", str(wtree), "show", commit + ":" + rel],
                          capture_output=True)
    if proc.returncode != 0:
        die("无法从 " + commit + " 恢复 " + rel + "（--source-commit 错？）")
    path = wtree / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(proc.stdout)
    note("[restore] " + rel + " ← " + commit)


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write_text(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")


def replace_once(text: str, old: str, new: str, what: str) -> str:
    n = text.count(old)
    if n != 1:
        die("glue 锚点不匹配（" + what + "）：期望 1 处，实际 " + str(n)
            + " 处。基线可能改动了这段代码，请最小适配 scripts/pack_windows_glue.py。")
    return text.replace(old, new, 1)


def patch_tree(wtree: Path, mods: list) -> int:
    """五步链 1-5：Cargo 依赖 / 工厂表 / 宿主 glue / manifest。返回工厂数。"""
    mods = [m for m in ORDER if m in mods]

    # --- 五步链 1：根 [workspace.dependencies]
    root_toml = read_text(wtree / "Cargo.toml")
    anchor = "[workspace.dependencies]\n"
    deps = "".join(
        INFO[m][0] + ' = { path = "../' + INFO[m][0] + '" }\n' for m in mods
    )
    root_toml = replace_once(
        root_toml,
        anchor,
        anchor
        + "# pack-windows.sh 临时树（不提交）：兄弟仓库 git archive 导出后的 path 依赖。\n"
        + deps,
        "根 Cargo.toml [workspace.dependencies]",
    )
    write_text(wtree / "Cargo.toml", root_toml)

    # --- 五步链 2：desktop Cargo.toml
    desk = read_text(wtree / "crates/live2d-ai-desktop/Cargo.toml")
    anchor = 'live2d-ai-mod-system = { path = "../live2d-ai-mod-system" }\n'
    deps = "".join(INFO[m][0] + " = { workspace = true }\n" for m in mods)
    desk = replace_once(
        desk,
        anchor,
        anchor
        + "# pack-windows.sh 临时树（不提交）：七个 Mod 实现编进本二进制（工厂数同步）。\n"
        + deps,
        "desktop Cargo.toml [dependencies]",
    )
    write_text(wtree / "crates/live2d-ai-desktop/Cargo.toml", desk)

    # --- 五步链 3：main.rs 工厂表 + 工厂数断言
    main_rs_path = wtree / "crates/live2d-ai-desktop/src/main.rs"
    main_rs = read_text(main_rs_path)
    empty = ("pub static AVAILABLE_MOD_FACTORIES: "
             "&[&dyn live2d_ai_mod_system::ModFactory] = &[];")
    factories = "".join("    &" + INFO[m][1] + "::" + INFO[m][2] + ",\n" for m in mods)
    main_rs = replace_once(
        main_rs,
        empty,
        "pub static AVAILABLE_MOD_FACTORIES: "
        "&[&dyn live2d_ai_mod_system::ModFactory] = &[\n"
        "    // pack-windows.sh 临时树（不提交）：兄弟仓库导出的 Mod 工厂。\n"
        + factories
        + "];",
        "main.rs AVAILABLE_MOD_FACTORIES",
    )
    count = len(mods)
    test_re = re.compile(r"#\[test\]\n    fn mod_count_is_zero\(\) \{.*?\n    \}\n", re.S)
    match = test_re.search(main_rs)
    if not match:
        die("glue 锚点不匹配（main.rs mod_count_is_zero）：基线改动了工厂数断言，"
            "请最小适配 scripts/pack_windows_glue.py。")
    new_test = (
        "    /// pack-windows.sh 临时树（不提交）：工厂数 = 链入的兄弟仓库数（%d）。\n"
        "    #[test]\n"
        "    fn chained_mod_count_is_expected() {\n"
        "        assert_eq!(\n"
        "            super::AVAILABLE_MOD_FACTORIES.len(),\n"
        "            %d,\n"
        '            "pack-windows.sh 临时树：工厂数必须等于链入的 Mod 仓库数"\n'
        "        );\n"
        "    }\n"
    ) % (count, count)
    main_rs = main_rs[: match.start()] + new_test + main_rs[match.end():]
    write_text(main_rs_path, main_rs)

    # --- 五步链 4：web_api/mod.rs 宿主 glue
    mod_path = wtree / "crates/live2d-ai-desktop/src/web_api/mod.rs"
    mod_rs = read_text(mod_path)
    if "external-input" in mods:
        mod_rs = replace_once(
            mod_rs,
            "pub mod external_queue;\n",
            "pub mod external_queue;\n"
            "// pack-windows.sh 临时树（不提交）：忙时合并窗口 + 常驻合并线程。\n"
            "mod external_coalesce;\n",
            "mod.rs external_coalesce 声明",
        )
        mod_rs = replace_once(
            mod_rs,
            "    pub folder_processes: std::sync::Arc<crate::folder_spawn::FolderProcesses>,\n",
            "    pub folder_processes: std::sync::Arc<crate::folder_spawn::FolderProcesses>,\n"
            "    /// pack-windows.sh 临时树：外部事件忙时合并窗口（3.0 队首第一片）。\n"
            "    pub external_coalescer: external_coalesce::ExternalCoalescer,\n",
            "mod.rs ServerContext.external_coalescer 字段",
        )
        mod_rs = replace_once(
            mod_rs,
            "            folder_processes: Arc::new(crate::folder_spawn::FolderProcesses::new()),\n",
            "            folder_processes: Arc::new(crate::folder_spawn::FolderProcesses::new()),\n"
            "            external_coalescer: external_coalesce::ExternalCoalescer::default(),\n",
            "mod.rs ServerContext::new 初始化",
        )
        sig = "pub fn run_request_loop(server: tiny_http::Server, ctx: ServerContext) {\n"
        mod_rs = replace_once(
            mod_rs,
            sig,
            sig
            + "    // pack-windows.sh 临时树（不提交）：合并线程随服务生命周期起停。\n"
            + "    let coalesce_thread = crate::web_api::external_coalesce::spawn_flusher(ctx.clone());\n",
            "mod.rs run_request_loop 起合并线程",
        )
        start = mod_rs.index(sig)
        end = mod_rs.index("\n}\n", start)
        tail = (
            "\n    // pack-windows.sh 临时树（不提交）：停机唤醒合并线程并等它退出。\n"
            "    ctx.external_coalescer.shutdown();\n"
            "    let _ = coalesce_thread.join();"
        )
        mod_rs = mod_rs[:end] + tail + mod_rs[end:]

    if "a2a" in mods:
        old_a2a = (
            "        if crate::web_api::a2a_gate::is_a2a_path(&path) {\n"
            "            let resp = crate::web_api::a2a_gate::handle_a2a(&ctx);\n"
            '            respond_and_log(request, &method, &path, "a2a", resp);\n'
            "            continue;\n"
            "        }\n"
        )
        new_a2a = (
            "        // pack-windows.sh 临时树（不提交）：A2A 宿主门（协议核在兄弟仓库 Mod）。\n"
            "        if crate::web_api::a2a_gate::is_a2a_path(&path) {\n"
            "            if method == Method::Get {\n"
            "                let resp = crate::web_api::a2a_gate::handle_a2a(\n"
            "                    &ctx,\n"
            "                    crate::web_api::a2a_gate::A2aRequest {\n"
            "                        method: &method,\n"
            "                        path: &path,\n"
            "                        body: &body_str,\n"
            "                        origin: origin.as_deref(),\n"
            "                        content_type: content_type.as_deref(),\n"
            "                    },\n"
            "                );\n"
            '                respond_and_log(request, &method, &path, "a2a", resp);\n'
            "            } else {\n"
            "                let (log_method, log_path) = (method.clone(), path.clone());\n"
            "                crate::web_api::a2a_gate::spawn_a2a(\n"
            "                    ctx.clone(),\n"
            "                    method,\n"
            "                    path,\n"
            "                    body_str,\n"
            "                    origin,\n"
            "                    content_type,\n"
            '                    move |resp| respond_and_log(request, &log_method, &log_path, "a2a", resp),\n'
            "                );\n"
            "            }\n"
            "            continue;\n"
            "        }\n"
        )
        mod_rs = replace_once(mod_rs, old_a2a, new_a2a, "mod.rs A2A 前置分支")
    write_text(mod_path, mod_rs)

    # --- 五步链 5：缺省 manifest 只启用出厂两项
    cli_path = wtree / "crates/live2d-ai-desktop/src/web_api/cli_entry.rs"
    cli = read_text(cli_path)
    enabled = []
    if "external-input" in mods:
        enabled.append('            "external-input": { "enabled": true, "config": {} }')
    if "local-tts" in mods:
        enabled.append(
            '            "local-tts-melo": {\n'
            '                "enabled": true,\n'
            '                "config": { "launch_mode": "with_app" }\n'
            "            }"
        )
    if enabled:
        manifest = (
            "// pack-windows.sh 临时树（不提交）：出厂只启用 external-input + local-tts-melo。\n"
            "    serde_json::json!({\n"
            '        "mods": {\n'
            + ",\n".join(enabled)
            + "\n        }\n"
            "    })"
        )
    else:
        manifest = '    serde_json::json!({ "mods": {} })'
    cli = replace_once(
        cli,
        'serde_json::json!({ "mods": {} })',
        manifest,
        "cli_entry.rs default_mods_manifest",
    )
    test_re = re.compile(
        r"#\[test\]\n    fn default_mods_manifest_has_no_mods\(\) \{.*?\n    \}\n", re.S
    )
    match = test_re.search(cli)
    if match:
        ids = ", ".join(
            '"' + i + '"' for i in (
                (["external-input"] if "external-input" in mods else [])
                + (["local-tts-melo"] if "local-tts" in mods else [])
            )
        )
        new_test = (
            "/// pack-windows.sh 临时树（不提交）：manifest 只启用出厂两项。\n"
            "    #[test]\n"
            "    fn default_mods_manifest_matches_pack() {\n"
            "        let m = default_mods_manifest();\n"
            '        let mut ids: Vec<&str> = m["mods"]\n'
            "            .as_object()\n"
            '            .expect("mods 必须是对象")\n'
            "            .keys()\n"
            "            .map(|k| k.as_str())\n"
            "            .collect();\n"
            "        ids.sort();\n"
            "        assert_eq!(ids, vec![" + ids + "]);\n"
            "    }\n"
        )
        cli = cli[: match.start()] + new_test + cli[match.end():]
    write_text(cli_path, cli)

    note("glue 完成：链入 " + str(count) + " 个 Mod（工厂数 " + str(count) + "）")
    return count


def copy_tree_contents(src: Path, dst: Path) -> None:
    dst.mkdir(parents=True, exist_ok=True)
    for item in sorted(src.iterdir()):
        target = dst / item.name
        if item.is_dir() and not item.is_symlink():
            shutil.copytree(item, target, symlinks=True, dirs_exist_ok=True)
        else:
            shutil.copy2(item, target, follow_symlinks=False)


def archive_paths(wtree: Path, paths: list, dest: Path) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    p1 = subprocess.Popen(["git", "-C", str(wtree), "archive", "HEAD", *paths],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    p2 = subprocess.Popen(["tar", "-x", "-C", str(dest)], stdin=p1.stdout)
    assert p1.stdout is not None
    p1.stdout.close()
    rc2 = p2.wait()
    rc1 = p1.wait()
    if rc1 != 0 or rc2 != 0:
        die("导出 " + " ".join(paths) + " 失败（git archive rc=" + str(rc1)
            + "，tar rc=" + str(rc2) + "）")


def main() -> int:
    here = Path(__file__).resolve().parent
    repo_root = Path(git(here, "rev-parse", "--show-toplevel").stdout.strip())

    parser = argparse.ArgumentParser(
        prog="pack-windows.sh",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        description="0.4.0 Windows 包组装：临时 worktree + 兄弟仓库 git archive + 交叉编译 + dist/windows/",
        epilog=(
            "前置条件：\n"
            "  * 七个兄弟仓库：/home/skystar/live2d-ai-mod-<id>（external-input persona\n"
            "    voice-input memory local-tts local-engine a2a）。存在才导出；缺哪个包里就\n"
            "    不含哪个，报告里列出。不克隆、不下载 Melo 权重。\n"
            "  * Windows 目标：本机唯一可走通 x86_64-pc-windows-gnu。需要 rustup target\n"
            "    add x86_64-pc-windows-gnu，且 ~/.local/bin 下有 MSYS2 mingw64 的\n"
            "    x86_64-w64-mingw32-{gcc,ar,dlltool,ld,windres} shim；脚本自动设置\n"
            "    CC/AR/LINKER 三个环境变量。MSVC 目标只尝试一次并记录缺 lib.exe/link.exe\n"
            "    的原文（工具链没装，不当作脚本失败）。\n"
            "  * Flutter SDK 在 ~/flutter（可用 --flutter-bin 覆盖）与 trunk 0.21+ 在 PATH。\n"
            "  * 磁盘：CARGO_TARGET_DIR 默认 ~/.cache/win-pack-target，脚本按阶段清理并在\n"
            "    结束时删除（--keep-target 保留）。\n"
            "\n"
            "产物（<dist>/windows/）：live2d-ai-desktop.exe（编不出就没有）、\n"
            "live2d-ai.toml（由仓库 example 生成的空密钥模板）、web/（Flutter 构建）、\n"
            "web/render/（l2d-wasm-demo 的 /render wasm 产物）、assets/models/（仅 tracked\n"
            "白名单）、assets/actions/、mods/（仅当临时树有文件夹 Mod）。\n"
            "报告：stdout 摘要 + dist/pack-logs/report-<base7>.txt（dist/ 已忽略）。"
        ),
    )
    parser.add_argument("--base", default="HEAD", metavar="REF",
                        help="临时 worktree 的基线（默认 HEAD；必须是已提交 ref）")
    parser.add_argument("--source-commit", default="6a0a1ef", metavar="COMMIT",
                        help="恢复宿主集成代码的源码提交（默认 6a0a1ef = v0.3.10-rc.3）")
    parser.add_argument("--dist", default=str(repo_root / "dist/windows"), metavar="DIR",
                        help="产物目录（默认 <repo>/dist/windows）")
    parser.add_argument("--target-dir", default=os.environ.get(
        "CARGO_TARGET_DIR", str(Path.home() / ".cache" / "win-pack-target")), metavar="DIR",
                        help="CARGO_TARGET_DIR（默认 ~/.cache/win-pack-target）")
    parser.add_argument("--sibling-root", default=os.environ.get("PACK_SIBLING_ROOT", "/home/skystar"),
                        metavar="DIR", help="兄弟仓库父目录（默认 /home/skystar）")
    parser.add_argument("--flutter-bin", default=os.environ.get(
        "FLUTTER_BIN", str(Path.home() / "flutter/bin/flutter")),
                        metavar="PATH", help="flutter 可执行文件（默认 ~/flutter/bin/flutter）")
    parser.add_argument("--no-zip", action="store_true",
                        help="不生成 dist/live2d-ai-windows-<base>.zip")
    parser.add_argument("--keep-temp", action="store_true", help="保留临时 worktree（调试用）")
    parser.add_argument("--keep-target", action="store_true", help="保留 target 目录（调试用）")
    parser.add_argument("--skip-cargo", action="store_true",
                        help="跳过 cargo 检查/编译（调试用，交付禁用）")
    parser.add_argument("--skip-linux-check", action="store_true",
                        help="跳过 Linux 主机目标 check（调试用，交付禁用）")
    parser.add_argument("--skip-msvc-check", action="store_true",
                        help="跳过 MSVC 目标 check（调试用；上一轮失败原文已记录时）")
    parser.add_argument("--skip-flutter", action="store_true", help="跳过 Flutter Web 构建（调试用）")
    parser.add_argument("--skip-trunk", action="store_true", help="跳过 /render wasm 构建（调试用）")
    args = parser.parse_args()

    dist_dir = Path(args.dist).resolve()
    log_dir = Path(repo_root) / "dist" / "pack-logs"
    target_dir = Path(args.target_dir)
    sibling_root = Path(args.sibling_root)
    flutter_bin = Path(args.flutter_bin)
    source_commit = args.source_commit
    log_dir.mkdir(parents=True, exist_ok=True)
    target_dir.mkdir(parents=True, exist_ok=True)

    # 基线：必须是本仓库可解析的已提交 commit；只取提交树，脏工作树不影响。
    base_sha = git(repo_root, "rev-parse", "--verify", "--quiet", args.base + "^{commit}",
                   check=False).stdout.strip()
    if not base_sha:
        die("--base '" + args.base + "' 不是本仓库里的已提交 ref（请用提交哈希或分支名）")
    if git(repo_root, "cat-file", "-e", base_sha + ":crates/live2d-ai-desktop/src/main.rs",
           check=False).returncode != 0:
        die("提交 " + base_sha + " 里没有 crates/live2d-ai-desktop/src/main.rs"
            + "——不像是 0.3/0.4 的树")
    if git(repo_root, "status", "--porcelain").stdout.strip():
        warn("当前工作树有未提交改动；本脚本只取 --base 指向的已提交树，这些改动不会进包")

    worktrees_before = len([
        line for line in git(repo_root, "worktree", "list", "--porcelain").stdout.splitlines()
        if line.startswith("worktree ")
    ])
    orig_https_proxy = os.environ.get("https_proxy") or os.environ.get("HTTPS_PROXY") or ""

    tmp = Path(tempfile.mkdtemp(prefix="live2d-ai-pack.",
                                dir=os.environ.get("TMPDIR", "/tmp")))
    wtree = tmp / "Live2D-Ai"
    stage = tmp / "stage"
    chained: list = []
    missing: list = []
    stages: list = []

    def stage_snapshot(label: str) -> None:
        """阶段快照：target 体积 + 磁盘可用（报告里逐条列出，便于复盘磁盘）。"""
        try:
            du = subprocess.run(["du", "-sh", str(target_dir)],
                                capture_output=True, text=True).stdout.split()
            target_du = du[0] if du else "?"
        except OSError:
            target_du = "?"
        try:
            free = shutil.disk_usage(target_dir).free / (1024 ** 3)
        except OSError:
            free = 0.0
        line = ("阶段 " + label + "：target=" + target_du
                + "，磁盘可用 " + ("%.1fG" % free))
        note(line)
        stages.append(line)

    result = {
        "base": args.base,
        "base_sha": base_sha,
        "linux": "skip",
        "msvc": "skip",
        "gnu": "skip",
        "exe": "not_built",
        "exe_file": "",
        "exe_sha": "",
        "zip": "",
    }
    try:
        log("基线：" + args.base + " = " + base_sha + "；临时树 " + str(tmp))
        proc = git(repo_root, "worktree", "add", "--detach", str(wtree), base_sha, check=False)
        if proc.returncode != 0:
            die("git worktree add 失败（" + str(wtree) + "）："
                + (proc.stderr.strip() or proc.stdout.strip()))
        note("worktree 就绪（detached at " + base_sha + "）")

        # ------------------------------------------------ 兄弟仓库导出
        log("导出兄弟仓库（git archive HEAD；不克隆、不带 target/.git/权重）")
        for name in MOD_NAMES:
            src = sibling_root / ("live2d-ai-mod-" + name)
            if src.is_dir() and git(src, "rev-parse", "--git-dir",
                                    check=False).returncode == 0:
                dest = tmp / ("live2d-ai-mod-" + name)
                export_repo_head(src, dest)
                chained.append(name)
                sha = git(src, "rev-parse", "--short", "HEAD").stdout.strip()
                note("[chain] " + name + " ← " + str(src) + "（HEAD " + sha + "）")
                if git(src, "status", "--porcelain").stdout.strip():
                    warn("兄弟仓库 " + name + " 有未提交改动：按规范只导出 HEAD，"
                         "未提交改动不会进包")
            else:
                missing.append(name)
                note("[skip]  " + name + "：" + str(src) + " 不存在（包里不含它）")
        if not chained:
            die("一个兄弟仓库都没找到——至少要有 external-input 等目录才能挂载")
        note("链入：" + (" ".join(chained) if chained else "无")
             + "；缺失：" + (" ".join(missing) if missing else "无"))

        # ------------------------------------------------ 恢复宿主集成代码
        if "external-input" in chained:
            restore_from_commit(wtree, source_commit,
                                "crates/live2d-ai-desktop/src/web_api/external_routes.rs")
            restore_from_commit(wtree, source_commit,
                                "crates/live2d-ai-desktop/src/web_api/external_coalesce.rs")
        if "voice-input" in chained:
            restore_from_commit(wtree, source_commit,
                                "crates/live2d-ai-desktop/src/web_api/voice_routes.rs")
        if "a2a" in chained:
            restore_from_commit(wtree, source_commit,
                                "crates/live2d-ai-desktop/src/web_api/a2a_gate.rs")
            restore_from_commit(wtree, source_commit,
                                "crates/live2d-ai-desktop/src/web_api/a2a_gate/llm.rs")

        # ------------------------------------------------ 五步链 glue
        log("在临时树里按 two-repo-mods.md 第 4 节挂载"
            "（Cargo 依赖 + 工厂表 + 宿主 glue + manifest）")
        factory_count = patch_tree(wtree, chained)
        note("临时树工厂数：" + str(factory_count))
        stage_snapshot("整树挂载后（尚未编译）")

        env = clean_env()
        env["CARGO_TARGET_DIR"] = str(target_dir)
        env["CARGO_INCREMENTAL"] = "0"

        def target_dir_free_gb() -> float:
            try:
                return shutil.disk_usage(target_dir).free / (1024 ** 3)
            except OSError:
                return 99.0

        def clean_target_triple(triple: str) -> None:
            for sub in ("debug", triple):
                shutil.rmtree(target_dir / sub, ignore_errors=True)

        # ------------------------------------------------ 1) Linux check
        if not args.skip_cargo:
            if args.skip_linux_check:
                note("--skip-linux-check：跳过 Linux 主机目标 check（调试用）")
            else:
                if target_dir_free_gb() < 1.5:
                    warn("磁盘可用 < 1.5G：交叉编译可能 ENOSPC")
                linux_log = log_dir / ("check-linux-" + base_sha[:7] + ".log")
                ok = run_logged("cargo check -p live2d-ai-desktop（Linux 主机目标）",
                                ["cargo", "check", "-p", "live2d-ai-desktop"],
                                linux_log, cwd=wtree, env=env, fatal=False)
                if not ok:
                    die("Linux 目标 cargo check 失败——先修 glue，别急着交叉编译（原文见 "
                        + str(linux_log) + "）")
                result["linux"] = "pass"
                shutil.rmtree(target_dir / "debug", ignore_errors=True)
                stage_snapshot("Linux check 后（已删 target/debug）")

            # -------------------------------------------- 2) MSVC（预期失败，记录原文）
            if args.skip_msvc_check:
                note("--skip-msvc-check：跳过（上一轮失败原文已记在 pack-logs）")
            elif target_installed(MSVC_TARGET):
                msvc_log = log_dir / ("check-msvc-" + base_sha[:7] + ".log")
                log("cargo check -p live2d-ai-desktop --target " + MSVC_TARGET
                    + "（已知缺 MSVC 原生工具链，ring 会失败）")
                ok = run_logged("MSVC check（预期失败）",
                                ["cargo", "check", "-p", "live2d-ai-desktop",
                                 "--target", MSVC_TARGET],
                                msvc_log, cwd=wtree, env=env, fatal=False)
                if ok:
                    result["msvc"] = "pass"
                    note("意外：MSVC check 通过了（记录为 pass）")
                else:
                    result["msvc"] = "fail_toolchain"
                    eprint("    [record] MSVC 失败原文（头几条 error / 尾 12 行，全文 "
                           + str(msvc_log) + "）：")
                    try:
                        lines = msvc_log.read_text(
                            encoding="utf-8", errors="replace").splitlines()
                        errs = [ln for ln in lines if re.match(
                            r"^(error|error\[|  = note:.*(lib\.exe|link\.exe))", ln)]
                        for ln in errs[:6]:
                            eprint("      " + ln)
                        for ln in lines[-12:]:
                            eprint("      " + ln)
                    except OSError:
                        pass
            else:
                result["msvc"] = "skip_target_missing"
                note("未安装 rust-std " + MSVC_TARGET
                     + "，跳过（记录：Windows MSVC 工具链没装）")
            clean_target_triple(MSVC_TARGET)
            stage_snapshot("MSVC check 后（已删 target/debug + target/" + MSVC_TARGET + "）")

            # -------------------------------------------- 3) windows-gnu check + release exe
            if target_installed(GNU_TARGET):
                gnu = dict(env)
                gnu["PATH"] = (str(Path.home() / ".local/bin")
                               + os.pathsep + gnu.get("PATH", ""))
                gnu.setdefault("CC_x86_64_pc_windows_gnu", "x86_64-w64-mingw32-gcc")
                gnu.setdefault("AR_x86_64_pc_windows_gnu", "x86_64-w64-mingw32-ar")
                gnu.setdefault("CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER",
                               "x86_64-w64-mingw32-gcc")
                if not shutil.which(gnu["CC_x86_64_pc_windows_gnu"], path=gnu["PATH"]):
                    die("缺 " + gnu["CC_x86_64_pc_windows_gnu"]
                        + "（MSYS2 mingw64 shim）；windows-gnu 无法编 C 依赖")
                gnu_log = log_dir / ("check-gnu-" + base_sha[:7] + ".log")
                if run_logged("cargo check -p live2d-ai-desktop --target "
                              + GNU_TARGET + "（GNU 交叉）",
                              ["cargo", "check", "-p", "live2d-ai-desktop",
                               "--target", GNU_TARGET],
                              gnu_log, cwd=wtree, env=gnu, fatal=False):
                    result["gnu"] = "pass"
                    shutil.rmtree(target_dir / GNU_TARGET / "debug", ignore_errors=True)
                    stage_snapshot("windows-gnu check 后（已删 target/" + GNU_TARGET + "/debug）")
                    build_log = log_dir / ("build-gnu-release-" + base_sha[:7] + ".log")
                    ok = run_logged("cargo build --release --target " + GNU_TARGET,
                                    ["cargo", "build", "--release", "-p",
                                     "live2d-ai-desktop", "--target", GNU_TARGET],
                                    build_log, cwd=wtree, env=gnu, fatal=False)
                    if ok:
                        exe_src = (target_dir / GNU_TARGET / "release"
                                   / "live2d-ai-desktop.exe")
                        if exe_src.is_file():
                            stage.mkdir(parents=True, exist_ok=True)
                            shutil.copy2(exe_src, stage / "live2d-ai-desktop.exe")
                            result["exe"] = "built_gnu"
                            result["exe_sha"] = hashlib.sha256(
                                (stage / "live2d-ai-desktop.exe").read_bytes()).hexdigest()
                            proc = subprocess.run(
                                ["file", "-b", str(stage / "live2d-ai-desktop.exe")],
                                capture_output=True, text=True)
                            result["exe_file"] = proc.stdout.strip() or "file 不可用"
                            note("exe：" + result["exe_file"])
                            note("exe sha256：" + result["exe_sha"])
                        else:
                            result["exe"] = "build_ok_no_exe"
                            warn("release build 成功但找不到 live2d-ai-desktop.exe")
                    else:
                        result["exe"] = "build_failed"
                        warn("exe 没编出来：记录原文，继续组装 web/assets"
                             "（Windows 实机未验）")
                    stage_snapshot("windows-gnu release build 后")
                else:
                    die("windows-gnu check 失败（这是本脚本的硬门槛；原文见 "
                        + str(gnu_log) + "）")
            else:
                result["gnu"] = "skip_target_missing"
                warn("未安装 rust-std " + GNU_TARGET
                     + "；无法验证 Windows 目标，exe 不编（记录：工具链没装）")
        else:
            note("--skip-cargo：跳过全部 cargo 检查/编译")
            stage_snapshot("cargo 全部跳过")

        # ------------------------------------------------ /render wasm（trunk）
        trunk_dist = None
        if not args.skip_trunk:
            if not shutil.which("trunk"):
                die("找不到 trunk（或 --skip-trunk）")
            trunk_log = log_dir / ("trunk-" + base_sha[:7] + ".log")
            trunk_cwd = wtree / "crates/l2d-wasm-demo"
            ok = run_logged("trunk build（l2d-wasm-demo，/render 渲染面）",
                            ["trunk", "build"], trunk_log, cwd=trunk_cwd,
                            env=env, fatal=False)
            if not ok and orig_https_proxy:
                warn("trunk 离线失败；按 scripts/ignite.sh 的既有做法"
                     "用 CARGO_HTTP_PROXY 重试一次")
                env2 = dict(env)
                env2["CARGO_HTTP_PROXY"] = orig_https_proxy
                ok = run_logged("trunk build（带 CARGO_HTTP_PROXY 重试）",
                                ["trunk", "build"], trunk_log, cwd=trunk_cwd,
                                env=env2, fatal=False)
            if not ok:
                die("trunk build 失败（/render 是「现有网页 + wasm」的一部分，不跳过）")
            trunk_dist = trunk_cwd / "dist"
            if not (trunk_dist / "index.html").is_file():
                die("trunk build 成功但没有 " + str(trunk_dist / "index.html"))
            stage_snapshot("trunk build 后（wasm32）")
        else:
            note("--skip-trunk：web/render 不带")

        # ------------------------------------------------ Flutter Web
        flutter_web = None
        if not args.skip_flutter:
            if not os.access(flutter_bin, os.X_OK):
                die("找不到 Flutter：" + str(flutter_bin)
                    + "（--flutter-bin 或 --skip-flutter）")
            flutter_log = log_dir / ("flutter-" + base_sha[:7] + ".log")
            fenv = dict(env)
            fenv["PATH"] = (str(Path.home() / "flutter/bin") + os.pathsep
                            + fenv.get("PATH", ""))
            flutter_cwd = wtree / "shell/flutter"
            run_logged("flutter pub get", [str(flutter_bin), "pub", "get"],
                       flutter_log, cwd=flutter_cwd, env=fenv, fatal=True)
            run_logged(
                "flutter build web --release --base-href /app/ --no-web-resources-cdn",
                [str(flutter_bin), "build", "web", "--release", "--base-href",
                 "/app/", "--no-web-resources-cdn"],
                flutter_log, cwd=flutter_cwd, env=fenv, fatal=True)
            flutter_web = flutter_cwd / "build/web"
            if not (flutter_web / "index.html").is_file():
                die("flutter build 成功但没有 " + str(flutter_web / "index.html"))
            stage_snapshot("flutter build web 后")
        else:
            note("--skip-flutter：web/ 只带 web/render（若有）")

        # ------------------------------------------------ 组装 stage
        log("组装产物（先拼临时 stage，成功后再替换 " + str(dist_dir) + "）")
        stage.mkdir(parents=True, exist_ok=True)
        (stage / "web").mkdir(parents=True, exist_ok=True)
        if flutter_web is not None:
            copy_tree_contents(flutter_web, stage / "web")
        if trunk_dist is not None:
            copy_tree_contents(trunk_dist, stage / "web/render")
        archive_paths(wtree, ["assets/models", "assets/actions"], stage)
        if not (stage / "assets/models/README.md").is_file():
            warn("assets/models/README.md 不在导出里（检查基线的 tracked 白名单）")
        # 模板：由 example 生成，绝不拷维护者的 live2d-ai.toml / .env。
        example = wtree / "live2d-ai.toml.example"
        if not example.is_file():
            die("基线没有 live2d-ai.toml.example，无法生成空密钥模板")
        shutil.copy2(example, stage / "live2d-ai.toml")
        # 临时树若有文件夹 Mod（缺省提交没有），才带 mods/。
        mods_tracked = git(wtree, "ls-tree", "-r", "--name-only",
                           "HEAD", "mods").stdout.strip()
        if mods_tracked:
            archive_paths(wtree, ["mods"], stage)
            note("mods/：随基线带了文件夹 Mod")
        else:
            note("mods/：临时树没有文件夹 Mod（缺省树不带，报告里写明）")

        dist_dir.parent.mkdir(parents=True, exist_ok=True)
        if dist_dir.exists():
            shutil.rmtree(dist_dir)
        shutil.move(str(stage), str(dist_dir))
        log("产物已就绪：" + str(dist_dir))
        stage_snapshot("组装完成（dist 已就位）")

        # ------------------------------------------------ 可选 zip
        if not args.no_zip and result["exe"] == "built_gnu":
            zip_path = dist_dir.parent / ("live2d-ai-windows-" + base_sha[:7] + ".zip")
            with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED,
                                 compresslevel=6) as zf:
                for base, _dirs, files in os.walk(dist_dir):
                    for name in sorted(files):
                        full = Path(base) / name
                        zf.write(full, os.path.relpath(full, dist_dir.parent))
            result["zip"] = str(zip_path)
            log("zip：" + str(zip_path))
        elif not args.no_zip:
            note("跳过 zip：没有 exe（不产半成品压缩包）")

        # ------------------------------------------------ 摘要 + 报告
        dist_files = sorted(
            str(p.relative_to(dist_dir)) for p in dist_dir.rglob("*") if p.is_file()
        )
        dist_du = "0"
        if dist_files:
            du = subprocess.run(["du", "-sh", str(dist_dir)],
                                capture_output=True, text=True).stdout.split()
            dist_du = du[0] if du else "0"
        report_lines = [
            "0.4.0 Windows 包（scripts/pack-windows.sh）",
            "基线：" + args.base + " = " + base_sha,
            "恢复宿主集成代码：" + source_commit,
            "链入兄弟仓库（" + str(len(chained)) + "）："
            + (" ".join(chained) if chained else "无"),
            "缺失兄弟仓库（" + str(len(missing)) + "）："
            + (" ".join(missing) if missing else "无"),
            "临时树工厂数：" + str(factory_count),
            "Linux check：" + result["linux"],
            "windows-msvc check：" + result["msvc"]
            + "（缺 MSVC 原生工具链时这是预期结果）",
            "windows-gnu check：" + result["gnu"],
            "exe：" + result["exe"],
            "",
            "阶段快照（target 体积 / 磁盘可用；脚本按阶段清理）：",
        ]
        report_lines.extend(stages)
        report_lines.extend([
            "",
            "清理顺序：Linux check 后删 target/debug；MSVC 后删 target/debug + target/<msvc>；",
            "windows-gnu check 后删 target/<gnu>/debug 再 release；结束删整个 target",
            "（--keep-target 保留）。" if not args.keep_target else "（--keep-target：target 保留）。",
        ])
        if result["exe_file"]:
            report_lines.append("exe file：" + result["exe_file"])
        if result["exe_sha"]:
            report_lines.append("exe sha256：" + result["exe_sha"])
        report_lines.append("产物：" + str(dist_dir) + "（" + dist_du + "）")
        if result["zip"]:
            report_lines.append("zip：" + result["zip"])
        report_lines.append(
            "Windows 实机：未验（本机只有 WSL2 交叉编译，没有 Windows 实机运行）")
        report_lines.append("")
        report_lines.append("产物清单（最多 80 个文件）：")
        report_lines.extend(dist_files[:80])
        report_path = log_dir / ("report-" + base_sha[:7] + ".txt")
        report_path.write_text("\n".join(report_lines) + "\n", encoding="utf-8")
        print("\n".join(report_lines), flush=True)
        log("报告：" + str(report_path))
        return 0
    finally:
        if args.keep_temp:
            print("[info] --keep-temp：临时树保留在 " + str(tmp), flush=True)
        else:
            if wtree.exists():
                subprocess.run(["git", "-C", str(repo_root), "worktree",
                                "remove", "--force", str(wtree)], capture_output=True)
            shutil.rmtree(tmp, ignore_errors=True)
            print("[info] 临时树已删除：" + str(tmp), flush=True)
        if args.keep_target:
            print("[info] --keep-target：target 目录保留在 " + str(target_dir), flush=True)
        else:
            shutil.rmtree(target_dir, ignore_errors=True)
            print("[info] target 目录已删除：" + str(target_dir), flush=True)
        worktrees_after = len([
            line for line in git(repo_root, "worktree",
                                 "list", "--porcelain").stdout.splitlines()
            if line.startswith("worktree ")
        ])
        if worktrees_after == worktrees_before:
            print("[info] git worktree list 已恢复原状（"
                  + str(worktrees_after) + " 个）", flush=True)
        else:
            warn("git worktree list 数量 " + str(worktrees_before) + " -> "
                 + str(worktrees_after) + "（预期恢复原状）")


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(130)
