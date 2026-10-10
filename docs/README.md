# Live2D-Ai 文档索引

> 本文档把 `docs/` 按用途分目录，避免所有文件堆在顶层。
>
> **文档地图 / 生命周期三分 / 归档规则见 [DOC-MAP.md](DOC-MAP.md)（2026-10-01 立）**；
> **当前执行计划（0.2.0 收口 + 去臃肿）见 [PLAN-debloat-and-closeout-2026-10-01.md](plans/PLAN-debloat-and-closeout-2026-10-01.md)**。

## 架构与契约（当前）

- [架构总览](architecture/ARCHITECTURE.md)
- [核心契约与架构边界](architecture/core-contracts.md)
- [**表演层 v0（`[performance]` 段）：每轮一份合法化 JSON，speak 是 TTS/上屏真源**](architecture/performance-layer-v0.md)
  ——**主模型不负责表演**（无工具、无表演类预设）；表演层每轮独立端点交回 `{"speak":…,"cues":[…]}`；
  默认关；关/超时/非 2xx/校验失败 → `speak=clean_for_tts(原文)` + 规则 cue（仅失败回退）；
  director Mod 的 `staging_*` 降为遗留并行实现（同轮只有一个 cue 产者）
- [**CosyVoice 3 TTS 接入（2026-09-10，能力就绪/未部署）**](architecture/cosyvoice3-tts-integration.md)
- [渲染纹理 / 离屏靶标档位 ADR（4096/8192/16384）](architecture/renderer-texture-tier-adr.md)
- [PC 无预设半身联动实现](architecture/pc-presetfree-halfbody-control.md)
- [目录约定](architecture/directory.md)
- [**外部事件注入契约：`POST /api/v1/external/chat`（0.2.0-rc.1）**](external-input.md)
  ——直播弹幕 / 礼物 / 本机脚本经**唯一** loopback 端点进主链；**B 站抓取不在主仓，
  在 Win sidecar**；含 text/token/Origin/loopback/忙碌/启停门禁/模板前缀与 curl 示例
- [**B 站弹幕 → Live2D-Ai 注入 sidecar 示例（Windows，blivedm）**](examples/bilibili-sidecar/README.md)
  ——抓 `DANMU_MSG`/`SEND_GIFT`、清洗、POST 本地端点；含房间号/SESSDATA/token 注意点与
  `SEND_GIFT_V2` 灰度缺口。**抓取不在主仓**，主仓只收已清洗文本
- [外部文字接入接口契约（历史：已归档 Python 实现，勿当现网）](architecture/external-text-input.md)
- [可观测性 / 健康自检](architecture/observability.md)
- [依赖与许可清单](architecture/dependencies.md)
- [**Mod 产品链路**](architecture/mod-product-chain.md)
- [**已封存 Mod 台账（ARCHIVED：wallpaper / pet-desktop）**](architecture/ARCHIVED-mods.md)
  ——与 `local-llm` 的 DEPRECATED 同口径：**不再注册、不再编译进 binary**；crate 暂留 workspace
  可编译可测，**禁止挂回**；含恢复条件与理由
- [**语音转写契约：`POST /api/v1/voice/transcript`（0.2.0-rc.3）**](voice-input.md)
  ——语音 → 文本 → `clean_transcript` → `say`；**ASR 本体在 sidecar**（`docs/examples/voice-sidecar/`，Win/本机进程），Rust 侧只收已转写文本
- [**会话记忆 Mod v0（0.2.0-rc.3，缺省停用；产品级加强波次补 `records`/`clear`）**](architecture/memory-mod-v0.md)
  ——本地 JSONL + 词元重叠检索 top-k → `apply_settings` 写 `persona.system_prompt`；**只对下一轮生效**，与 persona 是 last-writer-wins；面板可见条数/命中/清空
