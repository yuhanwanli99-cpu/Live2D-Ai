# R4-T3 证据：探针截图判据修复（A）+ 真实音频链路验收（B）· 2026-10-06

> 任务：共享板 **task-7 / R4-T3**（owner: probe-fix）
> 工作树：`/home/skystar/Live2D-Ai-fe`（分支 `main`）；被测服务 `http://127.0.0.1:18080`（Lead 的后台 job）
> 探针：`scripts/browser_probe.mjs`（本轮**只改这一个源文件**）

---

## 0. 一分钟结论

| 目标 | 结论 | 关键数字 |
| --- | --- | --- |
| **A. 截图判据**（N4） | **完成** | 根因两个都实测定位并修掉；`node scripts/browser_probe.mjs all` 连跑两次，`5a/5b/5d/offline` 全部 **pass**、两次逐条一致（旧 run 里这四条是 fail/blocked 型「全白截图」） |
| **B. 真实音频链路** | **完成** | 一轮对话：WS `audio` 帧 **131** 条、句子 **1** 句、`start` **1** 个 / `end` **1** 个、样本 **62400**（=2.6 s @24 kHz）、`muted=false`；页面 `<audio>` src=`blob:`、`duration=2.6`、`currentTime` **0.0417 → 1.8843 s** |

---

## 1. 怎么复现（照抄即可；环境是 Lead 起好的，**不要**重启服务或另起 Chrome）

```bash
cd /home/skystar/Live2D-Ai-fe
# 0) 前置（应当已在跑；不满足就别跑，别自己起第二份）
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:18080/app/      # 200
curl -sS http://127.0.0.1:9222/json/version | head -3                      # Chrome/153 headless

# 1) 目标 A：全量 12 个场景（每次约 6–10 分钟）。**重命令单飞**：
flock /tmp/l2d-browser.lock -c 'PROBE_CDP=http://127.0.0.1:9222 node scripts/browser_probe.mjs all'

# 2) 目标 B：真实音频链路（需要活端点：LLM + 本机 TTS）
#    audio **不在 all 里**（见 §5）：它要真发一轮对话，缺端点时应显式跳过而不是天天红
flock /tmp/l2d-browser.lock -c 'PROBE_CDP=http://127.0.0.1:9222 node scripts/browser_probe.mjs audio'

# 3) 只看汇总
node scripts/browser_probe.mjs summary
```

证据默认落 `docs/verification/evidence-2026-10-06/`（本目录）；
像素统计用 `docs/verification/evidence-2026-10-05/png_stats.py`（零依赖 stdlib；**只读**，本轮未改、未复制）。

被测对象元数据（两次 `all` 都在这个状态下取数）：

| 项 | 值 |
| --- | --- |
| 源码 HEAD（两次 `all` 都打印了同一个值） | `66798b11a10e412fe293f1a88ea205c00c5ca339` |
| 探针脚本 sha256 | `e57bcf4a62b20466292875cecf6f102868721c48eaf42b8bafd816073ad3e9c6` |
| 后端 | `http://127.0.0.1:18080`（Lead 的后台 job；pid 54115，环境**无** `LIVE2D_AI_MUTE_AUDIO`） |
| 浏览器 | CDP `http://127.0.0.1:9222`，`Chrome/153.0.8010.12` headless=new + swiftshader（Lead 起的那一份，本轮**未重启、未新建**） |
| LLM / TTS | `https://api.deepseek.com/v1`（deepseek-flash，key 来自服务进程 env）+ 本机 CosyVoice `http://127.0.0.1:8080/v1`（实测 `POST /v1/audio/speech` 200 / 124800 B / 5.46 s） |
| 前端产物 | `shell/flutter/build/web/main.dart.js` mtime = 2026-10-05T12:09:03Z（**比最新 dart 旧** ⇒ `8b` 红，见 §5.3） |

---

## 2. 目标 A：截图判据修复

### 2.1 根因（两个，都用同一页面同一时刻的对照实验钉死）

