# 皮套底座：把扩展、调试和聊天听写放回该放的地方

> 2026-10-08。维护者已定口径。交给下一个 agent 实现。
> 不要提交、不要打 tag、不要推送、不要升版本。做完由维护者审查。
> 上一份 `docs/plans/PROMPT-settings-plain-language-2026-10-08.md` 砍得过狠。本文件改口的部分以本文件为准。它没点名的人话文案（导航名、错误横幅、离线横幅、导演那一句）留着。

## 给下一个 agent 的提示词

```
你在 /home/skystar/Live2D-Ai（分支 main）。按 docs/plans/PROMPT-shell-base-2026-10-08.md 实现。不要提交、不要打 tag、不要推送、不要升版本。不要改 mods.json、.env、live2d-ai.toml。工作树里已有的未提交改动不要还原，只做本文件列出的改动。

底座是「跟人说话的皮套」：舞台、聊天、出声、人设提示词、记住几轮。角色卡、记忆、直播接入、听写配置是扩展。表情和动作的试播只在开发模式里。

不要动：一路和二路的请求体、thinking.type、按句 {display, speech}、分句、TTS、<audio> 播放、口型、动作幅度、导演 cue、action_tx、新的 WS 帧、clean_for_tts、SQLite、第三路同步模型。配置键继续能解析。开发模式关着时表单不画的键，保存时从已有 config 原样带走，不要用默认值覆盖。

产品面闲置说明里不要出现：epoch、schema、WS、base_url、sidecar、ASR、BCP-47、全局桶、手动闸、总闸、403、503、command_unavailable、Ollama、vLLM、api v、LIVE2D_AI_MUTE_AUDIO。按钮失败后的一句可以在句尾带错误码。

界面新文案必须过 font_subset_test。不要写子集外的字符，不要用 emoji。flutter analyze 和 flutter test 只在 shell/flutter 里跑，用 /home/skystar/flutter/bin/flutter。不要在仓库根目录跑 flutter analyze。

验收命令写在文末。
```

## 口径

| 项 | 定死 |
| --- | --- |
| 底座 | 舞台、聊天、发送、停止、静音、音量、口型、系统提示词、记住几轮、自己的背景图、四套主题 |
| 扩展 | 角色卡导入、记忆列表、直播接入的四项配置、听写的用法说明。开关仍在卡片标题行 |
| 开发模式 | 表情调试、动作调试、导演可观测、诊断、以及下面点名的 `devKeys`。分区本身永远留在导航里 |
| 呈现 | 新增 `ModPanel.devKeys`。`devMode == false` 时它们跟 `hiddenKeys` 一样不画，保存原样带走。`devMode == true` 时画在通用表单里，不塞进「高级」 |
| `ModPanelContext` | 增加 `bool devMode`，缺省 `false`。`ModsSection` / `_ModConfigTile` 已经有 `devMode`，传进去 |

`_ModConfigTileState` 的有效隐藏集：

```text
effectiveHidden = hiddenKeys ∪ (devMode ? ∅ : devKeys)
```

`_plainFields`、`_advancedFields`、`_buildConfig` 都用 `effectiveHidden`，不要只改渲染、保存时把没画出来的键写成默认值。

## 已经落地、不要动

- `docs/plans/PROMPT-tts-llm-clean-2026-10-08.md` 的清洗、思考开关、按句提交 TTS。
- 导航：对话、扩展、人设、开发模式。诊断只在开发模式出现。关掉开发模式时若正停在诊断，仍走 `_gotoSection(SettingsSection.persona)`。
- 离线横幅：开发模式关着是「没连上（短标签）· 点此重试」，开着是「后端未连接（短标签）· 点此重试」。状态胶囊始终是「没连上」。
- 错误横幅按钮「去对话设置」，落点仍是 `SettingsSection.llm`。
- 导演产品卡片仍是启用开关加 `kDirectorTakeoverNotice`（「打开后，说话时会带上表情和轻微的头、颈动作。」）。8 个键继续 `hiddenKeys`，不要把「开心 → 动作」和二路端点放回来。
- `DeveloperSection` 里 `devMode == true` 时已有的 `DebugPanels`（表情调试、动作调试）和 `DirectorObserverSection`（导演可观测）留着，并加一条 widget 测试钉住这三个标题。缺了就是本轮要修的，不是再设计一套。
- 外观里已经拿掉的描边强度、舞台单图轮播、铺法，不要加回来。
- 动作幅度滑条留着。播放仍是 `<audio>`。
- `LlmSettings.model` 的 serde 缺省仍是空串。

