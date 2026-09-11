# 交接：核心链路基线（2026-09-11 晚）—— 工具/动作拆除 + 音频走媒体元素 + 三个真缺陷

> **这是当前交接。** 上一版 `HANDOFF-2026-09-11.md`（v0.4.13 → v0.5.1 前端重做）
> 已随本轮提交落地，其 §12「未提交改动清单」**已完成、不再是待办**。
>
> 本轮用户的裁决与目标（原话）：
> - 「**llm 不暴露任何工具只做对话**」「从前后端抹去相关字段不做实现」
> - 「我需要的是一个**完整核心链路完善的基线**作为上层体验相关升级的**底座**」
> - 「核心链路只有 tts 驱动口型 llm 纯对话 live2d 皮套渲染 前端 ui」「**和相关设置等基础设施**」
> - 「后端推 mp3 文件前端 web 做相关声音播放」「**而不是后端播放**」
> - 「本轮测试通过 交接落盘 洗掉 api 之后可 push 并**标注基线**」

---

## 0. 一分钟上手

```bash
cd /home/skystar/Live2D-Ai
export PATH="$HOME/.cargo/bin:$HOME/flutter/bin:$PATH"   # 两者都不在 PATH 上

# 起服务（默认出声；.env 里已有 DEEPSEEK_API_KEY，TTS 需本机 CosyVoice 在 8080）
./scripts/ignite.sh
# 打开 http://127.0.0.1:18080/app/   ← / 会 302 到 /app/

# 端到端验收（只走公开 HTTP/WS，17 跳）——**改动核心链路后必跑**
python3 scripts/verify_core_chain.py
```

三条独立的门禁：

```bash
python3 scripts/verify_core_chain.py                      # 链路
cd shell/flutter && flutter analyze && flutter test        # 前端（778）
cargo test --workspace --all-targets                       # 核心层（765）
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio                           # ≥95%
```

---

## 1. 这一轮做了什么

### 1.1 LLM 工具 + 动作系统：前后端**整体拆除**

**为什么**：空 system prompt + 一个 `perform_action` 工具，会让模型有相当概率
**只调工具、不说话** —— 产生「正常完成但一个字都没有」的回合（实测复现：
`turn_state{completed}` + 零文字 + 零音频 + 后端零错误）。这正是挡住链路闭环的那道坎。

**Rust（删 7 文件 / 改 35）**：`live2d_perform_action` 工具与其 wire 解析、
`dialogue/tool.rs` 组装器、`tool_adapter`、`EngineEvent::ToolAction`、
`RenderCommand` / `AppEvent::Render`、WS `action_state` 投影、
`/api/v1/commands`（手动触发动作的端点）与它的注册表，**以及 D11 文本规则 fallback**
（`content_worth_fallback` → `rule_fallback`：绕过 `tool_adapter` 的**第二个**动作触发源）。

**前端（删 16 文件 + 1 目录）**：`lib/actions/*`、动作日志 UI、动作来源徽标、
`CommandsApi`、`ActionStateEvent` 解析、`sendActionState` 接线、动作设置分区。

**两处「名字骗人」的纠偏**（差点误删要保留的功能，**别按文件名判断内容**）：

| 旧文件 | 实际内容 | 处置 |
| --- | --- | --- |
| `settings/sections/actions_section.dart` | **整个「外观与互动」设置**（主题 / 展台图 / 口型灵敏度 / 缩放档位），只有最后一组是动作记录 | 改名 `appearance_section.dart`（`AppearanceSection`），内容保留 |
| `ui/action_toolbar.dart` | 只有 `StageCornerControls`（缩放角标）；动作工具条**早已移除** | 改名 `stage_corner_controls.dart` |

**关键断言**：`llm::tests::wire_request_has_no_tools_key` —— 断言请求体里 `tools`
**键不存在**（不是空数组；空数组仍会在 wire 上宣告「支持工具」），并连带断言
`tool_choice` / `functions` 也不存在。

### 1.2 音频播放：Web Audio → **`<audio>` 媒体元素 + WAV**

**为什么**：用户报「浏览器禁用声音但依旧有声音传出」。两条实测依据：

