# E9 探针稳健性证据（audio-c 三路取证 + 四主题像素阈值）· 2026-10-06

> 共享板 **task-2**（owner: probe-robustness）· 工作树 `/home/skystar/Live2D-Ai`（分支 `main`）
> 写面：`scripts/**` + `docs/verification/**`（本目录）· 被测服务 `http://127.0.0.1:18080`（本探针自己起的，收工已停）
> **只改了一个源文件**：`scripts/browser_probe.mjs`

---

## 0. 一分钟结论

| 项 | 结论 | 关键数字 |
| --- | --- | --- |
| **`all` 连跑两遍**（冻结脚本） | **通过** | 40 项 ×2：**pass 36 / manual-only 4 / fail 0 / blocked 0**；两次**判定 0 处不同**；探针 sha256 `302dc9cf…` 两次相同 |
| **E9-② 四主题像素阈值** | **完成** | 四主题各自实测（各 3 次无背景 + 1 次红图，共 16 次采样）；`themes` 场景每次 `all` 用真实像素复核（6d）：四主题 **rednessDelta = 0**、share == 表值 |
| **E9-① audio-c** | **完成** | 真实链路跑 **2 次，两次全 pass、0 blocked**（旧实现「2 次里 1 次 blocked」）；另加**受控自证** `mediaspy`：元素被移除后记录器仍保住 0.664s 前进量，阴性对照证据确实丢失 |
| **`8b` 前端产物新鲜度** | **真绿**（重建后） | main.dart.js `2026-10-06T12:36:38.677Z` > 最新 dart `2026-10-06T11:40:22.892Z`（0 个 dart 比它新） |
| 未跑到的场景 | **0 条** | 12 个 `all` 场景 + `audio` + `mediaspy` + `themebase` 全部真跑过 |

---

## 1. 怎么复现（照抄）

```bash
cd /home/skystar/Live2D-Ai

# 前置：服务（本目录取数时是探针自己起的）
flock /tmp/l2d-heavy.lock ./scripts/ignite.sh          # 终端 A（18080）
# 浏览器（Playwright 自带 Chrome，必须带 WebGPU swiftshader，否则渲染面起不来）
CHROME=$HOME/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome
setsid nohup $CHROME --headless=new --no-sandbox --disable-dev-shm-usage --disable-gpu-sandbox \
  --remote-debugging-address=127.0.0.1 --remote-debugging-port=9222 \
  --user-data-dir=/tmp/probe-chrome-profile \
  --enable-unsafe-swiftshader --use-angle=swiftshader --use-gl=angle \
  --enable-unsafe-webgpu --use-webgpu-adapter=swiftshader \
  --window-size=1440,900 --hide-scrollbars about:blank > /tmp/probe-chrome.log 2>&1 &

# 1) 验收主命令（本目录的两次就是它）
flock /tmp/l2d-browser.lock -c 'PROBE_OUT=$PWD/docs/verification/evidence-2026-10-06-e9 node scripts/browser_probe.mjs all'

# 2) 真实音频链路（要活端点 LLM + 本机 TTS；audio 不进 all）
flock /tmp/l2d-browser.lock -c 'PROBE_OUT=$PWD/docs/verification/evidence-2026-10-06-e9 node scripts/browser_probe.mjs audio'

# 3) E9-① 的受控自证（约 40 秒，不需要端点）
flock /tmp/l2d-browser.lock -c 'PROBE_OUT=$PWD/docs/verification/evidence-2026-10-06-e9 node scripts/browser_probe.mjs mediaspy'

# 4) E9-② 重新标定四主题阈值（约 3–4 分钟；原始样本落 themes/shell-baselines.json）
flock /tmp/l2d-browser.lock -c 'PROBE_OUT=$PWD/docs/verification/evidence-2026-10-06-e9 node scripts/browser_probe.mjs themebase'
```

被测对象元数据（两次 `all` 都打印了同样的值）：

