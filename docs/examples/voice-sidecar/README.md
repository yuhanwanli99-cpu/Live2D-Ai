# 语音输入 sidecar（占位示例，**尚未实现**）

> **本目录目前只有这份说明，没有脚本。** 它是 Wave 1（`mod/voice-input`）为
> 「完整 ASR 不进 Rust 核心」预留的接口说明，不是可用示例。
> 真正的 ASR（whisper / sherpa-onnx / 平台 SDK…）应当作为**独立进程**跑在
> 用户机器上，把转写文本注入主链路；Rust 侧只保留清洗 + 送 say 的一小段逻辑
> （crate `live2d-ai-mod-voice-input`，见其 `src/lib.rs` 头注）。

## 1. 目标链路

```
麦克风 → ASR（sidecar 进程） → HTTP POST → Live2D-Ai → say → LLM → TTS → 口型
```

## 2. 今天就能跑的路（不需要 voice-input Mod）

在 voice-input 有自己的端点之前，外部转写可以直接走**已有的**
`POST /api/v1/external/chat`（`external-input` Mod，缺省启用）：

- 契约全文：`docs/external-input.md`；
- sidecar 把 ASR 结果当普通文本 POST 过去即可，鉴权与地址约定与
  `docs/examples/bilibili-sidecar/` 完全一致（loopback-only + 可选 token）；
- 这条路径今天就能支撑「语音 → 皮套开口」原型。

## 3. 计划中的专属路径（**本分支未实现**）

当 voice-input Mod 注册进 `AVAILABLE_MOD_FACTORIES` 后，集成方需要二选一：

| 选项 | 说明 | 代价 |
|---|---|---|
| A. 复用 `/api/v1/external/chat` | 不新增端点；sidecar 与弹幕共用一条注入面 | voice-input 只提供 settings schema，Rust 侧 `inject_transcript` 仅服务进程内后端 |
| B. 新增 `POST /api/v1/voice/transcript` | voice-input 有自己的鉴权 / locale / backend 语义 | 需在 web_api 新增 handler + 契约文档 + 回归，属集成 PR 范围 |

**本分支不预设端点**：占位只描述形状，避免出现「文档写了、代码没有」的假契约。

## 4. 将来写示例脚本时的建议形状

- 输入：本机麦克风（或已录制的 wav）；
- ASR：本地模型优先，**不要**把模型或推理运行时放进 Rust 主仓；
- 清洗：去零宽字符 / 折叠空白 / 丢弃空文本——与 Rust 侧 `clean_transcript()`
  同语义（`crates/live2d-ai-mod-voice-input/src/lib.rs`）；
- 注入：`Content-Type: application/json`，loopback 地址，可选 Bearer token；
- 失败退避：服务端忙碌时丢弃本条（与弹幕 sidecar 同款），不重试到刷屏。

## 5. 与 AGENTS 红线的关系

- **完整 ASR 不进 Rust 核心**（`PLAN-voice-input.md` §Forbidden）；
- 非 Rust 实现只能标**实验**，不得出现在正式发布说明的 Mod 清单里
  （`docs/architecture/mod-product-chain.md` §6）；
- 主仓不得新增对在线 ASR 服务的默认依赖（本地优先、断网可用）。