## 对话：删掉「谁负责动作」

`shell/flutter/lib/settings/sections/llm_section.dart` 里的 `ReadonlyField`（label「谁负责动作」）和常量 `kLlmActionOwnerText` 删掉。导演仍按句出 cue，这里不再解释。不要把「表演层」那一行加回来。`LlmSection.devMode` 参数留着，供调用签名用。

改 `test/performance_layer_copy_test.dart`：不再期待这句正文。全库搜 `谁负责动作` / `kLlmActionOwnerText`，产品面和测试里都不要留。

## 人设页与角色卡

角色卡是 persona 扩展，不是主链人设页。

`persona_section.dart` 只留「系统提示词」和「记住几轮」。拿掉导入块：粘贴 JSON、选 PNG、导入并生效、导入为全局人设、清除当前会话的卡、清除全局导入卡，以及页头里「酒馆角色卡可以在这里导入」那句。`pickCardFile` 从 `PersonaSection` 去掉。

这些控件搬回 `persona_panel.dart` 的 `build`。命令和入参保持现在人设页已经在用的契约：

| 按钮 | 命令 | args |
| --- | --- | --- |
| 导入并生效 | `import_card` | `card_json` 或 `data_base64`，加上当前 `session_id`。没有活动会话时按钮禁用，并写明先发一条消息 |
| 导入为全局人设 | `import_card` | 只给卡，不给 `session_id` |
| 清除当前会话的卡 | `clear_import` | 带 `session_id` |
| 清除全局导入卡 | `clear_import` | 不带 `session_id` |

PNG 选择沿用 `PersonaCardFilePicker`。`package:web` 仍然只从组合根注入：`shell_settings.dart` 里现在传给 `PersonaSection` 的 `pickPersonaCardFile` 改传到 `ModsSection`，再放进 `ModPanelContext`。面板测试注入 fake，不直接 import `browser_io.dart`。

8 个 spec 键继续 `hiddenKeys`：`card_path`、`card_json`、`include_discipline`、`say_first_mes`、`name`、`description`、`personality`、`scenario`。不要在表单里再手填一遍卡。

卡片开头留 `kPersonaTakeoverNotice`。导入区下面，运行态里有卡名时用一句话写「当前卡」和它只对当前会话还是所有会话。没有卡就不画这句。不要把 `state_json` 的键名摊开。

`persona_panel_test.dart` 里现在泵 `PersonaSection` 来测导入的，改泵扩展卡片。命令入参的断言留着。

## 外部事件

四项配置回到产品卡片的通用表单，不要放进 `hiddenKeys` 或 `devKeys`：

| 键 | 新的 Rust label（只改 label） | `fieldHelp` |
| --- | --- | --- |
| `listen_port` | 端口（只作提示） | 服务实际听哪个端口不由这项决定 |
| `token` | 令牌 | 直播工具要带的口令。留空则只接受本机 |
| `text_template` | 弹幕怎么说给角色 | `{text}` 会换成弹幕或礼物原文 |
| `prefix` | 每条前面加的字 | 加在每条前面 |

键、`secret`、数值范围、默认值不动。`token` 仍是 secret。产品卡片仍有一句：「打开后，直播间的弹幕和礼物会说给角色听。」

开发模式才画、普通模式不画：

- 一个输入框加按钮「发送这条测试」。命令 `test_inject`，args `{"text": "<输入>"}`。成功句写清主链是否接住。失败句句尾可以带码。
- 四项计数用现有 `stateLabels`：已接受、已拒绝、忙碌拒绝、礼物 v2 兜底。按钮「计数清零」走 `reset_counters`，args `{}`。

