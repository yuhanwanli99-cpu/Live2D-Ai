# Live2D-Ai 项目说明（AGENTS.md）

> 本文件是项目级通用说明，供任何 CLI agent（pi / Claude Code / Hermes 等）读取。
> 状态：2026-09-10 重写——确立 **Rust 核心 + Flutter 前端** 双主导分层，
> 前端/接口层自 `rust-ratio` 门禁中**显式豁免**（原版只描述 Rust 单主线）。
> **2026-10-01 单一化：本文件是唯一现行真源。**
> **2026-10-06 工作树单一化**：linked worktree `Live2D-Ai-fe` 已删除，**唯一工作树 =
> `/home/skystar/Live2D-Ai`（分支 `main`）**——旧分支 `mod/persona-polish` @ `88342ce`
> 与它那份「3 个 Mod / 0.2.0-rc.1」的教义一并退役。同名 `AGENTS.md` 若出现在别处都是
> 过时副本，**不要**按它写代码或文档；文档地图与生命周期三分见 `docs/DOC-MAP.md`。
> **版本口径（现行，2026-10-06 复核）**：现状 = **`0.2.1-rc.1`**（`Cargo.toml` 的
> `version = "0.2.1-rc.1"`、tag `v0.2.1-rc.1`、发布说明 `docs/releases/v0.2.1-rc.1.md`）。
> 再往前的 **0.2.0 收口 + 去臃肿第一轮**（2026-10-01 起：S0 文档整理 → `rc.8-a` 正确性与诚实性
> → `rc.8-b` 结构 → 0.2.0 末版，计划书 `docs/plans/PLAN-debloat-and-closeout-2026-10-01.md`）
> 与下面 rc.7 的记录都是**历史**——`0.2.0-rc.7` 不是现状。

## 项目背景

- 文档索引见 `docs/README.md`；核心契约与目录约定见 `docs/architecture/core-contracts.md`
  与 `docs/architecture/directory.md`。
- 定位：**通用人形皮套 AI 接入一体化平台**——AI + Live2D 人形皮套的通用接入与应用层，
  **不绑定任何单一模型**（模型由用户合法导入，`assets/models/` 不捆绑二进制），
  **不做复杂上层**（实现保持最小）。验证「文本 → LLM（纯对话，无工具）→ TTS → 驱动口型
  → Live2D 皮套渲染 + 前端 UI」闭环。
- **上一版 `0.2.0`（0.2.0 收口 + 去臃肿第一轮；发布说明 `docs/releases/v0.2.0.md`；现行版本 `0.2.1-rc.1` 见本节首段）**：
  下面 ①–④ 是**沿用 rc.7「正确性与诚实性」那一轮**的详细记录（0.2.0 本轮的收口/去臃肿内容见发布说明）：
  ① **审计主发现 `F-0005-2`（重建放大链，本轮最有价值的单点）**：`AppShell` 新增宿主状态代际
  `settingsRevision`、`AppShellState` 新增 `_settingsTick`（设置数据通知计数），两者与 `section`
  组成设置面板 `_pane()` 的**缓存键** —— 键不变就返回**同一个 widget 实例**，父级重建被
  `Element.updateChild` 短路，于是**设置分区不再随每个 `text_delta` 重建与布局**（10 次增量的
  重建次数由 **+10 降到 0**，有计数断言）。**如实标注**：只挡住「当前分区」这棵子树；外壳、
  设置头部与 `ChatPanel` 仍随 delta 重建（更彻底的「外壳不再整体订阅 `_chat`」未做），**幅度未真机 profile**。
  ② **4 条假绿灯改成可失败断言**（`F-0005-1` / `F-0005-3` / `F-0005-6` / `F-0005-7`）：守卫对象从
  `app_shell.dart`（那里唯一命中是一句「不要这么做」的注释）改到 `ui/stage_host.dart` 且**剥注释**；
  空泡改真判据并**顺带修实现**（空消息不再画 16 px 空泡；失败轮 / 流式中 / 只有思考三种例外保留）；
  `ChatRole.system` 改**行为**断言；真泵 `StatePill` + `disableAnimations:false` 对照组。
  **每条都有「破坏实现 → 断言变红」的红-绿双向原始输出**（见 `docs/releases/v0.2.0-rc.7.md` §4.1）。
  ③ **Stage C2 仓库卫生**：worktree **25 → 3**、本地分支 **31 → 12**、stash **1 → 0**（先导出补丁再 drop）、
  未跟踪 **0**、rc 线 tag **补齐到 7 个**（含新补的 `v0.2.0-rc.2` / `v0.2.0-rc.3`）。
  ④ 版本三处（+ zh README）同步 rc.7；发布说明 `docs/releases/v0.2.0-rc.7.md`
  （上一版 `docs/releases/v0.2.0-rc.6.md` = 背景透传追平参考 + 背景域审计 10 条）。
- **更早（rc.4，2026-09-26，表演协议 v1 全链 + 导演可观测 + 单模型动作强度）**：
  把「动作/表演」从**两条驱动通道打架**收成一条，并补齐三块——
  ① **阶段3 单一驱动者**：退役「前端拉 `latest.preset_id`」驱动舞台的通道，动作只由 WS `action_cue` 驱动
  （`preset_id=="none"` = 撤销哨兵，`cues:[]` = 本轮不动，D10–D13）；
  ② **阶段4 表演协议 v1**：`speak` 退役为 `segments`（**只切分原文、逐字不变**；上屏 == 送 TTS ==
  `clean_for_tts(段)`），三表演字段 `body`/`head`/`expression`（同类 add、立即生效、`hold`），
  **唯一时间基准 = 音频播放时钟**（`stage-clock`），渲染面**事件级 ack 四条 + 段结束**，
  **会话 baseline** 绑会话（停止/新消息清三样并回 baseline、不补帧）；
  ③ **阶段5**：dev_mode 下「**导演可观测**」四栏（决策参数 / 事件流 / 传参对照 / 送 TTS 文本）+
  **单模型动作强度** `[action.models.<id>]`（三键各自回落全局）。契约真源
  `docs/architecture/performance-protocol-v1.md`；发布说明 `docs/releases/v0.2.0-rc.4.md`。
- **上一版本地集成 `mod/product-grade`（产品级加强波次，2026-09-14，未发布、不打 tag、
  版本仍 `0.2.0-rc.3`）**：**封存 `wallpaper` + `pet-desktop` 两个 Mod**（用户裁决：
  wallpaper 删除封存、本波不做；pet-desktop 封存、暂不推、不做真窗/应用级桌宠）——
  两者移出 `AVAILABLE_MOD_FACTORIES`，7 → **5**，`mod_count_is_seven` →
  **`mod_count_is_five`**；crate 暂留 workspace（可编译可测）并标 **ARCHIVED**，
  **禁止挂回**。理由 / 明确没连坐删掉什么 / 恢复条件见
  `docs/architecture/ARCHIVED-mods.md`；**用户手动的舞台/壳背景能力（`DisplayPrefs`）
  保留**——被拆掉的只是 wallpaper **Mod** 的接线（Flutter 侧 `wallpaper_api.dart` /
  `shell_wallpaper.dart` / `applyWallpaperPatch` 一并拆除）。其余五个 Mod 推到**产品级**：
  `external-input`（计数进 Mod 管理 UI + sidecar 节流可配）、`voice-input`（backend/locale
  说人话 + 失败码可读 + sidecar 最小成功路径）、`persona`（导入卡→enable→人设变→disable
  还原，UI 内完成）、`memory`（可见条数/hits/清空 + 注入可关 + 与 persona 策略钉死）、
  `director`（决策一等面板；当时的「**零投递不变**」结论**已变更**——director 现产按句
  `action_cue`（唯一驱动舞台；中性轮给 `preset_id=="none"` 撤销哨兵，D10），`latest.preset_id`
  **仅面板只读**、前端拉取驱动通道已退役（D12），现状见 §「动作与表演的现行状态（2026-09 实测）」；
  仍不复活 **core** 动作通道）。
  **未 bump 版本、未 push**；
  主链皮肤（LLM/TTS/口型/Live2D）与 `l2d-wasm-demo` 一行未改。
  收束见 `docs/legacy/plans/PRODUCT-GRADE-CLOSEOUT.md`。
- **上一版本地集成 `mod/wave3`（Wave 3 七轨闭环，2026-09-14，未发布、不打 tag、版本仍 `0.2.0-rc.3`）**：
  把六个已注册 Mod 补到可日用闭环并把 director 从 RFC 推进到**最小骨架**：
  ① `voice-input`：`backend=mock|sidecar` 走明确分支（结构性不开 socket）、`locale` 真影响
  转写归一化（新增 `normalize.rs`）、sidecar 六类失败码 + 逐条退避；
  ② `wallpaper`：列表可增删/排序（纯函数 + 最小 UI）、`playlist_len` 从可编辑 schema 移除
  （消除「手填被覆盖」矛盾）、interval/follow_stage 各一条 state 轨迹断言；
  ③ `memory`：固定语料命中/不命中断言、条数上限**物理淘汰**、`writes/hits/injects/errors`
  四计数、与 persona 的 last-writer-wins 对称回归；④ `pet-desktop`：**软闭环**（不设窗口）——
  Flutter「Mod 管理」消费 `GET /mods/{id}/state` 展示关键字段 + 配置热更新可测；
  ⑤ `persona`：坏卡 enable→Failed→修好→再 enable 的**值断言** E2E ×4；
  ⑥ `external-input`：接受/拒绝/busy 计数进 `state_json`、sidecar 节流可开关、`v2_ignored` 可见；
  ⑦ `director`：新 crate 最小骨架（零投递，缺省停用）。**基座（主 agent）**：
  `ModEventTopic::TurnEnded`。`AVAILABLE_MOD_FACTORIES` 6 → **7**，`mod_count_is_six` →
  `mod_count_is_seven`；**缺省仍只启用 `external-input`**。
  **未 bump 版本、未写 rc.4 发布说明、未 push**；主链皮肤与 `l2d-wasm-demo` 一行未改。
- **上一版 `0.2.0-rc.3`（Wave 2 五轨合成：语音 sidecar / 壁纸接线 / 记忆 / 导演 RFC / 桌宠，2026-09-14）**：
  把 Wave 2 的五条并行轨道合成一条集成分支：`AVAILABLE_MOD_FACTORIES` 5 → **6**
  （+ **memory**，只注册、**缺省停用**），`mod_count_is_five` → `mod_count_is_six`；
  **缺省仍只启用 `external-input`**。本版的目标是「**让至少两条能力从能编译变成能演示**」：
  ① **语音闭环**：新增 `POST /api/v1/voice/transcript`（**专用 loopback 端点**，不是复用
  `/api/v1/external/chat`——理由写在 `docs/voice-input.md` §10：voice 有自己的 token/locale 语义，
  且要让 `voice-input` crate 的 `clean_transcript` 真的跑在主链路上）；handler 复用该纯函数 →
  `supervisor.say`；**ASR 本体在 sidecar**（`docs/examples/voice-sidecar/`：`--transcriber fake|cmd:`、
  `--dry-run`、`--selftest`、失败码 0/2/3/4/5，纯标准库），**不把 ASR SDK 链进 binary**。
  ② **壁纸接线（结束占位）**：`WallpaperDecision::to_prefs_patch()` 是真投影（`apply_decision`
  的 warn+false **已删除**）→ Mod `state_json()`（内部 `Instant` 喂既有纯策略）→ Flutter
  `applyWallpaperPatch` → 既有 `DisplayPrefs` / `Live2DStage.sendStageBg`；播放列表**由 Flutter
  偏好持有**（`stagePlaylist` + 三条预算常量），不新增图库/文件服务/wasm 路径。
  ③ **记忆 v0**：新 crate `live2d-ai-mod-memory`（本地 JSONL + 中文 bigram/ASCII 词元重叠打分
  top-k，**零向量云依赖**），订阅**本轮新增的** `ModEventTopic::TurnPrompt`（payload = 本轮输入
  正文，因为 `TurnStarted` 只有 turn id）→ 经 `apply_settings` 写既有 `persona.system_prompt`；
  **只对下一轮生效**，与 persona 是 **last-writer-wins**（无仲裁，刻意）。
  ④ **导演 RFC**：`docs/architecture/director-rfc.md` 契约先行，**本轮不注册**（不新建 crate；
  「注册但 enable 即 Failed」被否决的理由在 RFC §8）；动作仍是**槽位占位**，`action_tx` 保持休眠。
  ⑤ **桌宠推进一格**：配置/事件态经 `GET /api/v1/mods/pet-desktop/state` 进可测面
  （**窗口未开**——原生壳休眠，见下方「原生第二壳的归属」）。
  **基座**（主 agent 独占）：`ModEventTopic::TurnPrompt`、`ModRuntime::state_json`、
  `ModRegistry::runtime_state`、`GET /api/v1/mods/{id}/state`（404 与 503 刻意分开）。
  **主链皮肤（LLM/TTS/口型/Live2D、壳/舞台背景）与 `l2d-wasm-demo` / framebuffer 一行未改**；
  契约范围见 `docs/plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md`。
  发布说明 `docs/releases/v0.2.0-rc.3.md`。
- **上一版 `0.2.0-rc.2`（Wave 1 三轨合成：voice-input + wallpaper + persona-polish，2026-09-14）**：
  把三条并行 Mod 轨道合成一条集成分支：`AVAILABLE_MOD_FACTORIES` 3 → **5**
  （external-input, pet-desktop, persona, **voice-input**, **wallpaper**），
  `mod_count_is_three` → `mod_count_is_five`；**缺省仍只启用 `external-input`**
  （voice-input / wallpaper 只注册、**缺省停用**）。**主链皮肤一行未改**，
  `l2d-wasm-demo` / framebuffer / 背景实现**未碰**。persona 行为变更：**坏配置现在显式
  `Failed`**（旧行为是记一行 error 后 `Ok`，界面显示「运行中」而主链没变）。wallpaper
  决策落点当时仍是**明文占位**（Wave 2 已接线）。
  发布说明 `docs/releases/v0.2.0-rc.2.md`。
- **上一版 `0.2.0-rc.1`（Mod 纪元第一基线：external-input 直播刚需 + 社区许可，2026-09-14）**：
  主链皮肤（LLM→TTS→口型→Live2D、壳/舞台背景）**冻结未改**。本版把**外部事件注入**
  做成刚需：`POST /api/v1/external/chat` 契约文档化（text/token/Origin/loopback/忙碌/
  启停门禁/模板前缀 + curl），**B 站抓取不在主仓，在 Win sidecar**
  （`docs/examples/bilibili-sidecar/`：blivedm → 清洗 → POST）；`external-input` Mod 加强
  （静态 `settings_spec` v2：`listen_port`/`token`/`text_template`/`prefix`；启停唯一真源 =
  manifest `enabled`，停用 → `403 mod_disabled`；**缺省启用**）；**`local-llm` 废除启动**
  （移出 `AVAILABLE_MOD_FACTORIES`，4→3；crate 暂留仓库并标 DEPRECATED，**禁止挂回**）；
  `docs/architecture/mod-community-license.md` 写明注册/分发/许可边界。
  发布说明 `docs/releases/v0.2.0-rc.1.md`；契约 `docs/external-input.md`。
- **上一版 `0.1.0-rc.5`（壳全局背景 + 与舞台同步，2026-09-14）**：
  主链 LLM→TTS→口型→Live2D **一行未改**。本版只动前端显示层：`DisplayPrefs` 增加
  `shellImage` 与 `syncShellStageBg`（默认 `true`＝壳与舞台**共用同一张图**，
  `effectiveShellImage` 是一份真相）；壳根铺一层**固定 0.15 透明度**的全局背景
  （`ui/shell_backdrop.dart`，**不做** opacity 滑条），聊天面板在有背景时留一点透
  （`kShellSurfaceAlpha = 0.86`）但保持可读，「外观与互动」新增「壳背景」块
  （选图 / 清图 + 与舞台同步）。背景**只住本机 `DisplayPrefs` / `localStorage`**：
  不写 `live2d-ai.toml`、不做分区独立背景 / 轮播 / 背景 Mod / 在线拉图 / 动态加载。
  发布说明（含 **Win 选图 → 可见 → 清图** 肉眼 checklist）：
  `docs/releases/v0.1.0-rc.5.md`。
