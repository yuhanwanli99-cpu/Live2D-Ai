# 未来路线图（2026-09）

> 面向 **通用人形皮套 AI 接入一体化平台**（不绑定单一模型 / 不做复杂上层）的后续工作排序。
> 本文件是 P0 滚动项 + 真机/外部验收项的分层 backlog。当前实现在 `dev/integrity`，
> 各节点执行计划见 `docs/plans/`（node-f-*/ integrity-*）。
> 口径：**可门禁交付的工程项**归 P- 内；**需真机/浏览器/真实 LLM·TTS 端点**的归「外部验收」，不伪完成。

---

## R0. 文档与治理（已完成，2026-09）

- [x] 平台定位（通用人形皮套 / 非绑单模型 / 非复杂上层）落入 README / AGENTS / AGENT；旧 Python/Android
      文档加「已归档」banner。
- [x] Renderer texture-tier ADR 冻结（`docs/architecture/renderer-texture-tier-adr.md`）。
- [x] 稳定版（v0.3.0）分支合并 + tag（见 release notes）。

## R1. 可交付 P0（当前环境可门禁，滚动）

- [ ] **纹理档位实现**（按 ADR）：tier 常量 + native limits 自定义 + wasm `stage-config` 档位字段 + 前端
      下拉 + 钳制/回退单测。
- [ ] **F7 残余**：README 已诚实标注 `--chat/--pet-mode` 为骨架（见 commit），若窗口生命周期接线完成再
      恢复 CLI 描述。
- [ ] **director producer 精简**：S2 事件桥已成（`dispatch_event` 生产侧接入 cli_entry），后续把
      `TurnStarted/ActionFinished` 从包裹到显式调用整理到 `docs/architecture/core-contracts.md` 投影表。
- [ ] **行/文件一致性门禁**：新增 doc-实现一致性 check（README/AGENTS 与 mod/crate 实态对齐的断言），
      防「3 Mod vs 5 Mod」类漂移再发生。

## R2. 外部验收项（需真机 / 浏览器 / 真实端点，不 fake 视为完成）

- [ ] **S3 原生 16384 离屏 target 实例**（真机独立 GPU，见 ADR §5）。
- [ ] **S4 档位视觉验收**（真机浏览器 `/render`：档位切换→清晰度/描边变化 + 回退不 crash）。
- [ ] **F6 端到端**：`LIVE2D_AI_WS_AUDIO=1` + 真实 LLM/TTS → 浏览器发消息 → 浏览器出声 → 口型动。
- [ ] **真实 LLM 对话闭环**（角色卡 system_prompt 注入的端到端）。
- [ ] **导入第二个模型**测试换模型（依赖版权资产）。

## R3. 后续（非当前 P0，排队）

- 背景 jpg 走 wgpu 底图层（`wgpu` 底层 target 合成）。
- director emotion_mapping schema / prodvision（N.E.K.O 借鉴项）。
- chat history API（语义待定，价值低，不优先）。
- Tauri 桌宠壳（F4）/ Android 悬浮窗壳（G1）——独立壳工程，不污染核心 Rust 边界。

---

## 验收口径

| 域 | 验收 |
|---|---|
| 工程项 | `cargo test --workspace --all-targets` 全绿 + fmt/clippy/rust-ratio ≥95% |
| doc/一致性 | README/AGENTS 与 crates 实态对齐（mod 计数、crate 名） |
| 外部项 | 真机/浏览器/真实端点，可见效果为凭，不作「大绿」代偿 |

## 变更历史
- 2026-09：初稿（与平台定位对齐，分 P0 滚项 / 外部验收 / 后续三档）。