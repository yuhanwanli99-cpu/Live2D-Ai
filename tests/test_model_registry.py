"""PC 端 model_registry.py 单元测试 + Live2dModel registry 兜底测试（task-pc，契约 v3-rename 更名后同步 T0.1）。

依据 .pi/spoq/contract-v3-rename.md §1.2 / §1.4 / §1.6 / §5（R1-R8）：
- 模型 id `mao_pro` → `niziiro_mao`（显示名 Niziiro Mao）；
- shared/model_registry.json 仅 1 条（shizuku/hiyori/haru 删除），available+default；
- PC model_dict.json live2d 条目 `mao_pro`→`niziiro_mao`，url→`/live2d-models/niziiro_mao/runtime/niziiro_mao.model3.json`，
  shizuku 条目删除（R5 三处零残留）。
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

# ---- 样例数据（镜像契约后 registry 结构：niziiro_mao 主 + pc_only 用于平台过滤） ----

_SAMPLE_MODELS = [
    {
        "id": "niziiro_mao",
        "name": "Niziiro Mao",
        "description": "Niziiro Mao - Live2D Free Material sample, default",
        "model3JsonPath": "niziiro_mao/runtime/niziiro_mao.model3.json",
        "thumbnailPath": None,
        "available": True,
        "default": True,
        "restrictPlatform": None,
        "metadata": {"creator": "Live2D Inc.", "license": "Free Material License"},
    },
    {
        "id": "pc_only",
        "name": "PC Only",
        "description": "pc-only test model",
        "model3JsonPath": "pc_only/runtime/pc_only.model3.json",
        "thumbnailPath": None,
        "available": True,
        "default": False,
        "restrictPlatform": "pc",
        "metadata": None,
    },
]

_NIZIIRO_DICT_ENTRY = {
    "name": "niziiro_mao",
    "url": "/live2d-models/niziiro_mao/runtime/niziiro_mao.model3.json",
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
    """从临时 JSON 文件加载，get_all() 返回全部条目。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    assert len(registry.get_all()) == 2


def test_get_by_id_exists(tmp_path):
    """get_by_id('niziiro_mao') 返回正确条目。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    entry = registry.get_by_id("niziiro_mao")
    assert entry is not None
    assert entry.id == "niziiro_mao"
    assert entry.name == "Niziiro Mao"
    assert entry.available is True
    assert entry.default is True
    assert entry.model3_json_path == "niziiro_mao/runtime/niziiro_mao.model3.json"
    assert entry.thumbnail_path is None
    assert entry.restrict_platform is None
    assert entry.metadata == {"creator": "Live2D Inc.", "license": "Free Material License"}


def test_get_by_id_removed_ids_not_exists(tmp_path):
    """已删除/改名的 id（mao_pro/shisuku/hiyori/haru）不得再被查到（R5）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    for removed in ("mao_pro", "shizuku", "hiyori", "haru"):
        assert registry.get_by_id(removed) is None, f"{removed} 不应存在"


def test_get_by_id_not_exists(tmp_path):
    """get_by_id('nonexistent') 返回 None。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    assert registry.get_by_id("nonexistent") is None


def test_get_all_ids_no_legacy(tmp_path):
    """get_all() id 列表不得残留 mao_pro/shisuku/hiyori/haru（R5 零残留）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    ids = [e.id for e in registry.get_all()]
    assert ids == ["niziiro_mao", "pc_only"]
    assert not any(x in ids for x in ("mao_pro", "shizuku", "hiyori", "haru"))


def test_get_available_filter(tmp_path):
    """get_available() 按 available=true 过滤（不按 restrictPlatform）→ 返回 2 条。
    """
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    available = registry.get_available()
    assert [e.id for e in available] == ["niziiro_mao", "pc_only"]
    assert all(e.available for e in available)


