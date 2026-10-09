# 静默吞错与防御性续跑 · 交给下一个 agent

> 立档：2026-10-09。状态：**活**。真源是本文件。
> 维护者口径（原话）：「挂掉时不管。代码内的任何类似设计基本不做。不静默吞错误和写相关防御性代码。」
> 本轮**只查找、只登记**。不要改业务代码，不要提交，不要打 tag，不要推送，不要升版本。
> 两类 TTS 已按 `docs/plans/PLAN-tts-two-class-2026-10-09.md` 落地，**不要重做那一轮，也不要修下面的已知样本**。

## 口径

要找的是：失败发生之后，调用方或用户仍被当成成功，或者程序自己换一条路继续跑，把这次失败盖住。

| 类 | 算 | 用户侧 |
| --- | --- | --- |
| A 吞成成功 | `Result` / 异常被丢掉，函数返回成功或什么都不说 | 界面、HTTP、Mod 状态都看不出这次失败 |
| B 防御性续跑 | 失败后重试、换地址、换进程、保留旧配置继续，并且对外说「已保存 / 已生效 / ok」 | 用户以为用的是新配置或新进程 |
| C 错误还在、东西丢了 | `Err` 有返回，但句柄、槽位、草稿已经 drop / 清空，下一次无法再报、再停、再看到同一次失败 | 看到一条错误，随后状态对不上 |

不算：

- `unwrap` / `expect` / `panic`（响的，不是静默）。
- 测试里 `let _ = remove_dir` 这类收尾。
- 失败仍然是 `Err` / `Failed` / WS `error` 帧，只是附带正文读不到（例如错误响应体用了 `unwrap_or_default()`，请求本身仍返回 `Err`）。这种放进「附带信息」附录，不编号。
- 文档写明的产品取舍：断网缺字显示豆腐块、`on_apply` 在开机时不 spawn、`LIVE2D_AI_MUTE_AUDIO` 出零 PCM。
- 历史文档、`docs/audit/**`、发布说明里的旧描述。

判据是**用户能不能看见这次失败**。只在日志里有 `tracing::error!`、HTTP 却是 200、界面写「已保存」，算 A 或 B。Mod 状态已经是 `Failed` 且界面会画出这条状态，算看得见，放附录。

## 已知样本（先读，用来校准，不要改）

1. **C，已核对。** `crates/live2d-ai-mod-local-tts/src/lib.rs` 的 `stop_child`：`stop()` 失败会把句柄放回；`wait()` 失败走 `?`，局部 `child` 被 drop，`self.child` 已是 `None`。宿主 `disable` 仍会把这个已经没有句柄的 runtime 放回槽位。单测只钉了 `stop()` 失败（`tests_command.rs` 的 `shutdown_keeps_the_handle_when_stop_fails`），没有钉 `wait()` 失败。
2. **B 的形状，请打开调用链再分类。** `crates/live2d-ai-desktop/src/supervisor.rs` 热重载失败分支：日志「热重载失败：保留旧配置继续运行」，注释写 PATCH 在 web 模式是 HTTP 200。查前端保存文案是「保存失败，仍用旧配置」还是「已保存并生效」。前者算看得见，进附录；后者算 B。
3. **边界，请核对界面。** `ModRegistry::enable_caused` 在 `start_one` 把 Mod 标成 `Failed` 之后仍 `Ok(())`。`mods_routes.rs` 的 enable 因此回 `{"ok":true}`。若 Mod 列表把 `Failed` 画出来，进附录；若界面只认 `ok:true`，算 A。

## 怎么搜

工作树只认 `/home/skystar/Live2D-Ai`。用 `git grep -n`，不要 `grep -r`（会扫到 `target/` 与 `.git`）。

范围：`crates/`、`shell/flutter/lib/`、`scripts/`。跳过 `target/`、`shell/flutter/build/`、`docs/audit/`、`docs/releases/`、`docs/legacy` 已移出的历史。

先走用户碰得到的链，再铺开：

1. 设置保存与热重载（`supervisor.rs`、`web_api` 的 settings / env PATCH、Flutter `settings_controller.dart`）。
2. TTS / LLM 请求失败（`live2d-ai-runtime` 的 `tts.rs`、`llm`、会话引擎、WS `error` 帧）。
3. Mod 生命周期（`mod_registry/registry.rs`、各 `live2d-ai-mod-*` 的 `start` / `shutdown` / `command`、Flutter 启停与配置保存）。
4. 音频播放与口型（一句失败后是否跳过、改音量、或假装播完）。
5. 其余 `crates/**/*.rs` 与 `shell/flutter/lib/**/*.dart`。

建议的检索词（命中不等于发现，必须打开上下 30 行）：

