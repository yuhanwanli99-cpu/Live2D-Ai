# Wave 2 并行协议（mod/memory-v0 / mod/director-rfc / mod/voice-sidecar-v1 / mod/wallpaper-wire / mod/pet-desktop-v1）

> 上游：[`PARALLEL-PROTOCOL-2026-09-14.md`](PARALLEL-PROTOCOL-2026-09-14.md) §4 的 Wave 2 条目，
> 由本轮任务书扩成五条轨。基线：`mod/integrate-0.2.0-rc.2` @ **`91c670aa`**（= `0.2.0-rc.2`）。
> 集成分支：**`mod/wave2`**（指挥台 worktree `/home/skystar/Live2D-Ai-wave2`）。
> 版本线：`0.2.0-rc.2` → **`0.2.0-rc.3`**（收束由主 agent 做，**任何轨不得改版本号**）。

## 0. 一句话

Wave 1 把骨架挂进了 `AVAILABLE_MOD_FACTORIES`；**Wave 2 要让能力可演示**：
语音 sidecar 真的能把一段音频变成主链开口、壁纸决策真的落到 `DisplayPrefs`、
记忆真的写进去检索出来、导演有契约、桌宠的配置/事件态真的能被 API 读到。
**再交一个「只有 `settings_spec` + 空 tick」的半成品不算过关。**

## 1. 基座提交（主 agent 独占；各轨**禁止**再碰这些文件）

`mod/wave2` 上先落一个「Wave 2 基座」提交，解决两条轨**共同缺**的公共面，
使各轨之间不再争抢共享文件：

| 基座改动 | 文件 | 为什么必须有 |
| --- | --- | --- |
| `ModEventTopic::TurnPrompt`（payload = 本轮输入正文） | `live2d-ai-mod-system/src/topics.rs` | `TurnStarted` 的 payload 只是 turn id，记忆/导演类 Mod 拿不到「本轮说了什么」。时序：该事件发出时本轮请求体马上构建，因此 Mod 在此做的 `apply_settings` **只对下一轮生效**——这正是「检索 top-k → 注入下一轮」的语义 |
| `ModRuntime::state_json(&mut self) -> Option<Value>` | `live2d-ai-mod-system/src/factory.rs` | Mod 内部状态原本只能从日志看（前端拿不到）。契约：**必须脱敏**、**只读**、**不得**在里面做网络/写盘/阻塞（它在 web_api 线程上被调用） |
| `ModRegistry::contains` / `runtime_state(id)`（`try_lock`） | `live2d-ai-desktop/src/mod_registry.rs` | 把 `state_json` 接到注册表；`try_lock` 失败 = 503，**绝不让 HTTP 线程等 Mod worker** |
| `GET /api/v1/mods/{id}/state` | `live2d-ai-desktop/src/web_api/mods_routes.rs` | 运行态的**唯一**读取面。404（不在注册表）/ 503 `state_unavailable`（在册但未启用、没实现 `state_json`、或 worker 正持锁）**刻意分开** |
| `TurnPrompt` 发点 | `live2d-ai-desktop/src/supervisor.rs` | 提交点紧随 `TurnStarted` 发出 |

**红线**：任何轨若发现自己需要改 `topics.rs` / `factory.rs` / `mod_registry.rs` /
`mods_routes.rs` / `supervisor.rs`，**停下来找主 agent**——说明基座不够，由主 agent 补，
不要自己动（那会与别的轨打架）。

其余只允许各轨动的东西：

- 自己的 crate `crates/live2d-ai-mod-<id>/**`；
- 自己的新 web_api 文件（**只允许 A 轨新增 `web_api/voice_routes.rs`**）+ `web_api/mod.rs`
  里**一处** `pub mod` 与**一段**前置路由钩子（紧邻现有 `external_routes` 钩子）；
- 自己的 Flutter 文件（**只允许 B 轨**动 `shell/flutter/**`）；
- 自己的文档：`docs/<topic>.md` + `docs/plans/parallel-mods/REGISTER-<id>.md` +
  `docs/architecture/<...>.md` 的**自己那一节**；
- 自己的 sidecar 示例目录 `docs/examples/voice-sidecar/**`（A 轨）。

**独占清单（谁都不许动，主 agent 收束时一次改到位）**：
`main.rs::AVAILABLE_MOD_FACTORIES` / `mod_count_*` / `mod_factory_ids_match_expected` /
`cli_entry::default_mods_manifest` / 根 `Cargo.toml` 的 `version` / `pubspec.yaml` /
`AGENTS.md` 的版本与 Mod 计数 / `docs/releases/**`。

## 2. 环境（**先读这一节再开工**）

本机磁盘只剩 ~14 GB、内存 7 GB，因此**所有 worktree 共用一个 cargo target**：

