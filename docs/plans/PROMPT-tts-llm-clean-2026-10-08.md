# 台词交给快速模型清洗后再送 TTS · 用户看不懂的导演设置收掉

> 2026-10-08。维护者已定口径。交给下一个 agent 实现。
> 不要提交、不要打 tag、不要推送、不要升版本。做完由维护者审查。

## 给下一个 agent 的提示词

```
你在 /home/skystar/Live2D-Ai（分支 main）。按 docs/plans/PROMPT-tts-llm-clean-2026-10-08.md 实现。不要提交、不要打 tag、不要推送、不要升版本。不要改 mods.json、.env。live2d-ai.toml 如果已经是 deepseek-flash 且 base_url 已是 https://api.deepseek.com/v1，不要改它。

模型定死 DeepSeek 官方 V4.1 Flash。请求里的 model 字符串是 deepseek-flash（官方文档：https://api-docs.deepseek.com/news/news260910 ，2026-09-10）。不要换成 deepseek-v4-pro，也不要新造一个模型名。deepseek-v4-flash 只是兼容别名，代码缺省写 deepseek-flash。

一路（写台词的对话模型）和二路（清洗并决定送什么给 TTS 的模型）都用这同一个模型。两路请求都默认关闭思考。官方 Chat Completions 的思考缺省是开的，effort 缺省 high；不写字段就还会想。OpenAI 兼容体必须显式带 thinking: { "type": "disabled" }。见 https://api-docs.deepseek.com/guides/thinking_mode 。不要用 Anthropic 的 reasoning.effort。

不要为了一路或二路的 token 出来有多快、token 速率、首 token 时间写任何优化、预算、探针或重试。关思考是开关，不是延迟课题。

不要新写正则去剥 emoji、颜文字或「不像台词的东西」。现有 clean_for_tts 不要扩展。清洗和「这段话里哪一部分拿去合成」交给二路模型。

播放不要等整段回复都合成完再开始。某一句的可念文本一到就提交 TTS，这一句的音频一到就开始播，并在同一时刻把这一句的原文（含 emoji、颜文字、括号里的字）上屏。后面的句子还在清洗或合成，不得挡住这一句。播放仍走 <audio> 媒体元素，不要改回 AudioContext。不要把一句再切成碎片重播。

用户能看见的、意义不明的导演设置按本文「设置」一节从产品面上拿掉。外观里的动作幅度滑条留着。

验收命令写在文末。界面文案必须过 font_subset_test，不要写子集外的字符。
```

## 口径

| 项 | 定死 |
| --- | --- |
| 模型 | 一路、二路都是 `deepseek-flash`（DeepSeek-V4.1-Flash）。base_url 用现有对话模型那一个（本机已是 `https://api.deepseek.com/v1`） |
| 思考 | 两路请求体都带 `thinking.type = disabled`。缺省就是关。不写这个字段等于上游继续想 |
| 延迟 | 不测量、不优化一路或二路的 token 延迟和 token 速率 |
| 清洗 | 二路模型做。禁止新正则、禁止扩展 `clean_for_tts` |
| 上屏 | 这一句的原文，😄、颜文字、`（挥手）` 这类都留着 |
| 送 TTS | 只有二路交回的可念文本 |
| 对齐 | 同一 `sentence_seq`。这一句开始出声时，上屏这一句原文 |
| 速度 | 禁止等整轮原文收齐、等二路把整段切完、等后面几句都合成完，再播第一句 |

二路失败、超时、JSON 坏、或者交回的上屏片段不是原文里按顺序能对上的切片：这一句改送原文，原文照上屏。失败路径也不要新写正则。

## 请求

`crates/live2d-ai-runtime/src/llm.rs` 的 `chat_request_body` 现在只有 `model` / `messages` / `stream` / `max_tokens`。对话请求（一路）在这里加上 `thinking: { "type": "disabled" }`。

