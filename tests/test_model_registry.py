"""PC 端 model_registry.py 单元测试 + Live2dModel registry 兜底测试（task-pc）。

覆盖 plan-pc.md §10.1 的 13 个用例（ModelRegistry 全部公开方法 + 边界），
并补充：
- ModelRegistry 与真实 shared/model_registry.json 的集成断言
- Live2dModel 的 registry 兜底链路（plan §4.4 / schema fallbackChain）：
  model_dict.json 命中 → 完整 model_info；未命中/缺失 → registry 最小 model_info；
  两处都无 → KeyError/FileNotFoundError。
"""
import json
import os
import sys

import pytest

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2Dai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.live2d_model import Live2dModel  # noqa: E402
from open_llm_vtuber.model_registry import ModelRegistry  # noqa: E402

# ---- 样例数据（镜像 shared/model_registry.json 的 4 条结构） ----

_SAMPLE_MODELS = [
    {
        "id": "mao_pro",
        "name": "Mao Pro",
        "description": "猫娘 Mao 的 Live2D PRO 版模型",
        "model3JsonPath": "mao_pro/runtime/mao_pro.model3.json",
        "thumbnailPath": None,
        "available": True,
        "default": True,
        "restrictPlatform": None,
        "metadata": {"creator": "Live2D Inc."},
    },
    {
        "id": "shizuku",
        "name": "Shizuku",
        "description": "Live2D 官方示例角色「しずく」PRO 版",
        "model3JsonPath": "shizuku/runtime/shizuku.model3.json",
        "thumbnailPath": None,
        "available": True,
        "default": False,
        "restrictPlatform": None,
        "metadata": {"creator": "Live2D Inc."},
    },
    {
        "id": "hiyori",
        "name": "Hiyori",
        "description": "预留模型",
        "model3JsonPath": "hiyori/runtime/hiyori.model3.json",
        "thumbnailPath": None,
        "available": False,
        "default": False,
        "restrictPlatform": None,
        "metadata": None,
    },
    {
        "id": "haru",
        "name": "Haru",
        "description": "预留模型",
        "model3JsonPath": "haru/runtime/haru.model3.json",
        "thumbnailPath": None,
        "available": False,
        "default": False,
        "restrictPlatform": None,
        "metadata": None,
    },
]

_MAO_DICT_ENTRY = {
    "name": "mao_pro",
    "url": "/live2d-models/mao_pro/runtime/mao_pro.model3.json",
    "kScale": 0.5,
    "initialXshift": 0,
    "initialYshift": 0,
    "kXOffset": 1150,
    "idleMotionGroupName": "Idle",
    "emotionMap": {"neutral": 0, "joy": 3, "anger": 2},
    "tapMotions": {"HitAreaHead": {"": 1}},
}


def _write_registry(tmp_path, models):
    path = tmp_path / "model_registry.json"
    path.write_text(
        json.dumps({"version": "1.0", "models": models}, ensure_ascii=False),
        encoding="utf-8",
    )
    return str(path)


def _write_model_dict(tmp_path, entries):
    path = tmp_path / "model_dict.json"
    path.write_text(json.dumps(entries, ensure_ascii=False), encoding="utf-8")
    return str(path)


# ---- ModelRegistry：加载与查询 ----

def test_load_valid_registry(tmp_path):
    """从临时 JSON 文件加载，get_all() 返回 4 条。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    assert len(registry.get_all()) == 4


def test_get_by_id_exists(tmp_path):
    """get_by_id('mao_pro') 返回正确条目。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    entry = registry.get_by_id("mao_pro")
    assert entry is not None
    assert entry.id == "mao_pro"
    assert entry.name == "Mao Pro"
    assert entry.available is True
    assert entry.default is True
    assert entry.model3_json_path == "mao_pro/runtime/mao_pro.model3.json"
    assert entry.thumbnail_path is None
    assert entry.restrict_platform is None
    assert entry.metadata == {"creator": "Live2D Inc."}


def test_get_by_id_not_exists(tmp_path):
    """get_by_id('nonexistent') 返回 None。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    assert registry.get_by_id("nonexistent") is None


def test_get_all_count(tmp_path):
    """get_all() 返回 4 条（含 unavailable），按注册表顺序。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    entries = registry.get_all()
    assert len(entries) == 4
    assert [e.id for e in entries] == ["mao_pro", "shizuku", "hiyori", "haru"]


def test_get_available_filter(tmp_path):
    """get_available() 返回 2 条（mao_pro, shizuku）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    available = registry.get_available()
    assert [e.id for e in available] == ["mao_pro", "shizuku"]
    assert all(e.available for e in available)


def test_get_default_has_default(tmp_path):
    """get_default() 返回 mao_pro（default: true）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    default = registry.get_default()
    assert default.id == "mao_pro"
    assert default.default is True


def test_get_default_no_default(tmp_path):
    """无 default:true 时，返回第一个 available。"""
    models = [dict(m, default=False) for m in _SAMPLE_MODELS]
    registry = ModelRegistry(_write_registry(tmp_path, models))
    assert registry.get_default().id == "mao_pro"


