# 两类 TTS · 交给下一个 agent

> 立档：2026-10-09。状态：**活**。真源：[`PLAN-tts-two-class-2026-10-09.md`](PLAN-tts-two-class-2026-10-09.md)。
> 不要提交、不要打 tag、不要推送、不要升版本。做完由管理员审查。

## 口径

两类，只加一个拉起进程的 Mod。出声地址仍只来自 `[tts]`。

| 类 | 行为 |
| --- | --- |
| 云端 / API | 现有语音合成页。它是 `[tts]` 的唯一写入者 |
| 本地 `local-tts` | 缺省停用，不进 `default_mods_manifest`。按 argv `spawn`。调用 `apply_settings` 的次数为 0 |

`launch_mode`：`on_apply`（缺省）只在 `StartCause::Apply` 和命令 `launch` 时 spawn；`with_app` 在 Boot、Enable、Apply 和 `launch` 时都 spawn。非法值在 `create` 失败。空 `program`、非法 `args_json` 在这次真要 spawn 时返回 `Err`。

`StartCause` 用线程局部，由宿主在 `start_all`（Boot）、公开 `enable`（Enable）、`restart` 成功停掉之后（Apply）设置。不改 `ModServices` 构造函数。`registry.rs` 保持 ≤500 行。

挂掉时把 `Err` 和 `Failed` 留在面上。`shutdown` 失败则把子进程句柄和 runtime 放回原处并返回错误，不再 `start`。`state_json` 只做非阻塞观察。不重试、不改地址、不把失败写成成功。

配置按钮改为「保存并应用」。已重启的成功文案是「已保存并生效」，未启用是「已保存，未启用」。成功的配置保存不再走 `_notifyModChanged`；启停开关仍然走。不加专用面板。

## 要改的文件

以计划书「要改的文件」「文档」「单测」三节为准。注册后工厂数 5 → 6，测试名 `mod_count_is_six`。`RATCHET_DESKTOP_DEPS` 22 → 23，`PLAN_DESKTOP_DEPS` 仍是 22。`cli_entry` 里 `local-tts` 不在缺省 manifest 的断言保留，只改文案。

语音合成页的分区说明 `音色和出声的服务` 一个字不改。页内 `SectionHeader` 的 description 用计划书里的那一句。

## 禁止

- 写 `[tts]` 的任何键，或在停用 Mod 时改请求地址。
- 健康检查、看门狗、退出后自动拉起、失败后改走云端、吞掉 `Err`。
- 改 TTS 客户端、口型、wasm、外观、persona、director、memory 的行为；`voice-input` 只改头注释和配置按钮那个 `find.text`。
- 接回 `action_tx`，或把 `local-llm` / `wallpaper` / `pet-desktop` 挂回来。
- 改 `mods.json`、`.env`、`live2d-ai.toml`；改 CosyVoice 目录或垫片；下载权重；停掉 8080 上的垫片。
- 改主保存按钮、离开确认、记忆对话框、保存密钥的「保存」文案。
- 把 `RATCHET_SRC_RS_500` 或 `PLAN_DESKTOP_DEPS` 调高；仓库根目录 `flutter analyze`；恒真测试。
- 升版本、提交、推送；再写一份计划。改 `AGENTS.md` 的历史段落。

## 验收

```bash
cargo test -p live2d-ai-mod-local-tts
cargo test -p live2d-ai-mod-system
cargo test -p live2d-ai-desktop --bin live2d-ai-desktop mod_count
cargo test -p live2d-ai-desktop --bin live2d-ai-desktop mod_factory_ids
cargo fmt --all -- --check
cargo clippy -p live2d-ai-mod-local-tts -p live2d-ai-mod-system -p live2d-ai-desktop --all-targets -- -D warnings
cargo run -p xtask -- code-stats --check --only deps
cd /home/skystar/Live2D-Ai/shell/flutter
/home/skystar/flutter/bin/flutter analyze
/home/skystar/flutter/bin/flutter test \
  test/mods_section_test.dart \
  test/mod_config_draft_test.dart \
  test/mod_state_surface_test.dart \
  test/voice_input_panel_test.dart \
  test/settings_sections_test.dart \
  test/font_subset_test.dart \
  test/restart_notice_test.dart
```

都要 exit 0，并贴出最后约 30 行。`HTTP_PROXY` 把桌面传输用例说成 `llm_upstream_502` 时，去掉代理再跑，不要改断言。浏览器只在 `18080` 已经开着时看「保存并应用」和未启用保存后没有重新点火条；不要拉起真实本地进程。没开服务就写明没验。

## 提示词（整段复制给下一个 agent）

