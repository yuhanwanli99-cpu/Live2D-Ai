# MeloTTS 中文（模型卡出处）

| 项 | 值 |
| --- | --- |
| 代码 | https://github.com/myshell-ai/MeloTTS |
| 代码 tag | `v0.1.2`（commit `b633f243412169b999526e19eb6fcac0974b5d30`） |
| 代码许可 | MIT，Copyright (c) 2024 MyShell.ai（见同目录 LICENSE） |
| 权重 | `myshell-ai/MeloTTS-Chinese`（Hugging Face） |
| 权重修订号 | `af5d207a364ea4208c6f589c89f57f88414bdd16`（2024-03-01） |
| 权重许可 | MIT |
| 只拉的语言 | **中文（ZH）**，不拉英/日/韩/法/西 |

## 仓库里放了什么

- `LICENSE`：MeloTTS 的 MIT 原文（逐字）。
- `weights/config.json`：**普通 git 入库**（小 JSON，官方原文）。
- `weights/checkpoint.pth`：约 208MB，**只走 Git LFS**（`.gitattributes` 里一条规则）。
  超过 GitHub 单文件 100MB 上限，禁止裸 `git add`。
- `download.sh`：按上表修订号把两份权重下到 `weights/`（失败非 0 退出）。
- `start.sh` / `serve.py`：薄封装（首次启用时装 venv，起 OpenAI 兼容的
  `POST /audio/speech`）。

## 不进仓库的东西

- `.venv/`（Python 包与虚拟环境；隐藏目录，也不进 rust-ratio 统计）。
- `checkpoint.pth` 的**文件本体**只在 LFS 里；
- `bert-base-multilingual-uncased`（中文推理要用）：本轮**不放进仓库**。
  缺它就让本次启动失败，并在错误里写出缺的是它——**不要**静默下载完还声称开箱。

## 本机来源声明

本目录的内容**不来自** `/home/skystar/tts/MeloTTS`，也不来自 conda 环境 `melo`
（那两者都不是我们可再分发的产物）。