| 项 | 值 |
| --- | --- |
| 源码 HEAD | `cc68f061d7b45645fa0937296c1f9a29ac0e0376`（两次 `all` 相同） |
| 探针 sha256 | `302dc9cfa18b558e5e753d980e7261264546a33a1331b41b8fd4c6a8d6953b74`（**脚本自报**，见 §2.3） |
| 后端 | `http://127.0.0.1:18080`（探针起的；环境**无** `LIVE2D_AI_MUTE_AUDIO`） |
| 浏览器 | CDP `http://127.0.0.1:9222`，Chrome/153.0.8010.12 headless + swiftshader WebGPU |
| LLM / TTS | `https://api.deepseek.com/v1`（deepseek-flash）+ 本机 CosyVoice3 `http://127.0.0.1:8080/v1`（`sample_rate=24000`，`pcm`） |
| 前端产物 | `shell/flutter/build/web/main.dart.js` mtime `2026-10-06T12:36:38.677Z`（本轮由本任务重建，见 §6） |

---

## 2. 改了什么（全部在 `scripts/browser_probe.mjs`，483 insertions / 68 deletions）

### 2.1 E9-① audio-c：从「一路 2 秒轮询」改成「三路证据 + 稳定身份」

旧实现的三个脆弱点（就是「2 次里 1 次 blocked」的成因）：

1. 轮询间隔 **2000ms**，而本机 TTS 一句只有 **2.04s** 量级 ⇒ 元素建了又销毁时可能一次都没采到；
2. 元素身份用**数组下标** ⇒ 元素重建后下标移位，前进量算错元素；
3. 只认「blob + duration>0 + 前进>0.2s」的**唯一**证据源（DOM 查询），没有别的兜底。

现在（三路同时记，任一路给出可判读证据即可判 pass）：

| 路 | 机制 | 身份 | 生效证据（本轮实测） |
| --- | --- | --- | --- |
| A | DOM 轮询，间隔 **2000 → 500ms** | 页内自增 id（`probeId`）优先，无才回落下标 | pass1：12 次轮询，`id:1` blob/duration **2.04**/delta **0.324s**；pass2：10 次，delta **0.51s** |
| B | **页内媒体记录器** `AUDIO_SPY_SOURCE`（建页前注入）：patch `HTMLMediaElement.prototype.play` + document capture 监听 loadedmetadata/play/playing/**timeupdate**/ended/error/emptied… + MutationObserver 记 removed | 自增 id | pass1：8 条事件；pass2：**9 条**，delta **0.42s**（两次都独立满足 >0.2s） |
| C | **CDP Media 域**（`Media.enable` + `playerPropertiesChanged`/`playerEventsAdded`） | `playerId` | pass1：1 条属性 + 9 条播放器事件；pass2：0 条（本轮它只在极短窗口里跑，没抢到属性事件）⇒ 作为**独立佐证**存在，判定不依赖它 |

**语义没有放宽**：仍然是「blob 源 + duration>0 + currentTime 前进 >0.2s」才判 pass；证据不足仍判 **blocked**，且 blocked 串里现在必须带**三路采样数 / 帧载荷字节数 / why**（`mediaWhy`）。

### 2.2 E9-① 的受控自证 `mediaspy`（新增场景，不进 `all`）

audio-c 的失败形态在真链路上**不可控**（跑十次未必撞上一次），所以把它**人为构造**出来：

* `mediaspy-a`：页内现场造一个 `<audio>`（24kHz/1.2s 正弦 WAV blob，`muted+volume=0`）→ `play()` → 播 700ms 后**从 DOM 移除**。
  实测：移除前 DOM 看到 1 个 / 移除后 **0** 个（移除时 currentTime=0.6637），而记录器仍有 **12 条事件**、blob=true、duration=1.2、前进量 **0.664s** ⇒ **pass**。
* `mediaspy-b`（**阴性对照**）：同一个操作在一个**不注入记录器**的新页面上重放 ⇒ `window.__audio=null`、可用前进量 `[]`、DOM 0 个元素 ⇒ 证据确实丢失（= 旧实现的 blocked 形态）⇒ pass（证明 `mediaspy-a` 不是白给）。

### 2.3 顺带补的两件小事

* **探针自报哈希**：每条日志/run.json 现在打印 `# PROBE sha256 …`（脚本自己算自己的 sha256）。理由：验收是「同一脚本连跑两次逐条一致」，而日志里原来只有 HEAD，事后无法证明两次跑的是同一份脚本。
* 场景表更新：新增 `themebase`（标定）与 `mediaspy`（自证），**都不进 `all`**；`audio` 照旧不进 `all`（要活端点）。