1. **后端根本不发声**：`audio.backend = "none"`、`available: false`（硬编码
   `web_api/app_routes.rs`），启动横幅写明「音频单源 = browser —— 服务端不开 cpal」；
2. **Web Audio 与媒体元素走两套权限**：Web Audio 归 Chromium 的 **autoplay 策略**管，
   而站点级「声音 = 阻止」作用在**媒体元素**上。用 `AudioContext` 逐片播放时，
   站点级静音很可能拦不住——这就是「禁了还有声」。

**实现**：后端 PCM 经 WS 下发（**一句一组分片**）→ 前端按句封 **WAV**
（44 字节头 + 裸 PCM，零依赖）→ `Blob` + `URL.createObjectURL` → `HTMLAudioElement.play()`。
口型改为 30 ms 轮询 `audio.currentTime` 查服务端 `volume` 包络。

- `lib/audio/sentence_assembler.dart`（纯逻辑：攒句 / 包络 / 串行队列）
- `lib/audio/wav.dart`（`wavFromPcm16` + `levelAtSec`）
- `lib/audio/audio_player.dart`（只剩 `<audio>`/Blob/URL 胶水）
- `lib/main.dart` **零改动**（公开 API 全部保住）

**为什么不是 mp3**：本机 TTS **实测出不了**（`mp3` → HTTP 400、`wav` → 400、
只有 `pcm` → 200，24 kHz s16le）。要「推文件」就用 WAV，别指望 mp3。

**产物级核验**：`main.dart.js` 里 `createObjectURL` / `revokeObjectURL` / `audio/wav`
都在，而 **`AudioContext` / `createBufferSource` 均为 0** —— 旧路径彻底消失。

### 1.3 三个**真**缺陷（都是跑验收脚本 / 真机点火抓到的）

| # | 症状 | 根因 | 修法 |
| --- | --- | --- | --- |
| ① | 一句正文产出 **0 个 `start`、13 个 `end`** | `start` 由 `Broadcaster::audio_epoch_seen` 按 **epoch 记账**推，而一句音频本来就被切成十几块；`end` 是「每次广播的末片」。两者层级不同 | 句界改为**引擎的契约**：`EngineEvent::AudioChunk` 新增 `first_chunk`，WS 帧据此给 `start`/`end`/`sentence_seq`，删掉记账状态 |
| ② | TTS 回 `400 {"detail":"input 为空"}`，**整轮判失败** | 分句器把**换行**也算句读（`is_terminator` 含 `'\n'`），于是 `"你好！  \n再见！"` 会切出**纯空白句**；它 `is_empty()==false` 躲过守卫，却被 TTS 上游 trim 成空串 → 400 → TTS 错误是 **fatal** | 纯空白句**不发 HTTP**，与「TTS 未配置」走同一条静音句路径（空 `final_chunk` + `SentenceVoiced`）。分句器「逐字不丢」的硬不变量不动 |
| ③ | 某些轮次的句子**听不到** | 引擎允许某句**末块 0 样本**（句子样本数是 `audio_chunk_samples`=4800 的整数倍时必然出现）；`build_audio_frames` 对空 PCM 返回空表、`handlers.rs` 又提前 `return` → 承载 `end=true` 的空末块被丢 → **前端永远等不到句尾闸门** | 空末块仍发一帧**只有边界、没有音频体**的帧（`audio: ""` + `end: true` + `volume: 0.0`）；`handlers.rs` 只跳过**声卡入环**，WS 广播照常 |

**①②③ 的回归都在，且都用「撤掉修复即变红」验过**（③ 实测：`--text "请只说两个字：好的。"`
连续 3 次**全部命中**空末块路径——没有修复时那几轮音频会整句播不出来）。

### 1.4 真机点火抓到：字体子集外的字符 → 偷偷去 Google 拉字体

**证据**（网络请求，不是推断）：

```text
GET https://fonts.gstatic.com/s/notosanssc/v37/…88.woff2  [200]
```

用 fontTools 读自托管子集的**真实 cmap**（22 036 码点）逐字符扫全仓 Dart 字符串字面量，
违规的只有**两个**：

