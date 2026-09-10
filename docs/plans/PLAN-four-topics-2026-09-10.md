# 四议题实施计划（2026-09-10）：渲染色调 / UI 增强 / TTS 速率 / 口型增益

> 依据：`docs/plans/HANDOFF-2026-09-10-four-questions.md`（同轮审计，含全部实测数据）。
> 基线：`main` @ `v0.4.0`（本轮 6 笔本地提交已落地，未 push）。
> 用户裁决：**先调查出计划**，不直接改代码；四项都要做。

---

## 0. 总览与排序

| 议题 | 改动面 | 风险 | 依赖 | 建议波次 |
| --- | --- | --- | --- | --- |
| ① 外观色调 | `l2d` 渲染核心 + 渲染面 + settings + UI | 中（新渲染 pass 有性能与 alpha 回归风险） | 无（P0 可用 CSS 兜底） | **波 1（先做 P0 零代码部分）** |
| ④ 口型增益 | 渲染面 1 处 + 可选 settings | **低**（一处乘法，可立即见效） | 若要可配置 → 依赖 ② | **波 1（与 ① 并行）** |
| ③ TTS 速率 | 客户端调用 + 服务端选型 + 常量暴露 | 低（但**收益上限在服务端**） | 无 | **波 1（配置侧）→ 波 3（流式）** |
| ② UI 增强 | Flutter 前端 + 治理文档 | 中（治理冲突需先拍板） | ①④③ 的配置项最终都要落在 UI 上 | **波 2（治理）→ 波 3（面板）** |

**关键依赖关系**：④ 的「灵敏度滑杆」、① 的「色调滑杆」、③ 的「分句/chunk 参数」
都要落在 ② 的设置面板上。所以 **② 的治理拍板是前置条件**，但不阻塞 ①④③ 的代码改动——
先按「硬编码默认值 + 预留配置读取点」实现，UI 后补。

---

## 1. 波 1-A：口型增益（④）— 最小风险、最立竿见影

### 1.1 目标

让 `ParamMouthOpenY` 在正常语音下走到量程的 50–70%，而不是现在的 3–12%。

### 1.2 现状证据（见审计 §5）

- 服务端 `ws.rs:485-500` 与客户端 `audio_player.dart:85-86` 都是**裸线性 RMS**；
  渲染面 `surface.rs:1083-1084` 直接写参，无增益。
- 实测真实语音 50 ms 窗 RMS：p50 = 0.030 / p90 = 0.118 / max = 0.282。
- 参数扫描：0.12 → 仅 135 px 变化（满幅的 27%）；1.0 → 492 px。

### 1.3 设计决策

**不采用「乘一个常数」的裸增益**——它依赖 TTS 输出电平：换音色、换服务端、
用户在系统里调过音量，都会让固定增益失效（要么仍不动，要么一直爆张）。

**采用 dB 映射（推荐）**：把 RMS 当作电平，映射到一个合理听觉动态范围。

```
level_db = 20 * log10(max(rms, 1e-6))          // 满幅 0 dB，静音 -120 dB
// 语音 RMS 典型落在 -30 .. -12 dBFS；取 -36 .. -6 dB 映射到 0..1
norm = (level_db + 36) / 30                    // -36dB→0, -6dB→1
mouth = clamp(norm, 0, 1) * sensitivity        // sensitivity 默认 1.0
```

验算：p50 = 0.030 → -30.5 dB → 0.18；p90 = 0.118 → -18.6 dB → **0.58**；
max = 0.282 → -11 dB → 0.83。正好落在「明显可见」区间，且不会常驻爆张。

### 1.4 落点选择（三选一）

| 方案 | 落点 | 优点 | 缺点 |
| --- | --- | --- | --- |
| **A（推荐）** | 渲染面 `surface.rs` 写参前 | 唯一写参数的地方；改一行；对 `/` 与 `/app/` 两条前端都生效 | 电平已被客户端平滑过，dB 映射作用在平滑后的值上（可接受） |
| B | 客户端 `audio_player.dart` `_emitLevel` | 语义上是「包络输出」，位置自然 | 只影响 Flutter；旧 JS 前端 `/` 不生效 |
| C | 服务端 `ws.rs` 的 `volume` | 一处改全局 | **该字段目前被客户端完全忽略**（`AudioEvent` 无 volume 字段），改了没用——除非先做 §1.6 |

