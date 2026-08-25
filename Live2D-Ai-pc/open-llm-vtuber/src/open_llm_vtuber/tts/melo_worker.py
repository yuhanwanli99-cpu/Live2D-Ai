#!/usr/bin/env python
"""MeloTTS 外部 worker（在 melo conda 环境内运行）。

Live2D-Ai PC 后端通过 stdin/stdout JSON-Lines 与此进程通信，避免每次合成
都重新加载 10~30s 的 Melo 模型。协议：

请求（一行 JSON）:
    {
      "id": 1,
      "text": "你好，这是对接测试。",
      "language": "ZH",
      "device": "cpu",
      "speaker": null,
      "speed": 1.0,
      "sdp_ratio": 0.2,
      "noise_scale": 0.6,
      "noise_scale_w": 0.8,
      "format": "wav",
      "output": "/abs/path/out.wav"
    }

响应（一行 JSON）:
    {"id": 1, "ok": true, "path": "/abs/path/out.wav"}
    {"id": 1, "ok": false, "error": "..."}
"""
import json
import os
import re
import sys
import traceback

_MODELS: dict = {}
_MAX_TEXT_LEN = int(os.environ.get("MELO_MAX_TEXT_LEN", "200"))
_MAX_SENTENCES = int(os.environ.get("MELO_MAX_SENTENCES", "4"))
_SENTENCE_END_RE = re.compile(r"[.!?。！？…\n]+")


def _count_sentences(text: str) -> int:
    """粗略统计句子结束符个数，用于限制 Melo 内部过度拆分。"""
    return len(_SENTENCE_END_RE.findall(text or ""))


def _load_model(language: str, device: str):
    key = (str(language).upper(), str(device or "cpu").lower())
    if key not in _MODELS:
        from melo.api import TTS

        sys.stderr.write(f"[melo-worker] loading {key[0]} on {key[1]}...\n")
        sys.stderr.flush()
        _MODELS[key] = TTS(language=key[0], device=key[1])
        sys.stderr.write(f"[melo-worker] loaded {key[0]} on {key[1]}\n")
        sys.stderr.flush()
    return _MODELS[key]


def _resolve_speaker(model, speaker):
    data = getattr(getattr(model, "hps", None), "data", None)
    spk2id = dict(getattr(data, "spk2id", None) or {})
    if speaker and speaker in spk2id:
        return spk2id[speaker]
    lang = getattr(model, "language", "")
    base_lang = str(lang).split("_")[0]
    if lang in spk2id:
        return spk2id[lang]
    if base_lang in spk2id:
        return spk2id[base_lang]
    if spk2id:
        return next(iter(spk2id.values()))
    return 0


def _handle(req: dict, retried: bool = False) -> dict:
    rid = req.get("id")
    text = str(req.get("text") or "").strip()
    if not text:
        return {"id": rid, "ok": False, "error": "text 不能为空"}
    if len(text) > _MAX_TEXT_LEN:
        return {
            "id": rid,
            "ok": False,
            "error": f"文本长度超过上限 {_MAX_TEXT_LEN} 字符",
        }
    sentence_count = _count_sentences(text)
    if sentence_count > _MAX_SENTENCES:
        return {
            "id": rid,
            "ok": False,
            "error": (
                f"文本包含约 {sentence_count} 个句子，超过上限 {_MAX_SENTENCES}；"
                "请拆成更短的输入再合成"
            ),
        }
    language = str(req.get("language") or "ZH").upper()
    device = str(req.get("device") or "cpu").lower()
    model = _load_model(language, device)
    speaker_id = _resolve_speaker(model, req.get("speaker"))
    output = req.get("output")
    if not output:
        return {"id": rid, "ok": False, "error": "缺少 output 路径"}
    os.makedirs(os.path.dirname(os.path.abspath(output)), exist_ok=True)
    try:
        model.tts_to_file(
            text=req.get("text", ""),
            speaker_id=speaker_id,
            output_path=output,
            sdp_ratio=float(req.get("sdp_ratio", 0.2)),
            noise_scale=float(req.get("noise_scale", 0.6)),
            noise_scale_w=float(req.get("noise_scale_w", 0.8)),
            speed=float(req.get("speed", 1.0)),
            format=req.get("format") or "wav",
            quiet=True,
        )
    except LookupError:
        if retried:
            raise
        # EN 场景首次需要下载 nltk 词性标注器；下载后重试一次。
        try:
            import ssl

            import nltk

            try:
                _create_unverified_https_context = ssl._create_unverified_context
            except AttributeError:
                pass
            else:
                ssl._create_default_https_context = _create_unverified_https_context
            nltk.download("averaged_perceptron_tagger_eng")
        except Exception:  # noqa: BLE001, S110 - 下载失败也重试一次，交给重试路径报错
            pass
        return _handle(req, retried=True)
    return {"id": rid, "ok": True, "path": output}


def main() -> int:
    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except Exception as exc:  # noqa: BLE001 - 单行 JSON 解析失败需返回可读错误
            sys.stdout.write(
                json.dumps({"id": None, "ok": False, "error": f"请求 JSON 解析失败: {exc}"})
                + "\n"
            )
            sys.stdout.flush()
            continue
        try:
            resp = _handle(req)
        except Exception as exc:  # noqa: BLE001 - 单请求失败不能杀死常驻 worker
            traceback.print_exc(file=sys.stderr)
            resp = {"id": req.get("id"), "ok": False, "error": str(exc)}
        sys.stdout.write(json.dumps(resp, ensure_ascii=False) + "\n")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
