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
- [插件 / 扩展 SDK 最小骨架](architecture/plugin-sdk.md)
- [Linux（WSL2）PC 主力环境](architecture/linux-dev.md)
- [Phase-0 架构评估](architecture/Phase-0-architecture.md)
- [渲染算法](architecture/renderer-algorithm.md)
- [Mask FBO 渲染算法](architecture/mask-premake-algorithm.md)

## 规划

- [**交接说明（2026-09-10）：核心链路闭环 + 音频「很吵」修复**](plans/HANDOFF-2026-09-10.md)
  ——接手先读本文：运行环境、门禁、后续待办、9 条关键陷阱、未提交清单
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
- [HANDOVER.md](../HANDOVER.md)
- [CHANGELOG.md](../CHANGELOG.md)
- [AGENT.md](../AGENT.md)
- [AGENTS.md](../AGENTS.md)