→ **选 A**。同时把方案 C 作为「统一数据源」的独立小任务（见 §1.6）。

### 1.5 具体改动清单（方案 A）

| # | 文件 | 改动 |
| --- | --- | --- |
| 1 | `crates/l2d-wasm-demo/src/web/surface.rs` | 新增纯函数 `fn mouth_level(rms: f32, sensitivity: f32) -> f32`（dB 映射 + clamp），在 `apply_bridge_effects` 的 `surface.rs:1083` 处替换 `apply_mouth.filter(...)` 的取值 |
| 2 | 同上 | `BridgeState` 增 `mouth_sensitivity: f32`（默认 1.0），由 `stage-config` 可选字段承载（协议 v1 payload 已支持扩展，见 §1.7） |
| 3 | 同上 | 门限 `surface.rs:1083` 的 `> 0.01` 改为 `> 0.003`（dB 映射后静音段自然接近 0，门限只用于收口） |
| 4 | 同上 | `TAU_MS`（`surface.rs:38`）160 → **90**（快速连续音节不被抹平） |
| 5 | `crates/l2d-wasm-demo/src/web/surface.rs` 测试模块 | 新增纯函数单测：dB 映射单调性、静音为 0、满幅为 1、sensitivity 线性、NaN/负值防御 |

> `mouth_level` 必须是**纯函数**并单独测试——这是本项目对「调度/电平算术」的既定风格
> （参见 `audio/schedule.dart` 的注释理由）。渲染面测试目前是 `surface.rs` 内的 `#[cfg(test)]`。

### 1.6 配套任务（P1）：统一电平数据源

现在服务端算一遍 `volume`、客户端又算一遍，且参数不一致（服务端 20 ms 片、
客户端 50 ms 窗 + attack/release），而服务端的 `volume` **被完全丢弃**。

- 二选一：**(a)** Flutter 直接消费服务端 `volume`（需给 `AudioEvent` 加字段 +
  改 `MouthEnvelope` 用法）；**(b)** 服务端停止下发 `volume`，只留 `audio`。
- 建议 **(b)**：客户端已有的 `MouthEnvelope` 带 attack/release 平滑，质量更好，
  且与播放时刻对齐（`LevelTimeline`）已经做对。删掉服务端那个字段可省 10 次/秒的
  重复计算与 wire 体积。
- 注意：旧 JS 前端 `chat.js` 是否消费 `volume` 需先确认（`chat.js:288-298` 附近）。

### 1.7 协议扩展

`surface.rs` 的 v1 分发已支持 `{version,type,payload}` 且 payload 字段是按需读取的，
新增 `mouth_sensitivity` 是**向后兼容**的（旧客户端不发 → 用默认 1.0）。
Flutter 侧 `live2d_bridge.dart` 的 `sendSync` 增一个可选参数即可。

### 1.8 验证

```bash
# 单测（纯函数 + 渲染面）
cargo test -p l2d-wasm-demo
# 视觉验证：用波 1 的探针脚本（审计 §6 的临时 example 手法）渲染 0.18 / 0.58 / 0.83
# 听感验证：跑 ignite.sh，说一句话，肉眼看嘴是否明显开合
```

**验收判据**：正常语速中文语音下，嘴部形变像素数应达到满幅的 **50% 以上**
（当前约 27%）。

---

## 2. 波 1-B：渲染色调（①）

### 2.1 目标

给「模型看起来偏亮/偏淡」一个**可调的呈现层参数**（曝光 / 对比度 / 饱和度 /
色温），默认值保持与现在逐像素一致（不改变既有视觉基线）。

### 2.2 现状证据（见审计 §2）

- 渲染器输出与两个独立参考实现色彩统计一致（平均饱和度 0.075 / 0.077，
  平均亮度 225.3 / 223.0）→ **不是渲染 bug**，是资产本身浅 + 没有可调项。
