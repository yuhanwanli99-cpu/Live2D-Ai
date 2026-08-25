from typing import Any


def normalize_provider_id(pid: str) -> str:
    """把 UI 常用别名归一为配置里的 provider id。"""
    return "openai_compatible_llm" if pid == "openai_compatible" else pid


def llm_protocol(provider: str) -> str:
    """按 provider 返回协议族：openai / anthropic / ollama。

    修复 B9：test/discover/check 不能再对所有 provider 一律 OpenAI 形态。
    - anthropic / claude_llm -> Anthropic /v1/messages
    - ollama / ollama_llm    -> Ollama /api/chat (+ /api/tags 列模型)
    - 其余 OpenLLM 各厂家均为 OpenAI 兼容形态（/models + /chat/completions）
    """
    provider = normalize_provider_id(provider or "")
    if provider in ("anthropic", "claude_llm"):
        return "anthropic"
    if provider in ("ollama", "ollama_llm"):
        return "ollama"
    return "openai"


def models_endpoint(protocol: str, base_url: str, api_key: str) -> tuple[str, dict[str, Any]]:
    """按协议族给出「列模型」的 URL 与请求头（审计 R-4：三处曾各写一遍）。"""
    base = base_url.rstrip("/")
    if protocol == "ollama":
        return base + "/api/tags", {}  # Ollama 列模型无需鉴权
    if protocol == "anthropic":
        headers = (
            {"x-api-key": api_key, "anthropic-version": "2023-06-01"}
            if api_key
            else {}
        )
        return base + "/models", headers
    headers = {"Authorization": f"Bearer {api_key}"} if api_key else {}
    return base + "/models", headers


def parse_models_response(raw) -> list[str]:
    """兼容各家 /models 返回结构：data 可能是 list、dict 或嵌套 models。"""
    models: list[str] = []

    def collect(items) -> None:
        if not isinstance(items, list):
            return
        for item in items:
            if isinstance(item, dict) and item.get("id"):
                models.append(str(item["id"]))
            elif isinstance(item, str):
                models.append(item)

    if isinstance(raw, dict):
        data = raw.get("data")
        if isinstance(data, dict):
            # 例如 { "data": { "models": [...] } }
            collect(data.get("models") or data.get("data") or [])
        elif isinstance(data, list):
            collect(data)
        collect(raw.get("models") or [])
        # 极少数结构：{ "models": { "data": [...] } }
        models_obj = raw.get("models")
        if isinstance(models_obj, dict):
            collect(models_obj.get("data") or models_obj.get("models") or [])
    elif isinstance(raw, list):
        collect(raw)
    return models


def models_from_response(protocol: str, raw) -> list[str]:
    """把「列模型」响应解析成模型名列表（Ollama 结构与 OpenAI 系不同）。"""
    if protocol == "ollama":
        return [
            str(m.get("name", ""))
            for m in (raw.get("models") or [])
            if isinstance(m, dict) and m.get("name")
        ]
    return parse_models_response(raw)
