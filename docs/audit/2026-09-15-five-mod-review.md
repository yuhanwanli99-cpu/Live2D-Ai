# 五 Mod 实现审查 + 同类实现调研（2026-09-15）

> **来源（2026-09-28 入库）**：本文原为旧 worktree `/home/skystar/Live2D-Ai` 的**未跟踪唯一副本**，
> 2026-09-28 判定**并入 0.2.0 线**（Mod 侧审查，属挂账：S2/S3/S4/S5 等）。
>
> **审查对象**：分支 `mod/l1-product`（worktree `/home/skystar/Live2D-Ai-l1`，HEAD `d2bdd4fd`
> + 未提交的 core-next 三件套工作区改动，即当前「5 Mod」实现）。
> **五个注册 Mod**：`external-input` / `persona` / `voice-input` / `memory` / `director`
> （`main.rs::mod_count_is_five` + id 集合双断言；`wallpaper` / `pet-desktop` 封存、
> `local-llm` 废除启动，见 `ARCHIVED-mods.md`）。
>
> **方法**：契约/宿主接线逐文件静态审查 + 4 条并行子审查（persona+memory /
> external+voice+director / 宿主集成 / 联网同类调研）+ 独立重跑门禁。
> **本审查只读**，未改动被审查 worktree 的任何文件。
>
> **独立实测（本审查亲自跑）**：
>
> ```text
> cargo test -p live2d-ai-mod-system -p ...-external-input -p ...-persona \
>            -p ...-voice-input -p ...-memory -p ...-director
>   → system 13 / external-input 37 / persona 69 / voice-input 47 / memory 95 / director 36
>   → 297 passed, 0 failed（全绿）
> cargo fmt --all -- --check                            → FMT_OK
> cargo clippy <6 crates> --all-targets -- -D warnings  → 0 error / 0 warning
> ```
>
> **注意**：门禁是绿的，下面所有问题都**不是单元测试能抓到的类型**——它们是跨层锁、
> 日志/指标、进程装配路径、文档边界与前端-服务端契约。多条缺陷恰好落在「测试怎么测
> 都是绿的、产品上却是错的」这一类（自检说谎 / 静默降级 / 静默丢写）。

---

## 0. 结论摘要

**骨架已经很成熟**：静态注册 + 工厂/实例分离 + 一等能力注入（`say`/`apply_settings`/
`settings`/`session_prompts`）+ 有界 channel 独立 worker + 纯数据 `settings_spec` +
会话多 owner 槽 + 封存/废除台账 + 高密度反向回归。**方向与同类最佳实践一致**（见 §6）。

**但「宿主接线」层有 6 个必须先处理的严重问题**，集中在四类：
① **安全**：写入 `.env` 的 token 端点看不见 → 静默不鉴权；保存任意 Mod 配置会抹掉已存 token。
② **装配/诚实性**：动态装配 supervisor 的路径不给 Mod 注入 HostChannels，Mod→host 回路永久失效，
却回 `Applied`；无 HostChannels 时 `say` 空实现返回 `true`，test_inject 谎报 accepted。
③ **正确性**：memory 会话注入把全局 `persona.system_prompt` 整段顶掉。
④ **可观测性/治理**：Mod 启动失败无日志、无 API 原因；director preset 实际形成一条绕过
core 仲裁的下行通道，而 AGENTS.md 仍写「零投递」。

**计数**：严重 6 / 中 12 / 低 16（含规范偏差）。建议按 §7 的 P0→P2 顺序收口。

---

## 1. 五 Mod 现状一览

| Mod | 缺省 | 职责 | 用到的宿主能力 | 亮点 | 主要风险 |
|---|---|---|---|---|---|
| `external-input` | **on** | 弹幕/礼物经 sidecar `POST /api/v1/external/chat` → `say_tx`；壳内 `test_inject` | `say_tx`、config、state、command | token 只回 `token_set`；计数与 handler 分支逐条对应；与 HTTP 共用同一渲染纯函数 | token 读法绕过 `.env`；逐 token 把助手正文 info 进日志 |
| `persona` | off | 酒馆 V1/V2 JSON / PNG `chara` → `system_prompt`，L1 起按会话绑定 | `apply_settings`、`settings`、`session_prompts`、command | 坏配置显式 `Failed` + 回滚；档案 tmp+rename；50 会话上限 | 超 64Ki 注入被宿主静默丢弃仍报「已恢复」 |
| `voice-input` | off | 转写 → 清洗 → 唤醒闸 → `say_tx`；ASR 在 Python sidecar | `say_tx`、command、state | 闸门是纯函数；argv 不过 shell；失败码/退避；跨语言缺省词对账 | 一键 sidecar 无 Running 互斥/无超时/stderr 全量入内存 |
| `memory` | off | 本地 JSONL + 词元重叠 top-k → 写会话注入槽（无会话降级全局写回） | `session_prompts`、`apply_settings`、`settings`、command | owner 槽避免误伤 persona；物理淘汰有上限；坏行可见 | 注入顶掉全局 persona；只记用户输入不记回复；无字节上限 |
| `director` | off | `TurnPrompt`/`TurnEnded` → 纯函数 `{emotion,intent,suggested_tts,preset_id}` | 事件订阅、state、command | crate 层红线可测（从不触碰 `action_tx`/`apply_settings`）；日志不写正文 | preset 经前端直写渲染面参数，绕过 core 仲裁 |

