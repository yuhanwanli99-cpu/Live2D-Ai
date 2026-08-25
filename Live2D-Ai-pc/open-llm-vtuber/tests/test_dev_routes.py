"""开发者模式本地路由测试（api/dev_routes.py）。

覆盖契约：
  - developer_mode 默认关闭 → 全部 /api/dev/* 403；
  - localhost 限制：非 localhost 请求即使开启 dev 模式也 403；
  - 动作 catalog（白名单 + halfbody channels + capabilities）；
  - 合法 trigger / release；
  - 未知动作 400；
  - 强度校验（越界/非法回落 2）；
  - director analyze 脱敏（raw 等字段被剥离，数值参数被钳制）；
  - LIVE2DAI_DEV_MODE=1 环境变量覆盖。

原则：与 settings_env 一致，使用隔离 FastAPI TestClient + mock ServiceContext，
绝不触碰真实 conf.yaml / 网络 / LLM。TestClient 默认 client host 为
"testclient"（非环回），因此“放行”用例显式把 _is_localhost 设为 True，
“localhost 限制”用例显式设为 False，保证断言与 starlette 版本无关。
"""

from types import SimpleNamespace

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from open_llm_vtuber.api import dev_routes as dev_routes_module
from open_llm_vtuber.api.dev_routes import ALLOWED_ACTIONS, init_dev_routes
from open_llm_vtuber.director import DirectorResult
from open_llm_vtuber.motion_catalog import (
    MOTION_REQUIRED_PARAMS,
    allowed_actions_for_parameter_ids,
)

# bai 形状模型参数集（含非标 Param4 嘟嘴 / Param6 歪嘴）
_BAI_PARAM_IDS = {
    "ParamAngleX", "ParamAngleY", "ParamAngleZ",
    "ParamEyeLOpen", "ParamEyeROpen", "ParamEyeBallX", "ParamEyeBallY",
    "ParamBrowLY", "ParamBrowRY",
    "ParamMouthOpenY", "ParamMouthForm",
    "ParamBreath", "Param4", "Param5", "Param6",
}


class _FakeDirector:
    """mock 导演：可配置返回结果或抛错，记录调用文本。"""

    def __init__(self, result: DirectorResult | None = None, error: Exception | None = None):
        self.result = result
        self.error = error
        self.calls: list[str] = []

    async def analyze(self, text: str) -> DirectorResult:
        self.calls.append(text)
        if self.error is not None:
            raise self.error
        return self.result


class _FakeWSHandler:
    """mock WebSocketHandler：记录 broadcast_dev_action 调用。"""

    def __init__(self):
        self.broadcasts: list[tuple[str, int]] = []

    async def broadcast_dev_action(self, action: str, strength: int) -> None:
        self.broadcasts.append((action, strength))


class _DevEnv:
    """隔离的 dev routes 环境。"""

    def __init__(self, client, ctx, ws, monkeypatch):
        self.client = client
        self.ctx = ctx
        self.ws = ws
        self._mp = monkeypatch

    def enable_dev_mode(self) -> None:
        self.ctx.system_config.developer_mode = True

    def enable_localhost(self) -> None:
        self._mp.setattr(dev_routes_module, "_is_localhost", lambda request: True)

    def block_non_localhost(self) -> None:
        self._mp.setattr(dev_routes_module, "_is_localhost", lambda request: False)


def _make_fake_context(director=None) -> SimpleNamespace:
    return SimpleNamespace(
        system_config=SimpleNamespace(developer_mode=False),
        director=director,
        live2d_model=None,
    )


@pytest.fixture
def dev_env(monkeypatch):
    fake_ws = _FakeWSHandler()
    fake_ctx = _make_fake_context()
    app = FastAPI()
    app.include_router(
        init_dev_routes(
            default_context_cache=fake_ctx,
            ws_handler=fake_ws,
        )
    )
    client = TestClient(app)
    env = _DevEnv(client, fake_ctx, fake_ws, monkeypatch)
    return env


