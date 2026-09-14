# Wave 3 收束报告 —— 七个已注册 Mod 的日常闭环（2026-09-14）

> **这不是 release 文件**。Wave 3 不发布、不 bump 版本号、不打 tag、**默认不 push**。
> 版本保持 `0.2.0-rc.3`（`Cargo.toml` / `pubspec.yaml` / README 一行未动），
> 也**没有**写 `docs/releases/v0.2.0-rc.4.md`。
>
> - **真源**：`/home/skystar/Live2D-Ai-wave2` @ `1e789cb6`（本地 `0.2.0-rc.3` 候选，未 push）
> - **集成分支**：`mod/wave3`（指挥台 worktree `/home/skystar/Live2D-Ai-wave3`）
> - **基座**：`118bd435`
> - **范围真源**：任务书 + [`PARALLEL-WAVE3-2026-09-14.md`](PARALLEL-WAVE3-2026-09-14.md)
>   （Wave 3 计划文档随基座一起落盘——与 Wave 2 同款做法）
> - **口径**：主链皮肤（LLM/TTS/口型/Live2D、壳/舞台背景）与
>   `crates/l2d-wasm-demo/`（`stage_bg.rs` / framebuffer / `LoadOp`）**一行未改**。

---

## 1. 一句话

Wave 2 把能力做成「能演示」；**Wave 3 把七个已注册 Mod 补到可日常用的闭环**，
并清掉 `v0.2.0-rc.3` §8 里挡日常的洞：语音的 `backend`/`locale` 不再是死配置、
壁纸列表可维护且 `playlist_len` 不再自相矛盾、记忆有质量基线 + **物理淘汰** + 四计数、
桌宠给 Flutter 一个可读状态面、persona 坏卡路径是可回归的值断言 E2E、
external-input 有接受/拒绝/忙碌/v2 计数与 sidecar 节流，**导演从 RFC 推进到可启用的最小骨架**
（零投递，缺省停用，收束时注册为第 7 个）。

## 2. 基座（主 agent 独占，先于七轨落盘）

| 基座改动 | 文件 | 为什么必须有 |
| --- | --- | --- |
| `ModEventTopic::TurnEnded`（payload = turn id；发点在 `run_one_turn` 返回之后，**成功/失败都发**） | `live2d-ai-mod-system/src/topics.rs`、`live2d-ai-desktop/src/supervisor.rs` | §8.8 点名的基座需求：Mod 只有「轮开始」没有「轮收口」。导演要在轮末结项决策日志、记忆要把本轮命中写进计数 |

**明确没有做新基座**：`apply_settings` 一等化、`state_json` + `GET /mods/{id}/state`、
`mods.json` 原子写回、Flutter 偏好写回（`DisplayPrefs` / `sendStageBg`）Wave 2 已经建好，
各轨直接用既有通道。**唯一**的主 agent 收束改动是注册 director（见 §4）。

## 3. 七条轨（tip / 闭环 / 证据 / 未决）

