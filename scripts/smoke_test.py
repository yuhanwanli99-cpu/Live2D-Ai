#!/usr/bin/env python3
"""End-to-end smoke test for the Live2D-Ai-pc portable zip.

Flow (plan §8.5):
  1. Extract the zip into a fresh directory.
  2. Replicate 启动.bat's environment setup exactly (bin/ on PATH, conf.yaml
     generated from conf.yaml.template, cwd = open-llm-vtuber) and launch
     ``.venv\\Scripts\\python.exe run_server.py`` (the bat-equivalent launch;
     the bat itself is exercised separately).
  3. Poll http://127.0.0.1:12393/ until HTTP 200 + Content-Type text/html,
     then check /live2d-models/.../niziiro_mao.model3.json → 200.
  4. WebSocket /client-ws handshake → "Connection established".
  5. Send a text-input message → wait for an LLM full-text reply (a real
     conversation round). TTS audio (edge-tts → pydub → bundled ffmpeg) is
     exercised too when the reply path reaches TTS.
  6. Verify conf.yaml was generated from the template.
  7. Kill the server process tree.

Notes:
  * faster-whisper downloads large-v3-turbo (~1.6 GB) into models/ on first
    launch of a fresh install.  Pass ``--seed-models <repo models dir>`` to
    skip that download during the smoke test (the download path itself is the
    standard faster-whisper mechanism, deliberately not packaged).
  * A real conversation needs outbound network (GLM free tier fallback key)
    and, for the TTS hop, the bundled ffmpeg.  If the LLM is unreachable the
    test still reports "backend ready + frontend + ws reachable" and exits 2.

Usage:
  python scripts/smoke_test.py --zip build/live2d-ai-pc-windows-<sha7>.zip \
      [--seed-models Live2D-Ai-pc/open-llm-vtuber/models] [--port 12393]
"""
from __future__ import annotations

import argparse
import base64
import http.client
import json
import os
import random
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time
import zipfile

APP_DIR = "Live2D-Ai-pc/open-llm-vtuber"


# ── minimal RFC 6455 client (stdlib only) ──────────────────────────────────
class MiniWS:
    def __init__(self, host: str, port: int, path: str, timeout: float = 15.0):
        self.sock = socket.create_connection((host, port), timeout=timeout)
        self.sock.settimeout(timeout)
        self._buf = b""  # bytes read past the HTTP header — first WS frames
        key = base64.b64encode(random.randbytes(16)).decode()
        req = (
            f"GET {path} HTTP/1.1\r\n"
            f"Host: {host}:{port}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n"
        )
        self.sock.sendall(req.encode())
        resp = self._recv_until(b"\r\n\r\n")
        head = resp.split(b"\r\n", 1)[0]
        if b"101" not in head:
            raise RuntimeError(f"WS handshake failed: {head.decode(errors='replace')}")

    def _recv_until(self, marker: bytes) -> bytes:
        """Read until marker; keep any bytes past it as pending frame data."""
        buf = self._buf
        self._buf = b""
        while marker not in buf:
            chunk = self.sock.recv(4096)
            if not chunk:
                break
            buf += chunk
        idx = buf.find(marker)
        if idx >= 0:
            self._buf = buf[idx + len(marker):]
            return buf[:idx + len(marker)]
        return buf

    def _recv_exact(self, n: int) -> bytes:
        # drain pending buffer first, then the socket
        parts = []
        if self._buf:
            take = min(n, len(self._buf))
            parts.append(self._buf[:take])
            self._buf = self._buf[take:]
        while sum(len(p) for p in parts) < n:
            chunk = self.sock.recv(n - sum(len(p) for p in parts))
            if not chunk:
                raise ConnectionError("socket closed mid-frame")
            parts.append(chunk)
        return b"".join(parts)

    def _send_frame(self, payload: bytes, opcode: int = 0x1) -> None:
        mask = os.urandom(4)
        n = len(payload)
        if n < 126:
            header = bytes([0x80 | opcode, 0x80 | n])
        elif n < 65536:
            header = bytes([0x80 | opcode, 0x80 | 126]) + struct.pack(">H", n)
        else:
            header = bytes([0x80 | opcode, 0x80 | 127]) + struct.pack(">Q", n)
        masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        self.sock.sendall(header + mask + masked)

    def send_text(self, text: str) -> None:
        self._send_frame(text.encode("utf-8"), 0x1)

    def recv_text(self) -> str | None:
        """Return the next text message, handling ping/close. None on close."""
        while True:
            b0 = self._recv_exact(2)
            opcode = b0[0] & 0x0F
            masked = b0[1] & 0x80
            n = b0[1] & 0x7F
            if n == 126:
                n = struct.unpack(">H", self._recv_exact(2))[0]
            elif n == 127:
                n = struct.unpack(">Q", self._recv_exact(8))[0]
            mask = self._recv_exact(4) if masked else None
            payload = self._recv_exact(n)
            if mask:
                payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
            if opcode == 0x9:  # ping → pong
                self._send_frame(payload, 0xA)
                continue
            if opcode == 0x8:  # close
                return None
            if opcode == 0x1:
                return payload.decode("utf-8", errors="replace")

    def close(self) -> None:
        try:
            self._send_frame(b"", 0x8)
        except OSError:
            pass
        try:
            self.sock.close()
        except OSError:
            pass