- [**角色卡 Mod（persona）：导入 / 生效 / 还原（产品级加强波次）**](architecture/persona-mod-v0.md)
  ——导入卡（粘贴 JSON 或选 `.json`/带 `chara` 的 `.png`）→ 写回主链 `system_prompt` → 停用还原基线；`GET /mods/persona/state` 由 503 升为 **200**；与 memory 仍是 last-writer-wins
- [壁纸 Mod 策略 v0（**已封存 ARCHIVED**，本波不做）](architecture/wallpaper-mod-v0.md)
  ——**不再注册、不再编译进 binary**；用户手动的舞台/壳背景（`DisplayPrefs` / `stage-bg`）
  **保留**，与被封存的 wallpaper **Mod** 是两回事；理由见 [已封存 Mod 台账](architecture/ARCHIVED-mods.md)
- [桌宠窗口 Mod v0（**已封存 ARCHIVED**，本波不做）](architecture/pet-desktop-mod-v0.md)
  ——**不再注册、不再编译进 binary**；crate 暂留 workspace，**禁止挂回**；理由见
  [已封存 Mod 台账](architecture/ARCHIVED-mods.md)
- [**背景透传：相对 `shalldie/vscode-background` v3.1.0 的偏离说明**](architecture/background-parity-vscode-background.md)
  ——**机制采纳**（分区 × 有序图列表 × 渲染参数），**实现不采纳**（不改宿主 `workbench.html`、
  不 sudo 提权、不屏蔽 integrity 提示）；逐条写明在线图 / 本地文件夹 / `~` 与环境变量 /
  任意 CSS / `useFront` / 舞台分区**为什么不做**；§6 短期目标表带**实施状态**（写「已支持」必附实测名），
  §7 是 **DEC-1…DEC-7 裁决结果**与实施波次（DEC-3 壳内子区域＝不做）
- [**Mod 社区许可与注册边界（0.2.0-rc.1）**](architecture/mod-community-license.md)
  ——注册面开放、分发面 AGPL 兼容；闭源走商业许可/私用；**无「闭源可进默认包」承诺**
- [**导演（director）最小骨架（Wave 3：已注册、缺省停用、零投递）**](architecture/director-mod-v0.md)
  ——只读 `TurnPrompt`/`TurnEnded` → 确定性 `{emotion,intent,suggested_tts}` 决策，只写日志 + `state_json`。
  **现状更正（2026-09-21）**：Wave 3 时写的「**不投递**任何动作 / TTS 参数」**已变更**——编译期
  `default_mods_manifest` 仍不收录它（**缺省停用**成立）；本工作树运行配置 `mods.json` 里启用后，
  它产 `latest.preset_id`（只读状态面）与按句 `action_cue`（WS，**驱动舞台**），只是不经 core reducer。
  现状见 `AGENTS.md` §「动作与表演的现行状态（2026-09 实测）」。设计契约仍见
  [director-rfc.md](architecture/director-rfc.md)
- [**动作包 v0：包 = 五官 + 小幅头身；intensity 是一等公民（2026-09-23）**](architecture/action-packs-v0.md)
  ——v3 合并（sad+angry → `unhappy` 按 intensity morph；happy 并 bounce；surprised 并 recoil；
  nod/shake 各并强弱档）；主 allowlist 19 → 10；旧 id deprecated 映射**一版**；
  表情槽 + 手势槽**可同轮**（微笑/unhappy + 点头）；含 N.E.K.O 五情对照与三档验法
- [插件 / 扩展 SDK 最小骨架（历史；已被 Mod 产品链路取代）](architecture/plugin-sdk.md)
- [Linux（WSL2）PC 主力环境](architecture/linux-dev.md)
- [Phase-0 架构评估](architecture/Phase-0-architecture.md)
- [渲染算法](architecture/renderer-algorithm.md)
- [Mask FBO 渲染算法](architecture/mask-premake-algorithm.md)

## 版本与发布

