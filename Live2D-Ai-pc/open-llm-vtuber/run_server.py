"""Launcher script for Open-LLM-VTuber server (Live2D-Ai slimmed, no-gateway).
- Resolves shared/ paths to the project root shared/ (cross-platform config)
- Pre-loads persona from shared/persona.yaml into persona_prompt
- All API calls go directly to DeepSeek / GLM (no gateway)
"""

import os
import shutil
import sys

import uvicorn
import yaml
from dotenv import load_dotenv

_project_root = os.path.dirname(os.path.abspath(__file__))
os.chdir(_project_root)

# 本地密钥：仓库根 .env（gitignored；不存在则静默跳过）——必须在读取配置前加载。
# load_dotenv 默认找 cwd（open-llm-vtuber/），而 .env 在项目根 → 显式指定绝对路径。
# 使用 override=True：即使父 shell 带有空的环境变量，也以 .env 文件为准。
_PROJECT_BASE = os.path.dirname(
    os.path.dirname(_project_root)
)  # 项目根 Live2D-Ai/（上两级）
load_dotenv(os.path.join(_PROJECT_BASE, ".env"), override=True)

sys.path.insert(0, os.path.join(_project_root, "src"))
sys.path.insert(0, _project_root)

# 统一日志：控制台紧凑彩色（LIVE2DAI_LOG_LEVEL，默认 INFO）+ logs/server.log 滚动文件；
# 同时压制 uvicorn 访问日志刷屏。必须在导入业务模块前安装。
from open_llm_vtuber.logsetup import apply_uvicorn_access_policy, setup_logging

_effective_level = setup_logging()
print(
    f"[Live2D-Ai] 日志级别: {_effective_level}（控制台）+ logs/server.log（滚动文件）"
)

# 版本横幅：产品版本 / 后端包版本 / 前端包版本 / commit（build/启动一次性探测，取不到 unknown）。
# 从 start.sh 启动时已打印过一次并设置 LIVE2DAI_BANNER_SHOWN=1，这里跳过避免重复；
# 直接运行 run_server.py 时无该标记，仍会打印一次。
if os.environ.get("LIVE2DAI_BANNER_SHOWN") != "1":
    from open_llm_vtuber.versions import format_banner

    print(f"[Live2D-Ai] {format_banner()}")

from open_llm_vtuber.config_manager.main import Config
from open_llm_vtuber.config_manager.utils import (
    read_yaml,  # 带 ${VAR} 环境替换的配置读取
)
from open_llm_vtuber.server import WebSocketServer
from open_llm_vtuber.utils.repo_paths import (
    resolve_shared_path as _resolve_shared_path,  # AUDIT P-1 单源
)


def _load_persona_from_config(config_dict: dict) -> str:
    """Load persona system_prompt from shared/persona.yaml if specified.

    人设收敛（2026-08-22）：persona 只承载 system_prompt；模型/温度等参数
    一律来自 conf.yaml 对应 provider 配置，不再从 persona 覆盖。
    """
    cc = config_dict.get("character_config", {})
    persona_file = cc.get("load_persona_from_file", "")
    if not persona_file:
        return cc.get("persona_prompt", "")
    resolved = _resolve_shared_path(persona_file)
    if not os.path.exists(resolved):
        print(f"[WARN] Persona file not found: {resolved}")
        return cc.get("persona_prompt", "")
    try:
        with open(resolved, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f)
        prompt = data.get("system_prompt", "")
        if prompt:
            print(f"[Live2D-Ai] Loaded persona from {resolved}")
        else:
            print(f"[WARN] Persona file has no system_prompt: {resolved}")
        return prompt
    except Exception as e:  # noqa: BLE001
        print(f"[WARN] Failed to load persona from {resolved}: {e}")
        return cc.get("persona_prompt", "")


async def main():
    print(
        "[Live2D-Ai] Starting Open-LLM-VTuber server (direct-API mode, no gateway)..."
    )
    print(f"[Live2D-Ai] Working directory: {_project_root}")

    # 若用户尚未创建 conf.yaml，从公开模板复制一份安全默认配置，确保首次启动即可打开设置页。
    if not os.path.exists("conf.yaml"):
        template_path = "conf.yaml.template"
        if os.path.exists(template_path):
            shutil.copy2(template_path, "conf.yaml")
            print("[Live2D-Ai] Created conf.yaml from conf.yaml.template (safe defaults).")
        else:
            print(
                "[FATAL] conf.yaml not found and conf.yaml.template is also missing. "
                "Please create conf.yaml manually."
            )
            sys.exit(1)

    config_dict = read_yaml(
        "conf.yaml"
    )  # read_yaml 完成 ${DEEPSEEK_API_KEY} 等环境变量替换（yaml.safe_load 不替换）

    # Pre-load persona from shared/persona.yaml (no longer injected by gateway)
    persona = _load_persona_from_config(config_dict)
    if persona:
        config_dict.setdefault("character_config", {})["persona_prompt"] = persona
    else:
        print("[FATAL] Persona prompt is empty! Cannot start without a valid persona.")
        print("[FATAL] Expected file: shared/persona.yaml")
        print(
            "[FATAL] Please ensure the file exists and contains a 'system_prompt' field."
        )
        sys.exit(1)

    # 人设收敛：模型/温度等参数不再从 persona 覆盖（LLM 设置即最终值）

    # MCP runtime is retired in the stable PC build. Legacy top-level
    # mcp_servers data is left untouched and ignored by the config model.

    config = Config(**config_dict)
    server = WebSocketServer(config)
    await server.initialize()

    # 端口取 config.system_config.port（不再写死 12393），使设置里改 port 后按新端口监听（B5）。
    # uvicorn 自身日志级别跟随 LIVE2DAI_LOG_LEVEL（访问日志已由 logsetup 压制刷屏）。
    listen_port = int(config.system_config.port or 12393)
    listen_host = str(config.system_config.host or "127.0.0.1")
    uvicorn_config = uvicorn.Config(
        server.app,
        host=listen_host,
        port=listen_port,
        log_level=_effective_level.lower(),
    )
    uvicorn_server = uvicorn.Server(uvicorn_config)
    # uvicorn 在 Server 初始化时会按 log_level 重置 uvicorn.access，把 setup_logging
    # 的压制冲掉；这里在 server 构造后再施加一次，确保 INFO 下重复轮询 access 不刷屏、
    # 错误 access 与 DEBUG 不受影响。
    apply_uvicorn_access_policy(_effective_level)
    print(f"[Live2D-Ai] Server listening on http://{listen_host}:{listen_port}")
    if listen_host not in {"127.0.0.1", "localhost", "::1"}:
        print(f"[WARN] Server is exposed beyond localhost at ws://{listen_host}:{listen_port}")
    await uvicorn_server.serve()


if __name__ == "__main__":
    from open_llm_vtuber.entrypoint import run_entry

    # Ctrl+C 只输出简洁停止日志并以 130 退出，不打印 KeyboardInterrupt /
    # CancelledError 的 traceback；真实启动异常仍由 Python 打印并返回非零码。
    sys.exit(run_entry(main))
