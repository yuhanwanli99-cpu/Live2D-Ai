# uv 工作流

> 本仓库 PC 端（`Live2D-Ai-pc/open-llm-vtuber`）使用 [uv](https://docs.astral.sh/uv/) 作为 Python 包管理器。

## 环境要求

- Python 3.12（见 `.python-version` 与 `pyproject.toml` 的 `requires-python`）
- uv（CI 使用 `astral-sh/setup-uv@v5`）

## 常用命令

```bash
cd Live2D-Ai-pc/open-llm-vtuber

# 安装依赖（严格按 uv.lock）
uv sync

# 运行服务
uv run run_server.py

# 运行测试
uv run pytest tests/ -q

# 运行 lint
uv run ruff check .

# 更新 lock 文件（修改 pyproject.toml 后）
uv lock

# 导出 requirements.txt
uv export --no-hashes --frozen -o requirements.txt
```

## CI 说明

- `.github/workflows/pr-checks.yml` 在 Python 3.12 下执行 `uv sync && uv run pytest tests/`。
- `ruff check .` 当前设为 `continue-on-error: true`，因为历史代码存在大量 broad-exception 等告警；新代码应保持干净。
- health check 与 cross-platform consistency 同样通过 `uv run` 执行。

## 故障排查

### uv sync 下载 CUDA/torch 包超时

```bash
UV_HTTP_TIMEOUT=600 uv sync
```

### lock 文件冲突

```bash
uv lock --upgrade
# 确认后重新生成 requirements.txt
uv export --no-hashes --frozen -o requirements.txt
```

### 本地没有 uv 二进制

可以先通过 pip/系统包管理器安装 uv，或直接在 CI 中依赖 `setup-uv`。
