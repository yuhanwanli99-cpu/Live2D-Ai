#!/usr/bin/env bash
# Live2D-Ai — WSL2/Linux 一键启动（PC 端主力入口）
# 访问 http://localhost:12393 （WSL2 localhost 自动转发到 Windows 浏览器）
# 依赖：WSL2 原生 venv，见 Live2D-Ai-pc/README.md「WSL2 环境准备」
set -euo pipefail

ProjectRoot="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RepoRoot="$(cd "$ProjectRoot/.." && pwd)"
VtuberDir="$ProjectRoot/open-llm-vtuber"

# venv 查找优先级：1) 环境变量 LIVE2DAI_WSL_VENV 显式覆盖
#                   2) 仓库内 .venv（项目根 /Live2D-Ai/.venv，本机 WSL2 默认位置）
#                   3) 文档默认 ~/Live2D-Ai/.venv（旧文档路径，兼容兜底）
if [ -n "${LIVE2DAI_WSL_VENV:-}" ]; then
    VenvDir="$LIVE2DAI_WSL_VENV"
elif [ -d "$RepoRoot/.venv" ]; then
    VenvDir="$RepoRoot/.venv"
elif [ -d "$HOME/Live2D-Ai/.venv" ]; then
    VenvDir="$HOME/Live2D-Ai/.venv"
else
    VenvDir="$RepoRoot/.venv"  # 仅用于错误提示
fi
PY="$VenvDir/bin/python"

if [ ! -x "$PY" ]; then
    echo "[Live2D-Ai] 未找到 WSL2 venv 的 Python: $PY"
    echo "[Live2D-Ai] 请先执行 Live2D-Ai-pc/README.md 的「WSL2 环境准备」"
    echo "[Live2D-Ai] 或通过环境变量 LIVE2DAI_WSL_VENV 指定 venv 路径"
    exit 1
fi

if ! "$PY" -c "import open_llm_vtuber, fastapi, edge_tts" >/dev/null 2>&1; then
    echo "[Live2D-Ai] venv 缺少核心依赖，请先安装：" >&2
    echo "[Live2D-Ai]   uv pip install -e '$VtuberDir'" >&2
    exit 1
fi

cd "$VtuberDir"
echo "[Live2D-Ai] 启动后端... http://localhost:12393"

# ---- 代理探测（仅提示，不强加） ----
# WSL2 常用：Windows 侧代理经 host 端口转发到 WSL localhost（默认 10809）。
# 探测顺序：1) 已设置的环境代理原样使用；2) WSL localhost:10809；3) 宿主地址同端口。
# 只探测 10809（用户实际使用的端口），绝不误报 10808。
ProxyPort="${LIVE2DAI_PROXY_PORT:-10809}"
ProxyHint=""
if [ -z "${http_proxy:-}" ] && [ -z "${https_proxy:-}" ]; then
    if (exec 3<>"/dev/tcp/127.0.0.1/$ProxyPort") 2>/dev/null; then
        exec 3>&- 3<&- 2>/dev/null || true
        ProxyHint="http://127.0.0.1:$ProxyPort"
    else
        # 回退：WSL2 宿主地址来自 /etc/resolv.conf nameserver（Windows 主网卡 IP）。
        HostAddr="$(awk '/^nameserver/{print $2; exit}' /etc/resolv.conf 2>/dev/null)"
        if [ -n "$HostAddr" ] && (exec 3<>"/dev/tcp/$HostAddr/$ProxyPort") 2>/dev/null; then
            exec 3>&- 3<&- 2>/dev/null || true
            ProxyHint="http://$HostAddr:$ProxyPort"
        fi
    fi
fi
if [ -n "$ProxyHint" ]; then
    echo "[Live2D-Ai] 检测到本机代理: $ProxyHint（可用 export https_proxy=... 显式覆盖）"
fi

# 版本横幅：产品版本 / 后端包版本 / 前端包版本 / commit（取不到显示 unknown）
if Banner="$(cd "$VtuberDir" && "$PY" -c \
  'from open_llm_vtuber.versions import format_banner; print(format_banner())' 2>/dev/null)"; then
    echo "[Live2D-Ai] $Banner"
fi

# 由 start.sh 打印过一次版本横幅；run_server.py 检测到该标记则跳过，避免重复打印。
# （直接运行 run_server.py 时无此标记，仍会打印一次。）
export LIVE2DAI_BANNER_SHOWN=1

exec "$PY" run_server.py
