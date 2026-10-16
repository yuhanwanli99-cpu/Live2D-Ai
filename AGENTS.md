# Live2D-Ai 项目说明（AGENTS.md）

> **现行契约见 [docs/architecture/CURRENT.md](docs/architecture/CURRENT.md)（一页，2026-10-13）。**
> 先读那一页：产品现状、导演与动作、Mod 在册、点火与验收都在那里。
> 下面的长文是沿革与逐条记录，历史段落一行不改。

> 本文件是项目级通用说明，供任何 CLI agent（pi / Claude Code / Hermes 等）读取。
> 状态：2026-09-10 重写——确立 **Rust 核心 + Flutter 前端** 双主导分层，
> 前端/接口层自 `rust-ratio` 门禁中**显式豁免**（原版只描述 Rust 单主线）。
> **2026-10-01 单一化：本文件是唯一现行真源。**
> **2026-10-06 工作树单一化**：linked worktree `Live2D-Ai-fe` 已删除，**唯一工作树 =
> `/home/skystar/Live2D-Ai`（分支 `main`）**——旧分支 `mod/persona-polish` @ `88342ce`
> 与它那份「3 个 Mod / 0.2.0-rc.1」的教义一并退役。同名 `AGENTS.md` 若出现在别处都是
> 过时副本，**不要**按它写代码或文档；文档地图与生命周期三分见 `docs/DOC-MAP.md`。
> **文档取回（2026-10-06 E8 文档减量）**：只具历史留证价值的文档（`docs/legacy/**`、
> `docs/design/legacy/**`、2026-08「节点 C」/原生壳验证系列、12 份已收口计划，共 **139 份 / 31,509 行**）已
> **移出工作树**——信息量一点没少：git 历史逐字保留，逐份取回方式见
> [`docs/REMOVED-docs-index-2026-10-06.md`](docs/REMOVED-docs-index-2026-10-06.md)。
> 本文历史段落里对它们的路径引用**刻意保留原样**（改历史引用 = 篡改记录）。
> **版本口径（现行，2026-10-16）**：现状 = **`0.2.6-rc.1`**（`Cargo.toml` 的
> `version = "0.2.6-rc.1"`、发布说明 `docs/releases/v0.2.6-rc.1.md`）。**功能冻结，不发布 `0.3.0`**，下一阶段只清债务。tag `v0.2.6-rc.1` 收 `v0.2.5-rc.1` 之后这一轮：手势曲线与待机摆头让开、同槽接力、表情起落与撤销放回、一句一键停止、本句 TTS 失败后续句继续。上一 tag `v0.2.5-rc.1` 盖住「导演开启时按句清洗并按原文出动作 / 出厂不设人设」，仍在原处不移动；`v0.2.4-rc.1` 盖住「本机出声接通 / 待机开关 + 物理跟导演 / 设置先草稿·保存并重载 / 渲染档位 = 画布最长边 + 运行期逐级降级 / 本地 TTS 子进程 `PR_SET_PDEATHSIG` 生命周期」，仍在原处不移动；`v0.2.3-rc.2` 仍在 `c14240a`，不移动。
> 再往前的 **0.2.0 收口 + 去臃肿第一轮**（2026-10-01 起：S0 文档整理 → `rc.8-a` 正确性与诚实性
> → `rc.8-b` 结构 → 0.2.0 末版，计划书 `docs/plans/PLAN-debloat-and-closeout-2026-10-01.md`）
> 与下面 rc.7 的记录都是**历史**——`0.2.0-rc.7` 不是现状。

## 项目背景

- 文档索引见 `docs/README.md`；核心契约与目录约定见 `docs/architecture/core-contracts.md`
  与 `docs/architecture/directory.md`。
- 定位：**通用人形皮套 AI 接入一体化平台**——AI + Live2D 人形皮套的通用接入与应用层，
  **不绑定任何单一模型**（出厂带白模型，条约见 `assets/models/bai/MODEL_LICENSE.md`，
  不是作者授权再分发），
  **不做复杂上层**（实现保持最小）。验证「文本 → LLM（纯对话，无工具）→ TTS → 驱动口型
  → Live2D 皮套渲染 + 前端 UI」闭环。
