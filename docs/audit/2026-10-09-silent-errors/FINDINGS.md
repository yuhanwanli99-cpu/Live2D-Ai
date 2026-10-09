# 静默吞错与防御性续跑 · 查找结果（2026-10-09）

> 状态：**活**。**本文件是查找结果，不是修改授权**——不要据此改业务代码；本轮只查找、只登记。
> 口径见 `docs/plans/PROMPT-silent-error-hunt-2026-10-09.md`：A 吞成成功 / B 防御性续跑并对外说已生效 /
> C 错误返回了但句柄或状态已丢。`unwrap`/panic、测试收尾、失败仍是 `Err` 只是丢了附带正文、
> 文档写明的产品取舍，都不编号，顶多进附录。**判据是用户能不能看见这次失败。**
>
> 方法：`git grep -n`（`crates/`、`shell/flutter/lib/`、`scripts/`；跳过 `target/`、`build/`、
> `docs/audit/`、`docs/releases/`），命中后打开上下 30 行。**注意**：两类 TTS 的
> `crates/live2d-ai-mod-local-tts/` 与若干新文件**尚未入库**，裸 `git grep` 扫不到它们——
> 本轮用 `git grep -n --untracked`；照抄提示词里的裸 `git grep` 会在 S-000 所在的 crate 上零命中。

## 计数

| 类 | 条数 | id |
| --- | --- | --- |
| A 吞成成功 | 6 | S-002 S-003 S-004 S-005 S-006 S-007 |
| B 防御性续跑且对外说已生效 | 1 | S-001 |
| C 错误返回、句柄/状态已丢 | 1 | S-000 |

## 台账

