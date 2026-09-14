#!/usr/bin/env python3
"""voice_sidecar.py —— 语音 → Live2D-Ai 转写注入（本机 sidecar 示例）。

# 这个脚本在哪一层

    麦克风 / 录音文件
        -> ASR（本脚本的 --transcriber，可插拔）
        -> clean_transcript()（与 Rust 侧 live2d_ai_mod_voice_input 同语义）
        -> POST /api/v1/voice/transcript
        -> Live2D-Ai supervisor.say -> LLM -> TTS -> 口型 -> 皮套

**ASR 本体不在 Live2D-Ai 主仓**：任何推理运行时（whisper / sherpa-onnx /
平台 SDK / 云服务 CLI）都住在**本脚本**这一侧。Rust 主仓只收一段文本，
不静态链接 ASR SDK、不开 socket、不读进程环境。

# 只用标准库

`argparse / json / os / shlex / subprocess / sys / unicodedata / urllib`
——不需要 `pip install`。真正的 ASR 由 `--transcriber cmd:"<你的命令>"`
接入（例如 `cmd:"whisper --model small --output_format txt --output_dir -"`）。

# 用法（一条可复制命令）

    python3 docs/examples/voice-sidecar/voice_sidecar.py \\
        --audio docs/examples/voice-sidecar/fixtures/fake_zh.wav --dry-run

去掉 `--dry-run` 即真发送（需要服务已由 `./scripts/ignite.sh` 点火，
且 `voice-input` Mod 已启用——缺省是**停用**，见 docs/voice-input.md §5）。

# 退出码（契约，与 README 失败码表一致）

    0  成功（HTTP 2xx 且 `ok:true`）
    2  参数或依赖错（缺 --audio / 文件读不到 / transcriber 写法非法 /
       `cmd:` 里的可执行文件不存在）
    3  转写失败（.txt 读失败 / 命令非 0 退出 / 超时 / 清洗后为空）
    4  HTTP 非 2xx **或**请求发不出去（连接被拒 / 超时）
    5  服务端 `ok:false`（当前只有 `busy`：主链忙碌，本条已被丢弃，请退避）
"""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys
import unicodedata
import urllib.error
import urllib.request

# ------------------------------------------------------------------ 常量

#: 缺省端点（`./scripts/ignite.sh` 的默认端口 18080）。
DEFAULT_URL = "http://127.0.0.1:18080/api/v1/voice/transcript"

#: 端点路径（URL 只给主机时补这一段）。
ENDPOINT_PATH = "/api/v1/voice/transcript"

#: `--transcriber fake` 且没有同名 `.txt` 时使用的固定串。
DEFAULT_FAKE_TEXT = "这是一条来自 voice sidecar 的假转写"

#: 服务端上限（清洗后字符数）；这里**不**预截断，交给服务端判定并报 text_too_long。
MAX_TEXT_CHARS = 2000

#: 请求超时（秒）。本地回环，给足 30s（ASR 之外的这一段很快）。
DEFAULT_TIMEOUT = 30.0

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_TRANSCRIBE = 3
EXIT_HTTP = 4
EXIT_REJECTED = 5

#: 与 Rust 侧 `clean_transcript` 相同的零宽字符集合。
ZERO_WIDTH = {"\u200b", "\ufeff", "\u200c", "\u200d"}

#: 服务端错误码 → 一句可执行处置（README 失败码表的代码侧真相）。
ERROR_HINTS = {
    "busy": "主链忙碌，本条转写已被丢弃：退避几秒再发，不要立即重试（避免刷屏）",
    "unauthorized": "token 缺失或不匹配：检查 --token / VOICE_INPUT_TOKEN 与服务端是否一致",
    "mod_disabled": "voice-input Mod 已停用：在前端「Mod 管理」启用，或 POST /api/v1/mods/voice-input/enable",
    "empty_transcript": "转写清洗后为空（空白 / 零宽字符）：这段音频没有可注入的文本",
    "text_too_long": f"转写超过 {MAX_TEXT_CHARS} 字符：切短一点再发（服务端按清洗后长度判定）",
    "invalid_payload": "请求体不合法：这通常意味着脚本与服务端版本不匹配",
    "origin_denied": "Origin 非 loopback 同源：本端点只服务本机进程",
    "origin_required": "缺 Origin 且服务端未开 allow_no_origin：用 curl/脚本时给服务端加 --allow-no-origin",
    "method_not_allowed": "方法不对：本端点只接受 POST",
    "unsupported_media_type": "Content-Type 必须是 application/json",
    "supervisor_unavailable": "supervisor 未就绪（配置不完整 / 尚未保存）：先让主链跑起来",
}