- [**v0.2.4-rc.1 — 本机出声接上、待机开关、设置草稿、渲染档位、TTS 生命周期**（当前）](releases/v0.2.4-rc.1.md)
  ——MeloTTS 可复现安装且 8091 真的应答；待机开关连每帧摆动一起停、物理并入导演角度；本机偏好改草稿、底部一次「保存并重载」；档位 = 画布最长边 + 运行期降级；本地 TTS 子进程装 `PR_SET_PDEATHSIG`。`v0.2.3-rc.2` 仍在原处，不移动。
- [v0.2.3-rc.1 — 开箱即用：白模型 / MeloTTS / CosyVoice3 封存 / 不带 Key](releases/v0.2.3-rc.1.md)
  ——白模型进仓库（克隆后舞台直接渲染）；`local-tts-melo` 缺省启用（`with_app`）；出厂 `[tts]` 指向 `127.0.0.1:8091/v1`；CosyVoice3 移出注册表并封存；仓库不带 LLM Key。
- [v0.2.1-rc.1 — 工程债清零（结构 / 离线 / 安全）+ 首次真实浏览器全量验收](releases/v0.2.1-rc.1.md)
  ——字体回落**结构性离线化**（注入子集外字符跨源请求 **0/0**；未镜像字族离线 = 豆腐块，取舍写明）＋结构硬指标
  （Rust `>1000` **4 → 0**、Dart `>800` **7 → 2** = PLAN 目标）＋探针判据修复与**首次真实音频链路验收**
  （131 audio 帧 / start 1 · end 1 / blob `<audio>` `currentTime` 前进 1.83 s）＋CI 三红线门禁与依赖 **25 → 22**
  ＋**历史清洗**（公开历史里写死的真实凭据退场，详见其 §6）；测试 cargo **1302 → 1311**、flutter **1524 → 1580**
- [v0.2.0 — 0.2.0 收口 + 去臃肿（第一轮）](releases/v0.2.0.md)
  ——文档治理（`docs/plans` **118 → 24**、AGENTS.md 单一化、CHANGELOG 停用）＋正确性与诚实性（45 条前端审计里
  剩余可修的 12 条逐条红-绿闭环，测试 **1378 → 1524**）＋去臃肿第一轮（度量门禁含 CI 三条硬门禁、
  **休眠资产两段式清理**、测试侧 8 份副本 → 1）；**没做**：D2 结构拆分 / D4 收尾 / D5 文档瘦身 / D6 测试治理（见其 §6）
- [v0.2.0-rc.7 — 正确性与诚实性：审计 A 组主发现 + 4 条假绿灯 + 提示词接地](releases/v0.2.0-rc.7.md)
  ——`F-0005-2` **重建放大链**（设置分区不再随每个 `text_delta` 重建，**+10 → 0**，有计数断言）；
  4 条假绿灯改成可失败断言（守卫对象改 `ui/stage_host.dart` 并剥注释 / 空泡真判据并修实现 / 行为断言 / 真泵 `StatePill` 对照组）；
  仓库卫生 worktree 25 → 3、tag 补齐 rc.2 · rc.3
- [v0.2.0-rc.6 — 背景透传追平参考 + 背景域审计收口](releases/v0.2.0-rc.6.md)
  ——`imageFit` **四档**（cover/contain/stretch/tile）+ `tileSize`、**逐图样式覆盖**、全局 `background.enabled`；
  DEC-1…DEC-7 全部落点（含 DEC-3「壳内子区域不做」）；背景域审计 10 条（解码备忘化 / 偏好变更不再全量重发 `stage-bg` / 水合不再整体覆盖偏好）
- [v0.2.0-rc.5 — Stage A：资产守护网 + 09-27 前端重设计并入 + 背景 P0](releases/v0.2.0-rc.5.md)
  ——A1–A8 接触点清单 + **19 条守护断言**；112 个 Flutter 文件重放并入 0.2.0 线（解 29 处冲突）；
  背景 P0-1…P0-5 全修；15 条审计复现转正
