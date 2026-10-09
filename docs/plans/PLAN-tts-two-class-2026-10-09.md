# 两类 TTS · 计划

> 立档：2026-10-09。状态：**活**。真源是本文件；交给代码 agent 的整段提示词在
> [`PROMPT-tts-two-class-2026-10-09.md`](PROMPT-tts-two-class-2026-10-09.md)。
> 维护者口径已定，实现时不要重开设计。不要提交、不要打 tag、不要推送、不要升版本。
> `docs/plans/` 顶层在本文件写入前已是 30 份（2026-10-06 的「≤25」人眼上限早已超过）。
> 本对文件是维护者点名要的交接，**不要**再写第三份计划，也**不要**顺手归档别的计划。

## 口径

语音合成仍是主链上的 HTTP 客户端：`POST {base_url}/audio/speech`，实现在
`crates/live2d-ai-runtime/src/tts.rs`。本轮只加一个**拉起外部进程**的 Mod。
出声地址仍然只有 `live2d-ai.toml` 的 `[tts]`。

| 类 | 是什么 | 谁写端点 |
| --- | --- | --- |
| 云端 / API | 现有「语音合成」设置页，缺省就是这一类 | 只有这一页，经 `PATCH /api/v1/settings` 写 `[tts]` |
| 本地 | 新 crate `live2d-ai-mod-local-tts`，id `local-tts`，缺省停用 | 不写。它只 `spawn` 用户给出的 argv |

`[tts]` 的键是 `base_url` / `voice` / `model` / `sample_rate` / `channels` /
`response_format` / `api_key_env`。Mod 对这些键的写次数必须是 0，包括
`ModServices.apply_settings`。停用 Mod 之后，链路继续请求已经配好的地址。
用户把地址指到本机、对面没有进程时，这一次合成请求按现有错误帧失败。

2026-09-11《TTS 是核心链路》的历史段落保留。本轮追加的是：允许一个**不改端点**的拉起 Mod。
旧 crate 那种「探活再改写 `base_url`」不恢复。

### 挂掉时

维护者原话：「挂掉时不管。代码内的任何类似设计基本不做。不静默吞错误和写相关防御性代码。」

进程没拉起来、参数不合法、停止失败、子进程自己退出，都把错误留在返回值和 Mod 状态里。
具体做法：

- 该 `spawn` 时失败 → `start` 返回 `Err`。宿主现有路径把该 Mod 标成 `ModStatus::Failed { message: err.to_string() }`。沿用这条，不要在外面再包一层重试或换地址。
- `state_json` 可以用非阻塞 `try_wait` 记下 pid、是否仍在、退出码。`try_wait` 本身出错时，把错误字符串放进快照字段 `wait_error`。快照里禁止因此去 `spawn`、改 `[tts]`、或把失败改写成「在跑」。
- 命令 `launch`：已经有子进程就返回 `Err`，禁止再起第二个。命令 `stop`：没有子进程就返回 `Err`。未知命令 → `ModError::UnsupportedCommand`。
- `shutdown`：先停子进程再等它结束。停止失败时，把 `Child` 放回运行时结构，返回这个 `Err`。
- 宿主 `disable` 今天是 `let _ = rt.shutdown()`，失败后 runtime 被丢掉，子进程会变成没人 `wait` 的孤儿。改成：`shutdown` 返回 `Err` 时把 runtime 放回槽位，`disable` / `restart` 返回这个 `Err`，不把 `enabled` 写成 false，不写 `mods.json`，也不调用下一次 `start`。成功路径仍是停掉、`enabled = false`、`persist_manifest`。
- 不写看门狗、不在退出后自动拉起、不做健康检查、不把请求改派到别的 URL、不把错误日志记完再返回 `Ok`。

`supervisor.rs` 里「热重载失败则保留旧配置继续运行」是既有行为。本轮不要把这种保留扩展到本地进程上，也不要为这句话去改 supervisor。

### 拉起方式

`settings_spec` 的 `launch_mode` 是 Select，两个值：

| 值 | 缺省 | `StartCause::Boot` | `Enable` | `Apply` | 命令 `launch` |
| --- | --- | --- | --- | --- | --- |
| `on_apply` | 是（键缺失时用它） | 不 spawn | 不 spawn | spawn | spawn |
| `with_app` | | spawn | spawn | spawn | spawn |

非法值在 `create` 返回 `Err`，禁止折成缺省。`on_apply` 在 Boot / Enable 下 `start` 返回 `Ok`，Mod 状态可以是 Running，子进程未起；`state_json` 用 `child_running: false` 说明这件事。空 `program` 只在**这次真要 spawn** 时返回 `Err`。