---

## 2. 严重（P0，建议先修）

### S1 外部/语音 token 绕过 `.env` 真源 → 配了 token 却静默不鉴权

**证据**：`web_api/external_routes.rs:248`、`web_api/voice_routes.rs:215`、
`live2d-ai-mod-external-input/src/lib.rs:155` 都用 `std::env::var(TOKEN_ENV_VAR)`。
项目口径是「`.env` = 唯一密钥真源，读取只能走 `live2d_ai_runtime::secrets::lookup`」
（AGENTS.md；`secrets.rs:1-25` 明说快照**不回灌**进程环境；`app_routes.rs:183` 有同款教训注释；
`IGNITION-CHECKLIST-stabilize.md` §1.2 明确写「设在哪：`.env`」）。

**后果**：只在 `.env` 写 `EXTERNAL_INPUT_TOKEN`/`VOICE_INPUT_TOKEN`（未经 `ignite.sh`
导出进程环境、或运行期手改/界面写 `.env`）时，端点取不到 token → 按「都空 = 不鉴权」放行；
`state_json.token_set` 同时报 `false`，面板与真实鉴权状态互相矛盾。
**现有 token 测试全部以 `if std::env::var(TOKEN_ENV_VAR).is_ok() { return; }` 跳过**，所以无回归。

**建议**：三处改 `secrets::lookup(TOKEN_ENV_VAR)`（沿用 `settings_routes/test_endpoints.rs:159`
的同类修复）；补「进程环境为空、只有 `.env` 有值」的回归。

### S2 动态装配路径不注入 HostChannels + 无 HostChannels 时 `say` 谎报成功

**证据**：`web_api/supervisor_slot.rs:101-147` 的 `ensure_after_patch` 动态创建 supervisor，
只把 `mod_registry` 用于 host→Mod 事件桥（`:116-117`），**不重建 registry、不注入 HostChannels**。
而 `cli_entry.rs:267-273` 在「无配置文件 → 纯控制平面启动」时创建的 registry 没有 HostChannels。
于是用户在 GUI 里首次保存 LLM 配置（PATCH → 动态装配 supervisor，返回 `Applied`）之后，
Mod 侧仍是：`session_prompts = disabled`、`apply_settings` 恒拒、`say…`。
`mod_registry.rs:320-322` 的无 HostChannels 兜底是 `Arc::new(|_: String| true)` ——
**返回 true**，于是 `test_inject`/`voice` 注入报告「accepted」而文本根本没进主链。

**后果**：新用户路径（先控制平面、后配 LLM）下 persona/memory/voice 静默失效，
API 回 `Applied`（前端 L1 的「重启提示」又只对 Mod 变更触发）——失败被伪装成成功。
**建议**：装配成功后给 registry 注入/重建 HostChannels（`set_host_channels`），
随 `Applied` 一起切换；无 HostChannels 的 `say` 空实现必须返回 `false`（并让路由回 503）。

### S3 保存任意 Mod 配置会抹掉已存的 secret token

**证据**：`mods_routes.rs:483-499` 的 `redacted_config` 把 `secret=true` 的 key **整个移除**；
前端 `settings/sections/dev_tools_section.dart:684-701` 的 `_buildConfig` 以「服务端回传（已脱敏）的
config」为合并基，遇到空的 secret 字段 `continue`（本意「留空 = 不修改」）；
服务端 `mod_registry.rs:455-459` 的 `reload_config` 是 `e.config = config` **全量替换**。
三方叠加：用户先填 token 保存成功 → 之后再改 prefix/template 等任意字段保存 → token 消失。

**后果**：不只是配置丢失，而是**安全降级**——external/voice 端点从「要 token」静默变成「不鉴权」，
面板不会报错。**建议**：服务端按 key 合并（或对 secret 提供 `preserve` 语义），
前端用「未修改」哨兵；并补一条跨端契约测试：保存非 secret 字段后服务端仍要求 token。

### S4 memory 会话注入把全局 `persona.system_prompt` 整段顶掉

**证据**：`memory/src/lib.rs:445-455` 的 `inject_into_session` 用
`strategy::compose_injection("", &memories)`（**base 恒为空串**）写 memory owner 槽；
supervisor `supervisor.rs:633-635` 的 `engine.set_system_prompt_override(\n  session_scopes.prompt_for(session))`
只在**该会话一个槽都没有**时才得到 `None` → 回落全局。
因此只要该会话有 memory 命中、而没有 persona 卡，组合结果就是「只有记忆块」——
全局 `persona.system_prompt` 在本轮被整段替换。
`docs/architecture/memory-mod-v0.md` §14.2 明确写的是
「base = 该会话当前覆盖…**没有才回落全局** `persona.system_prompt`」，实现与文档不符。

