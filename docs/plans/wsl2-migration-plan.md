# WSL2 迁移计划：pc 端主力代码迁移到 WSL2 环境

> 状态: **已执行（结构性迁移 + 更名完成）**（2026-08-19）| **2026-08-20 运行验证合拢**：start.sh venv 探测修复 + `/health` 端点落地，`start.sh` 一键启动、`/health` 200、前端渲染/配置加载均实机验证通过 | 创建日期: 2026-08-19 | 适用: `Live2D-Ai-pc/`
> 定位: **WSL2 为 PC 端主力（开发 + 运行 + CI）；原生 Windows 已放弃**（Windows 启动入口 `start.ps1` / `启动Live2D-Ai.bat` 已移除）。项目仓库已从 G 盘 `/mnt/g/git/Live2Dai` 迁移至 WSL2 原生盘并更名为 `Live2D-Ai`。
> 待验证: 完整运行验证（安装依赖 → 启动 → /health + 前端渲染 + 配置加载）需在本机执行 Phase 1-3 后落地。**（2026-08-20：已验证，见下方验收 checklist）**

---

## 1. 背景与目标

当前 PC 端（`Live2D-Ai-pc/`，基于 Open-LLM-VTuber）主要在 **Windows** 下运行：
一键入口是 `start.ps1` / `启动Live2D-Ai.bat`，依赖 Windows 上的 `.venv`（`Scripts/python.exe`）。

**项目轨迹（2026-08-19 用户定调）：PC 端主力迁移到 WSL2，放弃 Windows 作为主力，最终放弃原生 Windows。**
本次目标：**让 PC 端主力代码在 WSL2 里原生运行、可验证、可持续**，**移除原生 Windows 启动入口**（作为主力进一步升级为「放弃 Windows」）。

### 验收标准（Definition of Done）

1. WSL2 内存在独立 Python 3.10~3.12 虚拟环境（放 WSL 原生盘，不在 `/mnt/g`）。
2. `open-llm-vtuber` 全部 Python 依赖在 WSL 内安装成功（`import open_llm_vtuber` 通过）。
3. 新增 Linux 启动脚本 `Live2D-Ai-pc/start.sh`（含环境自检），能正常拉起后端，作为**主力入口**。
4. `curl http://localhost:12393/health` 返回 200。
5. 从 **Windows 浏览器** 访问 `http://localhost:12393` 能渲染 Live2D 前端（验证 WSL2 localhost 转发）。
6. 人设/配置加载正常（启动日志出现 `Loaded persona from .../shared/persona.yaml`）。
7. 文档（README / AGENT）补 WSL2 章节并把 `start.sh` 标为**主力**；本迁移流程记录到 CHANGELOG。
8. （遗留回归）Windows 端 `start.ps1` 未被破坏、仍可用——仅保证不删不坏，不投入新改进。

---

## 2. 现状盘点（已核查）

| 项 | 现状 | 对迁移的影响 |
| --- | --- | --- |
| 运行环境 | 当前就在 WSL2（Ubuntu 26.04, 内核 6.18 microsoft-standard-WSL2） | 直接在本机验证 |
| 仓库位置 | `/mnt/g/git/Live2D-Ai`（G 盘挂载，跨文件系统） | venv 不能放这里（IO 慢），需放 WSL 原生盘 |
| 系统 Python | 3.14.4 | **不满足** `requires-python >=3.10,<3.13`，需另装 3.12 |
| 现有 `.venv` | Windows venv（`Scripts/python.exe`） | WSL 下不可用；保留不动，另建 Linux venv |
| `uv` | 未安装 | 推荐引入（快速装 3.12 + 依赖） |
| `ffmpeg` | 未安装 | ASR（faster-whisper）需要，需 apt 安装 |
| TTS 默认 | `cosyvoice_cloud`（云端 WS）；`edge_tts` 也在列表 | 均为网络型，WSL2 可直接用 ✅ |
| 音频采集/播放 | 由**浏览器端 Web Audio** 采集麦克风并 WS 推流、播放 TTS | WSL2 后端**不需要本地音频设备** ✅ |
| `pyttsx3` | 依赖 SAPI（Windows 专有） | Linux 上不可用；非默认，保留风险项 |
| 端口 | `host 0.0.0.0 / port 12393` | WSL2 localhost 自动转发，Windows 浏览器可直连 |
| `shared/` 路径 | `run_server.py` 用两级向上解析 + `os.path.join` | 已是平台无关 ✅ |
| `.env`（仓库根） | `run_server.py` 绝对路径加载 | 双平台通用 ✅ |
| 前端 | `frontend/` 是 git submodule（已拉取 `index.html/libs/assets`） | 需确认 submodule 已 init |

### 结论
架构上无硬性 Windows 依赖（音频走浏览器、TTS/LLM 走网络、路径已平台无关），
迁移风险集中在：**Python 版本**、**依赖体量**、**venv 位置**、**ffmpeg**、**pyttsx3 降级**。

---

## 3. 关键 WSL2 适配点（风险与对策）