# ── helpers ────────────────────────────────────────────────────────────────
def http_get(port: int, path: str, timeout: float = 5.0) -> tuple[int, str]:
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
    conn.request("GET", path)
    resp = conn.getresponse()
    body = resp.read().decode("utf-8", errors="replace")[:200]
    ctype = resp.getheader("Content-Type", "")
    conn.close()
    return resp.status, f"{ctype} | {body[:80]}"


def kill_tree(proc: subprocess.Popen) -> None:
    try:
        subprocess.run(
            ["taskkill", "/F", "/T", "/PID", str(proc.pid)],
            capture_output=True, timeout=30,
        )
    except Exception:
        try:
            proc.terminate()
        except Exception:
            pass


def wait_for_conf_yaml(app_dir: str, timeout: float = 10.0) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if os.path.isfile(os.path.join(app_dir, "conf.yaml")):
            return True
        time.sleep(0.5)
    return False


def ws_conversation(port: int, prompt: str, timeout: float = 120.0) -> dict:
    """One real text-input conversation round; returns a result dict."""
    out = {"handshake": False, "established": False, "reply": False, "reply_text": ""}
    ws = MiniWS("127.0.0.1", port, "/client-ws", timeout=30)
    deadline = time.time() + timeout

    # The server's first frames are group-update / full-text "Connection
    # established" / set-model-and-conf — loop until the greeting arrives.
    while time.time() < deadline:
        msg = ws.recv_text()
        if msg is None:
            break
        out["handshake"] = True
        if "Connection established" in msg:
            out["established"] = True
            break

    if out["established"]:
        ws.send_text(json.dumps({"type": "text-input", "text": prompt}, ensure_ascii=False))
        while time.time() < deadline:
            msg = ws.recv_text()
            if msg is None:
                break
            try:
                data = json.loads(msg)
            except Exception:
                continue
            if data.get("type") == "full-text":
                text = data.get("text", "")
                if text and "Connection established" not in text:
                    out["reply"] = True
                    out["reply_text"] = text[:200]
                    break
    ws.close()
    return out


