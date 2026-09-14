# Wave 3 并行协议（mod/w3-voice / wall / memory / pet / persona / external / director）

> 上游：Wave 2 收束时留下的 §8 未决清单（[`docs/releases/v0.2.0-rc.3.md`](../../releases/v0.2.0-rc.3.md) §8）。
> **真源**：`/home/skystar/Live2D-Ai-wave2` @ `1e789cb6`（本地 `0.2.0-rc.3` 候选，**未 push**）。
> 集成分支：**`mod/wave3`**（指挥台 worktree `/home/skystar/Live2D-Ai-wave3`）。
> **本波不发布、不 bump 版本号、不打 tag、默认不 push。**
> 版本保持 `0.2.0-rc.3`（Cargo / pubspec / README **一行不改**）；**不要**写
> `docs/releases/v0.2.0-rc.4.md`。

## 0. 一句话

Wave 2 把能力做成「能演示」；**Wave 3 把每个已注册 Mod 补到可日常用的闭环**，
并清掉 §8 里挡日常的洞：语音 sidecar 真能喂音频且 `backend`/`locale` 不再是死配置、
壁纸列表可维护且无自相矛盾字段、记忆有质量基线与淘汰策略且计数可观察、
桌宠给 Flutter 一个可读的状态面、persona 坏卡路径是可回归的 E2E、
external-input 有接受/拒绝/忙碌计数与 sidecar 节流、导演从 RFC 推进到
**可启用的最小骨架**（缺省停用，收束时注册 6→7）。

## 1. 基座提交（主 agent 独占，先于各轨落盘）

`mod/wave3` 上先落一个「Wave 3 基座」提交，只解决**多轨共同缺**的公共面：

| 基座改动 | 文件 | 为什么必须有 |
| --- | --- | --- |
| `ModEventTopic::TurnEnded`（payload = turn id） | `live2d-ai-mod-system/src/topics.rs`、`live2d-ai-desktop/src/supervisor.rs` | §8.8 点名的基座需求：Mod 只有「轮开始」没有「轮收口」。导演要在轮末把决策日志结项、记忆要把本轮命中写进计数。发点在 `run_one_turn` 返回之后，**成功/失败都发** |

**明确不在基座里**（Wave 2 已经把通道建好，本波不需要新基座）：
`apply_settings` 一等化、`state_json` + `GET /mods/{id}/state`、
`mods.json` 原子写回、Flutter 偏好写回（`DisplayPrefs` / `sendStageBg`）——
各轨直接用既有通道。

**独占清单（谁都不许动，主 agent 收束时一次改到位）**：
`main.rs::AVAILABLE_MOD_FACTORIES` / `mod_count_*` / `mod_factory_ids_match_expected` /
`cli_entry::default_mods_manifest` / 根 `Cargo.toml` 的 `version` / `shell/flutter/pubspec.yaml` /
`AGENTS.md` / `docs/README.md` / `docs/releases/**` / `README.md` / `README.zh-CN.md` /
`docs/architecture/mod-product-chain.md` 的 Mod 清单表。

> 唯一例外：G 轨可以改根 `Cargo.toml` 的 **`members` 数组**（加一行新 crate），
> **不得**碰 `[workspace.package] version`。

## 2. 环境（先读这一节再开工）

本机磁盘只剩 ~12 GB、内存 ~7 GB，因此**所有 worktree 共用一个 cargo target**：

```bash
export CARGO_TARGET_DIR=/home/skystar/Live2D-Ai-integrate/target
export PATH="$HOME/flutter/bin:$PATH"   # 只有 B / D 轨需要
```

- **不要**在 worktree 里另起 `target/`（冷编译会撑爆磁盘）；
- 共享 target 的代价是 cargo 全局构建锁 + 偶发「假错误」：dep-info 是相对路径，
  同名 crate 在另一个 worktree 后编译会被误判 fresh。处置：
  `cargo clean -p <crate>` 后重跑。**不要**为了绕过而改 target 目录；