- `RenderColorspace` 硬编码在 `model_core.rs:46`，`RenderOptions`（第三方）无色调字段。
- 驱动层只有 `set_part_opacity_by_id`（`ayagami/src/driver/mod.rs:553`），**没有全局色调 API**。

### 2.3 设计决策

**不要用 CSS 滤镜作为最终方案**（虽然它是最快的验证手段）：CSS 滤镜作用在
canvas 元素上，会**同时影响背景与 UI 叠加层**，且桌面原生窗口（`--chat`/`--model-smoke`
路径）不经过浏览器，拿不到这个效果——两条渲染路径会分裂。

**最终方案：在 `l2d` 渲染核心里加一个后处理 pass。**

```
ModelRendererCore::render_to_view_submit
  ├─ 若 tint 为恒等 → 直接渲染到目标 view（保持现状，零开销）
  └─ 否则 → 渲染到内部 offscreen texture → 全屏三角形 pass 采样该纹理
            → out = pow(clamp(in.rgb * exposure, 0, 1), vec3(1.0/gamma))
                    * contrast_sat_matrix → 写目标 view（保持预乘 alpha）
```

关键约束：
- **必须保持预乘 alpha 语义**（`model_core.rs:164-172` 用 `LoadOp::Clear(TRANSPARENT)`，
  上游输出预乘）。后处理 pass 要按预乘处理：`rgb` 与 `alpha` 同步缩放，
  否则边缘会出现亮/暗描边。
- **默认恒等**（exposure=1, contrast=1, saturation=1, gamma=1）时不走后处理，
  保证既有金标准（`bai_asset.rs` 的确定性 hash）不变。

### 2.4 具体改动清单

| # | 文件 | 改动 |
| --- | --- | --- |
| 1 | `crates/l2d/src/renderer/tint.rs`（新增） | `TintParams { exposure, contrast, saturation, gamma }` + `is_identity()`；仿 `tier.rs` 的独立小模块风格 |
| 2 | `crates/l2d/src/renderer/model_core.rs` | 增 `tint: TintParams` 字段 + `set_tint`/`tint` 访问器；`render_to_view_submit` 按 `is_identity()` 分支 |
| 3 | `crates/l2d/src/renderer/post.rs`（新增） | 后处理管线：WGSL 全屏 pass + 采样纹理 + 缓存 pipeline；WGSL 计入 rust-ratio（`wgsl` 在统计扩展名内，见 `xtask/src/main.rs:26-27`） |
| 4 | `crates/l2d/src/renderer/offscreen.rs` | 离屏路径同样支持 tint（PNG 产物用于回归对比） |
| 5 | `crates/l2d-wasm-demo/src/web/surface.rs` | `stage-config` 增 `tint` 可选字段 → `core.set_tint(...)` |
| 6 | `crates/live2d-ai-desktop/src/app/frame.rs:300` 附近 | 原生窗口路径同样透传 tint |
| 7 | `crates/live2d-ai-runtime/src/settings.rs` | 新增 `[display]` 段（`DisplaySettings { exposure, contrast, saturation, gamma }`，全默认恒等） |
| 8 | `crates/live2d-ai-runtime/src/settings/view.rs` | 增 `DisplayView`（并入 `SettingsView`） |
| 9 | `crates/live2d-ai-desktop/src/web_api/settings_routes/` | patch DTO + 字段校验（范围 clamp、finite 检查，仿 `models_routes/dto.rs:244-250`） |
| 10 | `crates/live2d-ai-desktop/src/app/settings_ui.rs` | egui `Draft` + overlay 四滑杆 |
| 11 | `live2d-ai.toml.example` | 新增 `[display]` 段与注释 |
| 12 | 测试 | `tint.rs` 纯函数单测（恒等/极值/clamp/NaN）；`model_core` 的恒等分支不改变既有金标准 |

### 2.5 分步落地（降低风险）

