import copy
from typing import Any

from loguru import logger


class ConfigEndpointService:
    """/api/config 与 /api/config/tree 的写盘与校验逻辑。"""

    def __init__(self, config_service, config_helper, env_path: str):
        self.config_service = config_service
        self.config_helper = config_helper
        self.env_path = env_path

    def update_config_tree(self, raw: str) -> tuple[dict[str, Any], int]:
        import yaml as _yaml

        tree = _yaml.safe_load(raw)
        if not isinstance(tree, dict):
            return {"error": "配置树顶层必须是键值映射（YAML object）"}, 400

        candidate = copy.deepcopy(tree)
        self.config_helper.apply_persona_to_data(
            candidate, self.config_service.load_persona() or {}
        )
        try:
            from pydantic import ValidationError as _PydanticValidationError

            from open_llm_vtuber.config_manager.main import Config as AppConfig

            AppConfig(**candidate)
        except _PydanticValidationError as validation_error:
            logger.error(f"Config tree validation failed: {validation_error}")
            field_errors = {}
            for err in validation_error.errors():
                loc = ".".join(str(x) for x in err.get("loc", []))
                field_errors[loc] = err.get("msg", str(err))
            return {
                "ok": False,
                "error": "配置校验失败: 请检查标红字段",
                "field_errors": field_errors,
            }, 400
        except Exception as validation_error:  # noqa: BLE001
            logger.error(f"Config tree validation failed: {validation_error}")
            return {
                "ok": False,
                "error": f"配置校验失败: {validation_error}",
                "field_errors": {},
            }, 400

        self.config_service.write_raw_atomic(raw)
        return {
            "ok": True,
            "config": self.config_helper.get_common(tree),
            "applied": False,
            "requires_restart": True,
            "message": "已保存",
        }, 200