| id | 类 | 位置 | 用户看到什么 | 证据（≤3 行） | 置信 |
| --- | --- | --- | --- | --- | --- |
| S-000 | C | `crates/live2d-ai-mod-local-tts/src/lib.rs:102-114`（`stop_child`）+ `crates/live2d-ai-desktop/src/mod_registry/registry.rs:337-349`（`disable`） | 第一次停止失败：界面「操作失败」（409），开关仍是启用。**再点一次**：回 200「已停用」、「已停用」徽标——但子进程已被 kill 但无人 `wait`（僵尸/退出码永远拿不到），宿主槽位里那个 runtime 的 `self.child` 已是 `None`。 | `let Some(mut child) = self.child.take()` → `stop()` 失败才放回（lib.rs:103-109）；`wait()` 失败走 `?`，局部 `child` 被 drop，`self.child` 保持 `None`（lib.rs:110-113）。<br>`disable` 见 `Err` 就把 take 走的 runtime **放回槽位**再返回 `Err`（registry.rs:339-343），此时句柄已不存在。<br>单测只钉了 `stop()` 失败（`tests_command.rs:72`），`wait()` 失败无覆盖。 | 高 |
| S-001 | B | `crates/live2d-ai-desktop/src/supervisor.rs:467-475`（`apply_reload` 失败分支）+ `crates/live2d-ai-desktop/src/web_api/supervisor_slot.rs:108-112` | 保存设置后界面写**「已保存并生效」**（绿色成功态）。若重建失败，跑着的仍是旧 LLM/TTS client——失败只在后端日志里。手改 `live2d-ai.toml` 触发的那条路更彻底：设置快照会刷新（界面显示新值），引擎却保留旧配置。 | 失败分支只 `tracing::error!(... "热重载失败：保留旧配置继续运行")`，旧 engine 不动（supervisor.rs:467-474）；旧注释自己写着 web 模式 PATCH 端拿不到这个错误（:470-471）。<br>槽位非空时 **无条件** `existing.reload(); return ApplyStatus::Applied;`（supervisor_slot.rs:108-112）——`reload()` 返回 `()`，结构上没有回报通道。<br>前端按 `apply_status` 分流：`applied → '已保存并生效'`（settings_controller.dart:188-195、299-301），且 `isSuccess` 为真。 | 中（代码路径确定；触发需重建失败，PATCH 预校验使这一步多为竞态/外部改坏配置） |
| S-002 | A | `crates/live2d-ai-desktop/src/web_api/dispatch.rs:103-108`（`PUT /api/v1/env`）+ `crates/live2d-ai-desktop/src/web_api/env_routes.rs:149-155` | 保存密钥后界面写**「已写入 KEY，立即生效（不用重启）」**。若 supervisor 当时装不起来（配置里 `base_url` 还空着 / 解析不过）或热重载失败，密钥只是写进了 `.env`，链路并没有生效——响应体里根本没有能表达这件事的字段。 | `Ok(resp) => { ctx.ensure_supervisor_after_patch(); resp }`——返回的 `ApplyStatus`（`RestartRequired` / `NoSupervisor`）被**丢掉**，响应仍 200（dispatch.rs:103-108）。<br>`handle_put` 只回 `{key, set}`（env_routes.rs:149-155），前端 `write` 只看 statusCode 与 `set`（env_api.dart:94-104）。<br>文案写死：`'已写入 ${status.key}，**立即生效**（不用重启）'`（env_key_field.dart:77-80），调用方 `_saveEnvKey` 连 bool 都不看（shell_admin.dart:112-113）。 | 高 |
| S-003 | A | `crates/live2d-ai-desktop/src/mod_registry/registry.rs:106-171`（`persist_manifest`），调用点 :327 / :347 / :398 | 启用/停用开关、保存 Mod 配置后界面报「已启用 X」/「X 配置已保存」、HTTP 200 `{"ok":true}`。磁盘写失败（只读目录 / 满盘 / rename 失败）时**只有后端日志**；列表读的是内存态，所以界面一切正常——重启后开关和配置回到旧值。 | `persist_manifest` 三条失败路径都只 `tracing::error!` 后 `return`，函数返回 `()`（registry.rs:147-171），调用方 `self.persist_manifest();` 无返回值可查（:327/:347/:398）。<br>代码注释把这条写成 best-effort：「HTTP 不该因为磁盘只读就谎报操作失败」（registry.rs:106-107）——**代码注释不是文档口径**，故仍登记。<br>`enable_caused` / `disable` / `reload_config` 因此一律 `Ok(())` → `{"ok":true}`（mods_routes.rs:318-319）→ 界面「已启用/已停用/配置已保存」（shell_admin.dart:221、245）。 | 高 |
| S-004 | A | `crates/live2d-ai-mod-persona/src/runtime.rs:405-438`（`shutdown`）+ `crates/live2d-ai-desktop/src/mod_registry/registry.rs:337-349`、:291 | 停用 persona 后界面「已停用 persona」、状态徽标「已停用」。若写回主链 `system_prompt` 失败（配置不可写），主链上仍挂着导入卡的人设——失败只在日志里，而且这条 `logger.error` 会被宿主记成 **info**。 | 还原失败分支：`logger.error(...)` 之后**照常** `self.registered = false; Ok(())`（runtime.rs:426-438）。<br>`ModLogger::new(|_lvl, msg| tracing::info!(...))`——Mod 的日志级别被丢掉，error 也落成 info（registry.rs:291）。<br>`disable` 收 `Ok` → `ModStatus::Disabled` + 写回 manifest + 返回 `Ok`（registry.rs:344-348）→ 200 `{"ok":true}`。 | 中（触发=配置不可写；链路确定） |
| S-005 | A | `shell/flutter/lib/audio/audio_player.dart:363-366`（DOM `error`）、:345-356（`play()` 异常）、:300-311（建元素/blob 失败）、:456-459（`currentTime` 抛） | 某一句话音频解码/加载失败：**跳过这一句、下一句照放**，界面不报任何音频错误，反而显示「点击任意位置或按任意键以启用声音」+「启用声音」按钮——把「这一句坏了」讲成「你还没启用声音」。按下那个按钮也修不回来（该句已被 release）。 | `_onError` 只做 `_setUnlocked(false)` 然后 `_finish(item)`（跳过本句、放行下一句）（audio_player.dart:363-366）；`_play` 的 `catchError` 同样只 `_setUnlocked(false)`（:345-356）。<br>`_finish` → `_queue.finishCurrent()` + `_startNextIfIdle()`——队列继续，无上抛、无事件（:369-389）。<br>`audioUnlocked == false` 是界面唯一反应：`'点击任意位置或按任意键以启用声音'`（audio_bar.dart:210-222）。 | 高 |
| S-006 | A | `crates/live2d-ai-desktop/src/mod_registry/events.rs:36-48`、:64-67 + `crates/live2d-ai-desktop/src/mod_registry.rs:162-164` | 本轮 memory / persona 的会话注入若没写进去（事件被丢 / worker 已死），界面与 HTTP 都看不出来：这一轮就是少了注入，没有错误码。计数器字段存在，但从不自增、也没有读出的面。 | `dispatch_event_and_flush` 等到回执超时也只 `let _ = ack_rx.recv_timeout(...)`，随后返回 `queued`（= 入队成功，不代表处理完）；唯一调用方 `mod_event_sink` 连这个 bool 也丢（`let _ = `）（events.rs:42-48、:96-105）。<br>`dropped_events` 在结构体里注释为「可观察的 dropped 计数」（mod_registry.rs:162-164），但全仓无 `fetch_add`，getter 是 `#[allow(dead_code)]`（events.rs:64-67），worker 形参是未使用的 `_dropped`（:115-119、registry.rs:68-73）。<br>`try_send` 失败只返回 `false`，不计数、不记录 topic（events.rs:59-61）。 | 中（代码确定；「注入少了」本身无错误面） |
| S-007 | A | `crates/live2d-ai-desktop/src/web_api/app_routes.rs:192-208`（`refresh_from_disk`）+ `crates/live2d-ai-desktop/src/web_api/dispatch.rs:148-152` + `crates/live2d-ai-desktop/src/web_api/cli_entry.rs:349-357` | 用户手改 `live2d-ai.toml`（写成语法错）或改坏 `.env`：界面与 HTTP **一个字都没有**，设置面板还是旧值，改动既没生效也没被拒——界面不解释，只有后端日志里一条 warn / error。 | `refresh_from_disk` 解析失败 `tracing::warn!` + `return false`（app_routes.rs:195-201）；dispatch 侧只在成功时打 `debug!`，失败分支不存在（dispatch.rs:148-152）。<br>file_watcher 的钩子同样只消费返回值的一半：`if refresh_from_disk(..) { info! }`（cli_entry.rs:349-357），`PostReloadHook` 签名是 `Fn()`，false 连传都传不出去（file_watcher.rs:37、:193-196）。<br>同一次外部改动还会接着走 `apply_reload` 的失败分支（保留旧配置，见 S-001）。 | 中 |

