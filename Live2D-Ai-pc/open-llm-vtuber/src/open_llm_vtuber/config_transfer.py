# -*- coding: utf-8 -*-
"""JSON 配置导入/导出接口（仅 PC 端，与 Android 无关）。

提供两个端点：
  - GET  /api/config/export  导出 conf.yaml 原文 + shared/persona.yaml 解析结果
  - POST /api/config/import  校验后原子写入 conf.yaml / persona.yaml

安全约束：
  - 绝不读取或导出 .env；conf.yaml 里的 ${XXX_API_KEY} 占位符原样保留。
  - 写盘前用 AppConfig（Pydantic）做完整 schema 校验，校验失败不落盘。
  - 写盘走「临时文件 + os.replace」原子写；任一目标写失败则回滚两个文件。
"""

from __future__ import annotations

import copy
import os
import tempfile
from typing import Any

import yaml
from fastapi import APIRouter, Request
from loguru import logger
from pydantic import ValidationError
from starlette.responses import JSONResponse

_EXPORT_FORMAT = "live2dai-config"
_EXPORT_VERSION = 1


def _normalize_provider_id(pid: str) -> str:
    """把 UI 常用别名归一为配置里的 provider id（与 routes.py 保持一致）。"""
    return "openai_compatible_llm" if pid == "openai_compatible" else pid


def _apply_persona_to_data(data: dict, persona: dict) -> dict:
    """把 persona 的提示词/模型/温度/最大 token/视觉模型应用到候选配置。

    语义与 routes.py 的 _apply_persona_to_data 保持一致，但作用于当前选中的
    LLM Provider，不再硬编码 deepseek_llm。
    """
    cc = data.setdefault("character_config", {})
    if not isinstance(cc, dict):
        cc = data["character_config"] = {}
    if persona.get("system_prompt"):
        cc["persona_prompt"] = persona["system_prompt"]

    model_overrides = {
        k: persona[k]
        for k in ("model", "temperature", "max_tokens")
        if persona.get(k) is not None
    }
    if model_overrides:
        agent_cfg = cc.setdefault("agent_config", {})
        llm_configs = agent_cfg.setdefault("llm_configs", {})
        basic = (
            agent_cfg.setdefault("agent_settings", {})
            .setdefault("basic_memory_agent", {})
        )
        provider = _normalize_provider_id(
            basic.get("llm_provider") or "deepseek_llm"
        )
        if provider != basic.get("llm_provider"):
            basic["llm_provider"] = provider
        provider_cfg = llm_configs.setdefault(provider, {})
        if "model" in model_overrides:
            provider_cfg["model"] = model_overrides["model"]
        if "temperature" in model_overrides:
            provider_cfg["temperature"] = model_overrides["temperature"]
        if "max_tokens" in model_overrides:
            provider_cfg["max_tokens"] = model_overrides["max_tokens"]

    if persona.get("vision_model"):
        vision_cfg = data.setdefault("vision_config", {})
        if not isinstance(vision_cfg, dict):
            vision_cfg = data["vision_config"] = {}
        vision_cfg["model"] = persona["vision_model"]
    return data


