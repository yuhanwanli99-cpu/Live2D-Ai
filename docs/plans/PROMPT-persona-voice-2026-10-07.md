# 人设入口 + 语音现状 · 交给下一个 agent

> 2026-10-07。管理员已定口径。下一个 agent 只做 T6 的界面接线，并按 T7 核对语音、写回结论。
> 不要提交、不要打 tag、不要推送、不要升版本。

## 口径

两个 Mod 都已经在仓库里，缺省关闭。本机 `mods.json` 里 `persona` 与 `voice-input` 也是关的。不要重写解析器，不要把 ASR 放进 Rust，不要改编译期缺省清单（`default_mods_manifest` 仍只启用 `external-input`）。

| 编号 | 做什么 |
| --- | --- |
| T6 | 「人设」页接上现有的导入。默认绑当前会话。全局导入是第二个按钮。 |
| T7 | 不改语音代码。核对「听」按钮已经接到 `POST /api/v1/voice/transcript`，把结论写在回复里。 |

## T6 产品

「人设」页（`PersonaSection`）今天只有系统提示词和记住几轮。酒馆卡的导入在「Mod」分区的 `PersonaPanel` 里，命令是 `POST /api/v1/mods/persona/command`。

接到人设页之后，用户应能：

1. 粘贴卡 JSON，点「导入并生效」。请求带当前会话 id：`command = import_card`，`args = {card_json, session_id}`。没有活动会话时这个按钮禁用，并写明原因。不许改成偷偷全局导入。
2. 同一个粘贴框另有「导入为全局人设」。请求不带 `session_id`。文案写明它会改所有会话用的主链提示词。
3. 有文件选择器时，选 PNG 仍走会话导入：`args = {data_base64, session_id}`。选择器继续从组合根注入（`pickPersonaCardFile`），不要在 `persona_section.dart` 里直接 import `browser_io.dart`。
4. 成功或失败就地显示，失败带服务端错误码（`command_unavailable` / `command_failed` / `unsupported_command` / `not_found`）。Mod 没开时说明去「Mod」分区打开 `persona`，不要在人设页再做一把启停开关，也不要代用户改 `mods.json`。
5. 全局导入成功、且设置草稿是干净的：调用已有的 `SettingsController.load()`，让系统提示词显示服务端新值。草稿不干净时不要丢草稿，提示「服务端已写入，本页还有未保存的修改」。
6. 会话导入成功后不要指望系统提示词变字。页上写明：这张卡只对当前会话生效，不改下面这栏。

名称、描述、性格、场景、开场白这些卡字段不要回到人设页。不要做卡编辑器、在线卡库、和记忆的仲裁。

## T6 接线

- 分区构造在 `shell/flutter/lib/app/shell_settings.dart` 的 `SettingsSection.persona`。该文件是 `main.dart` 的 part。当前会话 id 用 `_chat.sessions.activeId`（Mod 页已经这么传给 `activeSessionId`）。
- 发命令用已有的 `ModsApi.command('persona', 'import_card', args: ...)`。不要新端点。
- 命令入参与按钮语义以 `shell/flutter/lib/settings/mods/persona_panel.dart` 和 `crates/live2d-ai-mod-persona/src/command.rs` 头注为准。Mod 页那套面板保留，两处发同一种请求。
- `test/wiring_test.dart` 现在断言人设页没有导入按钮。改成：名称 / 描述 / 性格 / 场景 / 开场白仍然没有；「导入并生效」和「导入为全局人设」在。无会话时前者禁用。全局成功且草稿干净时会 `load()`。会话成功的说明里能看出「不改系统提示词」。
- 新文案必须在 Noto Sans SC 子集内。能复用面板上已有句子就复用。

## T7 核对（不要改代码）

这些已经存在，核对后在回复里各写一句「在 / 不在」：

