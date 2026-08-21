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
- **插件管理**：查看已注册插件、原生/外部标识、启用/禁用、状态与错误、注册的钩子与能力

保存后会写入 `conf.yaml`、`shared/persona.yaml` 与仓库根 `.env`，**重启后生效**。

### 本地 Melo TTS（免费，无需云端 Key）

设置中心的 **语音/TTS → 本地 / 免费 → Melo TTS（本地）** 提供无需云端 TTS Key 的本地合成：

1. 在 TTS 提供商下拉选择 `melo_tts`，保存后重启后端。
2. 语言选择 `ZH`（实际为中英混合模型），设备默认 `cpu`；四韵律参数只需改节奏、不改音色：
   - `speed`：语速，0.5~2.0（默认 1.0）
   - `sdp_ratio`：随机韵律预测占比，0~1（默认 0.2，调高更有起伏）
   - `noise_scale`：音高随机度，0~1（默认 0.6，低平稳、高自然但可能不稳）
   - `noise_scale_w`：音素时长随机度，0~1（默认 0.8，调高则抑扬顿挫）
3. 首次试听会触发模型下载（从 hf-mirror 拉取）；未安装 Melo 时，试听/保存会给出可读错误，不影响默认启动。
4. 默认使用 CPU 合成；单次输入限制 `MELO_MAX_TEXT_LEN=200` 字、`MELO_MAX_SENTENCES=4` 句，超出会返回可读错误，避免 Melo 把长文本拆成过多小句。
5. 聊天链路的 LLM 回答会自动切成约 36 字的小块依次进 TTS 队列（`split_tts_chunks`），首块更早开始合成/播放，缓解长句等待。
6. 括注舞台说明（（）/()）不会朗读也不会显示；其中的动作词（扇风/点头等）会映射为半身参数动作。
7. 一次用户输入默认触发多段续聊（`MELO_MULTI_TURN_EXTRA=2`，设 0 可关），每段分别合成 TTS 并按序播放，用户可随时打断。

本地 HTTP 接口（供本机脚本调用）：

```bash
curl -X POST http://localhost:12393/api/tts/local/synthesize \
  -H 'Content-Type: application/json' \
  -d '{"text":"你好，这是本地合成测试。","language":"ZH","device":"cpu","speed":1.0,"sdp_ratio":0.2,"noise_scale":0.6,"noise_scale_w":0.8,"format":"wav"}'
```

返回 JSON 中的 `audio_url` 可直接下载生成的 WAV。

> **外部 Melo 环境对接**：本仓库运行环境（Python 3.12）不安装 Melo；后端会通过常驻
> `melo_worker.py` 子进程调用独立 conda 环境里的 `melo.api.TTS`，避免每句 10~30s 模型加载。
> 默认解释器为 `~/miniconda3/envs/melo/bin/python`，可用环境变量 `MELO_PYTHON` 覆盖：
>
> ```bash
> export MELO_PYTHON=~/miniconda3/envs/melo/bin/python
> ```
>
> 非交互式启动（如 cron / `wsl.exe -e`）不会加载 `.bashrc`，worker 已自动注入
> `HF_ENDPOINT=https://hf-mirror.com`，首次下载其他语言模型时无需手动带环境变量。
> 若 `MELO_PYTHON` 路径不存在，选择 Melo 或试听会返回可读错误，不影响默认启动。

### 插件系统

除核心链路（对话脑 / TTS / Live2D）外，其余能力都必须在插件系统注册并启用后才可用。
设置中心的「插件管理」会显示每个插件是 `native`（主仓库自带）还是 `external`（来自插件仓库/第三方）。

- 输入端口：`input-port`
- 通用直播事件输入：`live-ingress`
- 长期记忆：`memory`
- 长程人设：`persona`
- 上下文压缩：`context-compactor`
- 身体/表情导演：`body-director`
- 心情/TTS 导演：`mood-tts-director`
- 语音识别：`asr`
- 知识库：`knowledge`
- 联网查询：`web-search`
- 声音资产：`voice-assets`
- 模型资产：`model-assets`
- 配置包：`config-pack`

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