def test_get_default_has_default(tmp_path):
    """get_default() 返回 niziiro_mao（default: true）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    default = registry.get_default()
    assert default.id == "niziiro_mao"
    assert default.default is True


def test_get_default_no_default(tmp_path):
    """无 default:true 时，返回第一个 available。"""
    models = [dict(m, default=False) for m in _SAMPLE_MODELS]
    registry = ModelRegistry(_write_registry(tmp_path, models))
    assert registry.get_default().id == "niziiro_mao"


def test_get_default_none_available(tmp_path):
    """全部 unavailable 时，抛出 RuntimeError。"""
    models = [dict(m, available=False) for m in _SAMPLE_MODELS]
    registry = ModelRegistry(_write_registry(tmp_path, models))
    with pytest.raises(RuntimeError):
        registry.get_default()


def test_get_model3_json_path(tmp_path):
    """get_model3_json_path('niziiro_mao') 返回正确路径。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    assert (
        registry.get_model3_json_path("niziiro_mao")
        == "niziiro_mao/runtime/niziiro_mao.model3.json"
    )


def test_get_model3_json_path_removed_ids_none(tmp_path):
    """get_model3_json_path('mao_pro'/'shizuku') 返回 None（R5）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    assert registry.get_model3_json_path("mao_pro") is None
    assert registry.get_model3_json_path("shizuku") is None


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
    assert len(registry.get_all()) == 2


def test_real_shared_registry_integration():
    """与真实 shared/model_registry.json 集成：2 条（bai 默认 + niziiro_mao 可切换）；无旧 id 残留。"""
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    registry_path = os.path.join(repo_root, "shared", "model_registry.json")
    registry = ModelRegistry(registry_path)
    assert len(registry.get_all()) == 2
    bai = registry.get_by_id("bai")
    assert bai.default is True
    assert bai.available is True
    assert bai.name == "白-免费版 (Bai Free)"
    assert bai.model3_json_path == "bai/runtime/bai.model3.json"
    assert bai.metadata.get("license"), "bai license 应非空"
    niz = registry.get_by_id("niziiro_mao")
    assert niz.default is False
    assert niz.available is True
    assert niz.name == "Niziiro Mao"
    assert niz.model3_json_path == "niziiro_mao/runtime/niziiro_mao.model3.json"
    # license 非空且 = Free Material License
    license_val = niz.metadata.get("license")
    assert license_val, "license 应非空"
    assert license_val == "Free Material License"
    # description 无 U+FFFD 乱码
    desc = (bai.description or "") + (niz.description or "")
    assert "\ufffd" not in desc
    assert {e.id for e in registry.get_available()} == {"bai", "niziiro_mao"}
    # 旧 id 零残留
    for removed in ("mao_pro", "shizuku", "hiyori", "haru"):
        assert registry.get_by_id(removed) is None, f"shared registry 不应残留 {removed}"


def test_real_pc_model_dict_no_legacy_live2d():
    """PC 仓库 model_dict.json 的 live2d 条目：niziiro_mao 存在，mao_pro/shisuku 零残留（R5/§1.2）。"""
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    model_dict_path = os.path.join(
        repo_root, "Live2Dai-pc", "open-llm-vtuber", "model_dict.json"
    )
    with open(model_dict_path, encoding="utf-8") as fh:
        entries = json.load(fh)
    names = [e.get("name") for e in entries]
    assert "niziiro_mao" in names, "model_dict.json 应含 niziiro_mao 的 live2d 条目"
    for legacy in ("mao_pro", "shizuku"):
        assert legacy not in names, f"model_dict.json 不应残留 {legacy}"
    mao = next(e for e in entries if e.get("name") == "niziiro_mao")
    assert mao["url"] == (
        "/live2d-models/niziiro_mao/runtime/niziiro_mao.model3.json"
    )
    # emotionMap / tapMotions 值保留
    assert "emotionMap" in mao
    assert "tapMotions" in mao


# ---- P2-1：已注册但不可用的 id 不能绕过默认回退（plan-e2e §7，task-e2e 修复） ----

def test_resolve_model_name_registered_available_returns_as_is(tmp_path):
    """请求名已登记且 available=true → 原样返回（niziiro_mao）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    assert registry.resolve_model_name("niziiro_mao") == "niziiro_mao"