不要把 curl、真 HTTP 等价、端口长文、403/503 闲置说明加回来。

## 语音：点一下录音，字进输入框

聊天栏「听」改成两态，不再按住说话，也不再常驻等唤醒词：

1. 点「听」开始录音。按钮变成「停」，旁边写「正在听，说完再点一次」。
2. 再点「停」结束。把定稿放进 `_input`（光标在末尾）。输入框里已有文字时，接在后面，中间补一个空格。不要 `send()`，不要 `POST /api/v1/voice/transcript`。
3. 定稿是空的：输入框不动，写「没听清，再点一次说」。
4. 角色正在播报时不允许开始。写「角色在说话，说完再听」。已经在录的，播报开始时停掉识别，已听到的字仍放进输入框。

实现落点：

- `voice_listen_controller.dart`：产品路径增加「定稿交给宿主」的回调，成功和失败都走它。聊天按钮不再调用 `_sendNow`。`pressStart` / `pressRelease` / 常驻 `toggle` 不再接到 `chat_panel.dart`。
- `shell_cue_voice_wiring.dart` 的 `_onVoiceBusyResult` 是现在唯一填输入框的地方。新的定稿走同一段「写入 `_input`」，不要只在 busy 时写。
- `main.dart` 拿掉 `onPressStart`、`onPressRelease`、`pttActive` 这条产品接线。`listenBlockedReason`（Mod 未启用就禁用「听」）拿掉：这条路径不打语音端点。
- `chat_panel.dart` 的「听」按钮只留点按。不要 `Listener` 的按下/松开。

`POST /api/v1/voice/transcript`、Rust 闸门、`wake_phrase`、`manual_enabled` 都留在代码里。聊天按钮不用它们。

语音卡片产品面改成一句：「点聊天栏的「听」。说完再点一次，字会出现在输入框里。」

`devKeys`（开发模式才出现在表单里）恰好是现有 spec 的这两项加原来的 7 个隐藏键：`wake_phrase`、`manual_enabled`、`backend`、`locale`、`token`、`sidecar_script`、`sidecar_url`、`sidecar_transcriber`、`sidecar_python`。`hiddenKeys` 改为空。`fieldHelp` 可以留在开发模式的那两行上，不要再写「按住听」。

## 记忆列表

不换存储，不加模型。只修列表锁。

`memory_panel.dart`：

- 「刷新」可点条件是扩展已启用且没有别的命令在跑。不要看 `_bucketStale`。
- 导入、编辑、删除、清空仍要求快照等于当前会话，且不在读取中。避免拿旧行 id 写进新会话。
- `list` 失败时不更新 `_recordsSessionId`，留下 `_listError`，刷新保持可点。
- 切会话时在 `didUpdateWidget` 里只改字段（清空列表、`_listLoading = true`）。不要在那里直接 `setState`。取数用 `initial: true` 的 `_refreshList`，或者放到本帧结束之后。响应落地再 `setState`。

`hiddenKeys` 只留 `store_path`。下面这些改成 `devKeys`：`top_k`、`max_records`、`summary_enabled`、`summary_base_url`、`summary_model`、`summary_api_key_env`、`summary_timeout_ms`、`summary_ratio`、`summary_cooldown_turns`、`summary_keep_recent_turns`。

产品表单仍只默认露出 `enabled_injection`，说明仍是「关掉后仍会记住，但不会塞进角色的提示词。」列表、导入、编辑、删除、清空留在产品面。

## 会话删除后列表还在

`showSessionSheet` 把 `byRecency` 抄进浮层，`showModalBottomSheet` 的 builder 只跑一次。`deleteSession` 已经改了 store 并 `notifyListeners`，浮层不听，所以行还在。新建和重命名开着浮层时同样是旧的。

修法：浮层内部包 `ListenableBuilder`，听 `ChatController`。每次 build 读 `sessions.byRecency` 和 `activeId`。删掉的行立刻消失。删的是当前会话时，背后的聊天区仍按现有 `delete` 切到下一条。不要为了刷新去关浮层。