- **上一版 `0.1.0-rc.4`（Mod 产品链路 + 主链人设收敛，2026-09-13）**：
  主链 LLM→TTS→口型→Live2D 未变；本版把 Mod 从「能编进 binary 的骨架」做成
  **产品链路**——`mods.json` 启停/配置**原子写回**、`GET /api/v1/mods` 带
  `settings_spec`+`config`（`secret` 字段脱敏）供 Flutter 渲染表单、新 Mod 模板 crate +
  `descriptor.api_version` 门禁、`ModServices.apply_settings` **一等化**（删掉旧的
  `__apply_settings` 事件走私）；**酒馆角色卡抽成第一条标准 Mod**
  `live2d-ai-mod-persona`（V1/V2 JSON + PNG `chara` → 合成 `system_prompt`），
  主链 `[persona]` **只剩** `system_prompt` + `max_history_pairs`（**升级需手改一次
  旧 toml**，见发布说明 §4.3）；Win 舞台背景图三处根因修复。发布说明：
  `docs/releases/v0.1.0-rc.4.md`。上一版 rc.3 是结构质量（正文兜底 / 拆大文件 /
  双壳休眠 / CI 对齐）：`docs/releases/v0.1.0-rc.3.md`；rc.2（第二基线：动作层删到底 +
  模型库闭环 + `.env` 密钥真源）：`docs/releases/v0.1.0-rc.2.md`。
  **历史结论（rc.1 发布说明原文，已部分失效）**：「LLM 工具层与动作系统已整体拆除」
  指的是当时的 **LLM 工具调用 + core 动作仲裁通道**，**不含**渲染面动作预设。
  「LLM 调用工具」至今仍不存在，**不要**以它为前提取代码设计；但「动作系统」已随
  动作包 v0 + 渲染面 `preset` 协议部分回归——**现状见下文 §「动作与表演的现行状态
  （2026-09 实测）」**，写动作 / 表演相关代码或文档前先读那一节。
- `Live2D-Ai-pc/`（Python）已归档（tag `py-legacy`）；`Live2D-Ai-Android/` 已归档
  （`android-archive` 分支，见 `ANDROID_ARCHIVE_POINTER.md`）。
  **2026-09-11 起这两个归档的远端 ref 已删除，只在维护者本地保留**——公开历史重新起算
  （`main` 成为单个根提交），见 `docs/releases/v0.1.0-rc.1.md`「历史重置」。
- 增强能力通过 **Mod 边界**隔离：`live2d-ai-mod-system` trait 注册中心，
  **现行 5 个注册 Mod**（external-input / persona / voice-input / memory / director）
  为 workspace crate；
  **缺省只启用 `external-input`**（直播弹幕/礼物经 sidecar 注入，见
  `cli_entry::default_mods_manifest`），其余四个缺省停用（`memory` 会写
  `persona.system_prompt`，必须由用户明确打开；`director` 是**决策 + 按句 cue**骨架，异步第二路 LLM 默认关）。
  **`local-llm` 已于 `0.2.0-rc.1` 废除启动**（移出注册表；crate 已于
  **2026-10-01 W2-A/D1 删除**，只存在于 tag `checkpoint/pre-d1-dormant`，**禁止挂回**）。
  **`wallpaper` / `pet-desktop` 已于产品级加强波次封存（ARCHIVED），并于
  2026-10-01 W2-A/D1 一并删除**（crate 已不在 workspace，只存在于同一 tag，**禁止挂回**；
  见 `docs/architecture/ARCHIVED-mods.md`——内含 64 条退出测试全名与恢复步骤）。
  用户手动的舞台/壳背景能力（`DisplayPrefs`）**保留**，与被删的 wallpaper Mod 是两回事。
  **Mod 契约 / 加新 Mod 勾选表 / 正式版 Rust-C 规则**见
  `docs/architecture/mod-product-chain.md`（与旧 `plugin-sdk.md` 冲突时以它为准）；
  **许可与分发边界**见 `docs/architecture/mod-community-license.md`。
  **rc.2 那个「动作序列唯一驱动方」的 director 已于 `0.1.0-rc.2` 删除**
  （归档分支 `archive/action-layer-p6` **已不存在**——2026-10-06 复核：`git branch --list` 空、
  `git tag --list` 无、`git ls-remote --heads origin` 只回 `main` 与 `mainline/1-core-baseline`、
  四个历史 bundle 的 heads 里也没有；被删内容从 **`main` 自己的历史**取回：
  `git show 98469df^:crates/live2d-ai-mod-director/src/lib.rs` = 395 行、
  `git show ef9f428^:crates/l2d-wasm-demo/src/web/surface.rs` = 1721 行）——静态注册的工厂数由 `main.rs` 的
  `mod_count_is_five` 断言守住（产品级加强波次起恰为 **5**），**core 的 `action_tx` /
  `RootEvent::Action` 驱动通道不要再挂回去**（渲染面 `preset` 协议 + director 的 `action_cue`
  是现行路径，不在此禁令内，见 §「动作与表演的现行状态（2026-09 实测）」）。
  Wave 3（2026-09-14）新增的 `live2d-ai-mod-director` 是**同名不同职责**的
  决策骨架（`docs/architecture/director-mod-v0.md`），已注册但缺省停用。
  **2026-09-16（P1-3）更新**：它订阅新的 `ModEventTopic::SentenceReady`，规则推导
  常开兜底（priority 10），可按配置启用**异步第二路 LLM**（priority 40，**默认关**）产出按句 plan；
  该二路的**真实 HTTP 客户端已于 P1-4（2026-09-19）接线**——
  `crates/live2d-ai-mod-director/src/staging_http.rs`（reqwest blocking + rustls，
  `POST {staging_base_url}/chat/completions` 非流式、失败静默回落规则层；`staging_*` 独立配置，
  不读 `[llm]`）。产出经 host 能力 `ModServices.cues`
  广播 WS `action_cue`（缺省忽略 = 兼容）；`action_tx` / `apply_settings` 的
  **零调用红线未放松**。
- **TTS 不是 Mod**（2026-09-11 用户裁决）：语音合成是**核心链路**
  （LLM → TTS → 口型），端点唯一权威来源是 `live2d-ai.toml` 的 `[tts]` 段。
  见 `docs/architecture/tts-is-core.md`。

## 分层与技术栈（**双主导**）

本项目的「主导」是**两层**，不是一个语言：

| 层 | 语言 / 位置 | 职责 | 门禁 |
| --- | --- | --- | --- |
| **核心层（Rust）** | Rust workspace `crates/*` | 渲染（`l2d`）、状态机（`live2d-ai-core`）、LLM/TTS 网络与配置（`live2d-ai-runtime`）、桌面壳与 Web API（`live2d-ai-desktop`）、wasm 渲染面（`l2d-wasm-demo`）、工程工具（`xtask`） | `cargo test/fmt/clippy` + `rust-ratio` ≥95% |
| **前端/接口层（Flutter Web）** | `shell/flutter/`（Dart），由 Rust 在 `/app/` 同源托管 | 用户界面：Live2D 舞台、聊天、设置、外观/口型控制 | `flutter analyze` + `flutter test`（**豁免 `rust-ratio`**） |

- **唯一实现主线 = 核心层 Rust workspace**（`crates/`）：`l2d` / `live2d-ai-core` /
  `live2d-ai-runtime` / `live2d-ai-desktop` / `l2d-wasm-demo` / `xtask` / `live2d-ai-mod-system`。
- **前端以 Flutter Web 为准**（`shell/flutter/`），**且只有一个入口**：
  `GET /` 与 `GET /index.html` **302 到 `/app/`**。
  旧的**原生 JS 前端**（`web_api/index.html` + `app.js` + `chat.js` + `style.css`
  + `index_html.rs`）已于 **2026-09-11 删除**——它曾在 `/` 提供一套**与 `/app`
  不同**的界面：同一个服务两个产品，打开哪个 URL 看到哪个，是明确的坑。
  删除后 `rust-ratio` 反而升到 ~98%（`js` 不再计入分母）。新功能一律做在 Flutter 侧。

### 前端层豁免的理由（显式记录，勿当成遗漏）

1. **Flutter Web 无法用 `dart:ffi` 渲染**，业界统一做法是「隔离的 Web 渲染面 +
   版本化消息桥」——本项目即「Flutter 壳 + 复用 Rust 的 `/render`（wasm）」，
   渲染核心仍是 Rust，Dart 只承担界面与调度。
2. `xtask` 的 `rust-ratio` 统计扩展名（`rs/wgsl/py/ts/js/c/cc/cpp/h/hpp`）**不含 `dart`**，
   因此 Dart 不计入该门禁——这是**刻意的豁免**，不是统计漏洞。
   `xtask` 报告会**单独打印 Dart 行数**以保证可见性（可见但不设 Rust 占比门槛）。
3. 前端层的质量门槛改由 Flutter 自己的工具链承担：
   `flutter analyze`（静态检查）+ `flutter test`（含音频调度等纯逻辑回归）。
4. 治理红线仍然适用：前端**不得**绕过 core 仲裁、不得直接持有密钥、
   不得引入设备端推理运行时（LLM/TTS 仍走统一 OpenAI 兼容端点）。

## 工作区与点火纪律（WSL2 ↔ Windows，2026-09-12 定）

核心开发**只在 WSL2**（**唯一工作树 `/home/skystar/Live2D-Ai`**，分支 `main`；
2026-10-06 已把 `Live2D-Ai-fe` 这个 linked worktree 合并回本树并删除）；Windows 侧只承担**浏览器肉眼验收**。
两边不做第二套真相，也不互相复制产物。

| 角色 | 职责 |
| --- | --- |
| **WSL2**（唯一开发环境 + **唯一进程宿主**） | `cargo` 全部构建与测试；Flutter SDK 在 `~/flutter`；**服务进程一律在这里起**（`./scripts/ignite.sh`，默认端口 18080）；前端构建 `flutter build web --release --base-href /app/ --no-web-resources-cdn` |
| **Windows** | 只做两件事：开浏览器点 `http://127.0.0.1:18080/app/`；把看/听的结论写回。**不跑二进制、不编 Flutter** |
| **产物** | 单一真源 = WSL 的 `shell/flutter/build/web`。`LIVE2D_AI_FLUTTER_WEB_DIR` 一律写 **WSL 路径**；**不要**引入 Windows UNC 路径写法（`\\wsl.localhost\…`）——只有「哪天真的在 Windows 上跑二进制」才需要，那不在本计划内 |

- `scripts/ignite.sh` 会把 `LIVE2D_AI_FLUTTER_WEB_DIR` 锚定成仓库内绝对路径，
  所以「cwd 不对 → `/app/` 503」不该再出现；503 响应体现在会**列出实际找过的每个路径**。
- **点火体检**：服务跑起来后另开一个终端跑 `./scripts/ignite.sh --check`，断言
  `GET /` = 302 → `/app/`、`GET /app/` = 200、`index.html` 与 `main.dart.js`
  **不含 `gstatic.com/flutter-canvaskit`**（断网红线）。这三条只有真起过一次服务才验得到，
  单元测试覆盖不了托管层与产物内容。

详细分工与验收：`docs/legacy/plans/PLAN-rc2-second-baseline-2026-09-12.md` §3。

## 开发约定

### 门禁：本地必跑 vs CI 必跑（2026-09-13 rc.3 对齐）

**一张表，一套真相**：本地提交前跑的命令，必须能在 CI 清单里逐条找到；CI 里没有的
检查不进「提交前必跑」清单。对应 workflow 在 `.github/workflows/`。

| 检查 | 本地命令 | CI（workflow → job） |
| --- | --- | --- |
| Rust 测试（lib/bin/tests/examples） | `cargo test --workspace --all-targets` | `pr-checks.yml` / `nightly.yml` → `rust-*-test` |
| Rust doc 测试（rustdoc 示例） | `cargo test --doc --workspace` | 同上（`Test doc examples`） |
| 格式 | `cargo fmt --all -- --check` | 同上 |
| Clippy（全目标、0 warning） | `cargo clippy --workspace --all-targets -- -D warnings` | 同上 |
| Rust 占比 ≥95% | `cargo run -p xtask -- rust-ratio` | 同上（`Rust ratio (>= 95%)`） |
| MSRV 1.92 可编译 | 手动（可选） | `pr-checks.yml` → `msrv-check`（仅 `cargo check`） |
| 前端静态检查 + 测试 | `cd shell/flutter && flutter analyze && flutter test` | `flutter-checks.yml`（`paths: shell/flutter/**`） |
| 仓库根历史资产测试 | `python3 -m pytest tests/ -q` | `pr-checks.yml` → `root-py-tests` |
| 端到端核心链（需活端点） | `python3 scripts/verify_core_chain.py --timeout 300` | `nightly.yml` → `core-chain-verify`（**仅当**配置仓库变量 `LIVE2D_AI_VERIFY_BASE_URL`） |
| wasm 渲染面可编译 | `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` | **暂无**（本地手动；wasm 改动必须 rebuild + 肉眼） |
| 前端产物级离线门禁（构建 + 产物断言） | `cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn && cd ../.. && ./scripts/ignite.sh --check-dir shell/flutter/build/web` | `flutter-checks.yml` → `flutter-web-offline-artifacts` |
| 前端真服务离线体检（托管层；与 `ignite.sh --check` 同一份判据） | `./scripts/ignite.sh --check`（对已启动服务；`scripts/ignition-precheck.sh` 的 A 段同源） | `nightly.yml` → `web-offline-serve-check` |
| 仓库密钥扫描（红线 R：密钥不进仓库） | `python3 scripts/check_public_secrets.py` | `secret-scan.yml` → `public-secret-scan` |

两条纪律：
1. 端到端探针**只在有活端点时跑**，缺配置就明确跳过——绝不伪造绿灯
   （「天天红」比没有这条更坏，与「自检说谎」是同一条教训）；
2. wasm 那条暂时只有本地门禁——**已知缺口**，写在这里而不是假装它被 CI 守着。

### 核心层（Rust）

- **门禁**（提交前必须）：`cargo test --workspace --all-targets` + `cargo test --doc --workspace`
  + `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`
  + `cargo run -p xtask -- rust-ratio`（门槛 95%）。
- **源码 ≤500 行**（豁免 ≤1000 需头注理由）；测试文件 ≤800 行。
- **密钥安全**：密钥不进 GET/日志/WS/导出；loopback-only；mutating 需 `application/json`。
- **密钥真源 = `.env`**（2026-09-12 rc.2 用户口径）：查找优先级 **`.env` 快照 >
  进程环境 > 无（不鉴权）**；`live2d-ai.toml` 只持 `api_key_env`（变量**名**）。
  - 读取**只能**走 `live2d_ai_runtime::secrets::lookup`——直接 `std::env::var`
    会绕过 `.env`，于是「界面上刚写了 key、链路还说没配置」。
  - 写入走 **`PUT /api/v1/env`**（`{"key","value"}`；空值 = 清除）：原子写 +
    `0600` + **就地改行**（保留注释，与 `merge_into_toml` 同一条教训）；
    `GET /api/v1/env` **只回键名 + 是否已设置，永不回值**；写操作日志不记 body。
  - **热重载**：写入后刷新快照 + `supervisor.reload()`；`.env` 也在
    `file_watcher` 的监视集里（外部手改同样即时生效——与「配置快照必须与磁盘
    一致」是同一条纪律）。
- **Mod 边界**：Mod 必须通过 `live2d-ai-mod-system` 接入，不得绕过 core 仲裁。

### 推理模型的「思考」（2026-09-13 定）

上游可能是**推理模型**（实测 `deepseek-flash`）：除正文外还发 `reasoning_content`。

- 解析层必须**单列**一类事件（`LlmEvent::ReasoningDelta`），**不得**并进正文；
- **思考不进句子装配器、不进 TTS**——否则模型会把内心独白念出来（回归：
  `llm::reasoning_tests::reasoning_never_reaches_the_sentence_assembler`）；
