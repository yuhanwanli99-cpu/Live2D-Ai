# Live2D-Ai — PC 端（WSL2/Linux 主力）

基于 [Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber) 的 Live2D 前端，直连 DeepSeek API。
> **平台定位（2026-08-19 定调）：PC 端以 WSL2/Linux 为主力（开发 + 运行 + CI）。原生 Windows 已放弃**，不再提供 Windows 启动入口。

## 一键启动（WSL2）

```bash
cd ~/Live2D-Ai/Live2D-Ai-pc
./start.sh
```

启动后浏览器访问 **<http://localhost:12393/>** 即可看到 Live2D 界面（WSL2 localhost 自动转发，可在 Windows 浏览器打开）。

## WSL2 环境准备（一次性）

```bash
# 0. Python 3.12（项目约束 >=3.10,<3.13；系统 3.14 不满足）
uv python install 3.12

# 1. WSL 原生盘 venv（推荐放仓库根 .venv，IO 快；勿放 /mnt/g）
uv venv --python 3.12 .venv     # 仓库根 Live2D-Ai/.venv，start.sh 会自动探测

# 2. 安装 open-llm-vtuber（editable）
uv pip install --python .venv/bin/python \
    -e Live2D-Ai-pc/open-llm-vtuber

# 3. ffmpeg（Faster-Whisper ASR 需要；不装则语音输入不可用，对话仍正常）
sudo apt-get update && sudo apt-get install -y ffmpeg
```

> venv 查找优先级：`LIVE2DAI_WSL_VENV` 环境变量 > 仓库根 `.venv` > 旧文档默认 `~/Live2D-Ai/.venv`；`start.sh` 自带核心依赖自检。

> **前端重建后 Live2D Core 说明**：`renderer/public/libs/live2dcubismcore.min.js` 是 gitignored 的运行期二进制
> （Live2D 渲染必需，Vite 构建会把它拷到 `frontend/libs/`）。若该文件缺失（新 clone 或误删），请从官方 npm 恢复：
> ```bash
> cd open-llm-vtuber/renderer
> npm pack live2dcubismcore@1.0.2 && tar -xzf live2dcubismcore-1.0.2.tgz package/live2dcubismcore.min.js --strip-components=1
> mkdir -p public/libs && mv live2dcubismcore.min.js public/libs/
> rm -f live2dcubismcore-1.0.2.tgz
> ```

## 设置 / Dashboard

聊天面板右上角的 **⚙** 可打开完整设置中心，包含：

- **对话/LLM**：切换 LLM Provider、修改模型 / Base URL / 温度、测试连接、自动发现可用模型、全部 Provider 一键检查
- **语音/TTS**：切换 TTS / ASR、修改音色 / 模型 / WebSocket URL、试听（WSL2 请勿选 `pyttsx3`，其依赖 Windows SAPI）
- **形象/Live2D**：查看模型列表、切换模型
- **API Keys**：集中管理 DeepSeek / 智谱 / Qwen / OpenAI 兼容 Key
- **人设/提示词**：编辑 `shared/persona.yaml` 的系统提示词与人设参数
- **系统**：端口 / FPS / 日志级别，以及 `conf.yaml` 原始 YAML 设定树

保存后会写入 `conf.yaml`、`shared/persona.yaml` 与仓库根 `.env`，**重启后生效**。

## 前置准备

### 1. API Key

编辑 `Live2D-Ai-pc/open-llm-vtuber/conf.yaml`，填入密钥：

```yaml
llm_configs:
  deepseek_llm:
    llm_api_key: 'sk-你的DeepSeekKey'    # ← 必填

vision_config:
  zhipu_llm:
    zhipu_api_key: '你的智谱API Key'      # ← 选填（视觉功能）
```

> **人设**已自动从 `shared/persona.yaml` 加载，无需额外配置。

## 功能

| 功能 | 说明 |
| ------ | ------ |
| 语音对话 | 说话即聊，语音回复（麦克风/播放走浏览器 Web Audio，后端无需本地音频设备） |
| 视觉识别 | 能看到屏幕内容（需 Zhipu API Key） |
| 透明穿透 | 模式，不遮挡操作 |
| MCP 工具 | 查天气、搜索、记笔记（需 uvx） |
| 表情联动 | 根据对话情感自动切换表情 + 嘴型同步 |

## 配置人设

编辑 `shared/persona.yaml`，改动即时生效（需重启后端）。

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

## 常见问题（WSL2）

| 问题 | 解决 |
| ------ | ------ |
| 启动报 Python 版本错误 | 用 `uv python install 3.12`，勿用系统 Python 3.14 |
| venv 未找到 | 先执行「WSL2 环境准备」，或设 `LIVE2DAI_WSL_VENV` |
| 后端启动但网页白屏 | 检查 `open-llm-vtuber/frontend/` 是否存在，或重新 `git submodule update --init --recursive` |
| ASR 不工作 | 检查 ffmpeg：`sudo apt-get install -y ffmpeg` |
| 模型不说话 | 检查 DeepSeek API Key 是否正确 |

---

详细架构见 `HANDOVER.md`，Android 端见 `Live2D-Ai-Android/`。