## 附录（核实过、按口径**不编号**）

1. **已知样本 3：`enable_caused` 在 start 已 Failed 时仍 `Ok(())`**（`registry.rs:321-329` → `mods_routes.rs:319` 回 `{"ok":true}`，界面 toast 是「已启用 X」）。**不进编号**：Mod 列表把状态画出来了——`statusLabel` 认 `failed` → 「出错」（mods_api.dart:249-260），`_toggleMod` 成功后 `_loadAdmin()` 会重取列表并把徽标刷成「出错」（shell_admin.dart:214-232、dev_tools_mods.dart:118-135）。同一形状的 `restart` 也一样：`{"ok":true,"restarted":true}` + 「已保存并生效」（dev_tools_mod_config.dart:362-371），随后徽标同样会刷新。
2. **`ModRegistry::shutdown` 的 `let _ = rt.shutdown()`**（`events.rs:70-82`：失败被吞、状态照写 `Disabled`）与 S-000 同形状，但**工作树内无调用方**（`git grep '\.shutdown()'` 在 desktop crate 只命中它自己与各 Mod runtime 的 `shutdown`）——目前不可达，登记备查。
3. **`local-tts` 子进程自我退出后再 disable**：state 已如实报 `child_running=false` + `exit_code`，但那时再 `kill()` 一个已被 `try_wait` 收尸的 pid 在部分平台回 `Err` → `disable` 失败、Mod 停不掉。**置信低/未核实**（Rust 在 reap 之后再 kill 的行为未实测；且维护者口径是「挂掉时不管」）。
4. **`ModRegistry` 无 `Drop`**：进程退出时没人调 `shutdown()`，`local-tts` 拉起的子进程随宿主退出而成为孤儿（未编号：不是被吞的 `Err`，是没有清理路径）。
5. **`read_settings` 读失败静默回 `{}`**（`cli_entry.rs:273-287`）→ 所有 Mod 拿到空设置视图（persona 的 `current_main_prompt()` 会得到空串）。**组合不可达/未核实**：同一次读失败会让 `apply_settings` 也失败，persona 那条路会以 `Failed` 收场（可见）。
6. **`run_forever` 启动装配表演层用 `if let Ok(settings) = …`（`supervisor.rs:651-661`）**：解析失败就静默不装配，连日志都没有。与 S-001 同一分支族，触发窗口极窄（build_web_supervisor 已先要求解析成功）。
7. **`load_director_takeover`：`mods.json` 坏 JSON → 静默当「导演未启用」**（`performance_wiring.rs:102-116`），于是可能改用 `[performance]` 自己的端点。注释说与「缺文件」同一口径——那是**缺文件**的口径，损坏文件没写。
8. **文档写明的产品取舍（不编号）**：二路按句清洗失败一律回原文（`cleaning.rs:15-19、141-175`，用户会听到括号被念出来，但无错误码）；memory 失败只 warn + 本轮 no-op，但每条失败都进 `state_json.errors`（`lib.rs:149-154、:832`）；director 二路 LLM 失败静默回落规则层，但 `degraded_note`/计数可观察（`staging.rs:43、163-194`）；`tts.rs:63` 的 `unwrap_or_default()` 只丢错误响应体（请求本身仍 `Err`）；`AppStatus.audio.backend="none"/available=false` 是浏览器出声架构下的如实字段（`app_routes.rs:267-272`）。
9. **测试侧观察（不算假绿灯，登记）**：S-000 的 `wait()` 失败无覆盖（`tests_command.rs:72` 只钉 `stop()`）；`supervisor/tests_reload.rs:415 reload_without_config_path_is_noop` 把「Reload 收到但 `config_path` 为 None → 静默 no-op」**写成断言**（测试专用装配，未影响生产路径）；`enable_caused`/`persist_manifest` 的失败路径无任何断言（`tests_lifecycle.rs` 只验成功路径的磁盘回读）。`scripts/ignition-precheck.sh` 会累计 FAIL 并以 exit 1 收口（正常）；`scripts/ignite.sh:178-180` 的 `probe ... || true` 会把未就绪打成 `[warn]` 继续点火——**warn 在屏幕上**，不算吞。
10. **本轮未核实的用户面**：S-001/S-007 的触发频率（重建失败的可达性）未用真机复现，只做了静态链路核对；S-004 需要「配置不可写」这类环境条件，未实测。