- WS 用**独立帧类型** `reasoning_delta`（不复用 `text_delta`：后者的语义是
  「与音频同拍的正文」，混用会把思考追加进气泡正文）；
- **思考与正文共用 `max_tokens`**：所以默认上限是 **4096** 而不是 512。上限过小的
  表现不是「回复短一点」，而是**正文被挤成半句 → 切不出完整句 → 一个字都不上屏**
  （用户看到「模型没有返回」）。改这个默认值前先重跑 `settings_tests` 里那段实测记录；
- 前端思考**不落盘**（比正文长 5–20 倍，写进 localStorage 会膨胀一个数量级）；
  刷新后旧气泡不再有思考，这是刻意取舍。

### 动作与表演的归属（休眠台账，2026-09-12 rc.2 定）

**（历史结论，2026-09-12 rc.2）**：当时写的「**动作在产品路径上不存在**」是那一版的裁决
（`docs/architecture/core-chain-baseline.md` §3.1），**已被后续动作包 v0 + 渲染面 `preset`
协议部分推翻**——不要再把它当成无条件事实，现状见下文 §「动作与表演的现行状态（2026-09 实测）」。
下表仍然有效，它回答的是**休眠对象**的「谁休眠、为什么、谁能唤醒」：

| 对象 | 状态 | 谁能唤醒 |
| --- | --- | --- |
| `live2d-ai-core` 的 `action/` + `performance/` | **休眠保留**（类型 / reducer / capability gate 原样） | 只有先重新论证 `core-chain-baseline.md` §3.2 的三条理由 + `lib.rs` 第 4/6 条不变量之后 |
| `ModServices.action_tx`（仍属 Mod API 契约） | **休眠**：host 注入固定 sender，请求只留一行 debug 日志并返回 `false` | 同上；**不得**在 `mod_registry.rs` 里私自接回真通道 |
| `live2d-ai-mod-director`（动作序列的唯一驱动方） | **已删除** | 归档分支 `archive/action-layer-p6` **已不存在**（2026-10-06 复核，见上文）；取回：`git show 98469df^:crates/live2d-ai-mod-director/src/lib.rs` |
| `live2d-ai-mod-director`（2026-09-14 重加的**决策/按句 cue**版） | **已接线**（不再休眠）：规则层产 WS `action_cue`（**唯一驱动舞台**，中性轮给 `preset_id=="none"` 撤销哨兵），经前端转发到渲染面 `preset` 协议**会驱动动作**；`latest.preset_id` **仅面板只读**（前端拉取驱动已退役，D12）；只是**不经 core reducer**（`action_tx` 仍休眠） | 见下文 §「动作与表演的现行状态（2026-09 实测）」③ |
| 渲染面（`l2d-wasm-demo`）的编舞残件 | **已删除**（`action-state` 接收器 + `surface.rs` 编舞） | 归档分支 `archive/action-layer-p6` 已不存在（同上）；取回：`git show ef9f428^:crates/l2d-wasm-demo/src/web/surface.rs`（1721 行）；恢复必须 wasm 重建 + 肉眼验收 |
| **待机生命体征**（`IdleState` 呼吸/眨眼/微表情） | **必须保留**——与动作系统是两套机制，只共用 override 层 | 无（它一直在产品里，删动作时**绝不要**连带删它） |

为什么不能「顺手接回去」：一个 `live2d_perform_action` 工具 + 空 system prompt 会让模型
**只调工具、不说话**，产出「正常完成但一个字都没有」的回合（§3.1 当场复现过）。
护栏是两条断言：`main.rs::mod_count_is_five`（工厂数不得因动作 Mod 增加）与
`mod_registry::tests::action_request_is_dormant_not_delivered`（动作请求必须不被接受）。

### 动作与表演的**现行状态**（2026-09 实测）

> 本节是 2026-09-21 调研（`docs/plans/RESEARCH-actions-director-audit-2026-09-21.md`）在分支
> `mod/l1-product` 上的实测结论。**上节「休眠台账」只回答「谁休眠、为什么」，不要再把它
> 读成「动作 / 表演不存在」。** 逐条如下，每条给出真源文件：

| # | 现行事实 | 真源（树上可查） |
| --- | --- | --- |
| ① | **动作包（表情 / 手势）经 `preset` 帧直接驱动渲染面参数，不经 core reducer**：共 9 条包 `smile` / `unhappy` / `surprised` / `nod` / `shake` / `look_left` / `look_right` / `tilt_left` / `tilt_right`（另有 `none` 撤销哨兵）；渲染面 `PresetRuntime` 收 v1 协议 `preset` 消息 → `PresetCommand::Apply / Revoke / Ignore`，按 `PresetSlot::Face` / `Gesture` 两槽写参数，到点按槽撤销 | `assets/actions/presets.json`、`assets/actions/preset_labels.json`、`crates/l2d-wasm-demo/src/preset/mod.rs`（`handle` / `apply_frame`）、`crates/l2d-wasm-demo/src/preset/table.rs`（解析期红线）、`crates/l2d-wasm-demo/src/main.rs`（`"preset" =>` 分支）、`shell/flutter/lib/live2d/live2d_stage.dart`（`applyPreset`） |
| ② | **`[action]` 幅度倍率存在**：`head_scale=0.75` / `body_scale=1.4` / `expression_scale=1.0`，运行期参与 `最终值 = 表值 × 包络 × 通道倍率` 并按通道钳位（`ParamAngle*` ≤30 / `ParamBodyAngle*` ≤10 / 五官 ≤4）；Flutter「外观与互动」有滑条 | `live2d-ai.toml` 的 `[action]` 段、`live2d-ai.toml.example` 的 `[action]` 段、`crates/live2d-ai-runtime/src/settings.rs`（`ActionSettings`）、`crates/l2d-wasm-demo/src/preset/scales.rs`（`PresetScales::from_parts`）、`crates/l2d-wasm-demo/src/main.rs`（`set_scales`） |
| ③ | **director Mod 已接线，唯一驱动是 `action_cue`**：规则层按用户输入判 emotion / intent → 选包；`cues` 经 host `ModServices.cues` 广播 WS `action_cue`（`cues[].preset_id=="none"` = 该句音频开始时撤销两槽；中性轮也给这条撤销哨兵，D10）。`latest.preset_id` 经 `GET /api/v1/mods/director/state` **仅供面板只读**——前端拉取它驱动舞台的通道**已退役**（D12；Dart `_applyDirectorPreset` / `DirectorPresetGate` 已删，驱动走 `_applyDirectorCueForSeq`）。**本工作树**运行配置 `mods.json` 里 `director.enabled = true`（该文件未入库、属本机配置，见 `.gitignore`）；**编译期** `cli_entry::default_mods_manifest` 仍只收录 `external-input`——「缺省启用」指前者，两者是不同的真源，不要混写 | `crates/live2d-ai-mod-director/src/lib.rs`（`latest.preset_id`、`emit_cues`）、`crates/live2d-ai-mod-director/src/presets.rs`（`PRESET_IDS` 单一真源）、`mods.json`、`crates/live2d-ai-desktop/src/web_api/cli_entry.rs`（`default_mods_manifest`）、`crates/live2d-ai-desktop/src/web_api/mods_routes.rs`（`{id}/state`）、`shell/flutter/lib/main.dart`（`_applyDirectorCueForSeq`） |
| ④ | **`[performance]` 段存在但缺省关**：`enabled = false`；打开后是主链（引擎内）的**第二个 LLM 端点**，每轮交回 `{"speak":…,"cues":[…]}`。它与 Mod `staging_*` 是**两个可选提供者、都默认关、职责重叠**；**谁的 `speak` 能力该保留**未定（RESEARCH §3.7 Q1，**不裁决**，此处仅登记待定） | `live2d-ai.toml` 的 `[performance]` 段、`live2d-ai.toml.example` 的 `[performance]` 段、`crates/live2d-ai-runtime/src/performance/`（`client.rs` / `mod.rs` / `plan.rs` / `prompt.rs`）、`crates/live2d-ai-runtime/src/conversation/engine.rs`（`perf.resolve(...)`）、`docs/architecture/performance-layer-v0.md` |
| ⑤ | **core 的 action / performance 子系统仍无驱动方**：动作包走的是渲染面参数层（①②），**不经 core reducer**；`ModServices.action_tx` 仍是休眠 sender，`supervisor` 侧注入 `RootEvent::Action` 的分支已在 rc.2 删除 | `crates/live2d-ai-core/src/action/`、`crates/live2d-ai-core/src/performance/`、`crates/live2d-ai-core/src/lib.rs`（不变量 4/6）、`crates/live2d-ai-desktop/src/mod_registry.rs`（`action_tx` 休眠 + `action_request_is_dormant_not_delivered`）、`crates/live2d-ai-desktop/src/supervisor.rs`（rc.2 删除注入分支的注释） |

**读法**：①②③ 是**现行产品路径**（动作包 → 倍率 → director），④ 是**存在但未开**，⑤ 是**仍休眠**。
把 ⑤ 的「core 无驱动方」误读成「动作系统整体不存在」，正是本节要修掉的误导（漂移记录见
`docs/plans/RESEARCH-actions-director-audit-2026-09-21.md` §4.1）。

**产品口径（维护者 2026-09-21，含同日 §3.8 追加澄清；本段是裁决文本，改动前必须先拿到维护者新裁决）**：

> 产品 =「**酒馆（类酒馆角色扮演内核）+ Live2D 皮套壳子**」：人设由类酒馆内核稳定，**各功能由现有 mod 矩阵承担**
>   （记忆 / 外部输入 / 语音 / 人设 / 导演都在矩阵里），Live2D 皮套负责**情绪表达**；**导演是一个 AI**、
>   属**产品本体**（不是「辅助用户操控皮套」的工具），**不做架构搬迁**。
>   情绪/表演决策的**输入是用户输入**（用户说了什么 → 皮套怎么反应），**不是角色回复**。
>   后者属于『角色有自己心理』的产品形态，不是本项目底座要表达的东西——**不要**照别的项目（如 N.E.K.O 分析角色回复）改回去。
>
> ⚠ 同日 §3.6 曾把「导演/表演属场景能力、不属底座能力（主链不该有第二 LLM）」写成**推论**，该推论
>   **已被维护者明确否定**（见 `docs/plans/RESEARCH-actions-director-audit-2026-09-21.md` §3.7 / §3.8 与
>   `docs/plans/ORCHESTRATOR-PROMPT-actions-performance-round.md` §8）——**不得再作为口径或架构建议引用**。
>   本段改写由编排者在 W0 收口时执行，理由见本轮交付报告「未做 / 需维护者过目」一节。

### 导演可观测（2026-09-26 阶段5 定）

设置 → 开发工具（`dev_mode=true` 才渲染）里的「**导演可观测**」四栏是**零后端**的只读开发面：
A 决策参数读 `GET /api/v1/mods/director/state`；B 事件流 = 前端已收 WS 帧 + 渲染面 ack（复用 `live2d/render_events.dart`，环形缓冲上限 200 + 按类型过滤）；
C 传参对照并排「请求（前端 `preset` 帧）」与「生效（渲染面 ack 最终值 + `clamped`/`degraded`/`reason`）」；
D 送 TTS 文本左列 = 前端已收 `text_delta`（净化文本）+ 每句字符数，右列 = `/api/v1/logs` 的 `sentence_ready` 行（**来源=日志端点，不承诺逐句精确匹配**），`dev_mode=false` 时整块不在语义树。
**红线**：不新增任何 WS 帧（`dev_tts_clean` 已由 D41 撤销）、观测缓冲**不落盘**（不写 `localStorage` / `SharedPreferences`），D 栏不得为取正文而新增后端日志。

### 每皮套动作幅度（2026-09-26 阶段5 定，D40）

动作幅度倍率的**用户旋钮住 `live2d-ai.toml`**：全局 `[action]`（`head_scale`/`body_scale`/`expression_scale`）之外，可选 `[action.models.<model_id>]`，三键**各自可选、各自回落全局**，生效值 = 本模型覆盖 > 全局（逐键）；
量程沿用 `[0.2, 2.2]`，写回走 `merge_into_toml` 的**就地改值**（保留注释）+ 热重载；`assets/actions/field_map.json` / `crates/l2d-wasm-demo/src/preset/scales.rs` 仍只管**每通道峰值与方向**，不承担用户旋钮。**渲染面零改动**（前端按 `active_model_id` 算好有效值后经既有 `ActionScalesSyncer` 下发）。

### 原生第二壳的归属（休眠台账 → **已移出构建**，2026-10-01 W2-B/D1 定）

**主路径是 `--web`**：Rust 服务 + Flutter Web `/app/`（`./scripts/ignite.sh` 点火）。

rc.3（2026-09-13）曾裁「**不 feature-gate**，休眠保留」；**2026-10-01 维护者改判**：
去臃肿（D1）走「**物理移出**」——原生壳岛整体删除（crate 只存在于 tag
`checkpoint/pre-d1-dormant`），理由：`code_stats` / `rust-ratio` 按**文件系统**统计，
`[workspace] exclude` 不会让任何体量度量下降；feature-gate 又降不了
`live2d-ai-desktop` 的依赖计数（`optional = true` 也计入 `[dependencies]`）。

| 对象 | 状态（2026-10-01 起） | 移出前位置 | 谁能唤醒 |
|---|---|---|---|
| **Web 主链**（`--web` + Flutter `/app/`） | **主线** | `web_api/` + `shell/flutter/` | 不适用（它就是主线） |
| egui 原生壳（窗口 / 设置面 / 托盘 / 桌宠穿透） | **已删除**（物理移出构建） | `src/app/`、`src/tray/`、`src/backend/`、`src/adapter/`、`src/platform/window.rs`、`src/user_event.rs` | 单独立项 + 先论证「谁来维护第二个 UI 壳」+ 按 `ARCHIVED-native-shell.md` §6 恢复 |
| `--chat` 终端壳 | **已删除** | `src/repl.rs` + `cli` 的 chat 分支 | 同上 |
| `--window-smoke` / `--model-smoke` / `--benchmark` | **已删除**（工具随之移出） | `src/model_smoke/`、`src/benchmark/` | 同上；恢复后 `--benchmark` 仍**明令禁止**成为生产默认 |

**移出台账（逐条证据）**：`docs/architecture/ARCHIVED-native-shell.md`
——31 文件 / 8,954 行、退出测试 **111** 条全名（100 条在被删文件里 + 11 条在保留文件里）、
新增 3 条（净 −108）、五处主链触点（`app_event.rs` ×3 · `ws/events.rs` ×1 · `supervisor.rs` 测试样本 ×1）、
连带对象处置与恢复步骤都在那份文件里。**对外 WS 帧字段名零改动**（`AppEvent::Tray` 本就不投影）。

两条红线（**不变**）：
1. **默认文档入口只推销 `--web` / `scripts/ignite.sh`**；
2. **不得**再引入第二个 UI 壳/重复的设置面板——那是「第二个产品」，属独立立项；
   本次恢复条件见 `ARCHIVED-native-shell.md` §6。

### 前端层（Flutter）

- **门禁**（改动 `shell/flutter/**` 时必须）：
  `cd shell/flutter && flutter analyze && flutter test`。
- 前端产物**不入库**：`shell/flutter/build/`、`.dart_tool/` 已被嵌套 `.gitignore` 排除；
  `xtask` 亦按目录名排除 `build`（否则 `main.dart.js` 会打穿占比口径）。
- 改动前端后**必须重新构建**才生效：
  `flutter build web --release --base-href /app/ --no-web-resources-cdn`
  （静态文件由服务端按请求读盘，故前端改动只需重建、不必重启后端）。
  **`--no-web-resources-cdn` 不是可选项**：缺省构建把 CanvasKit 指向
  `https://www.gstatic.com/flutter-canvaskit/<engineRevision>/`，
  而 `build/web/canvaskit/` 那份本地副本不被引用——**断网即白屏**。
  本项目是本地优先的桌宠，不允许依赖 Google CDN（`scripts/ignite.sh` 会探测并告警）。