class UsageError(Exception):
    """参数或依赖错（退出码 2）。"""


class TranscribeError(Exception):
    """转写失败（退出码 3）。"""


# ------------------------------------------------------------------ 清洗


def clean_transcript(raw: str) -> str | None:
    """清洗一段 ASR 文本；清洗后为空返回 `None`。

    逐条对齐 Rust 侧 `live2d_ai_mod_voice_input::clean_transcript`：

    - 空白（含全角空格 U+3000 / \\t / \\n / \\r）折叠成一个半角空格；
    - 去掉零宽字符（U+200B/U+FEFF/U+200C/U+200D）与控制字符（Unicode Cc）；
    - 去掉首尾空白；结果为空 → `None`（**不发空回合**）。
    """
    out: list[str] = []
    pending_space = False
    for ch in raw:
        if ch.isspace():
            # 前导空白落不下（out 为空），尾随空白在循环结束后自然丢弃。
            pending_space = bool(out)
            continue
        if ch in ZERO_WIDTH or unicodedata.category(ch) == "Cc":
            continue
        if pending_space:
            out.append(" ")
            pending_space = False
        out.append(ch)
    text = "".join(out)
    return text or None


# ------------------------------------------------------------------ 请求形状


def build_payload(text: str, token: str | None) -> dict:
    """构造请求体：`{"text": ...}`，token 非空时才带 `token`。"""
    body: dict = {"text": text}
    if token:
        body["token"] = token
    return body


def mask_token(payload: dict) -> dict:
    """把 token 明文换成 `***`（dry-run / 日志**永不**回显 token）。"""
    safe = dict(payload)
    if "token" in safe:
        safe["token"] = "***"
    return safe


def normalize_url(raw: str) -> str:
    """URL 归一化（`--selftest` 有断言）。

    - 空 → 缺省端点；
    - 无 `scheme://` → 补 `http://`；
    - 只给主机（无路径）→ 补 `ENDPOINT_PATH`；
    - 去掉末尾 `/`。
    """
    url = (raw or "").strip()
    if not url:
        return DEFAULT_URL
    if "://" not in url:
        url = "http://" + url
    url = url.rstrip("/")
    scheme, _, rest = url.partition("://")
    if "/" not in rest:
        url = f"{scheme}://{rest}{ENDPOINT_PATH}"
    return url


def classify_http(status: int, body: object) -> tuple[int, str]:
    """HTTP 状态 + 响应体 → `(退出码, 说明)`（**唯一的错误码映射点**）。"""
    if not isinstance(status, int) or status < 200 or status >= 300:
        code = body_code(body)
        return EXIT_HTTP, f"HTTP {status}" + (f"（{code}）" if code else "")
    if isinstance(body, dict) and body.get("ok") is True:
        return EXIT_OK, "ok"
    code = body_code(body) or "unknown"
    return EXIT_REJECTED, f"服务端 ok:false（{code}）"


def body_code(body: object) -> str | None:
    """从 `{"error":{"code":...}}` 取 code（取不到返回 None）。"""
    if not isinstance(body, dict):
        return None
    err = body.get("error")
    if not isinstance(err, dict):
        return None
    code = err.get("code")
    return code if isinstance(code, str) else None


def hint_for_code(code: str | None) -> str:
    """错误码 → 处置建议。"""
    if code and code in ERROR_HINTS:
        return ERROR_HINTS[code]
    return "见 docs/voice-input.md §4 错误码表"


# ------------------------------------------------------------------ 转写后端


