# Live2D-Ai 文档索引

> 本文档把 `docs/` 按用途分目录，避免所有文件堆在顶层。

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

- [**v0.2.0-rc.4 — 0.2.0 线最后一个 RC：表演协议 v1 全链 + 导演可观测 + 单模型动作强度**（当前）**](releases/v0.2.0-rc.4.md)
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
- [**点火验收清单（产品级加强波次 / `mod/product-grade`）：给用户在 Windows 上照单勾选**](plans/IGNITION-CHECKLIST-product-grade.md)
  ——注册面 **5 个 Mod**（wallpaper / pet-desktop 已封存）/ 机器预检 / 人机验收（含五个 Mod 的产品级可见项）/ 通过标准 / 签名栏；
  配套脚本 [`scripts/ignition-precheck.sh`](../scripts/ignition-precheck.sh)（PASS/FAIL/SKIP 表；`--fsm` 五 Mod 矩阵）
- [**产品级加强波次收束报告**](plans/PRODUCT-GRADE-CLOSEOUT.md)
  ——封存结果（FACTORIES 7 → 5）/ 五个 Mod 的产品级达成 / 门禁数字 / 最短体验路径（版本仍 `0.2.0-rc.3`，未 bump）
- [点火验收清单（stabilize，**已被取代**）](plans/IGNITION-CHECKLIST-stabilize.md)
  ——前置（含 **TTS 未起时的预期**）/ 机器预检 / 十步人机验收（操作·期望·失败先看哪）/ 通过标准 / 签名栏；
  配套脚本 [`scripts/ignition-precheck.sh`](../scripts/ignition-precheck.sh)（PASS/FAIL/SKIP 表）
  与实跑记录 [`STABILIZE-PRECHECK-RESULT.md`](plans/STABILIZE-PRECHECK-RESULT.md)
  （Wave 3 之后的**稳定化小修**：修「前端 Mod 管理启停恒 415」，见
  [`STABILIZE-CLOSEOUT.md`](plans/parallel-mods/STABILIZE-CLOSEOUT.md)；版本仍 `0.2.0-rc.3`）
- [**Wave 3 收束报告：七个已注册 Mod 的日常闭环（2026-09-14，未发布 / 无版本变更）**](plans/parallel-mods/WAVE3-CLOSEOUT-2026-09-14.md)
  ——每轨 tip / 闭环证据 / 未决 / `v0.2.0-rc.3` §8 逐条处置 / 与将来 rc.4 的差距；
  协议见 [`PARALLEL-WAVE3-2026-09-14.md`](plans/parallel-mods/PARALLEL-WAVE3-2026-09-14.md)
  （基座 `ModEventTopic::TurnEnded`；`AVAILABLE_MOD_FACTORIES` 6 → 7，版本仍 `0.2.0-rc.3`）
- [**交接说明（2026-09-13）：rc.2 第二基线 —— 动作层删到底 + 模型闭环 + `.env` 密钥真源 + 推理模型思考**](plans/HANDOFF-2026-09-13-rc2-second-baseline.md)
  ——**接手先读本文**：一分钟上手、13 个提交的清单、门禁数字、交付态实测（含无头浏览器七项证据）、
  故意推到 rc.3 的事、下一轮建议顺序，以及**本轮新踩的七个坑**
  （「语义树不是像素」「自检与链路抢资源 → 自检说谎」「推理模型的思考与 max_tokens 共享」
  「正文上屏的闸门是 SentenceVoiced」「别用 taskkill /IM chrome.exe」…）
- [交接说明（2026-09-11 晚）：核心链路基线 —— 工具/动作拆除 + 音频走媒体元素 + 三个真缺陷](plans/HANDOFF-2026-09-11-core-chain-baseline.md)
  ——rc.1 的接手入口（平台与音频那一层仍然有效）：LLM 工具/动作系统为何整体拆除、
  音频为何从 Web Audio 改走 `<audio>`+WAV、三个真缺陷的根因、**七个必须知道的坑**
- [基线说明：核心链路（2026-09-11）](architecture/core-chain-baseline.md)
  ——链路逐环与出处、已移出链路的、刻意保留的、一键验证与真机点火看哪七项证据
- [**交接说明（2026-09-11）：前端重做 + 真机验收 + 错误可观测性（v0.4.13 → v0.5.1）**](plans/HANDOFF-2026-09-11.md)
  ——同一日的前半段（前端重做与真机验收十二个 bug）；其 §12 的未提交清单**已完成**
- [**交接说明（2026-09-10）：核心链路闭环 + 音频「很吵」修复**](plans/HANDOFF-2026-09-10.md)
  ——上一版交接（历史；运行环境与 9 条陷阱仍有参考价值）