def _long_path_prefix(path: str) -> str:
    r"""Windows: \\?\ prefix bypasses MAX_PATH for deep site-packages trees."""
    if os.name == "nt" and not path.startswith("\\\\?\\"):
        return "\\\\?\\" + os.path.abspath(path)
    return path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--zip", required=True, help="built zip artifact")
    parser.add_argument("--port", type=int, default=12393)
    parser.add_argument("--extract-to", default=None, help="extraction dir (default: temp)")
    parser.add_argument("--seed-models", default=None, help="copy <dir>/whisper into the fresh install (skip first-run download)")
    parser.add_argument("--http-timeout", type=float, default=240.0, help="how long to wait for the server to come up")
    parser.add_argument("--no-conversation", action="store_true", help="skip the real-conversation attempt")
    args = parser.parse_args(argv)

    zip_path = os.path.abspath(args.zip)
    keep = bool(args.extract_to)
    extract_dir = args.extract_to or tempfile.mkdtemp(prefix="live2d-ai-smoke-")
    if keep and os.path.isdir(extract_dir):
        shutil.rmtree(extract_dir)
    os.makedirs(extract_dir, exist_ok=True)

    print(f"[smoke_test] extracting {zip_path} -> {extract_dir}")
    with zipfile.ZipFile(zip_path) as zf:
        zf.extractall(_long_path_prefix(extract_dir))

    app_dir = os.path.join(extract_dir, *APP_DIR.split("/"))
    python = os.path.join(app_dir, ".venv", "Scripts", "python.exe")
    run_server = os.path.join(app_dir, "run_server.py")

    # replicate 启动.bat: conf.yaml from template, bin/ on PATH, cwd=app_dir
    if not os.path.isfile(os.path.join(app_dir, "conf.yaml")):
        shutil.copyfile(
            os.path.join(app_dir, "conf.yaml.template"), os.path.join(app_dir, "conf.yaml")
        )
        print("[smoke_test] conf.yaml generated from conf.yaml.template (as 启动.bat does)")
    env = dict(os.environ)
    env["PATH"] = os.path.join(app_dir, "bin") + os.pathsep + env.get("PATH", "")

    if args.seed_models:
        src_models = os.path.join(args.seed_models, "whisper")
        dst_models = os.path.join(app_dir, "models", "whisper")
        if os.path.isdir(src_models) and not os.path.isdir(dst_models):
            print(f"[smoke_test] seeding ASR models {src_models} -> {dst_models} (test shortcut)")
            shutil.copytree(src_models, dst_models)

    print(f"[smoke_test] launching: {python} run_server.py (cwd={app_dir})")
    log_path = os.path.join(extract_dir, "server.log")
    with open(log_path, "wb") as logf:
        proc = subprocess.Popen(
            [python, run_server],
            cwd=app_dir,
            env=env,
            stdout=logf,
            stderr=subprocess.STDOUT,
        )

    results: list[tuple[str, bool, str]] = []
    try:
        # 1. HTTP / up (torch import + ASR engine init can take a while)
        deadline = time.time() + args.http_timeout
        status, detail = 0, "timeout"
        while time.time() < deadline:
            if proc.poll() is not None:
                break
            try:
                status, detail = http_get(args.port, "/")
                if status == 200 and "text/html" in detail:
                    break
            except Exception:
                pass
            time.sleep(2)
        ok = status == 200 and "text/html" in detail
        results.append((f"GET / -> 200 text/html (port {args.port})", ok, detail))
        if not ok:
            print(f"[smoke_test] server did not come up. Last {4000} bytes of log:\n")
            with open(log_path, "rb") as fh:
                fh.seek(max(0, os.path.getsize(log_path) - 4000))
                sys.stderr.write(fh.read().decode(errors="replace"))
            kill_tree(proc)
            return 1

        # 2. live2d model served
        status, detail = http_get(args.port, "/live2d-models/niziiro_mao/runtime/niziiro_mao.model3.json")
        results.append(("GET live2d model3.json -> 200", status == 200, detail))

        # 3. conf.yaml generated (from template)
        conf_ok = wait_for_conf_yaml(app_dir)
        results.append(("conf.yaml present (from template)", conf_ok, ""))

        # 4. WebSocket handshake + real conversation
        ws = ws_conversation(args.port, "请用一句话回答：打包冒烟测试通过了吗？", timeout=120)
        results.append(("WS /client-ws handshake + Connection established", ws["established"], ""))
        results.append(("real LLM full-text reply", ws["reply"], ws["reply_text"] or ""))

        failed = any(not ok for _, ok, _ in results)
        print("\n[smoke_test] results:")
        for label, ok, detail in results:
            print(f"  [{'OK ' if ok else 'FAIL'}] {label}" + (f"  -> {detail}" if detail else ""))
        if failed:
            print("[smoke_test] FAILED")
            return 1
        if not ws["reply"]:
            print("[smoke_test] backend/frontend/ws verified; real conversation skipped "
                  "(LLM unreachable or no API key) — see report for the environment note.")
            return 2
        print("[smoke_test] PASSED (incl. a real conversation round)")
        return 0
    finally:
        kill_tree(proc)


if __name__ == "__main__":
    sys.exit(main())