def parse_transcriber(spec: str) -> tuple[str, str]:
    """解析 `--transcriber`：`fake` 或 `cmd:"<shell 命令>"`。

    返回 `("fake", "")` / `("cmd", "<命令>")`；写法非法 → [`UsageError`]。
    """
    spec = (spec or "").strip()
    if spec == "fake":
        return ("fake", "")
    if spec.startswith("cmd:"):
        command = spec[len("cmd:") :].strip()
        if not command:
            raise UsageError('--transcriber cmd: 后面必须是命令，例如 cmd:"whisper --model small"')
        return ("cmd", command)
    raise UsageError(f"未知 --transcriber：{spec!r}（可用 fake 或 cmd:\"<命令>\"）")


def txt_path_for(audio_path: str) -> str:
    """同名 `.txt`（fake 后端的转写来源）。"""
    return os.path.splitext(audio_path)[0] + ".txt"


def transcribe_fake(audio_path: str, fake_text: str | None) -> str:
    """fake 后端：读同名 `.txt`；没有就用 `--fake-text` / 固定串。"""
    path = txt_path_for(audio_path)
    if os.path.exists(path):
        try:
            with open(path, "r", encoding="utf-8") as fh:
                return fh.read()
        except OSError as exc:
            raise TranscribeError(f"读不到同名转写文件 {path}: {exc}") from exc
    return fake_text if fake_text is not None else DEFAULT_FAKE_TEXT


def transcribe_cmd(command: str, audio_path: str, timeout: float) -> str:
    """cmd 后端：把音频路径喂给用户的 ASR CLI，读 stdout。"""
    try:
        args = shlex.split(command)
    except ValueError as exc:
        raise UsageError(f"--transcriber 命令无法解析: {exc}") from exc
    if not args:
        raise UsageError("--transcriber cmd: 命令为空")
    argv = args + [audio_path]
    try:
        proc = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except FileNotFoundError as exc:
        # 可执行文件不存在 = 依赖错（不是转写错）。
        raise UsageError(f"ASR 命令不存在：{argv[0]}（请先安装，或用 --transcriber fake）") from exc
    except subprocess.TimeoutExpired as exc:
        raise TranscribeError(f"ASR 命令超时（>{timeout:g}s）：{argv[0]}") from exc
    except OSError as exc:
        raise UsageError(f"ASR 命令无法启动：{exc}") from exc
    if proc.returncode != 0:
        tail = (proc.stderr or "").strip().splitlines()[-3:]
        detail = " / ".join(tail) if tail else "(无 stderr)"
        raise TranscribeError(f"ASR 命令退出码 {proc.returncode}：{detail}")
    return proc.stdout or ""


# ------------------------------------------------------------------ 发送


def post_transcript(url: str, payload: dict, token: str | None, timeout: float) -> tuple[int, object]:
    """POST 转写；返回 `(status, 解析后的响应体)`。

    非 2xx **也**返回（由 [`classify_http`] 映射退出码），连接失败 → [`UsageError`]
    之外的普通错误由 caller 转成退出码 4。
    """
    data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read().decode("utf-8", errors="replace")
            return resp.status, _parse_json(raw)
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode("utf-8", errors="replace")
        return exc.code, _parse_json(raw)


def _parse_json(raw: str) -> object:
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return {"raw": raw}


# ------------------------------------------------------------------ selftest


