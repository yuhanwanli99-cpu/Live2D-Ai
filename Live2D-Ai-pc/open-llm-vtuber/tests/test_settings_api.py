"""设置中心 API 测试（spec docs/plans/settings-redesign-spec.md §1 / §3.2）。

被测目标：`POST/GET /api/config`、`/api/config/tree`、`/health`、
`/api/llm/discover-models`、`/api/providers/check`、`/api/test-llm`、`/api/test-tts`。

原则：
  - 全部走隔离的 `settings_env` fixture（tmp_path 里的 conf.yaml/.env/persona.yaml + mock
    共享 ServiceContext / WSHandler），绝不触碰仓库真实 conf.yaml/.env。
  - 外部 LLM/TTS 调用一律 mock（monkeypatch `requests` / `TTSFactory`），不发真实网络。
  - 断言贴合真实实现（apply 返回形状、discover/check/test 的 saved∪UI 语义与协议），
    不凭空造假失败。
"""
import copy

import pytest


# ─────────────────────────────────────────────────────────────
# GET /health
# ─────────────────────────────────────────────────────────────
class TestHealth:
    def test_health_200_with_status_version_uptime(self, settings_env):
        r = settings_env.client.get("/health")
        assert r.status_code == 200
        body = r.json()
        assert body["status"] == "ok"
        assert isinstance(body["version"], str) and body["version"]
        assert isinstance(body["uptime_s"], int)
        assert body["uptime_s"] >= 0


# ─────────────────────────────────────────────────────────────
# GET /api/config —— normalize 视图
# ─────────────────────────────────────────────────────────────
class TestGetConfig:
    def test_get_config_normalized_view(self, settings_env):
        r = settings_env.client.get("/api/config")
        assert r.status_code == 200
        body = r.json()
        # normalize 视图的关键字段都存在
        assert isinstance(body["llm_configs"], list)
        assert isinstance(body["tts_configs"], list)
        assert "log_level" in body
        assert "port" in body
        assert "persona" in body and isinstance(body["persona"], dict)
        assert "llm_provider" in body
        # 每个 provider 都带 has_key / configured（spec 1.2）
        for cfg in body["llm_configs"]:
            assert "has_key" in cfg
            assert "configured" in cfg
        for cfg in body["tts_configs"]:
            assert "has_key" in cfg
            assert "configured" in cfg

    def test_get_config_log_level_default_info(self, settings_env):
        body = settings_env.client.get("/api/config").json()
        assert body["log_level"] == "INFO"


# ─────────────────────────────────────────────────────────────
# POST /api/config —— 成功 / re-populate 契约
# ─────────────────────────────────────────────────────────────
class TestPostConfig:
    def test_post_config_ok_and_config_isomorphic_with_get(self, settings_env):
        # 改 port + persona，POST 成功后 GET 应与响应 config 同构（前端据此 re-populate）
        # 人设收敛：persona 仅承载 system_prompt
        r = settings_env.client.post(
            "/api/config",
            json={"port": 16000, "persona": {"system_prompt": "你是白。"}},
        )
        assert r.status_code == 200
        body = r.json()
        assert body["ok"] is True
        # 改动已反映到落盘后的 normalize 视图
        assert body["config"]["port"] == 16000
        assert body["config"]["persona"]["system_prompt"] == "你是白。"

        # re-populate 契约：POST 响应的 config 与随后 GET 完全一致
        get_body = settings_env.client.get("/api/config")
        assert get_body.status_code == 200
        assert get_body.json() == body["config"]

    def test_post_config_does_not_touch_env_without_api_keys(self, settings_env):
        env_before = settings_env.read_env()
        r = settings_env.client.post("/api/config", json={"port": 16001})
        assert r.status_code == 200
        assert settings_env.read_env() == env_before  # 没传 api_keys/log_level 不重写 .env

    def test_post_config_llm_provider_auto_fill_configured(self, settings_env):
        # 仅选中一个尚未配置的 provider：normalize 会按目录默认值自动补齐
        # base_url/model 并落盘（configured 变 True），校验通过。
        r = settings_env.client.post(
            "/api/config",
            json={"llm_provider": "openai"},
        )
        assert r.status_code == 200
        body = r.json()["config"]
        assert body["llm_provider"] == "openai"
        openai_cfg = next(x for x in body["llm_configs"] if x["id"] == "openai")
        assert openai_cfg["configured"] is True
        assert openai_cfg["model"]  # 由目录补齐，非空
        assert openai_cfg["base_url"]  # 由目录补齐，非空

        # re-populate：GET 与 POST 响应一致
        assert settings_env.client.get("/api/config").json() == body

    def test_post_config_llm_config_override_with_base_url(self, settings_env):
        # 给新 provider 显式带 base_url 的覆盖也可保存成功
        r = settings_env.client.post(
            "/api/config",
            json={
                "llm_provider": "openai",
                "llm_configs": {"openai": {"model": "gpt-5", "base_url": "https://api.openai.com/v1"}},
            },
        )
        assert r.status_code == 200
        body = r.json()["config"]
        openai_cfg = next(x for x in body["llm_configs"] if x["id"] == "openai")
        assert openai_cfg["model"] == "gpt-5"
        assert openai_cfg["configured"] is True