def test_get_default_none_available(tmp_path):
    """全部 unavailable 时，抛出 RuntimeError。"""
    models = [dict(m, available=False) for m in _SAMPLE_MODELS]
    registry = ModelRegistry(_write_registry(tmp_path, models))
    with pytest.raises(RuntimeError):
        registry.get_default()


def test_get_model3_json_path(tmp_path):
    """get_model3_json_path('shizuku') 返回正确路径。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    assert (
        registry.get_model3_json_path("shizuku")
        == "shizuku/runtime/shizuku.model3.json"
    )


def test_get_model3_json_path_not_exists(tmp_path):
    """get_model3_json_path('nonexistent') 返回 None。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    assert registry.get_model3_json_path("nonexistent") is None


# ---- ModelRegistry：边界/异常 ----

def test_load_missing_file(tmp_path):
    """不存在的路径抛出 FileNotFoundError。"""
    with pytest.raises(FileNotFoundError):
        ModelRegistry(str(tmp_path / "no_such_registry.json"))


def test_load_invalid_json(tmp_path):
    """非法 JSON 抛出 json.JSONDecodeError。"""
    path = tmp_path / "model_registry.json"
    path.write_text("{ not valid json !!!", encoding="utf-8")
    with pytest.raises(json.JSONDecodeError):
        ModelRegistry(str(path))


def test_load_empty_models(tmp_path):
    """models 数组为空：get_all() 为空，get_default() 抛出 RuntimeError。"""
    registry = ModelRegistry(_write_registry(tmp_path, []))
    assert registry.get_all() == []
    with pytest.raises(RuntimeError):
        registry.get_default()


def test_load_missing_required_field(tmp_path):
    """必填字段缺失的条目被跳过（健壮处理）：get_by_id 返回 None。"""
    broken = {
        "id": "broken",
        "name": "Broken",
        # 缺少必填字段 model3JsonPath / available / default
    }
    registry = ModelRegistry(_write_registry(tmp_path, [*_SAMPLE_MODELS, broken]))
    assert registry.get_by_id("broken") is None
    assert len(registry.get_all()) == 4


def test_real_shared_registry_integration():
    """与真实 shared/model_registry.json 集成：4 条、mao_pro 默认、shizuku 可用。"""
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    registry_path = os.path.join(repo_root, "shared", "model_registry.json")
    registry = ModelRegistry(registry_path)
    assert len(registry.get_all()) == 4
    assert registry.get_by_id("mao_pro").default is True
    assert registry.get_by_id("mao_pro").available is True
    assert registry.get_by_id("shizuku").available is True
    assert {e.id for e in registry.get_available()} == {"mao_pro", "shizuku"}


# ---- Live2dModel：registry 兜底链路（schema fallbackChain） ----

def test_live2d_model_dict_hit_uses_full_info(tmp_path):
    """model_dict.json 命中 → 使用完整 model_info（emotionMap 保留）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    model = Live2dModel(
        "mao_pro",
        model_dict_path=_write_model_dict(tmp_path, [_MAO_DICT_ENTRY]),
        registry=registry,
    )
    assert model.model_info["name"] == "mao_pro"
    assert model.model_info["url"] == "/live2d-models/mao_pro/runtime/mao_pro.model3.json"
    assert model.model_info["emotionMap"] == {"neutral": 0, "joy": 3, "anger": 2}
    assert model.emo_map == {"neutral": 0, "joy": 3, "anger": 2}


def test_live2d_model_registry_fallback_minimal(tmp_path):
    """model_dict.json 未命中 + registry 命中 → 最小 model_info（name+url+空 emotionMap）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    model = Live2dModel(
        "shizuku",
        model_dict_path=_write_model_dict(tmp_path, [_MAO_DICT_ENTRY]),
        registry=registry,
    )
    assert model.model_info["name"] == "shizuku"
    assert model.model_info["url"] == "/live2d-models/shizuku/runtime/shizuku.model3.json"
    assert model.model_info["emotionMap"] == {}
    assert model.emo_map == {}
    assert model.emo_str == ""


def test_live2d_model_no_registry_raises_keyerror(tmp_path):
    """无 registry 且 model_dict.json 未命中 → KeyError。"""
    with pytest.raises(KeyError):
        Live2dModel(
            "shizuku", model_dict_path=_write_model_dict(tmp_path, [_MAO_DICT_ENTRY])
        )


def test_live2d_model_missing_dict_falls_back_to_registry(tmp_path):
    """model_dict.json 缺失 + registry 命中 → 从 registry 构造最小 model_info。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    model = Live2dModel(
        "shizuku",
        model_dict_path=str(tmp_path / "no_model_dict.json"),
        registry=registry,
    )
    assert model.model_info["url"] == "/live2d-models/shizuku/runtime/shizuku.model3.json"


def test_live2d_model_missing_dict_no_registry_raises(tmp_path):
    """model_dict.json 缺失且无 registry → FileNotFoundError。"""
    with pytest.raises(FileNotFoundError):
        Live2dModel("mao_pro", model_dict_path=str(tmp_path / "no_model_dict.json"))