```bash
export CARGO_TARGET_DIR=/home/skystar/Live2D-Ai-integrate/target
export PATH="$HOME/flutter/bin:$PATH"   # 只有 B 轨需要
```

- **不要**在 worktree 里另起 `target/`（会各自冷编译 + 撑爆磁盘）；
- 共享 target 的代价是 cargo 全局构建锁：并行构建会排队，**这是预期**，不要绕过；
- 门禁命令（`AGENTS.md` 一张表的逐条对齐）：
  `cargo test --workspace --all-targets` / `cargo test --doc --workspace` /
  `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings` /
  `cargo run -p xtask -- rust-ratio`；B 轨另加 `flutter analyze && flutter test`。

## 3. 五条轨（各自 worktree / 分支）

| 轨 | 分支 | worktree | 交付 | REGISTER |
| --- | --- | --- | --- | --- |
| A 语音 sidecar | `mod/voice-sidecar-v1` | `/home/skystar/Live2D-Ai-w2-voice` | `POST /api/v1/voice/transcript` + Win/本机 sidecar 脚本 + 契约 | `REGISTER-voice-sidecar-v1.md` |
| B 壁纸接线 | `mod/wallpaper-wire` | `/home/skystar/Live2D-Ai-w2-wall` | 决策 → `DisplayPrefs` / `stage-bg` 真落点 | `REGISTER-wallpaper-wire.md` |
| C 记忆 | `mod/memory-v0` | `/home/skystar/Live2D-Ai-w2-memory` | 新 crate `live2d-ai-mod-memory` + 真读写 | `REGISTER-memory-v0.md` |
| D 导演 RFC | `mod/director-rfc` | `/home/skystar/Live2D-Ai-w2-director` | `docs/architecture/director-rfc.md`（契约先行） | 不需要（不注册） |
| E 桌宠 | `mod/pet-desktop-v1` | `/home/skystar/Live2D-Ai-w2-pet` | `pet-desktop` 配置/事件态可测面 + 边界 | `REGISTER-pet-desktop-v1.md` |

### A. voice-sidecar-v1（演示闭环，优先）

**端点二选一已钉死：选 B —— 新增专用 loopback 端点**
`POST /api/v1/voice/transcript`（不是复用 `/api/v1/external/chat`）。
理由写进契约文档：voice 侧有自己的 `token` / `locale` / `backend` 语义，且
**要证明 `voice-input` crate 的 `clean_transcript` 真的在主链路上跑**——
复用 external 只会让 voice Mod 继续当摆设。（现有
`docs/examples/voice-sidecar/README.md` §3 已把这个二选一列为集成 PR 的决定，本轮就是它。）

请求 / 响应契约（**handler 与 sidecar 脚本必须逐条对齐**）：

| 项 | 约定 |
| --- | --- |
| 方法 / 路径 | `POST /api/v1/voice/transcript`，其它方法 `405` |
| Content-Type | 必须 `application/json` 前缀，否则 `415` |
| Origin | 同源 loopback（复用 `security::is_allowed_origin`）；无 Origin 走 `allow_no_origin`；否则 `403 origin_denied` |
| body | `{"text": "<转写>", "token"?: "<token>"}`；`text` 缺失/非字符串/清洗后为空 → `400`（`invalid_payload` / `empty_transcript` 分开） |
| token | 优先级 **env `VOICE_INPUT_TOKEN` → Mod config `token` → 不鉴权**；请求侧 `body.token` 或 `Authorization: Bearer`；不匹配 `401 unauthorized` |
| 启停门禁 | `voice-input` 在注册表且 `enabled=false` → `403 mod_disabled`；**不在注册表**（测试上下文）→ 不设门禁（与 `external_routes` 同口径） |
| 成功 | `200 {"ok":true,"text":"<清洗后文本>"}` |
| 主链忙碌 | `200 {"ok":false,"error":{"code":"busy"}}`（**不要** 5xx：发送方自行退避，不要重试到刷屏） |
| 无 supervisor | `503 supervisor_unavailable` |
| 长度 | 清洗后 ≤2000 字符，超出 `400 text_too_long` |

- 落点：`crate::web_api::voice_routes`（新文件，形态照抄 `external_routes.rs`），
  清洗**复用** `live2d_ai_mod_voice_input::clean_transcript`（**不许**在 handler 里重写一份）；