- **上一版 `0.2.0`（0.2.0 收口 + 去臃肿第一轮；发布说明 `docs/releases/v0.2.0.md`；现行版本 `0.2.3-rc.2` 见本节首段）**：
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
  **现行 6 个注册 Mod**（external-input / persona / voice-input / memory / director / local-tts-melo）
  为 workspace crate；
  **缺省启用 `external-input` 与 `local-tts-melo`**（直播弹幕/礼物经 sidecar 注入；MeloTTS 由
  `local-tts-melo` 开机拉起仓库内 `melo/start.sh`，见 `cli_entry::default_mods_manifest`），
  其余四个缺省停用（`memory` 会写
  `persona.system_prompt`，必须由用户明确打开；`director` 是**决策 + 按句 cue**骨架，异步第二路 LLM 默认关；
  `local-tts-melo` **只拉起外部进程**，不写 `[tts]`、不探活、不改出声地址；
  `local-tts`（界面名 `本地tts_CosyVoice3-0.5B`）**已封存、移出注册表**，crate 与 `engine/` 留在树上，
  **未适配前不要挂回**）。
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
  `mod_count_is_six` 断言守住（产品级加强波次起为 5；2026-10-09「两类 TTS」加入
  第六个——**只拉起进程**的 `local-tts`——后恰为 **6**），**core 的 `action_tx` /
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
| **Windows** | 只把看/听的结论写回（浏览器由脚本就绪后自动打开）。**不跑二进制、不编 Flutter** |
| **产物** | 单一真源 = WSL 的 `shell/flutter/build/web`。`LIVE2D_AI_FLUTTER_WEB_DIR` 一律写 **WSL 路径**；**不要**引入 Windows UNC 路径写法（`\\wsl.localhost\…`）——只有「哪天真的在 Windows 上跑二进制」才需要，那不在本计划内 |

- **标准构建 + 启动**（仓库根执行，顺序不可换；两份 README 与本文件标注的是同一条）：

  ```bash
  (cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
  ./scripts/prune_web_artifacts.sh
  ./scripts/prune_web_artifacts.sh --check
  ./scripts/ignite.sh --build
  ```

  `flutter` = Flutter SDK 在 `~/flutter`（`ignite.sh` 把 `$HOME/flutter/bin` 放进 `PATH`）；
  `./scripts/ignite.sh --build` 编的是**调试版后端**并启动，**在
  `shell/flutter/build/web/index.html` 已存在时不会重编 Flutter**，所以第一行必须先跑；
  脚本等到本机 TTS 的 `GET {tts}/models` 为 200（云端或未配置 TTS 则等到 `GET /app/`
  为 200）后打印地址，并用默认浏览器打开；**脚本不退出**，体检仍另开终端
  `./scripts/ignite.sh --check`。
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
| **docs 行数预算**（`docs/**/*.md` **不含** `docs/audit/**` ≤ 45,000 行；E8 / E14 起为机器门禁） | `cargo run -p xtask -- code-stats --check --only docs` | `pr-checks.yml` → `Rust Build & Test`（**Docs budget**） |
| MSRV 1.92 可编译 | 手动（可选） | `pr-checks.yml` → `msrv-check`（仅 `cargo check`） |
| 前端静态检查 + 测试 | `cd shell/flutter && flutter analyze && flutter test` | `flutter-checks.yml`（`paths: shell/flutter/**`） |
| 仓库根历史资产测试 | `python3 -m pytest tests/ -q` | `pr-checks.yml` → `root-py-tests` |
| 端到端核心链（需活端点） | `python3 scripts/verify_core_chain.py --timeout 300` | `nightly.yml` → `core-chain-verify`（**仅当**配置仓库变量 `LIVE2D_AI_VERIFY_BASE_URL`） |
| wasm 渲染面可编译 | `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` | **暂无**（本地手动；wasm 改动必须 rebuild + 肉眼） |
| 前端产物级离线门禁（构建 + 产物断言） | `cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn && cd ../.. && ./scripts/ignite.sh --check-dir shell/flutter/build/web` | `flutter-checks.yml` → `flutter-web-offline-artifacts` |
| **前端产物预算门禁**（清减 + 四条判据；E4 / E13） | `./scripts/prune_web_artifacts.sh --check`（对 `shell/flutter/build/web`；先跑上面那条构建） | `flutter-checks.yml` → `flutter-web-offline-artifacts`（**Artifact budget gate**：构建 → `prune` → `--check` → `ignite.sh --check-dir`） |
| 前端真服务离线体检（托管层；与 `ignite.sh --check` 同一份判据） | `./scripts/ignite.sh --check`（对已启动服务；`scripts/ignition-precheck.sh` 的 A 段同源） | `nightly.yml` → `web-offline-serve-check`（**E16 起**：先 `prune_web_artifacts.sh` → `--check`，与 PR 路径同一份产物形态） |
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
护栏是两条断言：`main.rs::mod_count_is_six`（工厂数不得因动作 Mod 增加；第六个是
2026-10-09 加的**只拉起进程**的 `local-tts`）与
`mod_registry::tests::action_request_is_dormant_not_delivered`（动作请求必须不被接受）。

