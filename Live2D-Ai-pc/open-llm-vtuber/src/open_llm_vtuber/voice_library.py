# -*- coding: utf-8 -*-
"""PC 端声音克隆资产管理。

管理用户上传的参考音频，并可在「当前 TTS 支持克隆参考音频」时，
把参考音频路径应用到 conf.yaml 的对应引擎字段。
"""

from __future__ import annotations

import json
import os
import tempfile
import time
import uuid
from pathlib import Path
from typing import Any

import yaml
from fastapi import APIRouter, File, Form, Request, UploadFile
from loguru import logger
from starlette.responses import JSONResponse

_ALLOWED_EXTS = {".wav", ".mp3", ".m4a", ".flac", ".ogg"}
_MAX_AUDIO_BYTES = 50 * 1024 * 1024

# 当前 TTS 引擎 -> 参考音频字段名
_CLONE_FIELD_BY_TTS = {
    "gpt_sovits_tts": "ref_audio_path",
    "spark_tts": "prompt_wav_upload",
    "x_tts": "speaker_wav",
    "coqui_tts": "speaker_wav",
}


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[4]


def _default_pc_dir() -> Path:
    return _repo_root() / "Live2D-Ai-pc" / "open-llm-vtuber"


def _atomic_write_text(path: Path, content: str) -> None:
    directory = path.parent
    directory.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix="." + path.name + ".", suffix=".tmp", dir=directory)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            f.write(content)
            f.flush()
            os.fsync(f.fileno())
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def _atomic_write_json(path: Path, data: Any) -> None:
    _atomic_write_text(path, json.dumps(data, ensure_ascii=False, indent=2) + "\n")


def _load_json(path: Path, default: Any) -> Any:
    if not path.exists():
        return default
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:  # noqa: BLE001 - 状态损坏不该阻断
        logger.warning(f"voice library state 读取失败，按默认值处理: {e}")
        return default