- `web_api/mod.rs`：加 `pub mod voice_routes;` + 紧邻 `external_routes` 钩子的一段前置路由；
- `voice_routes.rs` 内 ≥10 条回归（门禁三态 / Bearer / body token / 空转写 / 超长 / 门禁禁用 / 方法 / CT / Origin / busy）；
- **sidecar 脚本** `docs/examples/voice-sidecar/voice_sidecar.py`（Python 3，标准库即可）：
  - 输入「一段语音 / 假音频文件」：`--audio <file>`（wav 或任意文件）；
  - 转写后端可插拔：`--transcriber fake`（缺省；读同名 `.txt`，没有就用 `--fake-text`/固定串）
    或 `--transcriber cmd:"<命令>"`（把音频路径喂给用户自己的 ASR CLI，读 stdout）；
    **不把任何 ASR SDK 静态链进 Rust，也不在 Rust 侧开 socket**；
  - `--dry-run`：只打印将要 POST 的 JSON + URL，**不发请求**；
  - `--selftest`：离线自检（清洗语义、payload 形状、错误码映射、URL 归一化），退出码 0/非 0；
  - 退出码：`0` 成功 / `2` 参数或依赖错 / `3` 转写失败 / `4` HTTP 非 2xx / `5` 服务端 `ok:false`；
  - README 重写：依赖、环境变量、**一条可复制命令**、dry-run、失败码表、与
    `external/chat` 的差异说明（为什么选专用端点）。
- `fixtures/`：至少一个**假音频文件**（几 KB 的 wav 头 + 静音）+ 对应 `.txt`，让命令开箱即跑。

**禁止**：云 ASR SDK 静态链进主 binary；B 站协议进 Rust；在 Mod 里开 socket / 读进程环境。

### B. wallpaper-wire（结束占位）

**接线方式已钉死：Flutter 偏好写回**（Mod 决策 → 既有 `DisplayPrefs` → 既有 `sendStageBg`）。

1. **Mod 侧**（`live2d-ai-mod-wallpaper`）：`WallpaperRuntime` 实现
   `state_json()`——用 `Instant` 算增量毫秒喂给**既有纯策略** `tick(delta_ms)`，返回：

   ```json
   {"mode":"interval","active":true,"playlist_len":3,
    "decision":{"kind":"advance","index":1},
    "prefs_patch":{"stage_index":1},
    "reason":"advance"}
   ```

   `kind` ∈ `none` / `sync_stage` / `advance`；`prefs_patch` =
   `{"sync_shell_stage_bg":true}` / `{"stage_index":N}` / `null`。
   **占位必须消失**：`apply_decision` 的「warn + 返回 false」改成**真投影**——
   纯函数 `WallpaperDecision::to_prefs_patch()`（可单测），不再对决策说假话。
   策略时钟**只**在 `state_json` 里推进（不在 Mod 里起线程，不订阅事件）。
2. **播放列表来源已钉死：Flutter 自己的偏好列表**（不新增图库 / 不新增文件服务 /
   不新增 wasm 路径）。`DisplayPrefs` 增 `stagePlaylist`（`List<String>` dataURL，
   缺省空，与 `stageImage` 同一条 localStorage 记录、同一份长度预算），
   并在既有「舞台背景」区加最小操作（「加入轮播」把当前图 append、「清空轮播」）。
3. **闭环**：
   - 列表长度变化时，Flutter 用**既有** `POST /api/v1/mods/wallpaper/config`
     写回 `{"config":{"playlist_len":N}}`（Mod config 增加 `playlist_len` 字段，纯数字）；
   - Mod 启用且 `mode != off` 时，Flutter 轮询**基座已有的**
     `GET /api/v1/mods/wallpaper/state`（间隔 ≥5 s，停用即停轮询）；
   - `prefs_patch.sync_shell_stage_bg=true` → `prefs.copyWith(syncShellStageBg: true)`；
   - `prefs_patch.stage_index=k` → `stageImage = stagePlaylist[k % len]` → 经既有
     `Live2DStage.sendStageBg` 下发；列表为空 → **明确跳过**（`skipped: playlist_empty`），
     不是「假装成功」。
   - 落点逻辑写成**纯函数**（`display_prefs.dart` 里 `applyWallpaperPatch`），VM 可测。
4. 测试：Mod 侧 `state_json` / `to_prefs_patch` 单测；Flutter 侧纯逻辑测试
   （三种 patch + 空列表 + 索引取模 + 停用不轮询）。文档：`wallpaper-mod-v0.md` §5 改成
   **已接线**（写清落点、节拍、列表来源、空缺情形），§7 的「落点未接线」删掉。

**禁止**：重做 framebuffer / 改 `stage_bg` 渲染算法 / 大轮播图库 / 在线拉图 / 新增 wasm 路径。

### C. memory-v0（新 crate + 真读写）

新 crate `crates/live2d-ai-mod-memory`（`live2d-ai-mod-template` 起手），
**注册进 `AVAILABLE_MOD_FACTORIES` 但缺省停用**（`default_mods_manifest` **不收录**）。

- 纯函数检索（**不许**外挂向量云、不许引入 embedding 依赖）：本地 JSONL 存储
  （`store_path` 配置，缺省在配置文件同目录 `memory.jsonl`）+ 词元重叠打分
  （中文按字 bigram + ASCII 词，去停用词，`score` 归一）+ top-k。
