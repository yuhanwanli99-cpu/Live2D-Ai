# 执行单：下划线照样念，一句合成失败不掐后文，碎片句不单独念（2026-10-15）

> 交给工作 agent。本单就是范围。不要再调研，不要扩大。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。已有未提交改动留着，不要还原，不要另开分支。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要改仓外脚本，不要改 `docs/examples/bilibili-sidecar/`。
> 不要 commit、tag、push，不要改版本号，不要提高 `DOCS_BUDGET_LINES`。
> 不要改 `CHANGELOG.md`、发布说明、`docs/audit/**`、`AGENTS.md`。
> 不要动 `supervisor.rs`（1000 行顶上）、导演 Mod、`presets.json`、渲染面、Flutter。
> 不要复活 `action_tx`。不要新增 WS 帧。不要改 `is_terminator` / 换行句界。
> 不要改 `CLEAN_SYSTEM_PROMPT`、思考关断、`[performance].enabled`、cue 的 2000ms、`mod_count_is_six`。
> 不要改 `.venv` 里的 MeloTTS。不要重试、不要换端点、不要把本机 TTS 切到云端。
> [`PROMPT-choppy-tts-and-eager-motion-2026-10-13.md`](PROMPT-choppy-tts-and-eager-motion-2026-10-13.md) 不要整份执行。它的 T1、T2 在本单。它的 T3、T4 不做。
> 用户名星号不讨论、不改。

## 0. 三件事

今天 Melo 对含 `_` 的句子回 500 `AssertionError()`。`conversation/worker.rs` 把这次合成失败当成 worker 死亡，`engine.rs` 里 `PushOutcome::QueueDead` 就 `break 'llm`，后文不再生成。点火终端还会把同一句再打出来，列表序号 `1.` 会单独送去合成。

做完是：

1. 送进 Melo 的文本里 ASCII `_` 被删掉再合成。屏幕上的字不动。`御寒_v4p` 念成「御寒v4p」，不再 500。
2. 某一句的 TTS 传输失败、上游 4xx/5xx、或这一句的 PCM 解码失败：这一句走已有静音句（一个字都不念、边界帧还在、正文照上屏），worker 不退出，后面的句子继续合成。轮次不以 Failed 收尾。日志仍打原来的 `code=`。
3. 点火终端不再 `print!` 模型原文。Melo 的 `tts_to_file` 传 `quiet=True`。纯列表序号和洗完没有可念文字的标记句不发 TTS。

## 1. 垫片：删下划线，合成时安静

文件：`crates/live2d-ai-mod-local-tts/melo/serve.py`（177 行）。

在 `synth` 调用 `tts_to_file` 之前，只删 U+005F `_`。别的字符不动，不要换成 `-` 或空格。删完 `strip` 为空就走现有的 `ValueError("input 为空")`。

`tts_to_file` 增加 `quiet=True`。不要改权重、端口、音色、采样率。

纯函数放在这个文件模块级，例如 `melo_speakable`，便于不加载模型就测。仓库根 pytest 加一个小测试，用文件路径加载这个函数，不要 `import melo`：`御寒_v4p` → `御寒v4p`，`a_b` → `ab`，`喵_喵` → `喵喵`，`___` → 空串，`你好。` 与 `#测试` 原样。

## 2. 一句失败，轮次继续

真源：`crates/live2d-ai-runtime/src/conversation/worker.rs`（435 行，上限 500）。`engine.rs` 的 `QueueDead` / `break 'llm` 不要改：worker 不退出，这条就走不到。

现在 `SynthOutcome::Failed` 会发 `EngineEvent::Error` 并 `return WorkerExit::Failed`。单句合成失败改成不走这条。

判定放纯函数，建议新文件 `crates/live2d-ai-runtime/src/conversation/speakable.rs`（≤500），由 `conversation/mod.rs` 引入。`worker.rs` 只调用。`cleaning.rs` 已 480 行，不要塞进去。

`synthesize_speech` 或这一句解码返回的 `Error`：