原始逐帧数据：[`clip-vs-full-same-instant.json`](clip-vs-full-same-instant.json)、[`screenshot-mode-matrix.json`](screenshot-mode-matrix.json)。

1. **`clip` 单独就会回全白帧。** 同一时刻、同一页面：
   - 整页帧（不带 clip）= 53131 B，主导色 `000000` 占 **77%**（真内容）；
   - 带 clip（1100,52,340×848）= 2434 B，**100% `ffffff`**。
   ⇒ 带 clip 的 `Page.captureScreenshot` 在本环境走的是另一条捕获路径。
2. **不带 `captureBeyondViewport` 时整页帧也可能是全白。** 8 次采样里 A(`{format:'png'}`) 出现 1 次
   5851 B / 100% 纯白，其余 51899 B 真画面；而 B(`captureBeyondViewport:true`) / C(`fromSurface:false`)
   **8/8 都是真画面**（51899 B；主导色 ffffff 71.57% 恰好是舞台 iframe 那块 WebGPU canvas 的面积，其余是壳）。
   ⇒ 它把「回放合成器上一帧」换成「强制重新光栅化」。

### 2.2 改了什么（都在 `scripts/browser_probe.mjs`）

| 改动 | 内容 |
| --- | --- |
| `SHOT_PARAMS` | `{format:'png', captureBeyondViewport:true}`，**全脚本唯一截图参数**（头注写明两条实测依据） |
| `Page.shot()` | 只做整页截图；**传 `clip` 直接抛错**（防回归），裁剪一律 `png_stats --crop` |
| `Page.waitForPaint()` | 整页帧 + `--crop` 壳区判「是不是 100% 纯白平帧」；返回 `paintedAfterMs / dominant / bytes / samples`，判不出来时返回 `null` **并且带 `why`** |
| `blankWhite()` | 「不可判读帧」的唯一判据：主导色占比 >99.9% **且** 纯白 ⇒ 该条判 **blocked（环境）**，不是产品 fail |
| 背景注入 | 走**真字节库**（IndexedDB：db `live2d-ai` v1 / store `backgrounds` / key=`bg`+8 位十六进制 id），localStorage 只留 `{kind:'image', id}`；id 用与 Dart `backgroundFingerprint` **同算法**（djb2，h=(h*33+c) mod 2^31-1）算出 |
| 阈值注释 | `REDNESS_ON=20 / REDNESS_OFF=8 / DIFF_MIN=0.005` 的来由全部写在脚本里（见 §4） |
| 跨源分类 | `net` 场景新增 `1d`：跨源请求**按 host 分类打印**（`net-crossorigin.json`），只打印不当判据 |
| `audio` 场景 | 新增（目标 B），**不进 `all`** |
| 证据目录默认值 | `OUT` 默认改为 `evidence-2026-10-06`（10-05 那份是本轮**只读**的前轮证据，不覆盖） |

### 2.3 两次 `all` 的原始汇总

见 [`run-all-pass1.json`](run-all-pass1.json) / [`run-all-pass2.json`](run-all-pass2.json)（各自是本轮 `all` 结束时的 run.json 快照）。

| 轮次 | 汇总时间（UTC） | 项数 | pass | manual-only | fail | 目标 A 六条 |
| --- | --- | --- | --- | --- | --- | --- |
| pass 1 | 2026-10-05T13:12:06Z | 42 | 35 | 4 | 3 | `5a/5b/5c/5d/5e/offline` **全 pass** |
| pass 2 | 2026-10-05T13:19:50Z | 42 | 35 | 4 | 3 | `5a/5b/5c/5d/5e/offline` **全 pass** |

- **同一脚本**：`sha256(scripts/browser_probe.mjs) = e57bcf4a62b20466292875cecf6f102868721c48eaf42b8bafd816073ad3e9c6`
  （两次运行前后各取一次，完全一致）。
- **逐条对照**：两次共 **39 个非 audio 条目，判定**0 处不同**（`all-two-pass-comparison.json`）。
  （42 项里含 3 条 audio 结果，是上一节单独跑的那一轮被 run.json 聚合进来的；两次内容一致，对比时已排除。）