| 字符 | 位置 | 何时上屏 |
| --- | --- | --- |
| `▍` U+258D | `ui/message_bubble.dart` 流式光标 | **每次流式回复** |
| `⌘` U+2318 | `app/app_shortcuts.dart` macOS 前缀 | 打开快捷鍵帮助 |

修法都**不再依赖字形**：流式光标画成竖条（`SizedBox(width: OpticalNudge.thin,
height: Space.s4)` + `ColoredBox`）、macOS 前缀改 ASCII `Cmd`。
**门禁**：`scripts/font_subset_ranges.py` 导出覆盖表（90 区间，头部记录字体**字节数 +
FNV-1a**），`test/font_subset_test.dart` 扫 `lib/**/*.dart` 字面量；换字体没重新生成
覆盖表会因哈希不符判红。修完后重载实测：**外部源 0 条**。

### 1.5 新增的可复跑资产

- `scripts/verify_core_chain.py` —— **17 跳端到端验收**，只走公开 HTTP/WS（stdlib
  自实现极简 WS 客户端）。它当场抓到 ①②③。还钉住两条基线不变量：全程**不得**出现
  `action_state`；`audio` 帧必须带 `sample_rate` 且与 TTS 配置一致。
- `docs/architecture/core-chain-baseline.md` —— **基线说明**：链路逐环与出处、
  已移出链路的、刻意保留的、三个已修缺陷、一键验证、真机点火操作指南。
- `docs/plans/PLAN-audio-wav-path-2026-09-11.md` —— 音频路径改造方案（为何 WAV 不用 mp3）。

---

## 2. 门禁与数字（最后一次实跑）

| 门禁 | 结果 |
| --- | --- |
| `cargo test --workspace --all-targets` | **765 通过 / 0 失败** |
| `cargo test --doc --workspace` | 3 通过 / 0 失败 |
| `cargo fmt --all -- --check` | 干净 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0 warning**（无新增 `#[allow]`） |
| `cargo run -p xtask -- rust-ratio` | **97.19%**（门槛 95% → PASS） |
| `flutter analyze` | **No issues found** |
| `flutter test` | **778 通过 / 0 失败** |
| `python3 scripts/verify_core_chain.py` | **17 跳全过**（连续多轮复跑） |

**基线 tag**：`baseline-core-chain-2026-09-11`（注解 tag）。
**版本号仍是 `0.5.1`**——沿用上一轮「不发新版」的口径（见 `CHANGELOG.md` v0.5.1 追加段）；
要发版再单独 bump `Cargo.toml` + 打 `v0.5.2`。

---

## 3. 交付态实测（本机，不是推断）

```
服务     : uptime 稳定；model=bai_001；llm/tts configured=true；dev_mode=ON
产物     : /app/main.dart.js 下发字节 == 磁盘字节；gstatic 命中 0
真机点火 : 见 §7 的七项证据（舞台 939×808 / WebGPU / stage-ack 往返 /
           <audio> duration 实数且 currentTime 推进 / mouth 393 条含 232 条非零 /
           一句一单元 maxPlaying 恒为 1 / 外部源 0）
音频实测 : 逐句字节与直连 TTS **完全一致**（收到。=86400、请说。=59520）
```

---

## 4. 接手第一件事

1. **读 `docs/architecture/core-chain-baseline.md`**（10 分钟）——它比本文更"是什么"，
   本文更"为什么 + 坑"。
2. **跑一次 `python3 scripts/verify_core_chain.py`**。需要本机 CosyVoice 在 8080
   （`~/CosyVoice 3.0/shim/server.py`），否则 TTS 段会失败——那是环境问题不是代码问题。
3. **确认 `persona.system_prompt`**：当前是**空串**。空 system prompt 会显著提高
   「模型不说话 / 跑偏」的概率。配置文件自己就写着「建议保留第 1/3 条」——
   写一段系统提示词是最省事的画质提升。

---

## 5. 已确认、**故意没做**的事（附成因与建议）