def init_config_transfer_routes(
    config_path: str = "conf.yaml",
    repo_root: str | None = None,
    plugin_manager=None,
) -> APIRouter:
    """注册 JSON 配置导入/导出 API。

    Args:
        config_path: conf.yaml 路径（相对路径按进程 cwd 解析）。
        repo_root: 仓库根目录；None 时由 config_path 向上三级推导（与 routes.py 一致）。
        plugin_manager: 可选 PluginManager；提供后，config-pack 插件未启用时接口不可用。
    """
    router = APIRouter()
    config_abs = os.path.abspath(config_path)
    if repo_root is None:
        repo_root = os.path.dirname(os.path.dirname(os.path.dirname(config_abs)))
    persona_path = os.path.join(repo_root, "shared", "persona.yaml")

    def _plugin_reason() -> str | None:
        if plugin_manager is None or plugin_manager.is_enabled("config-pack"):
            return None
        return "config-pack 插件未启用，配置导入/导出不可用"

    def _read_text(path: str) -> str:
        with open(path, "r", encoding="utf-8") as f:
            return f.read()

    def _atomic_write_text(path: str, content: str) -> None:
        """临时文件 + fsync + os.replace 原子写入，避免留下半份文件。"""
        directory = os.path.dirname(os.path.abspath(path)) or "."
        os.makedirs(directory, exist_ok=True)
        prefix = "." + os.path.basename(path) + "."
        fd, tmp_path = tempfile.mkstemp(
            prefix=prefix, suffix=".tmp", dir=directory
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(content)
                f.flush()
                os.fsync(f.fileno())
            os.replace(tmp_path, path)
        finally:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)

    def _restore(path: str, backup: str | None) -> None:
        """把 path 恢复到备份内容；backup 为 None 表示原文件不存在，删除它。"""
        try:
            if backup is None:
                if os.path.exists(path):
                    os.remove(path)
            else:
                _atomic_write_text(path, backup)
        except Exception as rollback_error:  # noqa: BLE001
            logger.error(f"config import rollback failed for {path}: {rollback_error}")

    def _load_persona_dict() -> dict[str, Any]:
        if not os.path.exists(persona_path):
            return {}
        try:
            with open(persona_path, "r", encoding="utf-8") as f:
                parsed = yaml.safe_load(f) or {}
            if not isinstance(parsed, dict):
                logger.warning(f"persona.yaml 不是映射，按空对象处理: {persona_path}")
                return {}
            return parsed
        except Exception as e:  # noqa: BLE001 - 导出不应因 persona 损坏而整体失败
            logger.warning(f"persona.yaml 解析失败，按空对象处理: {e}")
            return {}

    @router.get("/api/config/export")
    async def export_config():
        """导出配置：conf.yaml 原始文本 + shared/persona.yaml 解析后的 dict。"""
        reason = _plugin_reason()
        if reason:
            return JSONResponse({"ok": False, "error": reason}, status_code=403)
        try:
            conf_yaml = _read_text(config_abs)
            persona = _load_persona_dict()
            return {
                "ok": True,
                "format": _EXPORT_FORMAT,
                "version": _EXPORT_VERSION,
                "conf_yaml": conf_yaml,
                "persona": persona,
            }
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to export config: {e}")
            return JSONResponse({"ok": False, "error": str(e)}, status_code=500)

    @router.post("/api/config/import")
    async def import_config(request: Request):
        """导入配置：校验 conf_yaml/persona 后原子写盘，失败回滚。"""
        reason = _plugin_reason()
        if reason:
            return JSONResponse({"ok": False, "error": reason}, status_code=403)
        try:
            payload = await request.json()
        except Exception:  # noqa: BLE001
            return JSONResponse(
                {"ok": False, "error": "请求体必须是合法 JSON"}, status_code=400
            )
        if not isinstance(payload, dict):
            return JSONResponse(
                {"ok": False, "error": "请求体必须是 JSON 对象"}, status_code=400
            )

        conf_yaml_text = payload.get("conf_yaml")
        if not isinstance(conf_yaml_text, str) or not conf_yaml_text.strip():
            return JSONResponse(
                {"ok": False, "error": "conf_yaml 必须是非空字符串"}, status_code=400
            )

        persona = payload.get("persona")
        persona_provided = persona is not None
        if persona_provided and not isinstance(persona, dict):
            return JSONResponse(
                {"ok": False, "error": "persona 必须是对象"}, status_code=400
            )

        # 1) YAML 语法 + 顶层结构校验
        try:
            conf_data = yaml.safe_load(conf_yaml_text)
        except yaml.YAMLError as e:
            return JSONResponse(
                {"ok": False, "error": f"conf_yaml 不是合法 YAML: {e}"},
                status_code=400,
            )
        if not isinstance(conf_data, dict):
            return JSONResponse(
                {"ok": False, "error": "conf_yaml 顶层必须是键值映射（对象）"},
                status_code=400,
            )

        # 2) 构造候选配置并做完整 schema 校验
        candidate = copy.deepcopy(conf_data)
        if persona_provided:
            _apply_persona_to_data(candidate, persona or {})

        try:
            from .config_manager.main import Config as AppConfig

            AppConfig(**candidate)
        except ValidationError as validation_error:
            logger.error(f"Config import validation failed: {validation_error}")
            field_errors = {}
            for err in validation_error.errors():
                loc = ".".join(str(x) for x in err.get("loc", []))
                field_errors[loc] = err.get("msg", str(err))
            return JSONResponse(
                {
                    "ok": False,
                    "error": "配置校验失败",
                    "field_errors": field_errors,
                },
                status_code=400,
            )
        except Exception as validation_error:  # noqa: BLE001
            logger.error(f"Config import validation failed: {validation_error}")
            return JSONResponse(
                {"ok": False, "error": f"配置校验失败: {validation_error}"},
                status_code=400,
            )

        # 3) 备份原文件 → 原子写入；任一失败则回滚两个文件
        conf_backup = _read_text(config_abs) if os.path.exists(config_abs) else None
        persona_backup = (
            _read_text(persona_path) if os.path.exists(persona_path) else None
        )

        try:
            _atomic_write_text(config_abs, conf_yaml_text)
        except Exception as e:  # noqa: BLE001
            logger.error(f"Failed to persist conf.yaml on import: {e}")
            _restore(config_abs, conf_backup)
            return JSONResponse(
                {"ok": False, "error": f"写入 conf.yaml 失败: {e}"}, status_code=500
            )

        if persona_provided:
            try:
                persona_text = yaml.safe_dump(
                    persona, allow_unicode=True, sort_keys=False
                )
                _atomic_write_text(persona_path, persona_text)
            except Exception as e:  # noqa: BLE001
                logger.error(f"Failed to persist persona.yaml on import: {e}")
                _restore(persona_path, persona_backup)
                _restore(config_abs, conf_backup)
                return JSONResponse(
                    {"ok": False, "error": f"写入 persona.yaml 失败: {e}"},
                    status_code=500,
                )

        return {"ok": True, "persisted": True, "message": "已导入"}

    return router