二路走现成的非流式客户端 `crates/live2d-ai-runtime/src/performance/client.rs`（以及若仍会发出的 `crates/live2d-ai-mod-director/src/staging_http.rs`）。这两处同样加上该字段。`temperature` 与思考模式互相不生效，不要靠它关思考。

缺省模型名：`live2d-ai.toml.example` 的 `[llm].model` 改为 `deepseek-flash`，`base_url` 改为 `https://api.deepseek.com/v1`。代码缺省与 example 一致。不要在界面上再给一个模型下拉框。

设置里现有的「展示思考」只控制气泡里看不看得到思考，文案现在写着「关掉只是不显示，思考照样生成」。思考已经默认不生成，这句话改成与请求一致：默认不思考；只有用户打开思考时，请求才改成 `thinking.type = enabled`，折叠区才可能有内容。开关放在 LLM 分区，标签用「思考」，说明用「默认关闭。打开后模型会先想再答」。不要放到导演卡片上。

## 清洗与播放

主模型仍按现有句读（`。！？…` 和换行）切出一句。这是已有的分句，不是新的清洗正则。一句闭合后立刻把**这一句原文**交给二路，不要等后面的 token。

二路只处理这一句。它返回一份 JSON，形状固定为：

```json
{"display": "你好😄（挥手）", "speech": "你好"}
```

- `display` 必须是这一句原文的连续切片，逐码点能在原文里按顺序找到。找不到就整句作废，改送原文。
- `speech` 是拿去合成的字。emoji、颜文字、动作描写不要出现在这里。二路可以删掉这些，不能改写剩下的台词。
- `speech` 为空：这一句不发 TTS HTTP（空输入会 400，TTS 错误会弄失败整轮），原文仍然上屏，序号照占。

提交：`speech` 一到就作为这一句的 `TtsJob` 送进现有 worker。不要把整轮回复拼成一个 TTS 请求。不要等表演层那份整段 JSON 回来再送第一句。

上屏：`SentenceReady` / 气泡用 `display`（失败时用原文），不用 `speech`。现在 `conversation/engine.rs` 把 `clean_for_tts` 的结果同时写入 `SentenceReady` 和 `TtsJob`（注释里的 D23）。这条改成两条文本、同一个 `sentence_seq`。

播放：`shell/flutter/lib/audio/sentence_assembler.dart` 要等 `end` 才交出整句 WAV，`audio_player.dart` 再 `Blob` + `<audio>.play()`。保持媒体元素。某一句自己的音频到齐后立刻播这一句，不要等队列里后面的句子。一句之内不要中途重播、不要把 `currentTime` 打回 0。上游若本身要等整句合成完才回第一个字节，客户端不要再加一层「等整段」。

朗读高亮现在要求气泡正文以送去合成的字符串开头（`spoken_highlight.dart` 的 `highlightFitsText`）。上屏和送 TTS 不再是同一串字。高亮改按 `sentence_seq` 落在上屏文本上，对不上就不高亮。

回灌给主模型的历史仍是原文，不用 `speech`。

导演的头、颈、表情如果还要，不得挡住第一句的 TTS 提交。赶不上这一句第一个音频块的 cue 按现有规则丢掉。不要为了让 cue 准时去优化二路 token。

## 设置（用户视角看不懂的，全部在这里）

这些控件今天都摊在产品面上。导演卡片没有声明 `advancedKeys`，所以 `director_settings_spec()` 的 8 个字段全部平铺在 Mod 卡片里。配置键可以继续被解析，已经写进 `mods.json` 的值不要丢。产品界面不再渲染它们，也不要收进仍会展开的「高级」。