def run_selftest() -> int:
    """离线自检（不联网）：清洗语义 / payload 形状 / 错误码映射 / URL 归一化。"""
    failures: list[str] = []
    checks = 0

    def check(name: str, got: object, want: object) -> None:
        nonlocal checks
        checks += 1
        if got != want:
            failures.append(f"{name}: 期望 {want!r}，实际 {got!r}")

    # --- 1. 清洗语义（与 Rust clean_transcript 逐条对齐） ---
    check("clean 折叠空白", clean_transcript("  你好  世界  "), "你好 世界")
    check("clean 换行制表", clean_transcript("你好\n\t世界"), "你好 世界")
    check("clean 全角空格", clean_transcript("\u3000你\u3000\u3000好\u3000"), "你 好")
    check("clean 零宽+控制符", clean_transcript("\ufeff你\u200b好\x07"), "你好")
    check("clean 保留句读", clean_transcript("走吧？好。"), "走吧？好。")
    for blank in ["", "   ", "\n\t\u3000", "\u200b\ufeff"]:
        check(f"clean 纯空白({blank!r})", clean_transcript(blank), None)

    # --- 2. payload 形状 ---
    check("payload 无 token", build_payload("你好", None), {"text": "你好"})
    check("payload 带 token", build_payload("你好", "t"), {"text": "你好", "token": "t"})
    check("payload 空 token 不带键", "token" in build_payload("你好", ""), False)
    check("payload 可 JSON 往返", json.loads(json.dumps(build_payload("你好", "t"))),
          {"text": "你好", "token": "t"})
    check("payload 键集合", sorted(build_payload("你好", "t")), ["text", "token"])
    check("mask 不回显 token", mask_token(build_payload("你好", "secret")),
          {"text": "你好", "token": "***"})

    # --- 3. 错误码映射（HTTP / ok:false → 退出码） ---
    check("200 ok -> 0", classify_http(200, {"ok": True, "text": "hi"})[0], EXIT_OK)
    check("200 busy -> 5", classify_http(200, {"ok": False, "error": {"code": "busy"}})[0], EXIT_REJECTED)
    for status in (400, 401, 403, 405, 415, 503):
        check(f"{status} -> 4", classify_http(status, {"error": {"code": "x"}})[0], EXIT_HTTP)
    for code in ERROR_HINTS:
        check(f"错误码 {code} 有处置建议", hint_for_code(code) != "", True)
    check("未知码有兜底建议", hint_for_code("nope") != "", True)
    check("取 error.code", body_code({"error": {"code": "busy"}}), "busy")
    check("无 error 时不算 busy", classify_http(200, {"ok": False})[0], EXIT_REJECTED)

    # --- 4. URL 归一化 ---
    check("url 空 -> 缺省", normalize_url(""), DEFAULT_URL)
    check("url 无 scheme", normalize_url("127.0.0.1:18080"),
          "http://127.0.0.1:18080" + ENDPOINT_PATH)
    check("url 只给主机", normalize_url("http://h:1"), "http://h:1" + ENDPOINT_PATH)
    check("url 去尾部斜杠", normalize_url("http://h:1" + ENDPOINT_PATH + "/"),
          "http://h:1" + ENDPOINT_PATH)
    check("url 自定义路径保留", normalize_url("http://h:1/custom"), "http://h:1/custom")

    # --- 5. transcriber 解析 ---
    check("transcriber fake", parse_transcriber("fake"), ("fake", ""))
    check("transcriber cmd", parse_transcriber('cmd:whisper --model small'),
          ("cmd", "whisper --model small"))
    for bad in ("", "bogus", "cmd:"):
        try:
            parse_transcriber(bad)
            failures.append(f"transcriber 非法写法 {bad!r} 应报 UsageError")
        except UsageError:
            checks += 1

    # --- 6. fixtures ---
    here = os.path.dirname(os.path.abspath(__file__))
    wav = os.path.join(here, "fixtures", "fake_zh.wav")
    check("fixtures wav 存在", os.path.isfile(wav), True)
    check("fixtures txt 存在", os.path.isfile(txt_path_for(wav)), True)

    if failures:
        print(f"[selftest] {len(failures)} 项失败 / 共 {checks} 项检查", file=sys.stderr)
        for line in failures:
            print(f"  - {line}", file=sys.stderr)
        return 1
    print(f"[selftest] 全部通过（{checks} 项检查；离线，未发起任何请求）")
    return 0


# ------------------------------------------------------------------ main


