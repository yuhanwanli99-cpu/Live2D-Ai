import asyncio
from typing import Any

from loguru import logger

from ..utils.http_utils import extract_upstream_message, requests_get, requests_post
from ..validators.url_trust import is_trusted_base_url
from .llm_routes_helpers import (
    llm_protocol,
    models_endpoint,
    models_from_response,
    normalize_provider_id,
)


class LLMService:
    """LLM 相关设置接口的业务逻辑：发现模型、测试连通性、批量检查。"""

    def __init__(
        self,
        config_service,
        config_helper,
        llm_defaults: dict[str, Any],
        model_catalog: dict[str, list[str]],
        check_concurrency: int = 4,
    ):
        self.config_service = config_service
        self.config_helper = config_helper
        self.llm_defaults = llm_defaults
        self.model_catalog = model_catalog
        self.check_concurrency = check_concurrency

    async def discover_models(self, payload: dict[str, Any]) -> dict[str, Any]:
        provider = normalize_provider_id(payload.get("provider", ""))
        base_url = (payload.get("base_url") or "").strip()
        api_key = (payload.get("api_key") or "").strip()

        data = self.config_service.load_yaml()
        agent_cfg = (
            (data.get("character_config", {}) or {})
            .get("agent_config", {})
            or {}
        )
        cfg = (agent_cfg.get("llm_configs", {}) or {}).get(provider, {}) or {}
        default = self.llm_defaults.get(provider, {})

        if not base_url:
            base_url = cfg.get("base_url", "").strip() or default.get("base_url", "")

        client_provided_key = bool(api_key)
        if not api_key:
            key_env = default.get("key_env") or cfg.get("key_env") or "OPENAI_API_KEY"
            api_key = self.config_helper.env_value(key_env)

        if api_key and not client_provided_key and not is_trusted_base_url(
            base_url, provider, cfg, default
        ):
            raise PermissionError(
                "发现模型时 base_url 与配置不符，请显式提供 api_key"
            )

        if not base_url:
            raise ValueError("Base URL 为空，无法发现模型")

        protocol = llm_protocol(provider)
        url, headers = models_endpoint(protocol, base_url, api_key)
        resp = await requests_get(url, headers=headers, timeout=15)
        if resp.status_code >= 400:
            raise RuntimeError(
                f"HTTP {resp.status_code}: {extract_upstream_message(resp)}"
            )

        models = models_from_response(protocol, resp.json())
        return {"ok": True, "models": models}

    async def test_llm(self, payload: dict[str, Any]) -> dict[str, Any]:
        provider = normalize_provider_id(payload.get("provider", ""))
        key = (payload.get("api_key") or "").strip()
        base_url = (payload.get("base_url") or "").strip()
        model = (payload.get("model") or "").strip()

        data = self.config_service.load_yaml()
        agent_cfg = (
            (data.get("character_config", {}) or {})
            .get("agent_config", {})
            or {}
        )
        cfg = (agent_cfg.get("llm_configs", {}) or {}).get(provider, {}) or {}
        default = self.llm_defaults.get(provider, {})

        client_provided_key = bool(key)
        if not base_url:
            base_url = cfg.get("base_url", "").strip() or default.get("base_url", "")
        if not model:
            model = cfg.get("model", "")

        key_env = default.get("key_env") or cfg.get("key_env") or "OPENAI_API_KEY"
        if not key:
            key = self.config_helper.env_value(key_env) if key_env else ""

        if key and not client_provided_key and not is_trusted_base_url(
            base_url, provider, cfg, default
        ):
            raise PermissionError(
                "测试 LLM 时 base_url 与配置不符，请显式提供 api_key"
            )

        protocol = llm_protocol(provider)
        if protocol != "ollama" and not key:
            raise ValueError(f"{provider} 未配置 API Key")
        if not base_url:
            raise ValueError(f"{provider} 未配置 base_url")
        if not model:
            raise ValueError(f"{provider} 未配置 model")

        base = base_url.rstrip("/")
        max_tokens = payload.get("max_tokens") or 5

        if protocol == "ollama":
            url = base + "/api/chat"
            headers = {"Content-Type": "application/json"}
            body = {
                "model": model,
                "messages": [{"role": "user", "content": "ping"}],
                "stream": False,
            }
        elif protocol == "anthropic":
            url = base + ("/messages" if base.endswith("/v1") else "/v1/messages")
            headers = {
                "x-api-key": key,
                "anthropic-version": "2023-06-01",
                "Content-Type": "application/json",
            }
            body = {
                "model": model,
                "max_tokens": max_tokens,
                "messages": [{"role": "user", "content": "ping"}],
            }
        else:
            url = base + "/chat/completions"
            headers = {
                "Authorization": f"Bearer {key}",
                "Content-Type": "application/json",
            }
            body = {
                "model": model,
                "messages": [{"role": "user", "content": "ping"}],
                "max_tokens": max_tokens,
            }

        resp = await requests_post(url, headers=headers, json_body=body, timeout=10)
        if resp.status_code < 400:
            return {"ok": True, "status": resp.status_code}
        raise RuntimeError(
            f"HTTP {resp.status_code}: {extract_upstream_message(resp)}"
        )

    async def check_llm_providers(self, payload: dict[str, Any]) -> list[dict[str, Any]]:
        data = self.config_service.load_yaml()
        cc = data.get("character_config", {}) or {}
        agent_cfg = cc.get("agent_config", {}) or {}
        llm_configs = agent_cfg.get("llm_configs", {}) or {}
        llm_overrides = payload.get("llm_configs") or {}
        api_keys = payload.get("api_keys") or {}

        provider_ids = list(
            dict.fromkeys(list(llm_configs.keys()) + list(self.llm_defaults.keys()))
        )

        semaphore = asyncio.Semaphore(self.check_concurrency)

        async def probe(pid: str) -> dict[str, Any]:
            cfg = dict(llm_configs.get(pid, {}) or {})
            override = llm_overrides.get(pid)
            if isinstance(override, dict):
                for k in ("base_url", "model"):
                    if override.get(k) not in (None, ""):
                        cfg[k] = override[k]
            default = self.llm_defaults.get(pid, {})
            base_url = cfg.get("base_url", "").strip() or default.get("base_url", "")
            key_env = default.get("key_env") or cfg.get("key_env") or "OPENAI_API_KEY"
            override_key = override.get("api_key") if isinstance(override, dict) else None
            key = override_key or api_keys.get(key_env) or self.config_helper.env_value(key_env)

            if not key:
                return {
                    "provider": pid,
                    "kind": "llm",
                    "ok": False,
                    "status": "未配置 Key",
                    "models": [],
                }
            if not base_url:
                return {
                    "provider": pid,
                    "kind": "llm",
                    "ok": False,
                    "status": "未配置 Base URL",
                    "models": [],
                }

            try:
                protocol = llm_protocol(pid)
                url, headers = models_endpoint(protocol, base_url, key)
                async with semaphore:
                    resp = await requests_get(url, headers=headers, timeout=8)
                if resp.status_code < 400:
                    return {
                        "provider": pid,
                        "kind": "llm",
                        "ok": True,
                        "status": "ok",
                        "models": models_from_response(protocol, resp.json()),
                    }
                return {
                    "provider": pid,
                    "kind": "llm",
                    "ok": False,
                    "status": f"HTTP {resp.status_code}: {extract_upstream_message(resp)}",
                    "models": [],
                }
            except Exception as e:  # noqa: BLE001 - 单个 provider 失败不影响其余
                return {
                    "provider": pid,
                    "kind": "llm",
                    "ok": False,
                    "status": str(e),
                    "models": [],
                }

        return list(await asyncio.gather(*(probe(pid) for pid in provider_ids)))
