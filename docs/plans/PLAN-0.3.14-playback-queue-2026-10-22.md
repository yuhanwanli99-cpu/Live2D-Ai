# 0.3.14 弹幕等播完，动作按句留下

> 状态：**已派、未执行**（2026-10-22）。不并进已落地的 0.3.13，也不并进未开始的 `0.4.0` / `0.4.1`。
> `Cargo.toml` 保持 `0.3.1`，`pubspec` 保持 `0.3.1+22`。写法 `0.3.14-rc.z`：执行 agent 按代码拆 `z`；每个 rc 一次本地提交 + 一次本地附注 tag，**不 push**。最后一个 `rc.(z+1)` 只清理，活文档放在这一笔。
> 起点是 `c251f11`。不改 `.env`、仓库根 `mods.json`、`live2d-ai.toml`。不提交 `official_live2d_samples.7z`。不占 18080，不杀已有进程，不下载 Melo 权重，不点火。

## 工作区

`/home/skystar/Live2D-Ai` 上有未提交的七 Mod 挂载实验，不是版本。脏的有 `Cargo.toml`、`Cargo.lock`、desktop 依赖、`main.rs`、`web_api/mod.rs`、`external_routes.rs`、`a2a_gate.rs`、`cli_entry.rs`、`voice_routes.rs`，未跟踪 `web_api/a2a_gate/` 与 `web_api/external_coalesce.rs`。

不要提交这些，不要 `git restore` / `git stash` / `git clean` 整棵树，不要 `git add -A`。`external_coalesce.rs` 不入库。工厂保持 0。desktop 依赖棘轮保持 18。

`web_api/mod.rs` 以 `git show c251f11:crates/live2d-ai-desktop/src/web_api/mod.rs` 为准（709 行；`ServerContext` 到 `folder_processes` 为止，没有 `external_coalescer`）。先把工作区副本另存到 `/tmp`，不要覆盖已有的 `/tmp/live2d-ai-mount-7-mods.tracked.patch`。提交只含本单改动；然后把实验副本放回，且不要再提交。放回后若要在本机编译，只在未暂存的实验文件里补上新字段。`git show --stat` 不得出现工厂表、path 依赖或 `external_coalesce`。

## 原因

不要再调查。三件事：

1. `turn_in_flight` 在 `supervisor/run.rs` 里 `run_one_turn` 返回后立刻清掉（约 211 行）。这是生成结束，不是浏览器播完。
2. 旧行为把后来的弹幕用换行拼进同一次 `say`（`v0.3.1`）。`external_coalesce.rs` 不在 `c251f11` 上。
3. `DirectorCuePlan.replace` 整表清空（`director_cue_plan.dart` 55–61 行）。后一帧丢掉还没开播的 `sentence_seq`。`UserSubmitted` 不推进 epoch，序号每轮从 1 起。

## 行为

- `text_delta` 仍到达就上屏。不推迟字幕。`AudioPlayer` 仍一句一句播。不改串行队列，不改 `_finish` / duration。
- `Stop` 与 `new_epoch` 仍立刻打断音频，并 `clear` 计划。聊天框自己的下一句不等待，仍直接 `say`。
- 下一次**外部**注入要同时满足 `turn_in_flight == false` 和播放不忙，然后才 `say` 一条。多出来的是 FIFO。禁止用 `\n` 拼成一条。
- 待发字符合计上限 2000。放不下的那一条丢掉，已在队列里的留下。超时不得倒掉队列。
- 播放忙缺省是闲。没有浏览器上报时，外部队列只看 `turn_in_flight`，无头调用不卡住。`busy: true` 之后约 120 秒没有新上报，记一条 warn，视为闲。不丢队列。
- 同一轮后一句的 `action_cue` 只按 `sentence_seq` 覆盖这一条。同 seq 留最后一条。空 `preset_id` 仍是不动。`none` 仍是该句音频开始时撤销一次。`take` 仍在开始播放时取一次。空 `cues` 不清其它 seq。
- `clear` 只在：停止、`new_epoch`、baseline（仍立即应用到舞台，不写入按句 map）、换模、被接受的新 `user_line`（重复帧不清，见 `userLineAction`）、播放从忙变闲且聊天不在 `streaming`。
- 外部注入等播放闲才开下一轮，序号从 1 重计时计划已空。聊天框重叠时，新的 `user_line` 会清掉还没播的上一轮动作。

