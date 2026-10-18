# 0.3.7 执行计划：LLM 云/本地 + 动作表情扩充

> 状态：**待执行**（2026-10-18 立）· 上游：`PROMPT-3.0-motion-first-2026-10-17.md` 与
> `docs/architecture/CURRENT.md`。
>
> 写法：本块 tag `v0.3.7-rc.1` … `v0.3.7-rc.z`；最后 `v0.3.7-rc.(z+1)` 只清这一块，不新增功能。
> `z` 不预设。每步换一队、一次本地提交、一个本地附注 tag、**不 push**。`Cargo.toml` 仍为 `0.3.1`。

## 0. 现状

| 项 | 现状 | 证据 |
| --- | --- | --- |
| LLM | `[llm]` 只有 `base_url` / `model` / `api_key_env` / `max_tokens` / `show_reasoning` / `vendor` + 快照表；**没有 local/cloud 选择** | `crates/live2d-ai-runtime/src/settings.rs` 的 `LlmSettings` |
| TTS | 已有 `[tts].mode`（local / cloud）+ vendor 快照 + 设置页选择器，可作对齐样本 | `TtsSettings`、`shell/flutter/lib/settings/sections/tts_section.dart` |
| 动作 | 12 条 preset + `none` 哨兵，两槽（表情 / 手势）；`PRESET_IDS` 是 Rust 侧单一真源，另有 `presets.json`、`preset_labels.json` | `crates/live2d-ai-runtime/src/performance/rules/presets.rs`、`assets/actions/` |
| 参数边界 | 渲染面只允许 13 个标准参数；**没有手臂 / 手指通道** | `crates/l2d-wasm-demo/src/preset/{field_map.rs,table.rs}` |

## 1. 非目标

- **不做嵌入式本地模型**：不下载权重、不装推理运行时、不管理 GPU；本地 = 用户自己起的
  OpenAI-compatible 端点（Ollama / llama.cpp / vLLM 等）。
- 不新增 Rust 依赖（desktop 顶层依赖已在 24/24 上限）；不改 TTS 链路；不碰 Windows。
- 不复活 core `action_tx` / `RootEvent::Action`；不新增 WS 帧类型（只复用既有 ack / 字段）。
- 不把所有模型都当成有手臂参数；不做「静默假装支持」的假动作。

## 2. T1 LLM 云 / 本地（建议先做，独立可交付）

写作用域：`crates/live2d-ai-runtime/src/settings*`、`crates/live2d-ai-desktop/src/web_api/settings*`、
`shell/flutter/lib/settings/sections/llm_section.dart`、`shell/flutter/lib/api/settings_models*.dart`、
`live2d-ai.toml.example`、相关测试。

1. **模型**：`LlmMode { Cloud(缺省), Local }`，`[llm].mode`；用 `#[serde(default,
   skip_serializing_if = "is_cloud")]`，旧 toml（无 mode）逐字 round-trip，语义 = cloud。
2. **语义**：transport 不变，仍是同一条 OpenAI-compatible `/chat/completions`；local 只表示
   「端点在用户本机/局域网、通常不需要云 key」，**不预填厂商、不带 key**。
3. **设置页**：对齐 `tts_section.dart`：模式选择 + vendor 选择 + base_url / model / 密钥变量名；
   本地模式给 Ollama（`http://127.0.0.1:11434/v1`）等**示例文案**，不做硬编码下拉；切模式不动
   vendor 快照的既有语义（与 TTS 逐条对齐）。
4. **连通性自检**：沿用 TTS 的纪律——`GET {base_url}/models`，404 算可达，**不真跑一次对话**；
   失败分类可读；没配置 / 不可达时显示 `llm_local_unreachable` 之类的明确状态，**不得显示 ready**。
