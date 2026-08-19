import os
import sys
import asyncio

_project_root = os.path.dirname(os.path.abspath(__file__))
os.chdir(_project_root)

sys.path.insert(0, os.path.join(_project_root, "src"))
sys.path.insert(0, _project_root)

import yaml
from open_llm_vtuber.server import WebSocketServer
from open_llm_vtuber.config_manager.main import Config

async def main():
    print("[Live2D-Ai] Starting server...")
    with open("conf.yaml", "r", encoding="utf-8") as f:
        config_dict = yaml.safe_load(f)
    config = Config(**config_dict)
    server = WebSocketServer(config)
    await server.initialize()
    print("[Live2D-Ai] Server initialized OK")
    print("[Live2D-Ai] Press Ctrl+C to stop")

    import uvicorn
    uvicorn_config = uvicorn.Config(
        server.app, host="0.0.0.0", port=12393, log_level="info",
    )
    uvicorn_server = uvicorn.Server(uvicorn_config)
    await uvicorn_server.serve()

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("[Live2D-Ai] Server stopped")
