> 历史（2026-10-01 归档，勿当现网）。

# 稳定化预检结果（stabilize，2026-09-15）

> 本文件记录**在本 worktree 实跑**的点火预检：命令、时间、tip、逐项结果，
> 以及哪些项因为「没有 TTS / 没有 GUI / 没有 token」而被标 **SKIP**。
> 对应清单：[`IGNITION-CHECKLIST-stabilize.md`](IGNITION-CHECKLIST-stabilize.md)。

## 0. 环境与 tip

| 项 | 值 |
| --- | --- |
| worktree | `/home/skystar/Live2D-Ai-stabilize`（分支 `mod/stabilize`） |
| 真源 | `/home/skystar/Live2D-Ai-wave3` @ `57e32ae5` |
| **稳定化代码 tip** | **`0d33aa5b`**（`fix(mods): enable/disable 不再要求 Content-Type …`） |
| 预检执行时刻 | **2026-09-15 07:0x CST**（`2026-09-14T23:0xZ`） |
| 宿主 | WSL2（`Linux 6.18.33.2-microsoft-standard-WSL2`），8 核 /
      `CARGO_TARGET_DIR=/home/skystar/Live2D-Ai/target`（多 worktree 共用 target） |
| 服务 | `live2d-ai-desktop --web --http-port 18080`，`LIVE2D_AI_ALLOW_NO_ORIGIN=1`、
      `LIVE2D_AI_MUTE_AUDIO=1`（无人值守静音） |
| LLM | DeepSeek（`https://api.deepseek.com/v1`，key 来自 `.env`）——**可达** |
| TTS | `http://127.0.0.1:8080/v1` —— **未起**（故 §3 的「未收尾」路径被实测到） |
| 前端产物 | `shell/flutter/build/web`（本 worktree 现构建，`--no-web-resources-cdn`） |
| 渲染面产物 | `crates/l2d-wasm-demo/dist`（本 worktree `trunk build`，复用共享 wasm target） |

> **版本策略**：本波**未改** `Cargo.toml` / `pubspec.yaml` / README 的任何版本行，
> 仍是 **`0.2.0-rc.3`**。是否 bump 到 `0.2.0` 只在本波收束里**提议**，未执行。

---

## 1. 官方点火体检 `./scripts/ignite.sh --check`

```
==> 点火体检：http://127.0.0.1:18080
    [ok]   GET / → 302 Location: /app/
    [ok]   GET /app/ → 200
    [ok]   /app/index.html 不依赖 Google CDN
    [ok]   /app/main.dart.js 不依赖 Google CDN
==> 体检通过
```

**4/4 ok，exit 0**（CDN 红线通过 = 断网不会白屏）。

---

## 2. `./scripts/ignition-precheck.sh --fsm`（本波新增）

**结论：PASS 34 / FAIL 0 / SKIP 1**，exit 0。

### A. 壳 / 静态托管

| 结果 | 项 | 期望 | 实得 |
| --- | --- | --- | --- |
| PASS | `GET /` → 302 | 302 | 302 |
| PASS | `Location` | `/app/` | `/app/` |
| PASS | `GET /app/` | 200 | 200 |
| PASS | `GET /app/index.html` | 200 | 200 |
| PASS | `GET /app/main.dart.js` | 200 | 200 |
| PASS | main.dart.js 不依赖 Google CDN | 无 gstatic | 无 gstatic |
| PASS | `GET /render`（wasm 渲染面） | 200 | 200 |

### B. Mod 注册表

| 结果 | 项 | 实得 |
| --- | --- | --- |
| PASS | 已注册 id 集合 | `director,external-input,memory,persona,pet-desktop,voice-input,wallpaper`（7 个） |
| PASS | `external-input` 缺省启用 | `true` |

### C. `external/chat` 门禁

| 结果 | 项 | 期望 | 实得 |
| --- | --- | --- | --- |
| PASS | 正常注入 | 200 `ok:true` | 200 `ok:true` |
| PASS | `GET` 该端点 | 405 | 405 |
| PASS | 非 JSON CT | 415 | 415 |
| PASS | 空 text | 400 `invalid_payload` | 400 |
| PASS | 超长 text（2001 字） | 400 `text_too_long` | 400 |
| PASS | 无 Origin（allow_no_origin=开） | 200 或 403 | **200** |
| **SKIP** | 已配 token 却缺 token → 401 | 401 | **未配置 `EXTERNAL_INPUT_TOKEN`** |
| PASS | `POST …/disable`（无 body 无 CT） | 200 | **200**（本波修复点） |
| PASS | 停用后注入 | 403 `mod_disabled` | 403 `mod_disabled` |
| PASS | `POST …/enable`（无 body 无 CT） | 200 | **200**（本波修复点） |
| PASS | `external-input/state` 键 | accepts,busy,ready,rejects,v2_ignored | 一致 |