| 事项 | 为什么没做 | 建议 |
| --- | --- | --- |
| `live2d-ai-core` 的 `action/` + `performance/` 子系统 | 自洽且带单测，且 core 不变量文档（`lib.rs` 第 4/6 条）以它为前提；动它会牵动 reducer 的语义论证。现在**没有任何路径**会向 core 发 `Event::Action` | 留着当休眠能力；要删得连不变量一起重新论证 |
| 渲染面（`l2d-wasm-demo`）的 `action-state` 处理与 `action_frames` | 整体在 `#[cfg(target_arch = "wasm32")] mod web` 内，**原生 `cargo test` 覆盖不到**，改它只能靠 wasm 重建 + 肉眼看画面；而爆炸半径是**舞台不渲染**。且没有任何东西会再发 `action-state`（惰性死代码） | 真要删：wasm 重建 + 确认皮套仍渲染、口型仍动 |
| **待机生命体征**（`IdleState` / 呼吸 / 眨眼 / 微表情） | **不是动作系统**，只是共用 override 层 | **必须保留**（「外观与互动 → 待机小动作」就是它）。删动作残件时**绝不要连带删它** |
| `GET /api/v1/app/capabilities` 仍广告 `actions` / `action_sources` / `strength_levels` | 前端已不再读，但属**残留 wire 契约** | 可清；需同步后端 DTO + 其测试 |
| Mod→动作桥（`HostChannels::trigger_action` → `SupervisorHandle::trigger_action`） | 投影删除后**不产生任何桌面副作用**，完全惰性 | 可删；会外溢到 `mod_registry` 与多份测试 |
| 动作完成通道（`report_action_finished` / `ActionFinishedFact` / `ShellApp.performing`） | `performing` 恒为 `None`（无写入点），通道不可达 | 整段删除会外溢到 supervisor/turn 及多份测试 |
| `docs/` 里若干陈旧引用（`observability.md` 的 `IncompleteTools` 表行等） | 历史计划文档不该改写；但 `observability.md` 读起来像**现行契约表** | 只修 `observability.md` 那一行 |
| **moc3 版本覆盖度** | `crates/l2d` 是 `is_supported_v0()`：**v0 只锚定 Bai 实测档位（moc3 raw 字节 4 = Cubism 4.2）**，更新的版本明确报"不支持" | "通用接入容器 / 不限定模型"这个定位**目前工程上不成立**——比许可问题更早会咬到用户。要对外讲这个定位，先扩覆盖度 |
| 商标 / 专利 / 许可 | 用户裁决：「**尽可能避免即可，盈利了再说**」 | 已做的：树里**没有**任何 `*cubismcore*`（`Live2D-Ai-pc/` 与 `-Android/` 都已归档、不在工作树），Rust 线走 **Ayagami**（MIT/Apache），1.4/1.5 不适用。**待办只有两件便宜的**：加商标免责声明（全仓目前 0 命中）、把 `LICENSE-SCOPE.md`/`NOTICE` 里用现在时描述的 PC/Cubism 段落改成「已归档、不在当前产品内」（否则下一个人可能去恢复它） |

---

## 6. 下一轮建议顺序

1. **写 system prompt**（用户侧，零代码）——直接减少「不说话/跑偏」。
2. **上层体验升级**（用户说的"上层"）：现在基线干净了，可以在 `/app` 上做视觉与交互。
3. `docs/` 陈旧引用 + `capabilities` 残留字段清理（半小时内，纯清理）。
4. moc3 覆盖度调研（决定对外定位能不能讲）。
5. 发版时 bump 版本 + 打 `v0.5.2`。

---

## 7. 必须知道的坑（**都踩过**，含一次我自己的误判）

### 7.1 真机点火（无头浏览器）

```text
① 视口默认 0×0  → 必须先 resize（如 1280×860），否则 CanvasKit 不渲染、拿不到任何证据
② 语义树默认关闭 → 先点 flt-semantics-placeholder（"Enable accessibility"），
                    之后才拿得到按钮/文本节点来驱动界面
③ 用真实键入/点击驱动输入框 → MCP 的 fill 是**直接写 DOM textarea**，会让 Flutter 的
   editing 状态与 controller 失同步；表现为「发送后输入框看起来没清空」。
   **那是探测手段的假象，不是缺陷**（main.dart:503 明确 _input.clear()，
   实测按回车也不会重发）。
④ 别用 PulseAudio 的 sink-inputs 判断「无头实例有没有出声」——我犯过这个错：
   我的无头实例在隔离环境里，它的音频不走 WSLg 那台 PulseAudio，所以读到 0 并不
   证明它静音。**要证明，用 DOM 证据**（<audio> 元素、blob、currentTime）。
```