**后果**：启用 memory 后，用户配置的全局人设会在有记忆命中的轮次里静默消失（不是叠加，是替换）。
**建议**：宿主组合时把全局 prompt 作为**最低优先级 base**（表里有会话槽时也参与合并），
或明确裁决「会话槽即替换全局」并同步文档/面板；无论哪种，都要补一条
「全局 system_prompt + 会话 memory 槽共存」的 effective-prompt 端到端断言。

### S5 Mod 启动失败：无日志、无 API 原因 → 界面/日志只有 `failed`

**证据**：`mod_registry.rs:302-348` 的 `start_one`：`factory.create`/`runtime.start` 返回
`Err` 时只写 `status=Failed{message}`/`last_error`，**没有任何 `tracing`**（只有 api_version
不匹配那条有 warn）。而 `mods_routes.rs:343-370` 的 `GET /api/v1/mods` 只序列化
`"status": status.as_str()`，**从不带出 `last_error`**（全仓 `last_error` 只在
`mod_registry.rs` 内读写，无任何调用方）。

**后果**：persona「坏配置显式 Failed」的成果在界面上只剩一个没有原因的 `failed`，
后端日志同样空白；与 AGENTS.md「链路错误必须同时出现在后端日志与前端」「禁止静默错误路径」
直接冲突，也与 L1 目标「失败可读」冲突。**建议**：失败分支加
`tracing::error!(target:"mod", mod_id, error)`；`GET /api/v1/mods` 条目带脱敏后的
`last_error`/`failed_reason`。

### S6 director 的 preset 实际形成一条绕过 core 仲裁的下行通道（治理）

**证据链**：`shell/flutter/lib/main.dart:344-375`（`_applyDirectorPreset` 拉
`GET /api/v1/mods/director/state` 取 `latest.preset_id`）→
`live2d/live2d_bridge.dart:230-236`（协议 v1 `preset`）→
`crates/l2d-wasm-demo/src/main.rs:388-397` → `crates/l2d-wasm-demo/src/preset.rs:48+`
（写 `ParamMouthForm`/`ParamEye*`/`ParamBrow*`/`ParamAngle*`）。
crate 层「零调用 `action_tx`/`apply_settings`」为真，但产品上确实存在一条
「Mod 决策 → 前端拉取 → 渲染面写参数」的表演下行通道，且不经 core 仲裁；
AGENTS.md 仍写 director「只写日志 + state_json，零投递」、红线写「不得绕过 core 仲裁」。

**建议**：明确裁决——要么把它写成新的一等「表现通道」（同步 `core-chain-baseline.md`
与 AGENTS.md），要么撤回为纯展示；若保留，补「前端只按 id 查表、不透传任意参数」的
机器可检回归（`preset.rs` 已有 `presets_stay_inside_the_allowed_channel`）。

---

## 3. 中（P1）

### M1 一次 Mod panic 会拖垮所有 Mod 与所有 mods 路由

- `on_event`/`on_scoped_event` panic → `event_worker_loop` 线程 unwind 死亡，
  **所有** Mod 从此收不到事件且无告警；槽锁中毒后 `runtime_state`/`command` 恒 503。
- `start_one` 在 registry 锁内跑 `factory.create`/`runtime.start`；`mods_routes.rs:163-166,210`
  在锁内跑 `registry.command`（任意 Mod 代码）。Mod panic → registry `Mutex` 中毒 →
  之后每个 mods 路由的 `.expect("mod_registry mutex poisoned")` 都 panic。
- `mod_event_sink`（`mod_registry.rs:633-642`）用 `let Ok(reg) = registry.lock() else { return };`，
  中毒后**静默**丢弃此后全部 Mod 事件。
- `start_one` 的 `*slot.lock().unwrap() = Some(runtime)`（`:331`）在槽锁中毒时也会在 registry 锁内 panic。

**建议**：Mod 调用点统一 `catch_unwind` + 置 `Failed` + 日志；锁获取 poison-recover
（`session_scope.rs:109-111` 已有正确范例）；worker 对单 Mod panic 隔离而不是让线程死。

### M2 `TurnPrompt` 同步 flush 持 registry 锁最长 1s，且超时静默

`mod_event_sink`（`:633-642`）先 `registry.lock()` 再调 `dispatch_event_and_flush`
（`:573-586`，`recv_timeout(1000ms)`），**整段持锁**。后果：
① 每次开轮最多 1s 内 `GET /api/v1/mods`/`/state`/`/command`/enable/disable/config 全部阻塞；
② flush 等**所有** Running Mod，最慢的 Mod（memory 每轮两次全量读 JSONL）决定开轮延迟下界；
③ 超时的 `Err` 被丢弃、无日志，supervisor 继续读表 → 注入静默退回「下一轮生效」，
正是本轮要修的缺陷换了个触发路径。**建议**：flush 不持 registry 锁（先 clone `event_tx`），
超时打 warn 并做「本轮注入是否生效」可观察信号。

### M3 事件内核三处「实现比文档弱」

- **`tracked` ack 语义**：worker 对某 Mod `try_lock` 失败即 `continue`（`:657-659`），
  但循环结束**照样发 ack**（`:670-674`）→ `dispatch_event_and_flush` 返回「已处理」时，
  该轮 TurnPrompt 可能对 memory/persona **根本没被处理**（例如与 `GET /state` 撞锁）。
