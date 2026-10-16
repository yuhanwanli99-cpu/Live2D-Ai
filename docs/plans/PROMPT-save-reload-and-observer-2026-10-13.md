# 执行单：保存就热重载，长数字不要打死整轮，事件流改成能读（2026-10-13）

> 交给工作 agent。本单就是范围。不要再调研，不要扩大。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。工作区里已有未提交的点火等待 / 表演倾向，留着，不要还原，不要另开分支。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要启动或杀掉 `ignite.sh` / `live2d-ai-desktop`（18080 上有用户进程）。
> 不要 commit、不要 tag、不要 push、不要改版本号。不要提高 `DOCS_BUDGET_LINES`。不要改 `CHANGELOG.md`、发布说明、`docs/audit/**`。
> 不要新增 WS 帧类型。不要动 `CLEAN_SYSTEM_PROMPT`、思考关断请求体、`[performance].enabled` 缺省、`action_tx`、`mod_count_is_six`、cue 的 2000ms。
> 界面文案只用字体子集里已有的字。`font_subset_test` 变红时改文案，不要重新生成字体。
> `director_observer_section.dart` 现 666 行，`render_events.dart` 现 700 行。改完各自必须 ≤800。新的 Dart 库文件同样 ≤800。Rust 新 `src` 文件 ≤500；`supervisor.rs` 已是 1000 行，能不改就不要加行，要加就先把等量的行挪出去。

## 0. 做完是什么样

1. 一句台词里出现 17 位及以上的连续数字时，送去 TTS 的那份把这段换成「一长串数字」，屏幕上仍是原来那一句。本地 TTS 再回 `超出数据范围，最长支持 16 位` 时，这一句改走静音，后面的句子继续，整轮不失败。同一轮仍然只有一个助手气泡。
2. 导演卡片上改「表演倾向」，点卡片里的「保存并应用」，或点设置底部的「保存并重载」，都写进 `mods.json`，并立刻重建按句清洗器。日志里 `bias=` 是新档。进程不退出，本地 TTS 不被杀掉。页面只刷新一次。字段标签改为「表演倾向（保存并重载后生效）」。
3. 开发工具里的「B 事件流」默认不再被口型时钟淹没。过滤芯片是中文加英文线名。每一行换行，点一行能在原处看全文本。不靠左右滚动。

## 1. 已经核对过的事实

| 事实 | 证据 |
| --- | --- |
| 今晚两轮对话都死在 TTS 400，正文是 `超出数据范围，最长支持 16 位` | `~/.local/share/live2d-ai/logs/live2d-ai.log.2026-10-13` 的 12:50:10 与 12:54:16 |
| 第一轮送去合成的句子里有 `7182818284590452353602874713527`（31 位） | 同日志 `sentence_seq=14` |
| 失败轮会再发整段 `text_fallback`。前端 `_applyTextFallback` 是覆盖同一气泡；若此时 `_assistant` 已经是 null，会再 `sessions.append` 出一个气泡 | `shell/flutter/lib/chat/chat_controller.dart` |
| 底部「保存并重载」写的是 `live2d-ai.toml` 和本机显示偏好，不写 Mod 配置 | `shell/flutter/lib/app/shell_settings.dart` |
| 卡片里的「保存并应用」会 `POST /api/v1/mods/director/config`（今晚 12:55:06 回了 202），然后只重启导演 Mod，不重建清洗器 | `mods_routes.rs` 的 config 分支只 `spawn_mod_op(Restart)`。磁盘上 `mods.json` 已是 `motion_bias=eager`，12:55 之后没有新的 `二路按句清洗已装配` |
| 清洗器只在 supervisor 重载时读 `mods.director.config.motion_bias` | `performance_wiring.rs` `from_manifest`，`supervisor.rs` `build_sentence_cleaner` |
| B 栏是开发模式环形缓冲：前端已收 WS 帧 + 渲染面 ack，上限 200，一行省略号，`itemExtent: 20` | `director_observer_section.dart` `_eventsBlock`。它不是第二路模型的决策原文。口型时钟 `stage-clock` 约 30ms 一条，会把动作帧挤出去 |
| 预设中文名已在 `assets/actions/preset_labels.json` 的 `zh` | 事件流显示动作用这份，不要新写一套译名 |

