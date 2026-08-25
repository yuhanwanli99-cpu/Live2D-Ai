from typing import Any
from .tts_routes_helpers import normalize_tts_id


from loguru import logger
class TTSService:
    """TTS 相关设置接口的业务逻辑：可用性检查、试听、本地 Melo TTS。"""

    def __init__(
        self,
        config_service,
        config_helper,
        tts_defaults: dict[str, Any],
        tts_voice_catalog: dict[str, list[str]],
        tts_model_catalog: dict[str, list[str]],
        tts_non_provider_keys: tuple[str, ...],
    ):
        self.config_service = config_service
        self.config_helper = config_helper
        self.tts_defaults = tts_defaults
        self.tts_voice_catalog = tts_voice_catalog
        self.tts_model_catalog = tts_model_catalog
        self.tts_non_provider_keys = tts_non_provider_keys

    def check_tts_providers(self, payload: dict[str, Any]) -> list[dict[str, Any]]:
        data = self.config_service.load_yaml()
        cc = data.get("character_config", {}) or {}
        tts_cfg = cc.get("tts_config", {}) or {}
        tts_overrides = payload.get("tts_configs") or {}
        api_keys = payload.get("api_keys") or {}

        tts_ids = list(dict.fromkeys(list(tts_cfg.keys()) + list(self.tts_defaults.keys())))
        tts_results = []
        for pid in tts_ids:
            if pid in self.tts_non_provider_keys:
                continue
            cfg = dict(tts_cfg.get(pid, {}) or {})
            override = tts_overrides.get(pid)
            if isinstance(override, dict):
                for k in ("voice", "model", "ws_url"):
                    if override.get(k) not in (None, ""):
                        cfg[k] = override[k]
            key_env = self.tts_defaults.get(pid, {}).get("key_env")
            override_key = override.get("api_key") if isinstance(override, dict) else None
            has_ui_key = bool(override_key) or bool(key_env and api_keys.get(key_env))
            configured = has_ui_key if key_env else True
            tts_results.append(
                {
                    "provider": pid,
                    "kind": "tts",
                    "ok": configured,
                    "status": "已配置" if configured else "未配置 Key",
                    "models": [],
                }
            )
        return tts_results

    async def test_tts(self, payload: dict[str, Any], default_context_cache) -> dict[str, Any]:
        """TTS 试听：支持当前引擎或指定 provider。"""
        import copy

        provider = normalize_tts_id((payload.get("provider") or "").strip())
        data = self.config_service.load_yaml()
        tts_cfg = (data.get("character_config", {}) or {}).get("tts_config", {}) or {}
        preview_text = (
            self.tts_defaults.get("melo_tts", {}).get("preview_text", "你好，我是白～这是一段试听。")
            if provider == "melo_tts"
            else "你好，我是白～这是一段试听。"
        )

        if not provider:
            if default_context_cache is None or default_context_cache.tts_engine is None:
                raise ValueError("当前 TTS 引擎未初始化")
            path = await default_context_cache.tts_engine.async_generate_audio(
                text=preview_text,
                file_name_no_ext="settings_preview",
            )
            if not path:
                raise RuntimeError("TTS 合成失败：引擎未返回音频文件")
            return {"ok": True, "audio_path": path}

        params: dict[str, Any] = {}
        try:
            from open_llm_vtuber.config_manager.main import Config as AppConfig

            candidate = copy.deepcopy(data)
            self.config_helper.apply_persona_to_data(
                candidate, self.config_service.load_persona() or {}
            )
            parsed = AppConfig(**candidate)
            typed_tts = parsed.character_config.tts_config
            typed_provider = getattr(typed_tts, provider, None)
            if typed_provider is not None:
                params = typed_provider.model_dump()
        except Exception as parse_error:  # noqa: BLE001
            logger.warning(f"TTS test config parse failed, use raw fallback: {parse_error}")

        if not params:
            saved = tts_cfg.get(provider, {}) or {}
            if isinstance(saved, dict):
                for k in (
                    "voice", "model", "ws_url", "api_key",
                    "base_url", "api_url", "region", "model_path",
                    "language", "device", "speaker", "speed",
                    "sdp_ratio", "noise_scale", "noise_scale_w", "format",
                ):
                    if saved.get(k) not in (None, ""):
                        params[k] = saved[k]
            if provider == "melo_tts" and not params:
                from open_llm_vtuber.config_manager.tts import MeloTTSConfig
                params = MeloTTSConfig().model_dump()
            elif provider == "edge_tts" and not params.get("voice"):
                params["voice"] = (self.tts_voice_catalog.get("edge_tts") or ["zh-CN-XiaoxiaoNeural"])[0]
            elif provider == "piper_tts" and not params:
                from open_llm_vtuber.config_manager.tts import PiperTTSConfig
                params = PiperTTSConfig().model_dump()
            elif provider == "pyttsx3_tts":
                params = {}

        has_typed_params = bool(params)
        for field in (
            "voice", "model", "ws_url", "base_url", "api_url", "region",
            "model_path", "language", "device", "speaker", "speed",
            "sdp_ratio", "noise_scale", "noise_scale_w", "format",
        ):
            if payload.get(field) not in (None, ""):
                if not has_typed_params or field in params:
                    params[field] = payload[field]
        if provider == "melo_tts":
            if payload.get("voice") not in (None, ""):
                params["language"] = payload["voice"]
            if payload.get("model") not in (None, ""):
                params["device"] = payload["model"]
        if payload.get("api_key") not in (None, ""):
            if not has_typed_params or "api_key" in params:
                params["api_key"] = payload["api_key"]
        elif not params.get("api_key"):
            key_env = self.tts_defaults.get(provider, {}).get("key_env")
            if key_env:
                params["api_key"] = self.config_helper.env_value(key_env)

        if provider == "pyttsx3_tts":
            params = {}

        from open_llm_vtuber.tts.tts_factory import TTSFactory
        engine = TTSFactory.get_tts_engine(provider, **params)
        audio_path = await engine.async_generate_audio(
            text=preview_text,
            file_name_no_ext="settings_preview",
        )
        if not audio_path:
            raise RuntimeError("TTS 合成失败：引擎未返回音频文件")
        return {"ok": True, "audio_path": audio_path}