def init_voice_library_routes(
    voice_dir: str | None = None,
    state_path: str | None = None,
    conf_path: str | None = None,
    plugin_manager=None,
) -> APIRouter:
    router = APIRouter()
    pc_dir = _default_pc_dir()
    voice_root = Path(voice_dir or (pc_dir / "voice_clone")).resolve()
    state_file = Path(state_path or (pc_dir / "voice_library.json")).resolve()
    conf_file = Path(conf_path or (pc_dir / "conf.yaml")).resolve()

    def _plugin_reason() -> str | None:
        if plugin_manager is None or plugin_manager.is_enabled("voice-assets"):
            return None
        return "voice-assets 插件未启用，声音克隆管理不可用"

    def _state() -> dict[str, Any]:
        state = _load_json(state_file, {"voices": [], "active_voice_id": None})
        if not isinstance(state, dict):
            state = {"voices": [], "active_voice_id": None}
        state.setdefault("voices", [])
        if not isinstance(state["voices"], list):
            state["voices"] = []
        return state

    def _save_state(state: dict[str, Any]) -> None:
        voice_root.mkdir(parents=True, exist_ok=True)
        _atomic_write_json(state_file, state)

    def _voice_public(v: dict[str, Any]) -> dict[str, Any]:
        return {
            "id": v.get("id", ""),
            "name": v.get("name", ""),
            "file": v.get("file", ""),
            "ext": v.get("ext", ""),
            "bytes": int(v.get("bytes", 0)),
            "created_at": float(v.get("created_at", 0)),
            "ref_audio_path": v.get("ref_audio_path", ""),
            "active": bool(v.get("active", False)),
        }

    @router.get("/api/voice/list")
    async def list_voices():
        state = _state()
        voices = []
        active_id = state.get("active_voice_id")
        for v in state["voices"]:
            pub = _voice_public(v)
            pub["active"] = v.get("id") == active_id
            voices.append(pub)
        return {"ok": True, "voices": voices}

    @router.post("/api/voice/upload")
    async def upload_voice(
        file: UploadFile = File(...),
        voice_name: str | None = Form(None),
    ):
        reason = _plugin_reason()
        if reason:
            return JSONResponse({"ok": False, "error": reason}, status_code=403)
        if not file.filename:
            return JSONResponse({"ok": False, "error": "缺少文件名"}, status_code=400)
        ext = Path(file.filename).suffix.lower()
        if ext not in _ALLOWED_EXTS:
            return JSONResponse(
                {
                    "ok": False,
                    "error": f"不支持的音频格式 '{ext}'；支持 {', '.join(sorted(_ALLOWED_EXTS))}",
                },
                status_code=400,
            )
        raw = await file.read()
        if len(raw) == 0:
            return JSONResponse({"ok": False, "error": "音频文件为空"}, status_code=400)
        if len(raw) > _MAX_AUDIO_BYTES:
            return JSONResponse(
                {"ok": False, "error": "音频文件超过 50MB 上限"}, status_code=413
            )

        voice_id = uuid.uuid4().hex[:12]
        filename = f"{voice_id}{ext}"
        voice_root.mkdir(parents=True, exist_ok=True)
        target = voice_root / filename
        if target.exists():
            return JSONResponse(
                {"ok": False, "error": "文件名冲突，请重试"}, status_code=500
            )
        try:
            fd, tmp = tempfile.mkstemp(prefix=".voice-", suffix=ext, dir=voice_root)
            try:
                with os.fdopen(fd, "wb") as f:
                    f.write(raw)
                    f.flush()
                    os.fsync(f.fileno())
                os.replace(tmp, target)
            finally:
                if os.path.exists(tmp):
                    os.unlink(tmp)
        except Exception as e:  # noqa: BLE001
            logger.error(f"voice upload write failed: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=500)

        state = _state()
        now = time.time()
        voice = {
            "id": voice_id,
            "name": (voice_name or "").strip() or f"Voice {voice_id}",
            "file": filename,
            "ext": ext,
            "bytes": len(raw),
            "created_at": now,
            "ref_audio_path": f"voice_clone/{filename}",
        }
        state["voices"].append(voice)
        _save_state(state)
        return {"ok": True, "voice": _voice_public(voice)}

    @router.post("/api/voice/select")
    async def select_voice(request: Request):
        reason = _plugin_reason()
        if reason:
            return JSONResponse({"ok": False, "error": reason}, status_code=403)
        payload = await request.json() if await request.body() else {}
        if not isinstance(payload, dict):
            payload = {}
        voice_id = payload.get("voice_id")
        state = _state()
        if not any(v.get("id") == voice_id for v in state["voices"]):
            return JSONResponse({"ok": False, "error": "voice_id 不存在"}, status_code=404)
        state["active_voice_id"] = voice_id
        _save_state(state)
        matched = next(v for v in state["voices"] if v.get("id") == voice_id)
        pub = _voice_public(matched)
        pub["active"] = True
        return {"ok": True, "voice": pub}

    @router.post("/api/voice/apply")
    async def apply_voice(request: Request):
        reason = _plugin_reason()
        if reason:
            return JSONResponse({"ok": False, "error": reason}, status_code=403)
        payload = await request.json() if await request.body() else {}
        if not isinstance(payload, dict):
            payload = {}
        voice_id = payload.get("voice_id")
        state = _state()
        matched = next((v for v in state["voices"] if v.get("id") == voice_id), None)
        if matched is None:
            return JSONResponse({"ok": False, "error": "voice_id 不存在"}, status_code=404)

        try:
            conf = yaml.safe_load(conf_file.read_text(encoding="utf-8")) or {}
        except Exception as e:  # noqa: BLE001
            return JSONResponse(
                {"ok": False, "error": f"读取 conf.yaml 失败: {e}"}, status_code=500
            )
        if not isinstance(conf, dict):
            return JSONResponse(
                {"ok": False, "error": "conf.yaml 顶层必须是对象"}, status_code=500
            )

        tts_cfg = conf.setdefault("character_config", {}).setdefault("tts_config", {})
        if not isinstance(tts_cfg, dict):
            return JSONResponse(
                {"ok": False, "error": "character_config.tts_config 不是对象"},
                status_code=500,
            )
        tts_model = tts_cfg.get("tts_model")
        field = _CLONE_FIELD_BY_TTS.get(tts_model)
        if not field:
            supported = ", ".join(sorted(_CLONE_FIELD_BY_TTS))
            return JSONResponse(
                {
                    "ok": False,
                    "error": f"当前 TTS '{tts_model}' 不支持克隆参考音频；请先切换到: {supported}",
                },
                status_code=400,
            )

        provider_cfg = tts_cfg.setdefault(tts_model, {})
        if not isinstance(provider_cfg, dict):
            return JSONResponse(
                {"ok": False, "error": f"tts_config.{tts_model} 不是对象"}, status_code=500
            )
        provider_cfg[field] = matched["ref_audio_path"]

        try:
            new_yaml = yaml.safe_dump(
                conf, allow_unicode=True, sort_keys=False, default_flow_style=False
            )
            _atomic_write_text(conf_file, new_yaml)
        except Exception as e:  # noqa: BLE001
            return JSONResponse(
                {"ok": False, "error": f"写入 conf.yaml 失败: {e}"}, status_code=500
            )

        state["active_voice_id"] = voice_id
        _save_state(state)
        return {
            "ok": True,
            "applied": True,
            "requires_restart": True,
            "message": "已应用到当前 TTS 引擎，重启后端后生效",
        }

    @router.post("/api/voice/delete")
    async def delete_voice(request: Request):
        reason = _plugin_reason()
        if reason:
            return JSONResponse({"ok": False, "error": reason}, status_code=403)
        payload = await request.json() if await request.body() else {}
        if not isinstance(payload, dict):
            payload = {}
        voice_id = payload.get("voice_id")
        state = _state()
        matched = next((v for v in state["voices"] if v.get("id") == voice_id), None)
        if matched is None:
            return JSONResponse({"ok": False, "error": "voice_id 不存在"}, status_code=404)

        file_path = voice_root / matched.get("file", "")
        if file_path.exists():
            try:
                file_path.unlink()
            except Exception as e:  # noqa: BLE001
                logger.warning(f"删除音频文件失败（忽略）: {e}")
        state["voices"] = [v for v in state["voices"] if v.get("id") != voice_id]
        if state.get("active_voice_id") == voice_id:
            state["active_voice_id"] = None
        _save_state(state)
        return {"ok": True}

    return router