## 2. 任务

### T1. 长数字与单气泡

- 只改送 TTS 的文本。屏幕上的那一句保持原样，仍必须是一路原文的连续切片。
- 连续数字（`0-9`，中间没有别的字符）长度 ≥ 17 时，speech 里整段换成「一长串数字」。短于 17 的不动。不要用正则去删表情符号。
- 本地 TTS 若仍回 HTTP 400 且响应体含 `最长支持 16 位`：这一句走已有的静音句路径，轮次继续。其它 TTS 错误仍是致命。
- 前端：`text_fallback` 到达时，若本轮气泡已经收口，就写回那一个气泡并标「未收尾」，不要再 append 第二个助手气泡。
- 测试至少三条：31 位数字的 speech 不含这 31 位、display 仍含这 31 位；该 400 不把轮次打成 Failed；同一 epoch 先收口再来 fallback，会话里仍只有一个助手消息。

### T2. 保存并热重载

- 选热重载，不重启进程。`POST /api/v1/mods/director/config` 落盘成功后，调用已有的 `supervisor.reload()`。重载日志要带新的 `bias=`。导演 Mod 自己的 restart 可以保留。
- 设置底部「保存并重载」在写 toml / 显示偏好之外，把导演表单里尚未提交的配置一起提交（含 `motion_bias`）。两次保存不要各刷一次页面，合并成一次刷新。
- `setting_field()` 的 label 改为 `表演倾向（保存并重载后生效）`。缺省仍是 `normal`。三档值 `restrained` / `normal` / `eager` 不变。
- `hiddenKeys` 仍是原来 8 个，不要把 `motion_bias` 藏进去。
- 不要手改仓库里的 `mods.json`。用户机器上那份已经是 `eager`，下一轮重载会自己读到。

### T3. B 事件流

保持开发模式、不落盘、不新增 WS 帧。改展示：

- 区块标题下加一句：`这里是本轮交给舞台和页面的帧。动作看「动作」；口型时钟默认不列出。`
- 过滤芯片显示中文，括号里留线名。至少这些：动作（action_cue）、正文（text_delta）、未收尾（text_fallback）、出错（error）、预设生效（preset-applied）、预设丢弃（preset-dropped）、段结束（segment-ended）、口型时钟（stage-clock）。未知类型仍用线名。
- 默认勾选除外的是口型时钟和段结束。用户点芯片可以再打开它们。「全部类型」改成「当前：默认」或列出正在看的中文名。
- 列表行换行，删掉 `maxLines: 1`、`TextOverflow.ellipsis`、固定 `itemExtent: 20`。列表高度仍约 240。
- 点一行，在列表下方展开该条全文（可再次点收起）。不新开路由，不做左右滚动。
- `action_cue` 那一行要把 `preset_id` 显示成 `preset_labels.json` 的 `zh`，旁边保留 id。没有中文名就只显示 id。
- 单测改在 `test/director_observer_test.dart` 与 `test/render_events_test.dart`：默认过滤不含 `stage-clock`；一行中文芯片含线名；点按记录能拿到全文。`flutter test` 跑这两个再加 `test/font_subset_test.dart`。

## 3. 验收

不要跑全仓，不要点火。代理环境变量按仓库惯例卸掉再测：

```bash
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo test -p live2d-ai-runtime --lib long_digit
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo test -p live2d-ai-desktop --bin live2d-ai-desktop motion_bias
cd shell/flutter && flutter test test/director_observer_test.dart test/render_events_test.dart test/font_subset_test.dart test/director_panel_test.dart
```

过滤名可以对上你新加的测试。回报：改了哪些文件、上面每条命令的退出码、`supervisor.rs` 的 `wc -l`。没跑的写明没跑。

## 4. 明确不做

不改嘴型曲线，不改分句规则，不把用户输入拿去打情绪，不合并 core 动作层，不重写 A/C/D 三栏，不把事件流落盘。