- [v0.2.0-rc.4 — 表演协议 v1 全链 + 导演可观测 + 单模型动作强度](releases/v0.2.0-rc.4.md)
  ——动作从「两条驱动通道打架」收成 WS `action_cue` 一条；`speak` 退役为 `segments`（只切分、逐字不变）；
  三表演字段 + 音频时钟 + 事件级 ack + 会话 baseline；dev_mode「导演可观测」四栏；`[action.models.<id>]`
- [v0.2.0-rc.3 — Wave 2 五轨合成：语音 sidecar / 壁纸接线 / 记忆 / 导演 RFC / 桌宠](releases/v0.2.0-rc.3.md)
  ——两条能力从「能编译」变「能演示」：`POST /api/v1/voice/transcript` + 可跑 sidecar（`--dry-run` / `--selftest`）；壁纸决策真落 `DisplayPrefs`（不再 warn+false）；新增 `memory` crate（本地 JSONL + 检索注入下一轮）；导演 RFC 契约先行（**不注册**）；桌宠配置/事件态进 API 可测面。`AVAILABLE_MOD_FACTORIES` 5 → **6**，主链皮肤一行未改
- [v0.2.0-rc.2 — Wave 1 三轨合成：voice-input + wallpaper + persona-polish](releases/v0.2.0-rc.2.md)
  ——把三条并行 Mod 轨道合成一条集成分支：`AVAILABLE_MOD_FACTORIES` 3 → **5**
  （+ `voice-input` / + `wallpaper`，均**缺省停用**），`mod_count_is_five` 守住数字；
  persona 坏配置**显式 `Failed`**（不再假报「运行中」）；wallpaper 决策落点仍是**占位**
  （未碰 wasm / framebuffer）；主链皮肤一行未改
- [v0.2.0-rc.1 — Mod 纪元第一基线：external-input 直播刚需 + 社区许可](releases/v0.2.0-rc.1.md)
  ——主链皮肤冻结不回归：外部事件（B 站弹幕/礼物）由 **Windows sidecar** 抓取清洗后
  经 `POST /api/v1/external/chat` 注入；`external-input` Mod 加强（静态 `settings_spec`、
  模板/前缀、启停门禁、token env→config 回落）；**`local-llm` 废除启动**（移出注册面）；
  Mod 社区许可一页说清；版本 0.1.0-rc.5 → **0.2.0-rc.1**
- [v0.1.0-rc.5 — 壳全局背景 + 与舞台同步](releases/v0.1.0-rc.5.md)
  ——壳（聊天 / 侧栏背后）铺一层固定 0.15 透明度的**全局背景**，默认与舞台背景图
  共用同一张图（`DisplayPrefs.shellImage` / `syncShellStageBg`）；只住 localStorage，
  不写 toml、不做分区背景；主链一行未改
- [v0.1.0-rc.4 — Mod 产品链路 + 主链人设收敛](releases/v0.1.0-rc.4.md)
  ——Mod 从骨架变产品链路（mods.json 持久化 / `settings_spec` 表单 / 模板 + api_version 门禁 /
  一等 `apply_settings` / 脱敏设置读取）、酒馆角色卡抽成**第一条标准 Mod**、
  主链 `[persona]` 只留 `system_prompt` + `max_history_pairs`、Win 舞台背景图修复
- [v0.1.0-rc.3 — 结构质量](releases/v0.1.0-rc.3.md)
- [v0.1.0-rc.2 — 第二基线](releases/v0.1.0-rc.2.md)
  ——动作层**删到底**（director Mod / Action 注入 / 渲染面编舞全删，core 子系统明文休眠）、
  **模型库闭环**（单一模型根 + 激活即换皮 + 导入入口）、**`.env` = 唯一密钥真源**（前端可写 + 热重载）、
  推理模型**思考折叠区**与输出上限修正、旧 JS 预览隔离；含点火记录与已知问题