### 动作与表演的**现行状态**（2026-09 实测）

> 本节是 2026-09-21 调研（`docs/plans/RESEARCH-actions-director-audit-2026-09-21.md`）在分支
> `mod/l1-product` 上的实测结论。**上节「休眠台账」只回答「谁休眠、为什么」，不要再把它
> 读成「动作 / 表演不存在」。** 逐条如下，每条给出真源文件：

| # | 现行事实 | 真源（树上可查） |
| --- | --- | --- |
| ① | **动作包（表情 / 手势）经 `preset` 帧直接驱动渲染面参数，不经 core reducer**：共 12 条包 `smile` / `unhappy` / `surprised` / `thinking` / `nod` / `shake` / `look_left` / `look_right` / `look_up` / `look_down` / `tilt_left` / `tilt_right`（另有 `none` 撤销哨兵；`thinking` / `look_up` / `look_down` 由 T9 追加，2026-10-07）；渲染面 `PresetRuntime` 收 v1 协议 `preset` 消息 → `PresetCommand::Apply / Revoke / Ignore`，按 `PresetSlot::Face` / `Gesture` 两槽写参数，到点按槽撤销 | `assets/actions/presets.json`、`assets/actions/preset_labels.json`、`crates/l2d-wasm-demo/src/preset/mod.rs`（`handle` / `apply_frame`）、`crates/l2d-wasm-demo/src/preset/table.rs`（解析期红线）、`crates/l2d-wasm-demo/src/main.rs`（`"preset" =>` 分支）、`shell/flutter/lib/live2d/live2d_stage.dart`（`applyPreset`） |
| ② | **`[action]` 幅度倍率存在**：`head_scale=0.75` / `body_scale=1.4` / `expression_scale=1.0`，运行期参与 `最终值 = 表值 × 包络 × 通道倍率` 并按通道钳位（`ParamAngle*` ≤30 / `ParamBodyAngle*` ≤10 / 五官 ≤4）；Flutter 的滑条在**导演卡片**（设置 → 扩展 → 导演；2026-10-09 从「外观与互动」搬到它的下级块，数据/接口未变） | `live2d-ai.toml` 的 `[action]` 段、`live2d-ai.toml.example` 的 `[action]` 段、`crates/live2d-ai-runtime/src/settings.rs`（`ActionSettings`）、`crates/l2d-wasm-demo/src/preset/scales.rs`（`PresetScales::from_parts`）、`crates/l2d-wasm-demo/src/main.rs`（`set_scales`） |
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

**2026-10-07 追加（T9，维护者当轮裁决）**：接管之后，表演层可以读主模型原文里的问句，给这一句补一次歪头和一张思考表情。问句只认 `？` / `?`。这不是把整段回复拿去打五类情绪，也不把标签写进台词。上一段仍然禁止：整篇情绪分类、随机选表情、表情跨轮常驻、照 N.E.K.O 重做一套情绪模型。计划书 `docs/plans/PROMPT-director-expression-2026-10-07.md`。

**2026-10-13 追加（0.2 收口，维护者当轮裁决）**：导演 Mod 开启且接管生效时，动作按**一路原文的这一句**给出，不按用户输入；导演关闭时不出导演动作。出厂不设人设，`system_prompt` 为空；角色卡仍由 `live2d-ai-mod-persona` 在用户自己启用后导入。不整篇情绪分类，不把标签写进台词，不随机表情，不跨轮常驻，不照 N.E.K.O 重做情绪模型。

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
  本项目是本地优先的皮套接入服务，不允许依赖 Google CDN（`scripts/ignite.sh` 会探测并告警）。
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

> **变更历史（2026-10-16 起）已整段移出工作树**（`docs/DOC-MAP.md` §2 的「历史留证 = 移出工作树」同一条口径；git 历史逐字保留，不改一字）。逐字取回：`git show v0.2.6-rc.1:AGENTS.md`
