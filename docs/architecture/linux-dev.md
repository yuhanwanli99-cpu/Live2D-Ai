# Linux（WSL2）环境定位：PC 端主力

> 平台轨迹（2026-08-19 用户定调，见 `docs/plans/wsl2-migration-plan.md`）：
> **PC 端主力迁移到 WSL2（开发 + 运行 + CI），原生 Windows 已放弃**。
> （Windows 启动入口已移除；仓库已迁移至 WSL2 原生盘并按 `Live2D-Ai` 更名。）
> 双端结构：Android（手机/Compose）+ PC（WSL2/Linux 原生运行 Open-LLM-VTuber）。

## 为什么以 WSL2/Linux 为主力

- PC 端音频走浏览器 Web Audio（非本地设备）、TTS/LLM 走网络、路径已平台无关 —— 架构上无硬性 Windows 依赖。
- 方便直接跑 root pytest、`shared/check_cross_platform_consistency.py`、`scripts/health_check.py`、PC pytest。
- 在无 Windows/Android 的机器上即可做完整 PC 逻辑开发与验证。
- CI 可低成本跑「无 Key 地基验证」与 PC 单测，无需 Windows runner。
- 摆脱对 Windows `.venv` / `Scripts\python.exe` / 便携 Python 的依赖。

## 主力入口

- `Live2D-Ai-pc/start.sh`：Linux/WSL2 一键启动（**主力**）。
- venv：`~/Live2D-Ai/.venv`（WSL 原生盘，IO 快；用 `uv` 装 Python 3.12）。
- **原生 Windows 已放弃**：`start.ps1` / `启动Live2D-Ai.bat` 等 Windows 启动入口已移除，不再维护。

## 不做什么

- 不维护 Linux 版 Live2D **桌面图形**包装器/安装包（PC 运行形态 = WSL2 后端 + Windows 浏览器访问 `http://localhost:12393`；前端渲染仍由浏览器承担）。
- 不再把 Windows 专属能力（如便携 Python、ffmpeg.exe、pywin32、SAPI/pyttsx3）作为 PC 端维护目标；`pyttsx3` 属遗留，WSL2 下勿选。
- Android 构建仍以 Android Studio/CI 为主（与 PC 平台分轨）。

## 快速准备

```bash
bash scripts/setup_linux.sh
source .venv/bin/activate
python scripts/verify_all.py
```

`verify_all.py` 在 Linux 上默认跑：

- health check
- shared registry validation
- cross-platform consistency
- root pytest

PC 的 Open-LLM-VTuber 依赖较重（torch/onnxruntime），以 WSL2 原生盘上的独立 venv 运行（见
`docs/plans/wsl2-migration-plan.md` 的 Phase 0–2），也可在专用 CI 环境跑。

## 目录约定

- `scripts/setup_linux.sh`：Linux 轻量环境准备
- `Live2D-Ai-pc/start.sh`：PC 主力一键启动（WSL2）
- `scripts/health_check.py`：无 Key 健康自检
- `scripts/verify_all.py`：一键验证入口