| 变体 | 轮次 |
| --- | --- |
| `Status`（任何状态码，含 500 和「input 为空」的 400） | 继续 |
| `Http`（传输，含连不上 8091） | 继续 |
| `TruncatedPcm` / `UnsupportedFormat` / `InvalidAudioConfig` | 继续 |
| `InvalidBaseUrl` / `Sse` / `Json` | 仍是现在的致命：发 `Error`，worker 退出，轮次 Failed |

继续的做法：

- 这一句还没发出过 `AudioChunk`：打 `tracing::error!`，`code` 用现成 `ErrorKind::Tts` / `Decode` 的 `code()`，然后走现有 `silent_sentence`。不要发 `EngineEvent::Error`。
- 这一句已经发出过音频块：同样只打日志，用空尾块把 `final_chunk` 补上，再 `SentenceVoiced`。不要再发一个 `first_chunk`。不要第二次 HTTP。
- 「最长支持 16 位」的 400 仍走现有静音句和现有 `tts_upstream_400_digit_range` 警告，不要并进上面的 error 日志。

`is_fatal()` 不要改成 false。背压超时仍致命，取消仍是取消。`conversation/mod.rs` 里 `ErrorKind` 上方那四行注释改成与上表一致（该文件 623 行，只动这几行）。`worker.rs` 里「别的 400 仍一律致命」的测试注释作废：空 input 的 400 现在是继续；`is_tts_length_rejection` 对它仍返回 false。补断言：500 `Status`、`Http`、`TruncatedPcm` 为继续；`InvalidBaseUrl` 为致命。

## 3. 终端不回显，碎片句不发 TTS

`handlers.rs`（387 行）`EngineEvent::TextDelta` 里删掉 `print!("{text}")` 和那次 `flush`。错误路径的 `println!` 不动。上屏仍只走 `SentenceVoiced`，注释留着。不要改 `conversation/mod.rs` 文档示例里的 `print!("{text}")`。

碎片句在发 HTTP 之前判定，输入是 `speech_for_tts` 之后的那份，不是屏幕上那份。整段去掉首尾空白后：

- 空，或只由 `*`、`#`、反引号组成 → 静音句。
- 整段是「一位以上 ASCII 数字 + 一个英文句点」→ 静音句。`1.`、`  2.` 命中。`1. 你好`、`12.3`、`a_b` 不命中，照常合成。

不要按字符位置硬切。屏幕上的字不动。

## 4. 验收和交付

代理先卸掉：`http_proxy` `https_proxy` `HTTP_PROXY` `HTTPS_PROXY` `ALL_PROXY` `all_proxy`。`flutter` 在 `$HOME/flutter/bin`，本单不用它。

```bash
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo test -p live2d-ai-runtime --lib speakable
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo test -p live2d-ai-runtime --lib long_digit
python3 -m pytest tests/melo_speakable_test.py -q
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo build -p live2d-ai-desktop
```

测试名可以对上新名字。`worker.rs` 改完仍 ≤500，`cargo fmt --all -- --check` 与 `cargo clippy -p live2d-ai-runtime --all-targets -- -D warnings` 必须过。不要跑全仓，不要 trunk，不要编 Flutter。

本单改了桌面二进制，也改了正在跑的 `serve.py`。18080 上若已有 `live2d-ai-desktop`，停掉这一个进程后再 `./scripts/ignite.sh --build`，让新二进制和新的 Melo 起来。不要开第二个。等 ignite 自己打出页面地址。构建或点火失败就贴命令和最后几行。

回报写明：改了哪些文件、上面每条命令的退出码、18080 是否已换成新进程。

## 5. 明确不做

不改动作曲线、到期立即撤销、表情增益、主动档补点头。不改点火脚本的 Flutter 重编。不改 `[tts].channels`。不删 `mods.json` 里的 `local-tts` 警告。Melo 冷启动那几十秒仍由 ignite 等待，sidecar 提前投递时这一句静音、后文继续，不要在垫片里排队等模型。