- **两次的 3 条 fail 是同 3 条，且都与 task-7 无关**：
  - `8b` 前端产物新鲜度 —— 其他 teammate 正在改 `lib/*.dart`，产物由 Lead 独占重建；
  - `F-fonts-02` / `F-fonts-03` —— 字体运行期回落，由 **R4-T4** 修（`web/flutter_bootstrap.js`），本轮未落地。
- 4 条 manual-only：`1d`（跨源分类，只打印）、`3d`/`6c-pixel`（舞台 WebGPU canvas 拍不到，见 §5.2）、
  `5g`（拖动手柄命中点未知，需人工）。

### 目标 A 的关键原始值（两次逐字相同）

| id | pass1 | pass2 |
| --- | --- | --- |
| `5a` | 无图 redness **-4.33** → 有图 **50.45**；IndexedDB 键 `["bg6a2d74a9"]`；waitForPaint 600/600 ms | 同上（waitForPaint 584/632 ms） |
| `5b` | 刷新后 redness **50.45**；偏好只剩 `{kind:'image',id}`；IndexedDB 键 `["bg6a2d74a9"]` | 同上（waitForPaint 601 ms） |
| `5c` | 清图后 redness **-4.33** | 同上 |
| `5d` | 差异 `cover_vs_stretch 0.0079`、`cover_vs_tile 0.1534`、`contain_vs_stretch 0.2621`、`contain_vs_tile 0.2712`、`stretch_vs_tile 0.1466` | 逐字相同 |
| `5e` | 红 50.45 / 蓝 -30.31 交替，截图字节 52243/52266 | 逐字相同 |
| `offline` | 外部 URL 0 条、壳区主导色 `111111` 占 0.9237、截图 **51899 B** | 同上（字节 51899） |


---

## 3. 目标 B：真实音频链路（LLM → TTS → WS `audio` → `<audio>`）

### 3.1 方法（全部是真实路径，没有一条是模拟）

- **WS 帧**：`Page.addScriptToEvaluateOnNewDocument` 在页面任何脚本之前 patch `window.WebSocket`，
  只留**投影**（`start/end/sentence_seq/samples/muted`，整段 base64 PCM 不进记录）。
  这比 `Network.webSocketFrameReceived` 可靠：一定拿到文本原文，且不把几 MB base64 写进证据 JSON。
- **驱动**：真实 UI —— 点进聊天输入框（`flt-text-editing-host` 的 shadow DOM textarea）→ `Input.insertText`
  写 `请只回一句话，以句号结尾：今天天气不错。` → 回车发送。**如实说明**：本次语义树里没找到「发送」节点
  （`typed.sendNode=null`、`sendHow='keydown-Enter'`），走的是 Enter；`typed.text` 证明字确实进了
  textarea，而链路证据（WS 帧 + `<audio>`）本身不依赖点哪个按钮。
  另外这次是真指针点击 + 回车，页面有用户激活，所以 `play()` 没被 autoplay 策略拦（见 §5.5）。
- **媒体元素**：直接查 `document.querySelectorAll('audio')` 的 `src/duration/currentTime/paused/readyState`。

### 3.2 原始数字

WS：`ws://127.0.0.1:18080/ws/state`，139 帧（131 条 audio、无 error 帧；另有 subscribe_ack / heartbeat / action_cue / text_delta / turn_state）。

| 判据 | 原始值 | 判定 |
| --- | --- | --- |
| 句子边界 | 句子 `sentence_seq=1`：**start 1 个 / end 1 个**（131 帧全归这一句） | **pass** |
| `sentence_seq` | `[1]`，从 1 起严格递增 | **pass** |
| 样本数 | **62400** 样本（= 2.6 s @24 kHz；首帧 480 样本 = 20 ms slice，末帧 `audio:""` 0 样本 + `end:true`） | **pass** |
| 静音分支 | 所有帧 `muted:false`；`maxVolume=0.162`（口型包络非零） | **pass** |
| 媒体元素 | 1 个 `<audio>`：`src=blob:http://127.0.0.1:18080/9d010d62-…`、`duration=2.6`、`readyState=4`、`paused=false`、`muted=false`、`volume=1` | **pass** |
| `currentTime` 前进 | **0.041687 s（t+6017 ms）→ 1.884309 s（t+8021 ms）**，Δ=**1.843 s** | **pass** |