- [v0.1.0-rc.1 — 核心链路基线](releases/v0.1.0-rc.1.md)
  ——首个对外 RC：版本线由 `0.5.1` **重置**为 `0.1.0-rc.1`；LLM 工具层与动作系统整体拆除、
  音频改走 `<audio>`+WAV、三个真缺陷修复、公开历史**重新起算**（单根提交）的原因与保全方式
- [v0.3.0](releases/v0.3.0.md)（历史）
- [v0.2.0-preview.1](releases/v0.2.0-preview.1.md)（历史）

## 规划

> **2026-10-06（E8 文档减量）**：2026-10-01 归档进 `legacy/plans/` 的 94 份计划 + Python/Android
> 时代文档，连同 12 份已收口计划共 **139 份 / 31,509 行**，已**移出工作树**——归档只解决「活 / 历史混放」，不减行数；
> 行数只能靠**真删**。它们**逐字保存在 git 历史里**，取回方式见
> [已移出工作树的文档索引](REMOVED-docs-index-2026-10-06.md)；生命周期规则见 [DOC-MAP.md](DOC-MAP.md) §2。

- [**执行计划（2026-10-01）：0.2.0 收口 + 去臃肿 / 债务清零**](plans/PLAN-debloat-and-closeout-2026-10-01.md)
  ——**口径**：项目太大太臃肿，技术/工程债要收、代码精简与优化排上日程。**现在**：S0 文档整理 →
  rc.8-a 正确性与诚实性（9 条 P1）→ rc.8-b 结构 → 0.2.0 末版；**随后**：去臃肿 D0–D6
  （度量门禁 / 休眠裁决 / 结构拆分 / 重复消除 / 依赖瘦身 / 文档瘦身 / 测试治理）。
  目标：Rust 生产 −20%、>1000 行文件归零、休眠代码归零、`desktop` 依赖 34→≤22、`docs/plans` 115→≤25
  （**2026-10-01 归档后顶层 = 24，已达标**）。
  含**体量实测基线**（Rust 67,806 / Dart 33,476 / docs 69,733，2026-10-01 立档口径；
  **归档后复算见 [DOC-MAP.md §3](DOC-MAP.md)**）与**不可牺牲的红线清单**。
- [**文档地图与生命周期（2026-10-01）**](DOC-MAP.md)
  ——「这个问题看哪份文档」真源地图 + 活/历史/作废**三分** + 命名规范 + 归档规则与
  **94 份候选（2026-10-01 已执行，顶层 118 → 24）**；
  **新会话先看这份**，可避免 `-Ai` 旧 doctrine 与 `-fe` 现行口径混淆。
- [**开工提示词 · 主 leader（2026-10-01）：0.2.0 收口 + 去臃肿**](plans/ORCHESTRATOR-PROMPT-debloat-round-2026-10-01.md)
  ——**把本文件整块粘贴给 team 的主 leader**：角色边界（编排/复核/提交，不写业务码）/ 环境事实 /
  波次表（文件零重叠）/ 派发模板 / 公共前置 / **红线 8 条** / 验收协议（红-绿 + 非实施者复核 + 判别力自证）/
  **6 项必须停下问维护者的裁决项** / 提交与 tag 规范 / 磁盘纪律。
- [**实现提示词 · 去臃肿轮（2026-10-01）：W-S0 / W-D0 / W-R8a-1…5 / W-D1…D6 / W-VERIFY**](plans/IMPL-PROMPTS-debloat-round-2026-10-01.md)
  ——每个 worker 一块：文件归属 / 必做 / 禁做 / **验收测试（planner 指定）** / 证据 / 回报；
  含「planner 拥有测试规格、worker 实现、verifier 确认」的所有权说明。