### 2.4 E9-② 四主题像素阈值：把「只在黑主题量过」的常量换成实测表

老实现有三处**全局**阈值，全都只在默认黑主题上量过：`blankWhite()`（≥99.9% 平帧且纯白）、`scenarioOffline` 的 `1-share>0.02`（要求主导色 <98%）、`scenarioBg` 的 `REDNESS_ON=20 / REDNESS_OFF=8`。换成别套主题，同一个绝对阈值就不再是「基线上方多少点」。

现在引入 `SHELL_BASE` 表（脚本内，带完整来源头注）与三个按主题取值的函数：`blankFrame(st, wire)` / `rednessOn(wire)` / `rednessOff(wire)`，并且**主题自动取自页面当前偏好**（`themeWireOf(await page.prefsNow())`）——调用点不再需要、也不允许硬编码黑主题。

---

## 3. E9-②：四主题阈值表与来源（`themes/shell-baselines.json`）

**取样方式**：整页截图（`captureBeyondViewport`，全脚本禁止 clip）→ 进程内裁 `SHELL_CROP = 1100,52,1440,900`（聊天面板列，**不含**舞台 iframe）→ `png_stats.py`。
**样本量**：每主题 **3 次**无背景图采样 + **1 次**纯红 64×64 图（cover）采样，**共 16 次**（`node scripts/browser_probe.mjs themebase`）。
**噪声**：同一主题 3 次采样**逐字段相同**（share/redness 到小数点后 2 位一致），截图字节也相同（black 3×51899 B / white 3×51037 B / blue 3×51933 B / gray 3×51699 B）⇒ 噪声 < 0.01，比任何阈值余量小一个数量级。

| 主题 | shellHex | shellShare（实测） | maxShare（阈值） | redness 基线 | redSignal（本主题纯红图） | **REDNESS_ON** = 基线 + 40%×(信号−基线) | **REDNESS_OFF** = 基线 + 12 | blankHex |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| black | `111111` | 0.9237 | 0.9437 | −4.33 | +50.45 | **17.58**（旧常量 20） | **7.67**（旧常量 8） | `ffffff` |
| white | `ffffff` | 0.9243 | 0.9443 | +3.72 | +57.24 | **25.13** | **15.72** | `ffffff` |
| blue | `111133` | 0.9228 | 0.9428 | −16.76 | +40.88 | **6.30** | **−4.76** | `ffffff` |
| gray | `222222` | 0.9235 | 0.9435 | +4.47 | +57.33 | **25.61** | **16.47** | `ffffff` |

* `maxShare = 实测 share + 0.02`：用作「有没有真的画出来」的上限（超过说明这一帧平得不像产品）；
* **如实标注两处口径变化**：黑主题 `REDNESS_ON` 由手挑的 20 变成公式算出的 **17.58**（= 基线 + 40%×增量）。原注释写的是「实测红信号的 40%（20/50.45）」，现在统一成**相对本主题基线的增量**公式，四主题一致；安全性：黑主题「图只铺到壳区一半」时实测 ≈23.1，仍 > 17.58，且本轮实测红图 = 50.45（2.9 倍余量）。**判定结果没有因此变化**（5a/5b/5e 实测值与上一轮证据逐字相同）。
* ⚠ **白主题的 shellHex == blankHex == `ffffff`**：白主题壳区主导色**确实是纯白**（占比 0.9243，其余 7.6% 是文字/描边）。因此白主题下「整帧单色平帧且平色为 ffffff」既可能是合成器空帧、也可能是「产品只画了纯白」——**颜色不足以区分**，`blankFrame()` 返回 `'unknown'`，调用方判 **blocked（环境）**，**不许猜方向**。

### 3.1 `themes` 场景的实测结果（每次 `all` 都复核一遍，id `6d`）

```
[PASS] 6d 四主题壳区像素基线复核（阈值来源：themebase 实测，判据见 SHELL_BASE 头注）
black share 0.9237 / maxShare 0.9437 shareOk=true；redness -4.33 vs 基线 -4.33 → delta 0 rednessOk=true；hex 111111
white share 0.9243 / maxShare 0.9443 shareOk=true；redness  3.72 vs 基线  3.72 → delta 0 rednessOk=true；hex ffffff
blue  share 0.9228 / maxShare 0.9428 shareOk=true；redness -16.76 vs 基线 -16.76 → delta 0 rednessOk=true；hex 111133
gray  share 0.9235 / maxShare 0.9435 shareOk=true；redness  4.47 vs 基线  4.47 → delta 0 rednessOk=true；hex 222222
```