1. **Python 3.12 安装**
   - 用 `uv python install 3.12`（推荐，可多版本共存、不污染系统）。
   - 降级方案：`sudo add-apt-repository ppa:deadsnakes/ppa && sudo apt-get install python3.12 python3.12-venv`。

2. **venv 位置（已决策）**
   - 放 `~/Live2D-Ai-pc/.venv`（WSL 原生盘，IO 快）。
   - 与仓库内 Windows `.venv` 完全隔离，互不影响。

3. **依赖体量大（torch / onnxruntime / faster-whisper）**
   - 默认按 `pyproject.toml` + `requirements.txt` 全量安装。
   - 可选优化：torch 用 **CPU-only** 源（`https://download.pytorch.org/whl/cpu`）可省约 2GB。
   - faster-whisper `large-v3-turbo` 首次运行需下载模型（约 1.6GB 到 `models/whisper`）；
     冒烟阶段可临时 `asr_model: disabled`，验证通过后再启用。

4. **ffmpeg**
   - `sudo apt-get update && sudo apt-get install -y ffmpeg`。

5. **pyttsx3（SAPI-only）**
   - 默认 TTS 用 `cosyvoice_cloud` / `edge_tts`，无影响。
   - 若误选 `pyttsx3_tts`，Linux 会失败 → 需要在设置/文档中注明「WSL2 请勿选 pyttsx3」。

6. **前端 submodule**
   - 执行 `git submodule update --init --recursive`；若 `frontend/` 为空则页面白屏。

7. **启动脚本**（只新增不删改）
   - 新增 `Live2D-Ai-pc/start.sh`（Linux 版一键启动 + 环境自检），
     `启动Live2D-Ai.bat` 与 `start.ps1` 保持不变。

---

## 4. 分阶段任务

### Phase 0 — 环境准备（一次性）
```bash
# 0.1 进入项目
cd /mnt/g/git/Live2D-Ai/Live2D-Ai-pc/open-llm-vtuber

# 0.2 前端 submodule（frontend/ 为空会导致白屏）
git submodule update --init --recursive

# 0.3 uv（装 Python 3.12 与依赖，推荐）
curl -LsSf https://astral.sh/uv/install.sh | sh
# 重新打开 shell 或 source ~/.bashrc 后：
uv python install 3.12
uv python list

# 0.4 ffmpeg（ASR 需要）
sudo apt-get update
sudo apt-get install -y ffmpeg

# 0.5 （可选）确认基础工具
which git curl unzip
```

### Phase 1 — 创建 Linux venv 并安装依赖
```bash
# 1.1 venv 放 WSL 原生盘（不在 /mnt/g）
mkdir -p ~/Live2D-Ai-pc
uv venv --python 3.12 ~/Live2D-Ai-pc/.venv

# 1.2 安装 open-llm-vtuber（editable，含 src 路径）+ bilibili 可选
uv pip install \
    --python ~/Live2D-Ai-pc/.venv/bin/python \
    -e . --extra bilibili

# 1.3 （可选，省空间）torch 换 CPU 版：
# 先在 1.2 之外单独装：
# uv pip install --python ~/Live2D-Ai-pc/.venv/bin/python \
#     torch --index-url https://download.pytorch.org/whl/cpu

# 1.4 验证
~/Live2D-Ai-pc/.venv/bin/python --version            # 3.12.x
~/Live2D-Ai-pc/.venv/bin/python -c "import open_llm_vtuber, fastapi, edge_tts; print('imports OK')"
```

### Phase 2 — 新增 Linux 启动脚本（新增文件，不破坏 Windows 入口）
新增 `Live2D-Ai-pc/start.sh`（示意，执行时落盘并 `chmod +x`）：

```bash
#!/usr/bin/env bash
# Live2D-Ai — WSL2/Linux 一键启动（仅启动 + 日志）
# 默认人设: 白 | 访问 http://localhost:12393
set -euo pipefail
ProjectRoot="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VtuberDir="$ProjectRoot/open-llm-vtuber"
VenvDir="${LIVE2DAI_WSL_VENV:-$HOME/Live2D-Ai-pc/.venv}"
PY="$VenvDir/bin/python"

if [ ! -x "$PY" ]; then
    echo "[Live2D-Ai] 未找到 WSL venv: $PY"
    echo "[Live2D-Ai] 请先执行 Phase 1（uv venv + uv pip install -e .），或设 LIVE2DAI_WSL_VENV"
    exit 1
fi

cd "$VtuberDir"
echo "[Live2D-Ai] Starting server... http://localhost:12393"
exec "$PY" run_server.py
```

命令：`chmod +x Live2D-Ai-pc/start.sh && Live2D-Ai-pc/start.sh`

（可选）新增 `Live2D-Ai-pc/setup_wsl.sh` 一键完成 Phase 0+1+2（幂等：已有 venv 则跳过创建）。

### Phase 3 — 启动 & 验证
```bash
# 3.1 启动（前台观察日志；确认出现 Loaded persona from .../shared/persona.yaml）
Live2D-Ai-pc/start.sh

# 3.2 健康检查（另开终端）
curl -s http://localhost:12393/health

# 3.3 前端：从 Windows 浏览器打开
#     http://localhost:12393/   （WSL2 localhost 自动转发）
```