- [**交接说明（2026-10-06 夜）：工作树单一化 + E1 拆分 + 台账 6 条 P1 全清**](plans/HANDOFF-2026-10-06-e1-e5-debt-round.md)
  ——**接手先读这份**：一分钟上手（HEAD `08338f3`、工作区 40 项改动**未提交**）/
  E1（守卫静默漏扫 20 处 + `main.dart` 1417→657）/ E5 六条 P1 的修法与**红-绿双向自证门禁数字** /
  下一轮顺序（E9 探针稳健性 → E8 → …）/ **待维护者裁决的 E2（类分解）与 E4（产物预算）** /
  本轮踩到的 8 个坑 / 交接检查清单
- [**交接说明（2026-09-28 晚）：rc.5/rc.6/rc.7 已收口 —— 下一步 rc.8 + 全代码库审计**](plans/HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md)
  ——**接手先读这份**：一分钟上手（HEAD `932ea5d4` / tag rc.1–rc.7 / 门禁 1457 · 1378）/
  本轮只读复核的独立核验结果 / **必须修的 6 条文档债** / 待裁决 4 件 / rc.8-a·rc.8-b 波次 /
  `--ff-only` 合流路径 / 未验收的肉眼 8 项 / 6 个已知坑
- [**全代码库只读审计提示词（长跑版 · 弱模型可用，2026-09-28）**](plans/AUDIT-PROMPT-whole-repo-2026-09-28.md)
  ——整块粘贴给审计进程：范围含 `crates/**`（291 文件 / 97.5k 行，**此前从未被系统审计**）+
  `shell/flutter/**` + scripts/CI；账本 `AUDIT-REPO/`（未跟踪；**2026-10-06 复核：该账本已入库为 `docs/audit/2026-10-05-ledger/`**）；小批次 · 机械队列 · 键值块产出（不用表格）/
  红线 K–R / 已知已登记清单（45 条前端发现 + Mod 侧 S1–S6 等）/ 五阶段永动队列 / 对抗日
- [**全代码库只读审计提示词（长跑版 v3.2 · 2026-10-06，取代 09-28 版）**](plans/AUDIT-PROMPT-whole-repo-2026-10-06.md)
  ——锁 `main` @ `6be9984`（**不是** `feat/frontend-redesign`，它落后 28 个提交、领先 0）；**第一优先是前端设置 + Mod 面**
  （`Phase 0` 九批 + `§7.1` 用户侧审视 + 设置项三方对账）；台账 `AUDIT-REPO/`（未跟踪；上一轮台账已入库 `docs/audit/2026-10-05-ledger/`），批次从 `BATCH-1001` 起
  （上一轮已占用 0001–0927）；含「死树 AGENTS 注入」陷阱、账本禁止写回敏感原文、清洗后遗痕轴（旧 SHA 断链）
- [**动作 / 表情 / 导演链路调研（4 项症状 → 根因 → 下一轮任务，2026-09-21）**](plans/RESEARCH-actions-director-audit-2026-09-21.md)
  ——**本轮 W1–W11 的调研真源**：4 项症状的 `file:line` 根因、幅度死区量化表、与 `director-rfc.md`
  的 RFC 冲突表（§3.2）与脱轨清单（§3.4）；口径裁决在 §3.6–§3.8；**§8 目标链路 / §9 时间轴对齐 / §10 输出契约与字段分解是维护者已确认的规格**。⚠ **§3.4 的 D1 与由 R2 推出的
  「导演属场景 Mod / 主链不该有第二 LLM」均已撤回——引用必须连撤回标记一起引，见 §3.6/§3.7**
- [**调度提示词：动作 / 表情 / 导演链路修复（W1–W11 十一块）**](plans/ORCHESTRATOR-PROMPT-actions-performance-round.md)
  ——**§8 首次启动 / §9 Wave 1 续作 / §10 Wave 2 续作**，可直接整块粘贴给编排者
  ——文件归属 / 波次与「文件零重叠」约束 / 验收底线 / 给执行者的红线（**本轮调度真源**；
  产品形态口径见其 ★ 段，与 RESEARCH §3.7 同）
