"""PC 端 Live2D 模型注册表读取器（task-pc 新增）。

读取 `shared/model_registry.json`（跨平台唯一真相源，plan-registry 产出），
提供模型查询接口。设计决策（与 plan-registry.md / plan-pc.md 一致）：

- 本模块只负责**注册/发现元数据**（id、name、model3JsonPath、available、
  default、restrictPlatform 等）；渲染参数（emotionMap、kScale、tapMotions
  等）仍由 PC 端 `model_dict.json` 持有，两者通过 `id == name` 关联。
- `model3JsonPath` 相对于平台 Live2D 资源根（PC: `live2d-models/`）。
- 本模块**不假设工作目录**：路径由调用方传入（Server 端用
  `service_context._resolve_shared_path()`，测试端用 `tmp_path`）。

契约（plan-pc.schema.json）：
- `ModelEntry` dataclass：字段与 registry JSON 条目一一对应（camelCase →
  snake_case）。
- `get_default()` 兜底：`default=true` → 首个 `available=true` → RuntimeError。
- 健壮性：必填字段（id/name/model3JsonPath/available/default）缺失的条目
  直接跳过（get_by_id 返回 None），文件缺失/JSON 非法时按契约抛异常。
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Optional

# registry JSON 字段名（camelCase）→ ModelEntry 字段名（snake_case）映射
_FIELD_MAP = {
    "id": "id",
    "name": "name",
    "description": "description",
    "model3JsonPath": "model3_json_path",
    "thumbnailPath": "thumbnail_path",
    "available": "available",
    "default": "default",
    "restrictPlatform": "restrict_platform",
    "metadata": "metadata",
}

# 必填字段（shared/model_registry.schema.json modelEntry.required）
# 缺失时整条跳过（健壮处理，保证 get_by_id/get_default 不裸崩溃）。
_REQUIRED_KEYS = ("id", "name", "model3JsonPath", "available", "default")


@dataclass
class ModelEntry:
    """shared/model_registry.json 中单个模型条目（PC 端视图）。"""

    id: str
    name: str
    description: Optional[str] = None
    model3_json_path: str = ""
    thumbnail_path: Optional[str] = None
    available: bool = False
    default: bool = False
    restrict_platform: Optional[str] = None
    metadata: Optional[dict] = None


class ModelRegistry:
    """从 JSON 文件加载并查询 Live2D 模型注册表。"""

    def __init__(self, registry_path: str) -> None:
        """解析 registry JSON 并缓存全部条目。

        Raises:
            FileNotFoundError: registry 文件不存在。
            json.JSONDecodeError: registry 文件不是合法 JSON。
            ValueError: 顶层结构非法（非对象 / models 非数组）。
        """
        with open(registry_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        if not isinstance(data, dict):
            raise ValueError("Model registry root must be a JSON object.")
        raw_models = data.get("models", [])
        if not isinstance(raw_models, list):
            raise ValueError("'models' must be a JSON array.")

        self._models: list[ModelEntry] = [
            entry
            for entry in (self._parse_entry(model) for model in raw_models)
            if entry is not None
        ]

    @staticmethod
    def _parse_entry(raw: dict) -> Optional[ModelEntry]:
        """将 JSON 条目解析为 ModelEntry；必填字段缺失或类型非法时返回 None。"""
        if not isinstance(raw, dict):
            return None
        if any(key not in raw for key in _REQUIRED_KEYS):
            return None
        entry = ModelEntry(
            id=raw["id"],
            name=raw["name"],
            available=raw["available"],
            default=raw["default"],
        )
        for json_key, attr in _FIELD_MAP.items():
            if json_key in raw:
                setattr(entry, attr, raw[json_key])
        return entry

    def get_by_id(self, model_id: str) -> Optional[ModelEntry]:
        """按 id 查找条目；不存在返回 None。O(n) 线性查找。"""
        for model in self._models:
            if model.id == model_id:
                return model
        return None

    def get_all(self) -> list[ModelEntry]:
        """返回全部条目（含 unavailable），按注册表顺序。"""
        return list(self._models)

    def get_available(self) -> list[ModelEntry]:
        """仅返回 available=true 的条目。"""
        return [model for model in self._models if model.available]

    def get_default(self) -> ModelEntry:
        """返回默认条目：default=true 且 available=true → 首个 available=true → RuntimeError。

        说明：default 分支额外要求 available=true（与 model_registry.schema.json 的
        不变量「default 条目必须 available」一致）——否则会出现 plan 边界表
        「全部 unavailable → RuntimeError（服务器启动失败）」被 default 标记绕过。

        Raises:
            RuntimeError: 注册表中无可用模型（服务器启动失败）。
        """
        for model in self._models:
            if model.default and model.available:
                return model
        for model in self._models:
            if model.available:
                return model
        raise RuntimeError("No available model found in the model registry.")

    def get_model3_json_path(self, model_id: str) -> Optional[str]:
        """便捷方法：返回 model3.json 相对路径；模型不存在返回 None。"""
        entry = self.get_by_id(model_id)
        return entry.model3_json_path if entry is not None else None

    def resolve_model_name(self, requested: str) -> str:
        """按 P2-1 边界解析请求的模型名 → 实际使用的模型名（task-e2e 修复）。

        - 请求名已登记且 available=true → 原样返回。
        - 请求名未登记 / 已登记但不可用（available=false）→ 回退 get_default()。

        用途：conf.yaml `live2d_model_name` 的消费点（service_context
        `_resolve_live2d_model_name`）统一走此语义——避免 conf 指向「已注册但
        不可用」的 id 时绕过默认回退（plan-e2e.md §7 P2-1：全部 unavailable 时
        get_default() 抛 RuntimeError，服务器按 plan 边界拒绝启动）。

        Raises:
            RuntimeError: 注册表中无可用模型（get_default 语义）。
        """
        entry = self.get_by_id(requested)
        if entry is not None and entry.available:
            return requested
        return self.get_default().id