- **drop 计数恒 0**：`send_event`（`:589-599`）`try_send` 失败不 `fetch_add`；
  worker 的 `_dropped` 参数未用（`:655`）；`dropped_events()` 是 `#[allow(dead_code)]`
  且无调用方。契约声称「满则丢弃并计数」，实际指标永远是 0——又一个自检说谎。
- **订阅是装饰**：`HostRegistrar::subscribe`（`:707-712`）`let _ = topic;`，
  worker 对每个 Running Mod **无差别投递所有话题**。因此 external-input 会把
  `TurnPrompt`/`TurnEnded` 等也收进 `on_event`，并把每条 `TextDelta` **原文**
  info 进 tracing（`external-input/src/lib.rs:229-236`）——助手正文逐 token 入日志，
  与 AGENTS「不记录请求体/正文」的纪律相邻。

**建议**：ack 仅在真正处理过时发（或返回 processed 集合）；丢弃/跳过统一计数并暴露到
`state_json`/日志；按 topic 过滤投递；external-input 取消 TextDelta 订阅或只记 topic+长度。

### M4 日志等级丢失：所有 Mod 日志都变成 `info`

`mod_registry.rs:399`：`ModLogger::new(|_lvl, msg| tracing::info!(target: "mod", "{}", msg))`。
Mod 侧实际用了 `logger.warn` 20 处、`logger.error` 1 处（grep 统计），全部以 info 输出：
级别过滤/告警失效，Mod 错误进不了 error sink。**建议**：按 `log::Level` 映射到对应 `tracing` 宏。

### M5 生命周期缺口：重复 enable 泄漏 runtime、退出不关 Mod、失败仍报成功

- `enable`/`restart` 再次 `start_one` 时 `:331` 直接覆盖槽位，**旧 runtime 不 `shutdown`**。
- `disable` 忽略 `rt.shutdown()` 的 `Err`，状态照样置 Disabled（`:424-431`）。
- `ModRegistry::shutdown`（`:606-619`）在生产路径**从不被调用**（web_api/main 无调用方）：
  退出不 join worker、不跑 per-Mod `shutdown()`（persona 还原 / memory `strip_residue` 成死代码）。
- `enable` 无论 `start_one` 是否 `Failed` 都回 `Ok`（`:414-421`）→ 路由回 `{"ok":true}`
  且把 `enabled=true` 落盘；`config` 路径 restart 失败仍回 `{"ok":true,"restarted":true}`
  （`mods_routes.rs:268-274`），前端卡片照说「服务端已重启」。
- `mods_routes.rs:284-287`：`config` 成功但未启用时回 `{"ok":true,"enabled":false}`，
  前端 `_okMessage` 的措辞需要能区分这三态。

**建议**：覆盖前 `take()+shutdown()`；shutdown 失败标 `Failed`；退出显式
`registry.shutdown()`；enable/restart 响应带最终 `status`，前端按 status 措辞。

### M6 会话槽写入超限/超量是静默丢弃，Mod 仍报成功

`SessionScopeStore::set_owned`（`session_scope.rs:155-179`）在「单条贡献 > 64Ki 字符」或
「新会话且表满 200」时 `return`，签名是 `()`——写入方拿不到失败信号。
persona `sessions.rs:132-146` 的 `restore` 无条件 `restored += 1`（日志说「已恢复 N 个」）；
memory `lib.rs:453-456` 无条件 `injects += 1`。与 `session.rs:34-35` 头注
「超限时拒绝并返回，绝不静默淘汰」自相矛盾。**建议**：`set_owned -> Result/bool`，
Mod 据此回可读错误并回滚档案。

### M7 命令/状态面在 web_api 线程做文件 IO 与进程 spawn

`mods_routes.rs:210` 持 registry 锁调 `registry.command`；Mod command（persona 导入/清卡、
memory 清空/导入、voice 起 sidecar）在锁内做文件 IO / `Command::spawn`；
`voice-input/src/commands.rs:266,292` 在 HTTP 线程做 `fs::metadata`+`spawn`（同时持槽锁）；
`memory/src/lib.rs:601-623` 的 `state_json` 调 `record_count()` → `load(0)` **全量读 JSONL**
（上限 10k 条）。`factory.rs` 的 `state_json` 契约明写「不得发起网络/写盘/阻塞等待——
它在 web_api 线程上被调用，卡住就是卡住 HTTP」。**建议**：有副作用的命令与重 IO 移到
worker/后台任务，路由只提交 + 回执；`state_json` 只读内存计数。

### M8 voice 一键 sidecar：无 Running 互斥、无超时、stderr 全量入内存

`commands.rs:232` 的 `run_sidecar` 不检查 `SidecarState::Running`（连点即重复 spawn）；
`:301-315` 的 wait 线程把 stderr `read_to_string` **全量**读入后才在 `sidecar.rs:219`
截断到 600 字符；子进程无超时、无 kill。**建议**：Running 时拒绝（可读错误 + 回归）、
`take(N)` 限量流式读、超时 kill。

### M9 脱敏与日志纪律的两处缺口

