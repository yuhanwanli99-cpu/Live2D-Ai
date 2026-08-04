# Live2Dai — Windows 桌面端

> 基于 [Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber) 的 Windows 桌宠前端
> 当前架构: 直连 DeepSeek API（无中间 Gateway）

---

## 架构

```
  ┌─────────────────────────────────────┐
  │  Live2Dai-pc/                       │
  │  ├── start.ps1       ← 一键启动脚本  │
  │  ├── 启动桌宠.bat    ← 双击入口      │
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
| `start.ps1` | 一键启动脚本（自动检测环境、安装依赖、启动后端） |
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
> 编辑 `F:\Live2Dai\shared\persona.yaml` 即可，无需改 conf.yaml。

---

## 启动方式

```powershell
# 方式 1：双击
F:\Live2Dai\Live2Dai-pc\启动桌宠.bat

# 方式 2：PowerShell
cd F:\Live2Dai\Live2Dai-pc
.\start.ps1

# 方式 3：跳过环境检测（快速启动）
.\start.ps1 -SkipEnvCheck

# 方式 4：直接启动 Python 后端
cd F:\Live2Dai\Live2Dai-pc\open-llm-vtuber
.venv\Scripts\python run_server.py
```

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
cd F:\Live2Dai\Live2Dai-pc\open-llm-vtuber
.venv\Scripts\python run_server.py  # 前台运行看输出
```