即：**用另一条独立路径（`all` 的 themes 场景，同一张整页帧的另一个裁剪）复现了标定值，四个主题 delta 全为 0**。原始样本见 [`themes/shell-baselines-live.json`](themes/shell-baselines-live.json)。

受影响的其它场景实测（两次逐字相同）：

* `offline`：主题 black，壳区主导色 `111111` 占 **0.9237** ≤ 本主题上限 0.9437 ⇒ drawn；截图 **51899 B**（判据串里现在写明「本主题 black 的上限 0.9437；来源 SHELL_BASE/themebase」）。
* `5a/5b`：无图 −4.33 → 有图 **50.45**（阈值 >17.58）⇒ pass；`5c`：清图 **−4.33**（阈值 <7.67）⇒ pass；`5e`：轮播红 50.45 / 蓝 −30.31 交替。
* `5d`：四档 fit 逐像素差异 0.0079 / 0.1466 / 0.1534 / 0.2621 / 0.2712（阈值 0.005），与上一轮证据逐字相同。

---

## 4. E9-①：audio-c 实测

### 4.1 真实音频链路，两轮全 pass

被测：LLM = deepseek-flash，TTS = 本机 CosyVoice3（24kHz / pcm），prompt = `请只回一句话，以句号结尾：今天天气不错。`。

| 轮次 | audio-a | audio-b | **audio-c** | 三路证据摘要 |
| --- | --- | --- | --- | --- |
| pass1 | pass（102 帧 / start 1 / end 1 / seq=[1] 严格递增） | pass（48960 样本，muted=false） | **pass** | A：12 次轮询，blob + duration **2.04** + delta **0.324s**；B：8 条事件（play() 1 次、removed 0）；C：1 条属性 + 9 条播放器事件；载荷 97920 B |
| pass2 | pass（同上） | pass（同上） | **pass** | A：10 次轮询，delta **0.51s**；B：**9 条事件**，delta **0.42s**；C：0 条；载荷 97920 B |

**两轮共 6 条音频判定，0 条 blocked、0 条 fail。** 原始：[`audio/probe-audio-pass1.log.txt`](audio/probe-audio-pass1.log.txt)、[`audio/probe-audio-pass2.log.txt`](audio/probe-audio-pass2.log.txt)、[`audio/results-audio-pass1.json`](audio/results-audio-pass1.json)、[`audio/results-audio-pass2.json`](audio/results-audio-pass2.json)。

**如实标注**：这两轮里元素**都没有被销毁**（`removed=0`），所以它们证明的是「三路取证下稳定拿到可判读帧」，**不是**「已经实测到销毁场景」；销毁场景由下面的受控自证覆盖。

### 4.2 受控自证 `mediaspy`（覆盖「元素已销毁」这一形态）

```
[PASS] mediaspy-a 元素从 DOM 移除后，页内记录器仍保住 currentTime 前进量（>0.2s）
       注入 duration=1.2s readyState=4 playErr=null；移除前 DOM 看到 1 个 / 移除后 0 个
       （移除时 currentTime=0.663667）；记录器 events=12 条，前进量=[{id:id:1,samples:12,blob:true,
       duration:1.2,firstAt:6754,lastAt:7474,firstTime:0,lastTime:0.664068,deltaSec:0.664}]
[PASS] mediaspy-b 阴性对照：不注入记录器时，元素移除后证据确实丢失（证明上一条不是白给）
       对照页无记录器（window.__audio=null）；移除后 DOM 看到 0 个元素、可用前进量 []
       ⇒ 证据确实丢失（旧实现的 blocked 形态）
```

原始：[`audio/results-mediaspy-latest.json`](audio/results-mediaspy-latest.json)、[`audio/mediaspy-selftest-raw.json`](audio/mediaspy-selftest-raw.json)。
（`mediaspy` 不进 `all`：它是机制自证，不需要端点，但也不代表产品链路。）

---