### D. state 404 vs 503 分界

| 结果 | 项 | 期望 | 实得 |
| --- | --- | --- | --- |
| PASS | 未知 Mod state | 404 `not_found` | 404 |
| PASS | `persona` state | 503（未实现 `state_json`） | 503 |

### E. `voice/transcript` 门禁

| 结果 | 项 | 期望 | 实得 |
| --- | --- | --- | --- |
| PASS | voice-input 缺省停用 | 403 `mod_disabled` | 403 |
| PASS | 启用 voice-input | 200 | 200 |
| PASS | 注入 + 清洗/归一化（U+3000 / U+200B） | 200 `text=把窗户关小一点` | 200 `把窗户关小一点` |
| PASS | 复原（disable） | 200 | 200 |

### F. sidecar 离线自检

| 结果 | 项 | 实得 |
| --- | --- | --- |
| PASS | `voice_sidecar.py --selftest` | `全部通过（70 项检查；离线，未发起任何请求）` |
| PASS | `bilibili_sidecar.py --selftest` | `selftest OK（16 项断言）` |

### G. 七 Mod `enable → state → disable` 矩阵（`--fsm`）

| 结果 | Mod | enable / state / disable |
| --- | --- | --- |
| PASS | external-input | 200 / 200 / 200 |
| PASS | pet-desktop | 200 / 200 / 200 |
| PASS | persona | 200 / **503** / 200（无 `state_json`，503 正常） |
| PASS | voice-input | 200 / **503** / 200（无 `state_json`，503 正常） |
| PASS | wallpaper | 200 / 200 / 200 |
| PASS | memory | 200 / 200 / 200 |
| PASS | director | 200 / 200 / 200 |

### 归一化实测（供核对，`E` 项的原始响应）

```json
{"backend":"mock","locale":"zh-CN","ok":true,"text":"把窗户关小一点"}
```
（输入 `"  把<U+3000>窗户<U+200B>关小一点  "`：全角空格与零宽字符都被清洗掉。）

### 该矩阵下各 Mod 的 state 形状（本轮实取，供清单核对）

- `director`：`{channel:"none", decisions:0→1, delivered:false, turns_ended:0→1,
  recent_decisions:[{emotion:"neutral", intent:"chat", suggested_tts:{speed:1.0,pitch:1.0},
  closed:true, delivered:false, seq:1}]}` —— **零投递**，与裁决一致。
- `wallpaper`：`{mode:"off", active:false, decision:{kind:"none"}, playlist_len:0,
  prefs_patch:null, reason:"mode_off"}`。
- `pet-desktop`：`{always_on_top:true, click_through:false, opacity:0.95,
  voice_active:false, window:{opened:false, reason:"native_shell_dormant"}}`。
- `memory`：`{enabled_injection, writes, hits, injects, injected, errors, evicted,
  last_hits, max_records:200, top_k:3, turns_seen, store_path}`。

---

## 3. 无头浏览器自点火（本轮**强制加做**）

**为什么做**：`cargo test` 全绿 ≠ 界面是对的（本项目自己的教训）。CSS/GPU/托管层/
前端 gate 只有真的在浏览器里跑一遍才露出来（本波的 415 正是这样抓到的）。

**怎么搭**（无系统浏览器，装 Playwright 的 Chromium）：

```bash
npx --yes playwright@latest install chromium
export VK_ICD_FILENAMES=~/.cache/ms-playwright/chromium-1243/chrome-linux64/vk_swiftshader_icd.json
chrome --headless=new --no-sandbox --enable-unsafe-webgpu --enable-features=Vulkan \
       --use-vulkan=swiftshader --use-angle=swiftshader --enable-unsafe-swiftshader \
       --remote-debugging-port=9222 about:blank
```

### 3.1 逐项结果