- 钩子：
  - 订阅 `TurnPrompt`（基座新增）→ 把输入正文 `remember()` 进库（带上时间与 turn 序号）；
  - 在同一事件里按本轮正文检索 top-k（k 由 `top_k` 配置，缺省 3）→
    经 **`ModServices.apply_settings`** 注入 `persona.system_prompt`：
    `<base>\n\n<marker>\n- ...\n</marker>`（marker 固定，便于**幂等**剥离）；
  - 时序写在文档里：**只对下一轮生效**（基座已钉死的语义），不要写「当轮生效」的假承诺。
- **边界（必须写清）**：`persona` Mod 与 memory 都可能写 `system_prompt`；
  规则 = memory 每次注入前先用 marker 剥掉旧块再从 `ModServices.settings` 读到的
  base 重拼，**last-writer-wins**；两者同时启用时后写者覆盖前者的记忆块——
  这是已知取舍，写进 crate 头注 + REGISTER，不要假装有仲裁。
- 单测 **≥15**（纯函数为主：token 化 / 打分 / top-k 平局 / k 钳位 / JSONL 往返 /
  坏行容忍 / marker 剥离幂等 / 空输入 / 注入 patch 形状 / 缺省停用 / 忙碌 / 存储路径缺失）；
- `docs/architecture/memory-mod-v0.md` + `REGISTER-memory-v0.md`。

**禁止**：向量商业云写死；接管 LLM 客户端；改主链人设字段**结构**（只能写既有的
`persona.system_prompt` / `max_history_pairs`）。

### D. director-rfc（契约先行）

只交**文档**：`docs/architecture/director-rfc.md`，必须含：
输入（LLM 文本 → 情绪/意图）、输出（TTS 参数选择 / 预置动作**槽位**）、
与休眠 `action_tx` 的关系、晋升核心的**门槛**（可验证的准入条件）、
状态机草图、与 persona / memory 的边界、非目标清单。

**已钉死**：本轮**不新建 crate、不注册 `AVAILABLE_MOD_FACTORIES`**（RFC 里写明
「默认不注册」这一选择及其理由；另一选项「注册但 enable 即 Failed」被否决，
因为空工厂注册进静态表只会让计数断言变得没有意义）。

**禁止**：复活 Action 真通道（`action_tx` 保持休眠）；实现动作库。

### E. pet-desktop-v1（从骨架推进一格）

**选项已钉死：把配置 / 事件状态经 API 暴露到可测面**（不真开窗口——那属于
`docs/architecture/core-chain-baseline.md` §3.6 的「休眠原生壳」，唤醒要先论证
「谁来维护第二个 UI 壳」）。

- `PetDesktopRuntime`：修掉「启动后才注册 settings、`settings_spec` 静态缺失」的不一致
  （静态 schema 与 `start` 注册同一份，单测钉住不分叉）；
- 实现 `state_json()`：`{"always_on_top":…,"click_through":…,"opacity":…,
  "voice_active":…,"window":{"opened":false,"reason":"native_shell_dormant"}}`
  （配置从 `ModServices.settings`/config 读，事件态来自 `VoiceStarted`/`VoiceEnded`）；
- 闭环：既有 `POST /api/v1/mods/pet-desktop/config` 改配置 → 既有
  `GET /api/v1/mods/pet-desktop/state` 立刻反映（脚本级自检或回归证明）；
- crate 头注更新范围声明（**明确窗口未开**及为什么、与 egui 原生壳的边界）；
- `REGISTER-pet-desktop-v1.md`（**预计无需**改 FACTORIES——它已在表里）。

**禁止**：重写 desktop 壳；引入第二套渲染引擎（winit/egui/wgpu）。

## 4. 汇报格式（每个轨收工时必须给全）

```
轨：<A|B|C|D|E> / 分支：<branch>
worktree tip SHA：<sha>（分支头，未 rebase 别人的东西）
测例：<cargo 新增/总数> ; <flutter 新增/总数，若适用>
与 REGISTER 的偏差：<逐条，或「无」>
是否改过 FACTORIES / mod_count_* / 缺省 manifest / 版本号：<否 | 是 + 为什么（=打回）>
未决 / 已知缺口：<逐条>
门禁实跑结果：<命令 + 结果，含 fmt/clippy/rust-ratio>
```

## 5. 收束（主 agent）

1. 各轨 merge 进 `mod/wave2`；`FACTORIES` 5 → **6**（+ `memory`，缺省停用），
   `mod_count_is_five` → `mod_count_is_six`，id 断言同步；
2. 全量门禁绿 + `docs/releases/v0.2.0-rc.3.md`；
3. 交付摘要（每轨 tip / 演示步骤 / 未决 / 是否 push，默认**否**）。
