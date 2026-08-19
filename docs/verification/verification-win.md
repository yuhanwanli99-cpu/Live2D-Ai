# Live2D-Ai Windows 桌面端 — 验证结果报告

> 验证日期: 2026-07-18
> 验证环境: Windows (Git Bash / Python 3.10+)
> 验证范围: 配置文件修复 + 一键启动链路完整性

---

## 1. 修复内容

### 1.1 conf.yaml — 添加 `translator_config` 缺失字段

**问题**: Pydantic 模型 `TTSPreprocessorConfig` 要求 `translator_config: TranslatorConfig` 为必填字段，但 conf.yaml 中 `translate_audio` 被直接放在 `tts_preprocessor_config` 层级下，未嵌套在 `translator_config` 子对象中。

**错误信息**:

```
pydantic_core._pydantic_core.ValidationError: 1 validation error for Config
character_config.tts_preprocessor_config.translator_config
  Field required
```

**修复**: 将:

```yaml
tts_preprocessor_config:
    translate_audio: false
```

改为:

```yaml
tts_preprocessor_config:
    translator_config:
      translate_audio: false
      translate_provider: 'deeplx'
```

**验证**: `Config(**config_dict)` 创建成功，无验证错误。

---

## 2. 配置验证结果

### 2.1 conf.yaml 配置检查

| 字段 | 值 | 状态 |
| ------ | ----- | ------ |
| `system_config.live2d_fps` | `120` | ✅ |
| `character_config.conf_name` | `niziiro_mao` | ✅ |
| `character_config.load_persona_from_file` | `shared/persona.yaml` | ✅ |
| `agent_config.conversation_agent_choice` | `basic_memory_agent` | ✅ |
| `agent_config.llm_configs.deepseek_llm.model` | `deepseek-v4-flash` | ✅ |
| `agent_config.llm_configs.zhipu_llm.model` | `glm-4.6v` | ✅ |
| `asr_config.asr_model` | `faster_whisper` | ✅ |
| `tts_config.tts_model` | `edge_tts` | ✅ |
| `vad_config.vad_model` | `silero_vad` | ✅ |
| `vision_config.vision_enabled` | `true` | ✅ |

### 2.2 人设文件路径解析

**`run_server.py`** 中的 `_resolve_shared_path()` 函数正确处理：

- 输入：`shared/persona.yaml`
- 解析逻辑：`_project_root` = `F:\Live2D-Ai\desktop\open-llm-vtuber`
  → 两级上到 `F:\Live2D-Ai`
  → 拼接得 `F:\Live2D-Ai\shared\persona.yaml`
- 文件存在：✅ (1395 字符的 system_prompt)

### 2.3 Persona 覆盖配置

`shared/persona.yaml` 中的模型覆盖在 `run_server.py` 中生效：

| 字段 | persona.yaml 值 | 应用到 deepseek_llm |
| ------ | ----------------- | --------------------- |
| `model` | `deepseek-v4-flash` | ✅ |
| `temperature` | `0.8` | ✅ |
| `max_tokens` | `300` | 存储但不覆盖（LLM 不支持） |
| `vision_model` | `glm-4.6v` | 只读参考 |

### 2.4 MCP 服务器配置

`ServerRegistry` 直接从 `mcp_servers.json` 加载：

| MCP 服务器 | 运行时 | 状态 |
| ----------- | -------- | ------ |
| `time` | `uvx mcp-server-time` | ✅ |
| `ddg-search` | `uvx duckduckgo-mcp-server` | ✅ |
| `weather` | `uvx mcp-server-weather` | ✅ |
| `filesystem` | `uvx mcp-server-filesystem` | ✅ (限 F:/Live2D-Ai/data/) |
| `sequential-thinking` | `uvx mcp-server-sequential-thinking` | ✅ |

> **注意**: conf.yaml 中 `mcp_servers: 'mcp_servers.json'` 未在 Pydantic Config 模型中使用，但无负面影响。ServerRegistry 使用默认路径 `mcp_servers.json` (相对 CWD)，内容与 `shared/mcp_tools.json` 一致。

---

## 3. 文件完整性检查

### 3.1 前端静态文件

| 文件 | 大小 | 状态 |
| ------ | ------ | ------ |
| `frontend/index.html` | ~200B | ✅ |
| `frontend/assets/main-nu7uwxNJ.js` | 1.9 MB | ✅ (编译后 React 应用) |
| `frontend/assets/main-QEkl09-0.css` | 41 KB | ✅ |

### 3.2 Live2D 模型文件 (`live2d-models/niziiro_mao/runtime/`)

| 文件 | 状态 |
| ------ | ------ |
| `niziiro_mao.moc3` | ✅ |
| `niziiro_mao.model3.json` | ✅ |
| `niziiro_mao.cdi3.json` | ✅ |
| `niziiro_mao.physics3.json` | ✅ |
| `niziiro_mao.pose3.json` | ✅ |
| `expressions/` | ✅ |
| `motions/` | ✅ |