# ─────────────────────────────────────────────────────────────
# 校验失败不落盘
# ─────────────────────────────────────────────────────────────
class TestValidationRollback:
    def test_post_config_tts_selected_missing_required_returns_400_and_no_write(
        self, settings_env
    ):
        conf_before = settings_env.read_conf()
        # gpt_sovits_tts 的 catalog 补不出 voice，且 api_url/text_lang 等必填缺失
        # → AppConfig 校验失败 → 400，conf.yaml 必须未被改写（spec 1.3 步骤2/3）。
        r = settings_env.client.post(
            "/api/config",
            json={"tts_model": "gpt_sovits_tts", "tts_configs": {"gpt_sovits_tts": {}}},
        )
        assert r.status_code == 400
        body = r.json()
        assert body["ok"] is False
        assert "field_errors" in body
        assert settings_env.read_conf() == conf_before

    def test_post_config_tree_list_top_invalid_400_no_write(self, settings_env):
        conf_before = settings_env.read_conf()
        r = settings_env.client.post(
            "/api/config/tree", json={"yaml": "- a\n- b\n"}
        )
        assert r.status_code == 400
        assert "顶层必须是键值映射" in r.json()["error"]
        assert settings_env.read_conf() == conf_before

    def test_post_config_tree_bad_schema_400_no_write(self, settings_env):
        conf_before = settings_env.read_conf()
        # 合法 YAML 但结构损坏（缺必要字段）→ schema 校验失败 → 400 不落盘（spec 1.4）
        r = settings_env.client.post(
            "/api/config/tree", json={"yaml": "system_config:\n  port: 1\n"}
        )
        assert r.status_code == 400
        assert r.json()["ok"] is False
        assert settings_env.read_conf() == conf_before

    def test_post_config_tree_valid_writes_and_returns_isomorphic(self, settings_env):
        # 用当前 conf 文本写回（合法）→ 200 ok:true，返回 config 同构
        r = settings_env.client.post(
            "/api/config/tree", json={"yaml": settings_env.read_conf()}
        )
        assert r.status_code == 200
        assert r.json()["ok"] is True
        assert "config" in r.json() and "port" in r.json()["config"]


# ─────────────────────────────────────────────────────────────
# 显式清空（null）
# ─────────────────────────────────────────────────────────────
class TestExplicitClear:
    def test_persona_system_prompt_cannot_be_cleared(self, settings_env):
        # 人设收敛：system_prompt 唯一且不可为空（清空=400，文件不动）
        marker = "to-be-cleared-prompt"
        r = settings_env.client.post(
            "/api/config", json={"persona": {"system_prompt": marker}}
        )
        assert r.status_code == 200

        # null / 空串 / clear 标记 三种清空姿势全部拒绝
        for payload in (
            {"persona": {"system_prompt": None}},
            {"persona": {"system_prompt": ""}},
            {"persona": {"system_prompt": "abc", "clear": {"system_prompt": True}}},
        ):
            r = settings_env.client.post("/api/config", json=payload)
            assert r.status_code == 400, f"{payload} 应被拒绝"
            assert "System Prompt 不能为空" in r.json()["error"]

        # 文件未被改动，GET 视图仍是原值
        assert marker in settings_env.read_persona()
        body = settings_env.client.get("/api/config").json()
        assert body["persona"]["system_prompt"] == marker

    def test_env_key_null_removes_from_env_file(self, settings_env):
        assert "DEEPSEEK_API_KEY=" in settings_env.read_env()
        r = settings_env.client.post(
            "/api/config", json={"api_keys": {"DEEPSEEK_API_KEY": None}}
        )
        assert r.status_code == 200
        env_after = settings_env.read_env()
        assert "DEEPSEEK_API_KEY=" not in env_after  # 该 key 原行被移除
        assert "LIVE2DAI_LOG_LEVEL=" in env_after  # 未清空的行保留