- **步骤 1（可立即做）**：CSS 滤镜只作为**临时验证手段**，在本机确认「调暗到什么
  程度用户满意」，把目标参数记下来（例如 `exposure=0.92, contrast=1.06, saturation=1.08`）。
- **步骤 2**：实现 `tint.rs` 纯参数类型 + 单测（无 GPU 依赖，CI 可跑）。
- **步骤 3**：实现后处理 pass；用 `bai_asset.rs` 的确定性 hash 证明**恒等时逐像素不变**。
- **步骤 4**：接 settings / UI / 渲染面。

### 2.6 风险

| 风险 | 缓解 |
| --- | --- |
| 后处理 pass 破坏预乘 alpha → 边缘描边 | 单测覆盖半透明像素；用现有 alpha coverage / bbox 断言回归 |
| 多一个 pass 影响帧率（尤其 WASM） | 恒等时短路；实测 WASM 帧率（渲染面已有 FPS 徽标可读） |
| `RenderOptions` 是第三方类型，无法直接注入 | 后处理在**本项目自己的** `l2d` 层做，不碰 ayagami |

---

## 3. 波 1-C：TTS 速率（③）

### 3.1 现状证据（见审计 §4）

| 事实 | 数值 |
| --- | --- |
| RTF（合成耗时 / 音频时长） | 1.3–1.7×，抖动时 5–6× |
| 首音频字节（TTFA） | 4.3–13.7 s |
| 项目自定验收线 | TTFA ≤ 1 s |
| `stream` 字段 | **请求体里根本没有** |
| 服务端 `stream=true` | 无流式效果（整段一次返回） |

### 3.2 分层策略

**必须诚实区分「本项目能修的」与「必须换服务端的」**：

| 层 | 项 | 预期收益 |
| --- | --- | --- |
| A. 本项目客户端 | 请求体加 `stream` 字段（服务端支持时立即生效）；分句上限调小；chunk 调小；恢复连接池 | 中（TTFA 与端内空档） |
| B. 本项目服务端 | `[conversation]` 段暴露常量；WS 队列与 TCP_NODELAY | 小-中 |
| C. 外部 TTS 部署 | 换支持真流式的实现 / 更小模型 / 更好 GPU / 预置音色替代 zero_shot | **数量级** |

### 3.3 具体改动清单

| # | 层 | 文件 | 改动 | 风险 |
| --- | --- | --- | --- | --- |
| 1 | A | `crates/live2d-ai-runtime/src/tts.rs:21-29` | `SpeechRequestBody` 增 `stream: Option<bool>`；由配置控制（默认 **false**，因当前服务端不支持，开了也无害但会误导） | 低——有 wire 形状单测 `tts_default_request_sends_response_format_pcm_on_the_wire` 需同步 |
| 2 | A | `crates/live2d-ai-runtime/src/dialogue/sentence.rs:46` | `DEFAULT_MAX_CHARS` 48 → **16**。**这是不流式后端下最有效的单点**：每请求时长直接决定 TTFA | 中——过长文本会被切更多句，需确认不切断语义（该常量已是「按位置硬切」兜底） |
| 3 | A | `crates/live2d-ai-runtime/src/conversation/mod.rs:124` | `DEFAULT_AUDIO_CHUNK_SAMPLES` 4800 → **1200**（50 ms） | 低——分片更多，WS 帧数 ×4；需复测 WS 队列压力 |
| 4 | A | `crates/live2d-ai-runtime/src/lib.rs:131` | 恢复连接池（`pool_max_idle_per_host` > 0）。**不要全局改**：mock 竞态是真实回归（注释 `lib.rs:118-123`），应改为 per-test client | 中——必须复现那个曾间歇性红的测试 |
| 5 | B | `crates/live2d-ai-runtime/src/settings.rs:317-325` | 新增 `[conversation]` 段暴露 `audio_chunk_samples` / `tts_queue_capacity` / `sentence_max_chars` | 低 |
| 6 | B | `crates/live2d-ai-desktop/src/web_api/ws.rs:156-174,307` | 队列容量 256 → 按需；满时**丢最旧**而非踢订阅者 | 低-中——语义变更需测 |
| 7 | B | `crates/live2d-ai-desktop/src/web_api/ws.rs:296-304` | 升级后的 socket 设 `TCP_NODELAY` | 低（loopback ≈0，局域网有效） |
| 8 | C | 部署/文档 | 按 §3.4 评估换服务端 | — |
| 9 | 修 bug | `crates/live2d-ai-desktop/src/web_api/chat.js:352` | 去掉 `turn_state != running → interruptAudio()`（应按 epoch/`voice_ended`） | 低——**旧 JS 前端最后一句被切** |
| 10 | 修 bug | `crates/live2d-ai-desktop/src/web_api/chat.js:268-285` | 排片改为一次提前 2–3 片，不再靠 `onended` 链式 | 低——Flutter 侧已正确，照抄 |