其余字段：

| key | 种类 | 含义 |
| --- | --- | --- |
| `program` | String | `argv[0]`。标签写明按路径原样执行 |
| `args_json` | String，缺省 `[]` | 内容必须是 JSON 字符串数组。缺键当 `[]`。有空格的一项仍是一个 argv 元素。非法内容留到 spawn 时返回 `Err`，不要提前校验后吞掉 |
| `workdir` | String，可空 | 非空才设 `current_dir`。不创建目录 |
| `base_url` | String，缺省 `http://127.0.0.1:8080/v1` | 只放进 `state_json` 的声明地址。这是 CosyVoice 垫片的文档地址，供用户对照；**不是** spawn 失败后的回退地址 |

不放 `sample_rate` / `channels`，也不要 HTTP 去探这两个数。标签用中文，因为本轮不加专用面板，通用表单只显示 spec 的 label。建议标签：`拉起方式`、`程序（argv0，按路径原样执行）`、`参数（JSON 字符串数组）`、`工作目录（可空）`、`声明地址（只展示，不写入语音设置）`。选项标签：`保存并应用时拉起`、`随应用启动拉起`。

进程：`stdin = null`，`stdout` / `stderr` = inherit。禁止管道（没人读会把子进程堵死）。禁止 `sh -c`。测试注入 `Box<dyn Fn(...) -> ... + Send>`，单测不 exec 真进程。argv 拼法可参考 `crates/live2d-ai-mod-voice-input/src/sidecar.rs` 的「逐参数、不过 shell」，不要调用那个 crate，也不要改它的行为。

### `StartCause`

`ModRuntime::start` 今天分不清开机、启用、保存并应用。在 `live2d-ai-mod-system` 加线程局部：

```rust
pub enum StartCause { Boot, Enable, Apply }
pub fn set_start_cause(cause: StartCause);
pub fn start_cause() -> StartCause;
```

缺省值用 `Boot`（漏设时 `on_apply` 不会意外 spawn）。`local-tts` 只在 `start()` 里读它。不要改 `ModServices` 的构造函数（每个 Mod 都在用）。宿主在调用 `start` 的同一条同步调用栈上设置；不要把 `start` 挪到别的线程。

`crates/live2d-ai-desktop/src/mod_registry/registry.rs` 现为 **459** 行，改完必须 **≤500**（`RATCHET_SRC_RS_500 = 44`，没有余量，超限文件个数不能 +1）。接线：

- `start_all`：每次 `start_one` 之前 `set_start_cause(Boot)`。
- 公开 `enable`：`set_start_cause(Enable)`。`enable_with_config` 继续走 `enable`，所以是 Enable，不是 Apply。
- `restart`：先按上一节停掉；成功后再 `enable_caused(Apply)`。公开 `enable` 不能在这次调用里把 Apply 覆盖成 Enable。
- `start_one` 保持私有。

宿主测试放在新的兄弟文件里，用 `#[cfg(test)] #[path = "..."]` 挂上（先例 `web_api/mods_routes.rs` 末尾）。用一个只记录 `start_cause()` 的桩工厂，断言 Boot / Enable / Apply 三条。桩文件自己也要 ≤500 行。塞不进 `registry.rs` 就抽出去，**不要**把棘轮从 44 调高。

### 前端「保存并应用」

后端 `POST /api/v1/mods/{id}/config` 已经会合并配置（secret 键是按键合并，不是整份替换），已启用时再 `restart`。不要改这条路由的语义。

按钮在 `shell/flutter/lib/settings/sections/dev_tools_mod_config.dart`，所有 Mod 共用，不只 `local-tts`：

- 文案 `保存` → `保存并应用`。忙碌文案保持 `保存中…`。
- `restarted == true` → `已保存并生效`。
- `enabled == false` → `已保存，未启用`。
- 两句都不写「进程已启动」。

`shell/flutter/lib/app/shell_admin.dart` 的 `_saveModConfig`：成功后**不再**调用 `_notifyModChanged`。仍 `_refresh` 并 `_loadAdmin`。设置草稿不脏时（`SettingsController.dirty`，字段 `_settings`）再 `await _settings.load()` 然后刷新；`load()` 会丢掉草稿，脏的时候跳过。`_toggleMod` 仍调用 `_notifyModChanged`。各 Mod 产品面板自己的 `onModChanged` 保持。不要删 `ui/restart_notice.dart`。

`lib/main.dart` 里 `_modRestartNotice` 的注释现在写着配置保存也会写入这条提示，改成与上面一致。