def test_resolve_model_name_removed_falls_back(tmp_path):
    """已删除 id（mao_pro/shisuku/hiyori/haru）→ 回退默认 niziiro_mao（R5）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    for removed in ("mao_pro", "shizuku", "hiyori", "haru"):
        assert registry.resolve_model_name(removed) == "niziiro_mao"


def test_resolve_model_name_unknown_falls_back(tmp_path):
    """未登记 id → 回退默认 niziiro_mao（原有行为保持）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    assert registry.resolve_model_name("nonexistent") == "niziiro_mao"


def test_resolve_model_name_all_unavailable_raises_runtime_error(tmp_path):
    """P2-1 边界：全部 unavailable + conf 指向已注册不可用 id → RuntimeError（服务器拒绝启动）。"""
    models = [dict(m, available=False) for m in _SAMPLE_MODELS]
    registry = ModelRegistry(_write_registry(tmp_path, models))
    with pytest.raises(RuntimeError):
        registry.resolve_model_name("niziiro_mao")


# ---- Live2dModel：registry 兜底链路（schema fallbackChain） ----

def test_live2d_model_dict_hit_uses_full_info(tmp_path):
    """model_dict.json 命中 → 使用完整 model_info（emotionMap 保留）。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    model = Live2dModel(
        "niziiro_mao",
        model_dict_path=_write_model_dict(tmp_path, [_NIZIIRO_DICT_ENTRY]),
        registry=registry,
    )
    assert model.model_info["name"] == "niziiro_mao"
    assert (
        model.model_info["url"]
        == "/live2d-models/niziiro_mao/runtime/niziiro_mao.model3.json"
    )
    assert model.model_info["emotionMap"] == {"neutral": 0, "joy": 3, "anger": 2}
    assert model.emo_map == {"neutral": 0, "joy": 3, "anger": 2}


def test_live2d_model_registry_fallback_minimal(tmp_path):
    """model_dict.json 未命中 + registry 命中 → 最小 model_info（name+url+空 emotionMap）。

    覆盖 registry 兜底到 niziiro_mao（没有残留的 shizuku）。
    """
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    model = Live2dModel(
        "niziiro_mao",
        model_dict_path=_write_model_dict(tmp_path, []),
        registry=registry,
    )
    assert model.model_info["name"] == "niziiro_mao"
    assert (
        model.model_info["url"]
        == "/live2d-models/niziiro_mao/runtime/niziiro_mao.model3.json"
    )
    assert model.model_info["emotionMap"] == {}
    assert model.emo_map == {}
    assert model.emo_str == ""


def test_live2d_model_no_registry_raises_keyerror(tmp_path):
    """无 registry 且 model_dict.json 未命中 → KeyError。"""
    with pytest.raises(KeyError):
        Live2dModel(
            "niziiro_mao", model_dict_path=_write_model_dict(tmp_path, [])
        )


def test_live2d_model_missing_dict_falls_back_to_registry(tmp_path):
    """model_dict.json 缺失 + registry 命中 → 从 registry 构造最小 model_info。"""
    registry = ModelRegistry(_write_registry(tmp_path, _SAMPLE_MODELS))
    model = Live2dModel(
        "niziiro_mao",
        model_dict_path=str(tmp_path / "no_model_dict.json"),
        registry=registry,
    )
    assert (
        model.model_info["url"]
        == "/live2d-models/niziiro_mao/runtime/niziiro_mao.model3.json"
    )


def test_live2d_model_missing_dict_no_registry_raises(tmp_path):
    """model_dict.json 缺失且无 registry → FileNotFoundError。"""
    with pytest.raises(FileNotFoundError):
        Live2dModel("niziiro_mao", model_dict_path=str(tmp_path / "no_model_dict.json"))