- [**可复制实现提示词（W1–W11）：动作 / 表情 / 导演链路修复**](plans/IMPL-PROMPTS-actions-performance-round.md)
  ——每块自带公共前置（门禁 / 硬约束 / 回报格式），供 worker 整块粘贴（**本轮任务真源**）
- [**交接说明（2026-09-21）：动作 / 表情 / 导演链路 —— 已冻结裁决 + Wave 0/1 验收 + 未完成工作**](plans/HANDOFF-2026-09-21-actions-performance-round.md)
  ——**接手先读这份**：一分钟上手 / 不得翻案的裁决清单 / 门禁基线 / W7–W11 与 flaky、TTS 缺口 / 现状快照
- **历史计划与交接（Python/Android 双端时代 + 2026-08~09 已收口轮次）**
  ——含 2026-10-01 归档的 **94 份计划**、`mod/product-grade` / `mod/wave3` / stabilize 的收束报告与
  点火验收清单、rc.2 第二基线交接、前端加强计划、Rust 重建 RFC、节点 A 接线前交接等。
  **2026-10-06（E8 文档减量）起全部移出工作树**：不再占工作树行数，内容逐字保存在 git 历史里，
  逐份取回方式见 [已移出工作树的文档索引](REMOVED-docs-index-2026-10-06.md)。

## 调研

- [**导演层同类对照：N.E.K.O. 与 12 个项目（2026-10-07，导演层讨论）**](research/调研结果-导演层同类对照-2026-10-07.md)
  ——N.E.K.O. 是「独立小模型给**角色回复**贴 5 情绪 + 回合收尾触发」；多数同类走主 LLM 内联标签；
  成熟的只有仲裁层（VTS P0–P5 / Cubism priority / Warudo layer）。含本仓 `0.2.2` 导演对照与「可借鉴 / 不抄」清单
- [**AI-Vtuber（Ikaros-521）对照：差异与可借鉴（2026-10-07，导演层讨论）**](research/调研结果-AI-Vtuber对照-2026-10-07.md)
  ——对方没有情绪导演；Live2D 页是静态样例，身体靠外挂程序吃音频 URL。可借鉴的是事件种类、插队准入和短窗口收束，都放在进 `TurnPrompt` 之前