| 轨 | 分支 | tip（未改历史） | merge | 闭环 | 可跑证据 | §8 |
| --- | --- | --- | --- | --- | --- | --- |
| **A 语音** | `mod/w3-voice` | `ce049e16` | `3d58f5eb` | **是** | Mod 13→**21** 测；handler 25→**27** 测；sidecar `--selftest` 49→**70** 项；`backend` 走明确分支（`VoiceBackend::opens_network()` 恒 `false`，Rust 结构性不开 socket）；`locale` 真影响归一化（新 `normalize.rs`：CJK 词间空格 / 中英边界，幂等）；sidecar 六类失败码 + 逐条退避 | §8.6 |
| **B 壁纸** | `mod/w3-wall` | `6bb931a8` | `135a215e` | **是** | Mod 34→**37** 测（interval `advance 0→none→1→2→0` 取模回绕轨迹 / follow_stage 重同步轨迹）；Flutter `wallpaper_wiring_test` 24→**34**；列表增删/排序（`removeStagePlaylistAt` / `moveStagePlaylist` + 每行按钮）；`playlist_len` **退出可编辑 schema**（keys = `mode, interval_secs`），真源 = Flutter `stagePlaylist.length` | §8.5 |
| **C 记忆** | `mod/w3-memory` | `2561c8a0` | `f156e8f1` | **是** | 44→**61** 测：固定语料 fixtures（中文命中 / 词面不重叠不命中 / 同义改写边界）；`append_capped` **物理淘汰**（`.tmp`+rename 原子重写，裸读文件只剩 N 行）；`state_json` 含 `writes/hits/injects/errors`（+ `evicted/last_hits`）；与 persona last-writer-wins 两向对称回归 | §8.2/8.3/8.4 |
| **D 桌宠** | `mod/w3-pet` | `58fec833` | `de259260` | **是（软闭环）** | Rust 15→**17** 测（state 字段集 + `reason` 与 Dart 常量逐字一致）；Flutter **+9**（`pet_desktop_state_test.dart`）：`GET /mods/{id}/state`（200/404/503 三分）→ 卡片示 `always_on_top`/`click_through`/`opacity`/`voice_active`/`window`；配置保存后重取、字段跟着变 | §8.7 |
| **E 人设** | `mod/w3-persona` | `96dce03a` | `7e1c7416` | **是** | 32→**39** 测；新增 `tests_e2e.rs`（318 行，**7** 条）：坏卡 E2E ×4（坏 JSON / `card_json` 是数组 / PNG 无 `chara` / 非法 UTF-8）——断言打在**主链 `system_prompt` 的值**上（Failed 一字未动 / 无基线快照 → 修好逐字接管 → disable 回基线）；与 memory 共存 ×3（直接调 memory crate 的真实纯函数，防两侧契约漂移） | §8.4 |
| **F 外部输入** | `mod/w3-external` | `f33e3d97` | `050e4a21` | **是** | Mod **18** 测（`counters.rs` 计数契约）；handler **+3**（「3 成功 / 1 坏 token / 1 忙」逐键 `accepts+3/rejects+1/busy+1`，且与 `state_json` 同路径对齐）；sidecar `--selftest` **16** 项（无 blivedm/aiohttp/网络也能跑）；节流 `--min-interval-ms`（缺省 1000，`0`=关） | 任务书 §6 |
| **G 导演** | `mod/w3-director` | `608b25b6` | `b04f8945` | **是（骨架）** | 新 crate **28** 测；订阅 `TurnPrompt` + `TurnEnded`；纯函数 `derive(text, lexicon) → {emotion,intent,suggested_tts:{speed,pitch}}`；`state_json` 有界决策账本 + `delivered:false`/`channel:"none"`；间谍断言 `action_calls==0` **且** `apply_calls==0`（**零投递**） | §8.8 |

**各轨未决（原样登记，不假装已解决）**：

- **A**：未起活服务 curl（证据 = 脚本级 + handler/Mod 单测）；`locale` 只做空格档（不做标点全/半角）；sidecar 不自动重试（只打印退避）；`mock`/`sidecar` 的 Rust 本地落点相同（推模式的固有边界）。
- **B**：未做点火肉眼验收；列表无去重；超大图不裁剪；不做缩略图 / 拖拽排序 / 在线图库（非目标）。
- **C**：物理淘汰**无备份/导出**；每轮两次全量读盘（默认 200 条内可接受）；质量基线是回归基线而非质量分数；`max_records` 同时是上限与窗口（改默认值会删数据）；无记忆管理 UI。
- **D**：`window.opened` 恒 `false` 是**裁决**（软闭环）；`shell_settings.dart` 的显式 `onLoadState` 接线未做（自建同源 loader 已可用，写成可选）；`voice_active` 真值需主链开口时才为 true。
- **E**：E2E 是 `PersonaRuntime` 级，不是 host `mod_registry::start_one` 级；共存测试用 memory 纯函数而非 `on_event` 全链路；新增 persona→memory 的 `[dev-dependencies]`（刻意做跨 crate 契约检查）。
- **F**：计数是进程级 `AtomicU64`、重启归零（运行观察值，非审计账本）；未起活服务 curl。**上游查证**：blivedm 已在 PR #86（2026-08-11，v1.1.7）支持 `SEND_GIFT_V2` 并复用同一 `_on_gift` 回调——`v2_ignored` 是对**旧版**的兜底观察面；另修掉旧 sidecar「未知 cmd 走 `_on_unknown_cmd`」的错误假设（blivedm 实际只内部 log 就 return），并把不可满足的 `blivedm>=2.3,<3` 改成 git 安装。
- **G**：`suggested_tts` 无消费者（骨架的本意）；RFC §3.2 的槽位未实现；只订阅用户侧 `TurnPrompt`，回复侧情绪证据不完整；情绪词表是启发式（确定性、可单测）。

