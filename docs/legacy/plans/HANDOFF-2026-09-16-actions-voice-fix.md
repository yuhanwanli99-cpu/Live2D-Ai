> 历史（2026-10-01 归档，勿当现网）。

# HANDOFF 2026-09-16 —— 动作幅度/半身随动 + 语音识别进主链

> 范围真源：`docs/plans/PLAN-actions-voice-memory-2026-09-15.md` 的 **§1/§2（动作）** 与 **§4（主页语音）**
> 两条，按现网反馈修补。**未做**（任务书禁止）：motion3 / exp3 加载器、异步导演 LLM、记忆大改、本地 ASR。
> 工作树：`/home/skystar/Live2D-Ai-l1`（`mod/l1-product`）。**不 bump / 不 push / 不打 tag。**

---

## A) 动作：幅度 + 真摇头 + 半身随动

### A.1 前后对照（渲染面 `preset.rs` 的 `PRESETS`）

| id | 改前 | 改后 | 说明 |
|---|---|---|---|
| `shake` | `ParamAngleX 16` × `sin(pi*p)`（**单峰**：只往一边推一次） | `ParamAngleX 24` + `ParamBodyAngleX 9`，波形 `MotionWave::Oscillate{cycles:2.0}`（`sin(4*pi*p)`，2 个完整来回） | 叠同一个 `sin(pi*p)` 包络，**首末仍恰为 0**（撤销不跳变） |
| `nod` | `ParamAngleY -18` | `ParamAngleY -20` + `ParamBodyAngleY -8` | 同向，身幅 = 头的 0.40（任务书 0.3~0.5） |
| `look_left` | `AngleX 22, AngleZ 4` | `AngleX 24, AngleZ 5, BodyAngleX 8, BodyAngleZ 2` | 身略小（≈头的 1/3） |
| `look_right` | `AngleX -22, AngleZ -4` | `AngleX -24, AngleZ -5, BodyAngleX -8, BodyAngleZ -2` | 镜像 |

- 幅值口径（Cubism 标准量程）：头 `ParamAngle*` ±30 → 用到 ±20~24；身 `ParamBodyAngle*` ±10 → 用 ±2~9。
- 红线不变：**只允许 Face / Head / Body 标准角**，不碰口型（`ParamMouthOpenY`）/ 手臂 / 特效。
- 波形是 `PresetSpec` 的新字段 `wave: MotionWave`（`Single` = 老行为，逐值等价；`Oscillate{cycles}` 非法 cycles 回落 `SHAKE_CYCLES`）。

### A.2 调试面板强度

- `kDefaultPresetIntensity = 1.2`（`live2d_stage.dart`）：任务书「1.0 是微抖、肉眼看不出来」，出厂提到 1.2。
- `ActionDebugPanel` 新增强度滑条（0.5~3.0，默认 1.2），经 `onApplyPreset(id, intensity)` → `applyPreset(..., intensity:)` → `preset` 帧；渲染面钳 `[0,3]`。

### A.3 回归（原生 `cargo test`）

| 测试 | 锁什么 |
|---|---|
| `shake_oscillates_both_ways_with_body_in_phase` | 首末恰 0；采样 40 点 **变号 ≥2**；峰值 ≥16；`ParamBodyAngleX` 与头**同相**且幅值比 ∈ 0.3~0.5 |
| `nod_and_look_carry_body_follow` | nod/look 表里必须出现 `ParamBodyAngle*`、同向、身幅更小 |
| `presets_stay_inside_the_allowed_channel` | 通道白名单加 Body；**幅值分档**：Body ≤10、其余 ≤30 |
| `motion_gain_falls_back_and_stays_bounded` | 非法 cycles 回落；|gain| ≤1；`Single` 与老 `envelope` 逐值等价 |

---

## B) 语音：识别成功必须进主链

### B.1 断点根因（一句话）

**转写进主链的路径是 `POST /api/v1/voice/transcript` → `supervisor.say`，而前端没有任何一处在 `ok:true` 后把用户句写进聊天区**
（`ChatController` 只在用户按发送的 `send()` 里建用户气泡）——所以日志里 `turn_prompt` 已进主链、
AI 也回了话，**聊天区却只有回答、没有用户那句**；表现为「识别了但没发出去」。

### B.2 修了什么

1. **用户句上屏 + 开一轮**：`ChatController.acceptInjectedUserTurn(text)` + `main.dart` 把
   `_voiceApi.sendTranscript` 包一层：`ok:true` 时用服务端剥词后的 `result.text` 上屏（空串回落原文）。
2. **Mod 未启用 → 常驻红字**：新增 `kVoiceModDisabledMessage`（「语音输入未启用：请先在「设置 → Mod 管理」
   启用 voice-input」）。`_refreshVoiceModState()` 启动取一次 `GET /api/v1/mods`；控制器加
   `loadModEnabled` 预检（`start`/`pressStart` 都在起识别前拦下）。读失败**不误报**（`null` → 不显红字，
   端点 403 仍兜底）。
3. **busy 说清去向**：`200 ok:false` 文案改为「主链忙（busy）：角色还在说话，本条已填回输入框，改字后再发」
   （`onBusyResult` 落输入框不变）。