```bash
git grep -n -E 'let _ =' -- '*.rs' '*.dart' '*.sh'
git grep -n -E 'unwrap_or(_default|_else)?\(' -- 'crates/**/*.rs' 'shell/flutter/lib/**/*.dart'
git grep -n -E '\.ok\(\)|map_err\(|if let Err' -- 'crates/**/*.rs'
git grep -n -E '保留旧|继续运行|fallback|retry|watchdog|health' -- 'crates/**/*.rs' 'shell/flutter/lib/**/*.dart'
git grep -n -E 'catch \(|catchError|onError' -- 'shell/flutter/lib/**/*.dart'
```

每条候选写下来之前回答三句：失败值去哪了；调用方接下来返回什么；用户在界面或 HTTP 上看到什么。答不出第三句就标「未核实」，不要编用户故事。

测试若把吞错写成断言（删掉吞掉的 `let _` 测试仍绿，或测试名承诺检查错误、断言却只看默认值），单独记一条，标「假绿灯」。

## 登记

只写一个文件：`docs/audit/2026-10-09-silent-errors/FINDINGS.md`。

头部写日期、状态「活」、以及「本文件是查找结果，不是修改授权」。正文用表：

| id | 类 | 位置 | 用户看到什么 | 证据（≤3 行） | 置信 |
| --- | --- | --- | --- | --- | --- |

id 从 `S-001` 起。已知样本 1 写成 `S-000`，类 C，置信高，避免后面的人再报一遍。样本 2、3 核实后用 `S-001` 往后编号，核实完不算的放文末「附录」。

不要改 `FINDINGS.md` 以外的文件。不要把密钥、口令、`.env` 的值写进去。单文件尽量短；同一调用点只记一次。

## 禁止

- 改任何 `.rs` / `.dart` / `.toml` / `mods.json` / `.env` / `live2d-ai.toml`。
- 修 `stop_child`、改热重载策略、改 enable 的返回值。那是下一轮维护者点头之后的事。
- 给失败加回退、重试、默认值，让检索变少。
- 重做 local-tts，挂回 `local-llm` / `wallpaper` / `pet-desktop`，接回 `action_tx`。
- 改 CosyVoice 目录，停 8080 垫片，下载权重。
- 提交、推送、升版本、再写一份计划。
- 在仓库根目录跑 `flutter analyze`。

## 提示词（整段复制给下一个 agent）

```text
你在 /home/skystar/Live2D-Ai，分支 main。先读 docs/plans/PROMPT-silent-error-hunt-2026-10-09.md。这是一次只读查找。不要改业务代码，不要提交，不要推送，不要升版本。两类 TTS 已经落地，不要重做，也不要修已知样本。

维护者口径：挂掉时不管。静默吞掉错误、以及失败后自己换路继续跑（重试、换地址、再拉进程、保留旧配置还对用户说成功），都是要找的。

三类才编号：A 吞成成功（Err 丢了，界面/HTTP/状态仍像成功）；B 防御性续跑（失败后换路，并且对外说已保存或已生效）；C 错误返回了，但句柄或状态已经丢，下一次对不上。unwrap/panic、测试收尾、失败仍是 Err 只是丢了附带正文、文档写明的产品取舍，都不编号，顶多进附录。判据是用户能不能看见这次失败。

已知样本先读再写进账本，不要改代码：
S-000 类 C：crates/live2d-ai-mod-local-tts/src/lib.rs 的 stop_child。stop() 失败会把句柄放回；wait() 失败时 ? 返回，局部 child 被 drop，self.child 已是 None。宿主 disable 会把没有句柄的 runtime 放回槽位。
supervisor.rs 热重载失败「保留旧配置继续运行」且注释写 web 模式 HTTP 200：打开前端保存文案再决定算 B 还是附录。
enable_caused 在 start 已标 Failed 时仍 Ok(())，enable 回 {"ok":true}：界面若画出 Failed 就进附录，若只认 ok:true 就算 A。

用 git grep -n，不要 grep -r。范围 crates/、shell/flutter/lib/、scripts/。跳过 target、build、docs/audit、docs/releases。顺序：设置保存与热重载，TTS/LLM 与 WS error，Mod 启停，音频与口型，再铺开。检索词：let _ =、unwrap_or、.ok()、if let Err、保留旧、继续运行、fallback、retry、watchdog、catch (。命中后打开上下 30 行。每条写清：失败值去哪了、调用方返回什么、用户看到什么。第三句答不出就标未核实。

只写 docs/audit/2026-10-09-silent-errors/FINDINGS.md。表列 id、类、位置、用户看到什么、≤3 行证据、置信。S-000 先占上。不要改别的文件，不要写密钥。不要为了让清单变短而加回退或默认值。
```