| 结果 | 项 | 证据 |
| --- | --- | --- |
| PASS | `/app/` 加载 + 壳渲染 | a11y 树出现：`heading "Live2D Ai"`、`状态：空闲`、`实时通道：实时通道正常` |
| PASS | **零外部请求**（断网红线） | 一次加载 16 个请求**全部** 127.0.0.1：18080（含 `canvaskit.wasm`、两个 Noto 字体）；**无 gstatic / gstatic fonts** |
| PASS | 模型资产可达 | `/models/bai/runtime/bai.model3.json` 200，纹理/physics/cdi/moc3 全 200 |
| PASS | **舞台真渲染** | HUD：`GPU: webgpu/WebGPU`、`FPS 60.1`、`bg: solid`、`idle: b0.63 blO1383` |
| PASS | **画面在动（不是静止图）** | 两帧相隔 1.6s：**舞台区 1.29% 像素变化**，右侧面板 **0.0000%**（UI 静止、角色呼吸/眨眼） |
| PASS | WS 实时通道 | UI 文案「实时通道正常」 |
| PASS | **聊天框一轮** | 输入并发送 → 产出 user+assistant 两条 |
| PASS | **文本已进主链** | localStorage `live2d-ai.chat-sessions`：`{"role":"assistant","text":"你好，点火预检已就绪，随时准备为你服务！","epoch":0,"unfinished":true}` |
| PASS | 错误契约到界面 | 服务端 `code=tts_transport stage=tts fatal=true`；界面出现同码 + `去语音合成设置` 按钮 |
| PASS | Mod 启停（本波修复点） | 页面内 `fetch('/api/v1/mods/memory/enable',{method:'POST'})`（**无 CT**）→ **200**（修复前 415） |

### 3.2 关键日志（服务端，一轮失败收口）

```
INFO mod: external-input 收到事件 turn_prompt: 点火预检：用一句话打个招呼
你好，点火预检已就绪，随时准备为你服务！              ← LLM 正文（已进主链）
ERROR …: TTS 阶段错误: HTTP 传输错误: error sending request for url
      (http://127.0.0.1:8080/v1/audio/speech) code=tts_transport stage=tts
      epoch=0 fatal=true hint="连不上 [tts] 端点：确认服务在跑、端口/地址可达"
INFO mod: external-input 收到事件 turn_ended: 1              ← 失败也发
```

> 这条正是清单 §1.3 要写明的口径：**TTS 未起 ≠ 文本没进主链**。回复上屏并落盘
> （`unfinished:true`），轮次在 TTS 阶段 fatal 收口，UI 给出可执行处置。

### 3.3 本轮无头检查**够不到**的（如实标注）

| 项 | 为什么够不到 |
| --- | --- |
| 真实听感（出声/音色） | 无头环境无音频设备；且本机 TTS 未起 |
| 口型随动（肉眼） | 依赖真实音频包络；本轮无 TTS |
| 壁纸真实换图观感 | 需要交互式选图 + 肉眼 |
| 桌宠真窗口 | 既定休眠（原生壳非主线），本就不做 |
| Windows 上 Chrome/Edge 的 WebGPU 差异 | 只在 WSL 无头 Chromium 里验过 |

---

## 4. 门禁（同 tip 实跑）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| Rust 测试 | `cargo test --workspace --all-targets` | **1081 passed / 0 failed**（Wave 3 基线 1078；本波 +3 条回归） |
| Rust doc | `cargo test --doc --workspace` | **3 passed / 0 failed** |
| 格式 | `cargo fmt --all -- --check` | **clean** |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0 / 0 warning** |
| Rust 占比 | `cargo run -p xtask -- rust-ratio` | **96.2897% PASS**（门槛 95%；dart 33608 行豁免） |
| 前端 | — | **未改 `shell/flutter/**`**，故按约定未跑 `flutter analyze/test`（Flutter Web 产物已重建用于浏览器检查） |

---

## 5. 结论

- **机器侧全绿**：`ignite.sh --check` 4/4、`ignition-precheck.sh` **FAIL 0**、
  cargo 五件套全过、两 sidecar 自检全过。
- **浏览器侧真渲染**：舞台 WebGPU 60 FPS + 画面在动 + 零外部请求 + 一轮对话正文上屏。
- **唯一 SKIP**：token 鉴权项（本机未设 `EXTERNAL_INPUT_TOKEN`）。设了 token 后
  重跑即可覆盖（清单 §3.3）。
- **未做**：真实 TTS 出声 / 口型肉眼 / 壁纸观感 / 桌宠真窗 —— 留给用户 Win 侧。
- **不声称「真实点火已完成」**：以上是 agent 侧的脚本 + 无头证据，不是用户的肉眼验收。