- [竞品对比](research/benchmark-neko-vs-neurosama.md)
- [N.E.K.O UI 对标与「最小高级感」缺口清单](research/neko-ui-alignment-and-gaps.md)
- [Live2D 半身动作/情绪表演开源检索：nanlingyin 系 + soullink-emotion-sdk 深读](research/live2d-halfbody-motion-research.md)
- [半身控制·无预设皮套路线专项（三层契约/PR#3/Neuro 系对照，2026-08）](research/live2d-halfbody-presetfree-control.md)
- [AI 控制 Live2D 皮套：控制通道与生态补充调研（2026-08）](research/live2d-ai-control-ecosystem.md)
- [Neuro-sama 架构](research/NeuroSama-architecture-research.md)
- [N.E.K.O / Live2D 调研](research/NEURO_LIVE2D_RESEARCH.md)
- [TTS 调研](research/tts-research-report.md)
- [**TTS 选型再评估：8 GB 笔记本上的显存账（2026-10-08）**](research/tts-vram-selection-2026-10-08.md)
  ——实测 CosyVoice 3 常驻 4.7 GB / 峰值 5.2 GB、桌面侧 2.1–2.7 GB、TTFA 2.0–9.8 s（验收线 ≤1 s）；给出止血项、CosyVoice2-0.5B 主线与 0 显存（CPU / 云端）两档
- [**本地 TTS 显存与「本地 TTS Mod」对话记录（2026-10-08）**](research/tts-engine-mod-conversation-2026-10-08.md)
  ——维护者口径：Mod 只做「注册 + 拉起外挂 TTS + 暴露 API + TTS 选择里多一个本地选项」，TTS 端点也可走云端；含与 `tts-is-core.md` 现行裁决的接缝与待裁决项，只记录不实施
- [Flutter × Live2D 开源实现调研（2026-09，渲染面/消息桥/许可）](research/flutter-live2d-implementations-2026-09.md)
- [许可报告](research/license-report.md)

## 验收与验证

- ~~[v0.2.0 收口 · 肉眼验收勾选表（2026-10-01 立）](verification/v0.2.0-checklist.md)~~ **已于 2026-10-06 退役删除**
  ——维护者 2026-10-06 确认肉眼/听音验收通过并指示删除（它从来不是门禁）；可自动化的那部分改由
  `scripts/browser_probe.mjs` 覆盖（42 项 · fail 0），人工项现状见 `docs/plans/NEXT-ROUND-main-2026-10-06.md`。
  原文：[给**维护者**的可勾选人工验收单：壳背景 8 项 / 本轮 W1 真机项 / 资产四栏（动作·语音·口型·导演）/
  离线优先；每项带「自动化已覆盖 / 只能人工 / 需真服务或 Windows 侧」标签与**测试名**。
  **它不是门禁**——门禁 = `code-stats --check` + `cargo`/`flutter` 那两套
- [**点火自测清单（核心链路闭环，2026-09-10）**](verification/ignition-core-loop.md)
- [**浏览器音频「很吵的杂音」根因与修复（2026-09-10）**](verification/audio-playback-noise-2026-09-10.md)
  ——播放时间轴被分片起始标记重置，实测最多 17 个音源同时发声（重叠 529 对）；
  修复后 0 重叠、最大同时发声 1
- [冒烟测试清单](verification/smoke-checklist.md)
- [桌面端验证](verification/verification-win.md)
- [Rust Bakeoff 选型决策：Ayagami vs Mocari](verification/rust-bakeoff-decision.md)
  （实测报告：`verification/rust-bakeoff-ayagami.md`；未被选中候选 **mocari** 的 891 行报告已移出工作树）
- 验收报告：`verification/acceptance-metrics-report*.md`

## 代码审计

- [**前端夜间审计账本（2026-09-28 入库，封口只读）**](audit/2026-09-28-frontend-nightly/README.md)
  ——**长跑 21 批 45 条为准**（P0 0 / P1 12 / P2 19 / P3 14）＋ `short-run/` 快照；复算命令见其 §3；**目录只读**，实施记录写到 `docs/audit/<date>-<wave>/`
- [2026-08-20 可读性 / 高效性审计](audit/2026-08-20-readability-efficiency-audit.md)
- [2026-08-19 代码审计与管理收尾](audit/2026-08-19-code-audit-round.md)
- 分项：`audit/CONFIG_SCHEMA_AUDIT.md`、`audit/SETTINGS_SYSTEM_AUDIT.md`、`audit/tts_config_audit_report.md`

## 截图

- `docs/screenshots/`（历史验收截图）

## 部署

- [Headless / Termux 部署指南](headless-deploy.md)

## 根文档

- [README.md](../README.md)
- [CHANGELOG.md](../CHANGELOG.md)
- [AGENTS.md](../AGENTS.md)（**AI/协作者入口，现行**）
- 归档（Python/Android 双端时代 + 94 份已收口计划，**勿当现网**）：**已移出工作树**
  ——`HANDOVER.md` / `AGENT.md` / `PLAN.md` / `PROGRESS.md` / `AUDIT.md` / `REFACTOR_*.md` 与
  `legacy/plans/` 全部在内（共 139 份 / 31,509 行）；取回见
  [已移出工作树的文档索引](REMOVED-docs-index-2026-10-06.md)