## 5. 验收：`all` 连跑两遍（冻结脚本 `302dc9cf…`）

两次都在同一个 HEAD `cc68f061`、同一个探针 sha256 `302dc9cf…` 下跑（**日志自报哈希**，见 §2.3）。

| 轮次 | 汇总时间（UTC） | 项数 | pass | manual-only | fail | blocked |
| --- | --- | --- | --- | --- | --- | --- |
| pass 1 | 2026-10-06T13:11:56Z | 40 | 36 | 4 | **0** | **0** |
| pass 2 | 2026-10-06T13:19:41Z | 40 | 36 | 4 | **0** | **0** |

原始汇总行（两次逐字相同）：

```
# PROBE sha256 302dc9cfa18b558e5e753d980e7261264546a33a1331b41b8fd4c6a8d6953b74（scripts/browser_probe.mjs 自身）
# 合计 40 项，fail 0 项（来自 12 个场景结果文件）→ run.json
```

**逐条一致性**（[`all-two-pass-frozen-comparison.json`](all-two-pass-frozen-comparison.json)）：

* 40 个 id 两次都在；**判定 0 处不同**（`verdictDiff = []`）；
* 7 条 `evidence` 字符串不同（`5a/5b/5c/offline/render-a/3b/3c`），token 级 diff（[`all-two-pass-token-diff.json`](all-two-pass-token-diff.json)）显示**差异全部是计时/FPS**：`waitForPaint 590/610ms`、`48.4/47.9`、`2.6ms/3.0ms`、`fps1=60/60.1`；
* **判定字段（redness / share / 截图字节 / diff 比例 / IndexedDB 键 / 样本数）两次逐字相同**。

4 条 manual-only（设计如此，不是掩盖）：`1d`（跨源分类，只打印）、`3d` 与 `6c-pixel`（无头 swiftshader **拍不到 WebGPU canvas**，见 `stage/stage-canvas-white-evidence.json` 的当场证明）、`5g`（ReorderableListView 拖动手柄命中点未知，需人工）。

8b（前端产物新鲜度）**真绿**原始输出：

```
[PASS] 8b 前端产物新鲜度（main.dart.js 不早于最新的 lib/*.dart） —
       main.dart.js=2026-10-06T12:36:38.677Z；最新 dart=2026-10-06T11:40:22.892Z
       shell/flutter/lib/app/shell_lifecycle_wiring.dart；dart 文件 131 个
```

---

## 6. 前端产物重建（Lead 裁决 (B)：本任务为本轮 `shell/flutter/build/web` 唯一写者）

`8b` 在重建前**确实是红的**（不是假绿灯，是产物真的过期）：E1 拆分改了 114 个 `lib/**/*.dart`（mtime 2026-10-06 19:32/19:40），而 `main.dart.js` 还是 10-05 21:50 的产物。

```bash
cd shell/flutter && export PATH=$HOME/flutter/bin:$PATH && \
  flock /tmp/l2d-heavy.lock flutter build web --release --base-href /app/ --no-web-resources-cdn
```
构建输出（节选）：`✓ Built build/web`（`Compiling lib/main.dart for the Web... 26.1s`）。
产物 mtime：`main.dart.js = 2026-10-06 20:36:38.677891492 +0800（3 290 723 B）`、`index.html = 20:36:14`；`find lib -name '*.dart' -newer main.dart.js | wc -l` = **0**。

产物级离线门禁（在仓库根跑）：

```
$ flock /tmp/l2d-heavy.lock ./scripts/ignite.sh --check-dir shell/flutter/build/web
==> 产物离线体检：shell/flutter/build/web（--check-dir；不需要服务）
    [ok]   shell/flutter/build/web/index.html 不依赖 Google CDN
    [ok]   shell/flutter/build/web/main.dart.js 不依赖 Google CDN
    [ok]   shell/flutter/build/web/flutter_bootstrap.js "useLocalCanvasKit":true（不落 Google CDN 分支）
    [ok]   shell/flutter/build/web/flutter.js 仍读 useLocalCanvasKit（决策由 flutter_bootstrap.js 的配置给出）
    （CDN 探针共扫 4 个产物文件）
==> 产物体检通过（未发现 Google CDN CanvasKit 依赖）
exit=0
```