- **行数纪律**：`cargo run -p xtask -- rust-ratio` 会看源码行数，源码 ≤500 行
  （豁免 ≤1000 需头注理由）、测试文件 ≤800 行。新写的文件别超。
- 门禁命令（`AGENTS.md` 一张表逐条对齐）：
  `cargo test --workspace --all-targets` / `cargo test --doc --workspace` /
  `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings` /
  `cargo run -p xtask -- rust-ratio`；B / D 轨另加
  `cd shell/flutter && flutter analyze && flutter test`。

## 3. 七条轨

| 轨 | 分支 | worktree | 对应 §8 |
| --- | --- | --- | --- |
| A 语音 | `mod/w3-voice` | `/home/skystar/Live2D-Ai-w3-voice` | §8.6 |
| B 壁纸 | `mod/w3-wall` | `/home/skystar/Live2D-Ai-w3-wall` | §8.5 |
| C 记忆 | `mod/w3-memory` | `/home/skystar/Live2D-Ai-w3-memory` | §8.2 / §8.3 / §8.4 |
| D 桌宠 | `mod/w3-pet` | `/home/skystar/Live2D-Ai-w3-pet` | §8.7 |
| E 人设 | `mod/w3-persona` | `/home/skystar/Live2D-Ai-w3-persona` | §8.4（persona 侧）|
| F 外部输入 | `mod/w3-external` | `/home/skystar/Live2D-Ai-w3-external` | 任务书 §6 |
| G 导演 | `mod/w3-director` | `/home/skystar/Live2D-Ai-w3-director` | §8.8 |

**文件所有权（越权即打回）**：每条轨只许改自己「拥有」的文件。需要改别人的文件
= 基座不够，**停下报告主 agent**，不要自己动。

### 轨 A —— voice-input + voice-sidecar 闭环

- **worktree** `/home/skystar/Live2D-Ai-w3-voice`，**分支** `mod/w3-voice`。
- **拥有**：`crates/live2d-ai-mod-voice-input/**`、
  `crates/live2d-ai-desktop/src/web_api/voice_routes.rs`、
  `crates/live2d-ai-desktop/src/web_api/voice_routes_tests.rs`、
  `docs/voice-input.md`、`docs/examples/voice-sidecar/**`、
  `docs/plans/parallel-mods/REGISTER-voice-sidecar-v1.md`、
  `docs/plans/parallel-mods/REGISTER-voice-input.md`。
- **闭环定义**：sidecar 真实音频/文件 → transcript 端点 → 主链开口；
  `backend` / `locale` **不再是死配置**。
- **必做**：
  1. `backend=mock|sidecar` 行为**可测**：`mock` 不碰网络（断言 handler 不发起
     任何请求；用纯函数 + 注入点证明），`sidecar` 路径有**失败码表**
     （401/403/busy/empty/timeout/transport）与**退避说明**。
     建议落点：`voice_routes.rs` 的 handler 读取 Mod config 的 `backend`，
     `mock` 直接清洗后注入（当前行为），`sidecar` 走一条**明确的**处理分支
     （推模式：Rust 不开 socket；失败码由发送方/文档定义）。**不要**在 Rust 里开 socket。
  2. `locale` 至少影响**清洗或样例**，或写成「暂不影响 ASR 引擎」并用测试 + 文档钉死——
     **不许 silently 无效**。建议：`locale` 影响标点/空白归一化策略表
     （`zh-CN` / `en-US` 各一条断言），并在 `docs/voice-input.md` 写清
     「locale 只影响 text 归一化，不影响 ASR 引擎选型」。
  3. sidecar 除 `--dry-run` / `--selftest` 外，给**一条可复制演示**：
     `--audio fixtures/fake_zh.wav`（假 wav 已在库里）**或**麦克风二选一，
     写进 `docs/examples/voice-sidecar/README.md`；失败（403/401/busy/empty）
     有**逐条退避**说明。
  4. `docs/voice-input.md` 加一段**职责边界表**：何时用 `/voice/transcript`
     vs `/external/chat`。
  5. REGISTER 更新（`REGISTER-voice-sidecar-v1.md` + `REGISTER-voice-input.md`）。
