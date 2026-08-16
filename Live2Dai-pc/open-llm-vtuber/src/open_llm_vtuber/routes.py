import os
import json
from uuid import uuid4
import numpy as np
from datetime import datetime
from fastapi import APIRouter, WebSocket, UploadFile, File, Response, Request
from starlette.responses import JSONResponse
from starlette.websockets import WebSocketDisconnect
from loguru import logger
from ruamel.yaml import YAML
from .service_context import ServiceContext
from .websocket_handler import WebSocketHandler
from .proxy_handler import ProxyHandler


def init_config_routes(config_path: str = "conf.yaml") -> APIRouter:
    """轻量设置 Dashboard API：读取/写入 conf.yaml 与仓库根 .env。

    仅暴露常用可调项；保存后需要重启后端生效。
    """
    router = APIRouter()
    config_abs = os.path.abspath(config_path)
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(config_abs)))
    env_path = os.path.join(repo_root, ".env")

    def _load_yaml():
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        with open(config_abs, "r", encoding="utf-8") as f:
            return yaml.load(f) or {}

    def _save_yaml(data):
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        yaml.allow_unicode = True
        with open(config_abs, "w", encoding="utf-8") as f:
            yaml.dump(data, f)

    def _get_common(data: dict) -> dict:
        cc = data.get("character_config", {}) or {}
        agent = (
            cc.get("agent_config", {}) or {}
        ).get("agent_settings", {}) or {}
        basic = agent.get("basic_memory_agent", {}) or {}
        llm_configs = (
            (cc.get("agent_config", {}) or {}).get("llm_configs", {}) or {}
        )
        asr_cfg = cc.get("asr_config", {}) or {}
        tts_cfg = cc.get("tts_config", {}) or {}
        sys_cfg = data.get("system_config", {}) or {}
        tts_providers = [
            k
            for k, v in tts_cfg.items()
            if k not in ("tts_model", "concurrency_limit")
            and isinstance(v, dict)
        ]
        asr_providers = [
            k
            for k, v in asr_cfg.items()
            if k != "asr_model" and isinstance(v, dict)
        ]
        return {
            "llm_provider": basic.get("llm_provider", ""),
            "llm_providers": list(llm_configs.keys()),
            "tts_model": tts_cfg.get("tts_model", ""),
            "tts_providers": tts_providers,
            "asr_model": asr_cfg.get("asr_model", "disabled"),
            "asr_providers": asr_providers,
            "character_name": cc.get("character_name", ""),
            "port": sys_cfg.get("port", 12393),
            "live2d_fps": sys_cfg.get("live2d_fps", 120),
        }

    @router.get("/api/config")
    async def get_config():
        try:
            data = _load_yaml()
            return JSONResponse(_get_common(data))
        except Exception as e:
            logger.error(f"Failed to read config: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    @router.post("/api/config")
    async def update_config(request: Request):
        try:
            payload = await request.json()
            data = _load_yaml()

            cc = data.setdefault("character_config", {})
            agent_cfg = cc.setdefault("agent_config", {})
            settings = agent_cfg.setdefault("agent_settings", {})
            basic = settings.setdefault("basic_memory_agent", {})

            if "llm_provider" in payload and payload["llm_provider"]:
                basic["llm_provider"] = payload["llm_provider"]
            if "tts_model" in payload and payload["tts_model"]:
                cc.setdefault("tts_config", {})["tts_model"] = payload["tts_model"]
            if "asr_model" in payload and payload["asr_model"]:
                cc.setdefault("asr_config", {})["asr_model"] = payload["asr_model"]
            if "character_name" in payload and payload["character_name"]:
                cc["character_name"] = payload["character_name"]
            if "port" in payload and payload["port"]:
                data.setdefault("system_config", {})["port"] = int(payload["port"])
            if "live2d_fps" in payload and payload["live2d_fps"]:
                data.setdefault("system_config", {})["live2d_fps"] = int(
                    payload["live2d_fps"]
                )

            _save_yaml(data)

            # 写入仓库根 .env（Key 不落 conf.yaml，避免误提交）
            api_keys = payload.get("api_keys") or {}
            if api_keys:
                lines = []
                if os.path.exists(env_path):
                    with open(env_path, "r", encoding="utf-8") as f:
                        lines = f.read().splitlines()
                else:
                    os.makedirs(os.path.dirname(env_path), exist_ok=True)
                existing = {}
                for line in lines:
                    if "=" in line and not line.strip().startswith("#"):
                        k, _, v = line.partition("=")
                        existing[k.strip()] = v.strip()
                changed = False
                for k, v in api_keys.items():
                    if v and str(v).strip():
                        existing[k.strip()] = str(v).strip()
                        changed = True
                if changed:
                    new_lines = []
                    written = set()
                    for line in lines:
                        if "=" in line and not line.strip().startswith("#"):
                            k, _, _ = line.partition("=")
                            k = k.strip()
                            if k in existing:
                                new_lines.append(f"{k}={existing[k]}")
                                written.add(k)
                                continue
                        new_lines.append(line)
                    for k, v in existing.items():
                        if k not in written:
                            new_lines.append(f"{k}={v}")
                    with open(env_path, "w", encoding="utf-8") as f:
                        f.write("\n".join(new_lines) + "\n")

            return JSONResponse({"ok": True, "requires_restart": True})
        except Exception as e:
            logger.error(f"Failed to update config: {e}")
            return JSONResponse({"error": str(e)}, status_code=500)

    return router


def init_client_ws_route(default_context_cache: ServiceContext) -> APIRouter:
    """
    Create and return API routes for handling the `/client-ws` WebSocket connections.

    Args:
        default_context_cache: Default service context cache for new sessions.

    Returns:
        APIRouter: Configured router with WebSocket endpoint.
    """

    router = APIRouter()
    ws_handler = WebSocketHandler(default_context_cache)

    @router.websocket("/client-ws")
    async def websocket_endpoint(websocket: WebSocket):
        """WebSocket endpoint for client connections"""
        await websocket.accept()
        client_uid = str(uuid4())

        try:
            await ws_handler.handle_new_connection(websocket, client_uid)
            await ws_handler.handle_websocket_communication(websocket, client_uid)
        except WebSocketDisconnect:
            await ws_handler.handle_disconnect(client_uid)
        except Exception as e:
            logger.error(f"Error in WebSocket connection: {e}")
            await ws_handler.handle_disconnect(client_uid)
            raise

    return router


def init_proxy_route(server_url: str) -> APIRouter:
    """
    Create and return API routes for handling proxy connections.

    Args:
        server_url: The WebSocket URL of the actual server

    Returns:
        APIRouter: Configured router with proxy WebSocket endpoint
    """
    router = APIRouter()
    proxy_handler = ProxyHandler(server_url)

    @router.websocket("/proxy-ws")
    async def proxy_endpoint(websocket: WebSocket):
        """WebSocket endpoint for proxy connections"""
        try:
            await proxy_handler.handle_client_connection(websocket)
        except Exception as e:
            logger.error(f"Error in proxy connection: {e}")
            raise

    return router


def init_webtool_routes(default_context_cache: ServiceContext) -> APIRouter:
    """
    Create and return API routes for handling web tool interactions.

    Args:
        default_context_cache: Default service context cache for new sessions.

    Returns:
        APIRouter: Configured router with WebSocket endpoint.
    """

    router = APIRouter()

    @router.get("/web-tool")
    async def web_tool_redirect():
        """Redirect /web-tool to /web_tool/index.html"""
        return Response(status_code=302, headers={"Location": "/web-tool/index.html"})

    @router.get("/web_tool")
    async def web_tool_redirect_alt():
        """Redirect /web_tool to /web_tool/index.html"""
        return Response(status_code=302, headers={"Location": "/web-tool/index.html"})

    @router.get("/live2d-models/info")
    async def get_live2d_folder_info():
        """Get information about available Live2D models"""
        live2d_dir = "live2d-models"
        if not os.path.exists(live2d_dir):
            return JSONResponse(
                {"error": "Live2D models directory not found"}, status_code=404
            )

        valid_characters = []
        supported_extensions = [".png", ".jpg", ".jpeg"]

        for entry in os.scandir(live2d_dir):
            if entry.is_dir():
                folder_name = entry.name.replace("\\", "/")
                model3_file = os.path.join(
                    live2d_dir, folder_name, f"{folder_name}.model3.json"
                ).replace("\\", "/")

                if os.path.isfile(model3_file):
                    # Find avatar file if it exists
                    avatar_file = None
                    for ext in supported_extensions:
                        avatar_path = os.path.join(
                            live2d_dir, folder_name, f"{folder_name}{ext}"
                        )
                        if os.path.isfile(avatar_path):
                            avatar_file = avatar_path.replace("\\", "/")
                            break

                    valid_characters.append(
                        {
                            "name": folder_name,
                            "avatar": avatar_file,
                            "model_path": model3_file,
                        }
                    )
        return JSONResponse(
            {
                "type": "live2d-models/info",
                "count": len(valid_characters),
                "characters": valid_characters,
            }
        )

    @router.post("/asr")
    async def transcribe_audio(file: UploadFile = File(...)):
        """
        Endpoint for transcribing audio using the ASR engine
        """
        logger.info(f"Received audio file for transcription: {file.filename}")

        try:
            contents = await file.read()

            # Validate minimum file size
            if len(contents) < 44:  # Minimum WAV header size
                raise ValueError("Invalid WAV file: File too small")

            # Decode the WAV header and get actual audio data
            wav_header_size = 44  # Standard WAV header size
            audio_data = contents[wav_header_size:]

            # Validate audio data size
            if len(audio_data) % 2 != 0:
                raise ValueError("Invalid audio data: Buffer size must be even")

            # Convert to 16-bit PCM samples to float32
            try:
                audio_array = (
                    np.frombuffer(audio_data, dtype=np.int16).astype(np.float32)
                    / 32768.0
                )
            except ValueError as e:
                raise ValueError(
                    f"Audio format error: {str(e)}. Please ensure the file is 16-bit PCM WAV format."
                )

            # Validate audio data
            if len(audio_array) == 0:
                raise ValueError("Empty audio data")

            text = await default_context_cache.asr_engine.async_transcribe_np(
                audio_array
            )
            logger.info(f"Transcription result: {text}")
            return {"text": text}

        except ValueError as e:
            logger.error(f"Audio format error: {e}")
            return Response(
                content=json.dumps({"error": str(e)}),
                status_code=400,
                media_type="application/json",
            )
        except Exception as e:
            logger.error(f"Error during transcription: {e}")
            return Response(
                content=json.dumps(
                    {"error": "Internal server error during transcription"}
                ),
                status_code=500,
                media_type="application/json",
            )

    @router.websocket("/tts-ws")
    async def tts_endpoint(websocket: WebSocket):
        """WebSocket endpoint for TTS generation"""
        await websocket.accept()
        logger.info("TTS WebSocket connection established")

        try:
            while True:
                data = await websocket.receive_json()
                text = data.get("text")
                if not text:
                    continue

                logger.info(f"Received text for TTS: {text}")

                # Split text into sentences
                sentences = [s.strip() for s in text.split(".") if s.strip()]

                try:
                    # Generate and send audio for each sentence
                    for sentence in sentences:
                        sentence = sentence + "."  # Add back the period
                        file_name = f"{datetime.now().strftime('%Y%m%d_%H%M%S')}_{str(uuid4())[:8]}"
                        audio_path = (
                            await default_context_cache.tts_engine.async_generate_audio(
                                text=sentence, file_name_no_ext=file_name
                            )
                        )
                        logger.info(
                            f"Generated audio for sentence: {sentence} at: {audio_path}"
                        )

                        await websocket.send_json(
                            {
                                "status": "partial",
                                "audioPath": audio_path,
                                "text": sentence,
                            }
                        )

                    # Send completion signal
                    await websocket.send_json({"status": "complete"})

                except Exception as e:
                    logger.error(f"Error generating TTS: {e}")
                    await websocket.send_json({"status": "error", "message": str(e)})

        except WebSocketDisconnect:
            logger.info("TTS WebSocket client disconnected")
        except Exception as e:
            logger.error(f"Error in TTS WebSocket connection: {e}")
            await websocket.close()

    return router