不加专用面板。`dev_tools_mod_config.dart` 的 `_stateBlock` 在 `modPanelFor(id) != null` 时直接 `SizedBox.shrink()`，有面板就会挡住 pid / 退出码。`kModStateLabels`（`dev_tools_section.dart`）补通用标签：`pid` 进程号、`child_running` 子进程在跑、`exit_code` 退出码、`base_url` 声明地址、`launch_mode` 拉起方式、`wait_error` 等待错误。未知键本来就会显示原始 key。`font_subset_test` 变红就改短标签，不要重做字体。

语音合成页：`SettingsSection.tts.description` 保持原文 `音色和出声的服务`（`settings_sections_test.dart` 禁止这句里出现 `TTS`）。`tts_section.dart` 顶部 `SectionHeader` 的 description 可改成：`这里是云端接口的地址。扩展只拉起进程，不会改写这个地址。` 文件头注释第 4 条改成同一边界。不要做「把 Mod 地址抄进设置草稿」的选择器。

`find.text('保存')` 会精确匹配。只改配置按钮的查找：`mods_section_test.dart`（`_tapSave`、禁用按钮、无 spec 没有保存按钮）、`mod_config_draft_test.dart`、`mod_state_surface_test.dart`、`voice_input_panel_test.dart`。不要改 `settings_scaffold.dart` / `settings_scaffold_test.dart` 的主保存、`confirm_discard_dialog.dart` 的「保存并离开」、`memory_panel_test.dart` 对话框里的「保存」、`env_key_field.dart` 的「保存密钥」。

### 注册与门禁

- 根 `Cargo.toml` 的 `members` 在 director 与 xtask 之间加入该 crate。复制起点是 `live2d-ai-mod-template`，模板本身仍不注册。
- `live2d-ai-desktop/Cargo.toml` 加一条 path 依赖。direct `[dependencies]` 因此 22 → 23。同一次改动把 `xtask/src/code_stats/mod.rs` 的 `RATCHET_DESKTOP_DEPS` 改为 23，注释写明这是一条新的产品 Mod 直边。`PLAN_DESKTOP_DEPS` **保持 22**。CI 的 `code-stats --check --only deps` 看的是棘轮。`--strict-plan` 会继续在 deps 上是红的，不要靠抬 PLAN 把它涂绿。
- `AVAILABLE_MOD_FACTORIES` 追加 `&live2d_ai_mod_local_tts::FACTORY`。测试改名为 `mod_count_is_six`，id 列表加上 `local-tts`（排序后与另外五个一起断言）。
- **不**进入 `cli_entry::default_mods_manifest`。断言 `m["mods"].get("local-tts").is_none()` 保留，失败文案改成：缺省停用、只注册；出声端点仍只由 `[tts]` 决定。`cli_entry.rs` 现为 775 行，已经算在「>500」的 44 个里，改注释可以，**不要**超过 1000 行。
- 每个新 `.rs` ≤500 行；`src` 下的测试文件同样计入，且测试文件 ≤800。`registry.rs` 改前先 `wc -l` 复核。
- Flutter：`mod_state_surface_test.dart` 的 `containsAll` 加上 `local-tts`（只断言旧五个时，漏注册也是绿的）。`test/support/registered_mods.dart` 注释里的 `mod_count_is_five` / 「五个」改成六。`voice-input` 的 `lib.rs` 头注释若点名 `mod_count_is_five`，只改这个计数词。`admin_api_test.dart` 头注释加一句：2026-10-09 重新注册的是拉起 Mod，不写 `[tts]`；夹具 JSON 不用为此扩成全量名单。

### 文档（只动这几处活句）

- `docs/architecture/mod-product-chain.md` §5：标题和「5 个」改成 6；表加一行 `local-tts`（off，拉起外部进程，不写 `[tts]`）；`mod_count_is_five` 改成 `mod_count_is_six`。历史数字保留。
- `docs/architecture/tts-is-core.md` **文末新开一节**，写本文件「口径 / 挂掉时」。第 1–5 节一字不改写成「当初就有这个 Mod」。
- `AGENTS.md` 只改现行句：约第 169 行「现行 5 个注册 Mod」补上 `local-tts`，「其余四个」改为「其余五个」；约第 189 行和第 340 行把活的 `mod_count_is_five` 改成 `mod_count_is_six`，并写明第六个是这个拉起 Mod。`action_tx` 休眠那句保持。变更历史段落不改。
- `docs/local-inference-setup.md` 只改 2026-09-11 那条横幅：端点仍只走 `[tts]`；`local-tts` 只拉起进程，不探活，不写 `base_url`。正文里的旧 curl 保持原样。
- 界面新字符串必须落在 `assets/fonts/*.ranges.txt` 里。

## 要改的文件

