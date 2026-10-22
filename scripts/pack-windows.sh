#!/usr/bin/env bash
# 0.4.0 Windows 包入口（docs/versions/0.4.md「七个库怎么进包」）。
#
# 流程体在 scripts/pack_windows_glue.py（必要辅助文件）：临时 worktree、兄弟仓库
# git archive 导出、五步链 glue、cargo/flutter/trunk 验证与 dist/windows 组装都在
# 那边；这里只做解释器定位与 exec，避免 bash 里再嵌一层引号地狱。
#
# 用法 / 前置条件 / 产物清单：./scripts/pack-windows.sh --help
set -euo pipefail
here=$(cd -- "$(dirname -- "$0")" && pwd)
exec python3 "$here/pack_windows_glue.py" "$@"