## 建议的下一步（不是授权）

- 先由维护者裁决 S-000（`wait()` 失败也把句柄放回或明确标记「已失去句柄」）与 S-002（env PUT 把 `ApplyStatus` 回给前端）这两条**改动面最小、用户面最直接**的。
- S-001 / S-007 的修法方向是「让热重载的成败有一条回到 HTTP/界面的路」，但会牵动 PATCH 语义与前端文案，属下一轮。

## 2026-10-09 追加（0.2.3-rc.1）：只有「子进程立刻非 0 → Failed」这一条被修

维护者口径（`docs/plans/PROMPT-0.2.3-rc.1-oob-2026-10-09.md` §MeloTTS）：**只修**
「第一次启动装 venv 失败 / 进程马上以非 0 退出」这一条，**其余 S-000…S-007 不动**。
上面台账**一字不改**（它是查找结果，不是修改授权）。

- **现状**：`live2d-ai-mod-local-tts` 在 spawn 之后有一个 1 秒观察窗口
  （`IMMEDIATE_EXIT_GRACE`）；窗口内就退出（非 0 / **0** / 信号）⇒ `start` 返回
  `Err`，宿主把它标成 `ModStatus::Failed { message }`，`message` 里**带脚本印出的
  原因**（子进程 stderr 的尾巴，读线程同时把 stderr 透传到宿主终端）。观察本身
  失败（`try_wait` Err）也 Err 并停掉子进程——「没法确认它在跑」不写成「已启用」。
- **本条目不在上面的编号里**（它属于「附录 3 / 已知边界」那一族：旧口径明说
  「脚本内部失败发生在子进程里，Mod 不会把它翻成 Failed」）。所以本次是**新增编号外
  的修复**，不是改 S-000。回归：`crates/live2d-ai-mod-local-tts/src/tests_start_failure.rs`
  （5 条）。
- **仍未修**：S-000（`wait()` 失败丢句柄）、S-001…S-007，以及附录 1–10。