# ─────────────────────────────────────────────────────────────
# log_level round-trip（spec 1.7）
# ─────────────────────────────────────────────────────────────
class TestLogLevelRoundTrip:
    def test_log_level_round_trip(self, settings_env):
        r = settings_env.client.post(
            "/api/config", json={"log_level": "DEBUG"}
        )
        assert r.status_code == 200
        assert settings_env.client.get("/api/config").json()["log_level"] == "DEBUG"

    def test_log_level_kept_when_not_submitted(self, settings_env):
        # 改成 DEBUG 后，后续保存若不传 log_level，不应被重置回 INFO（spec 1.7 / 修复 A1）
        assert settings_env.client.post(
            "/api/config", json={"log_level": "DEBUG"}
        ).status_code == 200
        r = settings_env.client.post("/api/config", json={"port": 17000})
        assert r.status_code == 200
        assert settings_env.client.get("/api/config").json()["log_level"] == "DEBUG"
        assert settings_env.client.get("/api/config").json()["port"] == 17000


# ─────────────────────────────────────────────────────────────
# apply=true（不吹嘘 applied，requires_restart 诚实）
# ─────────────────────────────────────────────────────────────
class TestApply:
    def test_apply_no_connected_session_applied(self, settings_env):
        # 无已连接会话，共享 cache reload 成功
        body = copy.deepcopy(settings_env.client.get("/api/config").json())
        r = settings_env.client.post("/api/config", json={"apply": True})
        assert r.status_code == 200
        resp = r.json()
        assert resp["ok"] is True
        # 诚实字段存在
        assert "applied" in resp
        assert "requires_restart" in resp
        assert settings_env.fake_ctx.loaded  # 共享 cache 确实收到过一次 load
        get_body = copy.deepcopy(settings_env.client.get("/api/config").json())
        assert body == get_body  # 未改任何设置，仅 apply

    def test_apply_connected_session_failed_reload_requires_restart(self, settings_env):
        # 命中“已连接会话无法即时热更”分支 → requires_restart:true / applied:false（诚实）
        class FailingSession:
            async def load_from_config(self, config):
                raise RuntimeError("session busy")

        settings_env.fake_ws.client_contexts["sess-1"] = FailingSession()
        r = settings_env.client.post("/api/config", json={"apply": True})
        assert r.status_code == 200
        resp = r.json()
        assert resp["ok"] is True
        assert resp["applied"] is False
        assert resp["requires_restart"] is True

    def test_apply_connected_session_success(self, settings_env):
        class GoodSession:
            def __init__(self):
                self.loaded = []

            async def load_from_config(self, config):
                self.loaded.append(config)

        settings_env.fake_ws.client_contexts["sess-1"] = GoodSession()
        r = settings_env.client.post("/api/config", json={"apply": True})
        assert r.status_code == 200
        resp = r.json()
        assert resp["ok"] is True
        assert resp["applied"] is True
        assert resp["requires_restart"] is False

    def test_apply_shared_cache_load_failure_requires_restart(self, settings_env):
        # 共享 cache reload 抛错 → 同样诚实返回 requires_restart
        settings_env.fake_ctx.fail_apply = True
        r = settings_env.client.post("/api/config", json={"apply": True})
        assert r.status_code == 200
        resp = r.json()
        assert resp["ok"] is True
        assert resp["applied"] is False
        assert resp["requires_restart"] is True