`AppShell` 现在只有打开那一刻的 `List<ChatSession>`。把 `Listenable` 和「读取当前列表 / 当前 id」的回调从 `main.dart` 传进来。builder 里读回调，不要读打开时的那份副本。

## 渐变、光晕、网格、斜纹、玻璃高光

背景库不再提供内置图案。`appearance_background.dart` 里遍历 `BackgroundPatternId.values` 的那排 `ActionChip` 删掉。说明句改成只谈自己的图和纯色，不要再写「选了内置图案时，舞台回到纯色」。

读回背景库时丢掉 `kind == pattern` 的项（渐变、光晕、网格、斜纹都算）。不要写回。壳和舞台都不再画这些图案。四套主题色、用户自己的图、不透明度留着。`DisplayPrefs.toJson` 仍是那 15 个顶层键，不要新键，也不要把已经删掉的旧键写回去。

`GlassRim` 从这三处拿掉，子组件留在原地：`app_shell_state.dart`、`nav_host.dart`、`stage_corner_controls.dart`。压在舞台上的控件仍然套 `StagePointerInterceptor`。`glass_rim.dart` 没有引用之后删掉，对应测试改成「这三处外壳不再包 GlassRim」。不要换成另一圈渐变或阴影。

## 禁止

- 还原或重做二路清洗、思考开关、导演 8 个隐藏键、表演层那一行。
- 把「开心 → 动作」或二路 LLM 地址放回导演卡片。
- 新建 SQLite，或在 `TurnPrompt` 里再加一路同步模型。
- 让聊天栏「听」去打 `/api/v1/voice/transcript` 或自动发送。
- 把角色卡导入留在人设页，或人设页和扩展卡片各留一套。
- 把 `devKeys` 收进仍能展开的「高级」，让开发模式关着也能改到。
- 藏「开发模式」这个分区，或拿掉表情调试 / 动作调试 / 导演可观测。
- 新的 WS 帧、接回 `action_tx`、新的清洗正则。
- 改 `mods.json`、`.env`、`live2d-ai.toml`。
- 升版本、提交、打 tag、推送。
- 在仓库根目录跑 `flutter analyze`。
- 再写一份平行的说明文档。本文件就是范围。

## 验收

贴出每条最后 30 行。

```
cd shell/flutter && /home/skystar/flutter/bin/flutter test \
  test/performance_layer_copy_test.dart \
  test/persona_panel_test.dart \
  test/external_input_panel_test.dart \
  test/voice_input_panel_test.dart \
  test/memory_panel_test.dart \
  test/director_panel_test.dart \
  test/session_entry_test.dart \
  test/chat_session_test.dart \
  test/display_prefs_test.dart \
  test/appearance_background_library_test.dart \
  test/shell_backdrop_test.dart \
  test/background_logic_test.dart \
  test/glass_rim_test.dart \
  test/font_subset_test.dart
cd shell/flutter && /home/skystar/flutter/bin/flutter analyze
```

语音控制器和聊天栏按钮如果改了别的测试文件，把那些文件名补进同一条 `flutter test`，不要只跑上面这份就当语音过了。

Rust 只改了 external-input 的四条 label 时再跑：

```
cargo test -p live2d-ai-mod-external-input
```

没有改别的 Rust 行为就不要为了凑数跑全量 workspace。

断言要能失败：

- 人设页找不到「导入并生效」「选择 PNG 角色卡文件」。扩展里的 persona 卡片找得到。
- 开发模式关着时，外部事件卡片找得到「令牌」，找不到「发送这条测试」。开着时找得到。
- 开发模式关着时，语音卡片找不到「唤醒词」。开着时找得到。
- 对话页找不到「谁负责动作」。
- 背景区找不到「渐变」「光晕」「网格」「斜纹」。
- 开发模式开着时，开发模式页找得到「表情调试」「动作调试」「导演可观测」。
- 记忆列表在 `list` 失败后「刷新」仍可点。
- 会话浮层听 store：删掉一行后，浮层不关闭，该行消失。
