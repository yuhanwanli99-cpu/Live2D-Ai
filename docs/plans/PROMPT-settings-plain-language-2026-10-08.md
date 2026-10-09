# 用户看不懂的设置和常驻界面收成一句人话

> 2026-10-08。维护者已定口径。交给下一个 agent 实现。
> 不要提交、不要打 tag、不要推送、不要升版本。做完由维护者审查。
> 上一份 `docs/plans/PROMPT-tts-llm-clean-2026-10-08.md` 已经落地，本文件不要返工它。

## 给下一个 agent 的提示词

```
你在 /home/skystar/Live2D-Ai（分支 main）。按 docs/plans/PROMPT-settings-plain-language-2026-10-08.md 实现。不要提交、不要打 tag、不要推送、不要升版本。不要改 mods.json、.env、live2d-ai.toml。工作树里已有的未提交改动不要还原，只加本文件列出的界面改动。

本轮只改用户看得到的文案和「画不画」。不改对话、清洗、TTS、口型、动作幅度、导演 cue、分句、播放。配置键继续能解析；已经写进 mods.json 的值保存时原样保留，不要用默认值覆盖。藏起来的键用 ModPanel.hiddenKeys，不要收进仍能展开的「高级」。

产品面上不要再出现这些词，除非本文明确留在开发者模式里：epoch、schema、WS、base_url、sidecar、ASR、BCP-47、全局桶、手动闸、总闸、403、503、command_unavailable、Ollama、vLLM、api v、LIVE2D_AI_MUTE_AUDIO。失败按钮按下之后的错误句可以在句尾带一个错误码，闲置说明里不要带。

界面新文案必须过 font_subset_test。不要写子集外的字符，不要用 emoji。flutter analyze 和 flutter test 只在 shell/flutter 里跑，用 /home/skystar/flutter/bin/flutter。不要在仓库根目录跑 flutter analyze。

验收命令写在文末。
```

## 口径

| 项 | 定死 |
| --- | --- |
| 谁看得到 | 没开开发者模式时，用户只看到能直接使用的句子和开关 |
| 开发者模式 | 分区本身永远留在导航里。开关在这个分区里，藏掉分区会再次打不开 |
| 诊断 | 没开开发者模式时，导航里没有「诊断」。打开之后才出现，内容保持现在的只读快照 |
| 藏配置 | `hiddenKeys`：不渲染，也不进「高级」。保存时跳过这些键 |
| 能力不删 | 听、角色卡、记忆列表、弹幕接入、音量、静音、动作幅度、皮套导入，都还在。拿掉的是第二套入口和协议说明 |

## 已经落地、不要动

`docs/plans/PROMPT-tts-llm-clean-2026-10-08.md` 的实现已核对过，本轮不要改这些行为：

- 一路请求体恒带 `thinking.type`，缺省 `disabled`，设置里的「思考」打开后为 `enabled`。二路清洗和 `staging_http` 恒为 `disabled`。
- `show_reasoning` 同时是请求开关和思考帧是否上屏的闸。不要拆回「关掉只是不显示、思考照样生成」。`settings.rs` 里如果还写着后一句，改成与请求一致，不要改字段名。
- 一句闭合后走二路 `{display, speech}`。上屏用 `display`，TTS 用 `speech`，同一个 `sentence_seq`。失败两份都用原文。不要新正则，不要扩展 `clean_for_tts`。
- 整段表演层只在 `[performance].enabled = true` 时装配。缺省走按句清洗。导演 Mod 仍靠 `SentenceReady` 的规则层出 cue。不要改回「导演已启用就等整段 JSON」。
- 导演卡片只留启用开关和「打开后，说话时会带上表情和轻微的头、颈动作。」8 个键继续 `hiddenKeys`。
- 外观里的动作幅度滑条留着。播放仍是 `<audio>`，不要改 `sentence_assembler.dart` / `audio_player.dart`。
- `LlmSettings.model` 的 serde 缺省仍是空串。example 和界面 hint 已是 `deepseek-flash`。本轮不要改这个缺省。

## 导航和常驻界面

