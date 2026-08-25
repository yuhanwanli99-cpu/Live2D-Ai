import os
from typing import Any


class ConfigRoutesHelper:
    """conf.yaml / .env / persona 相关的配置辅助逻辑。"""

    def __init__(
        self,
        config_abs: str,
        env_path: str,
        config_service,
        llm_defaults: dict[str, Any],
        model_catalog: dict[str, list[str]],
        tts_defaults: dict[str, Any],
        tts_voice_catalog: dict[str, list[str]],
        tts_model_catalog: dict[str, list[str]],
        tts_non_provider_keys: tuple[str, ...],
    ):
        self.config_abs = config_abs
        self.env_path = env_path
        self.config_service = config_service
        self.llm_defaults = llm_defaults
        self.model_catalog = model_catalog
        self.tts_defaults = tts_defaults
        self.tts_voice_catalog = tts_voice_catalog
        self.tts_model_catalog = tts_model_catalog
        self.tts_non_provider_keys = tts_non_provider_keys

    def apply_persona_to_data(self, data: dict[str, Any], persona: dict[str, Any]) -> dict[str, Any]:
        """把 shared/persona.yaml 的提示词应用到待解析配置。"""
        cc = data.setdefault("character_config", {})
        cc["persona_prompt"] = persona.get("system_prompt") or ""
        return data

    def config_dict_for_reload(self) -> dict[str, Any]:
        """构造用于热重载的完整 Config dict。"""
        from open_llm_vtuber.config_manager.utils import read_yaml

        data = read_yaml(self.config_abs)
        return self.apply_persona_to_data(data, self.config_service.load_persona() or {})

    def env_value(self, name: str | None) -> str:
        if not name:
            return ""
        if os.path.exists(self.env_path):
            with open(self.env_path, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line.startswith("#") or "=" not in line:
                        continue
                    k, _, v = line.partition("=")
                    if k.strip() == name:
                        return v.strip()
        return os.getenv(name, "")

    def load_env_map(self) -> dict[str, str]:
        """一次性把 .env 读成 dict（审计 E-2）。"""
        env_map: dict[str, str] = {}
        if os.path.exists(self.env_path):
            with open(self.env_path, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line.startswith("#") or "=" not in line:
                        continue
                    k, _, v = line.partition("=")
                    env_map[k.strip()] = v.strip()
        return env_map

    def get_common(self, data: dict[str, Any]) -> dict[str, Any]:
        cc = data.get("character_config", {}) or {}
        agent_cfg = cc.get("agent_config", {}) or {}
        agent = agent_cfg.get("agent_settings", {}) or {}
        basic = agent.get("basic_memory_agent", {}) or {}
        llm_configs = agent_cfg.get("llm_configs", {}) or {}
        asr_cfg = cc.get("asr_config", {}) or {}
        tts_cfg = cc.get("tts_config", {}) or {}
        sys_cfg = data.get("system_config", {}) or {}
        persona = self.config_service.load_persona()
        env_map = self.load_env_map()

        def env_has(name: str | None) -> bool:
            if not name:
                return False
            return bool(env_map.get(name) or os.getenv(name, ""))

        provider_ids = list(dict.fromkeys(list(llm_configs.keys()) + list(self.llm_defaults.keys())))
        llm_provider_list = []
        for pid in provider_ids:
            cfg = llm_configs.get(pid, {}) or {}
            default = self.llm_defaults.get(pid, {})
            key_env = default.get("key_env")
            known_models = cfg.get("models") or self.model_catalog.get(pid, [])
            current_model = cfg.get("model", "")
            if not current_model and known_models:
                current_model = known_models[0]
            llm_provider_list.append(
                {
                    "id": pid,
                    "name": default.get("name", pid),
                    "model": current_model,
                    "models": known_models,
                    "base_url": cfg.get("base_url", default.get("base_url", "")),
                    "temperature": cfg.get("temperature", 0.8),
                    "max_tokens": cfg.get("max_tokens", None),
                    "top_p": cfg.get("top_p", None),
                    "thinking_enabled": bool(cfg.get("thinking_enabled", False)),
                    "thinking_effort": cfg.get("thinking_effort", None),
                    "key_env": key_env,
                    "has_key": env_has(key_env) if key_env else False,
                    "configured": pid in llm_configs,
                }
            )

        tts_ids = list(dict.fromkeys(list(tts_cfg.keys()) + list(self.tts_defaults.keys())))
        tts_provider_list = []
        for pid in tts_ids:
            if pid in self.tts_non_provider_keys:
                continue
            cfg = tts_cfg.get(pid, {}) or {}
            if not isinstance(cfg, dict):
                cfg = {}
            default = self.tts_defaults.get(pid, {})
            key_env = default.get("key_env")
            known_voices = cfg.get("voices") or self.tts_voice_catalog.get(pid, [])
            known_tts_models = cfg.get("models") or self.tts_model_catalog.get(pid, [])
            current_voice = cfg.get("voice", "")
            current_tts_model = cfg.get("model", "")
            if pid == "melo_tts":
                current_voice = current_voice or cfg.get("language", "")
                current_tts_model = current_tts_model or cfg.get("device", "")
            if not current_voice and known_voices:
                current_voice = known_voices[0]
            if not current_tts_model and known_tts_models:
                current_tts_model = known_tts_models[0]
            entry: dict[str, Any] = {
                "id": pid,
                "name": default.get("name", pid),
                "voice": current_voice,
                "model": current_tts_model,
                "ws_url": cfg.get("ws_url", ""),
                "voices": known_voices,
                "models": known_tts_models,
                "key_env": key_env,
                "has_key": env_has(key_env) if key_env else True,
                "configured": pid in tts_cfg,
            }
            if pid == "melo_tts":
                entry.update(
                    {
                        "language": current_voice,
                        "device": current_tts_model,
                        "speaker": cfg.get("speaker", "") or "",
                        "speed": cfg.get("speed", 1.0),
                        "sdp_ratio": cfg.get("sdp_ratio", 0.2),
                        "noise_scale": cfg.get("noise_scale", 0.6),
                        "noise_scale_w": cfg.get("noise_scale_w", 0.8),
                        "format": cfg.get("format", "wav"),
                    }
                )
            tts_provider_list.append(entry)

        return {
            "llm_provider": basic.get("llm_provider", ""),
            "llm_providers": list(dict.fromkeys(list(llm_configs.keys()) + list(self.llm_defaults.keys()))),
            "llm_configs": llm_provider_list,
            "tts_model": tts_cfg.get("tts_model", ""),
            "viseme_mode": tts_cfg.get("viseme_mode", "rms"),
            "tts_providers": list(
                dict.fromkeys(
                    [
                        k
                        for k, v in tts_cfg.items()
                        if k not in self.tts_non_provider_keys
                        and isinstance(v, dict)
                    ]
                    + list(self.tts_defaults.keys())
                )
            ),
            "tts_configs": tts_provider_list,
            "asr_model": asr_cfg.get("asr_model", "disabled"),
            "asr_providers": [
                k
                for k, v in asr_cfg.items()
                if k != "asr_model" and isinstance(v, dict)
            ],
            "character_name": cc.get("character_name", ""),
            "port": sys_cfg.get("port", 12393),
            "live2d_fps": sys_cfg.get("live2d_fps", 120),
            "log_level": self.env_value("LIVE2DAI_LOG_LEVEL") or "INFO",
            "developer_mode": bool(sys_cfg.get("developer_mode", False)),
            "persona": {
                "system_prompt": persona.get("system_prompt", ""),
            },
        }
