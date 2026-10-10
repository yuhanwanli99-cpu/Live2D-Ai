# 提示词 · 本地出声接到本机

> 立档：2026-10-10。状态：**活**。计划书 `docs/plans/PLAN-tts-speak-2026-10-10.md`。交给有无头浏览器的工作 agent。管理员不在这一轮写产品代码。

```text
你在 /home/skystar/Live2D-Ai，分支 main，HEAD 应是 e2c3344（已在 origin/main）。只做一件事：让本机本地语音真的出声。计划书 docs/plans/PLAN-tts-speak-2026-10-10.md 是范围，按它做，不要重做云端/本地开关。

已经接好、不要改：
- 对话在 [tts].base_url 非空时走 crates/live2d-ai-runtime/src/tts.rs 的 POST {base_url}/audio/speech。本机 live2d-ai.toml 的 [tts] 已是 http://127.0.0.1:8091/v1、voice ZH、pcm、44100、单声道、无密钥。
- 浏览器 <audio> 播放。诊断里的「服务端音频后端 none」是 app_routes.rs 写死的，表示 Rust 进程不打开声卡。不要改 backend / available。
- 口型、分句、AudioSpec::DEFAULT_SAMPLE_RATE（24000）、导演、待机、渲染档位。

本机事实：
- crates/live2d-ai-mod-local-tts/melo/.venv 是 Python 3.14，import melo 失败。start.sh 看见该 python 可执行就跳过 pip，所以 last_error 是「载入模型失败 / No module named melo」。
- 系统 python3 只有 3.14。GitHub myshell-ai/MeloTTS tag v0.1.2 的 setup.py 含 from pip.req import parse_requirements，现代 pip 装不上。
- 可用的 3.10：uv 的 ~/.local/share/uv/python/cpython-3.10.21-linux-x86_64-gnu/bin/python，以及 ~/miniconda3/envs/melo/bin/python（3.10.20）。只用它们创建仓库内的新 venv。不要往 conda 的 melo 环境里装包。
- 权重已在 melo/weights（44100，说话人只有 ZH）。磁盘余量约 21GB。serve.py 使用 device=cpu。torch 装 CPU 轮。

要做：
1. 改 melo/start.sh。venv 不能 import melo 时，只删除 melo/.venv，用 Python 3.10 重建。没有 3.10 就退出，说明系统 python3 是 3.14、v0.1.2 装不上，禁止改用 3.14。
2. 把 tag v0.1.2 克隆到 melo/.src（gitignore）。setup.py 若含 from pip.req import，删掉该 import 以及 parse_requirements 那两行，改为读取 requirements.txt 的非空非注释行。安装前 export PATH="$VENV_DIR/bin:$PATH"。先用 CPU 索引装 torch 与 torchaudio，再装其余。失败即停，不重试。
3. bert-base-multilingual-uncased 不入库。没有缓存时先打印一行「[melo] 正在下载 bert-base-multilingual-uncased（不入库）」，下载到 melo/.cache 并作为 HF_HOME。失败退出且错误含该模型名。然后 exec 现有 serve.py。把 melo/.src/ 与 melo/.cache/ 写入 .gitignore。
4. 不要改 find_interpreter，不要加长 1 秒观察窗，不要让 Mod 写 [tts]。

禁止：挂回 local-tts；碰 /home/skystar/CosyVoice 3.0 或 8080；复制 /home/skystar/tts/MeloTTS 进仓库；改 tts.rs；改 .env；整份改 live2d-ai.toml 或 mods.json；删 weights；提交 此项目的调研结果.md；改 docs/audit；升版本；打 tag；git commit；git push。

验收（都要真跑，贴命令输出）：
- ss 显示 127.0.0.1:8091 在听；GET /v1/models 为 200。
- curl POST http://127.0.0.1:8091/v1/audio/speech，JSON 为 input=你好、voice=ZH、response_format=pcm。HTTP 200，正文长度 > 1000 且为偶数。
- 无头浏览器打开 http://127.0.0.1:18080/app/，发一句短中文。确认 <audio> 的 duration > 0 且 currentTime 前进，口型有变化或 WS 音频帧 sample_rate=44100 且样本非空。日志里能看到这次 /v1/audio/speech。界面先显示运行中不算数：装包和载模型都超过 1 秒观察窗，要以端口和这次 POST 为准。
- cargo test -p live2d-ai-mod-local-tts 通过。
做不到就停在失败原话上（脚本输出、HTTP 体），不要改去云端，也不要报告成功。
```