- 聊天框有「听」。点按是常驻唤醒，按住是说话。
- 未启用时按钮旁是 `kVoiceModDisabledMessage`，不是等 403 才说话。
- 原文进 `VoiceApi.sendTranscript`，带 `ptt`。剥词在服务端。
- sidecar、`backend`、`locale`、`token` 在语音面板的「高级」里。缺省转写器是 `fake`。
- `cli_entry::default_mods_manifest` 不含 `voice-input`。

发现某一条不在，停下来写明文件和行，不要另做识别器。不要改 `speech_recognizer*.dart`、`voice_listen_controller.dart`、`crates/live2d-ai-mod-voice-input/`。

## 禁止

- 不要改 `crates/`（含 persona 与 voice）、wasm、`mods.json`、`.env`、`live2d-ai.toml`。
- 不要改 `AGENTS.md`、docs 预算常量、`docs/audit/**`。
- 不要动外观、动作滑条、口型、朗读高亮、音频播放。
- 不要新增 WS 帧。不要把测试改成恒真。
- 不要在仓库根目录跑 `flutter analyze`。
- 不要提交。

## 验收

```bash
cd /home/skystar/Live2D-Ai/shell/flutter
/home/skystar/flutter/bin/flutter analyze
/home/skystar/flutter/bin/flutter test
```

两条都要 exit 0。贴出各自最后 30 行，并附上 T7 的五条核对。没有浏览器，不要声称点过真麦克风。

## 提示词（整段复制给下一个 agent）

```text
你在 /home/skystar/Live2D-Ai，分支 main。先读 docs/plans/PROMPT-persona-voice-2026-10-07.md。做两件事：T6 给人设页接上现有角色卡导入；T7 只核对语音、不改语音代码。不要提交。

T6（只改 Flutter）：
「人设」页今天只有系统提示词和记住几轮。导入已经在 Mod 页的 PersonaPanel 里，命令是 ModsApi.command('persona', 'import_card', args: ...)。

在 PersonaSection 加上：
1. 粘贴 JSON。「导入并生效」带 session_id = 当前会话 _chat.sessions.activeId。没有活动会话就禁用，并说明原因。不许改成全局导入。
2. 「导入为全局人设」不带 session_id。文案写明它改的是所有会话的主链提示词。
3. 文件选择沿用组合根的 pickPersonaCardFile，会话导入，args 用 data_base64。persona_section.dart 不要 import browser_io.dart。
4. 成败就地显示，失败带错误码。Mod 未运行（503 command_unavailable）时告诉用户去「Mod」分区打开 persona。不要在人设页做第二把开关，不要改 mods.json。
5. 全局导入成功且 SettingsController 草稿干净时调用 load()。草稿不干净就保留草稿，并说明服务端已写入。
6. 会话导入成功后写明：只对当前会话生效，不改下面的系统提示词。不要为此调用 load() 来假装提示词变了。

不要把名称、描述、性格、场景、开场白画回人设页。不要做卡编辑器。不要改 crates/live2d-ai-mod-persona。Mod 页的 PersonaPanel 保留。

改 test/wiring_test.dart：卡字段仍然不在；两个导入按钮在。补会话按钮禁用、全局成功刷新、会话成功不改提示词这三条。新文案必须在字体子集内。

T7（零代码，写进回复）：
核对聊天框「听」、未启用时的 kVoiceModDisabledMessage、VoiceApi.sendTranscript、sidecar 在高级里且缺省 fake、default_mods_manifest 不含 voice-input。五条各写「在 / 不在」。有一条不在就停，不要新做识别器，不要改 speech_recognizer、voice_listen_controller、voice-input crate。

禁止：改 crates/、wasm、mods.json、.env、live2d-ai.toml、AGENTS.md、外观、动作滑条、口型、朗读高亮。不要在仓库根目录跑 flutter analyze。不要提交。

验收（必须真跑，贴最后 30 行）：
cd /home/skystar/Live2D-Ai/shell/flutter
/home/skystar/flutter/bin/flutter analyze
/home/skystar/flutter/bin/flutter test
```