5. **不自动回退**：local 失败不偷偷切 cloud；cloud 缺 key 仍走既有 401 错误路径。
6. **测试**：serde 旧 toml round-trip；PATCH 校验（未知 mode 拒绝）；自检分类；Flutter 草稿切模式
   / 保留 vendor 的交互测试；文档（CURRENT `7 + example toml + 发布说明）。

## 3. T2 动作 / 表情扩充

### 3.1 先裁决 D1：手臂参数怎么办

| 方案 | 内容 | 代价 |
| --- | --- | --- |
| A 保守 | 只加 13 参数能表达的动作，wave / clap / point / cover_mouth **不做** | 动作「更多」打折，但不碰参数白名单 |
| B 推荐 | 允许可选手臂/手参数（`ParamArmLA/RA`、`ParamHandL/R` 等）进白名单；每模型 `field_map.json` 声明；缺参数时 ack 标 `unsupported_params`，UI 显示「该模型不支持」 | 契约变更，要改白名单 / 表校验 / ack；必须证明不碰口型与头身红线 |

**建议 B**，并把「不支持就明说、不假装」作为验收硬条件。若维护者不接受白名单扩张，退回 A 并删掉
需要手臂的 4 个 id——**不要**用「只动头身」冒充挥手 / 鼓掌 / 指 / 捂嘴。

### 3.2 新增内容（按 D1 取舍）

- 表情 8 条：`angry`、`sad`、`blush`、`wink`、`confused`、`sleepy`、`excited`、`calm`。
- 手势 6 条：`bow`、`wave`、`clap`、`point`、`shrug`、`cover_mouth`。
- 合计 12 → **26**（不含 `none`）；每条按现有 v3 规则：表情 = 五官 + 小幅头身，手势 = 头 + 半身；
  幅值 / 周期 / 时长按 `scales.rs` 标定，不超头 30 / 身 10 / 五官 4；手臂类参数只走可选通道。

### 3.3 单一真源与接线（逐处改，不许漏）

| 位置 | 改动 |
| --- | --- |
| `assets/actions/presets.json` | 14 条包（参数表 / duration / morph 如需要） |
| `assets/actions/preset_labels.json` | 14 条 zh/en/channel/source/note |
| `performance/rules/presets.rs` | `PRESET_IDS` + `preset_slot` + `EXPRESSION_PRESET_IDS` |
| `preset/table.rs` / `field_map.rs` | 白名单（按 D1）、每模型可选通道、解析降级 |
| `preset/*` ack | 复用既有 ack 字段报告缺失参数 / 降级，不新增 WS 帧 |
| Flutter 标签 | 读 `preset_labels.json`，不在 Dart 手写中文 |

1. 未知 id 仍 `Ignore`；`none` 语义不变；同槽换包仍走 handoff / release。
2. 导演规则表可引用新 id，但仍是「这一句原文」裁决；不整篇分类、不随机、不跨轮常驻。
3. **一致性回归**：Rust `PRESET_IDS` ↔ `presets.json` ↔ `preset_labels.json` ↔ `preset_slot` /
   `EXPRESSION_PRESET_IDS` 四向断言；未知参数降级断言；每条 apply / revoke / ttl 边界。
4. **验收**：wasm 重建后真实浏览器逐条肉眼（至少表情 / 手势各 3 条 + 一条缺手臂参数的降级）；
   bai（无手臂参数）与 Epsilon（按实际参数）各测一次。未验就写「未验」。
5. 文档：CURRENT `4 动作表、`preset_labels` 说明、发布说明；写清「哪些模型不支持哪些包」。

## 4. T3 末步 `rc.(m+1)`：只清理本块

- docs 预算 ≤45,000；`>500` 不新增；不新增依赖；全门禁；一次本地提交 + 本地附注 tag；不 push。
- 与 0.3.6 的接口：0.3.6 的 model epoch / field map 重载必须能承载新 id；若本块改 field map schema，
  先回归 0.3.6 的换模与 cue 测试。

## 5. 验收汇总

| 判据 | 证据 | 未验边界 |
| --- | --- | --- |
| `[llm].mode` 旧 toml 兼容 | serde round-trip 测试 | — |
| 本地端点状态诚实 | 自检分类测试 + 实际 Ollama（可选） | 没装 Ollama 就只测分类 |
| 26 条动作四处一致 | 四向一致性测试 | — |
| 缺手臂参数明示降级 | bai 上跑 wave/clap/point/cover_mouth 的 ack + UI | 依赖 D1 方案 |
| 动作不越红线 | 幅值 / clamp 单测 + 肉眼 | — |

## 6. 风险

- 白名单扩张会把「未知参数静默丢」变成「可见降级」，是行为变化，必须写进发布说明。
- 本地 LLM 各家 `/models` 行为不一致（404 算可达）；不能拿一次对话当自检。
- 动作数量翻倍后 UI 选择器密度与标签翻译；保持 zh/en，不新增手写标签。
- docs 余量有限：新增 14 条动作的说明要进既有表，不另开长文。

## 7. 完成定义

LLM 设置能在 cloud / local 间切换且旧配置不破；本地不可达状态诚实；26 条动作在 Rust / JSON /
标签 / 槽位四处一致；无手臂参数的模型明确降级而不是假装；全门禁绿；本地 commit + tag；报告区分
「已验 / 未验」。