### 3.4 C 层的选型建议（需要你做决定）

三条路，成本差异很大：

1. **官方 FastAPI `/inference_*`（路径 B）**：本项目未内置 provider。
   需要新写一个 provider + multipart 表单 + 裸 PCM 流解析。
   只有在它**确实逐块流式返回**时才值得——**先做一次 30 分钟的裸 socket 验证**再说。
2. **换更小的模型 / 更好的 GPU**：不动代码，直接改善 RTF。
   但注意：**只要服务端仍不流式，TTFA 就等于整句合成时间**，
   模型快 2 倍 → TTFA 也只快 2 倍，仍可能 > 1 s。
3. **预置音色替代 `zero_shot`**：省掉每句的参考音频编码，是纯服务端配置。

> **建议先做的判定实验**：拿 `curl` + 原始 socket 时间戳，对候选服务端测
> 「首音频字节时间」。这是唯一的判据——`curl %{time_starttransfer}` 报的是响应头，
> 会给出 3 ms 的假象。

### 3.5 验证

```bash
# 每个改动后复测同一句，记录 TTFA / 总耗时 / RTF
printf '%s' '{"input":"你好，今天天气真不错，我们出去走走吧。","voice":"skystar","response_format":"pcm"}' > req.json
# 用原始 socket 测首字节（不要用 curl time_starttransfer）
```

**验收判据**：TTFA ≤ 1 s（项目既定线）。在服务端不流式的前提下，
先靠 #2（分句上限 16）把「单请求时长」压到 1 s 量级——这是本项目**唯一**
能在不换服务端的情况下逼近该线的杠杆。

---

## 4. 波 2：UI 治理拍板（② 的前置）

### 4.1 必须先决定的两件事

**决策 1：Flutter 是否保留为第二运行时？**

| 选项 | 动作 |
| --- | --- |
| 保留 | 更新 `docs/architecture/core-contracts.md:17`（删/改「无第二语言运行时」）+ `AGENTS.md`；把 `.dart` 纳入 `rust-ratio` 分母**或**新增独立 Dart 门禁；记录豁免理由 |
| 回退到 C1 | 删除 `shell/`，把精力投到 `/` 的视觉升级（`docs/plans/web-flutter-migration-and-loop-closure-plan.md` §3.3 原本推荐这条） |

> 无论选哪个，**先把这个决定写进文档**，否则后续每一笔 Flutter 改动都在账外。

**决策 2：旧的 `/` JS 前端怎么处置？**

两套前端并存会产生行为分歧（例如 §3.3 #9/#10 的两个 bug 只存在于 `/`）。
建议：**明确 `/` 为「开发/调试面板」，`/app/` 为「用户界面」**，并在两处 UI 上标注。

### 4.2 具体改动清单（治理）

| # | 文件 | 改动 |
| --- | --- | --- |
| 1 | `docs/architecture/core-contracts.md` | §1 语言栈声明：补 Flutter/Dart 的定位与门禁口径 |
| 2 | `AGENTS.md` | 「唯一实现主线」段落补第二运行时的边界与豁免 |
| 3 | `xtask/src/main.rs:26-27` | `COUNTED_EXTENSIONS` 是否纳入 `dart`——纳入会拉低 rust-ratio（2396 行 Dart），需同时调整门槛或改为独立指标 |
| 4 | `docs/README.md` | 索引两套前端的定位 |