- `redacted_config` 只按 schema **顶层** key 剥（`mods_routes.rs:483-499`）：spec 缺失、
  字段改名（GET 与运行时 schema 不一致）或嵌套 secret 都会把明文带出；
  静态 spec `validate()` 失败时在 `mod_registry.rs:200-205` **静默丢弃**该 spec。
  **建议**：无 spec 时按「key 名含 token/secret/password」一律剥，并对丢弃 spec 打日志。
- `apply_settings` 的日志把**整个 patch** 打进 info（`mod_registry.rs:364-371`）——
  含整张角色卡与记忆正文，违反 AGENTS「不记请求体/密钥不进日志」。
  **建议**：只记 key 名与长度（与 `.env`/dispatch 同一条纪律）。

### M10 `mods.json` 存在即覆盖内建缺省；写回丢弃未知 id

`cli_entry.rs:588-597` 的 `mods_manifest_for_web`：文件能解析就**原样返回**，不与
`default_mods_manifest` 合并。从旧版本升级的用户若 `mods.json` 里没有 `external-input`，
它会静默保持停用（缺省本应启用）。`persist_manifest`（`mod_registry.rs:245-282`）
只写**已注册 id**，因此文件里未知/已归档（wallpaper/pet-desktop/local-llm）的条目
会在下一次 enable/disable 时被抹掉。**建议**：缺失条目与缺省合并；写回保留未知 id。

### M11 memory 的资源与内容边界

- **无字节上限**：persona 卡有 16MiB + 64Ki 闸，memory 的单条正文（`store.rs:198-230`）
  与库文件都没有上限；`import`/弹幕可写超长文本，每轮全量 load + 分词 O(N·L)，
  且这条路径是**同步**投递（最多等 1s 的 TurnPrompt）→ 超大库直接进主链关键路径。
- **marker 字符串剥离可被内容污染**：`strategy.rs:245-264` 先 `find(BEGIN)` 再 `find(END)`，
  用户消息/角色卡若含 `MEMORY_MARKER_BEGIN|END` 常量，会永久残留或把 persona 全局卡片段剥掉。
- **self-hit 排除发生在 top-k 截断之后**（`lib.rs:278-284` + `strategy.rs:240`），
  自命中仍占一个 k 名额（doc §4 声称不占）。
- 「只读」`state_json` 的探测失败会 `errors += 1`（`lib.rs:310-323`）。

**建议**：单条字符上限 + 文件字节上限 + 只读尾部 N 行；marker 转义或拒绝；
self-hit 先按 index 过滤再截断；探测失败单列计数。

### M12 `Number` 字段无 default → 界面回填 0 并写回 0

`dev_tools_section.dart:662-667` 的 `_intValue` 在键缺失时 `return 0`，
`:684-698` 的保存会把 0 写回；`listen_port` 0 越界、memory `top_k` 被服务端钳到 min，
界面还会一直显示 0。**建议**：schema 给 `Number` 加可选 default，缺失时不写该键
（`ModSettingField::Number` 目前没有 default 字段，需要契约扩展）。

---

## 4. 低（P2，清理项）

| # | 位置 | 问题 | 建议 |
|---|---|---|---|
| L1 | `voice_routes.rs:254`、`commands.rs:132` | 400 `wake_phrase_required` 回显唤醒短语明文，与 `gate.rs:59`「不得回显」矛盾 | 文案只说未命中；短语只回 bool |
| L2 | `external_routes.rs:263-269` | `v2_ignored` 在长度/say 判定**之前**写计数，随后 400 也改可观察状态 | 移到成功分支或文档显式说明 |
| L3 | `external_routes.rs:369`、`voice_routes.rs:373` | 请求侧 token 不 `trim`、`==` 非常数时间比较 | 统一 trim；必要时恒定时间比较 |
| L4 | `mods_routes.rs:162` | `Box::leak(id.to_string()...)` 每个带 id 的请求永久泄漏一小段堆内存（含 404 路径） | 注册表改 `&str` 查询，或缓存 id 表 |
| L5 | `external-input/src/lib.rs:65` | `api_version` 硬编码 `1` 而非 `MOD_API_VERSION`（门禁本身在静态链接下只剩「自报数字相等」） | 用常量；文档写清门禁只防声明漂移 |
| L6 | `gate.rs:144-174` | 唤醒词匹配对每个起点重扫（O(n·m)），且在 1 MiB body 长度闸门之前 | 单次线性扫描；长度闸门提前 |
| L7 | `supervisor.rs:226-228` + `session_scope.rs:171-173` | 活动会话是进程级全局游标（多标签页互相改写）；超容量/超长静默拒绝 | 面板显式提示「已达上限，未绑定」 |
| L8 | memory `lib.rs:391-421` | `TurnPrompt` payload 是用户输入，故被动记忆**只记用户说的话**，不记助手回复（`TurnEnded` 只带 turn id） | 若产品要「对话记忆」，需新的轮末正文事件或明确单侧语义 |
| L9 | memory `store.rs:123-139`、persona `sessions.rs:296` | 固定 `.tmp` 名、无目录 fsync、无跨进程锁 | tmp 带 pid/随机后缀 + 目录 fsync + 实例锁 |
| L10 | `mod-system/src/session.rs:103` | 会话 id 允许冒号（Windows 文件名非法）与 `..` 打头（不穿越但产生隐藏文件） | 收紧字符集、禁点开头 |
| L11 | persona `lib.rs:604-615`、`:166` | `import_card` 先 `fs::read` 再判 16MiB（违背头注「读盘之前就拒」）；全局路径无 64Ki 级闸，16MiB 卡原样进 system_prompt | 统一走 `read_card_file`（先 metadata）+ 合成结果统一上限 |
| L12 | memory `commands.rs:127-179` vs `store.rs:155-175` | update/delete 两份实现，命令层绕开 store 方法 | 命令层复用 store 方法 |
| L13 | `settings_controller.dart:36-156` vs `dev_tools_section.dart:584-601` | Mod 表单草稿不在 SettingsController → 离开设置面板不触发「未保存先问」 | Mod 草稿纳入 dirty/confirmLeave |
| L14 | `director/src/lib.rs:41-71` | rustdoc 状态示例仍是 `channel:"none"`/`delivered:false` 且缺 `preset_id`，与同文件 `:74` 冲突 | 更新示例 |
| L15 | `mod-product-chain.md:51`、`session-scope-l1.md` §2.1/§2.2、能力表 | 文档落后：①「缺省只 enable `local-llm`」；②时序图把 `set_system_prompt_override` 画在 Mod 事件前（实现相反）；③仍写「谁后写谁覆盖」（实现已是多 owner 槽）；④能力表缺 `session_prompts` | 一次性同步 |
| L16 | `mod_registry.rs:1317`、persona `lib.rs:1004`（头注自称「< 1000」）、persona `tests.rs:849`/`tests_e2e.rs:930` | 违反 AGENTS「源码 ≤500（豁免 ≤1000 需头注理由）/ 测试文件 ≤800」；且 persona 的豁免注记已过期 | 拆分或补/更正豁免头注 |