- **中文字体必须自托管**（`assets/fonts/`，Noto Sans SC 子集，OFL-1.1）。
  Flutter Web 的 CanvasKit **取不到设备字体**，`fontFamilyFallback` 也不会命中系统字体。
  **运行期字体回落已于 2026-10-06 结构性离线化**：自定义
  `web/flutter_bootstrap.js` 用现代 `load({config})` → `initializeEngine(config)` API 把
  `fontFallbackBaseUrl` 设成**同源相对路径** `font-fallback/`，并按预算镜像 5 个整族
  （`web/font-fallback/`，21 文件 / 2.69 MiB）；缺字既不跨源也不会白屏，
  **离线时子集外字形显示为豆腐块**是**明确接受的取舍**。设计与实测证据见
  `docs/architecture/font-fallback-offline.md`；审计用 `scripts/font_fallback_mirror.sh --check`。
  不要删掉 `ThemeData.fontFamily`，也不要改用「系统字体回落」；回归在
  `test/theme_test.dart`（含 CJK 本地化后字体不被覆盖回 Roboto 的守卫）。
- **界面文案里不得出现子集外的字符**（2026-09-11 定，真机点火抓到的缺陷）：
  真机上实测到 `GET https://fonts.gstatic.com/s/notosanssc/v37/…woff2 [200]`
  ——因为流式光标用了 `▍`(U+258D)、macOS 前缀用了 `⌘`(U+2318)，两者都不在子集里。
  **门禁**：`test/font_subset_test.dart`（扫 `lib/**/*.dart` 字符串字面量 vs
  `assets/fonts/*.ranges.txt`；后者由 `scripts/font_subset_ranges.py` 生成，头部记录
  字体字节数 + FNV-1a——**换字体必须重新生成，否则门禁按哈希不符判红**）。
  修法是二选一：**改成不依赖字形的实现**（流式光标现在画竖条、`⌘` 改成 ASCII `Cmd`），
  或把字符加进子集后重新生成覆盖表。
- **协议约定**：前端与渲染面之间只走**版本化消息**（`{version,type,payload}`，见
  `l2d-wasm-demo` 的 v1 协议）；新增字段必须向后兼容（缺省即默认值）。
- **压在舞台 iframe 之上的可交互控件必须套 `StagePointerInterceptor`**
  （`lib/live2d/`，2026-09-11 定）：Flutter Web 的两张画布都是 `pointer-events: none`，
  真正接指针的是 iframe 那个 DOM 元素、且它在画布之上——指针落在舞台区域会进
  iframe 自己的文档，**父页收不到**。漏套不报错，只是**「看得见、点不着、也滑不动」**
  （设置面板、断线横幅、缩放角标、错误重试全中过），回归在
  `test/stage_pointer_interceptor_test.dart`。
- **链路错误必须走 `AppEvent::Error` + `tracing` + WS `error` 帧**（2026-09-11 定）：
  一次 LLM/TTS/解码失败要同时出现在**后端日志**（`tracing::error!`，带
  `code=` 结构化字段）与**前端**（WS `error` 帧，`{code,stage,message,hint,epoch,fatal}`），
  且两处的 `code` 是**同一个字符串**（用户要拿界面上的码去日志里搜）。
  错误码由 `live2d-ai-runtime::conversation::ErrorKind::code()` 给出
  （`<stage>_<suffix>`，上游状态码进码：`llm_upstream_401`），`hint()` 给一句
  可执行处置。**禁止**新增只 `println!` 的错误路径（不进文件 sink = 排障时一片空白），
  **禁止**让前端从显示文案里猜错误类型（文案会改，码是契约）。
- **请求级日志在 `web_api/dispatch.rs`**（≥500 `error` / ≥400 `warn` /
  mutating 成功 `info` / 其余 `debug`）；**不记录请求体**（含用户提示词等内容）。
- **配置快照必须与磁盘一致**：`file_watcher` 的外部修改路径除了
  `supervisor.reload()` 还要 `StatusContext::refresh_from_disk`——否则
  `GET /api/v1/settings` 回旧值，且下一次界面「保存」会把旧快照整份写回、
  **静默覆盖用户手改的配置**。PATCH 路径与它共用同一个函数。
- **不得**在 Flutter 侧复制核心逻辑（状态机、动作仲裁、LLM/TTS 协议）——
  那些属于 Rust 核心层，前端只做展示与调度。

### 语音输出约定（2026-09-10 用户裁决）

- **一句一单元**：一个句子必须**完整合成后连续播放**，不做句中切分、不做听感上的
  「断断续续」。**延迟可接受，断句不可接受。**
- 因此：分句只按真实句读边界（`。！？…` 等）切分；不允许按字符位置硬切。
- 播放侧允许为「整句完整性」引入预缓冲；宁可晚开口，不可中途卡顿或重复。
- **默认出声**（2026-09-10 用户裁决，v0.4.3 起）：出厂即有声——**「不出声」必须显式
  要求**，不能是默认状态（否则用户会以为 TTS 坏了）。要安静有两条路：用户按 UI 静音，
  或服务端 `LIVE2D_AI_MUTE_AUDIO=1`。**跑测试 / 无人值守一律用后者**：它在音频源头
  生效（下发零 PCM + 真实 `volume`），绕不过任何客户端缓存或第三方客户端。
- **主音量是客户端输出设置**，与口型**正交**：改音量、按静音都不得影响口型驱动。
  实现随音频路径变过，但这条正交性不变。
  **2026-09-11 变更**：播放路径从 `AudioContext` 逐片调度改为 **`<audio>` 媒体元素
  + Blob(WAV)**（见下条），因此静音/音量不再走 `GainNode` 增益，而是直接落在
  媒体元素上（`element.muted` / `element.volume`）。**不要**再把它折成增益——那样会
  重新绕开浏览器的站点级静音权限路径（`hasAudioContext` 这个名字保留，语义变成
  「媒体元素后端可用」）。
- **音频播放走媒体元素，不走 Web Audio**（2026-09-11 用户裁决，起因是「浏览器禁用
  声音但依旧有声音传出」）：Web Audio 归 Chromium 的 **autoplay 策略**管，而浏览器
  「站点声音 = 阻止」作用在**媒体元素**上——用 `AudioContext` 逐片播放时站点级静音
  很可能拦不住。现在后端把 PCM 经 WS 下发（一句一组分片，带 `start`/`end`/
  `sentence_seq` 句子边界），前端按句封成 **WAV**（= 44 字节头 + 裸 PCM，零依赖）交给
  `<audio>` 播放，口型用 `audio.currentTime` 查服务端 `volume` 包络。
  **本机 TTS 实测出不了 mp3**（`mp3`/`wav` 均 HTTP 400，只有 `pcm` 200）——所以要
  「推文件」就用 WAV，别指望 mp3。
- **句子边界必须由引擎给出**（2026-09-11 定）：WS 音频帧的 `start`/`end` 标的是
  **句**的首/末片（来自引擎 `EngineEvent::AudioChunk` 的 `first_chunk`/`final_chunk`），
  **不得**在发射侧用记账/推断去猜（旧实现按 epoch 推 `start`，实测产出
  「0 个 start、13 个 end」，前端会把一句切成十几段播）。**空末块也要发边界帧**
  （`audio: ""` + `end: true`）——句子样本数是 `audio_chunk_samples` 整数倍时末块
  0 样本，漏掉它就等于让那一句永远没有句尾闸门。
- **「连通性自检」只测连通，不合成**（2026-09-13，起因是实测假失败）：
  `POST /api/v1/settings/test/tts` 打的是 `GET {tts.base_url}/models`（毫秒级），
  **不再真合成一次**。三个理由都实测过：本机合成一次 2.4s 而自检默认只等 3s；
  链路正在合成时再点自检会稳定报 `timeout`（**而同时语音完全正常**）；
  HTTP 循环是单线程的，同步探针会把整个 API 占住它等待的全长。
  **自检与产品链路抢资源的代价是「自检说谎」**——那比没有自检更坏。
  失败分类：401/403 → `auth_failed`；404 → **算通过**（上游回话即可达，`/models`
  并非所有语音实现都提供）+ 一句 `note` 说明；其余 4xx/5xx → `protocol_error`。
  密钥读取一律走 `secrets::lookup`（这里也曾直接读进程环境 → 非 `ignite.sh` 启动时
  出现同一类假失败）。
- **分句器把换行也算句读**，因此会产生**纯空白句**：这类句子**不发 TTS 请求**
  （上游会回 400 "input 为空"，而 TTS 错误是 fatal，会把整轮判失败），走静音句
  路径即可（2026-09-11 修）。

## 变更历史

- **2026-10-06 夜（团队轮：冻结基线独立复核 + 文档漂移收口 + E9 探针 + E2/E4 决策；Lead 统一提交）**：
  维护者「开始派团队处理」后，Lead 把上一轮 40 项未提交改动**冻成 3 个提交**（`65c42f3` Rust 债 /
  `b0b0365` Dart 拆分 / `36937df` 文档），再派 4 名队友：T1 文档漂移收口 · T2 E9 探针稳健性 ·
  T3 E2/E4 决策论证 · T4 冻结基线独立复核；另有 T5 终局复验（见 ⑨）。
  ① **本轮最有价值的单点（T4）：冻结基线并非全绿 —— clippy 真红**：在 `36937df` 上
  `cargo clippy --workspace --all-targets -- -D warnings` **红 2 条** `doc_lazy_continuation`
  （`crates/live2d-ai-desktop/src/web_api/tests_mod.rs:276/277`：第 275 行以「+」起头被 markdown 当成
  新列表项，续行 3 空格不够）；而**只在不带 `--all-targets` 的 clippy 下不报** ⇒ 上一轮交接里那句
  「clippy 0 warning」正是**这个口径（+ 缓存）造成的假绿**。修复 = `cc68f06`（「+ 」→「另有 」，
  **纯注释、语义零变化**），T4 复检 **EXIT=0**。这是「**不许伪造绿灯**」在本轮的实例：**门禁命令
  一个字都不能省**，改注释也算改代码。
  ② **T1 文档漂移收口（`37f94ec`）**：R6 四条 —— 版本口径 `0.2.0`/`rc.7` → 现行 **`0.2.1-rc.1`**；
  `archive/action-layer-p6` 的 10+ 处引用按「**分支已不存在**」改写（本地 / 远端 / 4 个历史 bundle
  的 heads 均无）并给出**等价取回命令**（`git show 98469df^:crates/live2d-ai-mod-director/src/lib.rs` /
  `git show ef9f428^:crates/l2d-wasm-demo/src/web/surface.rs`）；R3 的「已删除 `mainline/1-core-baseline`」
  更正为「本地已删、远端 `b58b223` 仍在」；旧账本路径统一到 `docs/audit/2026-10-05-ledger/`。
  新复核三条 —— `README.zh-CN.md` 的 director「零投递…不驱动动作」过期、
  AGENTS.md 的「真实 HTTP 客户端尚未接线」过期（P1-4 `staging_http.rs` 已接线）、`xtask` 留债栏
  `main.dart 1411` 过期（实际 **657**）。另：台账历史引用加注「**刻意不改**，改台账 = 篡改证据」；
  `docs/plans/AUDIT-PROMPT-whole-repo-2026-10-06.md` 里指向已删除 `-fe` 树的路径整体改判。
  ③ **Dart 棘轮同 commit 收紧（`f5210f3`）**：`RATCHET_DART_800` **2 → 1**（补 E1-b 欠的那一步：
  让计数下降的 commit 必须同 commit 收紧常量，否则「涨回 2 仍绿」）。`cargo test -p xtask` **27 passed**、
  `code-stats --check` Dart 行 **1 ≤ 1 PASS**。
  ④ **E8 文档减量账（T1 实测）**：`docs/**/*.md` 全部 **1,279 份 / 143,270 行**；**不含 `docs/audit/**`
  = 265 份 / 68,122 行**；`docs/audit/` 自身 **1,014 份 / 75,148 行**。结论：`xtask code-stats` 的 docs 行
  **不排除** audit ⇒「归档不减总量」成立；PLAN §5 的「docs ≤45,000」按不含 audit **仍未达标**（超 23,122 行）。
  ⑤ **T2 E9 探针稳健性（`7d53def`）**：`audio-c` 改**三路取证**（DOM 2000→500 ms + 页内自增 id +
  页内媒体记录器 + CDP `Media` 域）⇒ 真实链路两轮 `audio-a/b/c` **全 pass、0 blocked**；四主题像素阈值
  **各自标定**（`themebase` 16 次实测：ON = 基线 + 40%×(红信号−基线)、OFF = 基线 + 12、
  `maxShare` = share + 0.02）；`all` 同一 HEAD / 同一脚本哈希连跑两遍：**各 40 项 pass 36 ·
  manual-only 4 · fail 0 · blocked 0，判定 0 处不同**；`8b` 新鲜度重建后真绿；离线产物门禁
  `ignite.sh --check-dir` 四条 ok；受控自证 `mediaspy`（含阴性对照）。证据
  `docs/verification/evidence-2026-10-06-e9/`。
  ⑥ **T3 E2/E4 决策论证（`43e465e`，两份方案纸）**：**E2** 推荐 **B2**（同库 `part` + extension 外搬，
  可 1 → 0；A 类分解暂不做）——关键事实：**24 键落盘全集零测试覆盖**；**E4** `build/web` 走**真减**
  （删 `*.symbols` + `skwasm*`/`wimp*`，**−20.18 MiB → 29.38 MiB**，`chromium/` 必须保留），
  `dist` 只靠剥 `name` 段**仍超预算 0.115 MiB**，`wasm-opt` 本机 absent ⇒ **不估数**。
  两者**均待维护者裁决**。
  ⑦ **仓库**：Lead 删除 5 条 0 领先本地分支（`feat/frontend-redesign` / `chore/debt-round-2026-10-05` /
  `mainline/1-core-baseline` / `pr-1` / `mod/persona-polish`，删前 `git rev-list --left-right --count`
  右值均 0，用 `git branch -d`）；`git push origin main` **失败**（remote 回
  `No anonymous write access.` / `fatal: Authentication failed`，**无 token**）。
  ⑧ 门禁（**在 `36937df` 上由 T4 自跑**）：cargo **1315 / 0** · doc **3** · fmt clean ·
  rust-ratio **96.1016% PASS** · `code-stats --check` 四条 **PASS**（Dart 行在 `f5210f3` 后为 **1 ≤ 1**）·
  flutter analyze **0** · flutter test **1583**；唯一例外 = ① 的 clippy（`cc68f06` 后 **EXIT=0**）。
  ⑨ **交接**：本轮落盘 [`docs/plans/HANDOFF-2026-10-06-team-round.md`](docs/plans/HANDOFF-2026-10-06-team-round.md)
  （一分钟上手 / 三块交付 / **待维护者裁决四项**：E2 · E4 · R8 · 推送 token / 本轮 6 个坑 / 文档指针）。
  **T5 终局复验**（独立复核）的结果只留指针：
  [`docs/verification/gate-baseline-2026-10-06.md`](docs/verification/gate-baseline-2026-10-06.md) 的 T5 章
  （§8，落盘后以其实际标题为准）——本条**不替它下结论**。
