# 执行单：弹幕只加前缀说明（2026-10-14）

> **不要执行。** 维护者收口：这不是第二轮。格式和清洗在仓外脚本；Mod 只收消息、注入、自动发送。下面正文作废。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。已有未提交改动留着，不要还原，不要另开分支。
> 仓外脚本：`/home/skystar/live2d-bili-sidecar/bilibili_sidecar.py`。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要点火，不要启停 18080。
> 不要 commit、tag、push，不要改版本号，不要提高 `DOCS_BUDGET_LINES`。
> 不要改 `docs/examples/bilibili-sidecar/`。不要恢复人设，不要写角色卡。

## 0. 维护者要的结果

弹幕原样进当前会话，只在前面加固定前缀 `弹幕消息：`。

一条正文是「你好」时，进会话的就是 `弹幕消息：你好`。不要用户名，不要 `*`，不要全角括号，不要 `观众`，不要把正文改写成引用。礼物同样没有用户名和星号：`弹幕消息：送出 {礼物名} x{数量}`。

这不是操作者在输入框打的那一句。历史里用已有的 `ChatRole.system`（标签「系统」），不要用 `ChatRole.user`（标签「你」）。助手回复仍接在后面。

模型怎么对待这句，只加在这一轮的提示词里，原句如下，不要改字、不要加性格：

`以「弹幕消息：」开头的内容是直播间弹幕，不是操作者本人说的话。`

普通对话不加这句。出厂人设仍是空的。

## 1. 仓外脚本

`danmaku_line` 改为前缀加弹幕原文。`gift_line` 改为上一节那一句。不要再调用 `display_name`。

`submit` 现在对所有正文跑 `clean_text`（压空白、去控制符）。这两类不要再跑它。正文里的字保持原样。只有超过现有 `MAX_TEXT_CHARS`（500）时，从尾部截断。`clean_text` 函数和它自己的自测留着，给别的路径用。

`--selftest` 改成断言新形状，并断言成品里没有 `*`、没有 `（）`。不要打印 cookie 或 token。

## 2. 屏幕

`user_line` 已经会把这一轮的原文送到页面。正文以 `弹幕消息：` 开头时：

- 追加一条 `ChatRole.system` 并走现有 `_persist()`
- 不要调用 `acceptInjectedUserTurn`（那会写成用户句，并把这一轮标成操作者在说）
- 同一句已经是最后一条时跳过，不要写两遍
- 已有一轮在飞时也不要 `_abandonTurn`，不要清助手气泡

输入框和语音仍走现在的用户句逻辑。判据放进 `turn_liveness.dart`。`chat_controller.dart` 现在 **798 行，不得超过 800**。

## 3. 提示词

在 `ConversationEngine::build_messages`（`crates/live2d-ai-runtime/src/conversation/engine.rs`，现 752 行，上限 1000）里：仅当本轮正文以 `弹幕消息：` 开头时，把第 0 节那一句接在**已经生效**的 system prompt 后面。原来没有 system 消息时，这一轮就只有这一句。不要写进 `live2d-ai.toml`，不要写进 persona / memory 的槽。

`empty_system_prompt_is_omitted_from_wire_messages` 用的是普通正文，必须仍然不带 system 消息。另加一条：`弹幕消息：你好` 的请求里有那句说明，正文仍是 `弹幕消息：你好`。

## 4. 验收

```bash
cd /home/skystar/live2d-bili-sidecar && .venv/bin/python bilibili_sidecar.py --selftest
cd /home/skystar/Live2D-Ai/shell/flutter && flutter test test/turn_liveness_test.dart test/ws_frame_test.dart
cargo test -p live2d-ai-runtime --lib empty_system_prompt_is_omitted_from_wire_messages
cargo test -p live2d-ai-runtime --lib danmaku_preface
test $(wc -l < shell/flutter/lib/chat/chat_controller.dart) -le 800
test $(wc -l < crates/live2d-ai-desktop/src/supervisor.rs) -le 1000
```

`danmaku_preface` 用你新加的测试名，退出码必须是 0。不要启动或关掉 18080。

## 5. 明确不做

不加过滤，不加用户名，不加星号，不加弹幕面板，不改外部端点字段，不改导演，不改人设卡。