4. **不再谎报命中**（Rust）：`wake_phrase_matched` 现在如实计算——PTT 裸正文 **false**（旧实现恒 true）。

### B.3 证据

**API（curl，Origin = loopback）**——voice-input 已启用、`wake_phrase = 小可爱`（缺省）：

```text
POST /api/v1/voice/transcript {"text":"小可爱你好呀","ptt":false}
  → 200 {"ok":true,"text":"你好呀","wake_phrase_matched":true,"ptt":false,...}
POST /api/v1/voice/transcript {"text":"按住说话测试","ptt":true}
  → 200 {"ok":true,"text":"按住说话测试","wake_phrase_matched":false,"ptt":true,...}
```

**后端日志（`~/.local/share/live2d-ai/logs/live2d-ai.log.2026-09-16`）**——两条都进了 say / LLM：

```text
... turn_started: 1 ... turn_prompt: 你好呀
... turn_started: 2 ... turn_prompt: 按住说话测试
```

**无头浏览器（Chromium 153 + SwiftShader WebGPU，`http://127.0.0.1:18080/app/`）**：

- 动作：设置 → 开发模式 → 动作调试，逐个点 `nod`/`shake`/`look_left`/`look_right`，面板本地状态
  「最近触发：<id> · 来源 debug · 约剩 N ms」与**渲染面 HUD**「`preset: <id> src=debug Nms`」同时命中；
  调试面板显示「强度 1.2 倍」。
- 语音主链：环境无真实麦克风。用 CDP 在页面内 patch `SpeechRecognition.prototype.start`（注入等价定稿
  「小可爱你好呀」），点「听」走**真实的 main.dart 路径** → 聊天区出现「**你说：你好呀** / 助手说：你好呀！
  很高兴见到你，有什么我可以帮你的吗？😊」；日志 `turn_started: 3` / `turn_prompt: 你好呀`。
  截图：`/tmp/act_voice_bubble.png`。
- Mod 未启用：`POST /api/v1/mods/voice-input/disable` 后刷新，**未点任何按钮**即在「听」按钮旁出现红字
  （截图 `/tmp/act_mod_off_hint.png`）。

> **诚实标注（环境限制）**：无头 Chromium 下 WebGPU canvas **不会被 CDP 截图重新捕获**——
> 实测「静止 vs look_left（bridge 下发，ttl 4s）」stage 区域像素差 **0**，而「显示 vs 隐藏 iframe」
> 差 YAVG 176。也就是说无头只能给「渲染面确实收到了 preset」（HUD 文本）与单测的数值幅度，
> **看不出**摆头幅度。真机肉眼验收仍需 Windows 浏览器（AGENTS 的 WSL2↔Windows 分工不变）。

---

## 门禁（本机实测）

| 检查 | 结果 |
|---|---|
| `cargo test --workspace --all-targets` | **1257 passed / 0 failed** |
| `cargo test --doc --workspace` | 3 passed / 0 failed |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 warning |
| `cargo run -p xtask -- rust-ratio` | **96.7604% PASS**（78494/81122；dart 40832 行豁免可见） |
| `flutter analyze` | No issues |
| `flutter test` | **975 passed**（rc.5 的 833 + L1 新增/本轮新增） |
| `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` | ok（`trunk build` 已重建 dist） |

产物：`crates/l2d-wasm-demo/dist`（trunk build）与 `shell/flutter/build/web`（`flutter build web --release
--base-href /app/ --no-web-resources-cdn`）均已重建；desktop 已 `cargo build` 后重启。

---

## 本轮踩到的两个环境坑（下次别再挖）

1. **`trunk build` 会被 `NO_COLOR=1` 打死**：trunk 0.21.14 把 `NO_COLOR` 当布尔 flag 解析，
   `NO_COLOR=1` → `error: invalid value '1' for '--no-color'`，而它经管道时退出码被 `tail` 吃掉（假绿）。
   修法：`env -u NO_COLOR trunk build`（或 `NO_COLOR=true`）。**验证 wasm 是否真的重建**：
   `strings dist/*_bg.wasm | grep ParamBodyAngle`。
2. **本 worktree 没有 `.env`**：密钥真源在主树 `/home/skystar/Live2D-Ai/.env`。直接起 desktop 会
   `has_api_key:false`（LLM 401）。为验证重启服务时按 `ignite.sh` 的做法 `set -a && . <主树>/.env && set +a`。

---

## 改了哪些文件

- 动作：`crates/l2d-wasm-demo/src/preset.rs`、`preset_tests.rs`
- 调试面板：`shell/flutter/lib/live2d/live2d_stage.dart`、
  `lib/settings/sections/dev_tools_section.dart`、`lib/app/shell_settings.dart`
- 语音前端：`lib/voice/voice_listen_controller.dart`、`lib/chat/chat_controller.dart`、`lib/main.dart`、
  `lib/app/shell_admin.dart`、`lib/app/app_shell.dart`、`lib/ui/chat_panel.dart`
- 语音后端（诚实性）：`crates/live2d-ai-desktop/src/web_api/voice_routes.rs`（+ 对应测试）
- 测试：`shell/flutter/test/voice_listen_controller_test.dart`、`chat_listen_button_test.dart`、
  `developer_section_test.dart`