## 4. 主 agent 收束（仅主 agent 做过的改动）

1. **注册 director（6 → 7）**：`crates/live2d-ai-desktop/Cargo.toml` 加 path 依赖；
   `main.rs::AVAILABLE_MOD_FACTORIES` 追加 `&live2d_ai_mod_director::FACTORY`；
   `mod_count_is_six` → **`mod_count_is_seven`**（断言 7）；`mod_factory_ids_match_expected`
   的 `expected` 加 `"director"`；`cli_entry` 的「director 不进缺省 manifest」断言保留（注释更新）。
2. **缺省 manifest 不动**：仍只启用 `external-input`；director / memory / persona /
   pet-desktop / voice-input / wallpaper **全部只注册、缺省停用**。
3. **存活文档同步 `mod_count_is_six` → `mod_count_is_seven`**：
   `mod-product-chain.md` §5（标题「7 个注册」+ director 行）、`directory.md`、
   `core-chain-baseline.md` §3.2 台账、`director-rfc.md`、`wallpaper-mod-v0.md`、
   `external-input.md`、`README.zh-CN.md`、`AGENTS.md`、`local-llm` / `voice-input` crate 头注。
   历史文档（`docs/releases/**`、旧 HANDOFF / PLAN）**保持历史数字不动**。
4. **修 `docs/README.md` 断链**：`../PLAN.md` → `legacy/PLAN.md`（§8.9），
   并把 director 条目从「未注册」改成「最小骨架」+ 指向 `director-mod-v0.md`。
5. `REGISTER-director-v0.md` 的收束 checkbox 逐条勾选（活服务验收一项如实留空）。

## 5. 门禁（本轮实跑，全部在 `mod/wave3` 集成 tip 上）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| Rust 测试 | `cargo test --workspace --all-targets` | **1078 passed / 0 failed**（Wave 2 基线同命令 1002） |
| Rust doc 测试 | `cargo test --doc --workspace` | **3 passed / 0 failed** |
| 格式 | `cargo fmt --all -- --check` | clean（exit 0） |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | **0 warning**（exit 0） |
| Rust 占比 | `cargo run -p xtask -- rust-ratio` | **96.2857% PASS**（门槛 95%；两套 sidecar 的 py 计入分母） |
| 前端静态检查 | `cd shell/flutter && flutter analyze` | **No issues found** |
| 前端测试 | `cd shell/flutter && flutter test` | **876 passed**（Wave 2 基线 857；+10 壁纸 / +9 桌宠） |
| 仓库根历史资产 | `python3 -m pytest tests/ -q` | **22 passed, 1 skipped** |
| 语音 sidecar 自检 | `voice_sidecar.py --selftest` | **70 项检查全过**（离线，未发请求） |
| B 站 sidecar 自检 | `bilibili_sidecar.py --selftest` | **OK（16 项断言）** |