# ─────────────────────────────────────────────────────────────
# check / discover / test —— saved∪UI 合成 + 协议感知（mock 外呼）
# ─────────────────────────────────────────────────────────────
class TestOutboundProtocols:
    @pytest.fixture(autouse=True)
    def _mock_requests(self, monkeypatch):
        """把 routes 里 `import requests as _requests` 对应的 requests 模块打桩。"""
        import requests as _req

        self._captured = []  # (method, url, headers, json)

        class FakeResp:
            def __init__(self, data, status=200, text=""):
                self._data = data
                self.status_code = status
                self.text = text or str(data)

            def json(self):
                return self._data

        def fake_get(url, headers=None, timeout=None):
            self._captured.append(("GET", url, dict(headers or {}), None))
            if url.endswith("/api/tags"):
                return FakeResp(
                    {"models": [{"name": "llama3:latest"}, {"name": "qwen:7b"}]}
                )
            return FakeResp({"data": [{"id": "gpt-5"}, {"id": "gpt-4o"}]})

        def fake_post(url, headers=None, json=None, timeout=None):
            self._captured.append(("POST", url, dict(headers or {}), json or {}))
            return FakeResp({"choices": [{"message": {"content": "ok"}}]})

        monkeypatch.setattr(_req, "get", fake_get)
        monkeypatch.setattr(_req, "post", fake_post)

    def _last(self):
        return self._captured[-1]

    # discover 协议感知
    def test_discover_openai_protocol(self, settings_env):
        r = settings_env.client.post(
            "/api/llm/discover-models",
            json={"provider": "openai", "base_url": "https://api.openai.com/v1", "api_key": "ui-key"},
        )
        assert r.status_code == 200
        assert r.json()["ok"] is True
        assert "gpt-5" in r.json()["models"]
        method, url, headers, _ = self._last()
        assert method == "GET"
        assert url == "https://api.openai.com/v1/models"  # OpenAI 形态 /models
        assert headers["Authorization"] == "Bearer ui-key"

    def test_discover_anthropic_protocol(self, settings_env):
        r = settings_env.client.post(
            "/api/llm/discover-models",
            json={"provider": "anthropic", "base_url": "https://api.anthropic.com/v1", "api_key": "ak"},
        )
        assert r.status_code == 200
        _, url, headers, _ = self._last()
        assert url == "https://api.anthropic.com/v1/models"
        assert headers["x-api-key"] == "ak"
        assert headers["anthropic-version"]
        # 响应走 OpenAI 解析夹具，但端点必须是对（非全 OpenAI）
        assert r.json()["ok"] is True

    def test_discover_ollama_protocol(self, settings_env):
        r = settings_env.client.post(
            "/api/llm/discover-models",
            json={"provider": "ollama", "base_url": "http://localhost:11434"},
        )
        assert r.status_code == 200
        _, url, _, _ = self._last()
        assert url == "http://localhost:11434/api/tags"  # Ollama 用 /api/tags
        assert r.json()["models"] == ["llama3:latest", "qwen:7b"]

    def test_discover_uses_saved_env_key_when_ui_key_missing(self, settings_env):
        # saved：.env 有 DEEPSEEK_API_KEY；UI 只给 provider+base_url → key 回退到环境
        r = settings_env.client.post(
            "/api/llm/discover-models",
            json={"provider": "deepseek_llm", "base_url": "https://api.deepseek.com/v1"},
        )
        assert r.status_code == 200
        _, url, headers, _ = self._last()
        assert url == "https://api.deepseek.com/v1/models"
        assert headers["Authorization"] == "Bearer sk-saved-key"

    # test-llm 协议感知（端点是具体协议，而非一律 OpenAI）
    def test_llm_test_anthropic_messages_payload(self, settings_env):
        r = settings_env.client.post(
            "/api/test-llm",
            json={
                "provider": "anthropic",
                "base_url": "https://api.anthropic.com/v1",
                "model": "claude-sonnet-4-5",
                "api_key": "ak",
            },
        )
        assert r.status_code == 200
        assert r.json()["ok"] is True
        method, url, headers, payload = self._last()
        assert method == "POST"
        assert url == "https://api.anthropic.com/v1/messages"
        assert headers["x-api-key"] == "ak"
        assert payload["model"] == "claude-sonnet-4-5"
        assert payload["messages"][0]["content"] == "ping"

    def test_llm_test_openai_chat_completions_payload(self, settings_env):
        r = settings_env.client.post(
            "/api/test-llm",
            json={
                "provider": "openai",
                "base_url": "https://api.openai.com/v1",
                "model": "gpt-5",
                "api_key": "ui-key",
            },
        )
        assert r.status_code == 200
        _, url, headers, payload = self._last()
        assert url == "https://api.openai.com/v1/chat/completions"
        assert headers["Authorization"] == "Bearer ui-key"
        assert payload["model"] == "gpt-5"

    def test_llm_test_ollama_chat_payload(self, settings_env):
        r = settings_env.client.post(
            "/api/test-llm",
            json={"provider": "ollama", "base_url": "http://localhost:11434", "model": "qwen:7b"},
        )
        assert r.status_code == 200
        method, url, _, payload = self._last()
        assert method == "POST"
        assert url == "http://localhost:11434/api/chat"
        assert payload["model"] == "qwen:7b"
        assert payload["stream"] is False

    def test_llm_test_uses_saved_model_when_ui_omits(self, settings_env):
        # saved∪UI：UI 给 provider+key，model/base_url 回退到已保存 deepseek 配置
        r = settings_env.client.post(
            "/api/test-llm",
            json={
                "provider": "deepseek_llm",
                "api_key": "ui-key",
                # 故意不给 base_url / model —— 应使用 conf 里已保存的
            },
        )
        assert r.status_code == 200
        _, url, _, payload = self._last()
        assert url == "https://api.deepseek.com/v1/chat/completions"  # 已保存 base_url
        assert payload["model"] == "deepseek-v4-flash-vision-exp"  # 已保存 model（默认带视觉）

    # providers/check：saved∪UI 合成（saved key + UI base_url 覆盖）
    def test_providers_check_saved_key_and_ui_base_url(self, settings_env):
        r = settings_env.client.post(
            "/api/providers/check",
            json={
                "llm_configs": {
                    "deepseek_llm": {"base_url": "https://ui.example.com/v1"}
                }
            },
        )
        assert r.status_code == 200
        body = r.json()
        assert "llm" in body and "tts" in body
        # UI base_url 覆盖已保存 base_url；key 回退到 .env 的 sk-saved-key
        call = next(
            (u, h)
            for (m, u, h, _) in self._captured
            if u == "https://ui.example.com/v1/models"
        )
        url, headers = call
        assert url == "https://ui.example.com/v1/models"
        assert headers["Authorization"] == "Bearer sk-saved-key"

    def test_providers_check_honest_unconfigured(self, settings_env):
        # 未配 key 的 provider → ok:false + 状态说明（不吞成 400）
        r = settings_env.client.post("/api/providers/check", json={})
        assert r.status_code == 200
        body = r.json()
        # .env 只配了 DEEPSEEK_API_KEY；zhipu 未配 key（无 UI）→ 未配置 Key
        zhipu = next(x for x in body["llm"] if x["provider"] == "zhipu_llm")
        assert zhipu["ok"] is False
        assert zhipu["status"] != "ok"


