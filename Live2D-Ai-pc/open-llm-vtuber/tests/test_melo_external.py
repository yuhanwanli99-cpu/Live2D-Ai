"""MeloExternalClient 单元测试（不启动真实外部进程）。

用内存 StringIO 模拟 worker stdin/stdout，验证 JSON-Lines 协议、请求/响应配对
与进程退出时的错误唤醒。
"""
import io
import json
import sys

import pytest

from open_llm_vtuber.tts import melo_external, melo_worker


class FakeProc:
    def __init__(self, response_lines=None):
        self.stdin = io.StringIO()
        self.stdout = io.StringIO(
            "\n".join(response_lines or []) + ("\n" if response_lines else "")
        )
        self.stderr = io.StringIO("worker-stderr-line\n")

    def poll(self):
        return None


def test_synthesize_sends_payload_and_returns_path(monkeypatch):
    response = {"id": 0, "ok": True, "path": "/tmp/out.wav"}
    fake_proc = FakeProc([json.dumps(response)])

    def fake_popen(*args, **kwargs):
        return fake_proc

    monkeypatch.setattr(melo_external.subprocess, "Popen", fake_popen)
    client = melo_external.MeloExternalClient(python_path=sys.executable)
    path = client.synthesize(
        text="你好",
        language="ZH",
        device="cpu",
        speaker=None,
        speed=1.0,
        sdp_ratio=0.2,
        noise_scale=0.6,
        noise_scale_w=0.8,
        format="wav",
        output_path="/tmp/out.wav",
        timeout=5,
    )
    assert path == "/tmp/out.wav"
    payload = json.loads(fake_proc.stdin.getvalue().strip())
    assert payload["text"] == "你好"
    assert payload["language"] == "ZH"
    assert payload["device"] == "cpu"
    assert payload["speed"] == 1.0
    assert payload["output"] == "/tmp/out.wav"
    client.close()


def test_worker_sentence_counter():
    assert melo_worker._count_sentences("你好。世界！") == 2
    assert melo_worker._count_sentences("你好，世界") == 0


def test_synthesize_raises_on_worker_error(monkeypatch):
    response = {"id": 0, "ok": False, "error": "模型加载失败"}
    fake_proc = FakeProc([json.dumps(response)])

    def fake_popen(*args, **kwargs):
        return fake_proc

    monkeypatch.setattr(melo_external.subprocess, "Popen", fake_popen)
    client = melo_external.MeloExternalClient(python_path=sys.executable)
    with pytest.raises(RuntimeError, match="模型加载失败"):
        client.synthesize(
            text="你好",
            language="ZH",
            device="cpu",
            speaker=None,
            speed=1.0,
            sdp_ratio=0.2,
            noise_scale=0.6,
            noise_scale_w=0.8,
            format="wav",
            output_path="/tmp/out.wav",
            timeout=5,
        )
    client.close()