| 位置 | 现在 | 怎么改 |
| --- | --- | --- |
| `shell/flutter/lib/settings/settings_sections.dart` 的 `LLM` | 标题是内部名。说明是「对话模型的服务地址、模型名与密钥」 | 标题改成「对话」。说明改成「服务地址、模型名和密钥」。说明里不要包含标题这两个字 |
| 同上，`Mod` | 「扩展模块的启停与配置」 | 标题改成「扩展」。说明改成「额外能力的开关」 |
| 同上，`诊断` | 说明写「连接、延迟、帧率与日志」。分区里没有延迟，也没有帧率 | 说明改成「连接和日志」。`visibleSections` 增加参数：开发者模式关时去掉这一项，开时留着。`开发模式` 在两种情况下都在。当前打开的分区如果是诊断、用户又关掉开发者模式，回到「人设」 |
| 同上，`语音合成` 的说明 | 带「TTS」 | 改成「音色和出声的服务」 |
| `llm_section.dart` 分区头 | 「OpenAI 兼容端点。本地 Ollama / vLLM 与云端服务同一套配置。」 | 改成「对话用 DeepSeek。地址和密钥要换的话在这里改。」服务地址、模型名、输出上限、「思考」、「谁负责动作」、密钥输入都留着 |
| `tts_section.dart` 分区头 | 「TTS 上游…TTS 是核心链路，不是 Mod。」 | 改成「角色出声用的服务和音色。」 |
| 同上，「服务端静音」只读行 | 平时也显示环境变量名 | 只在开发者模式里画。聊天栏上的静音，以及真的被服务端静音时音频条那句「服务端静音中」，都留着。不要把它做成开关 |
| `state_pill.dart` 与 `ui_phase.dart` 的 `UiPhase.thinking` | 两处都写「思考中」。这个相位是「这轮已开始、还没出声」，不是「思考」开关 | 两处一起改成「正在回复」。气泡里思考折叠的「思考中…（N 字）」不动，那一句只在真有思考文本时出现 |
| 同上，`offline` | 「后端未连接」 | 改成「没连上」 |
| `ws_status.dart` 的 `description` 和 `connection_badge.dart` 的语义标签 | 「实时通道」 | 改成「连接」。徽标上已经有的「未连接 / 连接中 / 已连接 / 重连中 / 已关闭」不动 |
| `dev_tools_section.dart` 模型行的徽章 | 「含物理」「有显示配置」 | 非开发者模式不画。空列表提示里的 `assets/models/<id>/` 改成「在下面填皮套的文件夹名。仓库不自带皮套。」导入框和「激活」留着 |
| `dev_tools_mods.dart`、`dev_tools_mod_config.dart` 的卡片副标题 | `id · v版本 · api vN` | 非开发者模式不画。卡片名和启用开关留着 |

`main.dart` 把当前的开发者模式传进 `visibleSections`。chip 按分区枚举辨认，不要改成按下标硬编码。

## 扩展卡片

每张卡片的启用开关都留着。下面的自定义面板收成一句。通用表单里点名的键全部放进该面板的 `hiddenKeys`。

### 人设

两处都能导入角色卡：`persona_section.dart`（粘贴 JSON、按会话或全局导入）和 `persona_panel.dart`（PNG、清除卡、再讲一遍导入）。只留人设分区那一处。

- PNG 选择和「清除当前会话的卡」「清除全局导入卡」如果只有 Mod 面板有，原样搬到人设分区。不要再复制一套 JSON 粘贴框。
- Mod 卡片只留：「打开后，用导入的角色卡说话。关掉会回到「人设」里写的提示词。」
- `hiddenKeys` 与 `crates/live2d-ai-mod-persona/src/factory.rs` 逐键对应，一个不要漏：`card_path`、`card_json`、`include_discipline`、`say_first_mes`、`name`、`description`、`personality`、`scenario`。

### 语音

聊天栏的「听」留着。唤醒词缺省仍是「小可爱」。

