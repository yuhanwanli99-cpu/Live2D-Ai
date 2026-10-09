#!/usr/bin/env python3
"""MeloTTS 的**薄** OpenAI 兼容适配层（POST /audio/speech，s16le PCM）。

只做三件事：载 MeloTTS 中文模型、合成一句、按 s16le 裸流回。**不是** MeloTTS
的实现，也不改主链的 tts.rs。

边界（与计划书一致）：
  * 只服务 /v1/audio/speech 与 /v1/models；
  * response_format 只认 pcm（s16le）；wav 带 44 字节头；其余明确 400；
  * **采样率按请求来**：请求里带 sample_rate 且与本模型输出不一致 -> 400。
    做不到就让这一次请求失败，不回一套错的 PCM 还报 200；
  * 中文推理要用的 bert-base-multilingual-uncased **不在仓库里**：缺它就失败，
    错误里**逐字写出缺的是它**，不静默下载完还声称开箱。
"""

import argparse
import io
import json
import os
import sys
import tempfile
import threading
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

MODEL_ID = "melotts-zh"
DEFAULT_VOICE = "ZH"
BERT_MODEL = "bert-base-multilingual-uncased"


class MeloEngine:
    def __init__(self, weights_dir: str) -> None:
        config = os.path.join(weights_dir, "config.json")
        ckpt = os.path.join(weights_dir, "checkpoint.pth")
        for path in (config, ckpt):
            if not os.path.isfile(path):
                raise RuntimeError("缺权重文件：" + path + "（先跑 melo/download.sh）")
        try:
            from melo.api import TTS
        except Exception as exc:  # noqa: BLE001
            raise RuntimeError(
                "载入 melo 失败：" + repr(exc) + "（首次启用时会按 tag v0.1.2 装进 melo/.venv）"
            ) from exc
        try:
            self.model = TTS(language="ZH", device="cpu", config_path=config, ckpt_path=ckpt)
        except Exception as exc:  # noqa: BLE001
            text = repr(exc)
            if BERT_MODEL in text or "bert" in text.lower() or "transformers" in text.lower():
                raise RuntimeError(
                    "中文推理需要 " + BERT_MODEL + " 但它不在仓库里、本机也取不到：" + text
                ) from exc
            raise
        rate = getattr(getattr(self.model, "hps", None), "data", None)
        self.sample_rate = int(getattr(rate, "sampling_rate", 0) or 44100)
        self.speakers = dict(getattr(rate, "spk2id", {}) or {})
        self.lock = threading.Lock()

    def synth(self, text: str, voice: str) -> bytes:
        if not text.strip():
            raise ValueError("input 为空")
        key = voice or DEFAULT_VOICE
        speaker_id = self.speakers.get(key)
        if speaker_id is None:
            raise ValueError(
                "未知音色 " + key + "（可用：" + ", ".join(sorted(self.speakers)) + "）"
            )
        with self.lock, tempfile.TemporaryDirectory() as tmp:
            out = os.path.join(tmp, "out.wav")
            self.model.tts_to_file(text, speaker_id, out, speed=1.0)
            with wave.open(out, "rb") as handle:
                if handle.getsampwidth() != 2:
                    raise RuntimeError("MeloTTS 产出不是 16 bit，拒绝回错的 PCM")
                rate = handle.getframerate()
                if rate != self.sample_rate:
                    raise RuntimeError(
                        "MeloTTS 产出 " + str(rate) + " Hz，与声明的 "
                        + str(self.sample_rate) + " Hz 不一致"
                    )
                return handle.readframes(handle.getnframes())


def wav_container(pcm: bytes, sample_rate: int) -> bytes:
    buf = io.BytesIO()
    with wave.open(buf, "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(pcm)
    return buf.getvalue()


class Handler(BaseHTTPRequestHandler):
    server_version = "live2d-ai-melotts/1"

    def log_message(self, fmt, *args):  # 日志走 stderr，交给宿主终端
        sys.stderr.write("[melo] " + (fmt % args) + "\n")

    def _json(self, code: int, payload) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):  # noqa: N802 - http.server 的固定接口名
        if self.path.rstrip("/") in ("/v1/models", "/models"):
            self._json(200, {"object": "list", "data": [{"id": MODEL_ID, "object": "model"}]})
            return
        self._json(404, {"error": {"message": "no such path: " + self.path}})

    def do_POST(self):  # noqa: N802
        if self.path.rstrip("/") != "/v1/audio/speech":
            self._json(404, {"error": {"message": "no such path: " + self.path}})
            return
        length = int(self.headers.get("Content-Length") or 0)
        try:
            body = json.loads(self.rfile.read(length) or b"{}")
        except ValueError as exc:
            self._json(400, {"error": {"message": "请求体不是 JSON：" + repr(exc)}})
            return
        engine = self.server.engine
        fmt = str(body.get("response_format") or "pcm").lower()
        if fmt not in ("pcm", "wav"):
            self._json(400, {"error": {"message": "只支持 response_format=pcm|wav，收到 " + fmt}})
            return
        asked = body.get("sample_rate")
        if asked is not None and int(asked) != engine.sample_rate:
            self._json(400, {"error": {"message": (
                "本次请求要 " + str(asked) + " Hz，本模型输出 " + str(engine.sample_rate)
                + " Hz；不做重采样（宁可失败也不回一套错的 PCM）")}})
            return
        try:
            pcm = engine.synth(str(body.get("input") or ""), str(body.get("voice") or ""))
        except ValueError as exc:
            self._json(400, {"error": {"message": str(exc)}})
            return
        except Exception as exc:  # noqa: BLE001
            self._json(500, {"error": {"message": "合成本次失败：" + repr(exc)}})
            return
        payload = pcm if fmt == "pcm" else wav_container(pcm, engine.sample_rate)
        self.send_response(200)
        self.send_header("Content-Type", "audio/wav" if fmt == "wav" else "application/octet-stream")
        self.send_header("X-Sample-Rate", str(engine.sample_rate))
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


def main() -> int:
    parser = argparse.ArgumentParser(description="MeloTTS 薄适配层")
    parser.add_argument("--weights", required=True, help="melo/weights 目录")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8091)
    args = parser.parse_args()

    try:
        engine = MeloEngine(args.weights)
    except Exception as exc:  # noqa: BLE001 - 起不来就以非 0 退出
        sys.stderr.write("[melo] 载入模型失败：" + repr(exc) + "\n")
        return 2
    sys.stderr.write(
        "[melo] 就绪：" + args.host + ":" + str(args.port)
        + " sample_rate=" + str(engine.sample_rate)
        + " speakers=" + str(sorted(engine.speakers)) + "\n"
    )
    server = ThreadingHTTPServer((args.host, args.port), Handler)
    server.engine = engine
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