---

## 5. 波 3：UI 增强（② 的实质内容）

### 5.1 现状

Flutter 壳只有舞台 + 聊天（`main.dart:96-152`）；`/` 有 7 个设置页签。
Flutter 侧 `ApiClient` 只用了 `/chat`、`/chat/stop`、`/app/status`
（`api_client.dart:58,80,90`）——**没有读 `/api/v1/settings`**。

### 5.2 建议的实施顺序

| 序 | 项 | 落点 | 依赖 |
| --- | --- | --- | --- |
| 1 | 设置读取链路：`ApiClient.fetchSettings()` + 一个 `SettingsController` | `api_client.dart` / 新 `settings/settings_controller.dart` | 无 |
| 2 | **外观页签**：曝光/对比度/饱和度/gamma 四滑杆 + 背景色/透明度 + 缩放 | 新 `settings/appearance_panel.dart`；写回 `/api/v1/settings` 的 `[display]`，并经 v1 `sync` 下发渲染面 | ① 的 #7-11 |
| 3 | **AI/TTS 页签**：base_url / model / voice / response_format + 试听按钮 | 复用 `/api/v1/settings` 与 `/api/v1/settings/test/{llm,tts}` | 无 |
| 4 | **口型页签**：灵敏度滑杆 + 实时试动 | ④ 的 `mouth_sensitivity` | ④ + #1 |
| 5 | **模型页签**：导入/激活/舞台配置 | `/api/v1/models*`（`models_routes/` 已实现） | 无 |
| 6 | **日志/命令页签** | `/api/v1/logs`、`/api/v1/commands` | 无 |
| 7 | persona 五字段（含 null-clear 语义） | 语义见 `app.js` —— `webapp_behavior.rs` 已有行为基线可对照 | 无 |

> **重要**：#7 的 persona 五字段「null-clear / `(已绑定)` 占位 / keyTouched」这些语义
> 已经在 `crates/live2d-ai-desktop/tests/webapp_behavior.rs` 里被**行为级锁定**了。
> Flutter 实现应逐条对照那套基线，避免语义漂移。

### 5.3 验证

- `cd shell/flutter && ~/flutter/bin/flutter analyze && ~/flutter/bin/flutter test`；
- 端到端：`./scripts/ignite.sh` → 浏览器开 `/app/` → 改外观滑杆 → 模型实时变化 →
  刷新后设置仍在（证明写盘 + 热重载链路通）。

---

## 6. 建议的执行顺序（合并视图）

```
波 1（可立即开始，互不阻塞，三个可并行）
  ├─ ④ 口型 dB 映射 + 门限/TAU          [低风险，立即见效]
  ├─ ① 步骤 1：CSS 滤镜定标（只取数值，不入库）
  └─ ③ #1/#2/#3/#5 配置与常量           [低风险，需复测 WS 压力]

波 2（治理拍板 —— 阻塞 ② 的实质工作）
  └─ ② 决策 1/2 + contracts/AGENTS 更新

波 3
  ├─ ① 步骤 2-4：tint 类型 → 后处理 pass → settings/UI 接线
  ├─ ③ #4/#9/#10：连接池 + 两个 chat.js bug
  └─ ② 设置面板（外观/AI/口型/模型/日志）

波 4（外部，需你决策）
  └─ ③ C 层：TTS 服务端流式选型（先做裸 socket 判定实验）
```

---

## 7. 每波的门禁（提交前必须全绿）

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo run -p xtask -- rust-ratio          # 门槛 95%，当前 95.4692%
cd shell/flutter && ~/flutter/bin/flutter analyze && ~/flutter/bin/flutter test
```

## 8. 诚实登记：本计划**不**解决的事

- 不解决 TTS 服务端不流式（那需要换部署，属波 4）。
- 不解决 STT 语音输入缺失（不在四议题内）。
- 不解决 `--web` 无原生窗口（不在四议题内）。
- ① 的「调暗到什么程度」最终仍需**人眼拍板**——计划只能提供可调项与目标区间。
