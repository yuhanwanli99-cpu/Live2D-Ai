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

- 背景 jpg 走 wgpu 底图层（`wgpu` 底层target 合成）。
- director emotion_mapping schema / prodvision（N.E.K.O 借鉴项）。
- **Tauri 桌宠壳（F4）/ Android 悬浮窗壳（G1）**——独立壳工程，不污染核心 Rust 边界。

### R3-TBD-A：会话记忆（**已登记，待实现** — 2026-09-11 用户裁决「A 记忆之后实现」）

> 状态：**待实现**。前置条件：协议 + 端点 + 服务端持久化，**属于后端改动**，
> 不在「只做前端」的边界内，故先记录、后实现。

**已经做完的（前端 A 档，2026-09-11 当日交付）**：浏览器本地的**多会话记录**。
聊天记录落 `localStorage`（`live2d-ai.chat-sessions`），可新建 / 切换 / 重命名 /
删除；刷新、重开页面都还在。实现在 `shell/flutter/lib/chat/chat_session.dart`
（纯逻辑）+ `lib/ui/session_sheet.dart`（UI）+ `main.dart` 的
`loadChatSessions` / `saveChatSessions`。

**它**不**解决的**：**模型没有记忆**。跨刷新、跨会话，LLM 都不记得上一轮说过什么。
这不是前端偷懒，是协议缺字段——理由有两条，都在代码里可查：

1. `POST /api/v1/chat` 只接受 `{text}`（`crates/live2d-ai-desktop/src/web_api/chat_routes.rs`），
   **没有历史字段**，前端无从把历史喂回去；
2. 服务端 `persona.max_history_pairs` 出厂为 **0**（`live2d-ai.toml`），
   即服务端自己也不带历史。`ConversationEngine` 有
   `commit_completed_turn` / `clear_history`（`conversation/engine.rs`），
   但**没有 HTTP 端点**去读或清。

**要做的最小一套（按依赖顺序）**：

| # | 改动 | 位置 | 关键约束 |
| --- | --- | --- | --- |
| 1 | `POST /api/v1/chat` 请求体增加可选 `history: [{role, text}]`（**缺省即空 = 向后兼容**，协议 v1 规则） | `web_api/chat_routes.rs` + `runtime/dialogue/` | 老前端不发这个字段，行为必须与今天完全一致 |
| 2 | 新增 `GET /api/v1/chat/history` 与 `DELETE /api/v1/chat/history` | `web_api/mod.rs` 路由表 | 走既有 `Orchestrator` 句柄；**mutating 要 `application/json` + Origin 校验**（沿用 `security.rs`） |
| 3 | 会话级持久化（多会话、跨重启） | 新存储层 | 服务端目前**只存单份内存历史**，没有 session 概念；要么引入 sqlite/文件，要么明确「单会话」 |
| 4 | 前端把本地会话与后端会话**对上** | `chat_session.dart` | 需要 `session_id` 贯穿：切会话 = 切后端历史；两边 id 不一致时必须能安全降级 |

**开工前必须先定的两件事**（都是产品裁决，不是技术问题）：

- **要不要真正的多会话后端**？还是「一个后端会话 + 前端只做展示分组」就够？
  后者小得多（1+2，不碰 3），代价是「切会话后模型仍记得全部」。
- **历史要不要落盘到服务端**？不落盘的话重启即失忆，前端那些本地记录仍然只是
  「记录」而不是「记忆」。

**验收口径**：换个会话问「我上一条说了什么」，模型要答得出**该会话内**的内容，
且**不该**答出别的会话的内容。这条只能真机 + 真实端点验收（R2 口径）。

---

## 验收口径

| 域 | 验收 |
|---|---|
| 工程项 | `cargo test --workspace --all-targets` 全绿 + fmt/clippy/rust-ratio ≥95% |
| doc/一致性 | README/AGENTS 与 crates 实态对齐（mod 计数、crate 名） |
| 外部项 | 真机/浏览器/真实端点，可见效果为凭，不作「大绿」代偿 |

## 变更历史
- 2026-09：初稿（与平台定位对齐，分 P0 滚项 / 外部验收 / 后续三档）。
- 2026-09-11：R3 新增 **R3-TBD-A 会话记忆（待实现）**——前端「本地多会话记录」
  当日已交付（`chat_session.dart` / `session_sheet.dart`），
  模型记忆因缺协议字段与端点而**登记待实现**，并写明最小改动清单与两个待裁决点。