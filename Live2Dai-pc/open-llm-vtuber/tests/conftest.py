"""pytest 共享配置：保证 `open_llm_vtuber` 包可导入。

项目根（open-llm-vtuber/）下源码位于 `src/`，沿用项目自身
`sys.path.insert(0, 'src')` 约定（见 test_server.py）。
此处把 `src/` 提前注入 sys.path，使测试内 `from open_llm_vtuber...`
能解析到待测实现，无需手动改 sys.path。

本 conftest 属于测试环境搭建，不触碰任何实现逻辑。
"""
import os
import sys

_SRC_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src")
_SRC_DIR = os.path.normpath(_SRC_DIR)

if _SRC_DIR not in sys.path:
    sys.path.insert(0, _SRC_DIR)
