# Live2Dai — Windows 桌面端

基于 [Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber) 的 Windows Live2D 前端，直连 DeepSeek API。

## 一键启动

```powershell
cd F:\Live2Dai\Live2Dai-pc
.\start.ps1
```

启动后浏览器访问 **<http://localhost:12393/>** 即可看到 Live2D 界面。

双击 `F:\Live2Dai\Live2Dai-pc\启动Live2Dai.bat` 效果相同。

## 前置准备

### 1. API Key

编辑 `F:\Live2Dai\Live2Dai-pc\open-llm-vtuber\conf.yaml`，填入密钥：

```yaml
llm_configs:
  deepseek_llm:
    llm_api_key: 'sk-你的DeepSeekKey'    # ← 必填

vision_config:
  zhipu_llm:
    zhipu_api_key: '你的智谱API Key'      # ← 选填（视觉功能）
```

> **人设**已自动从 `F:\Live2Dai\shared\persona.yaml` 加载，无需额外配置。

### 2. Python 环境

`start.ps1` 会自动检测 Python 3.10+，创建虚拟环境（优先 `uv sync`，降级 `python -m venv`），安装依赖。

如需手动搭建：

```powershell
cd F:\Live2Dai\Live2Dai-pc\open-llm-vtuber
# 安装 uv（可选，但推荐）
powershell -c "irm https://astral.sh/uv/install.ps1 | iex"
# 同步依赖
uv sync
```

### 3. ffmpeg（可选）

Faster-Whisper ASR 需要 ffmpeg 解码音频。如未安装，对话功能仍正常，只是语音输入不可用。

```powershell
winget install Gyan.FFmpeg
```

## 功能

| 功能 | 说明 |
| ------ | ------ |
| 语音对话 | 说话即聊，语音回复 |
| 视觉识别 | 能看到屏幕内容（需 Zhipu API Key） |
| 透明穿透 | 模式，不遮挡操作 |
| MCP 工具 | 查天气、搜索、记笔记（需 uvx） |
| 表情联动 | 根据对话情感自动切换表情 + 嘴型同步 |

## 配置人设

编辑 `F:\Live2Dai\shared\persona.yaml`，改动即时生效（需重启后端）。

```yaml
name: "白 (Bai)"
system_prompt: |
  你是白，一位温柔可爱的二次元少女 AI 助手，温柔、可爱、萌，
  说话简单直接，句尾常带"哦""呀""～"（完整设定见 shared/persona.yaml）。
model: "deepseek-v4-flash"
temperature: 0.8
```

## 添加 Live2D 模型

将 `.model3.json` + `.moc3` + 贴图放到 `open-llm-vtuber/live2d-models/`，编辑 `model_dict.json`。
免费模型来源：[模之屋](https://www.aplaybox.com/) / [Booth](https://booth.pm/)。

## 常见问题

| 问题 | 解决 |
| ------ | ------ |
| 启动报 Python 版本错误 | 安装 Python 3.10+ (winget install Python.Python.3.12) |
| 后端启动但网页白屏 | 检查 `frontend/` 目录是否存在，或重新 `git submodule update --init --recursive` |
| ASR 不工作 | 检查 ffmpeg 是否安装：`winget install Gyan.FFmpeg` |
| 模型不说话 | 检查 DeepSeek API Key 是否正确 |

---

详细架构见 `F:\Live2Dai\HANDOVER.md`，Android 端见 `F:\Live2Dai\Live2Dai-Android/`。