---

## 5. 值得肯定的设计（收口时不要连坐删掉）

1. **会话多 owner 槽**（`mod-system/src/session.rs` + `session_scope.rs`）：命名 owner、
   固定合并顺序（匿名 → persona → memory → 其余字典序）、空串撤销、空条目剪枝；
   有「memory 停用不误伤 persona」「非法 id 不落表」「容量满不挤掉别人」回归。
   它修掉的是一类真实的静默数据丢失。
2. **`TurnPrompt` 同步回执把「同轮生效」从竞态变成契约**（`supervisor.rs:620` 先投递、
   `:633` 后决议），并有「去掉等待即红」的反向回归（`mod_registry.rs:1030-1073`）。
3. **`settings_spec` 纯数据 schema**：静态（未启用也有表单）、字段 key 去重校验、
   `secret` 脱敏、前端按 kind 渲染；禁止 Mod 注入 HTML/JS。同类里少见的干净做法。
4. **防回归护栏**：`mod_count_is_five` + `mod_factory_ids_match_expected` +
   `mod_factory_ids_are_unique`；`ARCHIVED-mods.md` 写清「封存理由 / 没连坐删什么 / 恢复条件」，
   使「不做的能力」也可审计。
5. **sidecar 纪律**：argv 数组化、`build_sidecar_argv_orders_args_and_never_shells_out`
   断言无 `sh -c`；spawn 立即返回、wait 放后台线程（避开 sidecar 回 POST 时服务端在等的死锁）。
6. **脱敏与可观察**：token 只以 `token_set` 布尔出门，有 `selftest_never_leaks_token` /
   `state_json_reports_token_set_without_echoing_plaintext`；external-input 计数键与 handler
   分支逐条对应，`reset` 用逐字段 `swap(0)` 返回清空前快照。
7. **原子写纪律统一**：`mods.json` / persona 档案 / memory JSONL 重写都 tmp+rename；
   persona 的「先落盘再改内存」避免半截状态；memory `clear` 用重写而非删文件。
8. **persona 的「没接管就不留痕」**：坏配置显式 Failed、导入卡写主链失败回滚、
   `register_settings` 失败回滚；PNG/tEXt/iTXt 解析全程先量后切、越界报错不 panic。
9. **测试密度与反向断言**：本次实测 297 条 Mod 测试全绿，大量用例是「去掉修复即红」
   的时序/隔离断言。

---

## 6. 同类实现调研（Mod/插件宿主维度）

> 功能级竞品矩阵见 `docs/research/competitive-analysis-report.md`；本节聚焦
> **「宿主怎么装、怎么管、怎么隔离 Mod」**，由联网子调研覆盖 11 类实现，事实点均附官方来源。