# ─────────────────────────────────────────────
# 默认关闭（developer_mode=False）→ 403
# ─────────────────────────────────────────────
class TestDefaultDisabled:
    @pytest.mark.parametrize(
        "method,path,payload",
        [
            ("get", "/api/dev/actions/catalog", None),
            ("post", "/api/dev/actions/trigger", {"action": "nod"}),
            ("post", "/api/dev/director/analyze", {"text": "你好"}),
        ],
    )
    def test_all_dev_endpoints_403_when_disabled(self, dev_env, method, path, payload):
        r = dev_env.client.request(method, path, json=payload)
        assert r.status_code == 403
        assert r.json()["error"] == "developer mode disabled"

    def test_disabled_never_touches_broadcast(self, dev_env):
        dev_env.client.post("/api/dev/actions/trigger", json={"action": "nod"})
        assert dev_env.ws.broadcasts == []


# ─────────────────────────────────────────────
# localhost 限制
# ─────────────────────────────────────────────
class TestLocalhostOnly:
    def test_enabled_but_non_localhost_catalog_403(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.block_non_localhost()
        r = dev_env.client.get("/api/dev/actions/catalog")
        assert r.status_code == 403
        assert r.json()["error"] == "localhost only"

    def test_enabled_but_non_localhost_trigger_403(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.block_non_localhost()
        r = dev_env.client.post("/api/dev/actions/trigger", json={"action": "nod"})
        assert r.status_code == 403
        assert dev_env.ws.broadcasts == []

    def test_enabled_but_non_localhost_analyze_403(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.block_non_localhost()
        r = dev_env.client.post("/api/dev/director/analyze", json={"text": "hi"})
        assert r.status_code == 403


# ─────────────────────────────────────────────
# 动作 catalog
# ─────────────────────────────────────────────
class TestCatalog:
    def test_catalog_ok_when_enabled_and_localhost(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.get("/api/dev/actions/catalog")
        assert r.status_code == 200
        body = r.json()
        # 模型未知（live2d_model=None）→ fail-closed：目录只含无门槛基础动作 + idle
        expected = allowed_actions_for_parameter_ids(set())
        assert body["actions"] == sorted(expected)
        assert set(body["actions"]) <= set(ALLOWED_ACTIONS)  # 永远是静态白名单子集
        # 富目录：中文 label / description / halfbody / 建议时长 / release / requires。
        entries = body["action_meta"]
        assert {e["id"] for e in entries} == set(body["actions"])
        for entry in entries:
            assert isinstance(entry["label"], str) and entry["label"]
            assert isinstance(entry["description"], str)
            assert entry["halfbody"] is True, entry["id"]
            assert isinstance(entry["suggested_duration_ms"], int)
            assert entry["suggested_duration_ms"] > 0
            assert isinstance(entry["release_only"], bool)
            assert isinstance(entry["requires"], list)
            assert set(entry["requires"]) == set(
                MOTION_REQUIRED_PARAMS.get(entry["id"], ())
            )
        assert isinstance(body["halfbody_channels"], list)
        assert isinstance(body["capabilities"], dict)
        # 六个基础动作 + idle(release) 必须出现在白名单里
        for action in ("nod", "shake_no", "tilt", "look_around", "listen", "surprise"):
            assert action in body["actions"]
        # 能力门控动作在模型参数集未知时不显示（fail-closed）
        for gated in ("look_up", "glance_left", "smile", "pout", "wry_mouth", "deep_breath"):
            assert gated not in body["actions"], gated
        # 已删动作（bow/lean_in/happy/excited/sway/wave_big/bounce_happy/
        # wink_left/wink_right）绝不出现在 dev 目录 —— 与公开/导演协议同一目录
        # 边界。wink 双 id 经用户真机核验不可用，已整体删除。
        for removed in (
            "bow", "lean_in", "happy", "excited", "sway", "wave_big",
            "bounce_happy", "wink_left", "wink_right",
        ):
            assert removed not in body["actions"], removed
        # idle 仅作为 release，不能作为正向触发
        idle_entry = next(e for e in entries if e["id"] == "idle")
        assert idle_entry["release_only"] is True
        assert idle_entry["requires"] == []
        # 全身跳跃/手臂动作（bounce_happy / wave_big）不得出现在可触发目录
        # —— bai 无全身位移与手臂参数，不能叫「蹦跳/挥手」。
        assert "bounce_happy" not in body["actions"]
        assert "wave_big" not in body["actions"]

    def test_catalog_halfbody_channels_from_model_params(self, dev_env, monkeypatch):
        # live2d_model 带 _cdi3_parameter_ids 时，halfbody 通道按参数集推导。
        dev_env.ctx.live2d_model = SimpleNamespace(
            _cdi3_parameter_ids={"ParamBodyAngleX", "ParamEyeBallX", "ParamMouthOpenY"}
        )
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.get("/api/dev/actions/catalog")
        assert r.status_code == 200
        assert isinstance(r.json()["halfbody_channels"], list)

    def test_catalog_capability_filtered_by_loaded_model(self, dev_env):
        # 完整 bai 参数集 → 全部 15 动作 + idle 都进目录；requires 如实下发。
        dev_env.ctx.live2d_model = SimpleNamespace(_cdi3_parameter_ids=set(_BAI_PARAM_IDS))
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        body = dev_env.client.get("/api/dev/actions/catalog").json()
        expected = allowed_actions_for_parameter_ids(_BAI_PARAM_IDS)
        assert set(body["actions"]) == set(expected)
        assert len([a for a in body["actions"] if a != "idle"]) == 15
        meta = {e["id"]: e for e in body["action_meta"]}
        assert sorted(meta["pout"]["requires"]) == ["Param4"]
        assert sorted(meta["wry_mouth"]["requires"]) == ["Param6"]
        assert sorted(meta["look_up"]["requires"]) == ["ParamAngleY", "ParamEyeBallY"]
        # 已删除的 wink id 即使模型带全参数也绝不进目录/富元数据
        assert "wink_left" not in body["actions"]
        assert "wink_right" not in body["actions"]
        assert "wink_left" not in meta and "wink_right" not in meta

    def test_catalog_drops_pout_when_param4_missing(self, dev_env):
        # 模型缺 Param4 → pout 不显示；其余能力门控动作不受影响。
        dev_env.ctx.live2d_model = SimpleNamespace(
            _cdi3_parameter_ids=_BAI_PARAM_IDS - {"Param4"}
        )
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        body = dev_env.client.get("/api/dev/actions/catalog").json()
        assert "pout" not in body["actions"]
        assert "wry_mouth" in body["actions"]
        assert "smile" in body["actions"]


# ─────────────────────────────────────────────
# trigger / release / 未知动作 / 强度
# ─────────────────────────────────────────────
class TestTrigger:
    def test_trigger_legal_action(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post(
            "/api/dev/actions/trigger", json={"action": "nod", "strength": 3}
        )
        assert r.status_code == 200
        body = r.json()
        assert body["ok"] is True
        assert body["action"] == "nod"
        assert body["strength"] == 3
        assert dev_env.ws.broadcasts == [("nod", 3)]

    def test_trigger_release_forces_idle(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post(
            "/api/dev/actions/trigger",
            json={"action": "nod", "release": True},
        )
        assert r.status_code == 200
        assert r.json()["action"] == "idle"
        assert dev_env.ws.broadcasts == [("idle", 2)]

    def test_trigger_idle_explicit_is_treated_as_release(self, dev_env):
        # idle 只能作为 release：显式请求 idle 触发也一律转为释放回中，
        # 不重复触发「回中」动作，避免参数抖动。
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post("/api/dev/actions/trigger", json={"action": "idle"})
        assert r.status_code == 200
        assert r.json()["action"] == "idle"
        assert dev_env.ws.broadcasts == [("idle", 2)]

    def test_trigger_full_body_action_rejected(self, dev_env):
        # 全身跳跃动作（bounce_happy）不在安全半身目录内，必须 400。
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post(
            "/api/dev/actions/trigger", json={"action": "bounce_happy", "strength": 2}
        )
        assert r.status_code == 400
        assert r.json()["error"] == "unknown action"
        assert dev_env.ws.broadcasts == []

    @pytest.mark.parametrize(
        "removed",
        ["happy", "excited", "sway", "bow", "lean_in", "wave_big", "bounce_happy"],
    )
    def test_trigger_removed_ids_rejected_400(self, dev_env, removed):
        # 定版目录收敛：任何已删动作 id 的显式请求一律安全 400，绝不广播。
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post("/api/dev/actions/trigger", json={"action": removed})
        assert r.status_code == 400, removed
        assert r.json()["error"] == "unknown action"
        assert dev_env.ws.broadcasts == []

    @pytest.mark.parametrize("wink", ["wink_left", "wink_right", "WINK_LEFT"])
    def test_trigger_removed_wink_ids_rejected_400(self, dev_env, wink):
        # 用户真机核验后删除的 wink_left / wink_right：旧 id 显式触发一律
        # 400（unknown action），即使当前模型带完整眼参数也绝不广播。
        dev_env.ctx.live2d_model = SimpleNamespace(_cdi3_parameter_ids=set(_BAI_PARAM_IDS))
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post(
            "/api/dev/actions/trigger", json={"action": wink, "strength": 3}
        )
        assert r.status_code == 400, wink
        body = r.json()
        assert body["error"] == "unknown action"
        assert "wink_left" not in body.get("allowed", [])
        assert "wink_right" not in body.get("allowed", [])
        assert dev_env.ws.broadcasts == []

    def test_trigger_unknown_action_400(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post("/api/dev/actions/trigger", json={"action": "no_such_gesture"})
        assert r.status_code == 400
        body = r.json()
        assert body["error"] == "unknown action"
        assert "allowed" in body
        assert dev_env.ws.broadcasts == []

    def test_trigger_capability_gated_action_rejected_without_param4(self, dev_env):
        # 静态合法但模型缺门槛参数（无 Param4 的 pout）→ 拒绝且不广播。
        dev_env.ctx.live2d_model = SimpleNamespace(
            _cdi3_parameter_ids=_BAI_PARAM_IDS - {"Param4"}
        )
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post(
            "/api/dev/actions/trigger", json={"action": "pout", "strength": 2}
        )
        assert r.status_code == 400
        body = r.json()
        assert body["error"] == "action not supported by loaded model"
        assert body["required_params"] == ["Param4"]
        assert dev_env.ws.broadcasts == []

    def test_trigger_capability_gated_action_ok_with_param4(self, dev_env):
        # 完整 bai 参数集 → pout / smile / deep_breath 可正常触发广播。
        dev_env.ctx.live2d_model = SimpleNamespace(_cdi3_parameter_ids=set(_BAI_PARAM_IDS))
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        for action, strength in (("pout", 2), ("wry_mouth", 1), ("deep_breath", 3)):
            r = dev_env.client.post(
                "/api/dev/actions/trigger", json={"action": action, "strength": strength}
            )
            assert r.status_code == 200, (action, r.text)
            assert r.json()["ok"] is True
        assert dev_env.ws.broadcasts == [("pout", 2), ("wry_mouth", 1), ("deep_breath", 3)]

    def test_trigger_base_actions_still_work_without_model_info(self, dev_env):
        # 模型未知（live2d_model=None）→ 无门槛基础动作仍可触发；
        # 能力门控动作（smile 需眉+嘴形）fail-closed 拒绝。
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        assert dev_env.ctx.live2d_model is None
        r = dev_env.client.post("/api/dev/actions/trigger", json={"action": "nod"})
        assert r.status_code == 200
        assert r.json()["ok"] is True
        assert dev_env.ws.broadcasts == [("nod", 2)]

        r2 = dev_env.client.post("/api/dev/actions/trigger", json={"action": "smile"})
        assert r2.status_code == 400
        assert dev_env.ws.broadcasts == [("nod", 2)]

    def test_trigger_strength_out_of_range_coerced_to_2(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post(
            "/api/dev/actions/trigger", json={"action": "nod", "strength": 9}
        )
        assert r.status_code == 200
        assert r.json()["strength"] == 2

    def test_trigger_strength_zero_coerced_to_2(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post(
            "/api/dev/actions/trigger", json={"action": "nod", "strength": 0}
        )
        assert r.status_code == 200
        assert r.json()["strength"] == 2

    def test_trigger_strength_invalid_type_coerced_to_2(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post(
            "/api/dev/actions/trigger", json={"action": "nod", "strength": "boom"}
        )
        assert r.status_code == 200
        assert r.json()["strength"] == 2

    def test_trigger_invalid_json_400(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post("/api/dev/actions/trigger", content="not-json{", headers={"Content-Type": "application/json"})
        assert r.status_code == 400
        assert "invalid json" in r.json()["error"]


# ─────────────────────────────────────────────
# director analyze 脱敏
# ─────────────────────────────────────────────
class TestDirectorAnalyze:
    def _result_with_sensitive_fields(self) -> DirectorResult:
        return DirectorResult(
            emotion="joy",
            strength=3,
            motion_label="happy",
            parameter_overrides={"ParamAngleX": 10.0, "ParamBad": "string-should-drop"},
            half_body={"bodyX": -12.3},
            motion_timeline=[
                {"at": 0.5, "motion": "nod"},
                {"at": 0.7, "motion": "sway"},
            ],
            source="test",
            raw="SECRET prompt with api_key=sk-xxxx and system prompt internals",
        )

    def test_analyze_sanitized_drops_raw_and_credentials(self, dev_env):
        fake_director = _FakeDirector(result=self._result_with_sensitive_fields())
        dev_env.ctx.director = fake_director
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()

        r = dev_env.client.post(
            "/api/dev/director/analyze", json={"text": "说点高兴的事"}
        )
        assert r.status_code == 200
        body = r.json()
        assert fake_director.calls == ["说点高兴的事"]
        # 允许字段存在且值正确；已删 id（happy/sway）被目录白名单清空/丢弃
        assert body["emotion"] == "joy"
        assert body["strength"] == 3
        assert body["motionLabel"] is None
        assert body["halfBody"] == {"bodyX": -12.3}
        assert body["motionTimeline"] == [{"at": 0.5, "motion": "nod"}]
        # 脱敏：raw 绝不返回；非法参数值被钳制剔除
        assert "raw" not in body
        assert "SECRET" not in str(body)
        assert "sk-" not in str(body)
        assert body["parameterOverrides"] == {"ParamAngleX": 10.0}

    def test_analyze_never_echoes_removed_motion_ids(self, dev_env):
        # dev 协议与公开协议同边界：任何已删动作 id 都不得从该端点回显。
        fake_director = _FakeDirector(
            result=DirectorResult(
                emotion="neutral",
                strength=2,
                motion_label="wink_left",
                motion_timeline=[
                    {"at": 0.1, "motion": "happy"},
                    {"at": 0.2, "motion": "bounce_happy"},
                    {"at": 0.3, "motion": "lean_in"},
                    {"at": 0.4, "motion": "wink_right"},
                ],
            )
        )
        dev_env.ctx.director = fake_director
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post("/api/dev/director/analyze", json={"text": "hi"})
        assert r.status_code == 200
        body = r.json()
        assert body["motionLabel"] is None
        assert body["motionTimeline"] == []
        for removed in (
            "happy", "excited", "sway", "bow", "lean_in", "wave_big",
            "bounce_happy", "wink_left", "wink_right",
        ):
            assert removed not in str(body)

    def test_analyze_empty_text_400(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post("/api/dev/director/analyze", json={"text": "   "})
        assert r.status_code == 400
        assert r.json()["error"] == "text required"

    def test_analyze_invalid_json_400(self, dev_env):
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post("/api/dev/director/analyze", content="{oops", headers={"Content-Type": "application/json"})
        assert r.status_code == 400
        assert "invalid json" in r.json()["error"]

    def test_analyze_director_failure_500(self, dev_env):
        fake_director = _FakeDirector(error=RuntimeError("boom"))
        dev_env.ctx.director = fake_director
        dev_env.enable_dev_mode()
        dev_env.enable_localhost()
        r = dev_env.client.post("/api/dev/director/analyze", json={"text": "hi"})
        assert r.status_code == 500
        assert "director failed" in r.json()["error"]


# ─────────────────────────────────────────────
# 环境变量覆盖
# ─────────────────────────────────────────────
class TestEnvOverride:
    def test_dev_mode_env_var_enables_without_config(self, dev_env, monkeypatch):
        monkeypatch.setenv("LIVE2DAI_DEV_MODE", "1")
        dev_env.enable_localhost()
        r = dev_env.client.get("/api/dev/actions/catalog")
        assert r.status_code == 200
        # 环境变量无需改 system_config
        assert dev_env.ctx.system_config.developer_mode is False


# ─────────────────────────────────────────────
# 集成：真实走「保存配置 → 热重载共享 context → dev 门禁」
# 不 monkeypatch _developer_mode_enabled / _is_dev_enabled，而是让
# /api/config 的真实落盘 + apply 重载把 developer_mode 写进共享 context 的
# system_config，再访问 /api/dev/* 验证门禁随之开关。
# ─────────────────────────────────────────────
class _LiveContext:
    """最小复制真实 ServiceContext 行为：apply 时用新 Config 重建 system_config。

    只承载 dev_routes / config_update 实际访问的属性，不初始化任何引擎。
    """

    def __init__(self, system_config):
        self.system_config = system_config
        self.director = None
        self.live2d_model = None
        self.tts_engine = None

    async def load_from_config(self, config):
        # 与 service_context.load_from_config 一致：先重建 system_config，再继续后续。
        self.system_config = config.system_config or self.system_config


class _LiveWS:
    """apply 遍历已连接会话用（无会话即等价于即时热更成功）。"""

    def __init__(self):
        self.client_contexts = {}


def _real_dev_conf_layout(tmp_path):
    """把仓库合法 conf.yaml 复制到隔离目录并返回 config-abs/env/persona 路径。"""
    b = tmp_path / "a" / "b"
    b.mkdir(parents=True, exist_ok=True)
    (tmp_path / "shared").mkdir(parents=True, exist_ok=True)
    conf_path = b / "conf.yaml"
    conf_path.write_text(_repo_conf_text(), encoding="utf-8")
    env_path = tmp_path / ".env"
    env_path.write_text("DEEPSEEK_API_KEY=sk-saved-key\n", encoding="utf-8")
    persona_path = tmp_path / "shared" / "persona.yaml"
    persona_path.write_text(
        "system_prompt: 'You are 白, a cheerful virtual companion.'\n",
        encoding="utf-8",
    )
    return conf_path, env_path, persona_path


def _repo_conf_text() -> str:
    import os

    here = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(here)  # open-llm-vtuber/
    with open(os.path.join(project_root, "conf.yaml"), "r", encoding="utf-8") as f:
        text = f.read()
    # 测试必须是 hermetic：开发者本机 conf.yaml 可能开着 developer_mode，
    # 但这里需要「默认关闭」的确定基线，否则 test_disable_default_gate_403 会因
    # 开发者实时配置(false→true)而误判失败。强制覆盖为 false。
    text = text.replace("developer_mode: true", "developer_mode: false")
    text = text.replace("developer_mode: True", "developer_mode: false")
    return text


@pytest.fixture
def dev_save_env(tmp_path, monkeypatch):
    """隔离的 `config + dev` 双路由应用：共享同一个 live context。

    dev_routes 读 `default_context_cache.system_config`（request 时实时取），
    因此 POST /api/config 的 apply 一旦更新该 context 的 system_config，
    /api/dev/* 门禁应随之开关 —— 这正是用户实测断言的完整链路。
    """
    from fastapi import FastAPI
    from fastapi.testclient import TestClient

    from open_llm_vtuber.config_manager.main import Config as AppConfig
    from open_llm_vtuber.routes import init_config_routes
    from open_llm_vtuber.api.dev_routes import init_dev_routes

    conf_path, env_path, persona_path = _real_dev_conf_layout(tmp_path)

    # 以当前磁盘 conf 构造初始 system_config（与真实启动一致）。
    base = _load_config(conf_path, persona_path)
    fake_ws = _LiveWS()
    ctx = _LiveContext(base.system_config)

    def live_load(config: AppConfig):
        # 闭包把「最新落盘配置」解析后的 system_config 写进 ctx。
        ctx.system_config = config.system_config or ctx.system_config

    app = FastAPI()
    app.include_router(
        init_config_routes(
            config_path=str(conf_path),
            default_context_cache=ctx,
            ws_handler=fake_ws,
        )
    )
    app.include_router(init_dev_routes(default_context_cache=ctx, ws_handler=fake_ws))
    client = TestClient(app)

    # 让 config_update 的 apply 热重载真正更新 ctx.system_config（等价真实 ServiceContext）。
    async def real_load_from_config(config):
        live_load(config)

    ctx.load_from_config = real_load_from_config

    class Env:
        pass

    env = Env()
    env.client = client
    env.ctx = ctx
    env.conf_path = conf_path
    env.read_system_config = lambda: _load_config(conf_path, persona_path).system_config
    return env


def _load_config(conf_path, persona_path):
    from open_llm_vtuber.config_manager.main import Config as AppConfig
    from open_llm_vtuber.services.config_service import ConfigService

    svc = ConfigService(str(conf_path), str(persona_path))
    data = svc.load_yaml()
    # 与 init_config_routes 的 reload 一致：persona 提示词注入后解析。
    data.setdefault("character_config", {})["persona_prompt"] = (
        svc.load_persona().get("system_prompt") or ""
    )
    return AppConfig(**data)


class TestDevModeViaConfigSave:
    def test_disable_default_gate_403(self, dev_save_env):
        r = dev_save_env.client.get("/api/dev/actions/catalog")
        assert r.status_code == 403
        assert r.json()["error"] == "developer mode disabled"

    def test_apply_then_dev300_then_disable_403(self, dev_save_env):
        # 1) 保存并应用 developer_mode=true → 200 applied，共享 context 更新
        r = dev_save_env.client.post(
            "/api/config", json={"developer_mode": True, "apply": True}
        )
        assert r.status_code == 200
        assert r.json()["ok"] is True
        assert r.json()["applied"] is True
        # system_config 已反映到共享 context
        assert dev_save_env.ctx.system_config.developer_mode is True

        # 2) 立即访问 dev catalog → 200（真实走 save→reload→gate，不 monkeypatch）
        cat = dev_save_env.client.get("/api/dev/actions/catalog")
        assert cat.status_code == 200
        assert "nod" in cat.json()["actions"]

        # 3) 保存并应用 developer_mode=false → 200，上下文应立即关闭
        r2 = dev_save_env.client.post(
            "/api/config", json={"developer_mode": False, "apply": True}
        )
        assert r2.status_code == 200
        assert r2.json()["applied"] is True
        assert dev_save_env.ctx.system_config.developer_mode is False
        assert dev_save_env.client.get("/api/dev/actions/catalog").status_code == 403

    def test_failed_save_does_not_change_gate(self, dev_save_env):
        # 先把 dev 打开并 confirmed
        assert dev_save_env.client.post(
            "/api/config", json={"developer_mode": True, "apply": True}
        ).status_code == 200
        assert dev_save_env.client.get("/api/dev/actions/catalog").status_code == 200

        # 保存一个非法配置 → 400；developer_mode 状态与 gate 必须保持不变
        bad = dev_save_env.client.post(
            "/api/config", json={"tts_model": "gpt_sovits_tts", "tts_configs": {"gpt_sovits_tts": {}}}
        )
        assert bad.status_code == 400
        assert dev_save_env.ctx.system_config.developer_mode is True
        assert dev_save_env.client.get("/api/dev/actions/catalog").status_code == 200

    def test_persisted_disk_flag_survives_restart_reload(self, dev_save_env):
        # 保存 true（无 apply，仅落盘）——模拟「已保存但需重启」的跨进程语义。
        assert dev_save_env.client.post(
            "/api/config", json={"developer_mode": True}
        ).status_code == 200
        disk_cfg = dev_save_env.read_system_config()
        assert disk_cfg.developer_mode is True