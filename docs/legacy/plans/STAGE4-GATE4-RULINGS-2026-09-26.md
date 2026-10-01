> 历史（2026-10-01 归档，勿当现网）。

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

## 6. Gate 4f 已落 + D37（W4f 收尾轮）

| 提交 | 主题 |
| --- | --- |
| `refactor(performance): 退役 preset_id 信封，v1 键改显式字段（阶段4f-1）` | 8 文件（含新增 `tests_golden.rs`） |
| `feat(flutter): 会话 baseline 应用接线（阶段4f-2）` | 5 文件 |

**维护者独立复核**：`cargo test --workspace --all-targets` = **1447 passed / 0 failed**（26 组）· doc 3 · fmt clean ·
clippy **exit 0** · rust-ratio **97.3413% PASS**；`flutter analyze` No issues + **1069 passed**；
`grep -rn "V1_ENVELOPE_PREFIX|encode_v1_envelope|decode_v1_envelope" crates/ shell/` = **NONE**（信封真的没了）。

| # | 事项 | 裁决 |
| --- | --- | --- |
| **D37** | W4f-1 实际改动含 **4 条表外路径**：`performance/mod.rs`（删 re-export）、`performance/client.rs` + `tests/conversation_engine_performance.rs`（各 1 行 `..Default::default()`）、新增 `performance/tests_golden.rs` | **追认授权**。D27 的定义（给结构体加字段）**必然**触及这三处字面量构造与那一处 re-export；`tests_golden.rs` 属 `STAGE4-plan §6.7` 的拆分授权（守测试 ≤800 行，`tests.rs` 798）。与 **D32 同类**：是计划文件表不完整，不是 worker 越权。编排者按 C1 明列为 RED 而非自行扩权，**处置正确**。 |
| **D38** | 首轮全量出现既有 flaky `web_api::chat_routes::tests::chat_with_supervisor_returns_200`（`chat_routes.rs:665`） | **记 backlog P1**。与本轮无关（该用例 `mod_events: None`、performance 关闸，不构造 cue），维护者复跑 26 组全绿未复现。下轮按 **D1/D8 口径**做确定性同步（**不许只调大超时**），并审计同族「先失败再计数」的断言。 |
| **D39** | D36 陈旧服务 | `scripts/ignite.sh` 第 199 行自带 `pkill -f "live2d-ai-desktop --web"`，已顺手结束 PID 59888；**当前 18080/18081 均无监听**。肉眼验收前重新点火即可。 |

## 7. 维护者无头浏览器验收（2026-09-26，Gate 4f 后）

**环境**：重编 `target/debug/live2d-ai-desktop` → `./scripts/ignite.sh --port 18080`；`ignite --check` 四项 ok。
无头浏览器 = Playwright Chromium 153 + **SwiftShader Vulkan ICD**（`VK_ICD_FILENAMES=<chrome-linux64>/vk_swiftshader_icd.json` + `--enable-unsafe-webgpu --enable-features=Vulkan,DefaultANGLEVulkan --use-angle=vulkan --in-process-gpu`）。
> 首次不带 ICD 启动 → 渲染面报「无可适配器（WebGPU 与 WebGL 均不可用）」。**这条配方不是可选项。**

| # | 判据 | 原始读数 |
| --- | --- | --- |
| 1 | 舞台起来（WebGPU） | HUD `GPU: webgpu/WebGPU`，FPS 59~60，`bg: solid`，模型 bai 加载 ok |
| 2 | 新 v1 HUD 行在场 | `fields: body: - head: - expression: -`（无 v1 cue 时为空） |
| 3 | 真链路一轮（LLM+TTS） | `POST /chat` 200 → WS `audio` 帧 → 前端 WAV blob → 出声 |
| 4 | **无 TTS 重复** | blob 字节 = 线上 PCM + 44（**240044 / 240000**、**147884 / 147840**）；每句 `play()` 恰好一次；10 句回复 = 10 blob / 10 次不同 src 的 play；`loop:false` |
| 5 | D30 下行新键 | legacy cue 走 `preset{id,source,ttl_ms,intensity,epoch,sentence_seq}`；baseline 走 `preset{id,source,intensity,field,hold,at}` |
| 6 | V7 音频时钟 | `stage-clock{seg,pos_ms,playing}` 连续下发（pos_ms 0→700）；HUD `clock: wall → audio` |
| 7 | D31/D28 取消信号 | 每条新消息 → WS `action_cue{baseline:true,reason:new-message}` → 前端 `preset{id:none,source:new-message}` |
| 8 | **V10 会话 baseline 端到端** | 设 `{session_id:eye2, baseline:{expression,smile,intensity:2,hold:true}}` → `POST /chat{session_id:eye2}` 回包带 baseline → WS cue 带该字段 → 渲染面 `preset{field:expression,hold:true}` → **HUD `fields: expr:smile[1]`**（保持中） |
| 9 | 空句不产生假播放 | `✅` 段 → 1 帧 0 字节边界帧 → 无 blob、无 play |
| 10 | 控制台 | 无音频错误；唯一 404 = `field_map.json` 覆盖表缺（O9 允许，回落内建表） |

**两条观察（记录，未定性为缺陷）**

1. **用户报的「TTS 重复播放」根因 = 两个客户端**：维护者无头 Chromium 与用户本人的 Chrome 同时连同一服务，广播音频在两个浏览器各播一次。产品侧无重复（见 #4）。=> 多客户端 = 多路独立输出，属既有广播语义；**验收时只开一个客户端**。
2. **baseline 交付需要请求带 `session_id`**：裸 `POST /chat`（不带 session_id）回包 `session_id:null, baseline:null`，WS cue 的 `cues` 也为空；带 `session_id` 才正常（#8）。前端 `ChatController` 每轮都带当前会话，产品路径 OK；但「宿主活动会话游标作为缺省」在裸请求里没有兑现——**记观察**。

**仍未由维护者取证**：「提线木偶」（`segments` 多段 + `at:seg:N` 锚点）需要 `[performance]` 真/mock 端点（D19）；上面 #3–#9 全走 **degraded/规则路径**。

**收工状态**：无头 Chromium 已 kill；测试 baseline 已清（`baselines:0`）；**服务保留在 18080**（供用户单客户端验收）。