**没有为变绿放宽任何判据**：`8b` 的判据（main.dart.js 的 mtime ≥ 最新的 lib/*.dart）一行未改。

---

## 7. 未做 / 已知边界（不粉饰）

1. **`audio` 不进 `all`**（需要活端点 LLM+TTS，且会真发一轮对话）——与 AGENTS「端到端探针只在有活端点时跑、缺配置明确跳过」同一条纪律。本轮 `audio` 单独跑了 2 次。
2. **CDP Media 域（C 路）本轮没有成为判定依据**：pass1 拿到 1 条属性事件（无前进量）、pass2 拿到 0 条——因为一旦 A/B 路满足判据，采样循环立刻 break，Media 域没机会积累属性变化。它是**存在且已接线**的独立佐证路径，但「只靠 C 路也能判 pass」**未实测**。要把这条补齐，需要单独跑一轮「屏蔽 A/B 路」的实验（本轮未做）。
3. **audio-c 的「元素已销毁」形态**由 `mediaspy` 受控自证覆盖，**不是**在真实 LLM/TTS 链路上撞到的（两轮 `removed=0`）。
4. **舞台（WebGPU canvas）像素仍然拍不到**：`3d`/`6c-pixel` 仍 manual-only，本轮的修复只保证**壳区**那一半的截图可信。
5. **白主题「平帧白色」不可判读**（`blankFrame` 返回 `'unknown'` → blocked）：这是刻意保留的保守判定，不是缺陷；要区分需要另找判据（本轮未做）。
6. **`themebase` 的「建议值」不自动写回源码**：脚本只打印 + 落 JSON，阈值由人读数据后钉进常量（脚本替人做判断 = 另一种自证）。
7. 本轮**未跑**：`verify_core_chain.py`、`flutter analyze/test`、cargo 全套（都不是本任务写面；`gate-verifier` 在跑）。**不冒充已跑。**

---

## 8. 文件清单（本目录）

| 文件 | 内容 |
| --- | --- |
| `README.md` | 本文件 |
| `probe-all-frozen-pass1.log.txt` / `probe-all-frozen-pass2.log.txt` | 两次 `all` 的**完整控制台原文**（含 `# PROBE sha256`） |
| `run-all-frozen-pass1.json` / `run-all-frozen-pass2.json` | 两次 `all` 结束时的 run.json 快照（各 40 项） |
| `probe-all-pass1.log.txt` / `probe-all-pass2.log.txt` / `run-all-pass1.json` / `run-all-pass2.json` / `all-two-pass-comparison.json` / `all-two-pass-token-diff.json` | 重建后**第一对** `all` 的证据（脚本当时尚未加入 `mediaspy`，其余逻辑与冻结版相同；判定同上 fail 0） |
| `all-two-pass-frozen-comparison.json` | 冻结脚本两次 `all` 的逐条对照（判定差 0，7 条计时差） |
| `themes/shell-baselines.json` | **E9-② 原始标定样本**（4 主题 × (3+1) 次，含 dominant/share/redness/channels/bytes） |
| `themes/shell-baselines-live.json` | `themes` 场景每次 `all` 的实时复核样本（4 行） |
| `themes/base-*.png` / `themes/stage-*.png` / `themes/panel-*.png` | 标定与主题场景的原始截图 |
| `audio/probe-audio-pass{1,2}.log.txt` / `audio/results-audio-pass{1,2}.json` | 两轮真实音频链路的原文与判定 |
| `audio/results-mediaspy-latest.json` / `audio/mediaspy-selftest-raw.json` | E9-① 受控自证（含阴性对照）的原文与原始快照 |
| `audio/ws-frames.json` / `audio/media-elements.json` | 最后一轮 audio 的原始帧投影与三路媒体证据 |
| `bg/` `stage/` `settings/` `fonts/` `ui/` `stage-class/` `console.json` `net.json` `render.json` `api.json` … | 各场景原始证据（与上一轮同名同义） |
| `themebase-results.json` | `themebase` 场景的判定记录（**刻意用非 `results-*` 文件名**，不参与 run.json 聚合） |

> 备注：`run.json`（本目录）在最后一次跑 `all` 时被覆盖，只反映 pass 2 + 场景结果文件的聚合；两次的**快照**是 `run-all-frozen-pass*.json`。
