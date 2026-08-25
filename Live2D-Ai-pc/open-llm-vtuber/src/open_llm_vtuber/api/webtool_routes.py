"""Legacy web-tool, model discovery, ASR, and TTS WebSocket routes."""

import json
import os
from datetime import datetime
from uuid import uuid4

import numpy as np
from fastapi import APIRouter, File, Response, UploadFile, WebSocket
from fastapi.responses import JSONResponse
from loguru import logger
from starlette.websockets import WebSocketDisconnect

from ..service_context import ServiceContext


def init_webtool_routes(default_context_cache: ServiceContext) -> APIRouter:
    """Create web-tool compatibility and standalone audio routes."""
    router = APIRouter()

    @router.get("/web-tool")
    async def web_tool_redirect():
        return Response(status_code=302, headers={"Location": "/web-tool/index.html"})

    @router.get("/web_tool")
    async def web_tool_redirect_alt():
        return Response(status_code=302, headers={"Location": "/web-tool/index.html"})

    @router.get("/live2d-models/info")
    async def get_live2d_folder_info():
        live2d_dir = "live2d-models"
        if not os.path.exists(live2d_dir):
            return JSONResponse({"error": "Live2D models directory not found"}, status_code=404)
        characters = []
        for entry in os.scandir(live2d_dir):
            if not entry.is_dir():
                continue
            folder_name = entry.name.replace("\\", "/")
            model_path = os.path.join(live2d_dir, folder_name, f"{folder_name}.model3.json").replace("\\", "/")
            if not os.path.isfile(model_path):
                continue
            avatar = next((os.path.join(live2d_dir, folder_name, f"{folder_name}{ext}").replace("\\", "/") for ext in (".png", ".jpg", ".jpeg") if os.path.isfile(os.path.join(live2d_dir, folder_name, f"{folder_name}{ext}"))), None)
            characters.append({"name": folder_name, "avatar": avatar, "model_path": model_path})
        return JSONResponse({"type": "live2d-models/info", "count": len(characters), "characters": characters})

    @router.post("/asr")
    async def transcribe_audio(file: UploadFile = File(...)):
        logger.info(f"Received audio file for transcription: {file.filename}")
        try:
            contents = await file.read()
            if len(contents) < 44:
                raise ValueError("Invalid WAV file: File too small")
            audio_data = contents[44:]
            if len(audio_data) % 2 != 0:
                raise ValueError("Invalid audio data: Buffer size must be even")
            try:
                audio_array = np.frombuffer(audio_data, dtype=np.int16).astype(np.float32) / 32768.0
            except ValueError as exc:
                raise ValueError(f"Audio format error: {exc!s}. Please ensure the file is 16-bit PCM WAV format.") from exc
            if len(audio_array) == 0:
                raise ValueError("Empty audio data")
            text = await default_context_cache.asr_engine.async_transcribe_np(audio_array)
            logger.info(f"Transcription result: {text}")
            return {"text": text}
        except ValueError as exc:
            logger.error(f"Audio format error: {exc}")
            return Response(content=json.dumps({"error": str(exc)}), status_code=400, media_type="application/json")
        except Exception as exc:  # noqa: BLE001 - API boundary maps engine failures
            logger.error(f"Error during transcription: {exc}")
            return Response(content=json.dumps({"error": "Internal server error during transcription"}), status_code=500, media_type="application/json")

    @router.websocket("/tts-ws")
    async def tts_endpoint(websocket: WebSocket):
        await websocket.accept()
        logger.info("TTS WebSocket connection established")
        try:
            while True:
                data = await websocket.receive_json()
                text = data.get("text")
                if not text:
                    continue
                try:
                    for sentence_text in (s.strip() for s in text.split(".")):
                        if not sentence_text:
                            continue
                        sentence = sentence_text + "."
                        file_name = f"{datetime.now().strftime('%Y%m%d_%H%M%S')}_{str(uuid4())[:8]}"
                        audio_path = await default_context_cache.tts_engine.async_generate_audio(text=sentence, file_name_no_ext=file_name)
                        await websocket.send_json({"status": "partial", "audioPath": audio_path, "text": sentence})
                    await websocket.send_json({"status": "complete"})
                except Exception as exc:  # noqa: BLE001 - report per-request engine failure
                    logger.error(f"Error generating TTS: {exc}")
                    await websocket.send_json({"status": "error", "message": str(exc)})
        except WebSocketDisconnect:
            logger.info("TTS WebSocket client disconnected")
        except Exception as exc:  # noqa: BLE001 - WebSocket boundary cleanup
            logger.error(f"Error in TTS WebSocket connection: {exc}")
            await websocket.close()

    return router
