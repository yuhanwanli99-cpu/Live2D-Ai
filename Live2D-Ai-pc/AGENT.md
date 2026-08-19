# Live2D-Ai — PC 端（WSL2/Linux 主力）

> 基于 [Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber) 的 Live2D 前端，运行于 **WSL2/Linux**
> 当前架构: 直连 DeepSeek API（无中间 Gateway）
> 平台定位（2026-08-19）：**WSL2/Linux 主力，原生 Windows 已放弃**（无 Windows 启动入口）

---

## 架构

```
  ┌─────────────────────────────────────┐
  │  Live2D-Ai-pc/                       │
  │  ├── start.sh         ← 一键启动脚本（WSL2 主力）  │
  │  └── open-llm-vtuber/ ← 上游项目    │
  │      ├── conf.yaml   ← 配置         │
  │      ├── run_server.py              │
  │      ├── frontend/   ← Web 前端     │
  │      └── src/        ← Python 后端  │
  └─────────────────────────────────────┘
                                          │
                    ┌─────────────────────┤
                    ▼                     ▼
          DeepSeek API           Zhipu GLM-4V
          (聊天)                  (视觉)
```

---

## 关键文件

| 文件 | 说明 |
| ------ | ------ |
| `conf.yaml` | **主配置** — LLM、人设、TTS、ASR 等（~94行精简版） |
| `start.sh` | 一键启动脚本（WSL2，自动自检核心依赖、启动后端） |
| `run_server.py` | Python 后端入口 (FastAPI + WebSocket) |
| `shared/persona.yaml` | **人设单源真理**（在根目录 shared/ 下） |

---

## conf.yaml 关键配置

```yaml
# 直连 DeepSeek API
llm_configs:
  deepseek_llm:
    base_url: 'https://api.deepseek.com/v1'
    llm_api_key: 'sk-你的Key'
    model: 'deepseek-v4-flash'

# 人设从 shared/persona.yaml 自动加载
character_config:
  load_persona_from_file: 'shared/persona.yaml'
```

> **重要**: 人设由 `run_server.py` 自动从 `../../shared/persona.yaml` 加载并覆盖配置。
> 编辑 `shared/persona.yaml` 即可，无需改 conf.yaml。

---

## 启动方式（WSL2 主力）

```bash
# 方式 1：一键启动（主力入口，含核心依赖自检）
cd ~/Live2D-Ai/Live2D-Ai-pc
./start.sh

# 方式 2：指定 venv 路径后启动
LIVE2DAI_WSL_VENV=/path/to/.venv ./start.sh

# 方式 3：直接启动 Python 后端（调试）
cd ~/Live2D-Ai/Live2D-Ai-pc/open-llm-vtuber
~/Live2D-Ai/.venv/bin/python run_server.py
```

> 原生 Windows 启动方式已放弃，不再提供 `start.ps1` / `启动Live2D-ai.bat`。

启动后浏览器访问 **<http://localhost:12393/>** 即可使用。

---

## 配置流

```
                         ┌──────────────────┐
shared/persona.yaml ────→│  run_server.py   │──→ 覆盖 LLM model/temperature
                         │  _resolve_shared │
shared/mcp_tools.json ───→│  _path()        │──→ MCP 工具注册
                         └──────────────────┘
```

---

## 关键依赖

| 组件 | 依赖 | 说明 |
| ------ | ------ | ------ |
| Python 运行时 | 3.10+ | .venv 虚拟环境 |
| LLM | DeepSeek API Key | conf.yaml 配置 |
| 视觉 | Zhipu API Key | conf.yaml 配置 |
| TTS | Edge TTS | 内置于 .venv |
| ASR | Faster Whisper | 首次加载需数十秒 |
| ffmpeg | 系统安装 | 可选，ASR 需要 |
| MCP | uvx | 可选，工具功能需要 |

---

## 已知限制

1. **人设同步**：`conf.yaml` 仍内联了人设 prompt 作为 fallback，与 `shared/persona.yaml` 存在不同步风险
2. **ffmpeg 可选**：未安装时 ASR 不可用，但对话功能不受影响
3. **Electron 可选**：可通过浏览器直接访问 Web 前端，不依赖 Electron 桌面客户端

---

## 调试

```bash
# 启动后检查
curl http://localhost:12393/health

# 查看日志
cd ~/Live2D-Ai/Live2D-Ai-pc/open-llm-vtuber
~/Live2D-Ai/.venv/bin/python run_server.py  # 前台运行看输出
```