### 3.3 其他资源

| 资源 | 路径 | 状态 |
| ------ | ------ | ------ |
| 头像 | `avatars/mao.png` | ✅ |
| 背景 | `backgrounds/` (15 张) | ✅ |
| 人设 | `shared/persona.yaml` | ✅ |
| MCP 配置 | `shared/mcp_tools.json` | ✅ |
| 角色备选配置 | `characters/` (5 个) | ✅ |

### 3.4 虚拟环境

`.venv/Scripts/` 中包含关键可执行文件：

| 依赖 | 状态 |
| ------ | ------ |
| `python.exe` | ✅ |
| `uvicorn.exe` | ✅ |
| `fastapi.exe` | ✅ |
| `edge-tts.exe` | ✅ |
| `mcp.exe` | ✅ |
| `websockets.exe` | ✅ |
| `onnxruntime_test.exe` | ✅ (faster-whisper 所需) |

---

## 4. 启动流程验证

### 4.1 脚本链路

```
启动Live2D-Ai.bat
  └─→ powershell -ExecutionPolicy Bypass -File .\start.ps1
        └─→ cd open-llm-vtuber
              ├─ [1/6] Python 检测    → python 3.10+ (系统 PATH / .venv)
              ├─ [2/6] 虚拟环境检测   → .venv 已存在
              ├─ [3/6] 依赖检测       → pip install -e . (open_llm_vtuber 已安装)
              ├─ [4/6] ffmpeg 检测    → 可选（仅警告）
              ├─ [5/6] Electron 前端   → 可选（检测安装路径，但 Web 前端已内置在 frontend/）
              └─ [6/6] API Key 检测   → DeepSeek ✅ | 智谱 ✅
        └─→ python run_server.py
              ├─ 加载 conf.yaml
              ├─ 加载 shared/persona.yaml → 注入 persona_prompt
              ├─ 应用模型覆盖 (model, temperature)
              ├─ Config Pydantic 验证 → Config 对象
              ├─ WebSocketServer.initialize()
              └─ uvicorn listen http://0.0.0.0:12393
```

### 4.2 端口与地址

| 设置 | 值 |
| ------ | ----- |
| 监听地址 | `0.0.0.0:12393` |
| 前端 URL | `http://localhost:12393/` |
| WebSocket | `ws://<PC_IP>:12393/client-ws` |

### 4.3 帧率配置

`conf.yaml` 中 `live2d_fps: 120`，Pydantic 模型 `SystemConfig.live2d_fps` 预期值为 120。配置确认：✅

---

## 5. API Keys 状态

| Key | 值 | 状态 |
| ----- | ----- | ------ |
| DeepSeek API Key | `sk-***REDACTED-ROTATED***` | ✅ 已配置 |
| 智谱 API Key | `***ZHIPU-KEY-REDACTED-ROTATED***` | ✅ 已配置 |

---

## 6. 启动命令

```powershell
# 方式 1：双击
F:\Live2D-Ai\desktop\启动Live2D-Ai.bat

# 方式 2：PowerShell
cd F:\Live2D-Ai\desktop
.\start.ps1

# 方式 3：跳过环境检测（快速启动）
.\start.ps1 -SkipEnvCheck

# 方式 4：直接启动 Python 后端
cd F:\Live2D-Ai\desktop\open-llm-vtuber
.venv\Scripts\python run_server.py
```

启动后浏览器访问 **<http://localhost:12393/>** 即可看到前端界面。

---

## 7. 已知限制

1. **ffmpeg**: start.ps1 会检测 ffmpeg 是否安装。faster-whisper 需要 ffmpeg 进行音频解码。如未安装，ASR 可能无法工作。安装命令: `winget install Gyan.FFmpeg`
2. **uvx**: MCP 服务器需要 `uvx` 命令可用（通过 `pip install uv` 或独立安装）。如未安装，MCP 工具功能不可用但不影响核心对话。
3. **Electron 前端**: start.ps1 检测 Electron 桌面客户端安装路径，但预编译的 Web 前端已内置于 `frontend/` 目录，可通过浏览器直接访问，**不依赖 Electron**。
4. **首帧延迟**: faster-whisper 首次加载 large-v3-turbo 模型可能需要几十秒。后续使用不再有延迟。

---

## 8. 验证结论

| 检查项 | 结果 |
| -------- | ------ |
| conf.yaml Pydantic 验证通过 | ✅ |
| Persona 路径解析正确 | ✅ |
| API Keys 已配置 | ✅ |
| 前端静态文件完整 | ✅ |
| Live2D 模型文件完整 | ✅ |
| 帧率 120fps 配置确认 | ✅ |
| MCP 服务器配置可用 | ✅ |
| 一键启动脚本链路完整 | ✅ |

**总体结论**: ✅ 所有配置检查通过，一键启动链路验证完整。服务器应能在 `http://localhost:12393` 正常启动并服务前端页面。