| 项目 | 加载/发现 | 隔离/权限 | 事件钩子 | 配置 schema→UI | 版本兼容 | 失败隔离 |
|---|---|---|---|---|---|---|
| **AIRI** | 独立进程 + WebSocket 注册（connect→authenticate→announce） | 进程边界 + `AUTH_TOKEN` | `custom:event`（带因果 id） | **无静态 schema**，UI 下发配置 | SDK 自标 WIP；无区间协商 | 断链即注销 |
| **Open-LLM-VTuber** | **无通用插件系统**：编译期工厂，扩展=改源码 | 同进程无沙箱 | 扩展实际走 **MCP** | pydantic + `conf.yaml`，无自动表单 | `conf_version` 只记配置结构 | 无插件级隔离 |
| **SillyTavern** | UI extension（目录+manifest）与 server plugin（`plugins/`，需显式开启） | 官方明示**不沙箱**、可触达整个文件系统，只装信任来源 | manifest hooks + `generate_interceptor` + eventSource | **无 schema**，扩展自写 HTML | `minimum_client_version`/`loading_order`/`dependencies` | 加载失败仅跳过 |
| **VTube Studio** | 不加载代码：外部进程 + 本地 WebSocket | **逐项权限弹窗**，可随时撤销 | `EventSubscription` | 配置在插件侧 | 每请求 `apiName`+`apiVersion` | `APIError` errorID |
| **OBS Studio** | 动态库/`.so` + Lua/Python | C ABI 同进程，**无隔离** | libobs signal + hotkey | `obs_properties_t` 回调生成控件（最接近 schema 驱动） | 编译期 API | `obs_module_load` 返回 false 即不加载 |
| **Koishi** | npm 包 + `koishi.yml`，支持热重载 | 同进程；DI `inject` 提供**依赖回滚重载** | `ctx.on` + authority 等级 | `schemastery Config` → **控制台自动表单** | npm semver + 服务依赖 | 必需服务缺失则不加载/回滚 |
| **NoneBot2** | Python 模块 + import hook / nb-cli | 同进程无沙箱 | Matcher 注册 + DI | `PluginMetadata.config` → pydantic，**无自动表单** | `supported_adapters` | 加载异常跳过 |
| **Home Assistant** | `custom_components/<domain>/manifest.json` 启动扫描 | 同进程无沙箱 | 事件总线 + `async_setup_entry` | `config_flow.py` → **前端自动渲染表单** | manifest version + dependencies | setup 失败进 Repairs/重试 |
| **MCP** | host 为每 server 建 1:1 client（stdio/HTTP） | **不读整段会话、不看别的 server**；能力协商 + 用户同意 | 请求/通知 + capability | tool 输入 JSON Schema，客户端渲染/审批 | `YYYY-MM-DD` + 每请求声明 + `UnsupportedProtocolVersionError` 列区间 | 单 server 崩溃不波及 host |
| **Cherry Studio / LobeChat** | HTTP 服务（manifest+gateway）/ MCP | 网络/进程边界 | function calling + 自定义渲染 | 无统一 schema | 插件索引 + MCP 协商 | HTTP 超时即失败 |
| **Extism / wasmtime**（备选） | `.wasm` 从 path/URL/data 加载 + sha256 | **WASM 沙箱 + 显式能力**（allowed_hosts/paths、内存页、响应上限） | host functions + WIT | 无 UI schema | manifest 版本 | trap 被 host 捕获为插件级失败 |

**与本项目的对照结论**

- 「静态编译 + 宿主渲染表单」不是孤例：OBS `obs_properties_t`、Koishi schemastery、
  HA `config_flow` 都是同一思路；`settings_spec` 与它们同构，方向正确。
- 「同进程插件不沙箱」是行业常态（SillyTavern 官方直接警告）。替代方案是**权限声明 + 用户同意**
  （VTS）或**进程边界**（AIRI/MCP）。本项目走静态 crate，治理应落在「许可 + 权限声明 + 文档边界」，
  而不是假装有沙箱。
- **失败隔离**普遍只做到「加载失败就跳过」；本项目「失败仅 disable」方向一致，
  但当前连「失败原因可见」都缺（S5）。
- **版本兼容**做得最好的是 MCP：声明区间 + 明确错误 + 「向后兼容的更新不递增版本」；
  本项目整数 `api_version` 不匹配即整个 Mod `Failed`，且不告诉宿主支持区间。

**可借鉴点（按性价比排序）**

1. **许可与来源写进 descriptor**（ST 内容库要求开源+跟进 release；LobeChat 已从 MIT 改为
   Community License）：`ModDescriptor` 增 `license`/`source_url`/`redistributable` 并在
   `GET /api/v1/mods` 暴露。
2. **失败要「可读 + 可熔断」**：`last_error` 进 API（S5）；连续 N 次 `on_event` 超时/错误
   自动 disable 并写明 `disabled_reason`，而不是静默丢弃（M1/M3）。
3. **`api_version` 门禁升级为能力协商**：失败响应列出宿主支持区间（MCP 模式），
   减少「破坏性升级需手改一次」。
4. **secret 写回审计只记键名**（M9）：`PUT …/config` 日志不出现值；
   `settings_spec.secret` 字段在响应里固定回占位符（顺带修 S3 的前端契约）。
5. **把「HTTP/子进程 sidecar 型 Mod」立为一等形态**：OLV 没有插件系统、扩展全押 MCP；
   LobeChat/Cherry 也走 HTTP/MCP。external/voice 已是 sidecar 型，建议在契约里与静态 crate 型并列，
   复用同一 `settings_spec` 与启停门禁。
6. **静态权限声明 + 首次启用同意**：`permissions`（`net.listen`/`fs.read`…）写进 `ModFactory`，
   前端首次启用弹确认；token 继续脱敏。