- **2026-10-06 夜（第三轮：台账最后一条 P1 `F-0001-01` + E6/E10 复核；只本地改动，未提交）**：
  ① **F-0001-01（前置路由零日志，P1）**：四条前置路由（chat session / external chat / voice transcript / mods）
  与 WS 前门从前直接 `request.respond + continue`，**绕过 dispatch 的请求级日志** ⇒「没能形成响应的
  失败」零记录（AGENTS.md 明令禁止的「排障时一片空白」）。现在新增 `web_api::mod::respond_and_log`：
  **复用** dispatch 的同一份分级判据（`log_request_outcome` 收成 `pub(super)` + 字符串路由标签——
  前置路由没有 RouteId），**6 处**统一走它（四条 API 前置路由 + WS Origin 拒绝 + WS 方法/路径错）；
  静态资产（`/render` / `/models` / `/app`）与 dispatch 之后那一处**刻意白名单**
  （前者逐文件记 info 会淹掉日志；后者已由 dispatch 记过）。守卫
  `tests_mod::pre_dispatch_responses_go_through_respond_and_log`：判据 = mod.rs **生产段**的调用形状
  （`respond_and_log(` 恰 7 次、直接 `request.respond(` 恰 4 次，**零命中同样判红**），并做了
  「mods 路由改回直发 → 断言变红 → 恢复 → 变绿」的双向自证。
  **台账「仍未关闭的 P1」由此清空（6 → 0）**。
  ② **E6 复核关闭**：`F-0184-01`（「stripCommentsAndStrings 在 test/ 下有 8 份副本」）是 W3-D3 合并
  **之前**的旧状态——现状**只有一个定义点**（`test/support/source_scan.dart`）+ 反复制门禁
  `test/source_scan_test.dart` 20 条全过（含零命中判红）。NEXT-ROUND 该行已勾掉。
  ③ **E10 复核**：`scripts/font_fallback_mirror.sh --check` **PASS**（清单 == 磁盘 == 引擎表全集，
  21 文件 / 2 815 292 B，逐文件 sha256/bytes 相符）。红 = 升级 Flutter 时该重跑生成脚本的信号，不是 bug。
  ④ 门禁（本树实测）：cargo **1315 / 0** · doc 3 · fmt clean · clippy **0 warning** ·
  rust-ratio **96.1016% PASS** · `code-stats --check` 四条 **PASS**（44/44 · 0/0 · 1/2 · 22/22）·
  flutter analyze 0 · flutter test **1583**。
  ⑤ **交接**：本轮收尾落盘 [`docs/plans/HANDOFF-2026-10-06-e1-e5-debt-round.md`](docs/plans/HANDOFF-2026-10-06-e1-e5-debt-round.md)
  （一分钟上手 / 未提交改动清单 / 下一轮顺序 / **待维护者裁决的 E2 与 E4** / 本轮 8 个坑 / 交接检查清单）。
- **2026-10-06 夜（第二轮：E5 台账 P1 清债 —— 假绿灯 / 锁口径 / 数据保留 / 首跑窗口；只本地改动，未提交）**：
  台账 `docs/audit/2026-10-05-ledger/` 的 6 条未关 P1 里清掉 4 条（5 个 ID），每条都做了「回源码复核 + 破坏即红」的双向自证：
  ① **F-0002-02（假绿灯，P1）**：`supervisor_slot.rs` 的 `slot_lifecycle_set_get_take` 只断言「新槽位空 / 空槽位 take=None」——
  测试名承诺的 set / try_get / take 一条没验；毒化测试更毒化的是一个**类型不同、与槽位无关**的新建锁，断言恒真。
  现在：生命周期真走 `set → try_get（同一 handle）→ take → 二次 take=None`；毒化测试先 `is_poisoned()` 钉住前提，
  再验降级语义（读=None、写=no-op、take=None），并新增 `#[cfg(test)] inner_arc()` 让测试能毒化**槽位自己的锁**。
  ② **F-0006-03（锁口径，P1）**：`mods_routes.rs` 有 **5 处** `.expect("mod_registry mutex poisoned")`，而同仓
  `external_routes` / `voice_routes` 对**同一把锁**用 `poisoned.into_inner()`（不 panic）。裸 `expect` 的 panic 落在
  `run_request_loop`（无 `catch_unwind` 的 accept 循环）上 ⇒ 一次 Mod 侧 panic **永久打死 HTTP 面**。现统一为
  `lock_registry(ctx)` 单一定义（5 处改调用），并新增回归「毒化同一把锁后 `handle_mods_list` 仍回 200」。
  ③ **F-0013-01 / F-0644-01（静默丢数据，P1）**：`persist_manifest` 整份从「在册 factory」重建 ⇒ 用户按文档
  （`docs/external-input.md` 两处明确邀请手改）写进 `mods.json` 的**当前不存在的 Mod id** 与其它顶层键，
  第一次配置保存就被**静默抹掉**。现在以**磁盘当前内容**为基底（读不到 / 坏 JSON 退回构造时那份），只覆写在册 id；
  启动时 `tracing::warn!` 点名未知 id（新增 `raw_manifest` 字段）；回归逐键核对未知 id / 其 config / 顶层键都还在。
  ④ **F-0002-01（首跑窗口，P1）**：`file_watcher` 只在 supervisor 装配成功时才装 ⇒ 第一次运行（`live2d-ai.toml` 尚不存在）
  这个窗口里改 `.env` 要等下次启动才生效且无提示——而首跑窗口**正是**用户建配置、写 key 的那一步。现在 `FileWatcher`
  收 **`SupervisorSource`（惰性现取）** 并**无条件安装**：槽位空时仍刷新设置 / 密钥快照，只把 reload 留到下次启动；
  之后经 PATCH 动态装配也能被同一条监听看见。回归 `on_config_changed_without_supervisor_still_refreshes_snapshots`。
  ⑤ **顺带修掉的棘轮**：本批改动把 `mods_routes.rs` 顶到 **1009 行**（`>1000` 门禁 0→1 FAIL）⇒ 按既有配方把内联测试拆成
  `mods_routes_tests.rs`(360) + `mods_routes_tests_support.rs`(158)（`#[path]` 兄弟文件，先例 `external_routes_tests_*`），
  生产文件回到 508 行，`>500` 计数不变。
  ⑥ 门禁（本树实测）：cargo **1314 / 0**（基线 1311 + 新回归）· doc 3 · fmt clean · clippy **0 warning** ·
  rust-ratio **96.0981% PASS** · `code-stats --check` 四条 **PASS**（44/44 · **0/0** · 1/2 · 22/22）· flutter analyze 0 · flutter test **1583**。
  ⑦ **仍未关**：`F-0001-01`（`web_api/{chat,external,voice,mods}` 四个**前置路由** `respond + continue` 绕过 dispatch 的
  请求级日志 ⇒ 无响应失败零记录）——修法已定（`respond_and_log` 包装 + 四路统一 + 源码守卫），留到下一批。

- **2026-10-06 夜（工作树单一化 + E1：守卫不得静默漏扫 + `main.dart` 拆分；只本地改动，未 bump 版本、未提交）**：
  ① **工作树单一化**：删除 linked worktree `Live2D-Ai-fe`，**唯一工作树 = `/home/skystar/Live2D-Ai`（`main` @ `08338f3`）**；
  31 G `target/` 与前端产物迁入本树（免一次全量编译），运行态（`.env` / `mods.json` / `sessions/` / wasm `dist/`）以 `-fe` 为准并入；
  A/B 审计台账（`AUDIT-REPO/`、`AUDIT-REPO-B/`）按维护者指示**不再运行、仅作参考**，整份保运到树外 `/home/skystar/audit-ref-2026-10-06/`；
  旧分支 `mod/persona-polish` @ `88342ce` 退役。
  ② **E1-a 守卫不得静默漏扫**：`main.dart` 有 4 个 `part`，而 `test/` 里 **20 处**源码扫描守卫只读库文件本身
  （14 处 `File('lib/main.dart')`、`display_prefs_test` 手写拼接漏 3 个 part 中的 2 个、`setting_wiring_test._codeOf('lib/app/app_shell.dart')` 漏 `app_shell_state.dart`、
  `action_scales_wiring_test.readLib` 把路径交给变量）⇒ 全部改走 `test/support/dart_library.dart` 的 `readLibrarySource()`；
  新增门禁 `test/dart_library_guard_test.dart`（3 条：part 库不得被字面量直读 / 不得有「按路径读源码」的辅助函数 / 不得「变量路径 + part 库字面量」；
  目录递归遍历是唯一豁免），**红-绿双向自证**已做；盲区（路径经变量且同文件无该库字面量）如实写进文件头注。
  ③ **E1-b `main.dart` 1417 → 657 行**：新增 5 个 part（`shell_cue_voice_wiring` / `shell_background_scale_wiring` /
  `shell_section_wiring` / `shell_app_root` / `shell_lifecycle_wiring`），**逐字搬迁**（未重写一行业务代码）；
  part 里的 extension 不能调 `setState`（`@protected`）⇒ 新增**唯一**重建桥 `_rebuild()`（14 处调用改桥，头注写明「不要另开第二条」）；
  `initState` / `didUpdateWidget` / `dispose` 只留 `super.*` + 一次委托，本体进 part。Dart `lib >800` **2 → 1**（只剩 `display_prefs.dart` = E2）。
  ④ 门禁（本树实测）：`cargo test --workspace --all-targets` **exit 0** · `flutter analyze` **0 issue** ·
  `flutter test` **1583 通过 / 0 失败**（基线 1580 + 新门禁 3）· `code-stats --check` 四条 **PASS**（`>500` 44/44 · `>1000` 0/0 · Dart `>800` **1/2** · deps 22/22）。

- **2026-10-06（补齐轮：**字体回落结构性离线化** + 结构硬指标达标 + **真实音频链路首次验收** + 安全类 P1 收口 + 审计台账入库）**：
  主线契约（LLM→TTS→口型→Live2D、舞台背景绘制语义、`clean_for_tts`、`[action]`、`stage-clock`、`IdleState`）**一行未改**；
  **仍不升版本号、不发 release**（维护者肉眼清单未勾完，版本号不跑在验收前面）。详见
  `docs/audit/2026-10-06-gaps-round/ROUND-REPORT.md` 与 `docs/plans/NEXT-ROUND-main-2026-10-06.md`。
  ① **字体回落结构性离线化（N1 裁决：不接受「仅子集外字符出网」）**：新增自定义 `shell/flutter/web/flutter_bootstrap.js`，
  用**现代** `initializeEngine({fontFallbackBaseUrl:"font-fallback/"})`（相对路径、换 base-href 不失效；**不用**已 deprecated 的
  `window.flutterConfiguration`）；镜像 5 个**整族** 21 个 woff2（emoji/符号2/符号/音乐/数学，2.69 MiB）到 `web/font-fallback/`，
  附 `MANIFEST.txt`（sha256/bytes）+ `OFL.txt` + `README.md`。工具：`scripts/font_fallback_mirror.sh`（`--check` **不联网**，
  「清单 == 磁盘 == 引擎表全集」，不许半族）与 `scripts/font_offline_check.mjs`（零依赖 CDP + **阳性对照**）。
  **实测（重建产物 + 线上 18080）**：注入 `𠮷`/`🀄`/`𝄞` ⇒ 跨源请求 **0/0**，同源 `notocoloremoji` 200、`notomusic` 200、
  `notosansjp` 404 ⇒ 豆腐块（**明确接受的取舍**；CJK 五族约 11.9 MiB 未镜像，升级路径见下一轮 F4）。
  ② **结构硬指标（D2/N5）**：Rust `src` `.rs >1000` **4 → 0**、`>500` **48 → 44**；Dart `lib >800` **7 → 2**（= PLAN 目标，
  `main.dart` 1411 / `display_prefs.dart` 1169 如实留债并写进 `xtask` 留债栏）。配方沉淀：搬测试出 `src` 必须**每份 <500 行**否则
  `>500` 计数凭空上涨；Dart `part` **类体不能跨 part**，切割线只落顶层类边界（做了逐字节回环对账）。
  ③ **探针判据修复（N4）**：两个根因实测钉死——带 `clip` 的 `captureScreenshot` **稳定回全白**；不带 `captureBeyondViewport`
  的整页帧**间歇全白**（8 次 1 次）。修法：整页 + `captureBeyondViewport` + **全脚本禁止 clip**；「不可判读帧」判 **blocked（环境）**
  而不是产品 fail；背景注入改 **IndexedDB 真字节**（db `live2d-ai`/store `backgrounds`/key=id，id 与 Dart `backgroundFingerprint` 同算法）。
  ④ **真实音频链路首次验收**（上一轮 TTS 停机只能记 blocked）：131 条 audio 帧、`sentence_seq` 从 1 起严格递增、start 1/end 1、
  62400 样本（2.6 s @24 kHz）、`muted=false`、页面 `<audio>` `blob:` + `duration=2.6` + `currentTime` 前进 1.83 s。
  ⑤ **产品语义**：无 code 的「重试」= **重发上一条用户消息**（无上一条 ⇒ 按钮不出现）；**原判「运行态（只读）块在在册 Mod 上不可达」
  被队友回源码证伪**（`showRuntimeState` 缺省 `true`）⇒ 改判为**契约反转**：该块 = **没有专用面板的 Mod 的兜底面**，删掉死旗标 `showRuntimeState`。
  ⑥ **安全 P1（F-0616-01）**：`scripts/deploy_android.sh` 自 **v0.1.0-rc.1 根提交**起就在公开远端，头注/默认值写死**手机锁屏口令**。
  删脚本；`check_public_secrets.py` 补「文字口令 / 纯数字口令 / 中文『密码:』」三类模式（旧四条只认云厂商 key 形状）；台账摘录**脱敏**；
  中文模式收紧到「值必须像凭据」（起因是实测误报）。扫描面 **2022 个 tracked 文件 0 命中**。
  **维护者随后授权并给出令牌 ⇒ 同日执行历史清洗并推送**：`git filter-repo` 从**全部 573 个提交**删除该路径 + 替换两个标识串；
  新旧 `main` 的**树哈希相同**（只动历史）、逐 ref 比对 100 个 ref **0 处意外**、两个 needle 0 命中；随后 force-push `main` 与
  `mainline/1-core-baseline`、重写 6 个远端 tag、新增 `v0.2.0`/`v0.2.1-rc.1`。执行记录
  `docs/audit/2026-10-06-purge/PURGE-RECORD.md`（含独立验证 `PURGE-VERIFY.md`）。**仍残留**：GitHub 托管的
  `refs/pull/1/head` 与旧对象缓存**我们删不掉** ⇒ **轮换手机锁屏口令才是真正的修复**（维护者动作）。同日发布
  **`v0.2.1-rc.1`**（GitHub pre-release，说明见 `docs/releases/v0.2.1-rc.1.md`）。
  ⑦ **仓库治理**：审计台账 952 文件从工作树根未跟踪的 `AUDIT-REPO/` 并入 `docs/audit/2026-10-05-ledger/`（附 README：归档理由、
  已关闭条目、**仍未关闭的 6 条 P1**、已撤回 5 条）；`web-ui-spec-v3.md` 的豁免口径改为现行的「路径前缀 + 处数上限」；三处历史文档补更名注记。
  ⑧ **门禁（终局实测）**：cargo **1311/0** · doc **3** · fmt clean · clippy **0** · rust-ratio **96.0927%** PASS · code-stats PASS
  （**44/44 · 0/0 · 2/2 · 22/22**）· flutter analyze **0 issue** · flutter test **1580** · pytest 22/1skip · 秘密扫描 2022 文件 ok ·
  `ignite.sh --check` **6/6** · `browser_probe.mjs all` **42 项：pass 38 · manual-only 4 · fail 0**（含音频 3/3）。
  ⑨ **本轮自抓的假绿灯**：`browser_probe.mjs all,audio` **静默丢掉 audio**（注释却推荐这么跑，汇总照样印「fail 0」）—— 已修；
  `stage_cancel_test.dart` 用源码字符串**钉住旧的静默 no-op**（已改语义断言）。