验证条目：
- [ ] 启动无 ImportError / 无『未找到模块』
- [ ] `/health` 200
- [ ] 日志含 `Loaded persona from .../shared/persona.yaml`
- [ ] Windows 浏览器能渲染 Live2D 画面（模型 = bai）
- [ ] 设置页 / Dashboard 能打开
- [ ] （需 key）对话/LLM 连通；TTS 试听（edge_tts 或 cosyvoice）
- [ ] （可选，模型大）ASR：`faster_whisper` 下载一次模型后可用

### Phase 4 — 文档更新
1. `Live2D-Ai-pc/README.md`：新增「WSL2 启动」章节（一键 `./start.sh` + 依赖安装说明），
   并将绝对路径示例改为双平台说明（Windows `F:\Live2D-Ai` / WSL `/mnt/g/git/Live2D-Ai`）。
2. `Live2D-Ai-pc/AGENT.md`：启动方式表补 `start.sh`。
3. `CHANGELOG.md`：记录「PC 端 WSL2 迁移（2026-08-19）」。
4. 本文件（`docs/plans/wsl2-migration-plan.md`）执行后更新状态为「已落地 + 验证结果」。

---

## 5. 文件变更清单（预计）

| 文件 | 动作 | 说明 |
| --- | --- | --- |
| `Live2D-Ai-pc/start.sh` | 新增 | Linux/WSL2 一键启动 |
| `Live2D-Ai-pc/setup_wsl.sh` | 新增（可选） | 幂等的环境+依赖+venv 初始化 |
| `Live2D-Ai-pc/README.md` | 修改 | 补 WSL2 章节 |
| `Live2D-Ai-pc/AGENT.md` | 修改 | 启动方式补 start.sh |
| `CHANGELOG.md` | 修改 | 迁移记录 |
| `docs/plans/wsl2-migration-plan.md` | 新增 | 本计划 |

> 保留/不动（**遗留，非主力**）：`start.ps1`、`启动Live2D-Ai.bat`、仓库内 Windows `.venv`——
> 仅保证不被破坏，不作为新功能/发布的优先级目标。

---

## 6. 不做的事（Out of Scope）

- 不把 git 仓库从 G 盘搬到 WSL 原生盘（保持单仓库、双端同一文件）。
- 不改后端核心逻辑/路径解析（已验证平台无关）。
- 不做 Docker 化、不做 systemd 服务（如需后台常驻，可后续加 `nohup` / `systemd --user` 可选层）。
- 不删除 Windows 启动链——保留为**遗留降级**，但不再作为主力维护/发布目标。
- 不在本计划内处理 Android 端。

---

## 7. 风险与降级

| 风险 | 等级 | 对策 / 降级 |
| --- | --- | --- |
| Python 3.14 不满足约束 | 已规避 | 用 uv 装 3.12（固定），不用系统 python |
| 全量依赖体积大/慢 | 中 | 分步装；torch 可换 CPU-only；超时用后台任务 |
| faster-whisper 模型下载大 | 中 | 冒烟期 `asr_model: disabled`，验证后再启用 |
| `pyttsx3` SAPI 在 Linux 不可用 | 低 | 默认 TTS 已是云端/edge_tts；文档注明勿选 pyttsx3 |
| `/mnt/g` 跨盘运行 IO 慢 | 低 | venv 已放原生盘；代码只读，首启稍慢可接受 |
| Windows/WSL 双 venv 混淆 | 低 | 脚本显式区分：ps1 用 `.venv/Scripts`，sh 用 `~/Live2D-Ai-pc/.venv` |
| WSL IP/端口变化 | 低 | 用 localhost 转发即可；`0.0.0.0:12393` 已绑定 |
| 前端 submodule 缺失 | 中 | Phase 0 显式 `git submodule update --init --recursive` |

---

## 8. 验收 Checklist（勾选用）

- [x] `uv python install 3.12` 成功且 Python = 3.12.x（本机仓库内 `.uv-python/cpython-3.12.14` 托管）
- [x] `import open_llm_vtuber, fastapi, edge_tts` 通过（start.sh 自带自检）
- [x] `Live2D-Ai-pc/start.sh` 可执行、能拉起后端（2026-08-20 实机启动成功；已修复 venv 探测）
- [x] `curl http://localhost:12393/health` 200（新增 `/health` 端点，返回 status/version/uptime_s）
- [x] 浏览器访问 `http://localhost:12393` 渲染 Live2D + 设置中心可开（新 bundle + live2dcubismcore.min.js + 模型均 200）
- [x] 人设从 `shared/persona.yaml` 加载成功（启动日志确认）
- [ ] 对话/LLM 连通（需 `.env` 有 key——本机未配置真实 key，需用户填入后验证）
- [x] Windows 端 `start.ps1` 已被移除（迁移定调）；`_start_server.py` 已收敛复用 run_server bootstrap
- [x] README / AGENT / CHANGELOG 已更新