**末帧 0 样本这件事本身就是一条判据**：62400 恰好是 `audio_chunk_samples`(4800) 的整数倍 13，
所以那一句的末块是**空块**——正是 AGENTS 里「空末块也要发边界帧」那条实测缺陷的对应面；
本轮它带着 `end:true` 到了，所以前端能封口（duration 2.6 s 与 62400/24000 完全对上）。

原始文件：[`audio/ws-frames.json`](audio/ws-frames.json)、[`audio/media-elements.json`](audio/media-elements.json)。

---

## 4. 判据阈值与来由（都在脚本里，这里给一份人读版）

| 常量 | 值 | 来由（2026-10-06 实测） |
| --- | --- | --- |
| `REDNESS_ON` | 20 | 壳区 redness = 逐像素 `(R-(G+B)/2)` 均值。实测：无图基线 **-4.33**；纯红图 cover = **+50.45**；纯蓝图 = **-30.31**。20 = 实测信号的 40%，比基线高 24.3 点；图只铺到壳区一半时 ≈ **23.1**，仍 >20 ⇒ 不会把「部分铺到」误判成没上屏。同页重复采样逐像素相同（红图两次都是 52243 B）⇒ 噪声远小于 1 点 |
| `REDNESS_OFF` | 8 | 「清图后回到底色」的上限 = 基线 + 12 点；实测清图后 = **-4.33**（与基线一毫不差） |
| `DIFF_MIN` | 0.005 | 两张整页帧至少 0.5% 像素不同（1440×900 × 0.005 = 6480 px）。实测四档 fit 的两两差异 = 0.0079 / 0.1466 / 0.1534 / 0.2621 / 0.2712 ⇒ 最小的那对（cover vs stretch）也有 1.6 倍余量 |
| `blankWhite` | share>0.999 且 `ffffff` | 判据场景全在**默认黑主题**（`loadApp({clear:true})` 后 theme 回落 black）下取像素，壳区不可能 100% 纯白；纯白只可能是无头合成器的空白帧 ⇒ 判 **blocked（环境）** |

> 交叉校验：本目录 5e 轮播读到的红/蓝 redness（+50.45 / -30.31）与上一轮 `evidence-2026-10-05` 报告里的
> 同两项**完全一致** —— 说明新的截图路径没有改变被测像素，只是不再把空白帧当结果。

---

## 5. 已知边界与风险（不粉饰）

1. **`audio` 不在 `all` 里**：它需要活端点（LLM + TTS）并会真的发一轮对话。按 AGENTS「端到端探针只在有活端点时跑、
   缺配置明确跳过」，它单独跑；要全量就 `all,audio`。
2. **舞台 WebGPU canvas 仍然拍不到**（无头 swiftshader 的固有限制，本轮**未**改变这一事实）。`3d`/`6c-pixel`
   仍是 `manual-only`；本轮的修复只保证**壳区**这一半的截图可信。
3. **`8b`（前端产物新鲜度）本次 FAIL**：其他 teammate 正在改 `shell/flutter/lib/**`，产物是 Lead 独占重建的，
   `main.dart.js` 必然落后于最新 dart。与 task-7 无关，交给 Lead 在全量重建后复核。
4. **`fonts` 的 F-fonts-02/03 本次仍 FAIL**：字体回落由 R4-T4 修（`web/flutter_bootstrap.js`），
   本轮只按任务书要求把**跨源请求按 host 分类打印**出来（`1d` / `net-crossorigin.json`），最终判定由 Lead 在全量重建后跑。
5. `audio-c` 依赖浏览器的 autoplay 策略：本次是**真指针点击 + Enter**，页面有用户激活，所以 `play()` 没被拦。
   若以后在无用户激活的路径上跑，这条应如实判 blocked（脚本已按 blocked 处理）。