- **2026-10-05（0.2.x 债轮：CI 红线门禁 + Rust 依赖与 Mod 密钥接缝 + Flutter 真 bug/假绿灯 + **首次真实浏览器验收** + 仓库治理）**：
  主线口径未变（**不升版本号、不发 release**：维护者肉眼 13 项未做，版本号留到下一轮）。
  ① **CI 三条红线门禁**（审计 F-0048-01 / F-0049-01 / F-0050-01）：新增 `scripts/lib/cdn_probe.sh` **单一真源**
  （清单 + `cdn_judge` + `cdn_scan_http/dir`），`ignite.sh` 与 **同名缺陷的 `ignition-precheck.sh`** 都 source 同一份
  （缺库 = exit 2，不静默跳过）；判据从「裸串 grep」改成**生效路径**（`flutter_bootstrap.js` 的
  `useLocalCanvasKit` / 生效 `canvasKitBaseUrl`）+ **文件缺失判红**；新增 `--check-dir`（不需要服务）；
  CI 侧新增 `flutter-web-offline-artifacts`（PR）、`web-offline-serve-check`（nightly，起真二进制 + 60s 轮询）、
  以及 **`secret-scan.yml`**（另有：`check_public_secrets.py` 修掉与事实相反的 docstring、扫描面 292 → **937 文件**、
  覆盖 `shell/` 与 `docs/`）。**对账本的一处如实纠正**：无标志坏构建里 `main.dart.js` **会**命中 1
  ⇒ 账本猜「旧探针很可能仍 0 命中」不成立（结构缺口仍成立）。
  ② **Rust 债**：D4 依赖 **25 → 22**（真删 `futures-util`/`serde_with`/`tokio-util` 三条 direct 边；
  `xtask` 棘轮 **同 commit 收紧到 22**），**口径如实标注**：`tokio-util` 只去直边、仍在依赖树里（经 runtime）；
  **F-0062-01**（保存 Mod 配置抹掉 secret ⇒ 注入端点静默退回不鉴权）修为宿主侧**按键合并 + secret 保留**
  （声明 secret 的键**显式空串=删键**，非 secret 空串照存——与 `PUT /api/v1/env` 的「空=清除」刻意区分），
  回归**遍历 `AVAILABLE_MOD_FACTORIES`** 覆盖全部在册 Mod + 端点级显式清除两条。
  ③ **Flutter 债**：`F-0007-2`（busy「打断并重发」只 stop 不 send —— 实为**两层**：接线没接 `onSend`，
  且 `_send` 读的是已 `clear()` 的输入框）→ 新增纯编排 `interruptAndResend` + `lastUserText()` + `send(echoUser:false)`；
  `F-0034-01`（只改逐图样式时 `DisplayPrefs.sameAs` 早退 ⇒ 静默丢弃）→ 改 `!=` 且 `==`/`hashCode` 恢复对称；
  **D6 假绿灯治理**：令牌/时长豁免**整文件名单 → 路径前缀 + 处数上限**（前缀零命中 = 死豁免判红）、
  已删 crate 的硬编码夹具换成**读 `main.rs` 唯一真源的`registeredModIds()` 真断言**、`_stripComments` ×2 合并
  并**修掉「不认识字符串」这个假绿灯方向的洞**、反复制门禁扩成 5 个定义唯一性表（零命中同样判红）。
  ④ **首次真实浏览器验收跑通**（**推翻历史结论「本机无 Linux Chrome、无法从 WSL 驱动」**）：
  `~/.cache/ms-playwright/chromium-1243`（Chrome for Testing 153）+ CDP + DSH browser MCP；渲染面需
  `--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader`。38 项判定：**pass 29 / fail 3 / blocked 4 / manual-only 2**
  （冻结后 Lead 复跑 fail 6，其中 4 条经决定性实验判定为**探针裁剪截图伪影**、非产品回归）。
  **最有价值的实测**：界面自身文案走查 20 步 **0 次 gstatic**；注入子集外字符（`𠮷`/`🀄`/`𝄞`）**真的出网**
  `fonts.gstatic.com`（notosansjp / notocoloremoji / notomusic）⇒ **已知缺口 #1 有了实测形态与复现**
  （该缺口**已于 2026-10-06 轮结构性关闭**：同源 `fontFallbackBaseUrl = "font-fallback/"` + 5 个整族精选镜像
  ⇒ 注入同一组字符跨源请求 **0**、同源 `notocoloremoji`/`notomusic` 200，见
  `docs/architecture/font-fallback-offline.md`）；
  `useLocalCanvasKit:true` + 40 条请求全同源 ⇒ 主链**不会断网白屏**。
  ⑤ **仓库治理**：worktree 3 → **2**（移除 `-product`）、分支 12 → **11**、死树 `-Ai` 从 **136 脏项**冻结为 0 脏
  （改动 + 未跟踪全部保档到 `/home/skystar/backup-2026-10-05/`，`git apply --reverse --check` 逐字校验），
  回收 **8.7 G**；唯一未被既有 bundle 覆盖的 `archive/full-history-2026-09-11` 已补 bundle（`bundle verify` = complete）。
  ⑥ 门禁（冻结树实测）：cargo **1311/0** · doc 3 · fmt clean · clippy **0 warning** · rust-ratio **96.1382% PASS** ·
  `code-stats --check` 四条 PASS（deps 棘轮 22）· flutter analyze 0 · flutter test **1564** ·
  `ignite.sh --check` **四项 ok**（现扫 4 个产物文件）。
  报告：`docs/audit/2026-10-05-debt-round/ROUND-REPORT-2026-10-05.md`（+ CI/Rust/Flutter/仓库卫生四份分报告）、
  浏览器验收 `docs/verification/v0.2.1-browser-acceptance-2026-10-05.md`、
  下一轮清单 `docs/plans/NEXT-ROUND-main-2026-10-05.md`。

- **2026-09-28（v0.2.0-rc.7，正确性与诚实性：审计主发现 + 4 条假绿灯 + 仓库卫生）**：
  ① **`F-0005-2` 重建放大链**：`AppShell.settingsRevision` + `AppShellState._settingsTick` + `section`
  组成 `_pane()` 缓存键，设置分区不再随每个 `text_delta` 重建（**+10 → 0**，计数断言在
  `test/rebuild_scope_test.dart`）；**只挡住当前分区子树**，外壳 / 设置头部 / `ChatPanel` 仍重建（未做改法 B），幅度未真机 profile。
  ② **4 条假绿灯改可失败断言**：`F-0005-1`（守卫对象改 `ui/stage_host.dart` 并剥注释）、
  `F-0005-3`（空泡真判据 + 修实现 + 3 条例外对照）、`F-0005-6`（行为断言）、`F-0005-7`（真泵 `StatePill` + 对照组）；
  每条都由编排者**亲手破坏实现复现变红**（`docs/releases/v0.2.0-rc.7.md` §4.1）。
  ③ **仓库卫生**：worktree 25 → 3、分支 31 → 12、stash 1 → 0（补丁存 `docs/legacy/`）、未跟踪 0、tag 补 rc.2 / rc.3。
  ④ 门禁：cargo **1457/0** · doc 3 · fmt clean · clippy 0 · rust-ratio **97.3263%** · flutter **1378** · canvaskit gstatic **0/0** · ignite **4/4**。
  发布说明 `docs/releases/v0.2.0-rc.7.md`；分派对账 `docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md`。

- **2026-09-28（v0.2.0-rc.6，背景透传追平 shalldie/vscode-background v3.1.0 的机制子集 + 背景域审计收口）**：
  ① **Stage B**：`imageFit` **四档**（cover/contain/stretch/tile）+ `tileSize`（默认 64 / [16,256]，仅 tile 档）；
  **逐图样式覆盖**（`BackgroundImage.opacity/fit/align`，缺省回落全局）；**全局 `background.enabled`**（迁移默认 true）；
  DEC-1 `slideInterval` 端点夹持（新函数，未动 `_clampInt` 的 scrim 语义）；DEC-5 坏 `dataURL` 收紧到真形态；
  DEC-2 两套轮播分工 + 改名 + 按来源互斥（并补 `stagePlaylist` 守护测试，此前零覆盖）；
  DEC-6 轮播「当前项」改用运行时索引（不持久化）；DEC-7a/b 显隐对齐与空转控件禁用；D1 预览在**任意库大小**可达；
  DEC-3 壳内子区域**裁决不做**（理由落盘 parity §5.2）。
  ② **背景域审计 10 条**：解码**备忘化**（同 dataUrl 串 → 同一 `Uint8List` 实例 ⇒ `MemoryImage` 命中 ImageCache，
  流式期不再每 delta 整图重解码，同时消掉缩略图那条）；偏好变更**不再全量重发 `stage-bg`**（同值不重发）；
  启动**水合不再整体覆盖偏好**（只回填背景域，窗口内改动与导入不再回滚）；删掉占位内存库与假超时兜底；
  批量删除改存**项身份**（不再删错图）；预览跳转真的驱动轮播；删 `onBackgroundIndex` / `onBackgroundJump` 死参数。
  ③ 结构：`appearance_section.dart` **1489 → 709**（背景域抽到 `appearance_background.dart`）。
  门禁：cargo **1457/0** · clippy 0 · rust-ratio **97.3263%** · flutter **1370** · canvaskit gstatic **0/0** · ignite **4/4**。
  发布说明 `docs/releases/v0.2.0-rc.6.md`。

- **2026-09-27（v0.2.0-rc.5，Stage A：资产守护网 + 09-27 前端重设计并入 + 背景 P0）**：
  ① **A1 资产守护网**：`docs/architecture/frontend-asset-inventory.md` + **19 条守护断言**
  （含「`main.dart` 仍存在 `_applyDirectorCueForSeq` 的**调用点**」——旧断言只查名字，删掉调用点照样绿，rc.5 已实证）；
  ② **A2 重放**：09-27 前端重设计 **112 个 Flutter 文件**并入 0.2.0 线，解 **29 处冲突**（15 响应 / 14 哑）；
  ③ **背景 P0-1…P0-5** 全修（背景图永久丢失三段链 / 迁移不检查 `put` / 拖动排序双重减 1 / 缺陷被测试背书 / 遮罩文案与实现相反）；
  ④ 15 条审计复现转正、删 `zz_audit_tmp_test.dart`；`docs/architecture/background-parity-vscode-background.md` 偏离说明落盘。
  门禁：cargo **1457/0** · doc 3 · fmt clean · clippy 0 · rust-ratio **97.3595%** · flutter **1272** · ignite **4/4**。
  发布说明 `docs/releases/v0.2.0-rc.5.md`。

- **2026-09-26（v0.2.0-rc.4，0.2.0 线最后一个 RC：表演协议 v1 全链 + 导演可观测 + 单模型动作强度）**：
  ① **阶段3 单一驱动者（D10–D13）**：退役「前端拉 `latest.preset_id` 驱动舞台」，动作只由 WS
  `action_cue` 驱动；`preset_id=="none"` = 撤销哨兵、`cues:[]` = 本轮不动。
  ② **阶段4 表演协议 v1（D22–D39）**：`speak` 退役为 `segments`（只切分、逐字不变、拼接 == 原文），
  上屏 == 送 TTS == `clean_for_tts(段)`；三字段 `body`/`head`/`expression`（同类 add / 立即生效 / `hold` /
  `expression` 只写五官）；段↔句 1:1、空白段不跳号；**唯一时间基准 = 音频播放时钟**（`stage-clock` 30ms）；
  渲染面**事件级 ack 四条 + `segment-ended`**（含 clamped/degraded/reason）；**会话 baseline** 绑会话、
  停止/新消息清三样并回 baseline、不补帧；`action_cue`/`preset_id`/`speak` 一律不删（V11）。
  ③ **阶段5（D40–D48）**：dev_mode「**导演可观测**」四栏（决策参数 / 事件流 / 传参对照 / 送 TTS 文本，
  **零新 WS 帧**）+ **单模型动作强度** `[action.models.<id>]`（三键各自回落全局）；维护者补修两处
  （通道 B 判据语义化、观测环折叠 stage-clock）。契约 `docs/architecture/performance-protocol-v1.md`；
  门禁 cargo **1457/0** · clippy 0 · rust-ratio **97.3595%** · flutter **1091** · ignite 4/4 ·
  `verify_core_chain` **17/17**；发布说明 `docs/releases/v0.2.0-rc.4.md`。

- **2026-09-14（产品级加强波次，本地 `mod/product-grade`，未发布 / 无版本变更）：**
  真源 `mod/stabilize` @ `71348c93`（版本仍 `0.2.0-rc.3`）。① **封存 wallpaper + pet-desktop**：
  移出 `AVAILABLE_MOD_FACTORIES`（7 → 5），`mod_count_is_seven` → **`mod_count_is_five`**；
  移出 desktop 依赖；crate 暂留 workspace 并标 **ARCHIVED**（禁止挂回）；新增
  `docs/architecture/ARCHIVED-mods.md`（理由 / 没连坐删什么 / 恢复条件）；现行文档全同步。
  **用户手动的舞台/壳背景能力（`DisplayPrefs`）保留**——只拆 wallpaper **Mod** 的接线
  （Flutter `wallpaper_api.dart` / `shell_wallpaper.dart` / `applyWallpaperPatch`）。
  ② **共享基座**：`ModRuntime::command` + `POST /api/v1/mods/{id}/command`（200/400/404/409/503）
  + 每 Mod Flutter **产品面板扩展点**（`settings/mods/*_panel.dart`，并行五轨零冲突）。
  ③ **五个 Mod 产品级**：external-input（计数可见 + `reset_counters` + `token_set` + sidecar 文档修正）、
  voice-input（说人话面板 + `selftest` + 失败码/Windows 路径文档 + 与 external 职责表）、
  persona（卡导入命令 + `state` 503→200 + 面板 + `persona-mod-v0.md`）、
  memory（`records` + `clear` 原子清空 + 面板 + §12 可重复验收）、
  director（**一等决策面板** + `latest`/`clear`；当时的「**零投递不变**」结论**已变更**——现产
  按句 `action_cue` 驱动舞台（`latest.preset_id` 仅面板只读；前端拉取驱动已退役），现状见 §「动作与表演的现行状态（2026-09 实测）」；
  仍不做 apply-to-TTS）。
  ④ **真点火抓到并修掉两个缺陷**：`local-llm` 与 `ModRuntime::command` 方法名撞名
  （全量 `cargo test --workspace` 才暴露）；`config_path_for_web()` 返回裸文件名 →
  memory 的 `resolve_store_path` 得 `None` → **「运行中却一条都记不住」** → 配置路径统一**绝对化**。
  ⑤ **门禁**：cargo test **1132 passed / 0 failed**；doc 3；fmt clean；clippy 0 warning；
  rust-ratio **96.4194% PASS**；wasm check ok；flutter analyze 无问题 + **902** 测试通过；
  pytest 22 passed/1 skipped；两个 sidecar 自检 70 项 + 16 断言 OK；
  **真点火**（本机 DeepSeek + CosyVoice，18099）两轮对话：memory 2 写 1 命中、director 2 决策、
  persona 导入→还原→再启用、`command` 失败态（404/409/503/400/403）全部符合契约。
  收束：[`docs/legacy/plans/PRODUCT-GRADE-CLOSEOUT.md`](docs/legacy/plans/PRODUCT-GRADE-CLOSEOUT.md)；
  点火：[`docs/legacy/plans/IGNITION-CHECKLIST-product-grade.md`](docs/legacy/plans/IGNITION-CHECKLIST-product-grade.md)。
  **未 bump / 未 push / 未打 tag**；主链皮肤与 `l2d-wasm-demo` 一行未改。