| 位置 | 现在用户看到的 | 怎么改 |
| --- | --- | --- |
| Mod 导演卡片，`crates/live2d-ai-mod-director/src/lib.rs` `director_settings_spec` | 「开心 → 动作」「难过 → 动作」「打招呼 → 动作」 | 产品面不渲染。词表映射不再当用户旋钮 |
| 同上 | 「二路 LLM 异步 cue（不是接管开关；接管后本项不再发请求）」 | 产品面不渲染。这个开关在接管后本来就不发请求 |
| 同上 | 二路端点 base_url、二路模型名、二路密钥变量名、二路超时 | 产品面不渲染。模型已定死，不再让用户填 |
| `shell/flutter/lib/settings/mods/director_panel.dart` | `kDirectorPresetNotice`（按情绪触发表情 / 短动作） | 删掉这句 |
| 同上 | `kDirectorTakeoverNotice`（重载、分段送 TTS、两项留空、两项都填、只填一项则仍直送） | 换成一句人话。启用开关旁边只留：「打开后，说话时会带上表情和轻微的头、颈动作。」 |
| 同上 | 接管状态、规则预设、实际下发、本轮预设、依据情绪/意图、清空决策账本、503 command_unavailable | 产品面拿掉。用户不需要这些词 |
| `shell/flutter/lib/settings/sections/llm_section.dart` | 「谁负责动作」把人指去 Mod 里的导演 | 改成：「对话模型只负责写台词。表情和点头不用在这里设。」不要再指向那些下拉框 |
| 同上，仅开发者模式 | 「表演层（默认关）」和 `kLlmPerformanceText` / `kLlmPerformanceDescription`（只说 / 只动 / noop、`[performance].enabled`） | 从界面拿掉。这段与现在的开关不符 |
| 设置 → 开发工具，`director_observer_section.dart` | 「导演可观测」A 栏把词表账本显示成 latest.emotion / suggested_tts / preset_id | 留在开发者模式里。A 栏标题改成「词表记录」，说明写明「这张表不决定现在的表情」。不要删开发者的动作调试、表情调试 |
| 外观与互动 | 「动作幅度」头 / 身体 / 表情，以及当前皮套单独覆盖 | 留着。这是用户调摆幅的旋钮，不是导演映射 |

导演 Mod 的启用开关留着，它是唯一的用户开关。

要改断言的测试：`shell/flutter/test/director_panel_test.dart`、`performance_layer_copy_test.dart`、`action_scales_wiring_test.dart`，以及 `crates/live2d-ai-mod-director` 里钉住这 8 个 label 的测试。断言改成「产品卡片上找不到这些字符串」，不要改成恒真。

## 禁止

- 新的 emoji / 颜文字 / 括号正则，或扩展 `clean_for_tts`。
- 为 token 延迟、token 速率、首 token 时间加探针、缓存、投机提交或改 `max_tokens`。
- 等整轮收齐再送第一句 TTS。
- 把播放改回 Web Audio / AudioContext。
- 一句音频中途重播。
- 改口型、待机、拖动、动作幅度的数值、舞台背景、wasm 参数表。
- 接回 `action_tx`。
- 新的 WS 帧。
- 在仓库根目录跑 `flutter analyze`。
- 改 `mods.json`、`.env`。改已经是 `deepseek-flash` 的 `live2d-ai.toml`。

## 验收

贴出每条最后 30 行。

```
cargo test -p live2d-ai-runtime chat_request
cargo test -p live2d-ai-runtime --lib performance
cargo test -p live2d-ai-mod-director
cd shell/flutter && /home/skystar/flutter/bin/flutter test test/director_panel_test.dart test/performance_layer_copy_test.dart test/action_scales_wiring_test.dart test/spoken_highlight_test.dart test/font_subset_test.dart
```

请求体测试必须能看到 `thinking.type == disabled`，并且在「思考」开关打开时变成 `enabled`。关掉思考的测试不要只查界面文案。

二路测试用一份假响应 `{"display":"你好😄","speech":"你好"}`：TTS 任务文本是 `你好`，上屏文本是 `你好😄`，第一句的提交发生在第二句原文出现之前。假响应不是原文切片时，TTS 任务文本等于原文。
