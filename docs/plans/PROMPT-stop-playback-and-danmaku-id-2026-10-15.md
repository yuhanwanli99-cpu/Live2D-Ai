# 执行单：停止播放 + 弹幕只按 id：内容注入（2026-10-15）

> 交给工作 agent。本单就是范围。不要再调研，不要再拆一轮。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。已有未提交改动留着，不要还原，不要另开分支。
> 仓外脚本：`/home/skystar/live2d-bili-sidecar/bilibili_sidecar.py`（及同目录 README，只改和本单冲突的句子）。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要改 `docs/examples/bilibili-sidecar/`。
> 不要 commit、tag、push，不要改版本号，不要提高 `DOCS_BUDGET_LINES`。
> 不要恢复人设，不要写 system prompt，不要改外部端点字段（仍然只有 `text`）。
> 旧单 [`PROMPT-danmaku-history-line-2026-10-14.md`](PROMPT-danmaku-history-line-2026-10-14.md) 的格式段、[`PROMPT-danmaku-preface-2026-10-14.md`](PROMPT-danmaku-preface-2026-10-14.md) **不要执行**。`user_line` 已在树上的实现留着。

## 0. 维护者要的两件事

1. 仓外脚本只做一件事：接收 B 站房间弹幕，按 `id：内容` 打进 `POST /api/v1/external/chat`。探针关掉，不许再往对话里送 `sidecar-probe-…`。
2. 前端给一颗统一的「停止」按钮。按下丢掉还没播完的 TTS 队列和 Live2D 表演队列；这一轮还在生成时，同时走现有的停止。这颗按钮不删会话、不清历史。

做完必须由工作 agent 自己构建前端和后端再交付。不要把 `flutter` / `cargo` / `ignite.sh` 三个命令交回维护者。

## 1. 仓外脚本

文件：`/home/skystar/live2d-bili-sidecar/bilibili_sidecar.py`。

注入正文只有这一种形状，全角冒号：

`{id}：{内容}`

- `id` 用弹幕消息上的 `uname`。B 站自己打的码原样留下，脚本不再加 `*`。
- `uname` 为空时用十进制 `uid`。两个都空则跳过，不发明「观众」，不发送。
- `内容` 用现有 `danmaku_text`（先 `msg` 再 `content`）。不跑 `clean_text`。超过 500 字仍从尾部截断。
- 不要前缀 `弹幕消息：`，不要全角括号，不要用户名再包一层。
- 礼物若仍收到：同一形状，`内容` 为 `送出 {礼物名} x{数量}`，`id` 规则相同。不另做一种格式。
- 删掉 `--probe` 和 `probe()`。命令行再出现 `--probe` 时直接退出码 2，不访问 18080，不 POST。
- `--selftest` 仍只离线。断言新形状；断言源码里不再有 `sidecar-probe` 和 `def probe`。
- 心跳、行缓冲、只给本机请求带 Origin、`msg` 优先、节流、`BILI_ROOM_ID` 缺失则退出码 2，都保持。
- 本单不要启动直播监听，不要对正在跑的服务 POST 任何句子。

## 2. 统一停止

现状：输入框圆按钮只在「思考中 / 说话中」变成「停止本轮」，并调用 `_stopWithCancellation`（`shell/flutter/lib/app/shell_cue_voice_wiring.dart`）。那条路径已经会清 cue 计划、撤销两个槽、`AudioPlayer.interrupt()`、回待机，再 `POST /api/v1/chat/stop`。

缺口：服务端把 PCM 推完、相位离开「说话中」之后，按钮变回「发送」，浏览器里排队的句子和还没演完的动作没有入口。删会话 / 清空会话只走 `_abandonTurn`，不调 `/stop`，也不丢队列。不要把停止绑到删会话上。

要做的：

- 输入区增加一颗独立的「停止」按钮，不替换发送按钮。文案用「停止」，图标用已有的停止图标。
- 可按的条件（纯函数，放在 VM 能测的文件里，例如 `turn_liveness.dart` 旁或 `live2d/stage_cancel.dart` 同库的新小函数）：这一轮还在飞，或 `AudioPlayer` 队列非空，或 `DirectorCuePlan.length > 0`。三者都没有时按钮不可按。
- `AudioPlayer` 增加只读 `hasPending`，读现有 `SerialQueue.isEmpty` 的反面。不要改播放、口型、WAV。
- 按下时：
  - 永远先走现有 `StageCancellation.cancel('stop')`（清表演计划、撤销两槽、丢掉 TTS 队列和半截攒句、回待机）。
  - 仅当这一轮还在飞时，再走现有 `ChatController.stop()`（本地收口 + `POST /api/v1/chat/stop`）。
  - 没有在飞的轮次时，不调用 `_finishTurn`，不追加「已停止本轮」，不删消息，不切会话。
- `/chat/stop` 在空闲时本来就幂等。没有在飞的轮次时不要为了丢队列再打一次 stop，除非现有 `stop()` 不先收口就无法拆开；能拆开就不打。
- 不新增 WS 帧，不改 `supervisor.rs`（该文件在 1000 行顶上），不改 Rust。后端停止语义已经够用。

测试：纯函数三条（只在飞 / 只剩音频 / 什么都没有）。再一条：空闲时按下停止不调用删会话、不追加系统行。`flutter analyze` 与相关 `flutter test` 必须过。

## 3. 交付时自己构建

工作结束、测试过了之后，由工作 agent 在仓库根执行。`flutter` 不在默认 `PATH` 上，必须先放进去：

```bash
export PATH="$HOME/flutter/bin:$PATH"
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY all_proxy
(cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
cargo build -p live2d-ai-desktop
```

本单不改 Rust，所以不要杀、不要重启已经在听的 18080，不要再跑 `ignite.sh`。前端产物由正在跑的服务按请求读盘，刷新 `http://127.0.0.1:18080/app/` 即是新界面。

完工报告里用单独一节写明，不要只说「请维护者自己构建」：

- 前端：命令、退出码、`shell/flutter/build/web` 是否写出
- 后端：`cargo build -p live2d-ai-desktop` 的退出码；本单未改 Rust，18080 未重启
- 维护者要做的只剩刷新页面

构建失败就在报告里贴失败命令和最后几行，不要假装交付完成。