- **验收**：活服务或**脚本级证据**（sidecar `--selftest` + `--dry-run` 实跑输出）
  + 单测；`docs/voice-input.md` 与实现逐条对齐。
- **禁止**：云 ASR SDK 静态链进主 binary；在 Mod / handler 里开 socket 或读进程环境
  （token 一律走 `secrets::lookup`）；改主链。

### 轨 B —— wallpaper 列表可维护 + 无自相矛盾字段

- **worktree** `/home/skystar/Live2D-Ai-w3-wall`，**分支** `mod/w3-wall`。
- **拥有**：`crates/live2d-ai-mod-wallpaper/**`；
  Flutter：`shell/flutter/lib/settings/display_prefs.dart`、
  `shell/flutter/lib/api/wallpaper_api.dart`、
  `shell/flutter/lib/app/shell_wallpaper.dart`、
  `shell/flutter/lib/app/shell_prefs.dart`、
  `shell/flutter/lib/settings/sections/appearance_section.dart`、
  `shell/flutter/test/**`（**只**新增/修改壁纸相关的测试文件，
  例：`wallpaper_wiring_test.dart` / `display_prefs_test.dart`）；
  `docs/architecture/wallpaper-mod-v0.md`、`docs/plans/parallel-mods/REGISTER-wallpaper-wire.md`、
  `REGISTER-wallpaper.md`。
- **禁止**：改 `shell/flutter/lib/main.dart`（**不要**加 `part` 或 import——
  新纯逻辑写进已存在的 `display_prefs.dart` / `wallpaper_api.dart` 两个 library）；
  改 `api/mods_api.dart` / `settings/sections/dev_tools_section.dart`（D 轨的）；
  重做 framebuffer / `stage_bg` 渲染算法 / 在线图库大轮播。
- **闭环定义**：列表可维护；决策 → `DisplayPrefs` → stage-bg 日常可用，无自相矛盾字段。
- **必做**：
  1. **playlist 增删/排序**（最小 UI）：在「外观与互动 → 舞台背景」的轮播行加
     「删除当前/第 N 张」「上移/下移」（或等价的低成本操作）。纯逻辑
     （`removeStagePlaylistAt` / `moveStagePlaylist` 等）放 `display_prefs.dart`，VM 可测。
  2. **`playlist_len` 与真实列表一致**：把 `playlist_len` **从
     `wallpaper_settings_spec()` 的可编辑字段里删掉**（它由 Flutter 经
     `POST /mods/wallpaper/config` 写回，手填必被覆盖 → 自相矛盾）。
     同步改 `wallpaper_settings_spec` 的 keys 断言测试。数值改为**只读**暴露：
     继续出现在 `state_json`（已有）+ `config`（`GET /mods` 会回）。
     文档写明「`playlist_len` 是运行值，真源 = Flutter `stagePlaylist.length`」。
  3. **`follow_stage` / `interval` 各至少一条可脚本验证的 state 轨迹**：
     用 `cargo test`（Mod 侧 `tick` / `to_prefs_patch`）或脚本给出
     「相隔 T 后 `state.decision.index` 递增」「`sync_shell_stage_bg` 决策」的证据。
  4. `docs/architecture/wallpaper-mod-v0.md` 改成**已接线**，列出剩余**非目标**
     （不做在线图库大轮播 / 不做缩略图 / 不做去重）。
- **验收**：Mod 单测 + Flutter 纯逻辑测试（新增 ≥8 条）；
  `flutter analyze` 无问题 + `flutter test` 全绿。

### 轨 C —— memory 质量基线 + 淘汰 + 计数 + 与 persona 共存

- **worktree** `/home/skystar/Live2D-Ai-w3-memory`，**分支** `mod/w3-memory`。
- **拥有**：`crates/live2d-ai-mod-memory/**`、
  `docs/architecture/memory-mod-v0.md`、
  `docs/plans/parallel-mods/REGISTER-memory-v0.md`。
