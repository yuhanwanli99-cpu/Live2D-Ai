#!/usr/bin/env python3
"""CosyVoice3 的**薄** OpenAI 兼容适配层（POST /audio/speech，s16le PCM）。

它是启动脚本的一部分：把 CosyVoice 的推理结果收成主链已经在用的 OpenAI 兼容
形态。**它本身不是 CosyVoice 的实现**，也不改主链的 tts.rs。

边界（与计划书一致，逐条可查）：
  * 只服务 /v1/audio/speech 与 /v1/models（连通性自检打的是 /models）；
  * response_format 只认 pcm（s16le 裸流）；wav 带 44 字节头；其余明确 400；
  * **采样率按请求来**：请求里带 sample_rate 且与本模型输出不一致 -> 400。
    做不到就让这一次请求失败，不回一套错的 PCM 还报 200；
  * 失败一律非 0 / 5xx + 一行原因，不静默降级、不重试、不改地址。

源码由 engine/download.sh 按固定修订号检出到**仓库外**；模型类名随修订号而变，
所以这里按候选列表逐个试，全都载不起来就**明确报错**——不假装在服务。
"""

import argparse
import importlib
import io
import json
import sys
import threading
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

MODEL_ID = "fun-cosyvoice3-0.5b"
DEFAULT_VOICE = "中文女"


def pcm_s16le(speech) -> bytes:
    """torch tensor / ndarray -> s16le 裸字节（与 tts.rs 的 pcm 口径一致）。"""
    import numpy as np

    if hasattr(speech, "detach"):
        speech = speech.detach().cpu().numpy()
    arr = np.asarray(speech, dtype="float32").reshape(-1)
    arr = np.clip(arr, -1.0, 1.0)
    return (arr * 32767.0).astype("<i2").tobytes()


def wav_container(pcm: bytes, sample_rate: int) -> bytes:
    buf = io.BytesIO()
    with wave.open(buf, "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(pcm)
    return buf.getvalue()


class CosyVoiceEngine:
    """把「载模型 + 合成一句」收成一个对象；其余都是 HTTP 的事。"""

    def __init__(self, src_dir: str, weights_dir: str) -> None:
        sys.path.insert(0, src_dir)
        self.model = self._load(weights_dir)
        rate = getattr(self.model, "sample_rate", 0)
        self.sample_rate = int(rate) if rate else 24000
        self.voices = self._voices()
        self.lock = threading.Lock()

    @staticmethod
    def _load(weights_dir: str):
        failures = []
        try:
            module = importlib.import_module("cosyvoice.cli.cosyvoice")
        except Exception as exc:  # noqa: BLE001 - 载不起来就把原因原样报出去
            raise RuntimeError("导入 cosyvoice.cli.cosyvoice 失败：" + repr(exc)) from exc
        for name in ("CosyVoice3", "AutoModel", "CosyVoice2", "CosyVoice"):
            klass = getattr(module, name, None)
            if klass is None:
                failures.append(name + ": 这个修订号里没有这个类")
                continue
            for call in (
                lambda k=klass: k(weights_dir),
                lambda k=klass: k(model_dir=weights_dir),
            ):
                try:
                    return call()
                except Exception as exc:  # noqa: BLE001
                    failures.append(name + ": " + repr(exc))
        raise RuntimeError("按固定修订号载入 CosyVoice 失败（不是「模型不存在」）：" + " | ".join(failures))

    def _voices(self):
        node = self.model
        for attr in ("frontend", "frontend"):
            node = getattr(node, attr, node)
        for attr in ("spk2id", "speaker2id"):
            table = getattr(node, attr, None)
            if hasattr(table, "keys"):
                return [str(k) for k in table.keys()]
        return []

    def synth(self, text: str, voice: str) -> bytes:
        if not text.strip():
            raise ValueError("input 为空")
        chosen = voice or (self.voices[0] if self.voices else DEFAULT_VOICE)
        if self.voices and chosen not in self.voices:
            raise ValueError("未知音色 " + chosen + "（可用：" + ", ".join(self.voices) + "）")
        chunks = []
        with self.lock:
            for out in self.model.inference_sft(text, chosen, stream=False):
                speech = out.get("tts_speech") if isinstance(out, dict) else out
                if speech is None:
                    continue
                chunks.append(pcm_s16le(speech))
        if not chunks:
            raise RuntimeError("推理没有产出音频（tts_speech 一路为空）")
        return b"".join(chunks)


class Handler(BaseHTTPRequestHandler):
    server_version = "live2d-ai-cosyvoice3/1"

    def log_message(self, fmt, *args):  # 日志走 stderr，交给宿主终端
        sys.stderr.write("[cosyvoice3] " + (fmt % args) + "\n")

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
    parser = argparse.ArgumentParser(description="CosyVoice3 薄适配层")
    parser.add_argument("--src", required=True, help="CosyVoice 源码目录（仓库外缓存）")
    parser.add_argument("--weights", required=True, help="Fun-CosyVoice3-0.5B-2512 权重目录")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8080)
    args = parser.parse_args()

    try:
        engine = CosyVoiceEngine(args.src, args.weights)
    except Exception as exc:  # noqa: BLE001 - 起不来就以非 0 退出
        sys.stderr.write("[cosyvoice3] 载入模型失败：" + repr(exc) + "\n")
        return 2
    sys.stderr.write(
        "[cosyvoice3] 就绪：" + args.host + ":" + str(args.port)
        + " sample_rate=" + str(engine.sample_rate)
        + " voices=" + str(len(engine.voices)) + "\n"
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
