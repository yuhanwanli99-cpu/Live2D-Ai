# 执行单：弹幕按一句写进历史会话（2026-10-14）

> **2026-10-15 收口**：格式改由 [`PROMPT-stop-playback-and-danmaku-id-2026-10-15.md`](PROMPT-stop-playback-and-danmaku-id-2026-10-15.md) 执行。注入正文是 `id：内容`，探针关闭。本文件的格式段不要再执行。清洗仍在仓外脚本。Mod 只收 `text`、注入、自动发送。[`PROMPT-danmaku-preface-2026-10-14.md`](PROMPT-danmaku-preface-2026-10-14.md) **不要执行**。下面原文留着。

> 交给工作 agent。本单就是范围。不要再调研。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。已有未提交改动留着，不要还原，不要另开分支。
> 仓外脚本在 `/home/skystar/live2d-bili-sidecar/`，这一单可以改它。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要点火，不要启停 18080。
> 不要 commit、不要 tag、不要 push、不要改版本号。不要提高 `DOCS_BUDGET_LINES`。
> 不要改 `docs/examples/bilibili-sidecar/`（那份示例仍是旧的，这一单不修它）。

## 0. 维护者要的结果

一条弹幕进对话，历史会话里要有一句，形状固定为：

`弹幕消息：id：（内容）`

`id` 是 B 站给的用户名（打码也原样，例如 `吾***`）。`内容` 是这一条弹幕正文。括号用全角 `（）`。这句话就是喂给模型的那一句，也是历史里的用户气泡。角色的回复仍走现在的助手气泡，接在这句下面。

不要另做弹幕面板。不要改人设，不要写 system prompt。不要改外部端点的字段（仍然只有 `text`）。

## 1. 为什么现在历史里没有

历史只存在前端的会话存档里。输入框发送会自己追加用户气泡。语音是前端在 `ok:true` 之后调用 `ChatController.acceptInjectedUserTurn`（`shell_lifecycle_wiring.dart`）。

弹幕 sidecar 直接 `POST /api/v1/external/chat`，浏览器没参加这次发送，所以不会调用 `acceptInjectedUserTurn`。主链（LLM / TTS）已经跑了，历史里没有那句用户话。

## 2. 仓外脚本改格式

`/home/skystar/live2d-bili-sidecar/bilibili_sidecar.py`：

- 弹幕发：`弹幕消息：{用户名}：（{正文}）`
- 礼物发：`弹幕消息：{用户名}：（送出 {礼物名} x{数量}）`
- 用户名为空时用 `观众`
- 正文仍走现有 `clean_text`。不要再发旧的 `弹幕 {名}：{正文}`
- `--selftest` 断言这两句形状。其它坑（`msg`、只给本机带 Origin、心跳、行缓冲）保持

## 3. 服务端把这一句告诉当前页面

在 `say` 被循环取到、发出 `TurnPrompt` 的同一处（`supervisor.rs` 约 812 行附近），多广播一帧。正文就是这一轮的 `text`，与喂给模型的相同。

- 新帧类型 `user_line`，`data` 只有 `epoch` 和 `text`
- 加在 `ConversationUiEvent` 上，由 `app_event_to_ws_frame` 投影。未知类型旧页面会忽略，这是向后兼容的
- 这一帧必须在本轮任何 `text_delta` 之前发出
- `supervisor.rs` 现在 **1000 行，不得超过 1000**。多出来的行从旁边的注释里减掉，或把发射收成一行函数调用放到已有的 `handlers.rs`。不要把文件涨过 1000
- 聊天框自己发的、语音已经上屏的，也会走到这里。前端负责不去重画（下一节）

## 4. 前端写进当前会话并落盘

`ws_frame.dart` 解析 `user_line`。`chat_controller.dart` 现在 **797 行，不得超过 800**（Dart `lib >800` 棘轮是 0）。分支只留几行，判据放进已有的 `turn_liveness.dart` 纯函数。

收到 `user_line` 时：

- 没有一轮在飞：调用已有的 `acceptInjectedUserTurn`（上屏、落盘、把这轮标成进行中）
- 正在飞，且上一条用户句就是这句：跳过。这是输入框或语音已经写过的同一句
- 正在飞，且是另一句：只把这句追加成用户气泡并落盘。不要 `_abandonTurn`，不要清掉当前助手气泡

空文本不写。落盘走现有会话存档，刷新后还在当前这条历史里。

## 5. 验收

```bash
cd /home/skystar/live2d-bili-sidecar && .venv/bin/python bilibili_sidecar.py --selftest
cd /home/skystar/Live2D-Ai/shell/flutter && flutter test test/turn_liveness_test.dart test/ws_frame_test.dart
cargo test -p live2d-ai-desktop --bin live2d-ai-desktop user_line
cargo fmt --all -- --check
test $(wc -l < crates/live2d-ai-desktop/src/supervisor.rs) -le 1000
test $(wc -l < shell/flutter/lib/chat/chat_controller.dart) -le 800
cargo run -p xtask -- code-stats --check --only lines
```

`user_line` 的测试若还没有同名过滤器，就用你新加的测试名，退出码必须是 0。`>500` 仍是 44，`>1000` 仍是 0，Dart `lib >800` 仍是 0。

不要启动或关掉 18080。改动要等维护者自己重启后端并重新构建前端之后，历史里才会出现这句。

## 6. 明确不做

不改人设。不改表演。不改 `listen_port`。不把 B 站协议写进 Rust。不改仓库里的示例 sidecar。不提交。