**共享 target 的坑（实测一次）**：第一次 `cargo test --workspace --all-targets` 报
`E0425: cannot find function prepare_transcript`——各 worktree 共用
`CARGO_TARGET_DIR`，dep-info 是相对路径，desktop 的测试二进制误用了旧 rlib（**假错误**）。
按协议 `cargo clean -p live2d-ai-mod-voice-input -p live2d-ai-desktop` 后复跑即 **1078/0 绿**。

## 6. Wave 2 §8 未决的处置（逐条）

| §8 | 处置 |
| --- | --- |
| 1 端到端肉眼验收未做（CosyVoice / 浏览器） | **仍是缺口**，但已**显式降级**：本波证据是「脚本级 + 测试级」，活服务/浏览器验收需 Win/WSL + 本机 TTS，属 rc.4 |
| 2 `memory` 检索质量无基线评测 | **关闭**：固定语料 fixtures + 命中/不命中断言（含中文与「词面不重叠→不注入」） |
| 3 每轮全量读 JSONL + 不物理删除 | **部分关闭**：淘汰改为**物理**（条数上限原子重写）；全量读盘仍是已知取舍（文档写明） |
| 4 与 persona 只有 last-writer-wins | **关闭（钉死）**：`memory-mod-v0.md` §5.1 + REGISTER + **两向对称测试**（persona 侧与 memory 侧都写） |
| 5 壁纸无删除/排序、`playlist_len` 手填被覆盖 | **关闭**：增删/排序 UI + 纯函数；`playlist_len` 退出可编辑 schema、只读暴露 |
| 6 `voice-input` 的 `backend`/`locale` 不参与请求 | **关闭（可测化）**：`backend` 走明确分支（结构性无网络），`locale` 真影响归一化；活服务 E2E 归 rc.4 |
| 7 桌宠窗口未开、Flutter 不消费状态面 | **关闭（软闭环）**：Flutter「Mod 管理」消费 `/state` + 配置热更新可测；窗口未开是**继续生效的裁决** |
| 8 导演仍是 RFC | **关闭（推进一格）**：`live2d-ai-mod-director` 最小骨架，已注册第 7 个、缺省停用、**零投递** |
| 9 `docs/README.md` 断链 `../PLAN.md` | **关闭**：指向 `legacy/PLAN.md` |
| 10 未 push | **保持未 push**（本波默认否） |

## 7. 与将来 `0.2.0-rc.4` 的差距

rc.4 若要把 Wave 3 收成一次**发布**，还缺：

1. **活服务 / 肉眼验收**：`ignite.sh` + Flutter 产物 + 本机 CosyVoice，跑
   语音 sidecar → 主链开口、壁纸真换图、桌宠状态面在浏览器可见、director 启用后
   `GET /mods/director/state` = 200（未启用 503）；`scripts/verify_core_chain.py`。
2. **director 的投递通道**：per-request TTS 参数通道 / 预置动作槽位（RFC §3.2/§9 的
   **未实现**项）。在此之前 director 只能是「可观察决策日志」，不能影响输出。
3. **版本与发布**：三处版本号同步 + `docs/releases/v0.2.0-rc.4.md` +
   `AGENTS.md` 的「当前版本」段（本波**刻意未做**）。
4. **记忆工程化**：备份/导出/恢复入口、尾部读或索引（放大窗口前）。
5. **壁纸工程化**：去重、缩略图、拖拽排序（均非目标）。
6. **桌宠硬闭环**（若要做）：先立项论证「谁来维护第二个 UI 壳」
   （`core-chain-baseline.md` §3.6），再谈真窗口。
7. **push**：本波七条轨分支 + `mod/wave3` 全部只在本地。

## 8. 归档点

- 集成 tip：`mod/wave3`（本文件所在提交之后不再追加功能）。
- 各轨 tip 与 merge 见 §3 表；七条轨 worktree 保留在
  `/home/skystar/Live2D-Ai-w3-{voice,wall,memory,pet,persona,external,director}`。
- 回滚：`mod/wave3` 可整体丢弃（各轨分支仍在，基座 `118bd435` 可重建）。
- **未 push**：以上全部只存在本地。