- [**前端加强计划：参考 Morrow 前端（2026-09-11，已完成）**](plans/PLAN-frontend-strengthening-2026-09-11.md)
  ——搬「观感机制」（材质插值 / 折叠面板 / 页面过渡 / 视觉审查工具），**不搬**其断点与时长
  散值、也不搬 `BackdropFilter` 玻璃；含逐文件复用判定、P0–P4 分期
- [**音频路径方案：后端出 WAV、前端 `<audio>` 播放（2026-09-11，已实施）**](plans/PLAN-audio-wav-path-2026-09-11.md)
  ——为何用 WAV 不用 mp3（本机 TTS 实测 `mp3`/`wav` 均 400）、为何媒体元素才吃站点级静音
- [**点火计划：核心链路闭环 + 本地 Mod（2026-09-10，当前执行口径）**](plans/PLAN-ignition-core-loop-2026-09-10.md)
- [**Rust 重建 RFC**](plans/RUST-REWRITE-RFC.md)
- [**未来路线图（2026-09，P0 滚项/外部验收/后续）**](plans/future-roadmap-2026-09.md)
- [**接手修复与渲染档位计划（2026-09-07，S1/S2 完成）**](plans/integrity-takeover-fix-plan-2026-09-07.md)
- [**节点 A 接线前审计交接（高级 Agent 裁决用，2026-08-26）**](plans/node-a-wiring-audit-brief.md)
- [**PLAN-V2-PC-LOCAL-TTS.md（v1 完成计划，仅 PC 端，含本地 Melo TTS）**](plans/PLAN-V2-PC-LOCAL-TTS.md)
- [**PLAN-V3-SOULLINK-PERFORMANCE.md（表演引擎复用实现计划：直接引 MIT 包，少写代码）**](plans/PLAN-V3-SOULLINK-PERFORMANCE.md)
- [PLAN-V1.md（上一版 PC 计划，已被 V2 取代）](plans/PLAN-V1.md)
- [PLAN.md（Python/Android 双端时代的架构演进历史，已归档）](legacy/PLAN.md)
- 历史计划：`plans/plan-*.md`、`plans/plan-task-*.md`、`plans/PLAN-PHASE1*.md`、`plans/PLAN-PC-V1~V4`、`plans/PLAN-V1-draft-2026-08-21.md`
- [Phase-0 notes](plans/Phase-0-notes.md)

## 调研

- [竞品对比](research/benchmark-neko-vs-neurosama.md)
- [N.E.K.O UI 对标与「最小高级感」缺口清单](research/neko-ui-alignment-and-gaps.md)
- [Live2D 半身动作/情绪表演开源检索：nanlingyin 系 + soullink-emotion-sdk 深读](research/live2d-halfbody-motion-research.md)
- [半身控制·无预设皮套路线专项（三层契约/PR#3/Neuro 系对照，2026-08）](research/live2d-halfbody-presetfree-control.md)
- [AI 控制 Live2D 皮套：控制通道与生态补充调研（2026-08）](research/live2d-ai-control-ecosystem.md)
- [Neuro-sama 架构](research/NeuroSama-architecture-research.md)
- [N.E.K.O / Live2D 调研](research/NEURO_LIVE2D_RESEARCH.md)
- [TTS 调研](research/tts-research-report.md)
- [Flutter × Live2D 开源实现调研（2026-09，渲染面/消息桥/许可）](research/flutter-live2d-implementations-2026-09.md)
- [许可报告](research/license-report.md)

## 验收与验证

- [**点火自测清单（核心链路闭环，2026-09-10）**](verification/ignition-core-loop.md)
- [**浏览器音频「很吵的杂音」根因与修复（2026-09-10）**](verification/audio-playback-noise-2026-09-10.md)
  ——播放时间轴被分片起始标记重置，实测最多 17 个音源同时发声（重叠 529 对）；
  修复后 0 重叠、最大同时发声 1
- [冒烟测试清单](verification/smoke-checklist.md)
- [桌面端验证](verification/verification-win.md)
- [Rust Bakeoff 选型决策：Ayagami vs Mocari](verification/rust-bakeoff-decision.md)
  （实测报告：`verification/rust-bakeoff-ayagami.md` / `rust-bakeoff-mocari.md`）
- 验收报告：`verification/acceptance-metrics-report*.md`
- 测试任务：`plans/test-*.md`

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
- 归档（Python/Android 双端时代，**勿当现网**）：[docs/legacy/](legacy/README.md)
  —— `HANDOVER.md` / `AGENT.md` / `PLAN.md` / `PROGRESS.md` /
  `AUDIT.md` / `REFACTOR_*.md` 已搬进 `docs/legacy/`