- **2026-09-14（Wave 3 七轨闭环，本地 `mod/wave3`，**未发布 / 无版本变更**）：**真源 `mod/wave2` @ `1e789cb6`
  （本地 `0.2.0-rc.3` 候选）。**基座** `118bd435`：`ModEventTopic::TurnEnded`（payload = turn id；
  发点在 `run_one_turn` 返回之后，成功/失败都发）——Mod 终于有轮末钩子。七条轨各在自己 worktree
  （`mod/w3-voice|wall|memory|pet|persona|external|director`）从基座起分支。① **voice**：
  `backend`/`locale` 不再是死配置（`normalize.rs` + handler 明确分支 + 结构性无网络），sidecar
  失败码表 401/403/busy/empty/timeout/transport + 退避，`--selftest` 49→70；② **wallpaper**：
  列表增删/排序（纯函数 + 最小 UI）、`playlist_len` 退出可编辑 schema（消除手填矛盾）、
  interval/follow_stage 各一条 state 轨迹；③ **memory**：固定语料质量基线（中文命中/不命中）、
  `append_capped` 物理淘汰（`.tmp`+rename 原子重写）、`writes/hits/injects/errors` 四计数、
  last-writer-wins 两向对称回归；④ **pet-desktop**：**软闭环**（窗口不设）——Flutter
  「Mod 管理」消费 `/state` 展示关键字段 + 配置热更新可测；⑤ **persona**：坏卡
  enable→Failed→修好→再 enable 的**主链值**断言 E2E ×4 + 与 memory 共存契约；
  ⑥ **external-input**：接受/拒绝/busy 计数进 `state_json`、sidecar 节流 `--min-interval-ms`、
  `v2_ignored` 可见；⑦ **director**：从 RFC 推进到**最小骨架**（新 crate；订阅 `TurnPrompt`+
  `TurnEnded`；纯函数决策 `{emotion,intent,suggested_tts}`；**零投递**，`action_calls==0` /
  `apply_calls==0` 有间谍断言），已注册但缺省停用。**FACTORIES 6 → 7**（`mod_count_is_seven`），
  缺省 manifest 不变；**版本三处未动、未写 `docs/releases/v0.2.0-rc.4.md`、未 push**；
  主链皮肤 / `l2d-wasm-demo` / framebuffer 一行未改。收束见
  `docs/plans/parallel-mods/WAVE3-CLOSEOUT-2026-09-14.md`。
  基线 `mod/integrate-0.2.0-rc.2` @ `91c670aa`（**`main` 当时仍是 rc.1，故以 integrate tip 为准**），
  五条轨各在自己的 worktree/分支（`mod/memory-v0` / `mod/director-rfc` / `mod/voice-sidecar-v1` /
  `mod/wallpaper-wire` / `mod/pet-desktop-v1`）从**基座提交** `429609f2` 起分支，合入 `mod/wave2`。
  范围真源：任务书 + `docs/plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md`（**本轮没有**更早写下的
  Wave 2 计划文档，协议是随基座一起落盘的）。① **基座（主 agent 独占）**：`ModEventTopic::TurnPrompt`
  （payload = 本轮输入正文——`TurnStarted` 只有 turn id，记忆/导演必须拿正文）、
  `ModRuntime::state_json`（只读运行态快照；必须脱敏、不得写盘/阻塞）、
  `ModRegistry::runtime_state`（`try_lock`，HTTP 绝不等 Mod worker）、
  `GET /api/v1/mods/{id}/state`（**404「不在注册表」与 503「在册但读不到」刻意分开**）。
  ② **A 轨语音闭环**：`POST /api/v1/voice/transcript`（专用 loopback 端点，选它而非复用
  `/api/v1/external/chat` 的理由在 `docs/voice-input.md` §10）、`web_api/voice_routes.rs` +
  25 条回归、`docs/voice-input.md` 契约（6 条 curl / 错误码表）；sidecar
  `docs/examples/voice-sidecar/voice_sidecar.py`（纯标准库；`--transcriber fake|cmd:` /
  `--dry-run` / `--selftest` / 退出码 0/2/3/4/5）+ 假音频 fixture；**ASR 不进 Rust**。
  ③ **B 轨壁纸接线**：`apply_decision` 的 warn+false **占位整段删除**，改真投影
  `to_prefs_patch()`；`WallpaperRuntime::state_json()` 用内部 `Instant` 喂既有纯策略（不起线程）；
  Flutter 新增 `stagePlaylist` + 纯函数 `applyWallpaperPatch` + 「加入/清空轮播」+ 5 s 轮询闭环
  （列表长度经既有 `POST …/config` 写回）；**framebuffer / wasm / `stage_bg` 一行未改**。
  ④ **C 轨记忆**：新 crate `live2d-ai-mod-memory`（本地 JSONL + 中文 bigram/ASCII 词元重叠
  Jaccard + top-k 稳定平局，**零向量云依赖**；44 条单测）→ `apply_settings` 写既有
  `persona.system_prompt`（marker 幂等剥离；**只对下一轮生效**；与 persona **last-writer-wins**）。
  ⑤ **D 轨导演 RFC**：`docs/architecture/director-rfc.md`（输入/输出/休眠 `action_tx`/晋升门槛/
  与 persona·memory 的写入者×字段表/非目标），**本轮不注册**（否决「注册但 enable 即 Failed」的
  理由在 §8）；顺带钉死四条结构性事实（Mod 侧看不到 token 流、无 per-request TTS 参数通道、
  无 `TurnEnded` 主题、`ModelActivated` 是休眠主题）。⑥ **E 轨桌宠**：静态 `settings_spec` 与
  `start` 同源 + `state_json()`（配置/事件态进 API 可测面）；**窗口未开**（原生壳休眠，
  `window.opened=false` + `reason`），15 条集成测试。⑦ **收束**：`AVAILABLE_MOD_FACTORIES`
  5 → **6**（`memory`，缺省停用；`director` 不注册），`mod_count_is_five` → `mod_count_is_six`，
  id 断言同步，缺省 manifest 不动；版本三处 + 文档同步到 `0.2.0-rc.3`。
  发布说明 `docs/releases/v0.2.0-rc.3.md`。
- **2026-09-14（v0.2.0-rc.2，Wave 1 三轨合成：voice-input + wallpaper + persona-polish）**：
  把三条并行 Mod 轨道（tip `4e421993` / `64601710` / `d5d7dcef`）合入集成分支
  `mod/integrate-0.2.0-rc.2`。① **接线**：`AVAILABLE_MOD_FACTORIES` 3 → **5**
  （追加 `voice-input` / `wallpaper`），`mod_count_is_three` → `mod_count_is_five`，
  `mod_factory_ids_match_expected` 同步五个 id；**缺省 manifest 不动**——两者只注册、
  **缺省停用**（`external-input` 仍缺省启用）。② **persona 行为变更**：坏配置由
  `Running` 变**显式 `Failed`**（旧行为记一行 error 后 `Ok`，界面显示「运行中」而主链
  没变）；卡文件/JSON 体积上限、PNG `chara` 健壮性、基线快照语义厘清。③ **voice-input**：
  新 crate 骨架（转写 → 清洗 → `say_tx`，13 条单测），ASR 后端仍是配置占位。
  ④ **wallpaper**：新 crate 策略 v0（`mode`/`interval_secs` + 纯状态机，22 条测试），
  决策 → `DisplayPrefs` 的落点仍是**明文占位**（`apply_decision` 只对 `None` 返回 `true`）。
  ⑤ **文档**：`mod-product-chain.md` §5 加两行；`PARALLEL-PROTOCOL-2026-09-14.md`
  与两份 PLAN 入库（补断链）；存活架构文档的 `mod_count_is_*` 引用同步。
  **主链皮肤与 `l2d-wasm-demo` / framebuffer / 背景实现 diff 为空**；`local-llm` 未挂回、
  动作层保持休眠。发布说明 `docs/releases/v0.2.0-rc.2.md`。
  门禁：cargo **904** 通过 / 0 失败、doc 3、fmt clean、clippy 0 warning、
  rust-ratio **96.9243% PASS**；flutter analyze 无问题 + **833** 通过。
- **2026-09-14（v0.2.0-rc.1，Mod 纪元第一基线：external-input 直播刚需 + 社区许可）**：
  版本线 `0.1.0-rc.5` → **`0.2.0-rc.1`**（不是补丁增量：外部事件成为一等刚需）。
  **主链皮肤冻结**——LLM/TTS/口型/Live2D、壳/舞台背景一行未改。① **外部事件半成品做实**：
  `POST /api/v1/external/chat` 契约重写（端点表 / token 优先级 env→Mod config→不鉴权 /
  Origin+loopback / 忙碌 `ok:false` / 启停门禁 / 模板前缀 / 6 条 curl / 错误表），
  并**写明「B 站抓取不在主仓，在 Win sidecar」**。② **`external-input` Mod 加强**：
  工厂静态 `settings_spec` v2（去掉与 manifest 重复的 `enabled`；新增 `text_template`/`prefix`），
  纯函数 `render_injected_text`/`render_from_config`/`token_from_config`；handler 加
  `403 mod_disabled` 门禁并复用 Mod 纯函数；**修掉 Authorization Bearer 头未读取的缺陷**；
  补 6 条 handler 回归（门禁三态 / Bearer / config token / 模板膨胀）与 10 条 Mod 规格/纯函数测试。③ **Win py 示例**：
  `docs/examples/bilibili-sidecar/`（blivedm，只处理 `DANMU_MSG`+`SEND_GIFT`，
  其它 cmd 只 log；清洗后 POST；README 写房间号/SESSDATA/token 注意点，注明
  `SEND_GIFT_V2` 灰度需后续兼容）。④ **废除 `local-llm` 启动**：移出 `AVAILABLE_MOD_FACTORIES`
  （4→3）与 desktop 依赖、移出缺省 manifest，crate 暂留并标 DEPRECATED；缺省 manifest 改为
  启用 `external-input`。persona **不回退**（仍只在 Mod 侧）。⑤ **`mod-community-license.md`**：
  注册面开放 / 分发面 AGPL 兼容 / 闭源走私用或商业许可，**无「闭源可进默认包」承诺**。
  发布说明：`docs/releases/v0.2.0-rc.1.md`。
- **2026-09-14（v0.1.0-rc.5，壳全局背景 + 与舞台同步）**：主链一行未改，只动
  前端显示层。① **偏好字段**：`DisplayPrefs` 增加 `shellImage`（壳自己那张，
  只在同步关时用）与 `syncShellStageBg`（默认 `true`）；`effectiveShellImage`
  getter 是唯一判据（同步开 → 舞台那张）。② **渲染**：新增 `ui/shell_backdrop.dart`
  （`ShellBackdrop` + 纯函数 `decodeDataUrlBytes`，坏 dataURL 永不抛），
  `AppShell` 最外层铺主题底色 + **固定 0.15** 的背景图；有背景时脚手架底透明、
  聊天面板面透明度 `0.86`（保持可读），无背景时观感与改动前一致。
  ③ **UI**：「外观与互动」新增「壳背景」块（选图 / 清图 + 「与舞台同步」）；
  同步开时该行的选 / 清图改的就是 `stageImage`——**一份真相，不是两张**。
  ④ **版本三处同步** `0.1.0-rc.5`（`Cargo.toml` / `pubspec.yaml` / README 首屏）。
  ⑤ **rc.5 修复：舞台背景图不显示（2026-09-14，实测抓到）**——现象是标题栏
  （Flutter 壳）能看到背景图、**Live2D 舞台仍纯黑**。根因：渲染面把
  `canvas.style.background-image` 写成 `url(...) center/cover no-repeat`，
  而 `background-image` 是**长手**属性、只接受 `<image>#`，位置/尺寸/重复让**整条
  声明被 CSS 解析器丢弃**，且 `style.setProperty` 不报错——于是 backgroundImage 恒空。
  修法：只写 `url(...)` / `none`，位置/尺寸/重复分写三个长手属性；换算抽成
  **原生可测**的 `crates/l2d-wasm-demo/src/stage_css.rs`（旧实现埋在 wasm-only 的
  `web::surface::input` 里，原生 `cargo test` 编译不到 = **没有回归**，与 `mouth.rs` 同款教训）。
  （当时据「clear 为 `TRANSPARENT`、`alpha_mode` 优先 `PreMultiplied`」判断
  「不是合成问题」——**该判断被紧接着的 ⑥ 推翻**：caps 里根本没有 PreMultiplied。）
  渲染面产物已 `trunk build` 重建（`dist/` 被 gitignore，改 wasm 必须重建）。
  ⑥ **rc.5 修复二（已证伪并回滚）：黑幕盖住舞台**——CSS 值合法后，加载瞬间舞台能
  闪到背景、随后被**黑幕铺满**。根因在 wgpu 29.0.4 源码里：`webgpu` 后端的
  `get_capabilities` **只报** `alpha_modes: vec![Opaque]`，`wgpu-core` 又以
  `UnsupportedAlphaMode` 拒绝 caps 之外的 `alpha_mode` ⇒ WebGPU 路径**无法请求
  `PreMultiplied`**，canvas 必然按 `Opaque` 合成，`Clear(TRANSPARENT)` 变成不透明黑。
  曾据此改成 **WebGL2 优先** 想换透明 canvas，但**用户机 HUD 实测仍是
  `GPU: webgpu/WebGPU`**（`request_gl` 未生效）→ 该改动已**回滚**（`gpu.rs` 恢复
  WebGPU 优先），并判定「靠 canvas 透明露 CSS 背景」**结构性不可达**。
  ⑦ **rc.5 修复三（最终方案）：舞台背景画进 framebuffer**——背景预通道
  `LoadOp::Clear(<当前 stageColor>)` + 有图时全屏三角形按 `cover` 贴纹理
  （`web/surface/background.rs`），模型通道改用新增的
  `l2d::ModelRendererCore::render_to_view_submit_with_load(..., LoadOp::Load)` 只叠
  Live2D、**不再 clear**；叠放语义不变（图盖纯色底、模型在最上、清图回纯色），
  与 canvas 是否透明**完全无关**，WebGPU 保持默认。图像解码走浏览器
  （base64 → Blob → `createImageBitmap` → OffscreenCanvas 2D → `write_texture`），
  PNG/JPEG/WebP 全支持且**不引入 Rust 图像依赖**；坏图退回纯色底。
  canvas 的 CSS `background-color` / `background-image` 写入**已删除**、`stage_css.rs`
  随之删除（不透明 canvas 上永远看不见，留着只会误导）。纯逻辑在原生可测的
  `l2d-wasm-demo/src/stage_bg.rs`（`base64_decode_data_url` / `cover_uv` /
  `clear_color`）。HUD 新增 `bg: solid|image` 字段作为「图有没有上屏」的直接信号。
  门禁：`flutter analyze` 无问题 + `flutter test` **833** 通过（本轮修复未改 Flutter）；
  cargo **830** 通过 / 0 失败（rc.4 822 + `stage_bg` 8；§3.1/§3.2 的 3 条临时回归已随路线
  废弃删除）/ doc **3** / fmt clean / clippy **0 warning** / rust-ratio **97.1135% PASS** /
  `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` ok。
  发布说明（含 **Win 选图 → 可见 → 清图** 肉眼 checklist）：
  `docs/releases/v0.1.0-rc.5.md`。**范围真源缺口**：任务书点名的
  `docs/plans/PLAN-rc5-shell-bg-sync-2026-09-14.md` **不存在**（已确认不在任何分支 /
  stash / 工作区），本轮以任务书逐条列出的必做 / 禁止为范围，未列出的都没做。
- **2026-09-13（v0.1.0-rc.4，Mod 产品链路 + 主链人设收敛）**：主链
  LLM→TTS→口型→Live2D 未变。① **Mod 产品链路（M0–M4）**：新增
  `docs/architecture/mod-product-chain.md`（契约 + 加新 Mod 勾选表 + 正式版 Rust/C 规则）；
  `mods.json` 启停/配置**原子写回**（`plan_atomic_write`）；`crates/live2d-ai-mod-template`
  模板 + `descriptor.api_version` 门禁（不兼容 → Failed 不崩）；`ModServices.apply_settings`
  **一等化**并**删除** `__apply_settings` 事件走私；补 `ModServices.settings`（脱敏读取）
  + `config_path`。② **schema 发现（M2）**：`ModFactory::settings_spec`（静态，未启用也拿得到）
  → `GET /api/v1/mods` 带 `config` + `settings_spec`（secret 脱敏）+ `GET …/config`；
  Flutter `ModsSection` 按 kind 渲 Bool/String/Number/Select + 保存，secret 留空不提交。
  ③ **角色卡标准 Mod（M5）**：`live2d-ai-mod-persona`（Rust）解析 SillyTavern V1/V2 JSON +
  PNG `chara` → 合成 `system_prompt` 经一等 `apply_settings` 写回，禁用时按
  `persona-mod-base.txt` 基线**还原**；主链 `[persona]` 只留 `system_prompt` +
  `max_history_pairs`（**破坏性：老 toml 卡字段会解析失败，升级需手改一次**），Flutter 删
  `persona_card.dart`/`persona_import.dart` 与导入 UI；Mod 数 3 → 4（`mod_count_is_four`）。
  ④ **Win stage-bg（M6）**：`Live2DStage` 挂桥/重建时补发 `stage-bg`；`saveDisplayPrefs`
  返回 bool（写失败给老实文案）；`pickImageDataUrl` 三态（读失败不再冒充取消）。
  **Win 肉眼验收待做**。门禁：cargo **822** 通过 / clippy 0 warning / rust-ratio
  **97.0776% PASS**；flutter analyze 无问题 + **819** 测试通过；`ignite.sh --check` 四项全 ok；
  API 端到端验证角色卡写回/还原。发布说明：`docs/releases/v0.1.0-rc.4.md`。
