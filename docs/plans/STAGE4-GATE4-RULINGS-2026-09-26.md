# 阶段4 Gate 4 裁决与 W4f 清单（2026-09-26）

> 落盘人：顶层管理与代码审查。工作树 = `/home/skystar/Live2D-Ai-l1`（分支 `mod/l1-product`）。
> 前置：4a 契约冻结 `f703d0e5`。本文件 = **Gate 4 提交记录 + 对 4b–4e 未决项的裁决 + W4f 范围**。
> **不 bump / 不 push / 不打 tag**；Gate 5（并入 main）**在 W4f 收口前不得执行**。

## 1. Gate 4 已落（维护者执行）

| 提交 | 主题 | 文件数 |
| --- | --- | --- |
| `2a15a51c` | `feat(performance): 表演协议 v1 runtime 切分（阶段4b）` | 8 |
| `b7339dd6` | `feat(render): 字段化通道 + 音频时钟 + 事件级 ack（阶段4c）` | 11 |
| `08389e54` | `refactor(flutter): 消费 ack / 取消纪律 / 停发猜时长（阶段4d）` | 15 |
| `a3870ffd` | `feat(session): baseline 绑会话 + 收口（阶段4e）` | 7 |

41 条未提交路径全部归轨，工作树 `porcelain=0`。**无越权**（RED=0）。

## 2. 维护者独立复核（亲跑，非采信）

| 检查 | 我的读数 | 与报告 |
| --- | --- | --- |
| `cargo test --workspace --all-targets` | **1444 passed / 0 failed** | 一致 |
| `cargo test --doc` | 3 passed | 一致 |
| `cargo fmt --all -- --check` | clean | 一致 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** | 一致 |
| `xtask rust-ratio` | **97.3353% PASS** | 一致 |
| `flutter analyze` / `flutter test` | No issues / **1063 passed** | 一致 |
| C1 路径核对 | 41 条，授权面内 | 一致 |

**采信报告（需活服务/浏览器，未重跑）**：C5 两配置驱动实测、C6 `verify_core_chain` 17/17、C7 六条负对照、HUD/ack 原始序列。负对照逐条附四段原文且以 sha256 证明还原，**纪律达标**。

## 3. 裁决（D27–D36）

| # | 事项 | 裁决 |
| --- | --- | --- |
| **D27** | W4b 用**控制字符信封**（`\u{1}v1\u{1}` + JSON 塞进 `preset_id`）承载 v1 新键 | **不接受为终态**。根因是当时 `mod-director/src/presets.rs:241` 属 4e 独占、无法同时加字段——这是时序问题，不是设计理由。**W4f-1 必修**：给 cue 结构体加显式可选字段、改那处字面量构造、删掉 encode/decode；**wire 输出逐字不变**（golden 帧回归）。**Gate 5 前置**。 |
| **D28** | 前端 `_requestSessionBaseline` 仍是**空实现** | **W4f-2 必修**：授权 `shell/flutter/lib/{main.dart,live2d/**,api/**}` 消费 host 交付的 baseline（chat 响应 `baseline` + `action_cue{baseline:true}`）并立即应用；空 baseline → 等价于 revoke 回待机（现状保留）。**Gate 5 前置**（半接线不留进 main）。 |
| **D29** | 服务端「新消息 → 清 TTS 待播」未注入（需动禁区 `supervisor.rs`） | **不做**，降为 **P2 backlog**。理由：前端 `dropPendingAudio` 已中断待播、epoch 丢弃陈旧帧，**用户可感行为正确**；为资源整洁去碰禁区不划算。若日后要做，另立一轮 + supervisor.rs 例外授权 + 专用回归。 |
| **D30** | 前端→渲染面字段 cue **复用 `preset` 消息**（编排者钉死，O13 未覆盖） | **接受**，已补进协议 §12.3（只增不改） |
| **D31** | host 取消信号 = 既有 `action_cue` 加 `baseline:true`+`reason` | **接受**，已补进协议 §12.3 |
| **D32** | 授权表三处缺陷（`settings.rs` 不存在 / `ws_frame.dart` 漏列 / 3 个 `preset/field_*.rs`） | **全部追认授权**；`STAGE4-plan` §3 已更正（`settings_routes/**`） |
| **D33** | `performance_id_not_allowed` 为协议外 warn 码 | **接受**为正式码，已补协议 §12.3 |
| **D34** | stage-clock 不外推（两条 30ms 间同值） | **暂接受**；由 Windows 肉眼观感定夺 |
| **D35** | 真机 bai 13 参齐全 ⇒ 整条 `preset-dropped` 只能由原生回归 + 负对照覆盖 | **接受**为已知证据边界（与 D19 同类）；不算缺陷 |
| **D36** | 18080 上有**陈旧服务 PID 59888**（先于本轮全部改动启动） | **必须重启**再做任何肉眼验收；否则「看到的不是本轮代码」。**维护者未擅自 kill/重启**（用户未要求起服务）。 |

## 4. W4f（收口前最后一轮，两轨并行）

| Worker | 独占文件 | 任务 |
| --- | --- | --- |
| **W4f-1（Rust）** | `crates/live2d-ai-runtime/src/performance/{plan.rs,tests.rs}`、`crates/live2d-ai-desktop/src/web_api/ws/events.rs`、`crates/live2d-ai-mod-director/src/{presets.rs,staging.rs,tests_staging.rs}` | **D27 信封退场**：显式可选字段 + 改字面量构造 + 删 encode/decode；wire 逐字不变（golden 回归）；负对照四段 |
| **W4f-2（Flutter）** | `shell/flutter/lib/main.dart`、`shell/flutter/lib/live2d/**`、`shell/flutter/lib/api/**`、`shell/flutter/test/**` | **D28 baseline 应用接线**：消费 baseline 并应用；空 → revoke；回归 + 负对照四段 + build web 三证据 |

收口后由编排者跑 **C1–C8**（沿用 STAGE4-WORKER-PROMPTS 口径），维护者再落 **Gate 4f** 提交。

## 5. Gate 5 前置（用户点头）

1. W4f-1 / W4f-2 收口 + 维护者复核通过；
2. 用户在 Windows 上完成肉眼验收（**重启 18080 后**）：
   - 提线木偶：`segments` 两段 → 段 A 闭眼 hold 在开口瞬间**不被掐掉** → 第二段开始换表情/点头；
   - `clock: audio` 期间动作到点才 `preset-expired`（音频时钟生效）；
   - 缺通道时 HUD/ack 有 `degraded`，不静默。
3. 版本 = **`0.2.0-rc.x`**（用户 2026-09-26 定），bump/tag/并 main 均由用户点头后执行。
