"""pytest 共享配置：保证 `open_llm_vtuber` 包可导入，并提供设置中心 API 测试用的临时环境。

项目根（open-llm-vtuber/）下源码位于 `src/`，沿用项目自身
`sys.path.insert(0, 'src')` 约定（见 test_server.py）。
此处把 `src/` 提前注入 sys.path，使测试内 `from open_llm_vtuber...`
能解析到待测实现，无需手动改 sys.path。

同时把项目根（open-llm-vtuber/）注入 sys.path —— `open_llm_vtuber.service_context`
模块在加载时会 `from prompts import prompt_loader`，而 `prompts/` 位于项目根（cwd）。
`init_config_routes` → `routes` → `service_context` 的导入链需要项目根可见。
（不破坏现有导入：此注入仅是加到 sys.path 前部。）

本 conftest 属于测试环境搭建，不触碰任何实现逻辑。
"""
import os
import sys
import shutil

import pytest

_ROOT_DIR = os.path.dirname(os.path.abspath(__file__))          # <root>/open-llm-vtuber/tests
_PROJECT_ROOT = os.path.dirname(_ROOT_DIR)                       # <root>/open-llm-vtuber
_SRC_DIR = os.path.join(_PROJECT_ROOT, "src")

for _p in (_SRC_DIR, _PROJECT_ROOT):
    if _p not in sys.path:
        sys.path.insert(0, _p)

# 让 `prompts`、`characters` 等项目根包可解析（服务上下文导入链需要）
os.environ.setdefault("PYTHONPATH", _PROJECT_ROOT)


class _SettingsEnv:
    """tmp_path 隔离的一套设置中心环境（conf.yaml / .env / shared/persona.yaml + 应用）。"""

    def __init__(self, client, conf_path, env_path, persona_path, fake_ctx, fake_ws):
        self.client = client
        self.conf_path = conf_path
        self.env_path = env_path
        self.persona_path = persona_path
        self.fake_ctx = fake_ctx
        self.fake_ws = fake_ws

    def read_conf(self) -> str:
        return self.conf_path.read_text(encoding="utf-8")

    def read_env(self) -> str:
        return self.env_path.read_text(encoding="utf-8") if self.env_path.exists() else ""

    def read_persona(self) -> str:
        return self.persona_path.read_text(encoding="utf-8") if self.persona_path.exists() else ""


class _FakeContext:
    """mock 共享 ServiceContext：apply=true 时走到 `load_from_config`。

    与真实 `ServiceContext` 相似地暴露 `tts_engine`（text-tts 无 provider 分支会访问）。
    connected/load_from_config 行为可在具体测试中覆写。
    """

    def __init__(self):
        self.tts_engine = None
        self.loaded = []  # 记录 apply 收到过的 Config dict
        self.fail_apply = False

    async def load_from_config(self, config):
        if self.fail_apply:
            raise RuntimeError("simulated apply failure")
        self.loaded.append(config)


class _FakeWSHandler:
    """mock WebSocketHandler：承载 `client_contexts` 以支持 apply 逐会话热更遍历。"""

    def __init__(self):
        self.client_contexts = {}


def _write_env_layout(tmp_path, conf_text, env_text, persona_text):
    """构造 `init_config_routes` 期望的目录布局。

    `init_config_routes(config_path)` 会把 repo_root 计算为 config 文件往上三级：
        repo_root = dirname(dirname(dirname(abspath(config_path))))
    因此把 conf.yaml 放在 tmp_path/a/b/conf.yaml 时 repo_root == tmp_path，
    `.env` 落在 tmp_path/.env、persona 落在 tmp_path/shared/persona.yaml，
    全部落在 pytest 临时目录内，互不污染、失败也不影响真实仓库文件。
    """
    b = tmp_path / "a" / "b"
    b.mkdir(parents=True, exist_ok=True)
    (tmp_path / "shared").mkdir(parents=True, exist_ok=True)
    conf_path = b / "conf.yaml"
    conf_path.write_text(conf_text, encoding="utf-8")
    env_path = tmp_path / ".env"
    env_path.write_text(env_text, encoding="utf-8")
    persona_path = tmp_path / "shared" / "persona.yaml"
    persona_path.write_text(persona_text, encoding="utf-8")
    return conf_path, env_path, persona_path


_CONF_TEMPLATE_PATH = os.path.join(_PROJECT_ROOT, "conf.yaml")
_PERSONA_DEFAULT = (
    "name: bai\n"
    "role: '白'\n"
    "system_prompt: 'You are 白, a cheerful virtual companion.'\n"
    "model: deepseek-v4-flash\n"
    "temperature: 0.8\n"
)
_ENV_DEFAULT = "DEEPSEEK_API_KEY=sk-saved-key\nLIVE2DAI_LOG_LEVEL=INFO\n"


def _base_conf_text() -> str:
    """以仓库现有 conf.yaml 为合法底座（保证 AppConfig 校验通过）。"""
    with open(_CONF_TEMPLATE_PATH, "r", encoding="utf-8") as f:
        return f.read()


@pytest.fixture
def settings_env(tmp_path):
    """构造一个隔离的设置中心 FastAPI 应用（TestClient）。

    - conf.yaml 复制自仓库现有合法配置（避免手造 schema 出错且可复检）。
    - `.env` / `shared/persona.yaml` 用确定内容写进 tmp_path。
    - mock 共享 ServiceContext 与 WebSocketHandler，绝不发真实网络/引擎调用。
    """
    from fastapi import FastAPI
    from fastapi.testclient import TestClient

    from open_llm_vtuber.routes import init_config_routes

    conf_path, env_path, persona_path = _write_env_layout(
        tmp_path, _base_conf_text(), _ENV_DEFAULT, _PERSONA_DEFAULT
    )

    fake_ctx = _FakeContext()
    fake_ws = _FakeWSHandler()

    app = FastAPI()
    app.include_router(
        init_config_routes(
            config_path=str(conf_path),
            default_context_cache=fake_ctx,
            ws_handler=fake_ws,
        )
    )
    client = TestClient(app)
    return _SettingsEnv(client, conf_path, env_path, persona_path, fake_ctx, fake_ws)