- **禁止**：绑死商业向量云；改主链人设字段**结构**；改 `persona` crate（E 轨的）。
- **闭环定义**：写入 → 检索 → 注入 → 停用剥离 日常可依赖；与 persona 冲突可预期。
- **必做**：
  1. **最小质量基线**：固定语料 fixtures（`tests/fixtures/` 或 inline 常量）
     + 命中/不命中断言，**≥1 组中文**。至少含一组「应命中」与一组
     「词面不重叠 → 不命中」（`last_hits:0` 不注入）。
  2. **淘汰策略至少一种做实**——**选定：条数上限（count cap）物理淘汰**。
     当前 `max_records` 只是检索窗口。改为：写入时若超过上限，**物理截断**
     JSONL（或写回压缩后的文件）；有单测证明「第 N+1 条写入后文件里只剩 N 条」。
     若截断有数据丢失风险，**必须**在文档写明取舍（无备份语义 → 先不做，
     那就把「显式 delete API」做实）。**二选一，必须有一个是真物理的**。
  3. **与 persona 共存**：**选定 last-writer-wins**（不强做 marker 合并）。
     把规则**钉死**在 `memory-mod-v0.md` §5.1 + `REGISTER-memory-v0.md`：
     两者都写 `persona.system_prompt`；memory 每次注入前先剥自己的 marker 块，
     再从 `ModServices.settings` 读到的 base 重拼；**后写者覆盖前者的记忆块**，
     无仲裁。补一条测试：模拟 persona 先写 base、memory 再注入 → 断言
     memory 块存在且 base 保留；反向再来一次 → 断言 last-writer-wins 的可预期结果。
  4. `state_json` 暴露可观察计数：`writes` / `hits` / `injects` / `errors`
     （可与现有 `remembered` / `last_hits` 并存，但**这四个键必须在**）。
- **验收**：新增/更新测试 ≥12 条；`state_json` 计数有断言；文档与实现一致。

### 轨 D —— pet-desktop 软闭环（Flutter 消费状态面）

- **worktree** `/home/skystar/Live2D-Ai-w3-pet`，**分支** `mod/w3-pet`。
- **选定：软闭环**——**不设窗口**（硬开窗口要唤醒休眠原生壳，违反
  `core-chain-baseline.md` §3.6 的休眠台账）。产品标签：**「仅状态面」**。
- **拥有**：`crates/live2d-ai-mod-pet-desktop/**`；
  Flutter：`shell/flutter/lib/api/mods_api.dart`（加 `state(id)` 读取）、
  `shell/flutter/lib/settings/sections/dev_tools_section.dart`（`ModsSection` 里消费
  `/mods/{id}/state` 展示关键字段）、`shell/flutter/test/mods_section_test.dart`（可新建
  `pet_desktop_state_test.dart`）；
  `docs/architecture/pet-desktop-mod-v0.md`、`docs/plans/parallel-mods/REGISTER-pet-desktop-v1.md`。
- **禁止**：改 `main.dart`（不加 part/import）；改 B 轨的 Flutter 文件；
  重写 desktop 壳；引入第二套渲染引擎（winit/egui/wgpu）。
- **闭环定义**：启用后用户/前端能感知「窗在不在」——以**状态面**呈现。
- **必做**：
  1. Flutter「Mod 管理」里，选中的 Mod 可**展开运行态**（新 `ModsApi.state(id)` →
     `GET /api/v1/mods/{id}/state`）；`pet-desktop` 必须能看到
     `always_on_top` / `click_through` / `opacity` / `voice_active` /
     `window.opened`（恒 `false`）+ `reason`（`native_shell_dormant`），
     并在 UI 上明确写出「窗口未开（原生壳休眠），此面仅状态」。
  2. **配置热更新可测**：Flutter 改配置（`POST /mods/pet-desktop/config`）→
     重新取 state，字段跟着变；用 widget/单元测试（可注入 fake API）证明。
  3. Rust 侧保持 Wave 2 的 `state_json` 契约；若加字段，补 Rust 单测。
  4. `docs/architecture/pet-desktop-mod-v0.md` 写「软闭环」选择与理由 +
     与原生壳的边界。
