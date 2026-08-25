import json
import os
from typing import Any

from loguru import logger


class ModelService:
    """Live2D 模型注册表与形象切换服务。"""

    def __init__(self, repo_root: str, config_service, ws_handler=None):
        self.repo_root = repo_root
        self.config_service = config_service
        self.ws_handler = ws_handler

    def _registry_path(self) -> str:
        return os.path.join(self.repo_root, "shared", "model_registry.json")

    def get_models(self) -> dict[str, Any]:
        reg_path = self._registry_path()
        if os.path.exists(reg_path):
            with open(reg_path, "r", encoding="utf-8") as f:
                reg = json.load(f)
            models = []
            live2d_root = os.path.join(os.getcwd(), "live2d-models")
            for raw in reg.get("models", []):
                model = dict(raw)
                relative = str(model.get("model3JsonPath") or "")
                # Registry metadata may describe an optional model that is not bundled
                # in the public Preview. Report runtime availability from disk rather
                # than exposing the historical registry flag as a false positive.
                model["available"] = bool(relative) and os.path.isfile(
                    os.path.join(live2d_root, relative)
                )
                models.append(model)
            return {"models": models}
        return {"models": []}

    def scan_models(self) -> dict[str, Any]:
        live2d_dir = os.path.join(os.getcwd(), "live2d-models")
        reg_path = self._registry_path()
        registry: dict[str, Any] = {"version": "1.0", "models": []}
        if os.path.exists(reg_path):
            with open(reg_path, "r", encoding="utf-8") as f:
                registry = json.load(f)
        existing = {m.get("id") for m in registry.get("models", [])}
        added: list[str] = []
        if os.path.isdir(live2d_dir):
            for entry in os.scandir(live2d_dir):
                if not entry.is_dir():
                    continue
                folder = entry.name
                model3 = os.path.join(live2d_dir, folder, f"{folder}.model3.json")
                if not os.path.isfile(model3):
                    continue
                if folder in existing:
                    continue
                registry.setdefault("models", []).append(
                    {
                        "id": folder,
                        "name": folder,
                        "description": "本地扫描导入的 Live2D 模型",
                        "model3JsonPath": f"{folder}/{folder}.model3.json",
                        "thumbnailPath": None,
                        "available": True,
                        "default": False,
                        "restrictPlatform": None,
                        "metadata": {"creator": "local", "license": "user", "version": "1.0"},
                    }
                )
                existing.add(folder)
                added.append(folder)
        if added:
            with open(reg_path, "w", encoding="utf-8") as f:
                json.dump(registry, f, ensure_ascii=False, indent=2)
        return {"ok": True, "added": added, "models": registry.get("models", [])}

    async def select_model(self, model_id: str) -> dict[str, Any]:
        if not model_id:
            raise ValueError("model_id 不能为空")

        data = self.config_service.load_yaml()
        cc = data.setdefault("character_config", {})
        cc["live2d_model_name"] = model_id
        cc["conf_name"] = model_id
        self.config_service.save_yaml(data)

        applied = 0
        hot_error = ""
        if self.ws_handler is not None:
            try:
                applied = await self.ws_handler.broadcast_model_change(model_id)
            except Exception as e:  # noqa: BLE001
                hot_error = str(e)
                logger.error(f"形象热切换失败（配置已保存）: {e}")

        if hot_error:
            return {
                "ok": True,
                "applied": False,
                "requires_restart": True,
                "message": f"已保存，重启后生效（热切换失败: {hot_error}）",
            }
        return {
            "ok": True,
            "applied": True,
            "requires_restart": False,
            "clients_updated": applied,
            "message": (
                f"已切换并即时生效（{applied} 个窗口已更新）"
                if applied
                else "已切换；当前没有已连接窗口，打开页面即生效"
            ),
        }