## 实现

新文件各自连测试 ≤500 行。不用进程级 `OnceLock`。

- `web_api/playback_gate.rs`：`POST /api/v1/playback`，body 只有 `{"busy":true|false}`。缺布尔或非法 JSON → 400。注册进 `dispatch` 的现有 mutating 校验（Origin、`application/json`），不要做成绕过 dispatch 的前置路由。`dispatch.rs` 现 315 行，只留一行调用。状态是 `ServerContext` 的新字段（忙标志 + 最后上报时刻）。`web_api/mod.rs` 只加字段和 `mod playback_gate`。该文件已是超 500 的 22 个之一：不写逻辑，不拆文件。`git grep -n "ServerContext {" c251f11` 的每一处都补上字段。读标志时若距上次 `true` 超过 120 秒，视为闲并 warn 一次。
- `web_api/external_queue.rs`：`push` 收一条；`pump` 在 `!turn_in_flight && !playback_busy` 时至多 `say` 一条。`say` 返回 false 则头一条留着。不拼换行。不要从 `external_routes.rs` 调用。`c251f11` 上 `POST /api/v1/external/chat` 仍是 404 `mod_disabled`。挂回 `external-input` 不是本版；将来挂回时宿主调用 `push` / `pump`，禁止再恢复换行合并。
- Flutter：`DirectorCuePlan` 增加 `upsert` 与 `clear`。`shell_lifecycle_wiring.dart` 里非 baseline 的 `ActionCueEvent` 改 `upsert`。停止（约 60 行 `clearCuePlan`）、换模（约 179 行）、baseline、新 `user_line`、播放转闲且不在 streaming，改走 `clear`。`hasPending` 翻转时静默 POST `/api/v1/playback`。失败不影响播放。不新增界面文案。不得已的中文必须通过字体子集测试；缺字就改措辞，不换字体，不引用 gstatic。
- 不改 `launcher.rs`（441）、`mods_scan.rs`（497）、`folder_spawn.rs`（488）、`main.rs` 工厂表。`mod_count` 仍是 0。不改 `mod-product-chain.md` §8 与 `mod-community-license.md` §2。不挂回已封存 Mod。不接 `action_tx`。不新增预设。不改对话模型与 `[tts]`。

## 测试

改 `shell/flutter/test/action_cue_test.dart`：约 160、201–203、225–234 行现在要求整表替换，约 285 行要求源码含 `_directorCues.replace(event.cues)`。改成 upsert，不要留红。空列表不再等于清空。`clear` 后长度为 0。seq 2 还在时 upsert seq 3，seq 2 仍可 `take`。

Rust：不能发时连收两条，之后两次 `pump` 依次得到原文，断言没有拼起来的 `\n`。窗口满只丢当条。播放缺省闲；`busy true` 满 120 秒视为闲且队列还在。外部聊天仍是 404 `mod_disabled`。扫描仍不执行 entry。

提交前：`cargo test -p live2d-ai-desktop --all-targets`、`cargo fmt --all -- --check`、`cargo clippy -p live2d-ai-desktop --all-targets -- -D warnings`。改了 Dart 再跑 `cd shell/flutter && flutter analyze && flutter test`（`flutter` 在 `$HOME/flutter/bin`；先清 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` 及小写同名）。全 workspace `cargo test` 不是阻断；没跑就在发布说明里写明。浏览器没看就写没看。

## 文档（只在最后一个清理 rc）

本单和索引从「已派、未执行」改成「已落地」，写上 tag。`docs/external-input.md` §4.1 改为：下一次外部注入等播放闲且本轮生成已结束；多条是 FIFO；合计 2000 仍只丢当条。写明缺省二进制该路径仍是 404，挂回时走 `external_queue`。

索引已有：`CURRENT.md` §2、`DOC-MAP.md`、`docs/README.md`、`PROMPT-3.0-motion-first-2026-10-17.md`。发布说明 `docs/releases/v0.3.14-rc.N.md`。QQ 不写进 `README.zh-CN.md`。不改 `AGENTS.md` 历史段、`CHANGELOG.md`、旧发布说明。

`cargo run -q -p xtask -- code-stats --check --only docs` 必须通过。预算仍是 45000。清理 rc 改现有句子，不要新增长文。不提高 `DOCS_BUDGET_LINES`，不放宽 `src>500`（仍 22）与 desktop deps（仍 18）。