- **2026-09-13（v0.1.0-rc.3，结构质量）**：主链一行未改，收的是「挡住正式 0.1.0」的两类东西。
  ① **正文兜底（N0）**：上屏闸门是「该句语音已合成完毕」（同拍契约），代价是 TTS 故障 /
  半句切不出 / LLM 中途断流时**整轮一个字都不上屏**。新增 `EngineEvent::TextFallback` →
  `ConversationUiEvent::TextFallback` → WS **新帧 `text_fallback`**（覆盖式整段正文，
  不是残余；只在 `Failed` 发、在 `Terminal` 之前、健康轮永不发），前端气泡加
  「未收尾」说明行且**落盘**；契约写进 `core-chain-baseline.md` §2.1。
  **真机又抓到一个时序缺陷并修掉**：`run_one_turn` Stage A 的 biased `select!` 选中
  `gen_fut` 臂后不再回头 poll `event_rx`，引擎收尾时**同步发出**的那批事件被
  `drain_residual_events` 静默丢掉（`TextFallback` 首当其冲）→ 现在生成返回后先把
  通道排空；回归已实测「去掉修复即红」。
  ② **结构减法（N1）**：`main.dart` 1288→**543**（admin/settings/模型库/偏好回调外移到
  `app/shell_*.dart`，`package:web` 收进 `app/browser_io.dart`）；
  `surface.rs` 1167→**24**（render/input/idle/gpu，`IdleState` 逐字保留）；
  `ws.rs` 1073→**263**（audio/broadcaster 子模块）；`tests_models_routes.rs`
  1249→**4 个场景文件**（48 条测试一条不少）。
  ③ **双壳裁决（N2）**：egui 原生壳 / `--chat` **非主线**，休眠台账进 AGENTS
  （**选项 B：不 feature-gate**，理由与红线写在台账里）。
  ④ **半接线清干净（N3）**：`forcedByLaunchFlag` 不再写死 `false`（改由「有效
  dev_mode vs 落盘设置」推出）；修掉 `chat_panel.dart`「历史不落盘」的过时注释；
  UI 明示**会话记录 ≠ 模型记忆**；设置/诊断文案反向扫描无已删能力残留。
  ⑤ **门禁对齐（N4）**：`pr-checks.yml` / `nightly.yml` 补 `--all-targets` /
  `--doc` / `rust-ratio` / clippy `--all-targets`；新增
  `flutter-checks.yml`（`paths: shell/flutter/**`）——CI 之前**完全不管前端**；
  nightly 新增端到端探针 job（**只在配置 `LIVE2D_AI_VERIFY_BASE_URL` 时跑**，缺配置明确
  跳过，不伪造绿灯）；AGENTS 出一张「本地必跑 vs CI 必跑」表；`CONTRIBUTING.md` 整份
  重写（旧版还在教已归档的 Python 双端）。
  ⑥ **可读性收尾（N5，部分）**：仓库根 8 份 Python 时代旧计划 → `docs/legacy/`；
  `docs/design/` 4 份旧 JS 规格 → `docs/design/legacy/`（现行只剩
  `web-ui-spec-v3.md`）。
  门禁：cargo **801** 通过 / clippy 0 warning / rust-ratio **97.0054% PASS**；
  flutter analyze 无问题 + **822** 测试通过；`ignite.sh --check` 四项全 ok；
  `verify_core_chain.py --timeout 300` **18 跳全过**。
  发布说明：`docs/releases/v0.1.0-rc.3.md`。
- **2026-09-12（v0.1.0-rc.2，第二基线）**：rc.1 之后的第二个基线。范围冻结为「瘦版」
  （M0 点火纪律 + M1 动作裁决 + 模型闭环 + `.env` 密钥源 + 旧预览隔离）；
  `main.dart` 大拆 / `surface.rs` 大拆 / egui feature-gate / CI 对齐**都推到 rc.3**。
  ① **动作层删到底**：director Mod 整体删除、`SupervisorHandle::trigger_action` 与
  supervisor 的 `action_rx` 分支删除（那是**唯一**能把 `RootEvent::Action` 送进 core
  reducer 的路径）、`HostChannels.trigger_action` 删除（`ModServices.action_tx` 保留为
  Mod API 契约，但注入固定休眠 sender）、渲染面 `action-state` 接收器 + 编舞表删除
  （`surface.rs` 1721 → 1169）。**待机生命体征一行未动。**归档：分支 `archive/action-layer-p6`
  （**2026-10-06 复核：该分支已不存在**；取回 `git show ef9f428^:crates/l2d-wasm-demo/src/web/surface.rs`）。
  ② **capabilities 去广告**：删 `actions`/`action_sources`/`strength_levels`/
  `model_upload_supported`/`script_invoke_supported`（后两个是假广告），`schema_version` → 2。
  ③ **模型库闭环**：新增 `web_api/model_root.rs` 定为**唯一模型根**
  （`<cwd>/assets/models`，静态服务与 registry 同源；XDG 那条删除）、
  `find_model3_json` 向下看一层（原来连自家的 `bai/runtime/` 都导入不了）、
  `app/status.active_model_id` 读真实 registry、activate 恒 `requires_restart=false`
  且 `model_url` 可 GET；前端接上 `sendSync(model:)` 并**等渲染面 `loaded` 回执**才说
  「已切换」，同时补上一直缺的**导入入口**。
  ④ **`.env` = 唯一密钥真源**：新增 `live2d-ai-runtime::secrets`（快照读取，不用
  `set_var`）+ `GET/PUT /api/v1/env`（**永不回值**、原子写、`0600`、就地改行、热重载）
  + `.env` 纳入 `file_watcher`；前端设置面板可直接填 key。
  ⑤ **点火纪律**：`ignite.sh` 锚定 `LIVE2D_AI_FLUTTER_WEB_DIR` + `--check` 体检；
  503 响应体列出找过的每个路径；WSL2 ↔ Windows 分工写进本文件与 README。
  旧 JS 前端预览隔离到 `docs/design/legacy/` 并标注「**勿当现网**」。
  ⑥ **补丁（2026-09-13，rc.2 内，实测抓到）**：上游是**推理模型**，思考与正文共用
  `max_tokens`——512 时正文被挤成半句、**一个字都不上屏**（用户报「模型没有返回」）。
  修法：解析 `reasoning_content` 单列 `LlmEvent::ReasoningDelta` → WS **新帧
  `reasoning_delta`** → 前端气泡的**「思考」折叠区**；默认上限 512 → **4096**；
  「只有思考没有正文」单独收口。见下方「推理模型的思考」小节。
  门禁：cargo **795** 通过 / clippy 0 warning / rust-ratio **96.9754% PASS**；
  flutter analyze 无问题 + **813** 测试通过（含无头浏览器验收补丁）；`ignite.sh --check` 四项全 ok；
  `verify_core_chain.py`（长思考提问）**18 跳全过**。
  发布说明（含点火记录与已知问题）：`docs/releases/v0.1.0-rc.2.md`；
  **接手入口**：`docs/legacy/plans/HANDOFF-2026-09-13-rc2-second-baseline.md`
  （一分钟上手 / 门禁数字 / 交付态实测 / 推到 rc.3 的事 / 本轮新踩的七个坑）。
- **2026-09-11（v0.1.0-rc.1，核心链路基线）**：用户验收通过 → **交接落盘 + 标注基线**，
  随后裁决「**旧版本代码可只存在本地，仓库可以洗一下**」。本版：
  ① **版本线由 `0.5.1` 重置为 `0.1.0-rc.1`**（本版不是 0.5.x 的增量，而是核心链路重新
  定义后的第一个对外 RC；按 0.5.2 递增会让一次拆除看起来像小修补）；
  ② **LLM 工具层 + 动作系统从前后端整体拆除**（LLM 只做对话，请求体不再有
  `tools`/`tool_choice`/`functions`，有回归断言三键不存在）；**刻意保留但休眠**的是
  `live2d-ai-core` 的 action/performance 子系统、director Mod、`ModServices.action_tx`
  （仍属 Mod API 契约，无自动驱动方）；**必须保留**的是**待机生命体征**
  （`IdleState` 呼吸/眨眼/微表情，与动作系统是两套机制）；
  ③ **音频改走 `<audio>` 媒体元素 + Blob(WAV)**，静音/音量落在 `element.muted`/`volume`
  （不再折成 `GainNode`——站点级静音只作用在媒体元素上）；
  ④ **公开历史重新起算**：`main` 变成**单个根提交**，旧历史里的 Python/Android 工程、
  60 MB 调试 APK 与 **Live2D 模型二进制**不再公开（与本项目「模型不捆绑分发」的立场
  原本自相矛盾；公开 `.git` 256 MB vs 产品树 19.7 MB）；旧代码只存在本地。
  发布说明：`docs/releases/v0.1.0-rc.1.md`；基线逐环：`docs/architecture/core-chain-baseline.md`；
  交接：`docs/legacy/plans/HANDOFF-2026-09-11-core-chain-baseline.md`。
  门禁：cargo **765** 通过、clippy 0 warning、rust-ratio **96.95%** PASS；
  flutter analyze 无问题、flutter test **778** 通过；`scripts/verify_core_chain.py` **17 跳全过**。
- **2026-09-11（v0.5.1 追加）**：用户报「后端出错无具体错误代码 + 改动提示词就崩 +
  看后端日志什么都没有 + 前端无法知道错误信息」。查证**三条全对**：
  ① `dispatch.rs` 一行请求日志都没有；② 链路错误用 `println!`（不进 tracing 文件 sink）；
  ③ `AppEvent` 没有错误变体 → WS **从来不发** `error` 帧（前端那个 `WsErrorEvent`
  永远收不到实例）。修法：`ErrorKind::code()/stage()/is_fatal()/hint()` 错误码契约、
  `AppEvent::Error` + `error` 帧投影、请求级日志与 403 提到 `warn`、
  前端两条消费路径（`UiStateTracker` / `ChatController`）共用 `formatWsError`、
  保存失败 toast 带码、错误横幅「下一步」按码分流。**顺带查出真 bug**：
  手改 `live2d-ai.toml` 后 `GET /settings` 不跟随磁盘，下一次界面「保存」会把
  手改内容**覆盖**掉 → 新增 `StatusContext::refresh_from_disk`（回归
  `web_api/tests_reload.rs`）。复核：改提示词本身不失败（10 组取值全 200），
  当前那条失败是进程环境缺 `DEEPSEEK_API_KEY` 的 401。门禁 812 + 627 全绿。
- **2026-09-11（v0.5.1）**：v0.5.0 交付后接浏览器工具做**真机验收**，
  抓到**十二个 bug**（大部分逃过了当时全绿的 810+590 条测试）：
  ① 直接点「设置」不加载（面板写「读不到服务端设置」）；
  ② `showModalBottomSheet` 的 builder 只跑一次 → medium/compact 设置浮层**永远转圈**；
  ③ 路由把查询串当路径 → `GET /api/v1/logs?limit=200` 回 501，诊断日志从未成功；
  ④ `dev_mode` 的开关被自己所在的分区藏起来 → 界面上**永远打不开**；
  ⑤ **保存设置会抹掉配置文件全部注释**（`toml::to_string` 重生成文档）——
  当天真的发生一次，改用 `toml_edit` **就地改值**（`merge_into_toml`）；
  ⑥ UI 文案露出 Markdown `**`（15 处）→ 新增 `EmphasizedText` 真渲染 + 扫描规则；
  ⑦ 切主题后舞台不变色（`_applyPrefs` 读了尚未更新的 `widget.prefs`）；
  ⑧ 缩放读数永远是 `—`（没人读渲染面首帧的 `stage-ack`）；
  ⑨ 老 JS 前端占着 `/` 与 `/app` 是两套界面 → `/` 302 到 `/app/`，JS 前端整套删除；
  ⑩ 状态胶囊冒充「后端未连接」（`bridgeError → offline` 口径错 + 信号只置位不清除，
  普通刷新就能撞上）→ 删掉那条派生，渲染面失败只留在舞台自己的覆盖层上；
  ⑪ **压在舞台 iframe 上的控件全都点不着**（用户报「设置能唤醒，但点不动、不能上下滑」）
  → 新增 `StagePointerInterceptor` 指针垫层（透明 `<div>` 平台视图，零新依赖），
  设置面板 / 断线横幅 / 缩放角标 / 两处错误重试逐个接线（见上方前端层约定）；
  ⑫ **删掉左侧分区 rail**（用户裁决「把左边的这些设置一级选项去掉留给舞台」）——
  那一列 168 px 还给舞台，设置入口只剩 AppBar 的「设置」一处，分区切换在面板内的
  chip 行（顺带删掉「再点一次 rail 上同一分区收起」的切换语义）。
  浏览器工具到位后还重验了：本次加载**外部源请求 0**（断网红线通过），
  并端到端跑通圆形↑发送键（输入 → 点按钮 → 用户气泡 → LLM 回复）。
  教训：**测试全绿 ≠ 界面是对的**；异步时序、浮层构建时机、平台视图、
  精确路径匹配这四类问题只有真的在浏览器里点一遍才会露出来。
- **2026-09-11（v0.5.0）**：用户裁定「**只核心链路做丰满即可**」+「前端 ui 很 ai 化同质」。
  据此**先删后改**：动作手动触发移出成品（只留分支 `archive/action-trigger-p5`
  与 `docs/design/web-action-trigger-archive.md`）；清掉没有功能的占位 UI；
  **本地 TTS 从 Mod 提升为核心链路**（`docs/architecture/tts-is-core.md`，
  Mod 5 → 4）；新增**黑/白/蓝/灰四套配色**（主题住本地偏好，点一下立刻生效）；
  舞台改**纯色底**（协议新增 `sync.stageColor`，由渲染面写——iframe 内 canvas
  会盖住父页画的底色）+ 可选用户展台图；新增**组件外观统一层**
  `buildAppComponents`（elevation 全 0、圆角/字级统一）与「**发送 = 圆形上箭头**」
  「**尽量少用图片，用文字做按钮**」两条形状约定（`test/visual_language_test.dart`）。
  **删掉老 JS 前端**：`/` 302 → `/app/`，一个服务只有一个界面入口。
  另修三个「只有真浏览器才抓得到」的 bug（三个都逃过了当时的全部测试）：
  ① 直接点「设置」不触发加载 → 面板显示「读不到服务端设置」；
  ② `showModalBottomSheet` 的 builder 只跑一次 → medium/compact 的设置浮层
  **永远转圈**（内容不跟数据重建）；③ 路由把查询串当路径 →
  `GET /api/v1/logs?limit=200` 回 **501**，诊断面板的日志区从来没成功过。
- **2026-09-10（v0.4.4 / v0.4.5）**：确立前端**离线可用**的两条硬约束——
  构建必须带 `--no-web-resources-cdn`（否则 CanvasKit 走 Google CDN，断网白屏）；
  中文字体必须自托管（否则断网豆腐块）。两者都是「有网时看不出来」的缺陷。
- **2026-09-10（v0.4.3）**：语音输出约定新增「**默认出声**」与「主音量/静音与口型正交」；
  静音改为默认关闭的服务端开关 `LIVE2D_AI_MUTE_AUDIO=1`（跑测试/无人值守用）。
- **2026-09-10**：确立 **Rust 核心 + Flutter 前端** 双主导分层；前端/接口层
  自 `rust-ratio` 显式豁免并改由 Flutter 工具链门禁；原生 JS 前端降为遗留；
  新增「语音输出约定（一句一单元，不断句）」。
- **2026-08-31**：按 Rust 主线重写（移除 Android/Python 双端描述）。
- **2026-08-09**：重写——移除 pi 专属路径引用，保留项目级通用信息。
