# Live2D-Ai 文档索引

> 本文档把 `docs/` 按用途分目录，避免所有文件堆在顶层。

## 架构与契约（当前）

- [架构总览](architecture/ARCHITECTURE.md)
- [核心契约与架构边界](architecture/core-contracts.md)
- [**CosyVoice 3 TTS 接入（2026-09-10，能力就绪/未部署）**](architecture/cosyvoice3-tts-integration.md)
- [渲染纹理 / 离屏靶标档位 ADR（4096/8192/16384）](architecture/renderer-texture-tier-adr.md)
- [PC 无预设半身联动实现](architecture/pc-presetfree-halfbody-control.md)
- [目录约定](architecture/directory.md)
- [外部文字接入接口契约](architecture/external-text-input.md)
- [可观测性 / 健康自检](architecture/observability.md)
- [依赖与许可清单](architecture/dependencies.md)
- [**Mod 产品链路（rc.4 唯一 Mod 契约）**](architecture/mod-product-chain.md)
- [插件 / 扩展 SDK 最小骨架（历史；已被 Mod 产品链路取代）](architecture/plugin-sdk.md)
- [Linux（WSL2）PC 主力环境](architecture/linux-dev.md)
- [Phase-0 架构评估](architecture/Phase-0-architecture.md)
- [渲染算法](architecture/renderer-algorithm.md)
- [Mask FBO 渲染算法](architecture/mask-premake-algorithm.md)

## 版本与发布

- [**v0.1.0-rc.5 — 壳全局背景 + 与舞台同步（当前）**](releases/v0.1.0-rc.5.md)
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
- [PLAN.md](../PLAN.md)（架构演进历史）
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