- 主区删掉：总闸/手动闸状态行、`wake_phrase` / `403` / `503 command_unavailable` 闲置说明、整块「高级」（手动注入、sidecar 拉起、退出码表）。
- 留一句：「点聊天栏的「听」。说唤醒词，再把想说的话说完。」
- 表单只留两个键，并改写 `voice_input_settings_spec` 里的 label：`wake_phrase` → 「唤醒词」，说明「听到这几个字，才把后面的话当成人说的。留空等于不听。」；`manual_enabled` → 「按住说话」，说明「打开后，按住「听」可以直接说，不必先说唤醒词。」
- `hiddenKeys`：`backend`、`locale`、`token`、`sidecar_script`、`sidecar_url`、`sidecar_transcriber`、`sidecar_python`。从 `advancedKeys` 拿掉，不要改成折叠。

### 记忆

列表、导入一条、编辑、删除、清空留着。这些是用户会用的。

- 概览那一行不要再写命中、注入、淘汰、上轮命中。改成「记住了 N 条」。N 用现有的 `records`。
- `memoryBucketNotice`：没有会话时写「还没有对话。记住的内容先放到一起，开始聊天后再按这次对话分开。」有会话时写「只看这次对话记住的内容。」不要出现「桶」。
- 摘要块：有摘要正文时，标题用「压缩过的记忆」，只显示正文。不要「回滚上一版摘要」，不要版本号和「注入」字样。没有摘要就不画这块。
- 表单只留 `enabled_injection`，label 改成「聊天时用上这些记忆」，说明「关掉后仍会记住，但不会塞进角色的提示词。」
- 其余键全部 `hiddenKeys`：`store_path`、`top_k`、`max_records`、`summary_enabled`、`summary_base_url`、`summary_model`、`summary_api_key_env`、`summary_timeout_ms`、`summary_ratio`、`summary_cooldown_turns`、`summary_keep_recent_turns`。
- 删掉面板上指向 `docs/architecture/memory-mod-v0.md` 的句子。常量可以留在代码里，产品界面不要显示路径。

### 外部事件

缺省启用的直播接入留着，不要从注册表拿掉。

- 面板删掉：测试注入、真 HTTP 等价、计数、重置计数、模板怎么写、端口和令牌的长说明。
- 只留一句：「打开后，直播间的弹幕和礼物会说给角色听。」
- `hiddenKeys`：`listen_port`、`token`、`text_template`、`prefix`。`listen_port` 的现文案已经写明它不改真实端口，不要再画。

### 导演

不要改。回归仍要过 `director_panel_test.dart`。

## 禁止

- 还原或重做二路清洗、思考开关、导演 8 个隐藏键、表演层 opt-in。
- 删「听」、音量、聊天栏静音、动作幅度、人设提示词、记住几轮、皮套导入、背景和口型。
- 把藏起来的键放进「高级」折叠。
- 藏「开发模式」这个分区。
- 新的 WS 帧、接回 `action_tx`、新的清洗正则。
- 改 `mods.json`、`.env`、`live2d-ai.toml`。
- 升版本、提交、打 tag、推送。
- 在仓库根目录跑 `flutter analyze`。

## 验收

贴出每条最后 30 行。断言改成「产品卡片上找不到这些字符串」。开发者模式打开时，诊断分区和「服务端静音」只读行必须还在。关掉开发者模式时，导航找不到「诊断」，找得到「开发模式」「对话」「扩展」。

```
cd shell/flutter && /home/skystar/flutter/bin/flutter test \
  test/settings_sections_test.dart \
  test/director_panel_test.dart \
  test/voice_input_panel_test.dart \
  test/memory_panel_test.dart \
  test/external_input_panel_test.dart \
  test/persona_panel_test.dart \
  test/ui_state_tracker_local_failure_test.dart \
  test/message_bubble_test.dart \
  test/asset_guard_tts_config_test.dart \
  test/audio_bar_test.dart \
  test/font_subset_test.dart
cd shell/flutter && /home/skystar/flutter/bin/flutter analyze
```

人设测试文件名以 `shell/flutter/test/` 里实际文件为准。改了语音或记忆的 spec label 时，再跑：

```
cargo test -p live2d-ai-mod-voice-input
cargo test -p live2d-ai-mod-memory
cargo test -p live2d-ai-mod-persona
```

没有改 Rust 行为就不要为了凑数跑全量 workspace。