```text
你在 /home/skystar/Live2D-Ai，分支 main。先读 docs/plans/PLAN-tts-two-class-2026-10-09.md，再按它实现。不要提交，不要打 tag，不要推送，不要升版本。不要再写一份计划。

口径：语音合成仍是主链 HTTP 客户端。云端 / API 就是现有语音合成设置页，它是 [tts] 的唯一写入者。新增 Mod crate live2d-ai-mod-local-tts，id local-tts，缺省停用，不进 cli_entry::default_mods_manifest（断言 is_none 保留，只改文案）。它按用户给的 argv spawn 外部进程，apply_settings 调用次数为 0，不写 base_url / voice / model / sample_rate / channels / response_format / api_key_env。停用后链路仍请求已配置的地址。

launch_mode 仅 on_apply（缺省，键缺失也用它）与 with_app。非法值在 create 返回 Err。on_apply 在 Boot 和 Enable 不 spawn，在 Apply 和命令 launch 时 spawn。with_app 在 Boot、Enable、Apply、launch 都 spawn。program 是 argv0；args_json 缺省 []，必须是 JSON 字符串数组，带空格仍是一个元素，非法内容到 spawn 时才 Err；空 program 只在真要 spawn 时 Err。workdir 非空才设 current_dir。base_url 缺省 http://127.0.0.1:8080/v1，只出现在 state_json，是声明地址。不要探 sample_rate / channels。禁止 sh -c。stdin null，stdout/stderr inherit。测试注入 spawner，不 exec 真进程。

StartCause { Boot, Enable, Apply } 放在 live2d-ai-mod-system 的线程局部 set_start_cause / start_cause。不要改 ModServices 构造函数。start_all 在每次 start_one 前设 Boot；公开 enable 设 Enable（enable_with_config 仍算 Enable）；restart 先停，停成功后再以 Apply 启动，公开 enable 不得把这次 Apply 盖掉。start_one 保持私有。registry.rs 现约 459 行，改完 ≤500；测试放 #[path] 兄弟文件。disable 今天丢掉 shutdown 的 Err，改成失败时把 runtime 放回槽位并返回 Err，不写 enabled=false，restart 因此不再 start。shutdown 自己也要把 Child 放回运行时再返回 Err。

挂掉时：spawn 失败走现有 Failed { err.to_string() }。state_json 只用非阻塞 try_wait 报告 pid / child_running / exit_code；try_wait 出错写入 wait_error，禁止在快照里再 spawn 或改 [tts]。launch 在已有子进程时 Err；stop 在没有子进程时 Err；未知命令 UnsupportedCommand。不写看门狗、不自动重拉、不探活、不换 URL、不把 Err 变成 Ok。不要改 supervisor 的旧配置保留逻辑。

前端：dev_tools_mod_config.dart 的配置按钮改为 保存并应用，忙碌仍是 保存中…。restarted 时文案 已保存并生效；enabled==false 时 已保存，未启用。不要声称进程已启动。shell_admin.dart 的 _saveModConfig 成功后不要调用 _notifyModChanged，仍要 _refresh 和 _loadAdmin；_settings.dirty 为假时再 await _settings.load() 并刷新，脏则跳过（load 会丢草稿）。_toggleMod 仍通知。不要删 restart_notice.dart。不要做专用面板。kModStateLabels 补 pid / child_running / exit_code / base_url / launch_mode / wait_error 的中文标签，缺字就改短。SettingsSection.tts.description 保持 音色和出声的服务。tts_section 的 SectionHeader description 改为：这里是云端接口的地址。扩展只拉起进程，不会改写这个地址。

注册：工厂 5→6，测试改名 mod_count_is_six，id 列表含 local-tts。desktop 直依赖 22→23，同 commit 把 RATCHET_DESKTOP_DEPS 改为 23 并注释这是一条产品 Mod 直边；PLAN_DESKTOP_DEPS 保持 22。每个新 .rs ≤500，不要把 RATCHET_SRC_RS_500 从 44 调高。cli_entry.rs 不要超过 1000 行。Flutter containsAll 加上 local-tts。只改这四个测试里的配置按钮 find.text('保存')：mods_section_test、mod_config_draft_test、mod_state_surface_test、voice_input_panel_test。用 readLibrarySource('lib/main.dart') 做守卫：_saveModConfig 体内没有 _notifyModChanged，_toggleMod 仍有。

文档只改计划书点名的活句：mod-product-chain §5、tts-is-core 文末新节、AGENTS.md 里现行的五 Mod 与 mod_count_is_five 三处、local-inference-setup 的 2026-09-11 横幅。历史段落保持原样。

禁止：改 runtime TTS 客户端、口型、wasm、外观、persona、director、memory 的行为；voice-input 只改头注释和那个按钮查找；接回 action_tx；挂回 local-llm / wallpaper / pet-desktop；改 mods.json、.env、live2d-ai.toml；改 /home/skystar/CosyVoice 3.0 或下载权重或停 8080 垫片；新 WS 帧；仓库根目录 flutter analyze；恒真测试；提交、推送、升版本。

验收并贴出最后约 30 行，没跑的不要说绿：
cargo test -p live2d-ai-mod-local-tts
cargo test -p live2d-ai-mod-system
cargo test -p live2d-ai-desktop --bin live2d-ai-desktop mod_count
cargo test -p live2d-ai-desktop --bin live2d-ai-desktop mod_factory_ids
cargo fmt --all -- --check
cargo clippy -p live2d-ai-mod-local-tts -p live2d-ai-mod-system -p live2d-ai-desktop --all-targets -- -D warnings
cargo run -p xtask -- code-stats --check --only deps
cd shell/flutter && /home/skystar/flutter/bin/flutter analyze && /home/skystar/flutter/bin/flutter test test/mods_section_test.dart test/mod_config_draft_test.dart test/mod_state_surface_test.dart test/voice_input_panel_test.dart test/settings_sections_test.dart test/font_subset_test.dart test/restart_notice_test.dart
HTTP_PROXY 导致 llm_upstream_502 时去掉代理再跑，不要改断言。18080 已开就看保存并应用，且未启用的保存不出现重新点火条；不要拉起真实进程。服务没开就写明浏览器没验。
```