- 新建 `crates/live2d-ai-mod-local-tts/`（`lib.rs` 保持薄；决策与 argv 放纯函数文件；真 `Command` 单独文件；测试可拆）。
- `crates/live2d-ai-mod-system/`：`StartCause` 与两个函数，并从 `lib.rs` 导出。
- `Cargo.toml`、`crates/live2d-ai-desktop/Cargo.toml`、`main.rs`、`mod_registry/registry.rs`、新的 registry 测试兄弟文件、`web_api/cli_entry.rs` 的那条断言文案、`xtask/src/code_stats/mod.rs` 的棘轮。
- Flutter：`dev_tools_mod_config.dart`、`dev_tools_section.dart` 的标签表、`shell_admin.dart`、`main.dart` 注释、`tts_section.dart` 的头注释和那一句 description、上列四个 `find.text('保存')` 测试、`mod_state_surface_test.dart` 的 id 列表、`registered_mods.dart` 注释。另加一条源码守卫：用 `test/support/dart_library.dart` 的 `readLibrarySource('lib/main.dart')`（含 part），按函数签名切开 `_saveModConfig` 与 `_toggleMod`，断言前者体内没有 `_notifyModChanged`、后者仍有。切错范围导致恒真的写法不算数。
- 文档：上一节的四个文件，外加 `voice-input` `lib.rs` 与 `admin_api_test.dart` 的注释。

## 单测（删掉实现就必须红）

`live2d-ai-mod-local-tts`：

- `on_apply` + Boot / Enable 不调用 spawner；`on_apply` + Apply 调用；`with_app` + Boot 调用。
- argv 不含 `sh`、`-c`，也不把参数拼成一条字符串。`args_json` 里带空格的元素仍是一个元素。
- 非法 `launch_mode` 在 `create` 失败。非法 `args_json` 在不 spawn 的 Boot 下不失败，在 spawn 时失败。
- 要 spawn 时空 `program` 返回 `Err`。
- `shutdown` 会调用停止；停止失败时错误被返回，句柄还在。
- `launch` 在已有子进程时返回 `Err`；`stop` 在没有子进程时返回 `Err`。
- 全程 `apply_settings` 调用次数为 0（用计数间谍）。
- 工厂 id 为 `local-tts`，`api_version` 等于 `live2d_ai_mod_system::MOD_API_VERSION`，spec 的 key 唯一，缺省 `launch_mode` 是 `on_apply`。

宿主：桩工厂看到的 cause 分别为 Boot、Enable、Apply；`restart` 在 `shutdown` 失败时不再 `start`。

## 禁止

- 改 `crates/live2d-ai-runtime` 的 TTS 客户端、口型、wasm、外观、persona、director、memory 的行为。`voice-input` 只许改头注释和 `voice_input_panel_test.dart` 里那个配置按钮查找。
- 接回 `action_tx`。把 `local-llm` / `wallpaper` / `pet-desktop` 挂回来。在 Flutter 里嵌推理运行时。
- 改仓库里的 `mods.json`、`.env`、`live2d-ai.toml`。下载 CosyVoice2 权重。改 `/home/skystar/CosyVoice 3.0`（含垫片的 `_cv3()` 前缀）。停掉已经在 `127.0.0.1:8080` 上的垫片。
- 新的 WS 帧、健康检查路由、端点自动改写、失败后换云端、看门狗、吞掉 `Err` 再返回成功。
- 为 `local-tts` 做专用面板，除非该面板自己渲染 `ctx.state` 和 `ctx.stateError` 并登记进 `kModPanels`。本计划的选择是不加面板。
- 把 `RATCHET_SRC_RS_500` 或 `PLAN_DESKTOP_DEPS` 调高。把测试改成恒真。在仓库根目录跑 `flutter analyze`。
- 升版本、提交、打 tag、推送。再写一份计划或调研。

引擎换成哪一个是用户自己的 argv，不是本轮要改的垫片。

## 验收

都要 exit 0。贴出每条最后约 30 行。没跑的不要写成绿。

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

有时间再跑 `cargo test --workspace --all-targets` 和 `shell/flutter` 里的 `flutter test`。桌面测试若因 `HTTP_PROXY` 把传输失败说成 `llm_upstream_502`，先去掉代理再跑，不要改断言，也不要改 reqwest 客户端。

浏览器：服务已经在 `http://127.0.0.1:18080/app/` 时，打开 Mod 配置，确认按钮是「保存并应用」；对**未启用**的 Mod 保存一次，确认不出现「重新点火」常驻条。不要启用带真实 `program` 的 `local-tts`，也不要为此去点火。服务没开就写明浏览器没验，以 widget 测试为准。