7. **settings 变更要有显式状态迁移**：Koishi 依赖变化时**回滚重载**；本项目
   `apply_settings`/`enable` 已部分做到，但失败回滚与最终状态未回传前端（M5）。
8. **可查询的 announce 面**：`GET /api/v1/mods` 增 `capabilities`/`protocol_version`，
   不动静态注册表——AIRI announce 与 MCP `server/discover` 都是这个模式。

**来源（官方文档优先）**：
[AIRI Plugin System](https://mintlify.wiki/moeru-ai/airi/development/plugins) ·
[Open-LLM-VTuber CLAUDE.md](https://raw.githubusercontent.com/Open-LLM-VTuber/Open-LLM-VTuber/main/CLAUDE.md) ·
[Open-LLM-VTuber 开发指南](https://docs.llmvtuber.com/en/docs/development-guide/overview/) ·
[SillyTavern Server Plugins](https://docs.sillytavern.app/for-contributors/server-plugins/) ·
[SillyTavern Writing Extensions](https://docs.sillytavern.app/for-contributors/writing-extensions/) ·
[VTube Studio Plugin API](https://github.com/DenchiSoft/VTubeStudio/blob/master/README.md) ·
[VTube Studio Permissions](https://github.com/DenchiSoft/VTubeStudio/blob/master/Permissions/README.md) ·
[OBS Module API](https://docs.obsproject.com/reference-modules) ·
[Koishi 认识插件](https://koishi.chat/zh-CN/guide/plugin/) ·
[Koishi 服务与依赖](https://koishi.chat/guide/plugin/service.md) ·
[Koishi 配置构型](https://koishi.chat/guide/plugin/schema.md) ·
[NoneBot 创建插件](https://raw.githubusercontent.com/nonebot/nonebot2/master/website/docs/tutorial/create-plugin.md) ·
[Home Assistant Integration manifest](https://developers.home-assistant.io/docs/creating_integration_manifest/) ·
[MCP Architecture](https://modelcontextprotocol.io/specification/2025-06-18/architecture) ·
[MCP Versioning](https://modelcontextprotocol.io/docs/2026-07-28/learn/versioning) ·
[MCP Security Best Practices](https://modelcontextprotocol.io/docs/2025-11-25/tutorials/security/security_best_practices) ·
[LobeChat Chat Plugin SDK](https://github.com/lobehub/chat-plugin-sdk) ·
[Cherry Studio MCP](https://docs.cherryai.com.cn/docs/en-us/advanced-basics/extensions/mcp) ·
[Extism Manifest](https://extism.org/docs/concepts/manifest/)

---

## 7. 建议的收口顺序

| 优先级 | 项 | 为什么先做 |
|---|---|---|
| **P0-1** | S1 token 走 `secrets::lookup` | 安全：配了 token 却静默不鉴权 |
| **P0-2** | S2 动态装配注入 HostChannels + 空 `say` 回 false | 否则「先控制平面后配 LLM」路径 Mod 全废且谎报成功 |
| **P0-3** | S3 配置保存不抹 secret（服务端按 key 合并） | 安全降级 + 静默丢配置 |
| **P0-4** | S4 memory 会话注入保留全局 base | 启用 memory 会顶掉全局人设 |
| **P0-5** | S5 失败日志 + `last_error` 进 API | 排障；显式违反日志纪律 |
| **P1-1** | M1 panic 隔离 + poison 恢复 | 一个 Mod 崩 → 不拖垮全部 Mod 与路由 |
| **P1-2** | M2 flush 不持 registry 锁 + 超时告警；M3 ack/计数/订阅 | 1s 全局阻塞与「静默退回下一轮」 |
| **P1-3** | M4 日志等级；M9 脱敏与 patch 日志 | 低成本，直接改善可排障性 |
| **P1-4** | M5 生命周期；M6 会话槽返回结果 | 状态诚实性 |
| **P1-5** | M7 命令/状态面移出 HTTP 线程；M8 sidecar 互斥/超时/限量 | 稳定性（改动量较大） |
| **P2-1** | S6 director 通道裁决 + 文档同步 | 治理边界必须先说清 |
| **P2-2** | M10–M12 + L1–L16 | 清理与一致性 |

---

## 8. 本次审查的盲区与未覆盖项

- **未做活服务肉眼验收**（Windows 侧浏览器）与 `ignite.sh --check`；未跑 Flutter
  `analyze`/`test`（作者侧记录 944 passed，本次未复跑）。
- **未跑全 workspace `cargo test --all-targets`**（只重跑了 Mod 相关 6 个 crate 的 297 条），
  桌面壳/运行时/前端回归计数以作者收束报告为准。
- 被审查 worktree 处于**未提交状态**（core-next 三件套）；本报告行号基于该工作区快照，
  若后续提交/重排可能漂移。
- `docs/research/` 已有功能级竞品矩阵；本报告只补「Mod/插件宿主」维度。
- 子审查（persona+memory / external+voice+director / 宿主集成 / 同类调研）的部分结论
  本审查已抽查复核（S1–S6、M1–M3、M5、M8、M9、M11 的关键行号）；未逐条复跑其全部主张。