def build_parser() -> argparse.ArgumentParser:
    """命令行（`--selftest` 不需要 `--audio`）。"""
    p = argparse.ArgumentParser(
        prog="voice_sidecar.py",
        description="语音 -> 转写 -> POST /api/v1/voice/transcript（本机 sidecar 示例）",
    )
    p.add_argument("--audio", help="音频文件（wav 或任意文件；fake 后端按同名 .txt 取转写）")
    p.add_argument(
        "--transcriber",
        default="fake",
        help='转写后端：fake（缺省）或 cmd:"<你的 ASR 命令>"（音频路径会追加到命令末尾）',
    )
    p.add_argument("--fake-text", help="fake 后端在**没有**同名 .txt 时使用的文本")
    p.add_argument("--url", default=os.environ.get("VOICE_INPUT_URL", DEFAULT_URL),
                   help=f"服务端 URL（缺省 {DEFAULT_URL}；env VOICE_INPUT_URL）")
    p.add_argument("--token", default=os.environ.get("VOICE_INPUT_TOKEN", ""),
                   help="可选 token（env VOICE_INPUT_TOKEN；也可放 body，脚本用 Authorization 头）")
    p.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT, help="超时秒数")
    p.add_argument("--dry-run", action="store_true", help="只打印将要 POST 的 URL + JSON，不发请求")
    p.add_argument("--selftest", action="store_true", help="离线自检（不联网；退出码 0/非 0）")
    return p


def main(argv: list[str] | None = None) -> int:
    """入口：返回进程退出码。"""
    args = build_parser().parse_args(argv)

    if args.selftest:
        return run_selftest()

    if not args.audio:
        print("[voice-sidecar] 缺少 --audio（或改用 --selftest）", file=sys.stderr)
        return EXIT_USAGE
    if not os.path.isfile(args.audio):
        print(f"[voice-sidecar] 音频文件不存在：{args.audio}", file=sys.stderr)
        return EXIT_USAGE

    try:
        backend, command = parse_transcriber(args.transcriber)
    except UsageError as exc:
        print(f"[voice-sidecar] 参数错：{exc}", file=sys.stderr)
        return EXIT_USAGE

    token = (args.token or "").strip()

    # 1) 转写。
    try:
        if backend == "fake":
            raw = transcribe_fake(args.audio, args.fake_text)
        else:
            raw = transcribe_cmd(command, args.audio, args.timeout)
    except UsageError as exc:
        print(f"[voice-sidecar] 依赖错：{exc}", file=sys.stderr)
        return EXIT_USAGE
    except TranscribeError as exc:
        print(f"[voice-sidecar] 转写失败：{exc}", file=sys.stderr)
        return EXIT_TRANSCRIBE

    # 2) 清洗（与服务端同一语义；服务端还会再清洗一次）。
    cleaned = clean_transcript(raw)
    if cleaned is None:
        print("[voice-sidecar] 转写失败：清洗后为空（不发空回合）", file=sys.stderr)
        return EXIT_TRANSCRIBE
    if len(cleaned) > MAX_TEXT_CHARS:
        print(
            f"[voice-sidecar] 提示：清洗后 {len(cleaned)} 字符，超过服务端上限 "
            f"{MAX_TEXT_CHARS}，预计会被 400 text_too_long 拒绝",
            file=sys.stderr,
        )

    url = normalize_url(args.url)
    payload = build_payload(cleaned, token)

    # 3) dry-run：只打印（token 打码）。
    if args.dry_run:
        print(f"[voice-sidecar] dry-run（未发请求）")
        print(f"  POST {url}")
        print(f"  body {json.dumps(mask_token(payload), ensure_ascii=False)}")
        return EXIT_OK

    # 4) 发送。
    try:
        status, body = post_transcript(url, payload, token, args.timeout)
    except urllib.error.URLError as exc:
        print(f"[voice-sidecar] 请求发不出去：{exc.reason}", file=sys.stderr)
        print("[voice-sidecar] 服务是否已点火？（./scripts/ignite.sh）", file=sys.stderr)
        return EXIT_HTTP
    except OSError as exc:
        print(f"[voice-sidecar] 请求发不出去：{exc}", file=sys.stderr)
        return EXIT_HTTP

    code = body_code(body)
    exit_code, summary = classify_http(status, body)
    if exit_code == EXIT_OK:
        print(f"[voice-sidecar] {summary}（HTTP {status}）：已注入「{cleaned}」")
    else:
        print(f"[voice-sidecar] {summary}", file=sys.stderr)
        print(f"[voice-sidecar] 处置：{hint_for_code(code)}", file=sys.stderr)
        if code == "busy":
            print("[voice-sidecar] 服务端返回 200 + ok:false 是**刻意**的：本条已丢弃，退避即可", file=sys.stderr)
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