- **验收**：`flutter analyze` 无问题 + `flutter test` 全绿（新增 ≥5 条）；
  Rust 侧测试全绿。

### 轨 E —— persona 坏卡 E2E + 与 memory 共存

- **worktree** `/home/skystar/Live2D-Ai-w3-persona`，**分支** `mod/w3-persona`。
- **拥有**：`crates/live2d-ai-mod-persona/**`、
  `docs/plans/parallel-mods/REGISTER-persona-polish.md`、
  `docs/architecture/persona-mod-*.md`（若不存在则不改）。
- **禁止**：改 `memory` crate（C 轨的）；改主链 `[persona]` 字段结构。
- **闭环定义**：坏卡 Failed、启停还原、与 memory 共存策略在 UI/文档一致。
- **必做**：
  1. **坏输入 E2E 抽 2–3 条**：enable → `Failed`（坏 JSON / 非法字符 / PNG 无 chara 块/
     card_json 为数组）→ 修好 → 再 enable 成功；每条断言
     「Failed 时主链 `system_prompt` 一字未动」+「修好后再 enable 真的接管」+
     「disable 回基线」。用 `ModRuntime` 级测试（不是纯函数单测）。
  2. **与 memory 共存**（C 轨选定的 last-writer-wins）：补对称测试——
     persona 写 base 后 memory 注入 → 断言结果符合 `memory-mod-v0.md` §5.1；
     文档里引用同一段契约文字，**两侧措辞必须一致**。
  3. 更新 REGISTER。
- **验收**：persona crate 测试全绿（新增 ≥6 条）。

### 轨 F —— external-input + bilibili-sidecar 可观察

- **worktree** `/home/skystar/Live2D-Ai-w3-external`，**分支** `mod/w3-external`。
- **拥有**：`crates/live2d-ai-mod-external-input/**`、
  `crates/live2d-ai-desktop/src/web_api/external_routes.rs`
  （+ 其测试文件若存在）、`docs/examples/bilibili-sidecar/**`、
  `docs/external-input.md`、`docs/plans/parallel-mods/REGISTER-*.md`（external 相关）。
- **禁止**：B 站协议进 Rust；在 handler 里重写清洗/模板逻辑（复用 Mod 纯函数）。
- **闭环定义**：直播日常可跑，可观察，不全靠猜。
- **必做**：
  1. **接受/拒绝/busy 计数**进 `state_json`（**选定 state_json**，因为可测）。
     建议：`live2d-ai-mod-external-input` 用 `AtomicU64` 暴露
     `record_accept()` / `record_reject()` / `record_busy()`；
     handler 在对应分支调用；`state_json` 返回 `{"accepts":N,"rejects":N,"busy":N,...}`。
     补 handler 回归：连续 3 次成功 / 1 次坏 token / 1 次忙 → 计数正确。
  2. **sidecar 节流最小策略**（例如每秒最多 1 条）**可开关**：
     `--min-interval-ms`（缺省 1000，`0` = 关）。在 sidecar README 写清。
  3. **`SEND_GIFT_V2`**：若 blivedm 仍不支持，保持**只 log**，但在
     `state_json` 或 README 暴露 `v2_ignored` 计数（选定：`state_json` 里
     `v2_ignored`，由 sidecar 经一个新可选字段上报 → 或纯 README 计数，
     二选一，但必须**可见**）。若上游已支持则补回调——**先查证再决定**。
  4. 更新 REGISTER。
- **验收**：Mod 单测 + handler 回归；sidecar `--selftest` 覆盖节流逻辑。

### 轨 G —— director 最小骨架（可启用，缺省停用）