**点火要看的七项证据**（都是 DOM/网络级）：壳起来（`flt-glass-pane` + 视口非 0）、
舞台在渲染（iframe 内 canvas 尺寸非 0）、**渲染面能回话**（点放大 → `stage-ack` →
父页读数变化）、WS 通、LLM 有正文、**音频真在播**（`blob:true` + `paused:false` +
`duration` 是实数 + `currentTime` 推进）、**口型真在动**（`mouth` 消息大量非零）、
**一句一单元**（同时播放元素数恒 ≤1）、断网红线（无外部源）。

### 7.2 音频是**广播**的 → 多个页面 = 回音

服务端把音频广播给**所有**连上的客户端（设计如此）。**开了两个 app 页面/窗口/设备，
每一页各播一遍**，几十毫秒错位听起来就是回音。

> **排查口诀：一有"回声/叠音"，第一个检查项永远是"开了几个 app 页面"。**

（本轮就发生过：我验收时把无头浏览器开在 `/app/`，而用户也开着页面 → 两份声音。
用户关掉多余页面后回声消失。如果以后要做防护：`document.hidden` 时不启新句，
让后台标签页不再叠音。）

### 7.3 渲染面把**正常进度日志也走 `console.error`**

模型加载 / GPU / FPS 遥测共 300+ 条 error —— 于是「看控制台有没有报错」这个手段
对渲染面**失效**，真异常会被淹没。要按文案判断。（可考虑改为 info。）

### 7.4 无头环境的 FPS 不可当性能结论

渲染面自报 FPS ~170（rAF 在无头实例里不受显示器节流），不代表真机帧率。

### 7.5 两条「样本数正好整除」的坑

- 句子样本数正好是 `audio_chunk_samples`（4800）整数倍 → 末块 0 样本 → **必须**仍发
  边界帧，否则那一句永远没有 `end`（§1.3 ③）；
- 分句器把**换行**当句读 → 会产生**纯空白句** → 必须拦住它别发 TTS（§1.3 ②，
  否则 TTS 400 是 fatal，整轮失败）。

### 7.6 装饰性符号先查字体子集

ASCII 之外的任何装饰符号（`▍`/`⌘`/图标字）都要先确认在自托管子集里；
**装饰用符号更该"画出来"而不是"打出来"**（§1.4）。

### 7.7 前端改动必须重建才生效

```bash
cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn
```

`--no-web-resources-cdn` **不是可选项**（否则 CanvasKit 走 Google CDN，断网白屏）。
静态文件由服务端按请求读盘，所以**前端改动只需重建、不必重启后端**。

---

## 8. 归档与恢复位置

| 内容 | 位置 |
| --- | --- |
| 本轮基线 tag | `baseline-core-chain-2026-09-11` |
| 上一版交接（已完成） | `docs/plans/HANDOFF-2026-09-11.md` |
| 基线说明（**先读这个**） | `docs/architecture/core-chain-baseline.md` |
| 音频路径方案 | `docs/plans/PLAN-audio-wav-path-2026-09-11.md` |
| 前端加强计划（P0–P4，已完成） | `docs/plans/PLAN-frontend-strengthening-2026-09-11.md` |
| 逐版变更 | `CHANGELOG.md`（顶部是本轮） |
| 作废的旧交接 | `HANDOVER.md`（v0.1.0 归档，别照它理解项目） |

---

## 9. 提交方式

本轮**一个提交**：改动在**同一批文件**里互相交叠（例如 `web_api/ws.rs` 同时含
动作投影删除、句界修复、空末块边界帧;`chat/chat_controller.dart` 同时含动作移除、
音频透传、收尾闸门），硬拆会让中间提交**编译不过**——那比"大提交"糟得多。
提交信息里按 §1.1–§1.5 分段说明。