# ─────────────────────────────────────────────────────────────
# test-tts（mock TTSFactory，验证 saved∪UI 参数合成）
# ─────────────────────────────────────────────────────────────
class TestTtsCheckProtocol:
    @pytest.fixture(autouse=True)
    def _mock_tts_factory(self, monkeypatch):
        class FakeEngine:
            async def async_generate_audio(self, text, file_name_no_ext):
                self.text = text
                self.file_name = file_name_no_ext
                return "cache/settings_preview.mp3"

        captured = {}

        def fake_factory(provider, **params):
            captured["provider"] = provider
            captured["params"] = params
            return FakeEngine()

        from open_llm_vtuber.tts import tts_factory

        monkeypatch.setattr(
            tts_factory.TTSFactory, "get_tts_engine", staticmethod(fake_factory)
        )
        self._captured_tts = captured

    def test_tts_check_merges_saved_and_ui(self, settings_env):
        # saved：conf 里 cosyvoice_cloud 有 ws_url/model；UI 只覆盖 voice
        r = settings_env.client.post(
            "/api/test-tts", json={"provider": "cosyvoice_cloud", "voice": "longshu"}
        )
        assert r.status_code == 200
        assert r.json()["ok"] is True
        assert self._captured_tts["provider"] == "cosyvoice_cloud"
        params = self._captured_tts["params"]
        assert params["voice"] == "longshu"  # UI 覆盖
        assert params["ws_url"]  # 已保存字段保留
        assert params["model"]  # 已保存字段保留
