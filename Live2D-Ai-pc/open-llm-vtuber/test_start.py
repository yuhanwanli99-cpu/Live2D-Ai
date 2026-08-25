import sys, os, asyncio
_project_root = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(_project_root, 'src'))
sys.path.insert(0, _project_root)

try:
    from open_llm_vtuber.server import WebSocketServer
    from open_llm_vtuber.config_manager.main import Config
    print("Imports OK")
except Exception as e:
    print(f"Import error: {e}")
    import traceback
    traceback.print_exc()
