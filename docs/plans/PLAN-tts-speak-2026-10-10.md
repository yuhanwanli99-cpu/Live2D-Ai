# 本地出声接到本机：8091 真的应答

> 立档：2026-10-10。状态：**活**。真源：本文件。上一份 `docs/plans/PLAN-tts-choice-and-closeout-2026-10-09.md` 的界面、保存、档位已经在 `e2c3344` 推上 `origin/main`。本份只补它没达成的一句：本地模式要有进程在听 8091，并且一轮对话的原文打到 `POST /v1/audio/speech` 后，浏览器播出声音。

## 现状（2026-10-10 在本机核对）

对话到出声的请求已经接好，不要再改这条：

- `crates/live2d-ai-runtime/src/conversation/worker.rs` 在 `[tts].base_url` 非空时调用 `synthesize_speech`。
- `crates/live2d-ai-runtime/src/tts.rs` 发 `POST {base_url}/audio/speech`，体是 `input` / `voice` / `response_format`。本地配置是 `http://127.0.0.1:8091/v1`、音色 `ZH`、`pcm`。请求体不带 `sample_rate`，适配层因此不会因采样率拒掉。
- 播放在浏览器的 `<audio>` 上。`GET /api/v1/app/status` 里 `audio.backend = "none"`、`available = false` 是 `web_api/app_routes.rs` 写死的，意思是 Rust 进程自己不打开声卡。诊断里看到这行，不代表合成没接上。不要改这两个字段。

本机没有声音，是因为没有进程听 8091：

- 权重在 `crates/live2d-ai-mod-local-tts/melo/weights/`（`config.json` 的 `sampling_rate` 是 44100，`spk2id` 只有 `ZH`）。`checkpoint.pth` 约 199MB，已在磁盘上。
- `melo/.venv` 已存在，是 **Python 3.14.4**。`import melo` 得到 `ModuleNotFoundError`。
- `melo/start.sh` 看见 `.venv/bin/python` 可执行就跳过安装，直接 `exec serve.py`。所以界面上的 `last_error` 是「载入模型失败」，安装失败被盖住了。
- 系统 `python3` 只有 3.14。GitHub tag `v0.1.2` 的 `setup.py` 第 5 行是 `from pip.req import parse_requirements`，pip 10 起没有这个模块，`pip install git+https://…@v0.1.2` 在现代 pip 上必然失败。
- 本机另有 Python 3.10：`uv` 的 `cpython-3.10.21`（`~/.local/share/uv/python/cpython-3.10.21-linux-x86_64-gnu/bin/python`），以及 `~/miniconda3/envs/melo/bin/python`（3.10.20，里面没有 `melo` 包）。用它们当**创建 venv 的解释器**。不要把包装进那个 conda 环境。
- 仓库根磁盘余量约 21GB。Melo 在 `serve.py` 里固定 `device="cpu"`。torch 装 CPU 轮，不要装 CUDA 轮。

## 要改的

只动启动这一侧。

1. `melo/start.sh`：`.venv` 里的 python 不能 `import melo` 时，删掉这一个 `.venv`（它在 `.gitignore` 的 `.venv/` 下），用 Python 3.10 重建。找不到 3.10 就非 0 退出，原话写明系统 python3 是 3.14、v0.1.2 装不上，不要退回 3.14 再试一次。
2. 安装源仍是 GitHub tag `v0.1.2`。先克隆到 gitignore 的 `melo/.src/`（不要复制 `/home/skystar/tts/MeloTTS`，不要把这份源码提交）。若 `setup.py` 含 `from pip.req import`，只改这一处：删掉该 import 和 `parse_requirements` 两行，改成读 `requirements.txt` 的非空、非注释行。安装前把 `$VENV_DIR/bin` 放进 `PATH` 最前，这样钩子里的 `python -m unidic download` 打进 venv，而不是系统的 3.14。torch / torchaudio 用 PyTorch 的 CPU 索引先装，再装其余依赖。装不上就停，不重试，不改 `[tts]`。
3. `bert-base-multilingual-uncased` 仍不入库。启动前若 `melo/.cache` 里没有它，打一行 `[melo] 正在下载 bert-base-multilingual-uncased（不入库）` 再下载到这个目录（`HF_HOME`）。下不来就非 0 退出，错误里要有这个模型名。下载成功之后再拉起 `serve.py`。`.src/` 与 `.cache/` 写进 `.gitignore`。
4. 预检 `find_interpreter` 保持「只看文件在不在、不执行」。1 秒观察窗也不要加长：装依赖是分钟级，窗口内还活着就让 `start` 返回成功，这是现有口径。

## 不做

- 不把 `local-tts`（CosyVoice3）挂回注册表。不碰 `/home/skystar/CosyVoice 3.0`，不去拉 8080。
- 不改 `tts.rs`、分句、口型、播放器、`AudioSpec::DEFAULT_SAMPLE_RATE`。不改 `audio.backend` / `available`。
- Mod 仍不许 `apply_settings` 写 `[tts]`。不许重试、看门狗、失败后改走云端、静默成功。
- 不改 `.env`，不整份改 `live2d-ai.toml` / `mods.json`。不删 `melo/weights`。不提交 `此项目的调研结果.md`。
- 不修 `docs/audit/**` 的 S-000…S-007。不升版本，不打 tag，不提交，不推送。

## 验收

工作 agent 有无头浏览器。做到下面每一条才算完。`start` 返回或界面先显示「运行中」都不算，因为 1 秒窗口盖不住装包和载模型。

1. `ss` 上 `127.0.0.1:8091` 在听。`GET http://127.0.0.1:8091/v1/models` 为 200。
2. `curl` `POST /v1/audio/speech`，体为 `{"input":"你好","voice":"ZH","response_format":"pcm"}`，HTTP 200，正文长度大于 1000 且为偶数。
3. 浏览器打开 `http://127.0.0.1:18080/app/`，发一句短中文。页面上出现 `<audio>`，`duration > 0`，并且 `currentTime` 往前走。同一轮的口型有变化，或 WS 音频帧的 `sample_rate` 为 44100 且样本非空。服务端日志里有打到 8091 的 `/v1/audio/speech`。
4. 这一轮说出来的字就是发出去的原文。失败时 Mod 为 `failed`，`last_error` 带脚本原话（含安装失败或 BERT 模型名），界面不改回云端。
5. `cargo test -p live2d-ai-mod-local-tts` 通过。改了 shell 以外的 Rust 再跑 fmt 与 clippy。不要为了这次去跑会改写测试进程的整仓测试，除非动了 desktop。

标准命令仍是：Flutter 产物已在时，`./scripts/ignite.sh --build` 不会重编前端。本轮不改 Dart 就可以不重编。服务若已在 18080，启用 `local-tts-melo` 即可，不必为了换二进制重启，除非改了 Rust。