- **worktree** `/home/skystar/Live2D-Ai-w3-director`，**分支** `mod/w3-director`。
- **选定：从 RFC 推进到「可启用的最小骨架」**，收束时由主 agent 注册进
  `AVAILABLE_MOD_FACTORIES`（6 → 7），**缺省停用**（`default_mods_manifest` 不收录）。
- **拥有**：新 crate `crates/live2d-ai-mod-director/**`、
  根 `Cargo.toml` 的 **`members` 一行**（**不得**碰 `version`）、
  `docs/architecture/director-rfc.md`、
  `docs/architecture/director-mod-v0.md`（新建）、
  `docs/plans/parallel-mods/REGISTER-director-v0.md`（新建）。
- **禁止**：改 `main.rs` FACTORIES / `mod_count_*`（**主 agent 收束时改**）；
  改 `default_mods_manifest`；复活 Action 真通道（`action_tx` 保持休眠）；
  实现动作库；投递任何动作 / TTS 参数。
- **闭环定义**：启停可用、`state_json` 可观察决策日志、**不投递**。
- **必做**：
  1. crate 起手照 `live2d-ai-mod-template`，`descriptor.api_version` 对齐；
  2. `settings_spec`（例如 `enabled` 不由 spec 管；`log_capacity` Number 缺省 20、
     `emotion_lexicon` Select/String 可选）；
  3. 订阅 **`TurnPrompt`**（拿正文）+ **`TurnEnded`**（基座新增，把本轮决策结项）；
  4. 决策 = **纯函数**：正文 → `{emotion, intent, suggested_tts:{"speed":..,"pitch":..}}`
     （关键词/词典驱动，确定性、可单测）；**只写日志 + 进 `state_json`**；
  5. `state_json` 返回最近 N 条决策 + 计数（`turns_seen` / `decisions` /
     `errors`）+ **`delivered:false`**，明确「未投递」；
  6. 测试 ≥10 条：纯函数（命中/缺省/词表优先/空输入）、订阅主题正确、
     `state_json` 形状与容量上限、**`action_tx` 未被调用**（用 mock 断言）；
  7. `action_tx` 若要用，只能调用并记录返回 `false`（休眠），并在测试里断言
     「没有任何动作被投递」；
  8. 文档：`director-mod-v0.md` 写骨架范围、状态面、与休眠 `action_tx` 的关系、
     晋升门槛；`director-rfc.md` 更新 §8/§9（Wave 3 已推进到骨架，仍不投递）。
- **验收**：`cargo test -p live2d-ai-mod-director` 全绿；docs 与实现一致；
  **摘要必须写明「已推进到骨架，待主 agent 注册为第 7 个」**。

## 4. 汇报格式（每轨收工必须给全）

```
轨：<A|B|C|D|E|F|G> / 分支：<branch>
worktree tip SHA：<sha>
闭环定义是否达成：<是/否 + 一句话证据>
测例：<cargo 新增/总数> ; <flutter 新增/总数，若适用>
与 REGISTER 的偏差：<逐条，或「无」>
是否改过 FACTORIES / mod_count_* / 缺省 manifest / 版本号 / docs/README.md：<否 | 是 + 为什么（=打回）>
未决 / 已知缺口：<逐条>
门禁实跑结果：<命令 + 结果，含 fmt/clippy；B/D 加 flutter>
```

## 5. 收束（主 agent）

1. 各轨 merge 进 `mod/wave3`（G 轨另加 FACTORIES 6 → 7 + `mod_count_is_seven` + id 断言）；
2. 全量门禁绿（cargo test/doc/fmt/clippy/rust-ratio；Flutter analyze+test）；
3. 写 `docs/plans/parallel-mods/WAVE3-CLOSEOUT-2026-09-14.md`（每轨 tip / 闭环是否达成 /
   未决 / 与将来 rc.4 的差距）——**不是** release 文件；
4. 修 `docs/README.md` 既有断链（`../PLAN.md`）；
5. 交付摘要，**push 默认否**。
